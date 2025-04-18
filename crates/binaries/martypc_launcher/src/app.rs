/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2025 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the “Software”),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.

    --------------------------------------------------------------------------
*/
use crate::{
    emulator::{mouse_state::MouseData, Emulator},
    emulator_builder::EmulatorBuilder,
    event_loop::thread_events::handle_thread_event,
    timestep_update::process_update,
    MARTY_ICON,
};

use marty_egui_eframe::{context::GuiRenderContext, EGUI_MENU_BAR_HEIGHT};
use marty_frontend_common::{
    color::MartyColor,
    constants::NORMAL_NOTIFICATION_TIME,
    thread_events::*,
    timestep_manager::TimestepManager,
    GuiContextOptions,
};
use marty_web_helpers::FetchResult;

#[cfg(feature = "use_winit")]
use crate::event_loop::winit_events::handle_window_event;

#[cfg(feature = "use_wgpu")]
use eframe::egui_wgpu;

#[cfg(not(feature = "use_winit"))]
use crate::event_loop::web_keyboard::handle_web_key_event;

use crossbeam_channel::{Receiver, Sender};

use crate::emulator_builder::builder::EmuBuilderError;
#[cfg(target_arch = "wasm32")]
use crate::wasm::*;
use egui::{Context, CursorGrab, RawInput, Sense, ViewportCommand, ViewportId};
use gilrs::Gilrs;

#[cfg(target_arch = "wasm32")]
use marty_web_helpers::console_writer::ConsoleWriter;
#[cfg(target_arch = "wasm32")]
use url::Url;
// Grab mode. Must be "Locked" on web and macOS, "Confined" on Windows and Linux.

#[cfg(any(target_arch = "wasm32", target_os = "macos"))]
pub const GRAB_MODE: CursorGrab = CursorGrab::Locked;
#[cfg(not(any(target_arch = "wasm32", target_os = "macos")))]
pub const GRAB_MODE: CursorGrab = CursorGrab::Confined;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct MartyApp {
    current_size: egui::Vec2,
    last_size: egui::Vec2,
    focused: bool,
    hide_menu: bool,
    menu_height: f32,
    #[serde(skip)]
    gui: GuiRenderContext,
    #[serde(skip)]
    emu_loading: bool,
    #[serde(skip)]
    emu_receiver: Receiver<FetchResult>,
    #[serde(skip)]
    emu_sender: Sender<FetchResult>,
    #[cfg(feature = "use_winit")]
    #[serde(skip)]
    winit_receiver: Option<Receiver<(winit::window::WindowId, winit::event::WindowEvent)>>,
    #[cfg(not(feature = "use_winit"))]
    #[serde(skip)]
    web_receiver: Option<Receiver<eframe::WebKeyboardEvent>>,
    #[serde(skip)]
    pub emu: Option<Emulator>,
    #[serde(skip)]
    tm: TimestepManager,
}

impl Default for MartyApp {
    fn default() -> Self {
        let (sender, receiver) = crossbeam_channel::bounded(1);

        Self {
            hide_menu: false,
            current_size: egui::Vec2::ZERO,
            last_size: egui::Vec2::INFINITY,
            focused: false,
            menu_height: 22.0,
            // Example stuff:
            gui: GuiRenderContext::default(),
            emu_loading: false,
            emu_receiver: receiver,
            emu_sender: sender,
            #[cfg(feature = "use_winit")]
            winit_receiver: None,
            #[cfg(not(feature = "use_winit"))]
            web_receiver: None,
            emu: None,
            tm: TimestepManager::default(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
type MartyAppNewOptions = eframe::NativeOptions;

#[cfg(target_arch = "wasm32")]
type MartyAppNewOptions = ();

impl MartyApp {
    /// We split app initialization into two parts, since we can't make the callback eframe passes
    /// the creation context to async. So we first create the app, then let eframe call `init` with
    /// the partially initialized app - it should have the emulator built by then.
    pub async fn new(native_options: &mut MartyAppNewOptions) -> Self {
        // Build the emulator.
        let mut emu_builder = EmulatorBuilder::default();
        let emu_result;

        // Create the emulator immediately on native as we don't need to await anything
        #[cfg(not(target_arch = "wasm32"))]
        {
            emu_builder = emu_builder.with_toml_config_path("./martypc.toml");
            emu_result = emu_builder.build(&mut std::io::stdout(), &mut std::io::stderr()).await;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let base_url = get_base_url();
            let relative_config_url = base_url
                .join("/configs/martypc.toml")
                .expect("Failed to create relative config URL");

            let relative_manifest_url = base_url
                .join("/configs/file_manifest.toml")
                .expect("Failed to create relative manifest URL");

            log::debug!("Attemping to build emulator with config and manifest urls...");
            emu_builder = emu_builder
                .with_toml_config_url(&relative_config_url)
                .with_toml_manifest_url(&relative_manifest_url)
                .with_base_url(&base_url);

            emu_result = emu_builder.build(&mut std::io::stdout(), &mut std::io::stderr()).await;
        }

        // When the user runs our eframe app from a file browser, they typically will not get a
        // console window. So use rfd here to show some message boxes to tell them what failed.
        let mut emu = match emu_result {
            Ok(emu) => emu,
            Err(e) => {
                log::error!("Failed to build emulator: {}", e);
                let mut dialog = rfd::MessageDialog::new()
                    .set_title("Error initializing MartyPC!")
                    .set_level(rfd::MessageLevel::Error);

                let desc = match e {
                    EmuBuilderError::ConfigNotFound(filename) => {
                        format!("MartyPC couldn't find its main configuration file, '{filename}'!\n\
                        Marty typically looks for this file in the current directory, unless you have specified a location with the '--configfile' argument.\n\
                        If have built from source, make sure you are running MartyPC from the /install directory in the source tree.\n\
                        MartyPC needs various configuration files from there to run!")
                    }
                    EmuBuilderError::ConfigIOError(filename, e) => {
                        format!("MartyPC encountered an I/O error while trying to read its main configuration file, '{filename}]'\n\
                        Make sure it isn't open in another program, and that you have permission to read it.\n\n\
                        The error reported was:\n{e}")
                    }
                    EmuBuilderError::ConfigParseError(filename, e) => {
                        format!("MartyPC encountered an error while trying to parse the TOML of its main configuration file, '{filename}'!\n\
                        It is likely that you made a typo in the file, it is corrupted, or you used --configfile with the wrong file.\n\n\
                        The error reported was:\n{e}")
                    }
                    EmuBuilderError::UnsupportedPlatform(_) => e.to_string(),
                    EmuBuilderError::AudioDeviceError(e) => {
                        format!("MartyPC failed to initialize an audio device!\n\
                        This could be due to another program or process using your audio device in exclusive mode, or the device did not support the requested parameters.\n\
                        If you are unable to use a sound device, you can still run MartyPC by passing the --no_sound argument to MartyPC.\n\n\
                        The error reported was:\n{e}")
                    }
                    EmuBuilderError::AudioStreamError(e) => {
                        format!("MartyPC was able to open your audio device, but failed to initialize an audio stream!\n\
                        This could be due to another program or process using your audio device in exclusive mode, or the device did not support the requested parameters.\n\
                        If you are unable to use a sound device, you can still run MartyPC by passing the --no_sound argument to MartyPC.\n\n\
                        The error reported was:\n{e}")
                    }
                    EmuBuilderError::ValidatorNotSpecified => e.to_string(),
                    EmuBuilderError::NoResourcePaths => {
                        "MartyPC was unable to get all resource paths from the main configuration!\n\
                        If you have modified the configuration, please make sure you have defined all the necessary resource paths.".to_string()
                    }
                    EmuBuilderError::ResourceError(e) => {
                        format!("MartyPC encountered an error while trying to scan resource paths!\n\
                        MartyPC uses resource paths specified in the main configuration file to know where to look for machine configurations, \
                        ROMs, disk images, and other required resources.\n\
                        Make sure you are running MartyPC from within a valid distribution directory, or check your configuration.\n\n\
                        The error reported was:\n{e}")
                    }
                    EmuBuilderError::MachineConfigError(e) => {
                        format!("MartyPC encountered an error scanning for Machine Configuration files!\n\
                        At least one valid machine configuration TOML file must be present in /configs/machines for MartyPC to run.\n\n\
                        The error reported was:\n{e}")
                    }
                    EmuBuilderError::BadMachineConfig(e) =>{
                        format!("MartyPC encountered an error reading its Machine Configuration files!\n\
                        The specified machine configuration could not be found:\n\n\
                        '{e}'")
                    }
                    EmuBuilderError::IOError(e) => e.to_string(),
                    EmuBuilderError::Other(e) => e.to_string(),
                };

                dialog.set_description(desc).show();

                return MartyApp::default();
            }
        };

        // Apply configuration to emulator.
        match emu.apply_config() {
            Ok(_) => {
                log::debug!("Successfully applied configuration to Emulator state");
            }
            Err(e) => {
                log::error!("Failed to apply configuration to Emulator state: {}", e);
            }
        }

        // Create Timestep Manager
        let mut timestep_manager = TimestepManager::new();
        timestep_manager.set_cpu_mhz(emu.machine.get_cpu_mhz());

        // Set eframe's NativeOptions for fullscreen if specified by config
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(window) = emu.config.emulator.window.get_mut(0) {
            if window.fullscreen {
                native_options.viewport.inner_size = None;
                native_options.viewport.fullscreen = Some(true);
            }
        }

        MartyApp {
            emu: Some(emu),
            tm: timestep_manager,
            ..Default::default()
        }
    }

    /// Called once before the first frame.
    pub fn init(mut self, cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        // if let Some(storage) = cc.storage {
        //     return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        // }

        egui_extras::install_image_loaders(&cc.egui_ctx);

        let mut emu = self.emu.take().expect("Emulator should have been Some, but was None");

        // Apply fullscreen configuration now (doesn't seem to work applying to NativeOptions in new())

        if let Some(window) = emu.config.emulator.window.get_mut(0) {
            let _ = &cc
                .egui_ctx
                .send_viewport_cmd(ViewportCommand::Fullscreen(window.fullscreen));
        }

        // Get a list of video devices from machine.
        let cardlist = emu.machine.bus().enumerate_videocards();

        // Find the maximum refresh rate of all video cards
        let mut highest_rate = 50.0;
        for card in cardlist.iter() {
            let rate = emu.machine.bus().video(&card).unwrap().get_refresh_rate();
            if rate > highest_rate {
                highest_rate = rate;
            }
        }

        self.tm.set_emu_update_rate(highest_rate);
        self.tm.set_emu_render_rate(highest_rate);

        self.hide_menu = if emu.config.emulator.demo_mode {
            true
        }
        else {
            emu.config.gui.disabled
        };

        // TODO: Re-implement this stuff?
        // Create GUI parameters for the Display Manager.
        // let gui_options = DmGuiOptions {
        //     enabled: !emu.config.gui.disabled,
        //     theme: emu.config.gui.theme,
        //     menu_theme: emu.config.gui.menu_theme,
        //     menubar_h: EGUI_MENU_BAR_HEIGHT, // ignored on eframe
        //     zoom: emu.config.gui.zoom.unwrap_or(1.0),
        //     debug_drawing: false,
        // };

        // Create event receivers - for winit, we have a hook in egui_winit to receive raw
        // WindowEvents. For web we have a hook in eframe to receive custom WebKeyboardEvents,
        // which are Send + Sync copies of the raw web_sys::KeyboardEvent.
        #[cfg(feature = "use_winit")]
        let winit_receiver = {
            let (winit_sender, winit_receiver) = crossbeam_channel::unbounded();
            egui_winit::install_window_event_hook(winit_sender);
            winit_receiver
        };
        #[cfg(not(feature = "use_winit"))]
        let web_receiver = {
            let (web_sender, web_receiver) = crossbeam_channel::unbounded();
            eframe::install_keyboard_event_hook(web_sender);
            web_receiver
        };

        let gui_options = GuiContextOptions {
            enabled: !emu.config.gui.disabled,
            theme: emu.config.gui.theme,
            menu_theme: emu.config.gui.menu_theme,
            menubar_h: EGUI_MENU_BAR_HEIGHT, // ignored on eframe
            zoom: emu.config.gui.zoom.unwrap_or(1.0),
            debug_drawing: false,
        };

        // Create our GUI rendering context.
        let gui = GuiRenderContext::new(cc.egui_ctx.clone(), 0, 640, 480, 1.0, &gui_options);

        Self {
            gui,
            emu: Some(emu),

            #[cfg(feature = "use_winit")]
            winit_receiver: Some(winit_receiver),
            #[cfg(not(feature = "use_winit"))]
            web_receiver: Some(web_receiver),
            ..self
        }
    }

    pub fn viewport_resized(new_width: u32, new_height: u32) {
        let (adjust_x, adjust_y) = (0, 0);
        if new_width > 0 && new_height > 0 {
            // if let Err(e) = dm.on_viewport_resized(
            //     ViewportId::ROOT,
            //     new_width.saturating_sub(adjust_x),
            //     new_height.saturating_sub(adjust_y),
            // ) {
            //     log::error!("Failed to resize window: {}", e);
            // }
        }
        else {
            log::debug!("Ignoring invalid size: {}x{}", new_width, new_height);
            return;
        }
    }
}

impl eframe::App for MartyApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    /// A display manager must be created before this is called.
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Enumerate the host's gamepads if the feature is enabled
        #[cfg(feature = "use_gilrs")]
        let mut gilrs = Gilrs::new().unwrap();
        log::debug!("Enumerating {} gamepads...", gilrs.gamepads().count());
        for (_id, gamepad) in gilrs.gamepads() {
            log::debug!("Found gamepad: {:?}", gamepad.name());
        }

        // Get current viewport focus state.
        let vi = ctx.input(|i| {
            let vi = i.viewport();
            if let Some(focus) = vi.focused {
                if self.focused && !focus {
                    log::debug!("MartyApp::update(): Main viewport lost focus");

                    // Clear keyboard state when losing focus to avoid stuck keys.
                    // We will not receive the key up events if we lose focus while a key is pressed,
                    // and this will cause any key pressed when focus is lost to be stuck down forever.
                    if let Some(emu) = &mut self.emu {
                        if let Some(kb) = emu.machine.bus_mut().keyboard_mut() {
                            log::debug!("MartyApp::update(): Clearing keyboard on focus loss.");
                            kb.clear(true);
                        }
                    }
                    self.focused = false;
                }
                else if !self.focused && focus {
                    log::debug!("MartyApp::update(): Main viewport gained focus");
                    self.focused = true;
                }
            }
        });

        if let Some(emu) = &mut self.emu {
            self.current_size = ctx.screen_rect().size(); // Get window size

            if self.current_size != self.last_size {
                log::warn!("MartyApp::update(): Window resized to: {:?}", self.current_size);
                // MartyApp::viewport_resized(
                //     self.dm.as_mut().unwrap(),
                //     self.current_size.x as u32,
                //     self.current_size.y as u32 - self.menu_height as u32,
                // );
                self.last_size = self.current_size; // Update tracked size
            }

            // Receive hooked Winit events.
            #[cfg(feature = "use_winit")]
            if let Some(receiver) = &self.winit_receiver {
                for event in receiver.try_iter() {
                    log::trace!("Received winit event: {:?} from window id: {:?}", event.1, event.0);
                    // handle_window_event(
                    //     emu,
                    //     self.dm.as_mut().unwrap(),
                    //     ctx.clone(),
                    //     &mut self.tm,
                    //     event.0,
                    //     event.1,
                    //     self.focused,
                    //     ctx.memory(|mem| mem.focused()).is_some(),
                    // );
                }
            }

            // Receive hooked web_sys::KeyboardEvent events.
            #[cfg(not(feature = "use_winit"))]
            if let Some(receiver) = &self.web_receiver {
                for event in receiver.try_iter() {
                    log::trace!("Received web_sys event: {:?}", event);

                    handle_web_key_event(
                        emu,
                        self.dm.as_mut().unwrap(),
                        event,
                        ctx.memory(|mem| mem.focused()).is_some(),
                    );
                }
            }
            //
            // let dm = self.dm.as_mut().unwrap();
            // // Process timestep.
            // process_update(emu, dm, &mut self.tm);
            handle_thread_event(emu, ctx);

            // We can't access context in the closure below, so we need to set a flag to un-grab the mouse
            // afterward.
            let mut ungrab = false;
            ctx.input(|i| {});
        }

        // if let Some(dm) = &mut self.dm {
        //     // Present the render targets (this will draw windows for any GuiWidget targets).
        //     dm.for_each_surface(|backend, surface, scaler, gui| {
        //         //_ = backend.present();
        //     });
        // }

        // Pump the event loop by requesting a repaint every time.
        ctx.request_repaint();
    }

    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn raw_input_hook(&mut self, ctx: &Context, raw_input: &mut RawInput) {
        let gui_has_focus = ctx.wants_keyboard_input();

        //let gui_has_focus = ctx.memory(|mem| mem.focused()).is_some();

        // Suppress key events if the GUI doesn't explicitly have focus.
        if !gui_has_focus {
            raw_input.events.retain(|event| match event {
                egui::Event::Key { .. } => false,
                _ => true,
            });
        }
    }
}

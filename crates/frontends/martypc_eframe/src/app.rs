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
    emulator::Emulator,
    emulator_builder::EmulatorBuilder,
    event_loop::{egui_events::FileSelectionContext, thread_events::handle_thread_event},
    timestep_update::process_update,
    MARTY_ICON,
};

use display_manager_eframe::{
    builder::EFrameDisplayManagerBuilder,
    BufferDimensions,
    EFrameBackend,
    EFrameDisplayManager,
    TextureDimensions,
};
use marty_egui_eframe::{context::GuiRenderContext, EGUI_MENU_BAR_HEIGHT};
use marty_frontend_common::{
    display_manager::{DisplayManager, DmGuiOptions},
    timestep_manager::TimestepManager,
};
use marty_web_helpers::FetchResult;

#[cfg(feature = "use_winit")]
use crate::event_loop::winit_events::handle_window_event;

#[cfg(feature = "use_wgpu")]
use eframe::egui_wgpu;

#[cfg(not(feature = "use_winit"))]
use crate::event_loop::web_keyboard::handle_web_key_event;

use crossbeam_channel::{Receiver, Sender};

use egui::{Context, RawInput, Sense, ViewportId};

#[cfg(target_arch = "wasm32")]
use crate::wasm::*;
use marty_frontend_common::{
    color::MartyColor,
    display_manager::{DisplayTargetType, DtHandle},
};
use marty_videocard_renderer::AspectCorrectionMode;
#[cfg(target_arch = "wasm32")]
use marty_web_helpers::console_writer::ConsoleWriter;
#[cfg(target_arch = "wasm32")]
use url::Url;

#[derive(Clone, Debug)]
pub enum FileOpenContext {
    FloppyDiskImage { drive_select: usize, fsc: FileSelectionContext },
    CartridgeImage { slot_select: usize, fsc: FileSelectionContext },
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct MartyApp {
    current_size: egui::Vec2,
    last_size:    egui::Vec2,

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
    dm: Option<EFrameDisplayManager>,
    #[serde(skip)]
    tm: TimestepManager,
}

impl Default for MartyApp {
    fn default() -> Self {
        let (sender, receiver) = crossbeam_channel::bounded(1);

        Self {
            current_size: egui::Vec2::ZERO,
            last_size: egui::Vec2::INFINITY,
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
            dm: None,
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
    pub async fn new(_native_options: &mut MartyAppNewOptions) -> Self {
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

        let mut emu = match emu_result {
            Ok(emu) => emu,
            Err(e) => {
                log::error!("Failed to build emulator: {}", e);
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

        // Get a list of video devices from machine.
        let cardlist = emu.machine.bus().enumerate_videocards();

        // Find the maximum refresh rate of all video cards
        let mut highest_rate = 50;
        for card in cardlist.iter() {
            let rate = emu.machine.bus().video(&card).unwrap().get_refresh_rate();
            if rate > highest_rate {
                highest_rate = rate;
            }
        }

        self.tm.set_emu_update_rate(highest_rate);
        self.tm.set_emu_render_rate(highest_rate);

        // Create GUI parameters for the Display Manager.
        let gui_options = DmGuiOptions {
            enabled: !emu.config.gui.disabled,
            theme: emu.config.gui.theme,
            menu_theme: emu.config.gui.menu_theme,
            menubar_h: EGUI_MENU_BAR_HEIGHT, // TODO: Dynamically measure the height of the egui menu bar somehow
            zoom: emu.config.gui.zoom.unwrap_or(1.0),
            debug_drawing: false,
        };

        // Create DisplayManager.
        log::debug!("Creating DisplayManager...");
        let mut dm_builder = EFrameDisplayManagerBuilder::new();

        // If `use_wgpu` is set, we need to get the wgpu device and queue from the creation context, and
        // create a wgpu backend for the display manager.
        #[cfg(feature = "use_wgpu")]
        {
            if let Some(render_state) = &cc.wgpu_render_state {
                let wgpu_backend = match EFrameBackend::new(
                    cc.egui_ctx.clone(),
                    BufferDimensions {
                        w: 640,
                        h: 480,
                        pitch: 640,
                    },
                    TextureDimensions { w: 640, h: 480 },
                    render_state.device.clone(),
                    render_state.queue.clone(),
                    render_state.target_format,
                ) {
                    Ok(backend) => {
                        log::debug!(
                            "init(): Created wgpu backend, texture format: {:?}",
                            render_state.target_format
                        );
                        backend
                    }
                    Err(e) => {
                        log::error!("init(): Failed to create wgpu backend: {}", e);
                        return MartyApp::default();
                    }
                };
                log::debug!("init(): Installing wpgu backend");
                dm_builder = dm_builder.with_backend(wgpu_backend);
            }
            else {
                panic!("init(): use_wgpu feature enabled, but failed to get wgpu render state from eframe creation context");
            }
        }
        #[cfg(not(feature = "use_wgpu"))]
        {
            let egui_backend = match EFrameBackend::new(cc.egui_ctx.clone()) {
                Ok(backend) => {
                    log::debug!("init(): Created egui backend");
                    backend
                }
                Err(e) => {
                    log::error!("init(): Failed to create egui backend: {}", e);
                    return MartyApp::default();
                }
            };
            log::debug!("init(): Installing generic egui backend");
            dm_builder = dm_builder.with_backend(egui_backend);
        }

        dm_builder = dm_builder
            .with_egui_ctx(cc.egui_ctx.clone())
            .with_win_configs(&emu.config.emulator.window)
            .with_cards(cardlist)
            .with_scaler_presets(&emu.config.emulator.scaler_preset)
            .with_icon_buf(MARTY_ICON)
            .with_gui_options(&gui_options);

        let mut display_manager = match dm_builder.build() {
            Ok(dm) => dm,
            Err(e) => {
                log::error!("Failed to create display manager: {}", e);
                return MartyApp::default();
            }
        };

        // Set all DisplayTargets to hardware aspect correction
        display_manager.for_each_target(|dtc, _idx| {
            dtc.set_aspect_mode(AspectCorrectionMode::Hardware);
        });

        // Get a list of all cards
        let mut vid_list = Vec::new();
        display_manager.for_each_card(|vid| {
            vid_list.push(vid.clone());
        });

        // Resize each video card to match the starting display extents.
        for vid in vid_list.iter() {
            if let Some(card) = emu.machine.bus().video(vid) {
                let extents = card.get_display_extents();

                //assert_eq!(extents.double_scan, true);
                if let Err(_e) = display_manager.on_card_resized(vid, extents) {
                    log::error!("Failed to resize videocard!");
                }
            }
        }

        // Sort vid_list by index
        vid_list.sort_by(|a, b| a.idx.cmp(&b.idx));

        // Build list of cards to set in UI.
        let mut card_strs = Vec::new();
        for vid in vid_list.iter() {
            let card_str = format!("Card: {} ({:?})", vid.idx, vid.vtype);
            card_strs.push(card_str);
        }

        // -- Update GUI state with display info
        let dti = display_manager.display_info(&emu.machine);
        emu.gui.set_card_list(card_strs);
        emu.gui.init_display_info(dti);

        // Populate the list of display apertures for each display.
        display_manager.for_each_target(|dtc, dt_idx| {
            if let Some(card_id) = &dtc.get_card_id() {
                if let Some(video_card) = emu.machine.bus().video(card_id) {
                    emu.gui
                        .set_display_apertures(dt_idx, video_card.list_display_apertures());
                }
            }
        });

        // Insert floppies specified in config.
        match emu.insert_floppies(emu.sender.clone()) {
            Ok(_) => {
                log::debug!("Inserted floppies from config");
            }
            Err(e) => {
                log::error!("Failed to insert floppies from config: {}", e);
            }
        }

        // Attach VHD images specified in config.
        match emu.mount_vhds() {
            Ok(_) => {
                log::debug!("Mounted VHDs from config");
            }
            Err(e) => {
                log::error!("Failed to mount VHDs from config: {}", e);
            }
        }

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

        // Create our GUI rendering context.
        let gui = GuiRenderContext::new(cc.egui_ctx.clone(), 0, 640, 480, 1.0, &gui_options);

        Self {
            gui,
            dm: Some(display_manager),
            emu: Some(emu),

            #[cfg(feature = "use_winit")]
            winit_receiver: Some(winit_receiver),
            #[cfg(not(feature = "use_winit"))]
            web_receiver: Some(web_receiver),
            ..self
        }
    }

    pub fn viewport_resized(dm: &mut EFrameDisplayManager, new_width: u32, new_height: u32) {
        let (adjust_x, adjust_y) = (0, 0);
        if new_width > 0 && new_height > 0 {
            if let Err(e) = dm.on_viewport_resized(
                ViewportId::ROOT,
                new_width.saturating_sub(adjust_x),
                new_height.saturating_sub(adjust_y),
            ) {
                log::error!("Failed to resize window: {}", e);
            }
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
        if let Some(emu) = &mut self.emu {
            self.current_size = ctx.screen_rect().size(); // Get window size

            if self.current_size != self.last_size {
                log::warn!("MartyApp::update(): Window resized to: {:?}", self.current_size);
                MartyApp::viewport_resized(
                    self.dm.as_mut().unwrap(),
                    self.current_size.x as u32,
                    self.current_size.y as u32,
                );
                self.last_size = self.current_size; // Update tracked size
            }

            // Receive hooked Winit events.
            #[cfg(feature = "use_winit")]
            if let Some(receiver) = &self.winit_receiver {
                for event in receiver.try_iter() {
                    log::trace!("Received winit event: {:?} from window id: {:?}", event.1, event.0);
                    handle_window_event(
                        emu,
                        self.dm.as_mut().unwrap(),
                        &mut self.tm,
                        event.0,
                        event.1,
                        ctx.memory(|mem| mem.focused()).is_some(),
                    );
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

            let dm = self.dm.as_mut().unwrap();
            // Process timestep.
            process_update(emu, dm, &mut self.tm);
            handle_thread_event(emu);

            let fill_color = dm
                .main_display_target()
                .read()
                .unwrap()
                .viewport_opts
                .as_ref()
                .and_then(|vo| vo.fill_color)
                .and_then(|c| Some(MartyColor::from_u24(c).to_color32()));

            let show_bezel = emu.gui.primary_video_has_bezel();
            // Draw the emulator GUI.
            self.gui.show(
                &mut emu.gui,
                fill_color,
                |ctx| {
                    if let Some(DisplayTargetType::GuiWidget) = dm.display_type(DtHandle::MAIN) {
                        let dtc = dm.main_display_target();
                        let dtc_lock = dtc.read();
                        let dtc_ref = dtc_lock.as_ref().unwrap();

                        let display_name = dtc_ref.name.clone();
                        if let Some(scaler_geom) = dtc_ref.scaler_geometry() {
                            // Draw the main display in a window.
                            egui::Window::new(display_name).resizable(true).show(ctx, |ui| {
                                let ui_size = egui::Vec2::new(scaler_geom.target_w as f32, scaler_geom.target_h as f32);
                                let (rect, _) = ui.allocate_exact_size(ui_size, Sense::hover());

                                #[cfg(feature = "use_wgpu")]
                                {
                                    let callback = dm.main_display_callback();
                                    let paint_callback = egui_wgpu::Callback::new_paint_callback(rect, callback);

                                    ui.painter().add(paint_callback);

                                    if show_bezel {
                                        egui::Image::new(egui::include_image!("../../../../assets/bezel_trans_bg.png"))
                                            .paint_at(ui, rect);
                                    }
                                }
                                #[cfg(feature = "use_glow")]
                                {
                                    let dtc_lock = dm.main_display_target();
                                    let dtc = dtc_lock.read().unwrap();
                                    let surface = dtc.surface().unwrap();
                                    let texture = surface.read().unwrap().backing_texture();
                                    let uv_rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                                    log::debug!(
                                        "Drawing main display with glow: {}x{}",
                                        texture.size()[0],
                                        texture.size()[1]
                                    );
                                    ui.painter().image(texture.id(), rect, uv_rect, egui::Color32::WHITE);

                                    // let _ = dm.with_surface_mut(DtHandle::MAIN, |backend, surface| {
                                    //     let texture = surface.read().unwrap().backing_texture();
                                    //     let uv_rect =
                                    //         egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                                    //     ui.painter().image(texture.id(), rect, uv_rect, Color32::WHITE);
                                    // });
                                }
                            });
                        }
                        else {
                            log::warn!("No scaler geometry for main display!");
                        }
                    }
                },
                |ui| {
                    if let Some(DisplayTargetType::WindowBackground) = dm.display_type(DtHandle::MAIN) {
                        ui.allocate_ui(ui.available_size(), |ui| {
                            let rect = ui.max_rect();

                            //log::debug!("in allocate_ui with response rect: {:?}", rect);

                            #[cfg(feature = "use_wgpu")]
                            {
                                let callback = dm.main_display_callback();
                                let paint_callback = egui_wgpu::Callback::new_paint_callback(rect, callback);
                                ui.painter().add(paint_callback);
                            }
                        });
                    }
                },
            );
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

/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2024 Daniel Balsom

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

use display_manager_eframe::{DisplayBackend, EFrameDisplayManager, EFrameDisplayManagerBuilder};
use frontend_common::{
    display_manager::{DisplayManager, DmGuiOptions},
    timestep_manager::TimestepManager,
};
use marty_egui_eframe::{context::GuiRenderContext, EGUI_MENU_BAR_HEIGHT};
use marty_web_helpers::FetchResult;
use std::{env, pin::Pin, task::Poll};

use crossbeam_channel::{Receiver, Sender};

#[cfg(target_arch = "wasm32")]
use crate::wasm::*;
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
    // Example stuff:
    #[serde(skip)]
    gui: GuiRenderContext,
    #[serde(skip)]
    emu_loading: bool,
    #[serde(skip)]
    emu_receiver: Receiver<FetchResult>,
    #[serde(skip)]
    emu_sender: Sender<FetchResult>,
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
            // Example stuff:
            gui: GuiRenderContext::default(),
            emu_loading: false,
            emu_receiver: receiver,
            emu_sender: sender,
            emu: None,
            dm: None,
            tm: TimestepManager::default(),
        }
    }
}

impl MartyApp {
    /// We split app initialization into two parts, since we can't make the callback eframe passes
    /// the creation context to async.  So we first create the app, then let eframe call `init` with
    /// the partially initialized app - it should have the emulator built by then.
    pub async fn new() -> Self {
        // Build the emulator.
        let mut emu_builder = EmulatorBuilder::default();
        let mut emu_result;

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

        let emu = match emu_result {
            Ok(emu) => emu,
            Err(e) => {
                log::error!("Failed to build emulator: {}", e);
                return MartyApp::default();
            }
        };

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

        let emu = self.emu.take().expect("Emulator should have been Some, but was None");

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
            menubar_h: 24, // TODO: Dynamically measure the height of the egui menu bar somehow
            zoom: emu.config.gui.zoom.unwrap_or(1.0),
            debug_drawing: false,
        };

        // Create display targets.
        let display_manager = match EFrameDisplayManagerBuilder::build(
            cc.egui_ctx.clone(),
            &emu.config.emulator.window,
            cardlist,
            &emu.config.emulator.scaler_preset,
            None,
            Some(MARTY_ICON),
            &gui_options,
        ) {
            Ok(dm) => dm,
            Err(e) => {
                log::error!("Failed to create display manager: {}", e);
                return MartyApp::default();
            }
        };

        let gui_options = DmGuiOptions {
            enabled: !emu.config.gui.disabled,
            theme: emu.config.gui.theme,
            menu_theme: emu.config.gui.menu_theme,
            menubar_h: EGUI_MENU_BAR_HEIGHT,
            zoom: 1.0,
            debug_drawing: false,
        };

        Self {
            gui: GuiRenderContext::new(cc.egui_ctx.clone(), 0, 640, 480, 1.0, &gui_options),
            dm: Some(display_manager),
            emu: Some(emu),
            ..self
        }
    }
}

impl eframe::App for MartyApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(emu) = &mut self.emu {
            // Process timestep.
            process_update(emu, &mut self.dm.as_mut().unwrap(), &mut self.tm);
            handle_thread_event(emu);

            // Draw the emulator GUI.
            self.gui.show(&mut emu.gui);
        }

        if let Some(dm) = &mut self.dm {
            // Present the render targets (this will draw windows for any GuiWidget targets).
            dm.for_each_backend(|backend, scaler, gui| {
                _ = backend.present();
            });
        }

        // Pump the event loop by requesting a repaint every time.
        ctx.request_repaint();
    }

    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }
}

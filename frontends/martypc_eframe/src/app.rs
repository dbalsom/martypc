use crate::{
    emulator::Emulator,
    emulator_builder::EmulatorBuilder,
    event_loop::thread_events::handle_thread_event,
    native::startup,
    timestep_update::process_update,
    MARTY_ICON,
};
use display_manager_eframe::{DisplayBackend, EFrameDisplayManager, EFrameDisplayManagerBuilder};
use frontend_common::{
    display_manager::{DisplayManager, DmGuiOptions},
    timestep_manager::TimestepManager,
};
use marty_egui_eframe::{context::GuiRenderContext, EGUI_MENU_BAR_HEIGHT};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct MartyApp {
    // Example stuff:
    #[serde(skip)]
    gui: GuiRenderContext,
    #[serde(skip)]
    emu: Option<Emulator>,
    #[serde(skip)]
    dm:  Option<EFrameDisplayManager>,
    #[serde(skip)]
    tm:  TimestepManager,
}

impl Default for MartyApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            gui: GuiRenderContext::default(),
            emu: None,
            dm:  None,
            tm:  TimestepManager::default(),
        }
    }
}

impl MartyApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        // if let Some(storage) = cc.storage {
        //     return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        // }

        egui_extras::install_image_loaders(&cc.egui_ctx);

        // Build the emulator.
        let mut emu_builder = EmulatorBuilder::default();
        let mut emu_result;

        #[cfg(not(target_arch = "wasm32"))]
        {
            emu_builder = emu_builder.with_toml_config_path("./marty.toml");
            emu_result = emu_builder.build(&mut std::io::stdout(), &mut std::io::stderr());
        }
        #[cfg(target_arch = "wasm32")]
        {
            emu_builder = emu_builder.with_toml_config_url("./marty.toml");
            emu_result = emu_builder.build(&mut ConsoleWriter::default(), &mut ConsoleWriter::default());
        }

        let emu = match emu_result {
            Ok(emu) => emu,
            Err(e) => {
                log::error!("Failed to build emulator: {}", e);
                return MartyApp::default();
            }
        };

        // Create Display manager

        // Create GUI parameters for the Display Manager.
        let gui_options = DmGuiOptions {
            enabled: !emu.config.gui.disabled,
            theme: emu.config.gui.theme,
            menu_theme: emu.config.gui.menu_theme,
            menubar_h: 24, // TODO: Dynamically measure the height of the egui menu bar somehow
            zoom: emu.config.gui.zoom.unwrap_or(1.0),
            debug_drawing: false,
        };

        // Query the machine for video output devices
        let cardlist = emu.machine.bus().enumerate_videocards();

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

        // Create Timestep Manager
        let mut timestep_manager = TimestepManager::new();
        timestep_manager.set_cpu_mhz(emu.machine.get_cpu_mhz());

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

        timestep_manager.set_emu_update_rate(highest_rate);
        timestep_manager.set_emu_render_rate(highest_rate);

        let gui_options = DmGuiOptions {
            enabled: !emu.config.gui.disabled,
            theme: emu.config.gui.theme,
            menu_theme: emu.config.gui.menu_theme,
            menubar_h: EGUI_MENU_BAR_HEIGHT,
            zoom: 1.0,
            debug_drawing: false,
        };

        MartyApp {
            gui: GuiRenderContext::new(cc.egui_ctx.clone(), 0, 640, 480, 1.0, &gui_options),
            emu: Some(emu),
            dm:  Some(display_manager),
            tm:  timestep_manager,
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

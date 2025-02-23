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

   ---------------------------------------------------------------------------
*/

use std::path::PathBuf;

use crate::{DefaultResolver, DisplayBackend, EFrameDisplayManager};

use marty_core::device_traits::videocard::VideoCardId;
use marty_frontend_common::{
    display_manager::{DisplayManager, DisplayTargetMargins, DisplayTargetType, DmGuiOptions, DmViewportOptions},
    display_scaler::ScalerPreset,
    types::window::WindowDefinition,
};

// lib.rs should conditionally re-export the correct EFrameBackend for active features.
use super::EFrameBackend;

use anyhow::{anyhow, Error};
use display_backend_eframe_wgpu::DisplayTargetSurface;
use egui::{Context, ViewportId};
use winit::window::Icon;

#[derive(Default)]
pub struct EFrameDisplayManagerBuilder<'a> {
    egui_ctx: Context,
    backend: Option<EFrameBackend>,
    win_configs: Vec<WindowDefinition>,
    cards: Vec<VideoCardId>,
    scaler_presets: Vec<ScalerPreset>,
    icon_path: Option<PathBuf>,
    icon_buf: Option<&'a [u8]>,
    gui_options: Option<&'a DmGuiOptions>,
}

/// Display managers should be constructed via a [DisplayManagerBuilder]. This allows display targets
/// to be created as specified by a user-supplied configuration. For [EFrameDisplayManager], we build
/// our display targets using:
///
/// - the user configuration file
/// - a list of video cards from the emulator core
/// - a list of scaler preset definitions
/// - a path to an icon (TODO: support different icons per window?)
/// - a struct of GUI options
impl<'a> EFrameDisplayManagerBuilder<'a> {
    pub fn new() -> Self {
        EFrameDisplayManagerBuilder::default()
    }

    pub fn with_backend(mut self, backend: EFrameBackend) -> Self {
        self.backend = Some(backend);
        self
    }

    pub fn with_egui_ctx(mut self, egui_ctx: Context) -> Self {
        self.egui_ctx = egui_ctx;
        self
    }

    pub fn with_win_configs(mut self, win_configs: &[WindowDefinition]) -> Self {
        self.win_configs = win_configs.to_vec();
        self
    }

    pub fn with_cards(mut self, cards: Vec<VideoCardId>) -> Self {
        self.cards = cards;
        self
    }

    pub fn with_scaler_presets(mut self, scaler_presets: &[ScalerPreset]) -> Self {
        self.scaler_presets = scaler_presets.to_vec();
        self
    }

    pub fn with_icon_path(mut self, icon_path: Option<PathBuf>) -> Self {
        self.icon_path = icon_path;
        self
    }

    pub fn with_icon_buf(mut self, icon_buf: &'a [u8]) -> Self {
        self.icon_buf = Some(icon_buf);
        self
    }

    pub fn with_gui_options(mut self, gui_options: &'a DmGuiOptions) -> Self {
        self.gui_options = Some(gui_options);
        self
    }

    pub fn build(&mut self) -> Result<EFrameDisplayManager, Error> {
        let icon = {
            if let Some(path) = &self.icon_path {
                if let Ok(image) = image::open(path.clone()) {
                    log::debug!("Using icon from path: {}", path.display());
                    let rgba8 = image.into_rgba8();
                    let (width, height) = rgba8.dimensions();
                    let icon_raw = rgba8.into_raw();

                    let icon = winit::window::Icon::from_rgba(icon_raw.clone(), width, height)?;

                    Some(icon)
                }
                else {
                    log::error!("Couldn't load icon: {}", path.display());
                    log::error!("Couldn't load icon: {}", path.display());
                    None
                }
            }
            else {
                if let Some(buf) = self.icon_buf {
                    if let Ok(image) = image::load_from_memory(buf) {
                        let rgba8 = image.into_rgba8();
                        let (width, height) = rgba8.dimensions();
                        let icon_raw = rgba8.into_raw();

                        let icon = winit::window::Icon::from_rgba(icon_raw.clone(), width, height)?;

                        Some(icon)
                    }
                    else {
                        log::error!("Couldn't load icon from buffer.");
                        None
                    }
                }
                else {
                    log::warn!("No icon specified.");
                    None
                }
            }
        };

        let mut dm = EFrameDisplayManager::new();

        // Install the backend
        dm.backend = self.backend.take();

        // Sanity check - backend is some?
        if dm.backend.is_none() {
            return Err(anyhow!("EFrameDisplayManagerBuilder::build(): No backend specified!"));
        }

        // Install scaler presets
        for preset in self.scaler_presets.iter() {
            log::debug!(
                "EFrameDisplayManagerBuilder::build(): Installing scaler preset: {}",
                &preset.name
            );
            dm.add_scaler_preset(preset.clone());
        }

        // Only create windows if the config specifies any!
        if self.gui_options.is_some() && self.win_configs.len() > 0 {
            // Create the main window.
            Self::create_target_from_window_def(
                &mut dm,
                self.egui_ctx.clone(),
                true,
                &self.win_configs[0],
                &self.cards,
                self.gui_options.unwrap(),
                icon.clone(),
            )
            .expect("EFrameDisplayManagerBuilder::build(): FATAL: Failed to create a window target");

            // TODO: Reimplement this for egui Viewports

            // // Create the rest of the windows
            // for window_def in win_configs.iter().skip(1) {
            //     if window_def.enabled {
            //         Self::create_target_from_window_def(
            //             &mut dm,
            //             egui_ctx.clone(),
            //             false,
            //             &window_def,
            //             &cards,
            //             gui_options,
            //             icon.clone(),
            //         )
            //         .expect("FATAL: Failed to create a window target");
            //     }
            // }
        }

        Ok(dm)
    }

    pub fn create_target_from_window_def(
        dm: &mut EFrameDisplayManager,
        egui_ctx: Context,
        main_window: bool,
        window_def: &WindowDefinition,
        cards: &Vec<VideoCardId>,
        gui_options: &DmGuiOptions,
        icon: Option<Icon>,
    ) -> Result<(), Error> {
        let resolved_def = window_def.resolve_with_defaults();
        log::debug!("{:?}", window_def);

        let mut card_id_opt = None;
        let mut card_string = String::new();

        if let Some(w_card_id) = resolved_def.card_id {
            if w_card_id < cards.len() {
                card_id_opt = Some(cards[w_card_id]);
                card_string.push_str(&format!("{:?}", cards[w_card_id].vtype))
            }
            card_string.push_str(&format!("({})", w_card_id));
        }

        log::debug!(
            "Creating WindowBackground display target from window definition with card id: {:?}",
            card_id_opt
        );

        // TODO: Implement FROM for this?
        let mut viewport_opts: DmViewportOptions = Default::default();

        // Honor initial window size, but we may have to resize it later.
        viewport_opts.size = window_def.size.unwrap_or_default().into();
        viewport_opts.always_on_top = window_def.always_on_top;

        // If this is the main window, and we have a GUI...
        if main_window && gui_options.enabled {
            // Set the top margin to clear the egui menu bar.
            viewport_opts.margins = DisplayTargetMargins::from_t(gui_options.menubar_h);
        }

        // Is window resizable?
        if !window_def.resizable {
            viewport_opts.min_size = Some(viewport_opts.size);
            viewport_opts.max_size = Some(viewport_opts.size);
            viewport_opts.resizable = false;
        }
        else {
            viewport_opts.resizable = true;
        }

        // If this is Some, it locks the window resolution to some scale factor of card resolution
        viewport_opts.card_scale = window_def.card_scale;

        let preset_name = window_def.scaler_preset.clone().unwrap_or("default".to_string());

        // Construct window title.
        let window_title = format!("{}: {}", &window_def.name, card_string).to_string();

        let dt_type = DisplayTargetType::WindowBackground {
            main_window,
            has_gui: main_window,
            has_menu: main_window,
        };

        let dt_type = DisplayTargetType::GuiWidget {
            main_window,
            has_gui: main_window,
            has_menu: main_window,
        };

        dm.create_target(
            window_title,
            dt_type,
            Some(&egui_ctx),
            if main_window { Some(ViewportId::ROOT) } else { None },
            Some(viewport_opts),
            card_id_opt,
            preset_name,
            gui_options,
        )
        .expect("Failed to create window target!");

        let last_idx = dm.targets.len() - 1;

        // TODO: figure out how to set icon here
        //dm.targets[last_idx].window.as_mut().unwrap().set_window_icon(icon);

        Ok(())
    }
}

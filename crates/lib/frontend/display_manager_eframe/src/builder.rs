/*
   MartyPC
   https://github.com/dbalsom/martypc

   Copyright 2022-2026 Daniel Balsom

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

// lib.rs should conditionally re-export the correct EFrameBackend for active features.
use super::EFrameBackend;

use crate::{DefaultResolver, EFrameDisplayManager};

use marty_core::device_traits::videocard::VideoCardId;
use marty_display_common::{
    display_manager::{
        DisplayManager,
        DisplayTargetFlags,
        DisplayTargetMargins,
        DisplayTargetType,
        DmGuiOptions,
        DmViewportOptions,
    },
    display_scaler::ScalerPreset,
};
use marty_frontend_common::types::window::{
    CardDisplayTargetConfiguration,
    ScalerMode,
    WindowDefinition,
    MAX_DISPLAY_TARGETS,
};

use anyhow::{anyhow, Error};
use egui::{Context, ViewportId};

#[derive(Default)]
pub struct EFrameDisplayManagerBuilder<'a> {
    egui_ctx: Context,
    backend: Option<EFrameBackend>,
    win_configs: Vec<WindowDefinition>,
    display_target_configs: Vec<CardDisplayTargetConfiguration>,
    cards: Vec<VideoCardId>,
    scaler_presets: Vec<ScalerPreset>,
    icon_path: Option<PathBuf>,
    icon_buf: Option<&'a [u8]>,
    gui_options: Option<&'a DmGuiOptions>,
    display_type: Option<DisplayTargetType>,
}

struct GeneratedDisplayTarget {
    card_id: VideoCardId,
    target_idx: usize,
    target_count: usize,
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

    pub fn with_default_display_type(mut self, display_type: DisplayTargetType) -> Self {
        self.display_type = Some(display_type);
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

    pub fn with_display_target_configs(mut self, display_target_configs: &[CardDisplayTargetConfiguration]) -> Self {
        self.display_target_configs = display_target_configs.to_vec();
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
        let _icon = {
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

        // Only create viewports if the config specifies any.
        if self.gui_options.is_some() && !self.win_configs.is_empty() {
            let gui_options = self.gui_options.unwrap();
            // Per-card target counts decouple viewport creation from render-target creation.
            let generated_targets = self.generated_display_targets()?;
            let mut configured_viewports = Vec::with_capacity(self.win_configs.len());
            for (window_idx, window_def) in self.win_configs.iter().enumerate() {
                if window_idx == 0 || window_def.enabled {
                    let (viewport_id, viewport_options) =
                        Self::create_viewport_from_window_def(&mut dm, window_idx, window_def, gui_options)?;
                    configured_viewports.push((window_idx, viewport_id, viewport_options));
                }
            }

            configured_viewports
                .first()
                .ok_or_else(|| anyhow!("Display target configuration requires a main viewport"))?;
            let viewport_positions =
                Self::initial_viewport_positions(generated_targets.len(), configured_viewports.len());
            for (target, viewport_position) in generated_targets.into_iter().zip(viewport_positions) {
                let (window_idx, viewport_id, viewport_options) = &configured_viewports[viewport_position];
                let window_def = &self.win_configs[*window_idx];
                Self::create_generated_target(
                    &mut dm,
                    self.egui_ctx.clone(),
                    &target,
                    *viewport_id,
                    viewport_options,
                    window_def,
                    gui_options,
                )?;
            }
        }

        Ok(dm)
    }

    /// Generate requested targets - a config can specify multiple targets for a single card slot
    /// This supports attaching multiple monitors to a single card. The maximum number of display
    /// targets that can be created is MAX_DISPLAY_TARGETS and is a pool across all card slots.
    fn generated_display_targets(&self) -> Result<Vec<GeneratedDisplayTarget>, Error> {
        let target_counts: Vec<_> = (0..self.cards.len())
            .map(|card_idx| {
                self.display_target_configs
                    .get(card_idx)
                    .map_or(1, |config| config.targets)
            })
            .collect();
        let total_targets = target_counts.iter().copied().fold(0usize, usize::saturating_add);
        if total_targets > MAX_DISPLAY_TARGETS {
            return Err(anyhow!(
                "Display target configuration requests {} targets; the maximum total is {}",
                total_targets,
                MAX_DISPLAY_TARGETS
            ));
        }

        let mut targets = Vec::with_capacity(total_targets);
        for (card_id, target_count) in self.cards.iter().copied().zip(target_counts) {
            for target_idx in 0..target_count {
                targets.push(GeneratedDisplayTarget {
                    card_id,
                    target_idx,
                    target_count,
                });
            }
        }

        Ok(targets)
    }

    /// Assign the last target to the last viewport and continue backwards, leaving any targets
    /// that outnumber secondary viewports in the main viewport at position zero.
    fn initial_viewport_positions(target_count: usize, viewport_count: usize) -> Vec<usize> {
        if target_count == 0 || viewport_count == 0 {
            return Vec::new();
        }

        let main_target_count = target_count.saturating_sub(viewport_count.saturating_sub(1));
        (0..target_count)
            .map(|target_idx| {
                if target_idx < main_target_count {
                    0
                }
                else {
                    viewport_count - (target_count - target_idx)
                }
            })
            .collect()
    }

    /// Generate UI-compatible target name
    fn generated_target_name(target: &GeneratedDisplayTarget) -> String {
        let card_name = format!("{:?}({}) Display", target.card_id.vtype, target.card_id.idx);
        if target.target_count > 1 {
            format!("{} {}", card_name, target.target_idx)
        }
        else {
            card_name
        }
    }

    /// Get defined scaler preset / scaler mode for the given window definition
    fn window_scaler_options(dm: &mut EFrameDisplayManager, window_def: &WindowDefinition) -> (String, ScalerMode) {
        let preset_name = window_def
            .scaler_preset
            .clone()
            .unwrap_or_else(|| "default".to_string());
        // Scaler mode is a property of the display target, not the visual preset. An
        // omitted window mode inherits only from the default preset; the selected visual
        // preset must not determine it.
        let scaler_mode = window_def.scaler_mode.unwrap_or_else(|| {
            dm.scaler_preset("default".to_string())
                .and_then(|preset| preset.mode)
                .unwrap_or_default()
        });

        (preset_name, scaler_mode)
    }

    fn create_viewport_from_window_def(
        dm: &mut EFrameDisplayManager,
        window_idx: usize,
        window_def: &WindowDefinition,
        gui_options: &DmGuiOptions,
    ) -> Result<(ViewportId, DmViewportOptions), Error> {
        let resolved_def = window_def.resolve_with_defaults();
        let main_window = window_idx == 0;
        let viewport_id = if main_window {
            ViewportId::ROOT
        }
        else {
            ViewportId::from_hash_of(("martypc-display-target", window_idx))
        };

        let mut viewport_opts = DmViewportOptions {
            size: resolved_def.size.unwrap_or_default().into(),
            fullscreen: resolved_def.fullscreen,
            always_on_top: resolved_def.always_on_top,
            fill_color: resolved_def.background_color,
            background_organization: resolved_def.background_organization,
            title: resolved_def.name.clone(),
            resizable: resolved_def.resizable,
            can_grab: resolved_def.can_grab,
            ..Default::default()
        };

        if main_window && gui_options.enabled {
            viewport_opts.margins = DisplayTargetMargins::from_t(gui_options.menubar_h);
        }
        if !resolved_def.resizable {
            viewport_opts.min_size = Some(viewport_opts.size);
            viewport_opts.max_size = Some(viewport_opts.size);
        }

        dm.create_viewport(viewport_id, viewport_opts.clone())?;
        Ok((viewport_id, viewport_opts))
    }

    fn create_generated_target(
        dm: &mut EFrameDisplayManager,
        egui_ctx: Context,
        target: &GeneratedDisplayTarget,
        viewport_id: ViewportId,
        viewport_opts: &DmViewportOptions,
        window_def: &WindowDefinition,
        gui_options: &DmGuiOptions,
    ) -> Result<(), Error> {
        let target_name = Self::generated_target_name(target);
        let (preset_name, scaler_mode) = Self::window_scaler_options(dm, window_def);
        let main_window = viewport_id == ViewportId::ROOT;
        let dt_flags = DisplayTargetFlags {
            main_window,
            has_gui: main_window,
            has_menu: main_window,
        };

        dm.create_target(
            target_name,
            DisplayTargetType::WindowBackground,
            dt_flags,
            Some(&egui_ctx),
            Some(viewport_id),
            Some(viewport_opts.clone()),
            Some(target.card_id),
            preset_name,
            scaler_mode,
            gui_options,
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use marty_core::device_traits::videocard::VideoType;
    use marty_frontend_common::types::window::BackgroundOrganization;

    fn window_with_scaler(preset: Option<&str>, mode: Option<ScalerMode>) -> WindowDefinition {
        WindowDefinition {
            enabled: true,
            name: "Main GUI".to_string(),
            background_color: None,
            background_organization: BackgroundOrganization::default(),
            background: true,
            fullscreen: false,
            size: None,
            resizable: true,
            card_id: None,
            card_scale: None,
            scaler_mode: mode,
            always_on_top: false,
            can_grab: true,
            scaler_preset: preset.map(str::to_string),
        }
    }

    #[test]
    fn per_card_count_generates_multiple_targets() {
        let builder = EFrameDisplayManagerBuilder {
            cards: vec![VideoCardId {
                idx:   0,
                vtype: VideoType::CGA,
            }],
            display_target_configs: vec![CardDisplayTargetConfiguration { targets: 2 }],
            ..Default::default()
        };

        let targets = builder.generated_display_targets().unwrap();
        assert_eq!(targets.len(), 2);
        assert!(targets.iter().all(|target| target.card_id.idx == 0));
        assert_eq!(targets[0].target_idx, 0);
        assert_eq!(targets[1].target_idx, 1);
        assert_eq!(
            EFrameDisplayManagerBuilder::generated_target_name(&targets[0]),
            "CGA(0) Display 0"
        );
        assert_eq!(
            EFrameDisplayManagerBuilder::generated_target_name(&targets[1]),
            "CGA(0) Display 1"
        );
    }

    #[test]
    fn zero_targets_suppresses_a_cards_display_targets() {
        let builder = EFrameDisplayManagerBuilder {
            cards: vec![VideoCardId {
                idx:   0,
                vtype: VideoType::CGA,
            }],
            display_target_configs: vec![CardDisplayTargetConfiguration { targets: 0 }],
            ..Default::default()
        };

        assert!(builder.generated_display_targets().unwrap().is_empty());
    }

    #[test]
    fn omitted_card_count_defaults_to_one_target() {
        let builder = EFrameDisplayManagerBuilder {
            cards: vec![
                VideoCardId {
                    idx:   0,
                    vtype: VideoType::CGA,
                },
                VideoCardId {
                    idx:   1,
                    vtype: VideoType::MDA,
                },
            ],
            display_target_configs: vec![CardDisplayTargetConfiguration { targets: 2 }],
            ..Default::default()
        };

        let targets = builder.generated_display_targets().unwrap();
        assert_eq!(targets.len(), 3);
        assert_eq!(targets[2].card_id.idx, 1);
        assert_eq!(targets[2].target_count, 1);
    }

    #[test]
    fn empty_display_target_configuration_defaults_each_card_to_one_target() {
        let builder = EFrameDisplayManagerBuilder {
            cards: vec![
                VideoCardId {
                    idx:   0,
                    vtype: VideoType::CGA,
                },
                VideoCardId {
                    idx:   1,
                    vtype: VideoType::MDA,
                },
            ],
            ..Default::default()
        };

        let targets = builder.generated_display_targets().unwrap();
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].card_id.idx, 0);
        assert_eq!(targets[1].card_id.idx, 1);
        assert!(targets.iter().all(|target| target.target_count == 1));
    }

    #[test]
    fn display_target_counts_for_missing_cards_are_ignored() {
        let builder = EFrameDisplayManagerBuilder {
            cards: vec![VideoCardId {
                idx:   0,
                vtype: VideoType::CGA,
            }],
            display_target_configs: vec![
                CardDisplayTargetConfiguration { targets: 16 },
                CardDisplayTargetConfiguration { targets: usize::MAX },
            ],
            ..Default::default()
        };

        let targets = builder.generated_display_targets().unwrap();
        assert_eq!(targets.len(), MAX_DISPLAY_TARGETS);
        assert!(targets.iter().all(|target| target.card_id.idx == 0));
    }

    #[test]
    fn total_display_target_count_cannot_exceed_sixteen() {
        let builder = EFrameDisplayManagerBuilder {
            cards: vec![
                VideoCardId {
                    idx:   0,
                    vtype: VideoType::CGA,
                },
                VideoCardId {
                    idx:   1,
                    vtype: VideoType::MDA,
                },
            ],
            display_target_configs: vec![
                CardDisplayTargetConfiguration { targets: 9 },
                CardDisplayTargetConfiguration { targets: 8 },
            ],
            ..Default::default()
        };

        let err = builder.generated_display_targets().err().unwrap();
        assert!(err.to_string().contains("maximum total is 16"));
    }

    #[test]
    fn generated_target_settings_inherit_window_scaler_options() {
        let mut dm = EFrameDisplayManager::new();
        let window = window_with_scaler(Some("IBM 5153"), Some(ScalerMode::Fit));

        let (preset, mode) = EFrameDisplayManagerBuilder::window_scaler_options(&mut dm, &window);

        assert_eq!(preset, "IBM 5153");
        assert_eq!(mode, ScalerMode::Fit);
    }

    #[test]
    fn inherited_window_preset_does_not_supply_scaler_mode() {
        let mut dm = EFrameDisplayManager::new();
        let window = window_with_scaler(Some("IBM 5153"), None);

        let (preset, mode) = EFrameDisplayManagerBuilder::window_scaler_options(&mut dm, &window);

        assert_eq!(preset, "IBM 5153");
        assert_eq!(mode, ScalerMode::default());
    }

    #[test]
    fn generated_targets_fill_extra_viewports_from_the_end() {
        assert_eq!(
            EFrameDisplayManagerBuilder::initial_viewport_positions(5, 3),
            vec![0, 0, 0, 1, 2]
        );
        assert_eq!(
            EFrameDisplayManagerBuilder::initial_viewport_positions(2, 4),
            vec![2, 3]
        );
        assert_eq!(
            EFrameDisplayManagerBuilder::initial_viewport_positions(3, 1),
            vec![0, 0, 0]
        );
    }
}

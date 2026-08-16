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

//! Viewport-window and display-target definitions shared by display managers and
//! the `marty_config` crate.

use marty_common::VideoDimensions;
use serde_derive::Deserialize;

pub const MAX_DISPLAY_TARGETS: usize = 16;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub enum ScalerMode {
    Null,
    Fixed,
    #[default]
    Integer,
    Fit,
    Stretch,
    Windowed,
}

/// Controls how multiple window-background display targets share a viewport.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub enum BackgroundOrganization {
    /// Arrange every display in a single horizontal row.
    #[default]
    Linear,
    /// Arrange displays in the smallest square grid that can contain them.
    Square,
}

impl BackgroundOrganization {
    pub fn grid_dimensions(self, target_count: usize) -> (usize, usize) {
        if target_count == 0 {
            return (0, 0);
        }

        match self {
            Self::Linear => (target_count, 1),
            Self::Square if target_count < 3 => (target_count, 1),
            Self::Square => {
                let mut side = 1;
                while side * side < target_count {
                    side += 1;
                }
                (side, side)
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct WindowDefinition {
    #[serde(default)]
    pub enabled: bool,
    pub name: String,
    pub background_color: Option<u32>,
    #[serde(default)]
    pub background_organization: BackgroundOrganization,
    // Legacy display-target fields. New configurations should use
    // `[[emulator.display_targets.card]]` entries instead.
    #[serde(default)]
    pub background: bool,
    #[serde(default)]
    pub fullscreen: bool,
    pub size: Option<VideoDimensions>,
    #[serde(default)]
    pub resizable: bool,
    pub card_id: Option<usize>,
    pub card_scale: Option<f32>,
    /// Initial scaling policy for this display target.
    pub scaler_mode: Option<ScalerMode>,
    #[serde(default)]
    pub always_on_top: bool,
    #[serde(default = "default_can_grab")]
    pub can_grab: bool,
    pub scaler_preset: Option<String>,
}

const fn default_can_grab() -> bool {
    true
}

/// Configures how many independent display targets are created for each enumerated video card.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct DisplayTargetConfiguration {
    /// Array index corresponds to the enumerated video-card index.
    #[serde(default)]
    pub card: Vec<CardDisplayTargetConfiguration>,
}

#[derive(Copy, Clone, Debug, Deserialize)]
pub struct CardDisplayTargetConfiguration {
    pub targets: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct DisplayList {
        #[serde(default)]
        display_targets: DisplayTargetConfiguration,
    }

    #[test]
    fn background_organization_defaults_to_linear() {
        let window: WindowDefinition = toml::from_str("name = 'Main'").unwrap();
        assert_eq!(window.background_organization, BackgroundOrganization::Linear);
        assert_eq!(window.background_organization.grid_dimensions(4), (4, 1));
        assert!(window.can_grab);
    }

    #[test]
    fn window_can_disable_double_click_mouse_capture() {
        let window: WindowDefinition = toml::from_str("name = 'Main'\ncan_grab = false").unwrap();
        assert!(!window.can_grab);
    }

    #[test]
    fn square_background_uses_smallest_fitting_square_grid() {
        let window: WindowDefinition = toml::from_str(
            r#"
                name = "Main"
                background_organization = "Square"
            "#,
        )
        .unwrap();

        assert_eq!(window.background_organization.grid_dimensions(0), (0, 0));
        assert_eq!(window.background_organization.grid_dimensions(1), (1, 1));
        assert_eq!(window.background_organization.grid_dimensions(2), (2, 1));
        assert_eq!(window.background_organization.grid_dimensions(3), (2, 2));
        assert_eq!(window.background_organization.grid_dimensions(4), (2, 2));
        assert_eq!(window.background_organization.grid_dimensions(5), (3, 3));
        assert_eq!(window.background_organization.grid_dimensions(9), (3, 3));
    }

    #[test]
    fn display_target_counts_deserialize_per_card_values() {
        let config: DisplayList = toml::from_str(
            r#"
                [display_targets]

                [[display_targets.card]]
                targets = 0

                [[display_targets.card]]
                targets = 16
            "#,
        )
        .unwrap();

        assert_eq!(config.display_targets.card[0].targets, 0);
        assert_eq!(config.display_targets.card[1].targets, MAX_DISPLAY_TARGETS);
    }
}

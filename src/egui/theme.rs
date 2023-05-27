/*
    Marty PC Emulator 
    (C)2023 Daniel Balsom
    https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

    -------------------------------------------------------------------------

    egui::theme.rs

    EGUI Color theme manager.

*/

use crate::egui::*;
use crate::egui::color::*;

pub struct GuiTheme {
    visuals: Visuals,
}

impl GuiTheme {
    pub fn new(base: &egui::Visuals, color: Color32) -> Self {
        
        let mut new_visuals = base.clone();

        new_visuals.window_fill = color;
        new_visuals.extreme_bg_color = darken_c32(color, 0.50);
        new_visuals.faint_bg_color = darken_c32(color, 0.15);

        new_visuals.widgets.noninteractive.bg_fill = lighten_c32(color, 0.10);
        new_visuals.widgets.noninteractive.bg_stroke.color = lighten_c32(color, 0.75);
        new_visuals.widgets.noninteractive.fg_stroke.color = add_c32(color, 128);

        new_visuals.widgets.active.bg_fill = lighten_c32(color, 0.20);
        new_visuals.widgets.active.bg_stroke.color = lighten_c32(color, 0.35);

        new_visuals.widgets.inactive.bg_fill = lighten_c32(color, 0.35);
        new_visuals.widgets.inactive.bg_stroke.color = lighten_c32(color, 0.50);

        new_visuals.widgets.hovered.bg_fill = lighten_c32(color, 0.75);
        new_visuals.widgets.hovered.bg_stroke.color = lighten_c32(color, 0.75);

        Self {
            visuals: new_visuals
        }
    }

    pub fn visuals(&self) -> &Visuals {
        &self.visuals
    }
}
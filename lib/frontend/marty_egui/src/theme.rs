/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

    -------------------------------------------------------------------------

    egui::theme.rs

    EGUI Color theme manager.

*/

use crate::{color::*, *};

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
        new_visuals.widgets.active.weak_bg_fill = lighten_c32(color, 0.20);
        new_visuals.widgets.active.bg_stroke.color = lighten_c32(color, 0.35);

        new_visuals.widgets.inactive.bg_fill = lighten_c32(color, 0.35);
        new_visuals.widgets.inactive.weak_bg_fill = lighten_c32(color, 0.35);
        new_visuals.widgets.inactive.bg_stroke.color = lighten_c32(color, 0.50);

        new_visuals.widgets.hovered.bg_fill = lighten_c32(color, 0.75);
        new_visuals.widgets.hovered.weak_bg_fill = lighten_c32(color, 0.75);
        new_visuals.widgets.hovered.bg_stroke.color = lighten_c32(color, 0.75);

        Self { visuals: new_visuals }
    }

    pub fn visuals(&self) -> &Visuals {
        &self.visuals
    }
}

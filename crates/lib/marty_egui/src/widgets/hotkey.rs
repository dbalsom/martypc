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

    --------------------------------------------------------------------------

    marty_egui::widgets::hotkey.rs

    Renders a hotkey combination as a row of key-name pills.
*/

use std::fmt::Display;

use egui::{Frame, Margin, Response, RichText, Ui, Widget};

pub struct HotkeyWidget {
    labels: Vec<String>,
}

impl HotkeyWidget {
    pub fn new<I, T>(keys: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Display,
    {
        Self {
            labels: keys.into_iter().map(|key| key.to_string()).collect(),
        }
    }
}

impl Widget for HotkeyWidget {
    fn ui(self, ui: &mut Ui) -> Response {
        let visuals = ui.visuals().widgets.inactive;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            for (index, label) in self.labels.into_iter().enumerate() {
                if index > 0 {
                    ui.label(RichText::new("+").color(ui.visuals().weak_text_color()));
                }

                Frame::new()
                    .fill(visuals.weak_bg_fill)
                    .stroke(visuals.bg_stroke)
                    .corner_radius(visuals.corner_radius)
                    .inner_margin(Margin::symmetric(6, 2))
                    .show(ui, |ui| {
                        ui.label(RichText::new(label).monospace().color(visuals.text_color()));
                    });
            }
        })
        .response
    }
}

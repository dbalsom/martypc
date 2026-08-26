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

    marty_egui::windows::hotkeys.rs

    Displays all configured frontend hotkeys.
*/

use egui::{Grid, ScrollArea, Ui};
use marty_common::types::keys::MartyKey;
use marty_frontend_common::HotkeyEvent;

use crate::widgets::hotkey::HotkeyWidget;

#[derive(Default)]
pub struct HotkeysWindow {
    bindings: Vec<(HotkeyEvent, Vec<MartyKey>)>,
}

impl HotkeysWindow {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_bindings(&mut self, bindings: Vec<(HotkeyEvent, Vec<MartyKey>)>) {
        self.bindings = bindings;
    }

    pub fn draw(&mut self, ui: &mut Ui) {
        if self.bindings.is_empty() {
            ui.label("No hotkeys are configured.");
            return;
        }

        ScrollArea::vertical().show(ui, |ui| {
            Grid::new("hotkey_bindings")
                .num_columns(2)
                .spacing([24.0, 8.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.strong("Action");
                    ui.strong("Hotkey");
                    ui.end_row();

                    for (event, keys) in &self.bindings {
                        ui.label(event.to_string());
                        ui.add(HotkeyWidget::new(keys));
                        ui.end_row();
                    }
                });
        });
    }
}

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

    egui::pit_viewer.rs

    Implements a viewer control for the Programmable Interval Timer.

    This viewer displays data regarding the Programmable Interval Timer's
    3 channels, as well as displaying a graph of the timer output.

*/

use egui::*;

use crate::{color::*, constants::*, *};

use marty_core::{
    devices::{pit::PitDisplayState, serial::SerialPortDisplayState},
    syntax_token::*,
};

#[allow(dead_code)]
pub struct SerialViewerControl {
    serial_state: Vec<SerialPortDisplayState>,
}

impl SerialViewerControl {
    pub fn new() -> Self {
        Self {
            serial_state: Vec::new(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
        for (i, serialport) in self.serial_state.iter().enumerate() {
            egui::CollapsingHeader::new(format!("Port: {}", i))
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.set_min_width(PIT_VIEWER_WIDTH);
                        ui.group(|ui| {
                            ui.set_min_width(PIT_VIEWER_WIDTH);

                            egui::Grid::new(format!("serial_view{}", i))
                                .num_columns(2)
                                .spacing([40.0, 4.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    for (key, value) in serialport {
                                        if let SyntaxToken::StateString(text, _, age) = value {
                                            ui.label(egui::RichText::new(*key).text_style(egui::TextStyle::Monospace));
                                            ui.label(
                                                egui::RichText::new(text)
                                                    .text_style(egui::TextStyle::Monospace)
                                                    .color(fade_c32(Color32::GRAY, STATUS_UPDATE_COLOR, 255 - *age)),
                                            );
                                            ui.end_row();
                                        }
                                    }
                                });
                        });
                    });
                });
        }
    }

    pub fn update_state(&mut self, state: &PitDisplayState) {
        let mut new_serial_state = state.clone();

        // Update state entry ages
        for (i, port) in new_serial_state.iter_mut().enumerate() {
            for (key, value) in port.iter_mut() {
                if let SyntaxToken::StateString(_txt, dirty, age) = value {
                    if *dirty {
                        *age = 0;
                    }
                    else if i < self.serial_state.len() {
                        if let Some(old_tok) = self.serial_state[i].get_mut(key) {
                            if let SyntaxToken::StateString(_, _, old_age) = old_tok {
                                *age = old_age.saturating_add(2);
                            }
                        }
                    }
                }
            }
        }

        self.serial_state = new_serial_state;
    }
}

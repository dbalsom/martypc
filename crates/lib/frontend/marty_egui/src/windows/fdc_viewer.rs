/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2025 Daniel Balsom

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

    egui::fdc_viewer.rs

    Implements a viewer control for the PC FDC (Floppy Disk Controller).

    This viewer displays data regarding the state of the FDC, including a log
    of commands and responses.

*/

use crate::{layouts::MartyLayout, *};
#[allow(dead_code)]
use marty_core::devices::fdc::FdcDebugState;

pub const FDC_VIEWER_LINES: usize = 27;

pub struct FdcViewerControl {
    log_string: String,
    fdc_state:  FdcDebugState,
}

impl FdcViewerControl {
    pub fn new() -> Self {
        Self {
            log_string: String::new(),
            fdc_state:  Default::default(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
        MartyLayout::new(layouts::Layout::KeyValue, "fdc-status-grid").show(ui, |ui| {
            MartyLayout::kv_row(ui, "Last Command:", None, |ui| {
                ui.label(format!("{:?}", self.fdc_state.last_cmd));
            });

            if self.fdc_state.last_status.len() > 0 {
                MartyLayout::kv_row(ui, "Status Bytes:", None, |ui| {
                    ui.label(format!(
                        "{:02X} {:02X} {:02X}",
                        self.fdc_state.last_status[0], self.fdc_state.last_status[1], self.fdc_state.last_status[2]
                    ));
                });
            }
        });

        ui.add_sized(
            ui.available_size(),
            egui::TextEdit::multiline(&mut self.log_string).font(egui::TextStyle::Monospace),
        );
    }

    pub fn update_state(&mut self, state: FdcDebugState) {
        self.fdc_state = state;

        let log_slice = if self.fdc_state.cmd_log.len() > FDC_VIEWER_LINES {
            &self.fdc_state.cmd_log[self.fdc_state.cmd_log.len() - FDC_VIEWER_LINES..]
        }
        else {
            &self.fdc_state.cmd_log
        };

        self.log_string = log_slice.join("\n");
    }
}

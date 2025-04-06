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
    fdc_state: FdcDebugState,
}

impl FdcViewerControl {
    pub fn new() -> Self {
        Self {
            log_string: String::new(),
            fdc_state: Default::default(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
        let mut data_reg_out_string = String::new();
        for byte in self.fdc_state.data_register_out.iter() {
            data_reg_out_string.push_str(&format!("{:02X} ", byte));
        }

        let mut data_reg_in_string = String::new();
        for byte in self.fdc_state.data_register_in.iter() {
            data_reg_in_string.push_str(&format!("{:02X} ", byte));
        }

        #[rustfmt::skip]
        egui::Grid::new("fdc-status-grid").striped(true).show(ui, |ui| {

            ui.label("Interrupt line:");
            ui.label(format!("{}", if self.fdc_state.intr { "1" } else { "0" }));
            ui.end_row();        
    
            ui.label("DOR:");
            ui.label(format!("{:08b}", self.fdc_state.dor));
            ui.end_row();

            ui.label("Status register:");
            ui.label(format!("{:08b}", self.fdc_state.status_register));

            ui.label("DIO:");
            ui.label(format!("{:?}", self.fdc_state.dio));

            ui.label("MRQ:");
            ui.label(format!("{}", if self.fdc_state.status_register & 0x80 != 0 { "1" } else { "0" }));
            ui.end_row();

            ui.label("Data register in:");
            ui.label(data_reg_in_string);
            ui.label("Last written:");
            ui.label(format!("{:02X}", self.fdc_state.last_data_written));
            ui.end_row();

            ui.label("Data register out:");
            ui.label(data_reg_out_string);
            ui.label("Last read:");
            ui.label(format!("{:02X}", self.fdc_state.last_data_read));
            ui.end_row();

            ui.label("Last Command:");
            ui.label(format!(
                "{:?} ({})",
                self.fdc_state.last_cmd, self.fdc_state.last_cmd as u8
            ));
            ui.end_row();

            ui.label("Current Operation:");
            ui.label(format!("{}", self.fdc_state.operation));
            ui.end_row();

            if self.fdc_state.last_status.len() > 0 {
                let st0 = self.fdc_state.last_status[0];
                ui.label("ST0:");
                ui.label(format!("{st0:08b} [{st0:02X}]"));
                ui.end_row();
            }
            if self.fdc_state.last_status.len() > 1 {
                let st1 = self.fdc_state.last_status[1];
                ui.label("ST1:");
                ui.label(format!("{st1:08b} [{st1:02X}]"));
                ui.end_row();
            }
            if self.fdc_state.last_status.len() > 2 {
                let st2 = self.fdc_state.last_status[2];
                ui.label("ST2:");
                ui.label(format!("{st2:08b} [{st2:02X}]"));
                ui.end_row();
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
        } else {
            &self.fdc_state.cmd_log
        };

        self.log_string = log_slice.join("\n");
    }
}

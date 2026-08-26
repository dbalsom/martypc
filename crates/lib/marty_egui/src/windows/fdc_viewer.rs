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

    egui::fdc_viewer.rs

    Implements a viewer control for the PC FDC (Floppy Disk Controller).

    This viewer displays data regarding the state of the FDC, including a log
    of commands and responses.

*/

use crate::{token_listview::TokenListView, GuiEventQueue};
#[allow(dead_code)]
use marty_common::syntax_token::SyntaxTokenStream;
use marty_core::devices::fdc::FdcDebugState;

const FDC_VIEWER_DEFAULT_ROWS: usize = 32;
const FDC_VIEWER_LOG_HEIGHT_SCALE: f32 = 0.75;

fn visible_row_count(available_height: f32, row_height: f32) -> usize {
    if row_height > 0.0 {
        (((available_height * FDC_VIEWER_LOG_HEIGHT_SCALE) / row_height).floor() as usize).max(FDC_VIEWER_DEFAULT_ROWS)
    }
    else {
        FDC_VIEWER_DEFAULT_ROWS
    }
}

pub struct FdcViewerControl {
    log_tokens: Vec<SyntaxTokenStream>,
    log_row: usize,
    fdc_state: FdcDebugState,
    tlv: TokenListView,
}

impl FdcViewerControl {
    pub fn new() -> Self {
        Self {
            log_tokens: Vec::new(),
            log_row: 0,
            fdc_state: Default::default(),
            tlv: TokenListView::new(),
        }
    }

    fn visible_log_rows(&self, visible_rows: usize) -> Vec<SyntaxTokenStream> {
        self.log_tokens
            .iter()
            .skip(self.log_row)
            .take(visible_rows)
            .cloned()
            .collect()
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

        let font_id = egui::TextStyle::Monospace.resolve(ui.style());
        let mut row_height = 0.0;
        ui.fonts_mut(|f| row_height = f.row_height(&font_id) + ui.spacing().item_spacing.y);
        let visible_rows = visible_row_count(ui.available_height(), row_height);

        let max_row = self.log_tokens.len().saturating_sub(visible_rows);
        self.log_row = self.log_row.min(max_row);

        self.tlv.set_capacity(self.log_tokens.len());
        self.tlv.set_visible(visible_rows);
        self.tlv.set_contents(self.visible_log_rows(visible_rows), false);

        let mut new_row = self.log_row;
        let mut scrolled_to = None;
        ui.horizontal(|ui| {
            self.tlv.draw(ui, _events, &mut new_row, &mut |row, _events| {
                scrolled_to = Some(row);
            });
        });

        if let Some(row) = scrolled_to {
            self.log_row = row.min(max_row);
        }
    }

    pub fn update_state(&mut self, state: FdcDebugState) {
        let old_len = self.log_tokens.len();
        let visible_rows = self.tlv.visible_rows.max(1);
        let was_at_bottom = self.log_row >= old_len.saturating_sub(visible_rows);

        self.log_tokens = state.cmd_log_tokens.clone();
        self.fdc_state = state;

        if was_at_bottom {
            self.log_row = self.log_tokens.len().saturating_sub(visible_rows);
            self.tlv.set_scroll_pos(self.log_row);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_viewer_uses_default_rows_when_available_height_is_small() {
        assert_eq!(visible_row_count(160.0, 20.0), FDC_VIEWER_DEFAULT_ROWS);
    }

    #[test]
    fn viewer_can_expand_beyond_default_rows() {
        assert_eq!(visible_row_count(1_000.0, 20.0), 37);
    }
}

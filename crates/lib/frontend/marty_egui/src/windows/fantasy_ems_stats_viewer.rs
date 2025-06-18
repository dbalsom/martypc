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

    egui::io_stats_viewer.rs

    Implements a viewer for device IO statistics (reads and writes)

*/
use marty_core::devices::fantasy_ems::EMSDebugState;
use crate::{token_listview::*, *};
use marty_core::syntax_token::*;

const DEFAULT_ROWS: usize = 36;

pub struct FantasyEMSStatsViewerControl {
    state: EMSDebugState
}



impl FantasyEMSStatsViewerControl {
    pub fn new() -> Self {
        Self {
            state: Default::default(),

        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, events: &mut GuiEventQueue) {
        egui::Grid::new("ems_debug_view_header")
            .striped(true)
            .min_col_width(100.0)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Page Index (0xE8)").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.current_page_index_state.to_string()).font(egui::TextStyle::Monospace));
                ui.end_row();
                ui.label(egui::RichText::new("Auto-Increment").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.page_index_auto_increment_on_state.to_string()).font(egui::TextStyle::Monospace));
                ui.end_row();
                ui.label(egui::RichText::new("Page Lo (0xEA)").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.current_page_set_register_lo_value_state.to_string()).font(egui::TextStyle::Monospace));
                ui.end_row();
            });

        egui::Grid::new("ems_debug_view")
            .striped(true)
            .min_col_width(80.0)
            .show(ui, |ui| {

                ui.label(egui::RichText::new("Page/Segment").text_style(egui::TextStyle::Monospace));
                ui.label(egui::RichText::new("Virtual Page").text_style(egui::TextStyle::Monospace));
                ui.label(egui::RichText::new("Virtual Addr").text_style(egui::TextStyle::Monospace));

                ui.label(egui::RichText::new("Page/Segment").text_style(egui::TextStyle::Monospace));
                ui.label(egui::RichText::new("Virtual Page").text_style(egui::TextStyle::Monospace));
                ui.label(egui::RichText::new("Virtual Addr").text_style(egui::TextStyle::Monospace));

                ui.label(egui::RichText::new("Page/Segment").text_style(egui::TextStyle::Monospace));
                ui.label(egui::RichText::new("Virtual Page").text_style(egui::TextStyle::Monospace));
                ui.label(egui::RichText::new("Virtual Addr").text_style(egui::TextStyle::Monospace));

                ui.label(egui::RichText::new("Page/Segment").text_style(egui::TextStyle::Monospace));
                ui.label(egui::RichText::new("Virtual Page").text_style(egui::TextStyle::Monospace));
                ui.label(egui::RichText::new("Virtual Addr").text_style(egui::TextStyle::Monospace));
                ui.end_row();


                for i in 0..self.state.page_register_state.len() {
                    let label_str = format!("{} / ({:04X})", self.state.page_register_state[i].0, self.state.page_register_state[i].1);
                    ui.label(egui::RichText::new(label_str).text_style(egui::TextStyle::Monospace));

                    let label_2_str = format!("{:06X}", self.state.page_register_state[i].2);
                    ui.label(egui::RichText::new(label_2_str).text_style(egui::TextStyle::Monospace));

                    let mut label_3_str = format!("{:02X}", self.state.page_register_state[i].2 >> 14);

                    ui.add(egui::TextEdit::singleline(& mut label_3_str).font(egui::TextStyle::Monospace));



                    if (i % 4 == 3){
                        ui.end_row();
                    }
                }
            });
    }

    pub fn update_state(&mut self, state: &EMSDebugState) {
        self.state = state.clone();
    }

}

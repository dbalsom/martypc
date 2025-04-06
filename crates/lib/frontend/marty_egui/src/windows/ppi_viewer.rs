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

    egui::ppi_viewer.rs

*/

use crate::{
    color::{fade_c32, STATUS_UPDATE_COLOR},
    layouts,
    layouts::MartyLayout,
    GuiEventQueue,
};
use egui::Color32;
use marty_core::{devices::ppi::PpiDisplayState, syntax_token::SyntaxToken};

pub struct PpiViewerControl {
    ppi_state: PpiDisplayState,
}

impl PpiViewerControl {
    pub fn new() -> Self {
        Self {
            ppi_state: Default::default(),
        }
    }

    /*    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
        egui::Grid::new("ppi_view")
            .num_columns(2)
            .striped(true)
            .spacing([40.0, 4.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Port A Mode:  ").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.ppi_state.port_a_mode).font(egui::TextStyle::Monospace));
                ui.end_row();

                ui.label(egui::RichText::new("Port A Value: ").text_style(egui::TextStyle::Monospace));
                ui.add(
                    egui::TextEdit::singleline(&mut self.ppi_state.port_a_value_bin).font(egui::TextStyle::Monospace),
                );
                ui.end_row();

                ui.label(egui::RichText::new("Port A Value: ").text_style(egui::TextStyle::Monospace));
                ui.add(
                    egui::TextEdit::singleline(&mut self.ppi_state.port_a_value_hex).font(egui::TextStyle::Monospace),
                );
                ui.end_row();

                ui.label(egui::RichText::new("Port B Value: ").text_style(egui::TextStyle::Monospace));
                ui.add(
                    egui::TextEdit::singleline(&mut self.ppi_state.port_b_value_bin).font(egui::TextStyle::Monospace),
                );
                ui.end_row();

                ui.label(egui::RichText::new("Keyboard Byte:").text_style(egui::TextStyle::Monospace));
                ui.add(
                    egui::TextEdit::singleline(&mut self.ppi_state.kb_byte_value_hex).font(egui::TextStyle::Monospace),
                );
                ui.end_row();

                ui.label(egui::RichText::new("Last Keyboard Byte:").text_style(egui::TextStyle::Monospace));
                ui.add(
                    egui::TextEdit::singleline(&mut self.ppi_state.kb_last_byte_value_hex)
                        .font(egui::TextStyle::Monospace),
                );
                ui.end_row();

                ui.label(egui::RichText::new("Keyboard Resets:").text_style(egui::TextStyle::Monospace));
                ui.add(
                    egui::TextEdit::singleline(&mut self.ppi_state.kb_resets_counter).font(egui::TextStyle::Monospace),
                );
                ui.end_row();

                ui.label(egui::RichText::new("Port C Mode:  ").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.ppi_state.port_c_mode).font(egui::TextStyle::Monospace));
                ui.end_row();

                ui.label(egui::RichText::new("Port C Value: ").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.ppi_state.port_c_value).font(egui::TextStyle::Monospace));
                ui.end_row();
            });
    }*/

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
        for (_i, (group_name, group)) in self.ppi_state.iter().enumerate() {
            egui::CollapsingHeader::new(group_name)
                .default_open(true)
                .show(ui, |ui| {
                    for (k, map) in group.iter().enumerate() {
                        MartyLayout::new(
                            layouts::Layout::KeyValue,
                            &format!("ppi-viewer-grid{}{}", group_name, k),
                        )
                        .min_col_width(200.0)
                        .show(ui, |ui| {
                            for (key, value) in map {
                                if let SyntaxToken::StateString(text, _, age) = value {
                                    MartyLayout::kv_row(ui, *key, None, |ui| {
                                        ui.label(
                                            egui::RichText::new(text)
                                                .text_style(egui::TextStyle::Monospace)
                                                .color(fade_c32(Color32::GRAY, STATUS_UPDATE_COLOR, 255 - *age)),
                                        );
                                    });
                                }
                            }
                        });
                    }
                });
        }
    }

    pub fn update_state(&mut self, state: PpiDisplayState) {
        let mut new_state = state;
        // Update state entry ages
        for (group_name, group) in new_state.iter_mut() {
            for (i, map) in group.iter_mut().enumerate() {
                for (key, value) in map.iter_mut() {
                    if let SyntaxToken::StateString(_txt, dirty, age) = value {
                        if *dirty {
                            *age = 0;
                        }
                        else if let Some(old_tok) = self.ppi_state.get_mut(group_name).and_then(|g| g[i].get(key)) {
                            if let SyntaxToken::StateString(_, _, old_age) = old_tok {
                                *age = old_age.saturating_add(2);
                            }
                        }
                    }
                }
            }
        }
        self.ppi_state = new_state;
    }
}

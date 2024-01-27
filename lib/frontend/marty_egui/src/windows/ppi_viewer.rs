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

    egui::ppi_viewer.rs

*/

use crate::GuiEventQueue;
use marty_core::devices::ppi::PpiStringState;

pub struct PpiViewerControl {
    ppi_state: PpiStringState,
}

impl PpiViewerControl {
    pub fn new() -> Self {
        Self {
            ppi_state: Default::default(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
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
    }

    pub fn set_state(&mut self, state: PpiStringState) {
        self.ppi_state = state;
    }
}

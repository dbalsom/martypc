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

    -------------------------------------------------------------------------

    egui::dma_viewer.rs

    Implements a viewer control for the DMA Controller.

*/
use crate::{constants::*, *};
#[allow(dead_code)]
use marty_core::devices::dma::DMAControllerStringState;

pub struct DmaViewerControl {
    dma_state: DMAControllerStringState,
    dma_channel_select: u32,
}

impl DmaViewerControl {
    pub fn new() -> Self {
        Self {
            dma_state: Default::default(),
            dma_channel_select: 0,
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
        egui::Grid::new("dma_view")
            .num_columns(2)
            .striped(true)
            .min_col_width(50.0)
            .show(ui, |ui| {
                ui.set_min_width(DMA_VIEWER_WIDTH);

                ui.label(egui::RichText::new("Enabled:".to_string()).text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.dma_state.enabled).font(egui::TextStyle::Monospace));
                ui.end_row();

                ui.label(egui::RichText::new("DREQ:".to_string()).text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.dma_state.dreq).font(egui::TextStyle::Monospace));
                ui.end_row();

                ui.separator();
                ui.end_row();

                ui.horizontal(|ui| {
                    egui::ComboBox::from_label("Channel #")
                        .selected_text(format!("Channel #{}", self.dma_channel_select))
                        .show_ui(ui, |ui| {
                            for (i, _chan) in self.dma_state.dma_channel_state.iter_mut().enumerate() {
                                ui.selectable_value(&mut self.dma_channel_select, i as u32, format!("Channel #{}", i));
                            }
                        });
                });
                ui.end_row();

                if (self.dma_channel_select as usize) < self.dma_state.dma_channel_state.len() {
                    let chan = &mut self.dma_state.dma_channel_state[self.dma_channel_select as usize];

                    ui.label(
                        egui::RichText::new(format!("#{} CAR:         ", self.dma_channel_select))
                            .text_style(egui::TextStyle::Monospace),
                    );
                    ui.add(egui::TextEdit::singleline(&mut chan.current_address_reg).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(
                        egui::RichText::new(format!("#{} Page:        ", self.dma_channel_select))
                            .text_style(egui::TextStyle::Monospace),
                    );
                    ui.add(egui::TextEdit::singleline(&mut chan.page).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(
                        egui::RichText::new(format!("#{} CWC:         ", self.dma_channel_select))
                            .text_style(egui::TextStyle::Monospace),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut chan.current_word_count_reg).font(egui::TextStyle::Monospace),
                    );
                    ui.end_row();

                    ui.label(
                        egui::RichText::new(format!("#{} BAR:         ", self.dma_channel_select))
                            .text_style(egui::TextStyle::Monospace),
                    );
                    ui.add(egui::TextEdit::singleline(&mut chan.base_address_reg).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(
                        egui::RichText::new(format!("#{} BWC:         ", self.dma_channel_select))
                            .text_style(egui::TextStyle::Monospace),
                    );
                    ui.add(egui::TextEdit::singleline(&mut chan.base_word_count_reg).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(
                        egui::RichText::new(format!("#{} Service Mode:", self.dma_channel_select))
                            .text_style(egui::TextStyle::Monospace),
                    );
                    ui.add(egui::TextEdit::singleline(&mut chan.service_mode).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(
                        egui::RichText::new(format!("#{} Address Mode:", self.dma_channel_select))
                            .text_style(egui::TextStyle::Monospace),
                    );
                    ui.add(egui::TextEdit::singleline(&mut chan.address_mode).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(
                        egui::RichText::new(format!("#{} Xfer Type:   ", self.dma_channel_select))
                            .text_style(egui::TextStyle::Monospace),
                    );
                    ui.add(egui::TextEdit::singleline(&mut chan.transfer_type).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(
                        egui::RichText::new(format!("#{} Auto Init:   ", self.dma_channel_select))
                            .text_style(egui::TextStyle::Monospace),
                    );
                    ui.add(egui::TextEdit::singleline(&mut chan.auto_init).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(
                        egui::RichText::new(format!("#{} Terminal Ct: ", self.dma_channel_select))
                            .text_style(egui::TextStyle::Monospace),
                    );
                    ui.add(egui::TextEdit::singleline(&mut chan.terminal_count).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(
                        egui::RichText::new(format!("#{} TC Reached:  ", self.dma_channel_select))
                            .text_style(egui::TextStyle::Monospace),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut chan.terminal_count_reached).font(egui::TextStyle::Monospace),
                    );
                    ui.end_row();

                    ui.label(
                        egui::RichText::new(format!("#{} Masked:      ", self.dma_channel_select))
                            .text_style(egui::TextStyle::Monospace),
                    );
                    ui.add(egui::TextEdit::singleline(&mut chan.masked).font(egui::TextStyle::Monospace));
                    ui.end_row();
                }
            });
    }

    pub fn update_state(&mut self, state: DMAControllerStringState) {
        self.dma_state = state;
    }
}

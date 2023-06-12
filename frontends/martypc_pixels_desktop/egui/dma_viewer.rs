/*
    Marty PC Emulator 
    (C)2023 Daniel Balsom
    https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

    -------------------------------------------------------------------------

    egui::dma_viewer.rs

    Implements a viewer control for the DMA Controller.

*/
#[allow (dead_code)]

use marty_core::devices::dma::DMAControllerStringState;
use crate::egui::*;
use crate::egui::constants::*;

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
    
    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut VecDeque<GuiEvent> ) {

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
                                ui.selectable_value(&mut self.dma_channel_select, i as u32, format!("Channel #{}",i));
                            }
                        });
                });                        
                ui.end_row();   

                if (self.dma_channel_select as usize) < self.dma_state.dma_channel_state.len() {
                    let chan = &mut self.dma_state.dma_channel_state[self.dma_channel_select as usize];

                    ui.label(egui::RichText::new(format!("#{} CAR:         ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut chan.current_address_reg).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(egui::RichText::new(format!("#{} Page:        ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut chan.page).font(egui::TextStyle::Monospace));
                    ui.end_row();                      

                    ui.label(egui::RichText::new(format!("#{} CWC:         ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut chan.current_word_count_reg).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(egui::RichText::new(format!("#{} BAR:         ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut chan.base_address_reg).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(egui::RichText::new(format!("#{} BWC:         ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut chan.base_word_count_reg).font(egui::TextStyle::Monospace));
                    ui.end_row();    

                    ui.label(egui::RichText::new(format!("#{} Service Mode:", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut chan.service_mode).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(egui::RichText::new(format!("#{} Address Mode:", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut chan.address_mode).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(egui::RichText::new(format!("#{} Xfer Type:   ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut chan.transfer_type).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(egui::RichText::new(format!("#{} Auto Init:   ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut chan.auto_init).font(egui::TextStyle::Monospace));
                    ui.end_row();   

                    ui.label(egui::RichText::new(format!("#{} Terminal Ct: ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut chan.terminal_count).font(egui::TextStyle::Monospace));
                    ui.end_row();  

                    ui.label(egui::RichText::new(format!("#{} TC Reached:  ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut chan.terminal_count_reached).font(egui::TextStyle::Monospace));
                    ui.end_row();

                    ui.label(egui::RichText::new(format!("#{} Masked:      ", self.dma_channel_select)).text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut chan.masked).font(egui::TextStyle::Monospace));
                    ui.end_row();
                }
            });
    }

    pub fn update_state(&mut self, state: DMAControllerStringState) {
        self.dma_state = state;
    }

}
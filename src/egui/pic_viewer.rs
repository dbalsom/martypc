
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

    ---------------------------------------------------------------------------
    
    egui::pic_viewer.rs

    Implements a viewer control for the Programmable Interrupt Controller.
    
    This viewer displays data regarding the Programmable Interrupt 
    Controller's registers as well as statistics regarding the various
    interrupt levels.

*/

use crate::egui::*;

pub struct PicViewerControl {

    state: PicStringState,
}

impl PicViewerControl {

    pub fn new() -> Self {
        Self {
            state: Default::default(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut VecDeque<GuiEvent> ) {

        egui::Grid::new("pic_view")
        .striped(true)
        .min_col_width(100.0)
        .show(ui, |ui| {

            //ui.horizontal(|ui| {
                ui.label(egui::RichText::new("IMR Register: ").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.imr).font(egui::TextStyle::Monospace));
            //});
            ui.end_row();
            //ui.horizontal(|ui| {
                ui.label(egui::RichText::new("ISR Register: ").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.isr).font(egui::TextStyle::Monospace));
            //});
            ui.end_row();   
            //ui.horizontal(|ui| {
                ui.label(egui::RichText::new("IRR Register: ").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.irr).font(egui::TextStyle::Monospace));
            //});         
            ui.end_row();
            //ui.horizontal(|ui| {
                ui.label(egui::RichText::new("IR Lines: ").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.ir).font(egui::TextStyle::Monospace));
            //});         
            ui.end_row();            
            //ui.horizontal(|ui| {
                ui.label(egui::RichText::new("INTR Status: ").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.intr).font(egui::TextStyle::Monospace));
            //});
            ui.end_row();                    
            //ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Auto-EOI: ").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.autoeoi).font(egui::TextStyle::Monospace));
            //});
            ui.end_row();
            //ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Trigger Mode: ").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.trigger_mode).font(egui::TextStyle::Monospace));
            //});
            ui.end_row();                    

            // Add table header
            ui.label(egui::RichText::new("").text_style(egui::TextStyle::Monospace));
            ui.label(egui::RichText::new("IMR Masked").text_style(egui::TextStyle::Monospace));
            ui.label(egui::RichText::new("ISR Masked").text_style(egui::TextStyle::Monospace));
            ui.label(egui::RichText::new("Serviced").text_style(egui::TextStyle::Monospace));
            ui.end_row();

            // Draw table
            for i in 0..self.state.interrupt_stats.len() {
                let label_str = format!("IRQ {}", i );
                ui.label(egui::RichText::new(label_str).text_style(egui::TextStyle::Monospace));

                ui.add(egui::TextEdit::singleline(&mut self.state.interrupt_stats[i].0).font(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.interrupt_stats[i].1).font(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.interrupt_stats[i].2).font(egui::TextStyle::Monospace));
                ui.end_row();                                           
            }
          
        });
    }

    pub fn update_state(&mut self, state: &PicStringState ) {
        self.state = state.clone();
    }
}
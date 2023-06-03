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

    egui::composite_adjust.rs

    Implements hue, saturation and brightness controls for the composite 
    monitor simulation.

*/

use crate::egui::*;
use crate::render::CompositeParams;

pub struct CompositeAdjustControl {
    params: CompositeParams
}


impl CompositeAdjustControl {
    
    pub fn new() -> Self {
        Self {
            params: Default::default(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut VecDeque<GuiEvent> ) {
      
        egui::Grid::new("composite_adjust")
            .striped(false)
            .min_col_width(100.0)
            .show(ui, |ui| {
                
                    ui.label(egui::RichText::new("Hue:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::Slider::new(&mut self.params.hue, 0.0..=2.0));
                ui.end_row();
                    ui.label(egui::RichText::new("Saturation:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::Slider::new(&mut self.params.sat, 0.0..=2.0));

                ui.end_row();
                    ui.label(egui::RichText::new("Luminosity:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::Slider::new(&mut self.params.luma, 0.0..=2.0));     
                ui.end_row();                      
            }
        );
    }

    #[allow(dead_code)]
    pub fn update_params(&mut self, params: CompositeParams ) {
        self.params = params;
    }

    pub fn get_params(&self) -> &CompositeParams {
        &self.params
    }

}
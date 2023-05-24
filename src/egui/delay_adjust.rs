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

    egui::delay_adjust.rs

    Implement various debugging delays.

*/

use egui::*;
use crate::egui::*;

use crate::machine::DelayParams;

pub struct DelayAdjustControl {
    params: DelayParams
}


impl DelayAdjustControl {
    
    pub fn new() -> Self {
        Self {
            params: Default::default(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, events: &mut VecDeque<GuiEvent> ) {
      
        egui::Grid::new("composite_adjust")
            .striped(false)
            .min_col_width(100.0)
            .show(ui, |ui| {
                
                ui.label(egui::RichText::new("DRAM refreh delay:").text_style(egui::TextStyle::Monospace));
                if ui.add(egui::Slider::new(&mut self.params.dram_delay, 0..=100)).changed() {
                    events.push_back(GuiEvent::DelayAdjust);
                }
                ui.end_row();
                    
            }
        );
    }

    pub fn update_params(&mut self, params: DelayParams ) {
        self.params = params;
    }

    pub fn get_params(&self) -> &DelayParams {
        &self.params
    }

}
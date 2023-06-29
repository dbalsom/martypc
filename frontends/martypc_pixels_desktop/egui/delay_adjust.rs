/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

    egui::delay_adjust.rs

    Implement controls for various debugging delays.

*/

use crate::egui::*;
use marty_core::machine::DelayParams;

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
            .min_col_width(200.0)
            .show(ui, |ui| {
                

                ui.label(egui::RichText::new("DRAM refresh delay:").text_style(egui::TextStyle::Monospace));
                if ui.add(egui::Slider::new(&mut self.params.dram_delay, 0..=100)).changed() {
                    events.push_back(GuiEvent::DelayAdjust);
                }
                ui.end_row();
                    
                ui.label(egui::RichText::new("HALT resume delay cycles:").text_style(egui::TextStyle::Monospace));
                if ui.add(egui::Slider::new(&mut self.params.halt_resume_delay, 0..=256)).changed() {
                    events.push_back(GuiEvent::DelayAdjust);
                }
                ui.end_row();                    
            }
        );
    }

    #[allow(dead_code)]
    pub fn update_params(&mut self, params: DelayParams ) {
        self.params = params;
    }

    pub fn get_params(&self) -> &DelayParams {
        &self.params
    }

}
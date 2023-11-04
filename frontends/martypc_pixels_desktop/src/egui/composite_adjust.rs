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

    egui::composite_adjust.rs

    Implements hue, saturation and brightness controls for the composite 
    monitor simulation.

*/

use crate::egui::*;
use marty_render::CompositeParams;

pub struct CompositeAdjustControl {
    params: CompositeParams,
    temp_phase: f64,
}


impl CompositeAdjustControl {
    
    pub fn new() -> Self {
        Self {
            params: Default::default(),
            temp_phase: 0.0
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, events: &mut GuiEventQueue ) {
      
        egui::Grid::new("composite_adjust")
            .striped(false)
            .min_col_width(100.0)
            .show(ui, |ui| {
                
                let mut update = false;

                ui.label(egui::RichText::new("Contrast:").text_style(egui::TextStyle::Monospace));
                if ui.add(egui::Slider::new(&mut self.params.contrast, 0.0..=2.0)).changed() {
                    update = true;
                }
                ui.end_row();   

                ui.label(egui::RichText::new("Hue:").text_style(egui::TextStyle::Monospace));
                if ui.add(egui::Slider::new(&mut self.params.hue, -180.0..=180.0)).changed() {
                    update = true;
                }
                ui.end_row();

                ui.label(egui::RichText::new("Saturation:").text_style(egui::TextStyle::Monospace));
                if ui.add(egui::Slider::new(&mut self.params.sat, 0.0..=2.0)).changed() {
                    update = true;
                }
                ui.end_row();

                ui.label(egui::RichText::new("Luminosity:").text_style(egui::TextStyle::Monospace));
                if ui.add(egui::Slider::new(&mut self.params.luma, 0.0..=2.0)).changed() {
                    update = true;
                }     
                ui.end_row();

                ui.label(egui::RichText::new("Phase Offset:").text_style(egui::TextStyle::Monospace));
                if ui.add(egui::Slider::new(&mut self.temp_phase, 0.0..=270.0)).changed() {
                    update = true;
                }     
                ui.end_row();

                ui.label(egui::RichText::new("CGA Type:").text_style(egui::TextStyle::Monospace));
                if ui.checkbox(&mut self.params.new_cga, "New CGA").changed() {
                    update = true;
                }
                ui.end_row();

                if update {
                    self.params.phase = (self.temp_phase / 90.0).round() as usize;
                    events.send(GuiEvent::CompositeAdjust(self.params));
                }
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
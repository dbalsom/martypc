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

    egui::scaler_adjust

    Implements controls and options for MartyPC's display scaler.

*/


use crate::egui::*;

pub struct ScalerAdjustControl {
    params: ScalerParams
}

impl ScalerAdjustControl {
    pub fn new() -> Self {
        Self {
            params: Default::default(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, events: &mut GuiEventQueue) {
        egui::Grid::new("scaler_adjust")
            .striped(false)
            .min_col_width(100.0)
            .show(ui, |ui| {
                let mut update = false;

                ui.label(egui::RichText::new("CRT Effect:").text_style(egui::TextStyle::Monospace));
                if ui.checkbox(&mut self.params.crt_effect, "Enable").changed() {
                    update = true;
                }
                ui.end_row();

                ui.label(egui::RichText::new("Scanlines:").text_style(egui::TextStyle::Monospace));
                if ui.checkbox(&mut self.params.crt_scanlines, "Enable").changed() {
                    update = true;
                }
                ui.end_row();

                ui.label(egui::RichText::new("Horizontal Curvature:").text_style(egui::TextStyle::Monospace));
                if ui.add(egui::Slider::new(&mut self.params.crt_hcurvature, 0.0..=1.0)).changed() {
                    update = true;
                }
                ui.end_row();

                ui.label(egui::RichText::new("Vertical Curvature:").text_style(egui::TextStyle::Monospace));
                if ui.add(egui::Slider::new(&mut self.params.crt_vcurvature, 0.0..=1.0)).changed() {
                    update = true;
                }
                ui.end_row();

                ui.label(egui::RichText::new("Corner Radius:").text_style(egui::TextStyle::Monospace));
                if ui.add(egui::Slider::new(&mut self.params.crt_cornerradius, 0.0..=1.0)).changed() {
                    update = true;
                }
                ui.end_row();

                if update {
                    //log::debug!("Sending ScalerAdjust event!");
                    events.send(GuiEvent::ScalerAdjust(self.params));
                }
            },
            );
    }

    #[allow(dead_code)]
    pub fn update_params(&mut self, params: ScalerParams ) {
        self.params = params;
    }

    pub fn get_params(&self) -> &ScalerParams {
        &self.params
    }

}
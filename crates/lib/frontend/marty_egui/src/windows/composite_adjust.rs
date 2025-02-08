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

    -------------------------------------------------------------------------

    egui::composite_adjust.rs

    Implements hue, saturation and brightness controls for the composite
    monitor simulation.

*/

use crate::{layouts::MartyLayout, *};
use marty_videocard_renderer::CompositeParams;

pub struct CompositeAdjustControl {
    dt_descs: Vec<String>,
    dt_idx:   usize,

    params: Vec<CompositeParams>,
    temp_phase: Vec<f64>,
}

impl CompositeAdjustControl {
    pub fn new() -> Self {
        Self {
            dt_descs: Vec::new(),
            dt_idx: 0,
            params: Default::default(),
            temp_phase: Vec::new(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, events: &mut GuiEventQueue) {
        if self.dt_idx < self.dt_descs.len() {
            MartyLayout::new(layouts::Layout::KeyValue, "composite-adjust-card-grid").show(ui, |ui| {
                MartyLayout::kv_row(ui, "Card", None, |ui| {
                    egui::ComboBox::from_id_source("composite-adjust-card-select")
                        .selected_text(format!("{}", self.dt_descs[self.dt_idx]))
                        .show_ui(ui, |ui| {
                            for (i, desc) in self.dt_descs.iter_mut().enumerate() {
                                ui.selectable_value(&mut self.dt_idx, i, desc.to_string());
                            }
                        });
                });
            });
        }

        egui::Grid::new("composite_adjust")
            .striped(false)
            .min_col_width(100.0)
            .show(ui, |ui| {
                let mut update = false;

                ui.label(egui::RichText::new("Contrast:").text_style(egui::TextStyle::Monospace));
                if ui
                    .add(egui::Slider::new(&mut self.params[self.dt_idx].contrast, 0.0..=2.0))
                    .changed()
                {
                    update = true;
                }
                ui.end_row();

                ui.label(egui::RichText::new("Hue:").text_style(egui::TextStyle::Monospace));
                if ui
                    .add(egui::Slider::new(&mut self.params[self.dt_idx].hue, -180.0..=180.0))
                    .changed()
                {
                    update = true;
                }
                ui.end_row();

                ui.label(egui::RichText::new("Saturation:").text_style(egui::TextStyle::Monospace));
                if ui
                    .add(egui::Slider::new(&mut self.params[self.dt_idx].sat, 0.0..=2.0))
                    .changed()
                {
                    update = true;
                }
                ui.end_row();

                ui.label(egui::RichText::new("Luminosity:").text_style(egui::TextStyle::Monospace));
                if ui
                    .add(egui::Slider::new(&mut self.params[self.dt_idx].luma, 0.0..=2.0))
                    .changed()
                {
                    update = true;
                }
                ui.end_row();

                ui.label(egui::RichText::new("Phase Offset:").text_style(egui::TextStyle::Monospace));
                if ui
                    .add(egui::Slider::new(&mut self.temp_phase[self.dt_idx], 0.0..=270.0).step_by(90.0))
                    .changed()
                {
                    update = true;
                }
                ui.end_row();

                ui.label(egui::RichText::new("CGA Type:").text_style(egui::TextStyle::Monospace));
                if ui.checkbox(&mut self.params[self.dt_idx].new_cga, "New CGA").changed() {
                    update = true;
                }
                ui.end_row();

                if update {
                    self.params[self.dt_idx].phase = (self.temp_phase[self.dt_idx] / 90.0).round() as usize;
                    events.send(GuiEvent::CompositeAdjust(
                        DtHandle::from(self.dt_idx),
                        self.params[self.dt_idx],
                    ));
                }
            });
    }

    pub fn select_card(&mut self, dt_idx: usize) {
        self.dt_idx = dt_idx;
    }

    pub fn set_dt_list(&mut self, dt_list: Vec<String>) {
        self.dt_descs = dt_list;
        self.params.clear();
        self.temp_phase.clear();
        for _ in self.dt_descs.iter() {
            self.params.push(CompositeParams::default());
            self.temp_phase.push(0.0);
        }
    }

    #[allow(dead_code)]
    pub fn update_params(&mut self, dt_idx: usize, params: CompositeParams) {
        if dt_idx < self.params.len() {
            self.params[dt_idx] = params;
        }
    }

    #[allow(dead_code)]
    pub fn get_params(&self, dt_idx: usize) -> Option<&CompositeParams> {
        if dt_idx < self.params.len() {
            Some(&self.params[dt_idx])
        }
        else {
            None
        }
    }
}

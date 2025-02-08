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

    egui::scaler_adjust

    Implements controls and options for MartyPC's display scaler.

*/

use crate::{layouts::MartyLayout, *};
use marty_frontend_common::display_scaler::{PhosphorType, ScalerFilter, ScalerParams};

pub struct ScalerAdjustControl {
    params:   Vec<ScalerParams>,
    dt_descs: Vec<String>,
    dt_idx:   usize,
}

impl ScalerAdjustControl {
    pub fn new() -> Self {
        Self {
            params:   Default::default(),
            dt_descs: Vec::new(),
            dt_idx:   0,
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

        egui::Grid::new("scaler_adjust")
            .striped(false)
            .min_col_width(100.0)
            .show(ui, |ui| {
                let mut update = false;

                /*
                ui.label(egui::RichText::new("CRT Effect:").text_style(egui::TextStyle::Monospace));
                if ui.checkbox(&mut self.params.crt_effect, "Enable").changed() {
                    update = true;
                }
                ui.end_row();
                */

                ui.label(egui::RichText::new("Filtering Mode:").text_style(egui::TextStyle::Monospace));
                let previous_filter_selection = self.params[self.dt_idx].filter.clone();

                egui::ComboBox::from_id_source("filter_select")
                    .selected_text(format!("{:?}", self.params[self.dt_idx].filter))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.params[self.dt_idx].filter, ScalerFilter::Nearest, "Nearest");
                        ui.selectable_value(&mut self.params[self.dt_idx].filter, ScalerFilter::Linear, "Linear");
                    });

                if self.params[self.dt_idx].filter != previous_filter_selection {
                    update = true;
                }
                ui.end_row();

                ui.label(egui::RichText::new("Phosphor Type:").text_style(egui::TextStyle::Monospace));

                let previous_phosphor_selection = self.params[self.dt_idx].crt_phosphor_type.clone();

                egui::ComboBox::from_id_source("scaler_mono_select")
                    .selected_text(format!("{:?}", self.params[self.dt_idx].crt_phosphor_type))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.params[self.dt_idx].crt_phosphor_type,
                            PhosphorType::Color,
                            "Color",
                        );
                        ui.selectable_value(
                            &mut self.params[self.dt_idx].crt_phosphor_type,
                            PhosphorType::White,
                            "White",
                        );
                        ui.selectable_value(
                            &mut self.params[self.dt_idx].crt_phosphor_type,
                            PhosphorType::Green,
                            "Green",
                        );
                        ui.selectable_value(
                            &mut self.params[self.dt_idx].crt_phosphor_type,
                            PhosphorType::Amber,
                            "Amber",
                        );
                    });

                if self.params[self.dt_idx].crt_phosphor_type != previous_phosphor_selection {
                    update = true;
                }
                ui.end_row();

                ui.label(egui::RichText::new("Gamma:").text_style(egui::TextStyle::Monospace));
                if ui
                    .add(egui::Slider::new(&mut self.params[self.dt_idx].gamma, 0.0..=2.0))
                    .changed()
                {
                    update = true;
                }
                ui.end_row();

                ui.label(egui::RichText::new("Scanlines:").text_style(egui::TextStyle::Monospace));
                if ui
                    .checkbox(&mut self.params[self.dt_idx].crt_scanlines, "Enable")
                    .changed()
                {
                    update = true;
                }
                ui.end_row();

                ui.label(egui::RichText::new("Barrel Distortion:").text_style(egui::TextStyle::Monospace));
                if ui
                    .add(egui::Slider::new(
                        &mut self.params[self.dt_idx].crt_barrel_distortion,
                        0.0..=1.0,
                    ))
                    .changed()
                {
                    update = true;
                }
                ui.end_row();

                ui.label(egui::RichText::new("Corner Radius:").text_style(egui::TextStyle::Monospace));
                if ui
                    .add(egui::Slider::new(
                        &mut self.params[self.dt_idx].crt_corner_radius,
                        0.0..=1.0,
                    ))
                    .changed()
                {
                    update = true;
                }
                ui.end_row();

                if update {
                    //log::debug!("Sending ScalerAdjust event!");
                    events.send(GuiEvent::ScalerAdjust(self.dt_idx, self.params[self.dt_idx]));
                }
            });
    }

    pub fn select_card(&mut self, dt_idx: usize) {
        self.dt_idx = dt_idx;
    }

    pub fn set_dt_list(&mut self, dt_list: Vec<String>) {
        self.dt_descs = dt_list;
        self.params.clear();
        for _ in self.dt_descs.iter() {
            self.params.push(ScalerParams::default());
        }
    }

    #[allow(dead_code)]
    pub fn set_params(&mut self, dt_idx: usize, params: ScalerParams) {
        if dt_idx < self.params.len() {
            self.params[dt_idx] = params;
        }
    }

    #[allow(dead_code)]
    pub fn get_params(&self, dt_idx: usize) -> Option<&ScalerParams> {
        if dt_idx < self.params.len() {
            Some(&self.params[dt_idx])
        }
        else {
            None
        }
    }
}

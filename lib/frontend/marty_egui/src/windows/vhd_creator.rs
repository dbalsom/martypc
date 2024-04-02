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

use crate::{layouts::MartyLayout, widgets::big_icon::IconType, *};
use std::{ffi::OsString, path::PathBuf};

pub struct VhdCreator {
    vhd_formats: Vec<HardDiskFormat>,
    selected_format_idx: usize,
    vhd_requested_name: String,
    vhd_resolved_name: Option<PathBuf>,
}

impl VhdCreator {
    pub fn new() -> Self {
        Self {
            vhd_formats: Vec::new(),
            selected_format_idx: 0,
            vhd_requested_name: String::new(),
            vhd_resolved_name: None,
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, events: &mut GuiEventQueue) {
        ui.horizontal(|ui| {
            IconType::HardDisk.draw(ui, None);
            ui.label(
                egui::RichText::new(
                    "Create a VHD (Virtual Hard Drive)\n\
                        Currently only one disk geometry is supported.\n\
                        Enter a filename and click Create.",
                )
                .font(egui::FontId::proportional(15.0)),
            );
        });

        ui.separator();

        if !self.vhd_formats.is_empty() {
            MartyLayout::new(layouts::Layout::KeyValue, "vhd-grid").show(ui, |ui| {
                MartyLayout::kv_row(ui, "Disk Geometry", None, |ui| {
                    egui::ComboBox::from_id_source("vhd-formats")
                        .selected_text(format!("{}", self.vhd_formats[self.selected_format_idx].desc))
                        .show_ui(ui, |ui| {
                            for (i, fmt) in self.vhd_formats.iter_mut().enumerate() {
                                ui.selectable_value(&mut self.selected_format_idx, i, fmt.desc.to_string());
                            }
                        });
                });
                MartyLayout::kv_row(ui, "VHD Name", Some(300.0), |ui| {
                    ui.text_edit_singleline(&mut self.vhd_requested_name);
                });
                MartyLayout::kv_row(ui, "Filename", None, |ui| {
                    let resolved_name = Self::ensure_vhd_extension(&self.vhd_requested_name);
                    ui.label(resolved_name.display().to_string());
                    self.vhd_resolved_name = Some(resolved_name);
                });
            });
        }
        else {
            ui.vertical_centered(|ui| {
                ui.label("No VHD formats available. Please check your configuration.");
            });
        }

        let (button_text, enabled) = self.get_button_state();

        ui.vertical_centered(|ui| {
            if ui.add_enabled(enabled, egui::Button::new(button_text)).clicked() {
                events.send(GuiEvent::CreateVHD(
                    OsString::from(&self.vhd_resolved_name.clone().unwrap_or_default()),
                    self.vhd_formats[self.selected_format_idx].clone(),
                ))
            };
        });
    }

    fn get_button_state(&self) -> (String, bool) {
        if self.vhd_requested_name.is_empty() {
            return ("Enter a VHD name".to_string(), false);
        }
        else {
            ("Create VHD!".to_string(), true)
        }
    }

    fn ensure_vhd_extension(vhd_name: &str) -> PathBuf {
        let mut path = PathBuf::from(vhd_name);
        if !path.ends_with(".vhd") {
            path.set_extension("vhd");
        }
        path
    }

    #[allow(dead_code)]
    pub fn set_formats(&mut self, formats: Vec<HardDiskFormat>) {
        self.vhd_formats = formats;
    }
}

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

const VHD_REGEX: &str = r"[\w_]*.vhd$";

use crate::*;
use std::{ffi::OsString, path::PathBuf};

pub struct VhdCreator {
    vhd_formats: Vec<HardDiskFormat>,
    selected_format_idx: usize,
    new_vhd_filename: String,
    vhd_regex: Regex,
}

impl VhdCreator {
    pub fn new() -> Self {
        Self {
            vhd_formats: Vec::new(),
            selected_format_idx: 0,
            new_vhd_filename: String::new(),
            vhd_regex: Regex::new(VHD_REGEX).unwrap(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, events: &mut GuiEventQueue) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("🖴")
                    .font(egui::FontId::proportional(40.0))
                    .color(ui.visuals().strong_text_color()),
            );
            ui.label(
                (egui::RichText::new(
                    "Create a VHD (Virtual Hard Drive)\n\
                        Currently only one disk geometry is supported.\n\
                        Enter a filename and click Create.",
                )
                .font(egui::FontId::proportional(15.0))),
            );
        });

        ui.separator();

        egui::Grid::new("vhd_creator_grid")
            .striped(false)
            .min_col_width(100.0)
            .show(ui, |ui| {
                if !self.vhd_formats.is_empty() {
                    ui.label("Disk Geometry:");
                    egui::ComboBox::from_id_source("vhd-formats")
                        .selected_text(format!("{}", self.vhd_formats[self.selected_format_idx].desc))
                        .show_ui(ui, |ui| {
                            for (i, fmt) in self.vhd_formats.iter_mut().enumerate() {
                                ui.selectable_value(&mut self.selected_format_idx, i, fmt.desc.to_string());
                            }
                        });
                    ui.end_row();

                    ui.label("VHD Name: ");

                    ui.horizontal(|ui| {
                        ui.set_min_width(300.0);
                        ui.text_edit_singleline(&mut self.new_vhd_filename);
                    });
                    ui.end_row();

                    let vhd_path = Self::ensure_vhd_extension(&self.new_vhd_filename);
                    ui.label("Filename: ");
                    ui.label(vhd_path.display().to_string());
                    ui.end_row();
                }
            });

        let (button_text, enabled) = self.get_button_state();

        ui.vertical_centered(|ui| {
            if ui.add_enabled(enabled, egui::Button::new(button_text)).clicked() {
                events.send(GuiEvent::CreateVHD(
                    OsString::from(&self.new_vhd_filename),
                    self.vhd_formats[self.selected_format_idx].clone(),
                ))
            };
        });
    }

    fn get_button_state(&self) -> (String, bool) {
        if self.new_vhd_filename.is_empty() {
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

/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

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

use std::path::{Path, PathBuf};

use crate::{
    layouts::MartyLayout,
    widgets::big_icon::{BigIcon, IconType},
    *,
};
use marty_core::machine::MachineState;

struct VhdDriveSlot {
    index: usize,
    mounted_path: Option<PathBuf>,
}

impl VhdDriveSlot {
    fn label(&self) -> String {
        match &self.mounted_path {
            Some(path) => {
                let filename = path.file_name().unwrap_or(path.as_os_str()).to_string_lossy();
                format!("Drive {} — {}", self.index, filename)
            }
            None => format!("Drive {} — <empty>", self.index),
        }
    }
}

pub struct VhdCreator {
    vhd_formats: Vec<HardDiskFormat>,
    selected_format_idx: usize,
    output_path: Option<PathBuf>,
    partitioned: bool,
    formatted: bool,
    include_files: bool,
    source_path: Option<PathBuf>,
    mount_as: bool,
    selected_mount_slot: Option<usize>,
    drive_slots: Vec<VhdDriveSlot>,
    machine_state: MachineState,
}

impl VhdCreator {
    pub fn new() -> Self {
        Self {
            vhd_formats: Vec::new(),
            selected_format_idx: 0,
            output_path: None,
            partitioned: false,
            formatted: false,
            include_files: false,
            source_path: None,
            mount_as: false,
            selected_mount_slot: None,
            drive_slots: Vec::new(),
            machine_state: MachineState::Off,
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, events: &mut GuiEventQueue) {
        ui.horizontal(|ui| {
            ui.add(BigIcon::new(IconType::HardDisk, None));
            ui.label(
                egui::RichText::new(
                    "Create a VHD (Virtual Hard Drive)\n\
                        Available formats are determined by the current Hard Disk Controller.\n\
                        Choose an output file and click Create.",
                )
                .font(egui::FontId::proportional(15.0)),
            );
        });

        ui.separator();

        if !self.vhd_formats.is_empty() {
            MartyLayout::new(layouts::Layout::KeyValue, "vhd-grid").show(ui, |ui| {
                MartyLayout::kv_row(ui, "Disk Geometry", None, |ui| {
                    egui::ComboBox::from_id_salt("vhd-formats")
                        .selected_text(format!("{}", self.vhd_formats[self.selected_format_idx].to_string()))
                        .show_ui(ui, |ui| {
                            for (i, fmt) in self.vhd_formats.iter_mut().enumerate() {
                                ui.selectable_value(&mut self.selected_format_idx, i, fmt.to_string());
                            }
                        });
                });
                MartyLayout::kv_row(ui, "Output file", None, |ui| {
                    if ui.button("Browse...").clicked() {
                        events.send(GuiEvent::BrowseVhdOutputFile);
                    }
                    let output = self
                        .output_path
                        .as_deref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "No file selected".to_string());
                    ui.label(&output).on_hover_text(output);
                });

                MartyLayout::kv_row(ui, "Partition entire drive", None, |ui| {
                    ui.checkbox(&mut self.partitioned, "");
                });
                if !self.partitioned {
                    self.formatted = false;
                    self.include_files = false;
                }

                MartyLayout::kv_row(ui, "Format", None, |ui| {
                    ui.add_enabled(self.partitioned, egui::Checkbox::new(&mut self.formatted, ""));
                });
                if !self.formatted {
                    self.include_files = false;
                }

                MartyLayout::kv_row(ui, "Include files", None, |ui| {
                    ui.add_enabled(self.formatted, egui::Checkbox::new(&mut self.include_files, ""));
                });

                MartyLayout::kv_row(ui, "Source folder", None, |ui| {
                    ui.add_enabled_ui(self.include_files, |ui| {
                        if ui.button("Browse...").clicked() {
                            events.send(GuiEvent::BrowseVhdSourceDirectory);
                        }
                        let source = self
                            .source_path
                            .as_deref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "No folder selected".to_string());
                        ui.label(&source).on_hover_text(source);
                    });
                });

                MartyLayout::kv_row(ui, "Mount as", None, |ui| {
                    ui.add_enabled(
                        !self.drive_slots.is_empty(),
                        egui::Checkbox::new(&mut self.mount_as, ""),
                    );
                });
                if self.drive_slots.is_empty() {
                    self.mount_as = false;
                }

                MartyLayout::kv_row(ui, "Drive slot", None, |ui| {
                    ui.add_enabled_ui(self.mount_as, |ui| {
                        let selected_text = self
                            .selected_mount_slot
                            .and_then(|selected| self.drive_slots.iter().find(|slot| slot.index == selected))
                            .map(VhdDriveSlot::label)
                            .unwrap_or_else(|| "No drive slots available".to_string());

                        egui::ComboBox::from_id_salt("vhd-mount-slot")
                            .selected_text(selected_text)
                            .show_ui(ui, |ui| {
                                for slot in &self.drive_slots {
                                    let response = ui.selectable_value(
                                        &mut self.selected_mount_slot,
                                        Some(slot.index),
                                        slot.label(),
                                    );
                                    if let Some(path) = &slot.mounted_path {
                                        response.on_hover_text(path.display().to_string());
                                    }
                                }
                            });
                    });
                });

                if self.mount_as {
                    MartyLayout::kv_row(ui, "Machine state", None, |ui| {
                        if self.machine_state.is_on() {
                            ui.colored_label(ui.visuals().error_fg_color, "Powered on");
                            if ui.button("Power Off").clicked() {
                                events.send(GuiEvent::MachineStateChange(MachineState::Off));
                            }
                        }
                        else {
                            ui.label("Powered off");
                        }
                    });
                }
            });
        }
        else {
            ui.vertical_centered(|ui| {
                ui.label("No VHD formats available. Please check your configuration.");
            });
        }

        ui.separator();

        let (button_text, enabled) = self.get_button_state();

        ui.vertical_centered(|ui| {
            if ui.add_enabled(enabled, egui::Button::new(button_text)).clicked() {
                events.send(GuiEvent::CreateVHD(VhdCreateRequest {
                    path: self.output_path.clone().unwrap_or_default(),
                    format: self.vhd_formats[self.selected_format_idx].clone(),
                    partitioned: self.partitioned,
                    formatted: self.formatted,
                    source: if self.include_files {
                        self.source_path.clone()
                    }
                    else {
                        None
                    },
                    mount_drive: if self.mount_as { self.selected_mount_slot } else { None },
                }))
            };
        });
    }

    fn get_button_state(&self) -> (String, bool) {
        if self.vhd_formats.is_empty() {
            return ("No VHD formats available".to_string(), false);
        }
        if self.output_path.is_none() {
            return ("Select an output file".to_string(), false);
        }
        if self.include_files && self.source_path.is_none() {
            return ("Select a source folder".to_string(), false);
        }
        if self.mount_as && self.selected_mount_slot.is_none() {
            return ("Select a drive slot".to_string(), false);
        }
        if self.mount_as && self.machine_state.is_on() {
            return ("Power off to mount the VHD".to_string(), false);
        }
        else {
            (
                if self.mount_as {
                    "Create and Mount VHD!"
                }
                else {
                    "Create VHD!"
                }
                .to_string(),
                true,
            )
        }
    }

    fn ensure_vhd_extension(vhd_path: &Path) -> PathBuf {
        let mut path = vhd_path.to_path_buf();
        path.set_extension("vhd");
        path
    }

    #[allow(dead_code)]
    pub fn set_formats(&mut self, formats: Vec<HardDiskFormat>) {
        self.vhd_formats = formats;
        self.selected_format_idx = self.selected_format_idx.min(self.vhd_formats.len().saturating_sub(1));
    }

    pub fn set_source_path(&mut self, path: PathBuf) {
        self.source_path = Some(path);
    }

    pub fn set_output_path(&mut self, path: PathBuf) {
        self.output_path = Some(Self::ensure_vhd_extension(&path));
    }

    pub fn set_drive_slots(&mut self, slots: Vec<(usize, Option<PathBuf>)>) {
        if !slots.iter().any(|(index, _)| Some(*index) == self.selected_mount_slot) {
            self.selected_mount_slot = slots
                .iter()
                .find(|(_, mounted_path)| mounted_path.is_none())
                .or_else(|| slots.first())
                .map(|(index, _)| *index);
        }

        self.drive_slots = slots
            .into_iter()
            .map(|(index, mounted_path)| VhdDriveSlot { index, mounted_path })
            .collect();
    }

    pub fn set_machine_state(&mut self, state: MachineState) {
        self.machine_state = state;
    }
}

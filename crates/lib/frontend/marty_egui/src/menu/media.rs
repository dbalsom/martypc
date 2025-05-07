/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2025 Daniel Balsom

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

    --------------------------------------------------------------------------
*/
use crate::{file_dialogs::FileDialogFilter, modal::ModalContext, state::GuiState, GuiEvent, GuiWindow};
use fluxfox::ImageFormatParser;
use marty_frontend_common::thread_events::{FileOpenContext, FileSaveContext, FileSelectionContext};

impl GuiState {
    pub fn show_media_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Media", |ui| {
            //ui.set_min_size(egui::vec2(240.0, 0.0));
            //ui.style_mut().spacing.item_spacing = egui::Vec2{ x: 6.0, y:6.0 };
            ui.set_width_range(egui::Rangef { min: 100.0, max: 240.0 });

            // Display option to rescan media folders if native.
            // We can't rescan anything in the browser - what we've got is what we've got.
            #[cfg(not(target_arch = "wasm32"))]
            if ui.button("⟲ Rescan Media Folders").clicked() {
                self.event_queue.send(GuiEvent::RescanMediaFolders);
            }

            self.workspace_window_open_button(ui, GuiWindow::FloppyViewer, true, true);
            for i in 0..self.floppy_drives.len() {
                self.draw_floppy_menu(ui, i);
            }

            for i in 0..self.hdds.len() {
                self.draw_hdd_menu(ui, i);
            }

            for i in 0..self.carts.len() {
                self.draw_cart_menu(ui, i);
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                if ui.button("🖹 Create new VHD...").clicked() {
                    *self.window_flag(GuiWindow::VHDCreator) = true;
                    ui.close_menu();
                };
            }
        });
    }

    pub fn draw_floppy_menu(&mut self, ui: &mut egui::Ui, drive_idx: usize) {
        let floppy_name = match drive_idx {
            0 => format!("💾 Floppy Drive 0 - {} (A:)", self.floppy_drives[drive_idx].drive_type),
            1 => format!("💾 Floppy Drive 1 - {} (B:)", self.floppy_drives[drive_idx].drive_type),
            _ => format!(
                "💾 Floppy Drive {} - {}",
                drive_idx, self.floppy_drives[drive_idx].drive_type
            ),
        };

        let _menu_response = ui
            .menu_button(floppy_name, |ui| {
                self.event_queue.send(GuiEvent::QueryCompatibleFloppyFormats(drive_idx));

                ui.menu_button("🗁 Quick Access Image/Zip file", |ui| {
                    self.floppy_tree_menu.draw(ui, drive_idx, true, &mut |image_idx| {
                        //log::debug!("Clicked closure called with image_idx {}", image_idx);
                        self.event_queue.send(GuiEvent::LoadQuickFloppy(drive_idx, image_idx));
                    });
                });

                if ui.button("🗁 Browse for Image...").clicked() {
                    #[cfg(target_arch = "wasm32")]
                    {
                        self.event_queue.send(GuiEvent::RequestLoadFloppyDialog(drive_idx));
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let fc = FileOpenContext::FloppyDiskImage {
                            drive_select: drive_idx,
                            fsc: FileSelectionContext::Uninitialized,
                        };

                        let mut filter_vec = Vec::new();
                        let exts = fluxfox::supported_extensions();
                        filter_vec.push(FileDialogFilter::new("Floppy Disk Images", exts));
                        filter_vec.push(FileDialogFilter::new("Zip Files", vec!["zip"]));
                        filter_vec.push(FileDialogFilter::new("All Files", vec!["*"]));

                        self.open_file_dialog(fc, "Select Floppy Disk Image", filter_vec);

                        self.modal.open(ModalContext::Notice(
                            "A native File Open dialog is open.\nPlease make a selection or cancel to continue."
                                .to_string(),
                        ));
                    }
                    ui.close_menu();
                };

                #[cfg(not(target_arch = "wasm32"))]
                if !self.autofloppy_paths.is_empty() {
                    ui.menu_button("🗐 Create from Directory", |ui| {
                        for path in self.autofloppy_paths.iter() {
                            if ui.button(format!("📁 {}", path.name.to_string_lossy())).clicked() {
                                self.event_queue
                                    .send(GuiEvent::LoadAutoFloppy(drive_idx, path.full_path.clone()));
                                ui.close_menu();
                            }
                        }
                    });
                }

                ui.menu_button("🗋 Create New", |ui| {
                    for format in self.floppy_drives[drive_idx].drive_type.get_compatible_formats() {
                        let format_options = vec![("(Blank)", false), ("(Formatted)", true)];
                        for fo in format_options {
                            if ui.button(format!("💾{} {}", format, fo.0)).clicked() {
                                self.event_queue
                                    .send(GuiEvent::CreateNewFloppy(drive_idx, format, fo.1));
                                ui.close_menu();
                            }
                        }
                    }
                });

                ui.separator();

                let floppy_viewer_enabled = self.floppy_drives[drive_idx].filename().is_some()
                    || self.floppy_drives[drive_idx].is_new().is_some();

                if self.workspace_window_open_button(ui, GuiWindow::FloppyViewer, true, floppy_viewer_enabled) {
                    self.floppy_viewer.set_drive_idx(drive_idx);
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if let Some(floppy_name) = &self.floppy_drives[drive_idx].filename() {
                        let type_str = self.floppy_drives[drive_idx].type_string();
                        if ui.button(format!("⏏ Eject {}{}", type_str, floppy_name)).clicked() {
                            self.event_queue.send(GuiEvent::EjectFloppy(drive_idx));
                        }
                    }
                    else if let Some(format) = &self.floppy_drives[drive_idx].is_new() {
                        let type_str = self.floppy_drives[drive_idx].type_string();
                        if ui.button(format!("⏏ Eject {}{}", type_str, format)).clicked() {
                            self.event_queue.send(GuiEvent::EjectFloppy(drive_idx));
                        }
                    }
                    else {
                        ui.add_enabled(false, egui::Button::new("Eject image: <No Image>"));
                    }
                });

                // Add 'Save' option for native build to write back to the currently loaded disk image.
                // This is disabled in the browser due since we can't write to the loaded image.
                #[cfg(not(target_arch = "wasm32"))]
                {
                    ui.horizontal(|ui| {
                        if let Some(floppy_name) = &self.floppy_drives[drive_idx].filename() {
                            ui.add_enabled_ui(self.floppy_drives[drive_idx].is_writeable(), |ui| {
                                let type_str = self.floppy_drives[drive_idx].type_string();
                                if ui.button(format!("💾 Save {}{}", type_str, floppy_name)).clicked() {
                                    if let Some(floppy_path) = self.floppy_drives[drive_idx].file_path() {
                                        if let Some(fmt) = self.floppy_drives[drive_idx].source_format {
                                            self.event_queue.send(GuiEvent::SaveFloppyAs(
                                                drive_idx,
                                                fmt,
                                                floppy_path.clone(),
                                            ));
                                        }
                                    }
                                }
                            });
                        }
                        else {
                            ui.add_enabled(false, egui::Button::new("Save image: <No Image File>"));
                        }
                    });
                }

                // Add 'Save As' options for compatible formats.
                for format_tuple in &self.floppy_drives[drive_idx].supported_formats {
                    let fmt = format_tuple.0;
                    let fmt_name = fmt.to_string();
                    let extensions = &format_tuple.1;

                    if !extensions.is_empty() {
                        if ui
                            .button(format!("Save As .{}...", extensions[0].to_uppercase()))
                            .clicked()
                        {
                            #[cfg(target_arch = "wasm32")]
                            {
                                self.event_queue.send(GuiEvent::RequestSaveFloppyDialog(drive_idx, fmt));
                            }
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                let fc = FileSaveContext::FloppyDiskImage {
                                    drive_select: drive_idx,
                                    format: fmt,
                                    fsc: FileSelectionContext::Uninitialized,
                                };

                                let mut filter_vec = Vec::new();
                                let exts = fmt.extensions();
                                filter_vec.push(FileDialogFilter::new(fmt_name, exts));

                                self.save_file_dialog(fc, "Save Floppy Disk Image", filter_vec);

                                self.modal.open(ModalContext::Notice(
                                    "A native File Save dialog is open.\nPlease make a selection or cancel to continue."
                                        .to_string(),
                                ));
                                ui.close_menu();
                            }
                        }
                    }
                }

                if ui
                    .checkbox(&mut self.floppy_drives[drive_idx].write_protected, "Write Protect")
                    .changed()
                {
                    self.event_queue.send(GuiEvent::SetFloppyWriteProtect(
                        drive_idx,
                        self.floppy_drives[drive_idx].write_protected,
                    ));
                }
            })
            .response;
        ui.end_row();
    }

    pub fn draw_hdd_menu(&mut self, ui: &mut egui::Ui, drive_idx: usize) {
        let hdd_name = format!("🖴 Hard Disk {}", drive_idx);

        // Only enable VHD loading if machine is off to prevent corruption to VHD.
        ui.menu_button(hdd_name, |ui| {
            if self.machine_state.is_on() {
                // set 'color' to the appropriate warning color for current egui visuals
                let error_color = ui.visuals().error_fg_color;
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(
                        egui::RichText::new("Machine must be off to make changes").color(error_color),
                    ));
                });
            }
            ui.add_enabled_ui(!self.machine_state.is_on(), |ui| {
                ui.menu_button("Load image", |ui| {
                    self.hdd_tree_menu.draw(ui, drive_idx, true, &mut |image_idx| {
                        self.event_queue.send(GuiEvent::LoadVHD(drive_idx, image_idx));
                    });
                });

                let (have_vhd, detatch_string) = match &self.hdds[drive_idx].filename() {
                    Some(name) => (true, format!("Detach image: {}", name)),
                    None => (false, "Detach: <No Disk>".to_string()),
                };

                ui.add_enabled_ui(have_vhd, |ui| {
                    if ui.button(detatch_string).clicked() {
                        self.event_queue.send(GuiEvent::DetachVHD(drive_idx));
                    }
                });
            });
        });
    }

    pub fn draw_cart_menu(&mut self, ui: &mut egui::Ui, cart_idx: usize) {
        let cart_name = format!("📼 Cartridge Slot {}", cart_idx);

        ui.menu_button(cart_name, |ui| {
            ui.menu_button("Insert Cartridge", |ui| {
                self.cart_tree_menu.draw(ui, cart_idx, true, &mut |image_idx| {
                    self.event_queue.send(GuiEvent::InsertCartridge(cart_idx, image_idx));
                });
            });

            let (have_cart, detatch_string) = match &self.carts[cart_idx].filename() {
                Some(name) => (true, format!("Remove Cartridge: {}", name)),
                None => (false, "Remove Cartridge: <No Cart>".to_string()),
            };

            ui.add_enabled_ui(have_cart, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(detatch_string).clicked() {
                        self.event_queue.send(GuiEvent::RemoveCartridge(cart_idx));
                    }
                });
            });
        });
    }
}

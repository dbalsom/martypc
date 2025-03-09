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
use marty_frontend_common::thread_events::{FileOpenContext, FileSaveContext, FileSelectionContext};

use fluxfox::ImageFormatParser;

impl GuiState {
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
}

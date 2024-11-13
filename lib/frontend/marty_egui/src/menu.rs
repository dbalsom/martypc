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

    --------------------------------------------------------------------------

    egui::menu.rs

    Implement the main emulator menu bar.

*/

use crate::{state::GuiState, GuiBoolean, GuiEnum, GuiEvent, GuiVariable, GuiVariableContext, GuiWindow};
use egui_file::FileDialog;
//use egui_file_dialog::FileDialog;

use marty_core::{device_traits::videocard::VideoType, devices::serial::SerialPortDescriptor};

use crate::modal::ModalContext;
use marty_core::machine::MachineState;

impl GuiState {
    pub fn draw_menu(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("Emulator", |ui| {
                ui.set_width_range(egui::Rangef { min: 80.0, max: 100.0 });

                if !self.modal.is_open() {
                    if ui.button("⏱ Performance...").clicked() {
                        *self.window_flag(GuiWindow::PerfViewer) = true;
                        ui.close_menu();
                    }

                    if ui.button("❓ About...").clicked() {
                        *self.window_flag(GuiWindow::About) = true;
                        ui.close_menu();
                    }
                    ui.separator();
                }

                if ui.button("⎆ Quit").clicked() {
                    self.event_queue.send(GuiEvent::Exit);
                    ui.close_menu();
                }
            });

            // Only show the Emulator menu if a modal dialog is open.
            if self.modal.is_open() {
                return;
            }

            ui.menu_button("Machine", |ui| {
                ui.menu_button("Input/Output", |ui| {
                    // Create a vector of ports that are currently bridged. We will use this to disable
                    // those ports from selection in the menu.
                    let bridged_ports = self
                        .serial_ports
                        .iter()
                        .filter_map(|port| port.brige_port_id)
                        .collect::<Vec<_>>();

                    for SerialPortDescriptor {
                        id: guest_port_id,
                        name: guest_port_name,
                        ..
                    } in self.serial_ports.clone().iter()
                    {
                        ui.menu_button(format!("Passthrough {}", guest_port_name), |ui| {
                            let mut selected = false;

                            for (host_port_id, host_port) in self.host_serial_ports.iter().enumerate() {
                                if let Some(enum_mut) = self.get_option_enum(
                                    GuiEnum::SerialPortBridge(Default::default()),
                                    Some(GuiVariableContext::SerialPort(*guest_port_id)),
                                ) {
                                    selected = *enum_mut == GuiEnum::SerialPortBridge(host_port_id);
                                }

                                let enabled = !bridged_ports.contains(&host_port_id);

                                if ui
                                    .add_enabled(enabled, egui::RadioButton::new(selected, host_port.port_name.clone()))
                                    .clicked()
                                {
                                    self.event_queue.send(GuiEvent::BridgeSerialPort(
                                        *guest_port_id,
                                        host_port.port_name.clone(),
                                        host_port_id,
                                    ));
                                    ui.close_menu();
                                }
                            }
                        });
                    }
                });

                ui.separator();

                let (is_on, is_paused) = match self.machine_state {
                    MachineState::On => (true, false),
                    MachineState::Paused => (true, true),
                    MachineState::Off => (false, false),
                    _ => (false, false),
                };

                ui.add_enabled_ui(!is_on, |ui| {
                    if ui.button("⚡ Power on").clicked() {
                        self.event_queue.send(GuiEvent::MachineStateChange(MachineState::On));
                        ui.close_menu();
                    }
                });

                if ui
                    .checkbox(&mut self.get_option_mut(GuiBoolean::TurboButton), "Turbo Button")
                    .clicked()
                {
                    let new_opt = self.get_option(GuiBoolean::TurboButton).unwrap();

                    self.event_queue.send(GuiEvent::VariableChanged(
                        GuiVariableContext::Global,
                        GuiVariable::Bool(GuiBoolean::TurboButton, new_opt),
                    ));
                    ui.close_menu();
                }

                ui.add_enabled_ui(is_on && !is_paused, |ui| {
                    if ui.button("⏸ Pause").clicked() {
                        self.event_queue
                            .send(GuiEvent::MachineStateChange(MachineState::Paused));
                        ui.close_menu();
                    }
                });

                ui.add_enabled_ui(is_on && is_paused, |ui| {
                    if ui.button("▶ Resume").clicked() {
                        self.event_queue
                            .send(GuiEvent::MachineStateChange(MachineState::Resuming));
                        ui.close_menu();
                    }
                });

                ui.add_enabled_ui(is_on, |ui| {
                    if ui.button("⟲ Reboot").clicked() {
                        self.event_queue
                            .send(GuiEvent::MachineStateChange(MachineState::Rebooting));
                        ui.close_menu();
                    }
                });

                ui.add_enabled_ui(is_on, |ui| {
                    if ui.button("⟲ CTRL-ALT-DEL").clicked() {
                        self.event_queue.send(GuiEvent::CtrlAltDel);
                        ui.close_menu();
                    }
                });

                ui.add_enabled_ui(is_on, |ui| {
                    if ui.button("🔌 Power off").clicked() {
                        self.event_queue.send(GuiEvent::MachineStateChange(MachineState::Off));
                        ui.close_menu();
                    }
                });
            });

            let _media_response = ui.menu_button("Media", |ui| {
                //ui.set_min_size(egui::vec2(240.0, 0.0));
                //ui.style_mut().spacing.item_spacing = egui::Vec2{ x: 6.0, y:6.0 };
                ui.set_width_range(egui::Rangef { min: 100.0, max: 240.0 });

                if ui.button("⟲ Rescan Media Folders").clicked() {
                    self.event_queue.send(GuiEvent::RescanMediaFolders);
                }

                for i in 0..self.floppy_drives.len() {
                    self.draw_floppy_menu(ui, i);
                }

                for i in 0..self.hdds.len() {
                    self.draw_hdd_menu(ui, i);
                }

                for i in 0..self.carts.len() {
                    self.draw_cart_menu(ui, i);
                }

                if ui.button("🖹 Create new VHD...").clicked() {
                    *self.window_flag(GuiWindow::VHDCreator) = true;
                    ui.close_menu();
                };
            });

            ui.menu_button("Display", |ui| {
                ui.set_min_size(egui::vec2(240.0, 0.0));

                // If there is only one display, emit the display menu directly.
                // Otherwise, emit named menus for each display.
                if self.display_info.len() == 1 {
                    self.draw_display_menu(ui, 0);
                }
                else if self.display_info.len() > 1 {
                    for i in 0..self.display_info.len() {
                        ui.menu_button(format!("Display {}: {}", i, &self.display_info[i].name), |ui| {
                            self.draw_display_menu(ui, i);
                        });
                    }
                }
            });

            ui.menu_button("Debug", |ui| {
                ui.menu_button("CPU", |ui| {
                    self.workspace_window_open_button(ui, GuiWindow::CpuControl, true, true);
                    self.workspace_window_open_button(ui, GuiWindow::CpuStateViewer, true, true);

                    ui.menu_button("CPU Debug Options", |ui| {
                        if ui
                            .checkbox(
                                &mut self.get_option_mut(GuiBoolean::CpuEnableWaitStates),
                                "Enable Wait States",
                            )
                            .clicked()
                        {
                            let new_opt = self.get_option(GuiBoolean::CpuEnableWaitStates).unwrap();

                            self.event_queue.send(GuiEvent::VariableChanged(
                                GuiVariableContext::Global,
                                GuiVariable::Bool(GuiBoolean::CpuEnableWaitStates, new_opt),
                            ));
                            ui.close_menu();
                        }
                        if ui
                            .checkbox(
                                &mut self.get_option_mut(GuiBoolean::CpuInstructionHistory),
                                "Instruction History",
                            )
                            .clicked()
                        {
                            let new_opt = self.get_option(GuiBoolean::CpuInstructionHistory).unwrap();

                            self.event_queue.send(GuiEvent::VariableChanged(
                                GuiVariableContext::Global,
                                GuiVariable::Bool(GuiBoolean::CpuInstructionHistory, new_opt),
                            ));
                            ui.close_menu();
                        }
                        if ui
                            .checkbox(
                                &mut self.get_option_mut(GuiBoolean::CpuTraceLoggingEnabled),
                                "Trace Logging Enabled",
                            )
                            .clicked()
                        {
                            let new_opt = self.get_option(GuiBoolean::CpuTraceLoggingEnabled).unwrap();

                            self.event_queue.send(GuiEvent::VariableChanged(
                                GuiVariableContext::Global,
                                GuiVariable::Bool(GuiBoolean::CpuTraceLoggingEnabled, new_opt),
                            ));
                            ui.close_menu();
                        }
                        #[cfg(feature = "devtools")]
                        if ui.button("Delays...").clicked() {
                            *self.window_flag(GuiWindow::DelayAdjust) = true;
                            ui.close_menu();
                        }

                        if ui.button("Trigger NMI").clicked() {
                            self.event_queue.send(GuiEvent::SetNMI(true));
                            ui.close_menu();
                        }

                        if ui.button("Clear NMI").clicked() {
                            self.event_queue.send(GuiEvent::SetNMI(false));
                            ui.close_menu();
                        }
                    });

                    self.workspace_window_open_button(ui, GuiWindow::InstructionHistoryViewer, true, true);
                    self.workspace_window_open_button(ui, GuiWindow::CycleTraceViewer, true, true);
                    self.workspace_window_open_button(ui, GuiWindow::CallStack, true, true);
                    self.workspace_window_open_button(ui, GuiWindow::DisassemblyViewer, true, true);

                    ui.menu_button("Disassembly Listing", |ui| {
                        if ui.button("⏺ Start Recording").clicked() {
                            self.event_queue.send(GuiEvent::StartRecordingDisassembly);
                            ui.close_menu();
                        }
                        if ui.button("⏹ Stop Recording and Save").clicked() {
                            self.event_queue.send(GuiEvent::StopRecordingDisassembly);
                            ui.close_menu();
                        }
                    });
                });

                ui.menu_button("Memory", |ui| {
                    self.workspace_window_open_button(ui, GuiWindow::MemoryViewer, true, true);
                    self.workspace_window_open_button(ui, GuiWindow::DataVisualizer, true, true);
                    self.workspace_window_open_button(ui, GuiWindow::IvtViewer, true, true);

                    ui.menu_button("Dump Memory", |ui| {
                        if ui.button("Video Memory").clicked() {
                            self.event_queue.send(GuiEvent::DumpVRAM);
                            ui.close_menu();
                        }
                        if ui.button("Code Segment").clicked() {
                            self.event_queue.send(GuiEvent::DumpCS);
                            ui.close_menu();
                        }
                        if ui.button("All Memory").clicked() {
                            self.event_queue.send(GuiEvent::DumpAllMem);
                            ui.close_menu();
                        }
                    });
                });

                ui.menu_button("Devices", |ui| {
                    #[cfg(feature = "devtools")]
                    if ui.button("Device control...").clicked() {
                        *self.window_flag(GuiWindow::DeviceControl) = true;
                        ui.close_menu();
                    }
                    self.workspace_window_open_button(ui, GuiWindow::IoStatsViewer, true, true);
                    self.workspace_window_open_button(ui, GuiWindow::PicViewer, true, true);
                    self.workspace_window_open_button(ui, GuiWindow::PitViewer, true, true);
                    self.workspace_window_open_button(ui, GuiWindow::PpiViewer, true, true);
                    self.workspace_window_open_button(ui, GuiWindow::DmaViewer, true, true);
                    self.workspace_window_open_button(ui, GuiWindow::SerialViewer, true, true);
                    self.workspace_window_open_button(ui, GuiWindow::FdcViewer, true, true);
                    self.workspace_window_open_button(ui, GuiWindow::VideoCardViewer, true, true);

                    /*
                    if ui
                        .checkbox(
                            &mut self.get_option_mut(GuiBoolean::ShowBackBuffer),
                            "Debug back buffer",
                        )
                        .clicked()
                    {
                        let new_opt = self.get_option(GuiBoolean::ShowBackBuffer).unwrap();

                        self.event_queue.send(GuiEvent::VariableChanged(
                            GuiVariableContext::Global,
                            GuiVariable::Bool(GuiBoolean::ShowBackBuffer, new_opt),
                        ));
                        ui.close_menu();
                    }
                     */
                });

                if ui
                    .checkbox(&mut self.get_option_mut(GuiBoolean::ShowBackBuffer), "Show Back Buffer")
                    .clicked()
                {
                    let new_opt = self.get_option(GuiBoolean::ShowBackBuffer).unwrap();

                    self.event_queue.send(GuiEvent::VariableChanged(
                        GuiVariableContext::Global,
                        GuiVariable::Bool(GuiBoolean::ShowBackBuffer, new_opt),
                    ));
                }

                if ui
                    .checkbox(
                        &mut self.get_option_mut(GuiBoolean::ShowRasterPosition),
                        "Show Raster Position",
                    )
                    .clicked()
                {
                    let new_opt = self.get_option(GuiBoolean::ShowRasterPosition).unwrap();

                    self.event_queue.send(GuiEvent::VariableChanged(
                        GuiVariableContext::Global,
                        GuiVariable::Bool(GuiBoolean::ShowRasterPosition, new_opt),
                    ));
                }

                if ui.button("Flush Trace Logs").clicked() {
                    self.event_queue.send(GuiEvent::FlushLogs);
                    ui.close_menu();
                }
            });

            // Draw drive indicators, etc.
            self.draw_status_widgets(ui);
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

                if ui.button("🗁 Browse for Image/Zip file...").clicked() {
                    // Do something
                    self.modal.open(
                        ModalContext::OpenFloppyImage(drive_idx, Vec::new()),
                        self.default_floppy_path.clone(),
                    );
                };

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

                // Add 'Save As' options for compatible formats.
                for format_tuple in &self.floppy_drives[drive_idx].supported_formats {
                    let format = format_tuple.0;
                    let extensions = &format_tuple.1;

                    if !extensions.is_empty() {
                        if ui
                            .button(format!("Save As .{}...", extensions[0].to_uppercase()))
                            .clicked()
                        {
                            self.modal.open(
                                ModalContext::SaveFloppyImage(drive_idx, format, extensions.clone()),
                                self.default_floppy_path.clone(),
                            );
                            ui.close_menu();
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
                if ui.button(detatch_string).clicked() {
                    self.event_queue.send(GuiEvent::RemoveCartridge(cart_idx));
                }
            });
        });
    }

    pub fn draw_display_menu(&mut self, ui: &mut egui::Ui, display_idx: usize) {
        let ctx = GuiVariableContext::Display(display_idx);

        ui.menu_button("Scaler Mode", |ui| {
            for (_scaler_idx, mode) in self.scaler_modes.clone().iter().enumerate() {
                if let Some(enum_mut) =
                    self.get_option_enum_mut(GuiEnum::DisplayScalerMode(Default::default()), Some(ctx))
                {
                    let checked = *enum_mut == GuiEnum::DisplayScalerMode(*mode);

                    if ui.add(egui::RadioButton::new(checked, format!("{:?}", mode))).clicked() {
                        *enum_mut = GuiEnum::DisplayScalerMode(*mode);
                        self.event_queue.send(GuiEvent::VariableChanged(
                            GuiVariableContext::Display(display_idx),
                            GuiVariable::Enum(GuiEnum::DisplayScalerMode(*mode)),
                        ));
                    }
                }
            }
        });

        ui.menu_button("Scaler Presets", |ui| {
            for (_preset_idx, preset) in self.scaler_presets.clone().iter().enumerate() {
                if ui.button(preset).clicked() {
                    self.set_option_enum(GuiEnum::DisplayScalerPreset(preset.clone()), Some(ctx));
                    self.event_queue.send(GuiEvent::VariableChanged(
                        GuiVariableContext::Display(display_idx),
                        GuiVariable::Enum(GuiEnum::DisplayScalerPreset(preset.clone())),
                    ));
                    ui.close_menu();
                }
            }
        });

        if ui.button("Scaler Adjustments...").clicked() {
            *self.window_flag(GuiWindow::ScalerAdjust) = true;
            self.scaler_adjust.select_card(display_idx);
            ui.close_menu();
        }

        ui.menu_button("Display Aperture", |ui| {
            let mut aperture_vec = Vec::new();
            if let Some(aperture_vec_ref) = self.display_apertures.get(&display_idx) {
                aperture_vec = aperture_vec_ref.clone()
            };

            for aperture in aperture_vec.iter() {
                if let Some(enum_mut) =
                    self.get_option_enum_mut(GuiEnum::DisplayAperture(Default::default()), Some(ctx))
                {
                    let checked = *enum_mut == GuiEnum::DisplayAperture(aperture.aper_enum);

                    if ui.add(egui::RadioButton::new(checked, aperture.name)).clicked() {
                        *enum_mut = GuiEnum::DisplayAperture(aperture.aper_enum);
                        self.event_queue.send(GuiEvent::VariableChanged(
                            GuiVariableContext::Display(display_idx),
                            GuiVariable::Enum(GuiEnum::DisplayAperture(aperture.aper_enum)),
                        ));
                    }
                }
            }
        });

        let mut state_changed = false;
        let mut new_state = false;
        if let Some(GuiEnum::DisplayAspectCorrect(state)) =
            &mut self.get_option_enum_mut(GuiEnum::DisplayAspectCorrect(false), Some(ctx))
        {
            if ui.checkbox(state, "Correct Aspect Ratio").clicked() {
                //let new_opt = self.get_option_enum_mut()
                state_changed = true;
                new_state = *state;
                ui.close_menu();
            }
        }
        if state_changed {
            self.event_queue.send(GuiEvent::VariableChanged(
                GuiVariableContext::Display(display_idx),
                GuiVariable::Enum(GuiEnum::DisplayAspectCorrect(new_state)),
            ));
        }

        // CGA-specific options.
        if matches!(self.display_info[display_idx].vtype, Some(VideoType::CGA)) {
            let mut state_changed = false;
            let mut new_state = false;

            if let Some(GuiEnum::DisplayComposite(state)) =
                self.get_option_enum_mut(GuiEnum::DisplayComposite(Default::default()), Some(ctx))
            {
                if ui.checkbox(state, "Composite Monitor").clicked() {
                    state_changed = true;
                    new_state = *state;
                    ui.close_menu();
                }
            }
            if state_changed {
                self.event_queue.send(GuiEvent::VariableChanged(
                    GuiVariableContext::Display(display_idx),
                    GuiVariable::Enum(GuiEnum::DisplayComposite(new_state)),
                ));
            }

            /* TODO: Snow should be set per-adapter, not per-display
            if ui
                .checkbox(&mut self.get_option_mut(GuiBoolean::EnableSnow), "Enable Snow")
                .clicked()
            {
                let new_opt = self.get_option(GuiBoolean::EnableSnow).unwrap();

                self.event_queue.send(GuiEvent::OptionChanged(GuiOption::Bool(
                    GuiBoolean::EnableSnow,
                    new_opt,
                )));

                ui.close_menu();
            }
             */

            if ui.button("Composite Adjustments...").clicked() {
                *self.window_flag(GuiWindow::CompositeAdjust) = true;
                self.composite_adjust.select_card(display_idx);
                ui.close_menu();
            }
        }

        self.workspace_window_open_button_with(ui, GuiWindow::TextModeViewer, true, |state| {
            state.text_mode_viewer.select_card(display_idx);
        });

        if ui.button("🖵 Toggle Fullscreen").clicked() {
            self.event_queue.send(GuiEvent::ToggleFullscreen(display_idx));
            ui.close_menu();
        };

        ui.separator();

        if ui.button("🖼 Take Screenshot").clicked() {
            self.event_queue.send(GuiEvent::TakeScreenshot(display_idx));
            ui.close_menu();
        };
    }

    pub fn draw_status_widgets(&mut self, _ui: &mut egui::Ui) {
        // Can we put stuff on the right hand side of the menu bar?
        // ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
        //     ui.label("💾");
        // });
        //
        // ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
        //     ui.label("🐢");
        // });
    }
}

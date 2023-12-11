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

    --------------------------------------------------------------------------

    egui::menu.rs

    Implement the main emulator menu bar.

*/

use crate::{GuiBoolean, GuiEnum, GuiEvent, GuiState, GuiVariable, GuiVariableContext, GuiWindow};
use egui::gui_zoom::zoom_menu_buttons;
use marty_core::videocard::VideoType;
use std::time::Duration;

use marty_core::machine::MachineState;

impl GuiState {
    pub fn draw_menu(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("Emulator", |ui| {
                if ui.button("⏱ Performance...").clicked() {
                    *self.window_flag(GuiWindow::PerfViewer) = true;
                    ui.close_menu();
                }

                ui.menu_button("GUI", |ui| {
                    if ui.button("Toast!").clicked() {
                        self.toasts
                            .info("Hello world!")
                            .set_duration(Some(Duration::from_secs(5)));
                    }
                });

                if ui.button("❓ About...").clicked() {
                    *self.window_flag(GuiWindow::About) = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("🚫 Quit").clicked() {
                    self.event_queue.send(GuiEvent::Exit);
                    ui.close_menu();
                }
            });
            ui.menu_button("Machine", |ui| {
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

            let media_response = ui.menu_button("Media", |ui| {
                let (is_on, _is_paused) = match self.machine_state {
                    MachineState::On => (true, false),
                    MachineState::Paused => (true, true),
                    MachineState::Off => (false, false),
                    _ => (false, false),
                };

                ui.set_min_size(egui::vec2(240.0, 0.0));
                //ui.style_mut().spacing.item_spacing = egui::Vec2{ x: 6.0, y:6.0 };

                ui.menu_button("💾 Load Floppy in Drive A:...", |ui| {
                    for name in &self.floppy_names {
                        ui.set_min_size(egui::vec2(200.0, 0.0));

                        if ui.button(name.to_str().unwrap()).clicked() {
                            log::debug!("Selected floppy filename: {:?}", name);

                            self.floppy0_name = Some(name.clone());
                            self.event_queue.send(GuiEvent::LoadFloppy(0, name.clone()));
                            ui.close_menu();
                        }
                    }
                });

                ui.menu_button("💾 Load Floppy in Drive B:...", |ui| {
                    for name in &self.floppy_names {
                        ui.set_min_size(egui::vec2(200.0, 0.0));

                        if ui.button(name.to_str().unwrap()).clicked() {
                            log::debug!("Selected floppy filename: {:?}", name);

                            self.floppy1_name = Some(name.clone());
                            self.event_queue.send(GuiEvent::LoadFloppy(1, name.clone()));
                            ui.close_menu();
                        }
                    }
                });

                ui.add_enabled_ui(self.floppy0_name.is_some(), |ui| {
                    if ui.button("💾 Save changes to Floppy in Drive A:").clicked() {
                        log::debug!("Saving floppy filename: {:?}", self.floppy0_name);

                        if let Some(name) = &self.floppy0_name {
                            self.event_queue.send(GuiEvent::SaveFloppy(0, name.clone()));
                        }
                        ui.close_menu();
                    }
                });

                ui.add_enabled_ui(self.floppy1_name.is_some(), |ui| {
                    if ui.button("💾 Save changes to Floppy in Drive B:").clicked() {
                        log::debug!("Saving floppy filename: {:?}", self.floppy1_name);

                        if let Some(name) = &self.floppy1_name {
                            self.event_queue.send(GuiEvent::SaveFloppy(1, name.clone()));
                        }
                        ui.close_menu();
                    }
                });

                if ui.button("⏏ Eject Floppy in Drive A:").clicked() {
                    self.event_queue.send(GuiEvent::EjectFloppy(0));
                    self.floppy0_name = None;
                    ui.close_menu();
                };

                if ui.button("⏏ Eject Floppy in Drive B:").clicked() {
                    self.event_queue.send(GuiEvent::EjectFloppy(1));
                    self.floppy1_name = None;
                    ui.close_menu();
                };

                // Only enable VHD loading if machine is off to prevent corruption to VHD.
                ui.add_enabled_ui(!is_on, |ui| {
                    ui.menu_button("🖴 Load VHD in Drive 0:...", |ui| {
                        for name in &self.vhd_names {
                            if ui
                                .radio_value(&mut self.vhd_name0, name.clone(), name.to_str().unwrap())
                                .clicked()
                            {
                                log::debug!("Selected VHD filename: {:?}", name);

                                self.event_queue.send(GuiEvent::LoadVHD(0, name.clone()));
                                self.new_vhd_name0 = Some(name.clone());
                                ui.close_menu();
                            }
                        }
                    });

                    ui.menu_button("🖴 Load VHD in Drive 1:...", |ui| {
                        for name in &self.vhd_names {
                            if ui
                                .radio_value(&mut self.vhd_name1, name.clone(), name.to_str().unwrap())
                                .clicked()
                            {
                                log::debug!("Selected VHD filename: {:?}", name);

                                self.event_queue.send(GuiEvent::LoadVHD(0, name.clone()));
                                self.new_vhd_name1 = Some(name.clone());
                                ui.close_menu();
                            }
                        }
                    });
                });

                if ui.button("🖹 Create new VHD...").clicked() {
                    *self.window_flag(GuiWindow::VHDCreator) = true;
                    ui.close_menu();
                };
            });

            if media_response.response.clicked() {
                self.event_queue.send(GuiEvent::RescanMediaFolders);
            }

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
                if ui.button("CPU Control...").clicked() {
                    *self.window_flag(GuiWindow::CpuControl) = true;
                    ui.close_menu();
                }
                if ui.button("CPU State...").clicked() {
                    *self.window_flag(GuiWindow::CpuStateViewer) = true;
                    ui.close_menu();
                }
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
                if ui.button("Memory...").clicked() {
                    *self.window_flag(GuiWindow::MemoryViewer) = true;
                    ui.close_menu();
                }
                if ui.button("Instruction History...").clicked() {
                    *self.window_flag(GuiWindow::HistoryViewer) = true;
                    ui.close_menu();
                }
                if ui.button("Instruction Cycle Trace...").clicked() {
                    *self.window_flag(GuiWindow::CycleTraceViewer) = true;
                    ui.close_menu();
                }
                if ui.button("Call Stack...").clicked() {
                    *self.window_flag(GuiWindow::CallStack) = true;
                    ui.close_menu();
                }
                if ui.button("Disassembly...").clicked() {
                    *self.window_flag(GuiWindow::DisassemblyViewer) = true;
                    ui.close_menu();
                }
                if ui.button("IVR...").clicked() {
                    *self.window_flag(GuiWindow::IvrViewer) = true;
                    ui.close_menu();
                }
                #[cfg(feature = "devtools")]
                if ui.button("Device control...").clicked() {
                    *self.window_flag(GuiWindow::DeviceControl) = true;
                    ui.close_menu();
                }
                if ui.button("PIC...").clicked() {
                    *self.window_flag(GuiWindow::PicViewer) = true;
                    ui.close_menu();
                }
                if ui.button("PIT...").clicked() {
                    *self.window_flag(GuiWindow::PitViewer) = true;
                    ui.close_menu();
                }
                if ui.button("PPI...").clicked() {
                    *self.window_flag(GuiWindow::PpiViewer) = true;
                    ui.close_menu();
                }
                if ui.button("DMA...").clicked() {
                    *self.window_flag(GuiWindow::DmaViewer) = true;
                    ui.close_menu();
                }
                if ui.button("Video Card...").clicked() {
                    *self.window_flag(GuiWindow::VideoCardViewer) = true;
                    ui.close_menu();
                }

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

                if ui.button("Flush Trace Logs").clicked() {
                    self.event_queue.send(GuiEvent::FlushLogs);
                    ui.close_menu();
                }
            });
            ui.menu_button("Options", |ui| {
                ui.menu_button("Attach COM2: ...", |ui| {
                    for port in &self.serial_ports {
                        if ui
                            .radio_value(
                                &mut self.serial_port_name,
                                port.port_name.clone(),
                                port.port_name.clone(),
                            )
                            .clicked()
                        {
                            self.event_queue
                                .send(GuiEvent::BridgeSerialPort(self.serial_port_name.clone()));
                            ui.close_menu();
                        }
                    }
                });
            });

            // Draw drive indicators, etc.
            self.draw_status_widgets(ui);
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

        if ui.button("Scaler Adjustments...").clicked() {
            *self.window_flag(GuiWindow::ScalerAdjust) = true;
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
                ui.close_menu();
            }
        }

        ui.separator();

        if ui.button("🖼 Take Screenshot").clicked() {
            self.event_queue.send(GuiEvent::TakeScreenshot);
            ui.close_menu();
        };
    }

    pub fn draw_status_widgets(&mut self, ui: &mut egui::Ui) {
        // Can we put stuff on the right hand side of the menu bar?
        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            ui.label("💾");
        });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            ui.label("🐢");
        });
    }
}

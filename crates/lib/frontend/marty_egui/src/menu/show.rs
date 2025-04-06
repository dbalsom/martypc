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

//! Implement the main emulator menu.

use crate::{state::GuiState, GuiBoolean, GuiEvent, GuiFloat, GuiVariable, GuiVariableContext, GuiWindow};
use marty_core::machine::MachineState;
use marty_display_common::display_manager::DtHandle;

use egui::RichText;

impl GuiState {
    pub fn show_menu(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("Emulator", |ui| {
                ui.set_min_width(120.0);

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
                ui.menu_button("Emulation Speed", |ui| {
                    ui.horizontal(|ui| {
                        let mut speed = self.option_floats.get_mut(&GuiFloat::EmulationSpeed).unwrap();

                        ui.label("Factor:");
                        if ui
                            .add(
                                egui::Slider::new(speed, 0.1..=2.0)
                                    .show_value(true)
                                    .min_decimals(2)
                                    .max_decimals(2)
                                    .suffix("x"),
                            )
                            .changed()
                        {
                            self.event_queue.send(GuiEvent::VariableChanged(
                                GuiVariableContext::Global,
                                GuiVariable::Float(GuiFloat::EmulationSpeed, *speed),
                            ));
                        }
                    });
                });

                ui.menu_button("Input/Output", |ui| {
                    self.show_input_menu(ui);
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

            ui.menu_button("Sound", |ui| {
                ui.set_min_width(240.0);
                if !self.sound_sources.is_empty() {
                    self.draw_sound_menu(ui);
                }
                else {
                    ui.label(RichText::new("No sound sources available.").italics());
                }
            });

            ui.menu_button("Display", |ui| {
                ui.set_min_size(egui::vec2(240.0, 0.0));

                // If there is only one display, emit the display menu directly.
                // Otherwise, emit named menus for each display.
                if self.display_info.len() == 1 {
                    self.draw_display_menu(ui, DtHandle::default());
                }
                else if self.display_info.len() > 1 {
                    // Use index here to avoid borrowing issues.
                    for i in 0..self.display_info.len() {
                        ui.menu_button(format!("Display {}: {}", i, &self.display_info[i].name), |ui| {
                            self.draw_display_menu(ui, self.display_info[i].handle);
                        });
                    }
                }
            });

            self.draw_debug_menu(ui);

            // Draw drive indicators, etc.
            self.draw_status_widgets(ui);
        });
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

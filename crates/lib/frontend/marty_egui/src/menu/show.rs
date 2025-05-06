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

                #[cfg(not(target_arch = "wasm32"))]
                if ui.button("⎆ Quit").clicked() {
                    self.event_queue.send(GuiEvent::Exit);
                    ui.close_menu();
                }
            });

            // Only show the Emulator menu if a modal dialog is open.
            if self.modal.is_open() {
                return;
            }

            self.show_machine_menu(ui);

            self.show_media_menu(ui);

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

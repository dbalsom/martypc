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
use crate::{state::GuiState, GuiBoolean, GuiEvent, GuiFloat, GuiVariable, GuiVariableContext};
use marty_core::machine::MachineState;

impl GuiState {
    pub fn show_machine_menu(&mut self, ui: &mut egui::Ui) {
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
    }
}

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
use crate::{state::GuiState, GuiEnum, GuiEvent, GuiFloat, GuiVariable, GuiVariableContext};
use marty_common::types::{joystick::ControllerLayout, ui::MouseCaptureMode};
use strum::IntoEnumIterator;

impl GuiState {
    pub fn show_input_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Serial Ports", |ui| {
            for port in self.serial_ports.clone() {
                ui.menu_button(&port.name, |ui| {
                    #[cfg(feature = "use_serialport")]
                    {
                        // Create a vector of ports that are currently bridged. We will use this to disable
                        // those ports from selection in the menu.
                        let bridged_ports = self
                            .serial_ports
                            .iter()
                            .filter_map(|port| port.brige_port_id)
                            .collect::<Vec<_>>();

                        let mut selected = false;

                        for (host_port_id, host_port) in self.host_serial_ports.iter().enumerate() {
                            if let Some(enum_mut) = self.get_option_enum(
                                GuiEnum::SerialPortBridge(Default::default()),
                                Some(GuiVariableContext::SerialPort(port.id)),
                            ) {
                                selected = *enum_mut == GuiEnum::SerialPortBridge(host_port_id);
                            }

                            let enabled = !bridged_ports.contains(&host_port_id);

                            let port_string = format!("Host port {}", host_port.port_name);
                            if ui
                                .add_enabled(enabled, egui::RadioButton::new(selected, port_string))
                                .clicked()
                            {
                                self.event_queue.send(GuiEvent::BridgeSerialPort(
                                    port.id,
                                    host_port.port_name.clone(),
                                    host_port_id,
                                ));
                                ui.close_menu();
                            }
                        }
                    }
                });
            }
        });

        ui.menu_button("Mouse", |ui| {
            ui.menu_button("Capture Mode", |ui| {
                for mode in MouseCaptureMode::iter() {
                    let mode_mut = self
                        .get_option_enum_mut(GuiEnum::MouseCaptureMode(MouseCaptureMode::default()), None)
                        .unwrap();
                    if let GuiEnum::MouseCaptureMode(mode_inner) = mode_mut {
                        let mut checked = *mode_inner == mode;
                        if ui.checkbox(&mut checked, &mode.to_string()).changed() {
                            if checked {
                                *mode_inner = mode;
                                let capture_enum = GuiEnum::MouseCaptureMode(mode.clone());
                                self.event_queue.send(GuiEvent::VariableChanged(
                                    GuiVariableContext::Global,
                                    GuiVariable::Enum(capture_enum),
                                ));
                            }
                        }
                    }
                }
            });
            ui.menu_button("Speed", |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        let speed = self.option_floats.get_mut(&GuiFloat::MouseSpeed).unwrap();
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
                                GuiVariable::Float(GuiFloat::MouseSpeed, *speed),
                            ));
                        }
                    });
                });
            });
        });

        ui.menu_button("Keyboard", |ui| {
            if ui.button("Reset keyboard").clicked() {
                self.event_queue.send(GuiEvent::ClearKeyboard);
                ui.close_menu();
            }
        });

        // Only show the game port menu if we have a game port, naturally
        if self.gameport {
            let mut enum_event = None;

            ui.menu_button("Game Port", |ui| {
                if ui.button("Joykeys").clicked() {
                    log::debug!("Clicked on joykey option.");
                    ui.close_menu();
                }

                match self.controller_layout {
                    ControllerLayout::TwoJoysticksTwoButtons => {
                        for i in 0..2 {
                            ui.menu_button(format!("Joystick {}", i + 1), |ui| {
                                let mut clicked_id = None;

                                ui.vertical(|ui| {
                                    let no_joystick = self.selected_gamepad[i].is_none();
                                    if ui.radio(no_joystick, "None").clicked() {
                                        log::debug!("Selected no joystick");
                                        self.selected_gamepad[i] = None;
                                        let mapping_enum_mut = self
                                            .get_option_enum_mut(GuiEnum::GamepadMapping((None, None)), None)
                                            .unwrap();

                                        if let GuiEnum::GamepadMapping(mapping) = mapping_enum_mut {
                                            log::debug!("Updating gamepad mapping for joystick: {} to None", i);

                                            *mapping_enum_mut = match i {
                                                0 => GuiEnum::GamepadMapping((None, mapping.1)),
                                                1 => GuiEnum::GamepadMapping((mapping.0, None)),
                                                _ => unreachable!(),
                                            };

                                            // Defer sending the event due to borrow checker being mean
                                            enum_event = Some(GuiEvent::VariableChanged(
                                                GuiVariableContext::Global,
                                                GuiVariable::Enum(mapping_enum_mut.clone()),
                                            ));
                                        }
                                    }

                                    for gamepad in &self.gamepads {
                                        let gamepad_selected = Some(gamepad.internal_id) == self.selected_gamepad[i];

                                        ui.horizontal(|ui| {
                                            if ui
                                                .radio(gamepad_selected, format!("{}: {}", gamepad.id, gamepad.name))
                                                .clicked()
                                            {
                                                log::debug!("Selected gamepad {}, id: {}", gamepad.name, gamepad.id);
                                                clicked_id = Some(gamepad.internal_id);
                                            }
                                        });
                                    }

                                    if let Some(clicked_id) = clicked_id {
                                        self.selected_gamepad[i] = Some(clicked_id);
                                        let mapping_enum_mut = self
                                            .get_option_enum_mut(GuiEnum::GamepadMapping((None, None)), None)
                                            .unwrap();

                                        if let GuiEnum::GamepadMapping(mapping) = mapping_enum_mut {
                                            log::debug!("Updating gamepad mapping for id: {}", clicked_id);

                                            *mapping_enum_mut = match i {
                                                0 => GuiEnum::GamepadMapping((Some(clicked_id), mapping.1)),
                                                1 => GuiEnum::GamepadMapping((mapping.0, Some(clicked_id))),
                                                _ => unreachable!(),
                                            };

                                            // Defer sending the event due to borrow checker being mean
                                            enum_event = Some(GuiEvent::VariableChanged(
                                                GuiVariableContext::Global,
                                                GuiVariable::Enum(mapping_enum_mut.clone()),
                                            ));
                                        }
                                    }
                                });
                            });
                        }
                    }
                    ControllerLayout::OneJoystickFourButtons => {
                        for gamepad in &self.gamepads {
                            let gamepad_selected = Some(gamepad.internal_id) == self.selected_gamepad[0];
                            let mut clicked_id = None;

                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    if ui
                                        .radio(gamepad_selected, format!("{}: {}", gamepad.id, gamepad.name))
                                        .changed()
                                    {
                                        log::debug!("Selected gamepad {}", gamepad.name);
                                        clicked_id = Some(gamepad.internal_id);
                                    }
                                });
                            });
                        }
                    }
                }
            });

            if let Some(event) = enum_event {
                self.event_queue.send(event);
            }
        }
    }
}

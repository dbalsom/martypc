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

    --------------------------------------------------------------------------
*/
use crate::{state::GuiState, GuiBoolean, GuiEnum, GuiEvent, GuiFloat, GuiVariable, GuiVariableContext};
use marty_common::types::joystick::ControllerLayout;
#[cfg(feature = "use_serial_bridge")]
use marty_core::devices::serial_bridge::{SerialPortBridgeState, SerialPortBridgeTransport};
use marty_frontend_common::types::gamepad::JoystickMapping;

impl GuiState {
    pub fn show_input_menu(&mut self, ui: &mut egui::Ui) {
        self.show_serial_menu(ui);
        self.show_mouse_menu(ui);
        self.show_lightpen_menu(ui);
        self.show_keyboard_menu(ui);
        self.show_game_port_menu(ui);
    }

    fn show_serial_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Serial Ports", |ui| {
            for port in self.serial_ports.clone() {
                ui.menu_button(&port.name, |ui| {
                    #[cfg(feature = "use_serial_bridge")]
                    if let Some(bridge) = port.bridge.as_ref() {
                        let transport_label = match bridge.transport {
                            SerialPortBridgeTransport::Serial => "Host serial",
                            SerialPortBridgeTransport::TcpClient => "TCP Client",
                        };
                        let bridge_label = match bridge.state {
                            SerialPortBridgeState::WaitingForOutput => {
                                format!(
                                    "{}: (Waiting for output to connect to \"{}\")",
                                    transport_label, bridge.target
                                )
                            }
                            SerialPortBridgeState::WaitingForRequest => {
                                format!(
                                    "{}: (Waiting for request to connect to \"{}\")",
                                    transport_label, bridge.target
                                )
                            }
                            SerialPortBridgeState::Connecting => {
                                format!("{}: (Connecting to \"{}\")", transport_label, bridge.target)
                            }
                            SerialPortBridgeState::Connected => {
                                format!("{}: (Connected to \"{}\")", transport_label, bridge.target)
                            }
                            SerialPortBridgeState::ReconnectPending => {
                                format!("{}: (Waiting to reconnect to \"{}\")", transport_label, bridge.target)
                            }
                            SerialPortBridgeState::Suspended => {
                                format!("{}: (Disconnected from \"{}\")", transport_label, bridge.target)
                            }
                        };

                        ui.horizontal(|ui| {
                            ui.label(bridge_label)
                                .on_hover_text(bridge.last_error.as_deref().unwrap_or("No bridge error"));
                        });

                        if bridge.state == SerialPortBridgeState::Connected {
                            ui.horizontal(|ui| {
                                if ui.button("🚫 Disconnect").clicked() {
                                    self.event_queue.send(GuiEvent::DisconnectSerialBridge(port.id));
                                    ui.close();
                                }
                            });
                        }

                        if bridge.state != SerialPortBridgeState::Connected {
                            ui.horizontal(|ui| {
                                if ui.button("⟲ Reconnect now").clicked() {
                                    self.event_queue.send(GuiEvent::ReconnectSerialBridge(port.id));
                                    ui.close();
                                }
                            });
                        }

                        ui.horizontal(|ui| {
                            if ui.button("🔌 Detach Bridge").clicked() {
                                self.event_queue.send(GuiEvent::DetachSerialBridge(port.id));
                                ui.close();
                            }
                        });
                    }

                    #[cfg(feature = "use_serial_bridge")]
                    if port.bridge.is_none() {
                        for connection in &self.serial_bridge_connections {
                            let transport = connection.target.transport();
                            let port_name = connection.target.label();
                            let enabled = !self
                                .serial_ports
                                .iter()
                                .filter_map(|port| port.bridge.as_ref())
                                .any(|bridge| bridge.transport == transport && bridge.target == port_name);
                            let label = match transport {
                                SerialPortBridgeTransport::Serial => format!("Host serial: \"{port_name}\""),
                                SerialPortBridgeTransport::TcpClient => {
                                    format!("TCP Client: \"{port_name}\"")
                                }
                            };

                            ui.horizontal(|ui| {
                                if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                                    self.event_queue
                                        .send(GuiEvent::BridgeSerialConnection(port.id, connection.clone()));
                                    ui.close();
                                }
                            });
                        }
                    }

                    #[cfg(feature = "use_serialport")]
                    if port.bridge.is_none() {
                        let bridged_host_ports = self
                            .serial_ports
                            .iter()
                            .filter_map(|port| port.bridge.as_ref())
                            .filter(|bridge| bridge.transport == SerialPortBridgeTransport::Serial)
                            .map(|bridge| bridge.target.as_str())
                            .collect::<Vec<_>>();

                        for host_port in &self.host_serial_ports {
                            let has_configured_connection = self.serial_bridge_connections.iter().any(|connection| {
                                connection.target.transport() == SerialPortBridgeTransport::Serial
                                    && connection.target.label() == host_port.port_name
                            });
                            if has_configured_connection {
                                continue;
                            }

                            let enabled = !bridged_host_ports.contains(&host_port.port_name.as_str());
                            let label = format!("Host serial: \"{}\"", host_port.port_name);
                            ui.horizontal(|ui| {
                                if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                                    self.event_queue
                                        .send(GuiEvent::BridgeSerialPort(port.id, host_port.port_name.clone()));
                                    ui.close();
                                }
                            });
                        }
                    }
                });
            }
        });
    }

    fn show_mouse_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Mouse", |ui| {
            let mut enabled = self.get_option(GuiBoolean::MouseEnabled).unwrap_or(true);
            if ui.checkbox(&mut enabled, "Enabled").changed() {
                self.set_option(GuiBoolean::MouseEnabled, enabled);
                self.event_queue.send(GuiEvent::VariableChanged(
                    GuiVariableContext::Global,
                    GuiVariable::Bool(GuiBoolean::MouseEnabled, enabled),
                ));
            }

            ui.separator();
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
    }

    fn show_lightpen_menu(&mut self, ui: &mut egui::Ui) {
        if self.lightpen_available {
            ui.menu_button("Light Pen", |ui| {
                let mut enabled = self.get_option(GuiBoolean::LightPenEnabled).unwrap_or(false);
                if ui.checkbox(&mut enabled, "Enabled").changed() {
                    self.set_option(GuiBoolean::LightPenEnabled, enabled);
                    self.event_queue.send(GuiEvent::VariableChanged(
                        GuiVariableContext::Global,
                        GuiVariable::Bool(GuiBoolean::LightPenEnabled, enabled),
                    ));
                }
            });
        }
    }

    fn show_keyboard_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Keyboard", |ui| {
            let keyboard_available = self.osd_keyboard_available();
            let mut osd_keyboard_enabled = self.get_option(GuiBoolean::OsdKeyboard).unwrap_or(false);
            if ui
                .add_enabled(
                    keyboard_available,
                    egui::Checkbox::new(&mut osd_keyboard_enabled, "On-screen keyboard"),
                )
                .changed()
            {
                self.set_osd_keyboard_enabled(osd_keyboard_enabled);
                self.event_queue.send(GuiEvent::VariableChanged(
                    GuiVariableContext::Global,
                    GuiVariable::Bool(GuiBoolean::OsdKeyboard, osd_keyboard_enabled),
                ));
                ui.close();
            }

            ui.separator();
            if ui.button("Reset keyboard").clicked() {
                self.event_queue.send(GuiEvent::ClearKeyboard);
                ui.close();
            }
        });
    }

    fn show_game_port_menu(&mut self, ui: &mut egui::Ui) {
        // Only show the game port menu if we have a game port, naturally
        if self.gameport {
            let mut enum_event = None;

            ui.menu_button("Game Port", |ui| {
                match self.controller_layout {
                    ControllerLayout::TwoJoysticksTwoButtons => {
                        for i in 0..2 {
                            ui.menu_button(format!("Joystick {}", i + 1), |ui| {
                                let mut gamepad_clicked_id = None;
                                let mut joykeys_clicked = false;

                                ui.vertical(|ui| {
                                    let no_joystick = self.selected_joystick_mapping[i].is_none();
                                    if ui.radio(no_joystick, "None").clicked() {
                                        log::debug!("Selected no joystick");
                                        self.selected_joystick_mapping[i] = None;
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

                                    let joykeys_selected =
                                        Some(JoystickMapping::JoyKeys) == self.selected_joystick_mapping[i];
                                    ui.horizontal(|ui| {
                                        let enabled = self.selected_joystick_mapping[opposite_joystick(i)]
                                            .is_none_or(|m| matches!(m, JoystickMapping::Gamepad(_)));

                                        if ui
                                            .add_enabled(enabled, egui::RadioButton::new(joykeys_selected, "JoyKeys"))
                                            .clicked()
                                        {
                                            log::debug!("Selected Joykeys for joystick {}", i);
                                            joykeys_clicked = true;

                                            self.selected_joystick_mapping[i] = Some(JoystickMapping::JoyKeys);
                                            let mapping_enum_mut = self
                                                .get_option_enum_mut(GuiEnum::GamepadMapping((None, None)), None)
                                                .unwrap();

                                            if let GuiEnum::GamepadMapping(mapping) = mapping_enum_mut {
                                                *mapping_enum_mut = match i {
                                                    0 => GuiEnum::GamepadMapping((
                                                        Some(JoystickMapping::JoyKeys),
                                                        mapping.1,
                                                    )),
                                                    1 => GuiEnum::GamepadMapping((
                                                        mapping.0,
                                                        Some(JoystickMapping::JoyKeys),
                                                    )),
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

                                    for gamepad in &self.gamepads {
                                        let gamepad_selected = Some(JoystickMapping::Gamepad(gamepad.internal_id))
                                            == self.selected_joystick_mapping[i];

                                        ui.horizontal(|ui| {
                                            let id = gamepad.internal_id;
                                            let enabled = self.selected_joystick_mapping[opposite_joystick(i)]
                                                .is_none_or(
                                                    |m| !matches!(m, JoystickMapping::Gamepad(gid) if gid == id),
                                                );

                                            if ui
                                                .add_enabled(
                                                    enabled,
                                                    egui::RadioButton::new(
                                                        gamepad_selected,
                                                        format!("{}: {}", gamepad.id, gamepad.name),
                                                    ),
                                                )
                                                .clicked()
                                            {
                                                log::debug!("Selected gamepad {}, id: {}", gamepad.name, gamepad.id);
                                                gamepad_clicked_id = Some(gamepad.internal_id);
                                            }
                                        });
                                    }

                                    if let Some(clicked_id) = gamepad_clicked_id {
                                        self.selected_joystick_mapping[i] = Some(JoystickMapping::Gamepad(clicked_id));
                                        let mapping_enum_mut = self
                                            .get_option_enum_mut(GuiEnum::GamepadMapping((None, None)), None)
                                            .unwrap();

                                        if let GuiEnum::GamepadMapping(mapping) = mapping_enum_mut {
                                            log::debug!("Updating gamepad mapping for id: {}", clicked_id);

                                            *mapping_enum_mut = match i {
                                                0 => GuiEnum::GamepadMapping((
                                                    Some(JoystickMapping::Gamepad(clicked_id)),
                                                    mapping.1,
                                                )),
                                                1 => GuiEnum::GamepadMapping((
                                                    mapping.0,
                                                    Some(JoystickMapping::Gamepad(clicked_id)),
                                                )),
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
                            let mut clicked_id = None;

                            ui.vertical(|ui| {
                                let gamepad_selected = Some(JoystickMapping::Gamepad(gamepad.internal_id))
                                    == self.selected_joystick_mapping[0];
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

#[inline]
fn opposite_joystick(slot: usize) -> usize {
    if slot == 0 {
        1
    }
    else {
        0
    }
}

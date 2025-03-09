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
use crate::{
    state::GuiState,
    widgets::big_icon::BigIcon,
    GuiEnum,
    GuiEvent,
    GuiFloat,
    GuiVariable,
    GuiVariableContext,
};
use marty_core::devices::serial::SerialPortDescriptor;

impl GuiState {
    pub fn show_input_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Serial Ports", |ui| {
            for port in &self.serial_ports {
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
                                        .add_enabled(
                                            enabled,
                                            egui::RadioButton::new(selected, host_port.port_name.clone()),
                                        )
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
                    }
                });
            }
        });

        ui.menu_button("Mouse", |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.horizontal(|ui| {
                        let mut speed = self.option_floats.get_mut(&GuiFloat::MouseSpeed).unwrap();

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
                                GuiVariable::Float(GuiFloat::MouseSpeed, *speed),
                            ));
                        }
                    });
                });
            });
        });
    }
}

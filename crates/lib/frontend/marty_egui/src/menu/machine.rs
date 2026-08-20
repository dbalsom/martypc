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
use crate::{state::GuiState, GuiBoolean, GuiEvent, GuiFloat, GuiVariable, GuiVariableContext};
use marty_core::machine::{ExecutionOperation, MachineState};

const NORMAL_EMULATION_SPEED: f32 = 1.0;
const NORMAL_SPEED_SLIDER_POSITION: f32 = 0.5;

fn slider_position_to_speed(position: f32, min: f32, max: f32) -> f32 {
    let position = position.clamp(0.0, 1.0);

    if position <= NORMAL_SPEED_SLIDER_POSITION {
        min + (NORMAL_EMULATION_SPEED - min) * (position / NORMAL_SPEED_SLIDER_POSITION)
    }
    else {
        NORMAL_EMULATION_SPEED
            + (max - NORMAL_EMULATION_SPEED)
                * ((position - NORMAL_SPEED_SLIDER_POSITION) / NORMAL_SPEED_SLIDER_POSITION)
    }
}

fn speed_to_slider_position(speed: f32, min: f32, max: f32) -> f32 {
    let speed = speed.clamp(min, max);

    if speed < NORMAL_EMULATION_SPEED && min < NORMAL_EMULATION_SPEED {
        NORMAL_SPEED_SLIDER_POSITION * (speed - min) / (NORMAL_EMULATION_SPEED - min)
    }
    else if speed > NORMAL_EMULATION_SPEED && max > NORMAL_EMULATION_SPEED {
        NORMAL_SPEED_SLIDER_POSITION
            + NORMAL_SPEED_SLIDER_POSITION * (speed - NORMAL_EMULATION_SPEED) / (max - NORMAL_EMULATION_SPEED)
    }
    else {
        NORMAL_SPEED_SLIDER_POSITION
    }
}

fn format_speed_percentage(position: f32, min: f32, max: f32) -> String {
    let percentage = slider_position_to_speed(position, min, max) * 100.0;
    let mut formatted = format!("{percentage:.1}");
    if formatted.ends_with(".0") {
        formatted.truncate(formatted.len() - 2);
    }
    formatted
}

fn parse_speed_percentage(text: &str, min: f32, max: f32) -> Option<f64> {
    let percentage = text.trim().trim_end_matches('%').trim().parse::<f32>().ok()?;
    let speed = percentage / 100.0;

    Some(speed_to_slider_position(speed, min, max) as f64)
}

impl GuiState {
    pub fn show_machine_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Machine", |ui| {
            egui::containers::menu::SubMenuButton::new("Emulation Speed")
                .config(
                    egui::containers::menu::MenuConfig::new()
                        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
                )
                .ui(ui, |ui| {
                    let min = self.min_emulation_speed;
                    let max = self.max_emulation_speed;
                    let mut updated_speed = None;

                    ui.horizontal(|ui| {
                        let speed = self.option_floats.get_mut(&GuiFloat::EmulationSpeed).unwrap();
                        let mut slider_position = speed_to_slider_position(*speed, min, max);

                        ui.label("Speed:");
                        let slider_changed = ui
                            .add(
                                egui::Slider::new(&mut slider_position, 0.0..=1.0)
                                    .show_value(true)
                                    .custom_formatter(move |position, _| {
                                        format_speed_percentage(position as f32, min, max)
                                    })
                                    .custom_parser(move |text| parse_speed_percentage(text, min, max))
                                    .suffix("%"),
                            )
                            .changed();
                        let reset_clicked = ui.button("⟲").on_hover_text("Reset emulation speed to 100%").clicked();

                        if slider_changed || reset_clicked {
                            *speed = if reset_clicked {
                                NORMAL_EMULATION_SPEED
                            }
                            else {
                                slider_position_to_speed(slider_position, min, max)
                            };
                            updated_speed = Some(*speed);
                        }
                    });

                    if let Some(speed) = updated_speed {
                        self.event_queue.send(GuiEvent::VariableChanged(
                            GuiVariableContext::Global,
                            GuiVariable::Float(GuiFloat::EmulationSpeed, speed),
                        ));
                    }
                });

            ui.menu_button("Input/Output", |ui| {
                self.show_input_menu(ui);
            });

            ui.separator();

            let is_on = self.machine_state.is_on();
            let exec_state = self.exec_control.borrow().get_state();
            let can_pause = exec_state.can_pause();
            let can_resume = exec_state.can_run();

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

            ui.add_enabled_ui(is_on && can_pause, |ui| {
                if ui.button("⏸ Pause").clicked() {
                    self.exec_control.borrow_mut().set_op(ExecutionOperation::Pause);
                    ui.close_menu();
                }
            });

            ui.add_enabled_ui(is_on && can_resume, |ui| {
                if ui.button("▶ Resume").clicked() {
                    self.exec_control.borrow_mut().set_op(ExecutionOperation::Run);
                    ui.close_menu();
                }
            });

            ui.add_enabled_ui(is_on, |ui| {
                if ui.button("⟲ Reboot").clicked() {
                    self.event_queue.send(GuiEvent::Reboot);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_approx_eq(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < f32::EPSILON * 8.0, "{actual} != {expected}");
    }

    #[test]
    fn slider_midpoint_is_normal_speed() {
        assert_approx_eq(slider_position_to_speed(0.0, 0.25, 4.0), 0.25);
        assert_approx_eq(slider_position_to_speed(0.5, 0.25, 4.0), 1.0);
        assert_approx_eq(slider_position_to_speed(1.0, 0.25, 4.0), 4.0);
    }

    #[test]
    fn each_half_of_slider_maps_linearly() {
        assert_approx_eq(slider_position_to_speed(0.25, 0.25, 4.0), 0.625);
        assert_approx_eq(slider_position_to_speed(0.75, 0.25, 4.0), 2.5);
    }

    #[test]
    fn speed_mapping_round_trips() {
        for speed in [0.25, 0.625, 1.0, 2.5, 4.0] {
            let position = speed_to_slider_position(speed, 0.25, 4.0);
            assert_approx_eq(slider_position_to_speed(position, 0.25, 4.0), speed);
        }
    }

    #[test]
    fn speed_percentage_uses_at_most_one_decimal_place() {
        assert_eq!(format_speed_percentage(0.5, 0.25, 4.0), "100");
        assert_eq!(format_speed_percentage(0.25, 0.25, 4.0), "62.5");
        assert_eq!(format_speed_percentage(0.75, 0.25, 4.0), "250");
    }

    #[test]
    fn percentage_input_maps_to_slider_position() {
        assert_approx_eq(parse_speed_percentage("100%", 0.25, 4.0).unwrap() as f32, 0.5);
        assert_approx_eq(parse_speed_percentage("62.5", 0.25, 4.0).unwrap() as f32, 0.25);
    }
}

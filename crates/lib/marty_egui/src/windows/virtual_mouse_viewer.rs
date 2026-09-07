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

use marty_core::devices::mouse::{VirtualMouseDebugState, VirtualMouseInputMode};

use crate::GuiEventQueue;

pub struct VirtualMouseViewerControl {
    state: Option<VirtualMouseDebugState>,
}

impl VirtualMouseViewerControl {
    pub fn new() -> Self {
        Self { state: None }
    }

    pub fn update_state(&mut self, state: Option<VirtualMouseDebugState>) {
        self.state = state;
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
        let Some(device) = self.state
        else {
            ui.label("No virtual mouse is configured for this machine.");
            return;
        };

        egui::Grid::new("virtual_mouse_device_state")
            .num_columns(2)
            .spacing([24.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                Self::state_row(
                    ui,
                    "Capture mode",
                    match device.input_mode {
                        VirtualMouseInputMode::Absolute => "Uncaptured (absolute)",
                        VirtualMouseInputMode::Relative => "Captured (relative)",
                    },
                );
                Self::state_row(ui, "Device coordinates", format!("{}, {}", device.x, device.y));
                Self::state_row(
                    ui,
                    "Current buttons",
                    button_state(device.left_button, device.right_button),
                );
                Self::state_row(ui, "Reported bitmap", format!("{:04X}h", device.buttons));
                Self::state_row(
                    ui,
                    "Latched presses",
                    button_state(device.left_press_pending, device.right_press_pending),
                );
                Self::state_row(
                    ui,
                    "Change counter",
                    format!("{} ({:04X}h)", device.change_counter, device.change_counter),
                );
                Self::state_row(ui, "Sensitivity", format!("{:.2}x", device.speed));
                Self::state_row(ui, "IRQ", device.irq.to_string());
                Self::state_row(ui, "Event pending", yes_no(device.event_pending));
                Self::state_row(ui, "IRQ asserted", yes_no(device.interrupt_asserted));
                Self::state_row(ui, "IRQ clear pending", yes_no(device.lower_interrupt));
            });

        ui.add_space(8.0);
        ui.separator();
        ui.heading("DOS consumer");
        egui::Grid::new("virtual_mouse_consumer_state")
            .num_columns(2)
            .spacing([24.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                Self::state_row(ui, "Driver loaded", yes_no(device.consumer_driver_loaded));
                if let Some(range) = device.consumer_range {
                    Self::state_row(ui, "INT 33h X range", format!("{}..{}", range.min_x, range.max_x));
                    Self::state_row(ui, "INT 33h Y range", format!("{}..{}", range.min_y, range.max_y));
                    Self::state_row(
                        ui,
                        "Range-mapped position",
                        format!(
                            "{}, {}",
                            map_axis(device.x, range.min_x, range.max_x),
                            map_axis(device.y, range.min_y, range.max_y)
                        ),
                    );
                }
                else {
                    Self::state_row(ui, "INT 33h range", "Not reported");
                }
            });
    }

    fn state_row(ui: &mut egui::Ui, label: impl Into<String>, value: impl Into<String>) {
        ui.label(label.into());
        ui.label(egui::RichText::new(value.into()).monospace());
        ui.end_row();
    }
}

fn map_axis(value: u16, min: u16, max: u16) -> u16 {
    let extent = max.saturating_sub(min) as u64;
    min.saturating_add(((value as u64 * extent) / u16::MAX as u64) as u16)
}

fn button_state(left: bool, right: bool) -> String {
    match (left, right) {
        (false, false) => "None".to_string(),
        (true, false) => "Left".to_string(),
        (false, true) => "Right".to_string(),
        (true, true) => "Left + Right".to_string(),
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "Yes"
    }
    else {
        "No"
    }
}

#[cfg(test)]
mod tests {
    use super::map_axis;

    #[test]
    fn range_mapping_covers_the_full_absolute_range() {
        assert_eq!(map_axis(0, 10, 639), 10);
        assert_eq!(map_axis(u16::MAX, 10, 639), 639);
    }
}

use crate::input::get_mouse_buttons;

use marty_frontend_common::marty_common::types::ui::MouseCaptureMode;

#[allow(dead_code)]
pub struct MouseData {
    pub reverse_buttons: bool,
    pub l_button_id: u32,
    pub r_button_id: u32,
    pub is_captured: bool,
    pub capture_mode: MouseCaptureMode,
    pub have_update: bool,
    pub l_button_was_pressed: bool,
    pub l_button_was_released: bool,
    pub l_button_is_pressed: bool,
    pub r_button_was_pressed: bool,
    pub r_button_was_released: bool,
    pub r_button_is_pressed: bool,
    pub frame_delta_x: f32,
    pub frame_delta_y: f32,
}

impl MouseData {
    pub fn new(reverse_buttons: bool) -> Self {
        Self {
            reverse_buttons,
            l_button_id: get_mouse_buttons(reverse_buttons).0,
            r_button_id: get_mouse_buttons(reverse_buttons).1,
            is_captured: false,
            capture_mode: MouseCaptureMode::Mouse,
            have_update: false,
            l_button_was_pressed: false,
            l_button_was_released: false,
            l_button_is_pressed: false,
            r_button_was_pressed: false,
            r_button_was_released: false,
            r_button_is_pressed: false,
            frame_delta_x: 0.0,
            frame_delta_y: 0.0,
        }
    }
    pub fn reset(&mut self) {
        if !self.l_button_is_pressed {
            self.l_button_was_pressed = false;
        }
        if !self.r_button_is_pressed {
            self.r_button_was_pressed = false;
        }

        self.l_button_was_released = false;
        self.r_button_was_released = false;

        self.frame_delta_x = 0.0;
        self.frame_delta_y = 0.0;
        self.have_update = false;
    }
}

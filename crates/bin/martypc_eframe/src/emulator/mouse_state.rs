use crate::input::get_mouse_buttons;

use marty_frontend_common::marty_common::types::ui::MouseCaptureMode;

#[allow(dead_code)]
pub struct MouseState {
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
    pub pending_absolute_position: Option<(f32, f32)>,
    pub absolute_position: Option<(f32, f32)>,
}

impl MouseState {
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
            pending_absolute_position: None,
            absolute_position: None,
        }
    }
    pub fn reset(&mut self) {
        self.l_button_was_pressed = false;
        self.r_button_was_pressed = false;
        self.l_button_was_released = false;
        self.r_button_was_released = false;

        self.frame_delta_x = 0.0;
        self.frame_delta_y = 0.0;
        self.pending_absolute_position = None;
        self.have_update = false;
    }
}

#[derive(Copy, Clone)]
pub(crate) struct VirtualPointerInput {
    position: (f32, f32),
    left_pressed: bool,
    left_released: bool,
    left_down: bool,
    right_pressed: bool,
    right_released: bool,
    right_down: bool,
}

impl VirtualPointerInput {
    pub(crate) fn new(input: &egui::InputState, position: (f32, f32)) -> Self {
        Self {
            position,
            left_pressed: input.pointer.button_pressed(egui::PointerButton::Primary),
            left_released: input.pointer.button_released(egui::PointerButton::Primary),
            left_down: input.pointer.button_down(egui::PointerButton::Primary),
            right_pressed: input.pointer.button_pressed(egui::PointerButton::Secondary),
            right_released: input.pointer.button_released(egui::PointerButton::Secondary),
            right_down: input.pointer.button_down(egui::PointerButton::Secondary),
        }
    }

    pub(crate) fn apply(self, mouse: &mut MouseState) {
        mouse.pending_absolute_position = Some(self.position);
        mouse.absolute_position = Some(self.position);
        mouse.l_button_was_pressed |= self.left_pressed;
        mouse.r_button_was_pressed |= self.right_pressed;
        mouse.l_button_was_released |= self.left_released;
        mouse.r_button_was_released |= self.right_released;
        mouse.l_button_is_pressed = self.left_down;
        mouse.r_button_is_pressed = self.right_down;
        mouse.have_update = true;
    }
}

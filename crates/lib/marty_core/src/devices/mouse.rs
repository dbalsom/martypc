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

   devices::mouse.rs

   Implements a Microsoft Serial Mouse

*/
use crate::devices::{pic::Pic, serial::SerialPortController};

/// User-facing sensitivity is a multiplier around a calibrated nominal conversion.
const DEFAULT_MOUSE_SPEED: f32 = 1.0;
const SERIAL_COUNTS_PER_POINT: f64 = 0.375;
const VIRTUAL_COUNTS_PER_POINT: f64 = 48.0;

// Microseconds with RTS low before mouse considers itself reset
const MOUSE_RESET_TIME: f64 = 10_000.0;

// Mouse sends this byte when RTS is held low for MOUSE_RESET_TIME
// 0x4D = Ascii 'M' (For 'Microsoft' perhaps?)
const MOUSE_RESET_ACK_BYTE: u8 = 0x4D;

const MOUSE_UPDATE_STARTBIT: u8 = 0b0100_0000;
const MOUSE_UPDATE_LBUTTON: u8 = 0b0010_0000;
const MOUSE_UPDATE_RBUTTON: u8 = 0b0001_0000;
const MOUSE_UPDATE_HO_BITS: u8 = 0b1100_0000;
const MOUSE_UPDATE_LO_BITS: u8 = 0b0011_1111;

pub const VMOUSE_BUTTON_LEFT: u16 = 0b0000_0001;
pub const VMOUSE_BUTTON_RIGHT: u16 = 0b0000_0010;

#[derive(Copy, Clone, Debug, Default)]
pub struct MouseInput {
    pub delta_x: f64,
    pub delta_y: f64,
    pub absolute_x: Option<f64>,
    pub absolute_y: Option<f64>,
    pub left_button: bool,
    pub right_button: bool,
    pub left_pressed_since: bool,
    pub right_pressed_since: bool,
}

pub enum Mouse {
    Serial(SerialMouse),
    Virtual(VirtualMouse),
}

impl Mouse {
    pub fn new_serial(port: usize, speed: Option<f32>) -> Self {
        Self::Serial(SerialMouse::new(port, speed))
    }

    pub fn new_virtual(irq: u8, speed: Option<f32>) -> Self {
        Self::Virtual(VirtualMouse::new(irq, speed))
    }

    pub fn speed(&self) -> f32 {
        match self {
            Mouse::Serial(mouse) => mouse.speed(),
            Mouse::Virtual(mouse) => mouse.speed(),
        }
    }

    pub fn set_speed(&mut self, speed: f32) {
        match self {
            Mouse::Serial(mouse) => mouse.set_speed(speed),
            Mouse::Virtual(mouse) => mouse.set_speed(speed),
        }
    }

    pub fn submit_input(&mut self, input: MouseInput) {
        match self {
            Mouse::Serial(mouse) => mouse.submit_input(input),
            Mouse::Virtual(mouse) => mouse.submit_input(input),
        }
    }

    pub fn set_virtual_input_mode(&mut self, mode: VirtualMouseInputMode) {
        if let Mouse::Virtual(mouse) = self {
            mouse.set_input_mode(mode);
        }
    }

    pub fn take_virtual_state(&mut self) -> Option<VirtualMouseState> {
        match self {
            Mouse::Virtual(mouse) => Some(mouse.take_state()),
            Mouse::Serial(_) => None,
        }
    }

    pub fn virtual_irq(&self) -> Option<u8> {
        match self {
            Mouse::Virtual(mouse) => Some(mouse.irq()),
            Mouse::Serial(_) => None,
        }
    }

    pub fn set_virtual_consumer_range(&mut self, range: VirtualMouseConsumerRange) -> bool {
        match self {
            Mouse::Virtual(mouse) => {
                mouse.set_consumer_range(range);
                true
            }
            Mouse::Serial(_) => false,
        }
    }

    pub fn set_virtual_consumer_status(&mut self, loaded: bool) -> bool {
        match self {
            Mouse::Virtual(mouse) => {
                mouse.set_consumer_status(loaded);
                true
            }
            Mouse::Serial(_) => false,
        }
    }

    pub fn reset_virtual_consumer(&mut self) {
        if let Mouse::Virtual(mouse) = self {
            mouse.set_consumer_status(false);
        }
    }

    pub fn virtual_debug_state(&self) -> Option<VirtualMouseDebugState> {
        match self {
            Mouse::Virtual(mouse) => Some(mouse.debug_state()),
            Mouse::Serial(_) => None,
        }
    }
}

pub struct SerialMouse {
    speed: f32,
    pending_x: f64,
    pending_y: f64,
    left_button: bool,
    right_button: bool,
    reported_left_button: bool,
    reported_right_button: bool,
    left_press_pending: bool,
    right_press_pending: bool,
    rts: bool,
    rts_low_timer: f64,
    port: usize,
}

impl SerialMouse {
    pub fn new(port: usize, speed: Option<f32>) -> Self {
        Self {
            speed: speed.unwrap_or(DEFAULT_MOUSE_SPEED),
            pending_x: 0.0,
            pending_y: 0.0,
            left_button: false,
            right_button: false,
            reported_left_button: false,
            reported_right_button: false,
            left_press_pending: false,
            right_press_pending: false,
            rts: false,
            rts_low_timer: 0.0,
            port,
        }
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.max(0.01);
    }

    pub fn submit_input(&mut self, input: MouseInput) {
        self.pending_x += input.delta_x;
        self.pending_y += input.delta_y;

        self.left_press_pending |= input.left_pressed_since || (!self.left_button && input.left_button);
        self.right_press_pending |= input.right_pressed_since || (!self.right_button && input.right_button);
        self.left_button = input.left_button;
        self.right_button = input.right_button;
    }

    fn movement_scale(&self) -> f64 {
        SERIAL_COUNTS_PER_POINT * self.speed as f64
    }

    fn pending_packet(&self) -> Option<([u8; 3], i8, i8, bool, bool)> {
        let delta_x = (self.pending_x * self.movement_scale())
            .trunc()
            .clamp(i8::MIN as f64, i8::MAX as f64) as i8;
        let delta_y = (self.pending_y * self.movement_scale())
            .trunc()
            .clamp(i8::MIN as f64, i8::MAX as f64) as i8;
        let left_button = self.left_button || self.left_press_pending;
        let right_button = self.right_button || self.right_press_pending;

        let movement_pending = delta_x != 0 || delta_y != 0;
        let button_pending = left_button != self.reported_left_button || right_button != self.reported_right_button;
        if !movement_pending && !button_pending {
            return None;
        }

        let mut byte1 = MOUSE_UPDATE_STARTBIT;

        if left_button {
            byte1 |= MOUSE_UPDATE_LBUTTON;
        }
        if right_button {
            byte1 |= MOUSE_UPDATE_RBUTTON;
        }

        // Pack HO 2 bits of Y into byte1
        byte1 |= ((delta_y as u8) & MOUSE_UPDATE_HO_BITS) >> 4;
        // Pack HO 2 bits of X into byte1;
        byte1 |= ((delta_x as u8) & MOUSE_UPDATE_HO_BITS) >> 6;

        // LO 6 bits of X into byte 2
        let byte2 = (delta_x as u8) & MOUSE_UPDATE_LO_BITS;
        // LO 6 bits of Y into byte 3
        let byte3 = (delta_y as u8) & MOUSE_UPDATE_LO_BITS;

        Some(([byte1, byte2, byte3], delta_x, delta_y, left_button, right_button))
    }

    fn consume_packet(&mut self, delta_x: i8, delta_y: i8, left_button: bool, right_button: bool) {
        self.pending_x -= delta_x as f64 / self.movement_scale();
        self.pending_y -= delta_y as f64 / self.movement_scale();
        self.reported_left_button = left_button;
        self.reported_right_button = right_button;
        self.left_press_pending = false;
        self.right_press_pending = false;
    }

    fn clear_pending(&mut self) {
        self.pending_x = 0.0;
        self.pending_y = 0.0;
        self.left_press_pending = false;
        self.right_press_pending = false;
        self.reported_left_button = self.left_button;
        self.reported_right_button = self.right_button;
    }

    /// Run the mouse device for the specified number of microseconds
    pub fn run(&mut self, serial: &mut SerialPortController, us: f64) {
        // Check RTS line for mouse reset
        let rts = serial.get_rts(self.port);
        let mut reset = false;

        if self.rts && !rts {
            // RTS has gone low
            self.rts = false;
            self.rts_low_timer = 0.0;
        }
        else if !self.rts && !rts {
            // RTS remains low, count
            self.rts_low_timer += us;
        }
        else if rts && !self.rts {
            // RTS has gone high

            self.rts = true;

            if self.rts_low_timer > MOUSE_RESET_TIME {
                // Reset mouse
                self.rts_low_timer = 0.0;
                reset = true;
            }
        }

        if reset {
            self.clear_pending();
            log::trace!("Sending reset byte: {:02X}", MOUSE_RESET_ACK_BYTE);
            serial.queue_byte(self.port, MOUSE_RESET_ACK_BYTE);
            return;
        }

        if let Some((packet, delta_x, delta_y, left_button, right_button)) = self.pending_packet() {
            if serial.try_queue_rx_bytes(self.port, &packet) {
                self.consume_packet(delta_x, delta_y, left_button, right_button);
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum VirtualMouseInputMode {
    #[default]
    Absolute,
    Relative,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct VirtualMouseState {
    pub x: u16,
    pub y: u16,
    pub buttons: u16,
    pub change_counter: u16,
    pub relative_x: i16,
    pub relative_y: i16,
    pub input_mode: VirtualMouseInputMode,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct VirtualMouseConsumerRange {
    pub min_x: u16,
    pub max_x: u16,
    pub min_y: u16,
    pub max_y: u16,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct VirtualMouseDebugState {
    pub irq: u8,
    pub speed: f32,
    pub input_mode: VirtualMouseInputMode,
    pub x: u16,
    pub y: u16,
    pub buttons: u16,
    pub left_button: bool,
    pub right_button: bool,
    pub left_press_pending: bool,
    pub right_press_pending: bool,
    pub event_pending: bool,
    pub interrupt_asserted: bool,
    pub lower_interrupt: bool,
    pub change_counter: u16,
    pub consumer_driver_loaded: bool,
    pub consumer_range: Option<VirtualMouseConsumerRange>,
}

pub struct VirtualMouse {
    irq: u8,
    speed: f32,
    input_mode: VirtualMouseInputMode,
    x: f64,
    y: f64,
    pending_relative_x: f64,
    pending_relative_y: f64,
    left_button: bool,
    right_button: bool,
    left_press_pending: bool,
    right_press_pending: bool,
    event_pending: bool,
    interrupt_asserted: bool,
    lower_interrupt: bool,
    change_counter: u16,
    consumer_driver_loaded: bool,
    consumer_range: Option<VirtualMouseConsumerRange>,
}

impl VirtualMouse {
    pub fn new(irq: u8, speed: Option<f32>) -> Self {
        Self {
            irq,
            speed: speed.unwrap_or(DEFAULT_MOUSE_SPEED),
            input_mode: VirtualMouseInputMode::Absolute,
            x: (u16::MAX / 2) as f64,
            y: (u16::MAX / 2) as f64,
            pending_relative_x: 0.0,
            pending_relative_y: 0.0,
            left_button: false,
            right_button: false,
            left_press_pending: false,
            right_press_pending: false,
            event_pending: false,
            interrupt_asserted: false,
            lower_interrupt: false,
            change_counter: 0,
            consumer_driver_loaded: false,
            consumer_range: None,
        }
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }

    pub fn irq(&self) -> u8 {
        self.irq
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.max(0.01);
    }

    pub fn set_input_mode(&mut self, mode: VirtualMouseInputMode) {
        if self.input_mode == mode {
            return;
        }

        self.input_mode = mode;
        self.pending_relative_x = 0.0;
        self.pending_relative_y = 0.0;
        self.event_pending = true;
        self.change_counter = self.change_counter.wrapping_add(1);
    }

    pub fn set_consumer_range(&mut self, range: VirtualMouseConsumerRange) {
        self.consumer_range = Some(range);
    }

    pub fn set_consumer_status(&mut self, loaded: bool) {
        self.consumer_driver_loaded = loaded;
        if !loaded {
            self.consumer_range = None;
        }
    }

    /// Return device state for host-side inspection without acknowledging an event or IRQ.
    pub fn debug_state(&self) -> VirtualMouseDebugState {
        VirtualMouseDebugState {
            irq: self.irq,
            speed: self.speed,
            input_mode: self.input_mode,
            x: self.x.round() as u16,
            y: self.y.round() as u16,
            buttons: self.buttons_for_report(),
            left_button: self.left_button,
            right_button: self.right_button,
            left_press_pending: self.left_press_pending,
            right_press_pending: self.right_press_pending,
            event_pending: self.event_pending,
            interrupt_asserted: self.interrupt_asserted,
            lower_interrupt: self.lower_interrupt,
            change_counter: self.change_counter,
            consumer_driver_loaded: self.consumer_driver_loaded,
            consumer_range: self.consumer_range,
        }
    }

    pub fn submit_input(&mut self, input: MouseInput) {
        let old_x = self.x.round() as u16;
        let old_y = self.y.round() as u16;
        let old_left_button = self.left_button;
        let old_right_button = self.right_button;

        let relative_input = self.input_mode == VirtualMouseInputMode::Relative
            && (input.delta_x != 0.0 || input.delta_y != 0.0);

        if relative_input {
            self.pending_relative_x += input.delta_x;
            self.pending_relative_y += input.delta_y;
        }

        if let (Some(absolute_x), Some(absolute_y)) = (input.absolute_x, input.absolute_y) {
            self.x = absolute_x.clamp(0.0, 1.0) * u16::MAX as f64;
            self.y = absolute_y.clamp(0.0, 1.0) * u16::MAX as f64;
        }
        else {
            let scale = VIRTUAL_COUNTS_PER_POINT * self.speed as f64;
            self.x = (self.x + input.delta_x * scale).clamp(0.0, u16::MAX as f64);
            self.y = (self.y + input.delta_y * scale).clamp(0.0, u16::MAX as f64);
        }

        self.left_press_pending |= input.left_pressed_since || (!self.left_button && input.left_button);
        self.right_press_pending |= input.right_pressed_since || (!self.right_button && input.right_button);
        self.left_button = input.left_button;
        self.right_button = input.right_button;

        let movement_changed = old_x != self.x.round() as u16 || old_y != self.y.round() as u16;
        let buttons_changed = old_left_button != self.left_button
            || old_right_button != self.right_button
            || input.left_pressed_since
            || input.right_pressed_since;

        if movement_changed || relative_input || buttons_changed {
            self.event_pending = true;
            self.change_counter = self.change_counter.wrapping_add(1);
        }
    }

    pub fn run(&mut self, pic: &mut Pic) {
        if self.lower_interrupt {
            pic.clear_interrupt(self.irq);
            self.interrupt_asserted = false;
            self.lower_interrupt = false;
            return;
        }

        if self.event_pending && !self.interrupt_asserted {
            pic.request_interrupt(self.irq);
            self.interrupt_asserted = true;
        }
        else if !self.event_pending && self.interrupt_asserted {
            pic.clear_interrupt(self.irq);
            self.interrupt_asserted = false;
        }
    }

    fn buttons_for_report(&self) -> u16 {
        let mut buttons = 0;
        if self.left_button || self.left_press_pending {
            buttons |= VMOUSE_BUTTON_LEFT;
        }
        if self.right_button || self.right_press_pending {
            buttons |= VMOUSE_BUTTON_RIGHT;
        }
        buttons
    }

    /// Return one coherent state snapshot and acknowledge the event currently driving the virtual mouse IRQ.
    pub fn take_state(&mut self) -> VirtualMouseState {
        let movement_scale = SERIAL_COUNTS_PER_POINT * self.speed as f64;
        let relative_x = take_scaled_motion(&mut self.pending_relative_x, movement_scale);
        let relative_y = take_scaled_motion(&mut self.pending_relative_y, movement_scale);
        let state = VirtualMouseState {
            x: self.x.round() as u16,
            y: self.y.round() as u16,
            buttons: self.buttons_for_report(),
            change_counter: self.change_counter,
            relative_x,
            relative_y,
            input_mode: self.input_mode,
        };

        self.event_pending = false;
        self.left_press_pending = false;
        self.right_press_pending = false;

        let reported_left = state.buttons & VMOUSE_BUTTON_LEFT != 0;
        let reported_right = state.buttons & VMOUSE_BUTTON_RIGHT != 0;
        if reported_left != self.left_button || reported_right != self.right_button {
            self.event_pending = true;
            self.change_counter = self.change_counter.wrapping_add(1);
        }

        if self.interrupt_asserted {
            self.lower_interrupt = true;
        }

        state
    }
}

fn take_scaled_motion(pending: &mut f64, scale: f64) -> i16 {
    let counts = (*pending * scale)
        .trunc()
        .clamp(i16::MIN as f64, i16::MAX as f64) as i16;
    *pending -= counts as f64 / scale;
    counts
}

#[cfg(test)]
mod tests {
    use super::{
        MouseInput,
        SerialMouse,
        VirtualMouse,
        VirtualMouseConsumerRange,
        VirtualMouseInputMode,
        VMOUSE_BUTTON_LEFT,
    };
    use crate::devices::pic::Pic;

    #[test]
    fn serial_mouse_combines_small_movements() {
        let mut mouse = SerialMouse::new(0, Some(1.0));
        mouse.submit_input(MouseInput {
            delta_x: 1.0,
            ..MouseInput::default()
        });
        assert!(mouse.pending_packet().is_none());

        mouse.submit_input(MouseInput {
            delta_x: 1.0,
            ..MouseInput::default()
        });
        assert!(mouse.pending_packet().is_none());

        mouse.submit_input(MouseInput {
            delta_x: 1.0,
            ..MouseInput::default()
        });
        let (_, delta_x, delta_y, _, _) = mouse.pending_packet().unwrap();
        assert_eq!(delta_x, 1);
        assert_eq!(delta_y, 0);
    }

    #[test]
    fn serial_mouse_carries_large_movements_across_packets() {
        let mut mouse = SerialMouse::new(0, Some(2.0));
        mouse.submit_input(MouseInput {
            delta_x: 600.0,
            ..MouseInput::default()
        });

        let (_, delta_x, delta_y, left, right) = mouse.pending_packet().unwrap();
        assert_eq!(delta_x, 127);
        mouse.consume_packet(delta_x, delta_y, left, right);

        let (_, delta_x, _, _, _) = mouse.pending_packet().unwrap();
        assert_eq!(delta_x, 127);
        assert!(mouse.pending_x > 0.0);
    }

    #[test]
    fn serial_mouse_does_not_lose_a_quick_click() {
        let mut mouse = SerialMouse::new(0, Some(1.0));
        mouse.submit_input(MouseInput {
            left_button: false,
            left_pressed_since: true,
            ..MouseInput::default()
        });

        let (_, delta_x, delta_y, left, right) = mouse.pending_packet().unwrap();
        assert!(left);
        mouse.consume_packet(delta_x, delta_y, left, right);

        let (_, _, _, left, _) = mouse.pending_packet().unwrap();
        assert!(!left);
    }

    #[test]
    fn virtual_mouse_reports_position_and_change_count() {
        let mut mouse = VirtualMouse::new(5, Some(0.5));
        mouse.submit_input(MouseInput {
            absolute_x: Some(0.25),
            absolute_y: Some(0.75),
            ..MouseInput::default()
        });

        let state = mouse.take_state();
        assert_eq!(state.x, (0.25 * u16::MAX as f64).round() as u16);
        assert_eq!(state.y, (0.75 * u16::MAX as f64).round() as u16);
        assert_eq!(state.change_counter, 1);
        assert_eq!(state.relative_x, 0);
        assert_eq!(state.relative_y, 0);
        assert_eq!(state.input_mode, VirtualMouseInputMode::Absolute);
        assert!(!mouse.event_pending);
    }

    #[test]
    fn virtual_mouse_reports_input_mode_changes() {
        let mut mouse = VirtualMouse::new(5, None);

        mouse.set_input_mode(VirtualMouseInputMode::Relative);

        assert!(mouse.event_pending);
        assert_eq!(mouse.change_counter, 1);
        assert_eq!(mouse.debug_state().input_mode, VirtualMouseInputMode::Relative);

        let state = mouse.take_state();
        assert_eq!(state.input_mode, VirtualMouseInputMode::Relative);
        assert_eq!(state.relative_x, 0);
        assert_eq!(state.relative_y, 0);
    }

    #[test]
    fn virtual_mouse_can_move_past_the_screen_edge() {
        let mut mouse = VirtualMouse::new(5, Some(1.0));
        mouse.submit_input(MouseInput {
            absolute_x: Some(1.0),
            absolute_y: Some(0.5),
            ..MouseInput::default()
        });
        mouse.take_state();

        mouse.set_input_mode(VirtualMouseInputMode::Relative);
        mouse.take_state();
        mouse.submit_input(MouseInput {
            delta_x: 8.0,
            ..MouseInput::default()
        });

        let state = mouse.take_state();
        assert_eq!(state.x, u16::MAX);
        assert_eq!(state.relative_x, 3);
        assert_eq!(state.relative_y, 0);

        let drained = mouse.take_state();
        assert_eq!(drained.relative_x, 0);
    }

    #[test]
    fn virtual_mouse_keeps_irq_active_until_state_is_read() {
        let mut mouse = VirtualMouse::new(5, None);
        let mut pic = Pic::new();
        mouse.submit_input(MouseInput {
            delta_x: 1.0,
            ..MouseInput::default()
        });

        mouse.run(&mut pic);
        assert!(mouse.interrupt_asserted);

        mouse.take_state();
        mouse.run(&mut pic);
        assert!(!mouse.interrupt_asserted);
        assert!(!mouse.event_pending);
    }

    #[test]
    fn virtual_mouse_reports_both_halves_of_a_quick_click() {
        let mut mouse = VirtualMouse::new(5, None);
        mouse.submit_input(MouseInput {
            left_pressed_since: true,
            ..MouseInput::default()
        });

        let pressed = mouse.take_state();
        assert_ne!(pressed.buttons & VMOUSE_BUTTON_LEFT, 0);
        assert_eq!(pressed.change_counter, 1);
        assert!(mouse.event_pending);

        let released = mouse.take_state();
        assert_eq!(released.buttons & VMOUSE_BUTTON_LEFT, 0);
        assert_eq!(released.change_counter, 2);
    }

    #[test]
    fn virtual_mouse_counts_each_update() {
        let mut mouse = VirtualMouse::new(5, None);
        mouse.submit_input(MouseInput {
            delta_x: 1.0,
            ..MouseInput::default()
        });
        mouse.submit_input(MouseInput {
            delta_y: 1.0,
            ..MouseInput::default()
        });

        let state = mouse.take_state();
        assert_eq!(state.change_counter, 2);
    }

    #[test]
    fn inspecting_virtual_mouse_does_not_clear_pending_events() {
        let mut mouse = VirtualMouse::new(5, None);
        let range = VirtualMouseConsumerRange {
            min_x: 0,
            max_x: 639,
            min_y: 0,
            max_y: 199,
        };
        mouse.set_consumer_status(true);
        mouse.set_consumer_range(range);
        mouse.submit_input(MouseInput {
            absolute_x: Some(0.25),
            absolute_y: Some(0.75),
            left_button: true,
            ..MouseInput::default()
        });

        let debug_state = mouse.debug_state();
        assert!(debug_state.consumer_driver_loaded);
        assert_eq!(debug_state.consumer_range, Some(range));
        assert_ne!(debug_state.buttons & VMOUSE_BUTTON_LEFT, 0);
        assert!(debug_state.event_pending);
        assert!(mouse.event_pending);

        mouse.set_consumer_status(false);
        let debug_state = mouse.debug_state();
        assert!(!debug_state.consumer_driver_loaded);
        assert_eq!(debug_state.consumer_range, None);
    }
}

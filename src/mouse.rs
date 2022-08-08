/*

    mouse.rs 
    Emulate a Microsoft Serial Mouse
 
 */
use std::{
    cell::RefCell,
    rc::Rc,
};

use crate::serial::SerialPortController;

const MOUSE_SCALE: f64 = 0.25;

// Mouse port is always attached to COM1
const MOUSE_PORT: usize = 0;

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

pub struct Mouse {

    serial_ctrl: Rc<RefCell<SerialPortController>>,
    rts: bool,
    rts_low_timer: f64,
    dtr: bool,
}

impl Mouse {
    pub fn new(serial_ctrl: Rc<RefCell<SerialPortController>>) -> Self {
        Self {
            serial_ctrl,
            rts: false,
            rts_low_timer: 0.0,
            dtr: false,
        }
    }

    pub fn update(&self, l_button_pressed: bool, r_button_pressed: bool, delta_x: f64, delta_y: f64) {

        let mut scaled_x = (delta_x * MOUSE_SCALE);
        let mut scaled_y = (delta_y * MOUSE_SCALE);
        // Minimum movement of one unit
        if scaled_x > 0.0 && scaled_x < 1.0 {
            scaled_x = 1.0;
        }
        if scaled_y > 0.0 && scaled_y < 1.0 {
            scaled_y = 1.0;
        }
        let delta_x_i8 = scaled_x as i8;
        let delta_y_i8 = scaled_y as i8;

        let mut byte1 = MOUSE_UPDATE_STARTBIT;

        if l_button_pressed {
            log::trace!("Sending mouse button down");
            byte1 |= MOUSE_UPDATE_LBUTTON;
        }
        if r_button_pressed {
            byte1 |= MOUSE_UPDATE_RBUTTON;
        }

        // Pack HO 2 bits of Y into byte1
        byte1 |= ((delta_y_i8 as u8) & MOUSE_UPDATE_HO_BITS) >> 4;
        // Pack HO 2 bits of X into byte1;
        byte1 |= ((delta_x_i8 as u8) & MOUSE_UPDATE_HO_BITS) >> 6;

        // LO 6 bits of X into byte 2
        let byte2 = (delta_x_i8 as u8) & MOUSE_UPDATE_LO_BITS;
        // LO 6 bits of Y into byte 3
        let byte3 = (delta_y_i8 as u8) & MOUSE_UPDATE_LO_BITS;

        // Send update
        let mut serial = self.serial_ctrl.borrow_mut();
        serial.queue_byte(MOUSE_PORT, byte1);
        serial.queue_byte(MOUSE_PORT, byte2);
        serial.queue_byte(MOUSE_PORT, byte3);

     }

    /// Run the mouse device for the specified number of microseconds
    pub fn run(&mut self, us: f64) {

        // Check RTS line for mouse reset

        let mut serial = self.serial_ctrl.borrow_mut();
        let rts = serial.get_rts(MOUSE_PORT);

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
                // Send reset ack byte
                log::trace!("Sending reset byte: {:02X}", MOUSE_RESET_ACK_BYTE );
                serial.queue_byte(MOUSE_PORT, MOUSE_RESET_ACK_BYTE);
            }
        }
    }
}


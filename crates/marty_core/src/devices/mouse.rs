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

   devices::mouse.rs

   Implements a Microsoft Serial Mouse

*/
use std::collections::VecDeque;

use crate::devices::serial::SerialPortController;

// Scale factor for real vs emulated mouse deltas. Need to play with
// this value until it feels right.
const DEFAULT_MOUSE_SPEED: f32 = 0.25;
const DEFAULT_POLL_RATE: f32 = 40.0; // 40 Hz

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

use web_time::{Duration, Instant};

#[allow(dead_code)]
pub struct Mouse {
    speed: f32,
    updates: VecDeque<MouseUpdate>,
    rts: bool,
    rts_low_timer: f64,
    dtr: bool,
    port: usize,
    last_update: Instant,
    poll_rate: f32,
}

pub enum MouseUpdate {
    Update(u8, u8, u8),
}

impl Mouse {
    pub fn new(port: usize, speed: Option<f32>, poll_rate: Option<f32>) -> Self {
        Self {
            speed: speed.unwrap_or(DEFAULT_MOUSE_SPEED),
            updates: VecDeque::new(),
            rts: false,
            rts_low_timer: 0.0,
            dtr: false,
            port,
            last_update: Instant::now(),
            poll_rate: poll_rate.unwrap_or(DEFAULT_POLL_RATE),
        }
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    pub fn update(&mut self, l_button_pressed: bool, r_button_pressed: bool, delta_x: f32, delta_y: f32) {
        let mut scaled_x = delta_x * self.speed();
        let mut scaled_y = delta_y * self.speed();

        // Mouse scale can cause fractional integer updates. Adjust to Minimum movement of one unit
        if scaled_x > 0.0 && scaled_x < 1.0 {
            scaled_x = 1.0;
        }
        if scaled_x < 0.0 && scaled_x > -1.0 {
            scaled_x = -1.0;
        }
        if scaled_y > 0.0 && scaled_y < 1.0 {
            scaled_y = 1.0;
        }
        if scaled_y < 0.0 && scaled_y > -1.0 {
            scaled_y = -1.0;
        }
        let delta_x_i8 = scaled_x as i8;
        let delta_y_i8 = scaled_y as i8;

        let mut byte1 = MOUSE_UPDATE_STARTBIT;

        if l_button_pressed {
            //log::debug!("Sending mouse button down");
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

        // Queue update
        self.updates.push_back(MouseUpdate::Update(byte1, byte2, byte3));
    }

    /// Run the mouse device for the specified number of microseconds
    pub fn run(&mut self, serial: &mut SerialPortController, us: f64) {
        // Send a queued update.
        if let Some(MouseUpdate::Update(byte1, byte2, byte3)) = self.updates.pop_front() {
            serial.queue_byte(self.port, byte1);
            serial.queue_byte(self.port, byte2);
            serial.queue_byte(self.port, byte3);
        }

        // Check RTS line for mouse reset
        let rts = serial.get_rts(self.port);

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
                log::trace!("Sending reset byte: {:02X}", MOUSE_RESET_ACK_BYTE);
                serial.queue_byte(self.port, MOUSE_RESET_ACK_BYTE);
            }
        }
    }
}

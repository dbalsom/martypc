/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2024 Daniel Balsom

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

    devices::game_port.rs

    Implementation of the IBM Game Port.

    The game port uses some an ... interesting method to read the state of an
    analog joystick axis using one input pin.

    Each axis is actually connected to a 100KOhm potentiometer, with a mostly
    linear response from 0-100KOhms.

    A stick 'position' is in the range from -1.0 to 1.0. This is mapped to a
    resistance value from 0 to 100KOhms.

    The resistance of each potentiometer affects how fast a capacitor charges.
    The time it takes to charge the capacitor is measured by the game port and
    used to determine the stick position.  This of course, requires frequent
    and costly polling of the game port to determine the stick position.


*/

use crate::{
    bus::{BusInterface, DeviceRunTimeUnit, IoDevice, NO_IO_BYTE},
    cpu_common::LogicAnalyzer,
};

pub const GAMEPORT_DEFAULT_PORT: u16 = 0x201;
pub const GAMEPORT_DEFAULT_MASK: u16 = 0xFFFF;

pub const BUTTON1: u8 = 0b0001_0000;
pub const BUTTON2: u8 = 0b0010_0000;
pub const BUTTON3: u8 = 0b0100_0000;
pub const BUTTON4: u8 = 0b1000_0000;
pub const BUTTON_BITS: [u8; 4] = [BUTTON1, BUTTON2, BUTTON3, BUTTON4];

pub const STICK1_X: u8 = 0b0000_0001;
pub const STICK1_Y: u8 = 0b0000_0010;
pub const STICK2_X: u8 = 0b0000_0100;
pub const STICK2_Y: u8 = 0b0000_1000;

pub const POT_OHMS: f64 = 100_000.0;

// IBM provides the following formula for timing the charge of the capacitor:
// Time = 24.2us + (0.011 * R)us
pub const BASE_CHARGE_TIME_US: f64 = 25.2;
pub const CHARGE_FACTOR: f64 = 0.011;

#[derive(Default)]
pub enum ControllerLayout {
    #[default]
    TwoJoysticksTwoButtons,
    OneJoystickFourButtons,
}

#[derive(Default)]
pub struct Axis {
    pos:    f64,
    time:   f64,
    timing: bool,
}

#[derive(Default)]
pub struct Stick {
    x: Axis,
    y: Axis,
}

#[derive(Default)]
pub struct GamePortState {
    pub buttons: [bool; 4],
    pub sticks: [(f64, f64); 2],
    pub resistance: [(f64, f64); 2],
}

#[derive(Default)]
pub struct GamePort {
    port_base: u16,
    layout:    ControllerLayout,
    sticks:    [Stick; 2],
    buttons:   [bool; 4],
}

impl GamePort {
    pub fn new(port_base: Option<u16>) -> Self {
        GamePort {
            port_base: port_base.unwrap_or(GAMEPORT_DEFAULT_PORT),
            ..Default::default()
        }
    }

    pub fn set_button(&mut self, controller: usize, button: usize, state: bool) {
        match self.layout {
            ControllerLayout::TwoJoysticksTwoButtons => {
                if button < 2 && controller < 2 {
                    self.buttons[button + (controller * 2)] = state;
                }
            }
            ControllerLayout::OneJoystickFourButtons => {
                if controller == 0 && button < 4 {
                    self.buttons[button] = state;
                }
            }
        }
    }

    pub fn set_stick_pos(&mut self, controller: usize, stick: usize, x: Option<f64>, y: Option<f64>) {
        match self.layout {
            ControllerLayout::TwoJoysticksTwoButtons => {
                if controller < 2 && stick == 0 {
                    if let Some(x) = x {
                        self.sticks[controller].x.pos = x;
                    }
                    if let Some(y) = y {
                        self.sticks[controller].y.pos = y;
                    }
                }
            }
            ControllerLayout::OneJoystickFourButtons => {
                if controller == 0 && stick == 0 {
                    if let Some(x) = x {
                        self.sticks[controller].x.pos = x;
                    }
                    if let Some(y) = y {
                        self.sticks[controller].y.pos = y;
                    }
                }
            }
        }
    }

    pub fn get_controller_count(&self) -> usize {
        match self.layout {
            ControllerLayout::TwoJoysticksTwoButtons => 2,
            ControllerLayout::OneJoystickFourButtons => 1,
        }
    }

    pub fn reset_oneshots(&mut self) {
        for sticks in self.sticks.iter_mut() {
            sticks.x.timing = true;
            sticks.y.timing = true;
            sticks.x.time = 0.0;
            sticks.y.time = 0.0;
        }
    }

    pub fn port_read(&self) -> u8 {
        let mut data = 0u8;

        for (i, button) in self.buttons.iter().enumerate() {
            if *button {
                data |= BUTTON_BITS[i];
            }
        }

        if !self.sticks[0].x.timing {
            data |= STICK1_X;
        }
        if !self.sticks[0].y.timing {
            data |= STICK1_Y;
        }
        if !self.sticks[1].x.timing {
            data |= STICK2_X;
        }
        if !self.sticks[1].y.timing {
            data |= STICK2_Y;
        }

        // It's easier to think internally of buttons being 1 when set and sticks being 1 when not timing.
        // But the game port reverses this logic, so return the inverse of what we just calculated.
        !data
    }

    pub fn get_state(&self) -> GamePortState {
        GamePortState {
            buttons: self.buttons,
            sticks: [
                (self.sticks[0].x.pos, self.sticks[0].y.pos),
                (self.sticks[1].x.pos, self.sticks[1].y.pos),
            ],
            resistance: [
                (pos_to_ohms(self.sticks[0].x.pos), pos_to_ohms(self.sticks[0].y.pos)),
                (pos_to_ohms(self.sticks[1].x.pos), pos_to_ohms(self.sticks[1].y.pos)),
            ],
        }
    }

    pub fn run(&mut self, us: f64) {
        for sticks in self.sticks.iter_mut() {
            time_axis(&mut sticks.x, us);
            time_axis(&mut sticks.y, us);
        }
    }
}

#[inline]
pub fn pos_to_ohms(pos: f64) -> f64 {
    // The stick position is in the range -1.0 to 1.0.
    // This is mapped to a resistance value from 0 to 100KOhms.
    ((pos + 1.0) / 2.0) * POT_OHMS
}

pub fn time_axis(axis: &mut Axis, us: f64) {
    if axis.timing {
        axis.time += us;

        let charge_time = BASE_CHARGE_TIME_US + (CHARGE_FACTOR * pos_to_ohms(axis.pos));
        if axis.time >= charge_time {
            // Stop timing, the stick can now be read.
            axis.timing = false;
        }
    }
}

impl IoDevice for GamePort {
    fn read_u8(&mut self, port: u16, _delta: DeviceRunTimeUnit) -> u8 {
        if (port & GAMEPORT_DEFAULT_MASK) == self.port_base {
            self.port_read()
        }
        else {
            NO_IO_BYTE
        }
    }

    fn write_u8(
        &mut self,
        _port: u16,
        _data: u8,
        _bus: Option<&mut BusInterface>,
        _delta: DeviceRunTimeUnit,
        _analyzer: Option<&mut LogicAnalyzer>,
    ) {
        // Writing to the game port resets the one-shot counters.
        self.reset_oneshots();
    }

    fn port_list(&self) -> Vec<(String, u16)> {
        vec![("Game Port".to_string(), self.port_base)]
    }
}

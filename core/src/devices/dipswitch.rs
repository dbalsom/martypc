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

    devices::dipswitch.rs

    Implement a simple dipswitch device.
    Handles common dipswitch issues such as converting from physical to logical
    state, masking based on switch size, and retrieving individual switch states.

*/

#[derive(Copy, Clone, Debug)]
pub enum DipSwitchSize {
    Dip4,
    Dip8,
}

#[derive(Copy, Clone, Debug)]
pub struct DipSwitch {
    size: DipSwitchSize,
    physical_state: u8,
    invert_bits: bool, // Whether to invert logical bits vs physical state of switches. Defaults to true to the design of most DIP switches.
    reverse_bits: bool, // Whether to reverse bits. Defaults to false. May be used in some contexts depending on how DIP is wired.
    and_mask: u8,       // Mask to emulate switches that always read OFF.
    or_mask: u8,        // Mask to emulate switches that always read ON.
}

impl Default for DipSwitch {
    fn default() -> Self {
        Self {
            size: DipSwitchSize::Dip8,
            physical_state: 0,
            invert_bits: true,
            reverse_bits: false,
            and_mask: 0xFF,
            or_mask: 0x00,
        }
    }
}

impl DipSwitch {
    pub fn new(size: DipSwitchSize, state: u8) -> Self {
        let mut new_switch = DipSwitch {
            size,
            ..Default::default()
        };
        new_switch.set_physical_state(state);
        new_switch
    }

    pub fn with_reverse_bits(mut self, state: bool) -> Self {
        self.reverse_bits = state;
        self
    }

    pub fn with_invert_bits(mut self, state: bool) -> Self {
        self.invert_bits = state;
        self
    }

    pub fn with_and_mask(mut self, mask: u8) -> Self {
        match self.size {
            DipSwitchSize::Dip4 => {
                self.and_mask = mask & 0b0000_1111;
            }
            DipSwitchSize::Dip8 => {
                self.and_mask = mask;
            }
        }
        self
    }

    pub fn with_or_mask(mut self, mask: u8) -> Self {
        match self.size {
            DipSwitchSize::Dip4 => {
                self.or_mask = mask & 0b0000_1111;
            }
            DipSwitchSize::Dip8 => {
                self.or_mask = mask;
            }
        }
        self
    }

    /// Set the physical state of the DIP switches (0 is OFF, 1 is ON)
    /// LSB is switch 0, MSB is switch 7.
    pub fn set_physical_state(&mut self, state: u8) {
        match self.size {
            DipSwitchSize::Dip4 => {
                self.physical_state = state & 0b0000_1111;
            }
            DipSwitchSize::Dip8 => self.physical_state = state,
        }
    }

    pub fn get_physical_state(&self) -> u8 {
        self.physical_state
    }

    /// Simulate reading the dipswitch electrical state.
    /// This will apply the 'and_mask' and 'or_mask' to the physical state, and then apply the
    /// reverse_bits and invert_bits settings.
    pub fn read(&self) -> u8 {
        let mut logical_state = self.physical_state;
        if self.reverse_bits {
            match self.size {
                DipSwitchSize::Dip4 => {
                    logical_state = logical_state.reverse_bits() >> 4;
                }
                DipSwitchSize::Dip8 => {
                    logical_state = logical_state.reverse_bits();
                }
            }
        }
        if self.invert_bits {
            logical_state = !logical_state;
        }
        logical_state & self.and_mask | self.or_mask
    }

    /// Return the physical state of a switch number. This differs from the electrical state.
    pub fn is_on(&self, switch: u8) -> bool {
        (self.physical_state & (1 << switch)) != 0
    }
}

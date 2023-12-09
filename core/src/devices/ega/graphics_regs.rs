/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

    ---------------------------------------------------------------------------

    ega::graphics_regs.rs

    Implement the EGA Graphics registers.

*/

use crate::devices::ega::EGACard;
use modular_bitfield::prelude::*;

#[derive(Copy, Clone, Debug)]
pub enum GraphicsRegister {
    SetReset,
    EnableSetReset,
    ColorCompare,
    DataRotate,
    ReadMapSelect,
    Mode,
    Miscellaneous,
    ColorDontCare,
    BitMask,
}

#[bitfield]
pub struct GDataRotateRegister {
    pub count: B3,
    #[bits = 2]
    pub function: RotateFunction,
    #[skip]
    unused: B3,
}

#[bitfield]
pub struct GModeRegister {
    #[bits = 2]
    pub write_mode: WriteMode,
    pub test_condition: bool,
    #[bits = 1]
    pub read_mode: ReadMode,
    pub odd_even: bool,
    #[bits = 1]
    pub shift_mode: ShiftMode,
    #[skip]
    unused: B2,
}

#[bitfield]
pub struct GMiscellaneousRegister {
    pub graphics_mode: bool,
    pub chain_odd_even: bool,
    pub memory_map: MemoryMap,
    #[skip]
    pub unused: B4,
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum MemoryMap {
    A0000_128k,
    A0000_64K,
    B0000_32K,
    B8000_32K,
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum RotateFunction {
    Unmodified,
    And,
    Or,
    Xor,
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum WriteMode {
    Mode0,
    Mode1,
    Mode2,
    Invalid,
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum ReadMode {
    ReadSelectedPlane,
    ReadComparedPlanes,
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum ShiftMode {
    Standard,
    CGACompatible,
}

impl EGACard {
    /// Handle a write to one of the Graphics Position Registers.
    ///
    /// According to IBM documentation, both these registers should be set to
    /// specific values, so we don't really do anything with them other than
    /// log if we see an unexpected value written.
    pub fn write_graphics_position(&mut self, reg: u32, byte: u8) {
        match reg {
            1 => {
                if byte != 0 {
                    log::warn!("Non-zero value written to Graphics 1 Position register.")
                }
            }
            2 => {
                if byte != 1 {
                    log::warn!("Non-1 value written to Graphics 2 Position register.")
                }
            }
            _ => {}
        }
    }

    /// Handle a write to the Graphics Address Register
    pub fn write_graphics_address(&mut self, byte: u8) {
        self.graphics_register_select_byte = byte & 0x0F;

        self.graphics_register_selected = match self.graphics_register_select_byte {
            0x00 => GraphicsRegister::SetReset,
            0x01 => GraphicsRegister::EnableSetReset,
            0x02 => GraphicsRegister::ColorCompare,
            0x03 => GraphicsRegister::DataRotate,
            0x04 => GraphicsRegister::ReadMapSelect,
            0x05 => GraphicsRegister::Mode,
            0x06 => GraphicsRegister::Miscellaneous,
            0x07 => GraphicsRegister::ColorDontCare,
            0x08 => GraphicsRegister::BitMask,
            _ => self.graphics_register_selected,
        }
    }

    pub fn write_graphics_data(&mut self, byte: u8) {
        match self.graphics_register_selected {
            GraphicsRegister::SetReset => {
                // Bits 0-3: Set/Reset Bits 0-3
                self.graphics_set_reset = byte & 0x0F;
            }
            GraphicsRegister::EnableSetReset => {
                // Bits 0-3: Enable Set/Reset Bits 0-3
                self.graphics_enable_set_reset = byte & 0x0F;
            }
            GraphicsRegister::ColorCompare => {
                // Bits 0-3: Color Compare 0-3
                self.graphics_color_compare = byte & 0x0F;
            }
            GraphicsRegister::DataRotate => {
                // Bits 0-2: Rotate Count
                // Bits 3-4: Function Select
                self.graphics_data_rotate = GDataRotateRegister::from_bytes([byte]);
            }
            GraphicsRegister::ReadMapSelect => {
                // Bits 0-2: Map Select 0-2
                self.graphics_read_map_select = byte & 0x07;
            }

            GraphicsRegister::Mode => {
                // Bits 0-1: Write Mode
                // Bit 2: Test Condition
                // Bit 3: Read Mode
                // Bit 4: Odd/Even
                // Bit 5: Shift Register Mode
                self.graphics_mode = GModeRegister::from_bytes([byte]);
            }
            GraphicsRegister::Miscellaneous => {
                self.graphics_micellaneous = GMiscellaneousRegister::from_bytes([byte]);
            }
            GraphicsRegister::ColorDontCare => {
                // Bits 0-3: Color Don't Care
                self.graphics_color_dont_care = byte & 0x0F;
            }
            GraphicsRegister::BitMask => {
                // Bits 0-7: Bit Mask
                self.graphics_bitmask = byte;
            }
        }
    }
}

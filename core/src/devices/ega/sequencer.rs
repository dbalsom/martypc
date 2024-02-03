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

    ---------------------------------------------------------------------------

    ega::sequencer_regs.rs

    Implement the EGA Sequencer

*/

use crate::devices::ega::vram::Vram;
use modular_bitfield::{
    bitfield,
    prelude::{B1, B4, B5},
    BitfieldSpecifier,
};

#[derive(Copy, Clone, Debug)]
pub enum SequencerRegister {
    Reset,
    ClockingMode,
    MapMask,
    CharacterMapSelect,
    MemoryMode,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct SClockingModeRegister {
    #[bits = 1]
    pub character_clock: CharacterClock,
    pub bandwidth: B1,
    pub shift_load: B1,
    pub dot_clock: DotClock,
    #[skip]
    unused: B4,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct SMemoryModeRegister {
    #[bits = 1]
    pub alpha_mode: bool,
    pub extended_memory: B1,
    pub odd_even: B1,
    #[skip]
    unused: B5,
}

// Ferraro has this bit flipped. 0 == 9 Dots. IBM docs are correct.
#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum CharacterClock {
    NineDots,
    EightDots,
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum DotClock {
    Native,
    HalfClock,
}

pub struct Sequencer {
    pub address_byte: u8,
    pub register_selected: SequencerRegister,
    pub reset: u8,                            // S(0) Reset (WO)
    pub clocking_mode: SClockingModeRegister, // S(1) Clocking Mode (WO)
    pub map_mask: u8,                         // S(2) Map Mask (wO)
    pub character_map_select: u8,             // S(3) Character Map Select (WO)
    pub memory_mode: SMemoryModeRegister,     // S(4) Memory Mode (wO)

    pub clock_change_pending: bool,
    pub clock_divisor: u32,
    pub char_clock: u32,

    pub vram: Vram,
}

impl Default for Sequencer {
    fn default() -> Self {
        Self {
            address_byte: 0,
            register_selected: SequencerRegister::Reset,
            reset: 0,
            clocking_mode: SClockingModeRegister::new(),
            map_mask: 0,
            character_map_select: 0,
            memory_mode: SMemoryModeRegister::new(),
            clock_change_pending: false,
            clock_divisor: 1,
            char_clock: 8,

            vram: Vram::new(),
        }
    }
}

impl Sequencer {
    pub fn new() -> Self {
        Sequencer::default()
    }

    pub fn reset(&mut self) {
        *self = Sequencer::default();
    }

    /// Handle a write to the Sequencer Address register.
    ///
    /// The value written to this register controls which regsiter will be written to
    /// when a byte is sent to the Sequencer Data register.
    pub fn write_address(&mut self, byte: u8) {
        //log::trace!("CGA: CRTC register {:02X} selected", byte);
        self.address_byte = byte & 0x1F;

        self.register_selected = match self.address_byte {
            0x00 => SequencerRegister::Reset,
            0x01 => SequencerRegister::ClockingMode,
            0x02 => SequencerRegister::MapMask,
            0x03 => SequencerRegister::CharacterMapSelect,
            0x04 => SequencerRegister::MemoryMode,
            _ => {
                log::debug!("Select to invalid sequencer register: {:02X}", byte);
                self.register_selected
            }
        }
    }

    /// Handle a write to the Sequencer Data register.
    ///
    /// Will write to the internal register selected by the Sequencer Address Register.
    pub fn write_data(&mut self, byte: u8) {
        match self.register_selected {
            SequencerRegister::Reset => {
                self.reset = byte & 0x03;
                log::trace!("Write to Sequencer::Reset register: {:02X}", byte);
            }
            SequencerRegister::ClockingMode => {
                self.clocking_mode = SClockingModeRegister::from_bytes([byte]);
                log::trace!("Write to Sequencer::ClockingMode register: {:02X}", byte);

                self.clock_change_pending = true;
                (self.clock_divisor, self.char_clock) = match self.clocking_mode.dot_clock() {
                    DotClock::HalfClock => (2, 16),
                    DotClock::Native => (1, 8),
                }
            }
            SequencerRegister::MapMask => {
                self.map_mask = byte & 0x0F;
                // Warning: noisy
                //log::trace!("Write to Sequencer::MapMask register: {:02X}", byte);
            }
            SequencerRegister::CharacterMapSelect => {
                self.character_map_select = byte & 0x0F;
                log::trace!("Write to Sequencer::CharacterMapSelect register: {:02X}", byte);
            }
            SequencerRegister::MemoryMode => {
                self.memory_mode = SMemoryModeRegister::from_bytes([byte & 0x07]);
                log::trace!("Write to Sequencer::MemoryMode register: {:02X}", byte);
            }
        }
    }

    #[inline]
    pub fn read_u8(&self, plane: usize, addr: usize, a0: usize) -> u8 {
        // Handle odd/even addressing
        let vram_plane = match self.memory_mode.odd_even() {
            0 => plane,
            _ => plane & !0x01 | a0, // Force plane to even/odd
        };
        self.vram.read_u8(vram_plane, addr)
    }

    pub fn write_u8(&mut self, plane: usize, addr: usize, a0: usize, data: u8) {
        // Handle odd/even addressing
        let vram_plane = match self.memory_mode.odd_even() {
            0 => plane,
            _ => plane & !0x01 | a0,
        };
        self.vram.write_u8(vram_plane, addr, data);
    }

    #[inline]
    pub fn peek_u8(&self, plane: usize, addr: usize, a0: usize) -> u8 {
        self.read_u8(plane, addr, a0)
    }

    #[inline]
    pub fn read_linear(&self, addr: usize) -> u8 {
        self.vram.read_linear(addr)
    }

    #[inline]
    pub fn serialize_linear(&self, addr: usize) -> &[u8] {
        self.vram.serialize_linear(addr)
    }

    #[inline]
    pub fn plane_set(&mut self, plane: usize, addr: usize, a0: usize, data: u8) {
        // Handle odd/even addressing
        let vram_plane = match self.memory_mode.odd_even() {
            0 => plane,
            _ => plane & !0x01 | a0,
        };
        if self.map_mask & (1 << plane) != 0 {
            self.vram.plane_set(vram_plane, addr, data);
        }
    }

    #[inline]
    pub fn plane_and(&mut self, plane: usize, addr: usize, a0: usize, data: u8) {
        if self.map_mask & (1 << plane) != 0 {
            self.vram.plane_and(plane, addr, data);
        }
    }

    #[inline]
    pub fn plane_or(&mut self, plane: usize, addr: usize, a0: usize, data: u8) {
        if self.map_mask & (1 << plane) != 0 {
            self.vram.plane_or(plane, addr, data);
        }
    }
}

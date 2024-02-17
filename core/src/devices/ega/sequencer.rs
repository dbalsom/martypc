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

use crate::{
    device_traits::videocard::VideoCardStateEntry,
    devices::ega::{
        tablegen::{BIT_EXTEND_REVERSE_TABLE64, BIT_EXTEND_TABLE64, BYTE_EXTEND_TABLE64},
        vram::Vram,
        EGA_CHARACTER_HEIGHT,
    },
};
use modular_bitfield::{bitfield, prelude::*, BitfieldSpecifier};

#[derive(Copy, Clone, Debug)]
pub enum SequencerRegister {
    Reset,
    ClockingMode,
    MapMask,
    CharacterMapSelect,
    MemoryMode,
    Invalid,
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
    pub alpha_mode: bool,
    pub extended_memory: B1,
    pub odd_even: OddEvenMode,
    #[skip]
    unused: B5,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct SCharacterMapSelect {
    pub generator_b: B2,
    pub generator_a: B2,
    #[skip]
    unused: B4,
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum OddEvenMode {
    OddEven,
    Sequential,
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

const ODD_EVEN_MASK: u8 = 0b0101;

pub struct Sequencer {
    pub address_byte: u8,
    pub register_selected: SequencerRegister,
    pub reset: u8,                                 // S(0) Reset (WO)
    pub clocking_mode: SClockingModeRegister,      // S(1) Clocking Mode (WO)
    pub map_mask: u8,                              // S(2) Map Mask (wO)
    pub character_map_select: SCharacterMapSelect, // S(3) Character Map Select (WO)
    pub memory_mode: SMemoryModeRegister,          // S(4) Memory Mode (wO)

    pub clock_change_pending: bool,
    pub clock_divisor: u32,
    pub char_clock: u32,

    pub font_select_enabled: bool,
    pub font_offset_a: usize,
    pub font_offset_b: usize,
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
            character_map_select: SCharacterMapSelect::new(),
            memory_mode: SMemoryModeRegister::new(),
            clock_change_pending: false,
            clock_divisor: 1,
            char_clock: 8,
            font_select_enabled: false,
            font_offset_a: 0,
            font_offset_b: 0,
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
    /// The value written to this register controls which register will be written to
    /// when a byte is sent to the Sequencer Data register.
    pub fn write_address(&mut self, byte: u8) {
        //log::trace!("CGA: CRTC register {:02X} selected", byte);
        self.address_byte = byte & 0x0F;

        self.register_selected = match self.address_byte {
            0x00 => SequencerRegister::Reset,
            0x01 => SequencerRegister::ClockingMode,
            0x02 => SequencerRegister::MapMask,
            0x03 => SequencerRegister::CharacterMapSelect,
            0x04 => SequencerRegister::MemoryMode,
            _ => {
                log::debug!("Select to invalid sequencer register: {:02X}", byte);
                SequencerRegister::Invalid
            }
        };
        log::trace!("Sequencer register: {:?} selected", self.register_selected);
    }

    /// Handle a write to the Sequencer Data register.
    ///
    /// Will write to the internal register selected by the Sequencer Address Register.
    pub fn write_data(&mut self, data: u8) {
        let data_byte = data & 0x0F;
        match self.register_selected {
            SequencerRegister::Reset => {
                self.reset = data_byte & 0x03;
                log::trace!("Write to Sequencer::Reset register: {:02X}", data_byte);
            }
            SequencerRegister::ClockingMode => {
                self.clocking_mode = SClockingModeRegister::from_bytes([data_byte]);
                log::trace!("Write to Sequencer::ClockingMode register: {:02X}", data_byte);

                self.clock_change_pending = true;
                (self.clock_divisor, self.char_clock) = match self.clocking_mode.dot_clock() {
                    DotClock::HalfClock => (2, 16),
                    DotClock::Native => (1, 8),
                }
            }
            SequencerRegister::MapMask => {
                self.map_mask = data_byte;
                // Warning: noisy
                //log::trace!("Write to Sequencer::MapMask register: {:02X}", data_byte);
            }
            SequencerRegister::CharacterMapSelect => {
                self.character_map_select = SCharacterMapSelect::from_bytes([data_byte]);
                log::trace!("Write to Sequencer::CharacterMapSelect register: {:02X}", data_byte);

                self.update_character_maps();
            }
            SequencerRegister::MemoryMode => {
                self.memory_mode = SMemoryModeRegister::from_bytes([data_byte]);
                log::trace!("Write to Sequencer::MemoryMode register: {:02X}", data_byte);
            }
            SequencerRegister::Invalid => {
                // ...Do nothing
            }
        }
    }

    pub fn update_character_maps(&mut self) {
        // Character font selection is only enabled if the two generator selections differ.
        self.font_select_enabled = self.character_map_select.generator_a() != self.character_map_select.generator_b();

        self.font_offset_a = match self.character_map_select.generator_a() {
            0b00 => 0x0000,
            0b01 => 0x4000,
            0b10 => 0x8000,
            0b11 | _ => 0xC000,
        };

        self.font_offset_b = match self.character_map_select.generator_b() {
            0b00 => 0x0000,
            0b01 => 0x4000,
            0b10 => 0x8000,
            0b11 | _ => 0xC000,
        };
    }

    pub fn read_u8(&self, plane: usize, addr: usize, a0: usize) -> u8 {
        // Handle odd/even addressing
        match self.memory_mode.odd_even() {
            OddEvenMode::Sequential => self.vram.read_u8(plane, addr),
            //OddEvenMode::OddEven if a0 == (plane & 1) => self.vram.read_u8(plane, addr),
            OddEvenMode::OddEven => self.vram.read_u8(plane, addr),
            //_ => 0,
        }
    }

    #[inline]
    pub fn write_u8(&mut self, plane: usize, addr: usize, a0: usize, data: u8) {
        // Handle odd/even addressing
        match self.memory_mode.odd_even() {
            OddEvenMode::Sequential => {
                if self.map_mask & (1 << plane) != 0 {
                    self.vram.write_u8(plane, addr, data);
                }
            }
            OddEvenMode::OddEven => {
                if (self.map_mask & (ODD_EVEN_MASK << a0)) & (1 << plane) != 0 {
                    self.vram.write_u8(plane, addr, data);
                }
            }
        };
    }

    /*    #[inline]
    pub fn read_u8(&self, plane: usize, addr: usize, a0: usize) -> u8 {
        // Handle odd/even addressing
        let vram_plane = match self.memory_mode.odd_even() {
            OddEvenMode::Sequential => plane,
            OddEvenMode::OddEven => plane & !0x01 | a0, // Force plane to even/odd
        };
        self.vram.read_u8(vram_plane, addr)
    }*/

    /*
    pub fn write_u8(&mut self, plane: usize, addr: usize, a0: usize, data: u8) {
        // Handle odd/even addressing
        let vram_plane = match self.memory_mode.odd_even() {
            OddEvenMode::Sequential => plane,
            OddEvenMode::OddEven => plane & !0x01 | a0, // Force plane to even/odd
        };
        if self.map_mask & (1 << vram_plane) != 0 {
            self.vram.write_u8(vram_plane, addr, data);
        }
    }
    */

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
        let mask = match self.memory_mode.odd_even() {
            OddEvenMode::Sequential => self.map_mask,
            OddEvenMode::OddEven => self.map_mask & (ODD_EVEN_MASK << a0),
        };
        if mask & (1 << plane) != 0 {
            self.vram.plane_set(plane, addr, data);
        }
    }

    #[inline]
    pub fn plane_and(&mut self, plane: usize, addr: usize, a0: usize, data: u8) {
        // Handle odd/even addressing
        let mask = match self.memory_mode.odd_even() {
            OddEvenMode::Sequential => self.map_mask,
            OddEvenMode::OddEven => self.map_mask & (ODD_EVEN_MASK << a0),
        };
        if mask & (1 << plane) != 0 {
            self.vram.plane_and(plane, addr, data);
        }
    }

    #[inline]
    pub fn plane_or(&mut self, plane: usize, addr: usize, a0: usize, data: u8) {
        // Handle odd/even addressing
        let mask = match self.memory_mode.odd_even() {
            OddEvenMode::Sequential => self.map_mask,
            OddEvenMode::OddEven => self.map_mask & (ODD_EVEN_MASK << a0),
        };
        if mask & (1 << plane) != 0 {
            self.vram.plane_or(plane, addr, data);
        }
    }

    pub fn font_select_enabled(&self) -> bool {
        self.font_select_enabled
    }

    /// Return a packed u64 value representing the 8-pixel span of the selected font glyph.
    pub fn get_glyph_span(&self, glyph: u8, font: u8, row: u8) -> u64 {
        BIT_EXTEND_TABLE64[self.vram.read_glyph(self.get_glyph_address(glyph, font, row)) as usize]
    }

    pub fn test_glyph_span(&self, row: u8) -> u64 {
        // Return a test character
        match row {
            0 => 0x0101010101010101,
            1 => 0x0100000000000001,
            2 => 0x0100000000000001,
            3 => 0x0100000000000001,
            4 => 0x0100000000000001,
            5 => 0x0100000000000001,
            6 => 0x0100000000000001,
            7 => 0x0100000000000001,
            8 => 0x0101010101010101,
            _ => 0,
        }
    }

    pub fn get_glyph_address(&self, glyph: u8, font: u8, row: u8) -> usize {
        let mut offset = match font {
            0 => self.font_offset_b,
            _ => self.font_offset_a,
        };
        offset + ((glyph as usize) * EGA_CHARACTER_HEIGHT) + row as usize
    }

    #[rustfmt::skip]
    pub fn get_state(&self) -> Vec<(String, VideoCardStateEntry)> {
        let mut sequencer_vec = Vec::new();
        sequencer_vec.push((format!("{:?}", SequencerRegister::Reset), VideoCardStateEntry::String(format!("{:02b}", self.reset))));
        sequencer_vec.push((format!("{:?}", SequencerRegister::ClockingMode), VideoCardStateEntry::String(format!("{:04b}", self.clocking_mode.into_bytes()[0]))));
        sequencer_vec.push((format!("{:?} [cc]", SequencerRegister::ClockingMode), VideoCardStateEntry::String(format!("{:?}", self.clocking_mode.character_clock()))));
        sequencer_vec.push((format!("{:?} [bw]", SequencerRegister::ClockingMode), VideoCardStateEntry::String(format!("{}", self.clocking_mode.bandwidth()))));
        sequencer_vec.push((format!("{:?} [sl]", SequencerRegister::ClockingMode), VideoCardStateEntry::String(format!("{}", self.clocking_mode.shift_load()))));
        sequencer_vec.push((format!("{:?} [dc]", SequencerRegister::ClockingMode), VideoCardStateEntry::String(format!("{:?}", self.clocking_mode.dot_clock()))));

        sequencer_vec.push((format!("{:?}", SequencerRegister::MapMask), VideoCardStateEntry::String(format!("{:04b}", self.map_mask))));
        sequencer_vec.push((format!("{:?}", SequencerRegister::CharacterMapSelect), VideoCardStateEntry::String(format!("{:04b}", self.character_map_select.into_bytes()[0]))));
        sequencer_vec.push((format!("{:?}", SequencerRegister::MemoryMode), VideoCardStateEntry::String(format!("{:04b}", self.memory_mode.into_bytes()[0]))));
        sequencer_vec.push((format!("{:?} [ag]", SequencerRegister::MemoryMode), VideoCardStateEntry::String(format!("{}", self.memory_mode.alpha_mode() as u8))));
        sequencer_vec.push((format!("{:?} [em]", SequencerRegister::MemoryMode), VideoCardStateEntry::String(format!("{}", self.memory_mode.extended_memory() as u8))));
        sequencer_vec.push((format!("{:?} [oe]", SequencerRegister::MemoryMode), VideoCardStateEntry::String(format!("{:?}", self.memory_mode.odd_even()))));
        sequencer_vec
    }
}

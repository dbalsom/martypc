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

    vga::sequencer_regs.rs

    Implement the VGA Sequencer registers.

*/

use crate::devices::vga_old::*;
use modular_bitfield::prelude::*;

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
    pub bandwidth: B1, // Unused on VGA
    pub shift_load: B1,
    pub dot_clock: DotClock,
    pub shift_four: ShiftFour,
    pub screen_off: bool,
    #[skip]
    unused: B2,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct SCharacterMapSelect {
    pub select_generator_b: B2,
    pub select_generator_a: B2,
    pub sbh_bit: B1,
    pub sah_bit: B1,
    #[skip]
    unused: B2,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct SMemoryMode {
    #[skip]
    unused: B1, // Alpha/Graphics bit on the EGA, Ferraro says not to use this for A/G status on VGA.
    pub extended_memory: bool, // We will always set this to 1 for 256Kb of VRAM
    pub odd_even_enable: bool,
    pub chain4_enable: bool,
    #[skip]
    unused: B4,
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum ShiftFour {
    EveryClock,
    EightDots,
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum CharacterClock {
    EightDots,
    NineDots,
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum DotClock {
    Native,
    HalfClock,
}

impl VGACard {
    /// Handle a write to the Sequencer Address register.
    ///
    /// The value written to this register controls which regsiter will be written to
    /// when a byte is sent to the Sequencer Data register.
    pub fn write_sequencer_address(&mut self, byte: u8) {
        //log::trace!("CGA: CRTC register {:02X} selected", byte);
        self.sequencer_address_byte = byte & 0x1F;

        self.sequencer_register_selected = match self.sequencer_address_byte {
            0x00 => SequencerRegister::Reset,
            0x01 => SequencerRegister::ClockingMode,
            0x02 => SequencerRegister::MapMask,
            0x03 => SequencerRegister::CharacterMapSelect,
            0x04 => SequencerRegister::MemoryMode,
            _ => {
                trace!(self, "Select to invalid sequencer register: {:02X}", byte);
                log::warn!("Select to invalid sequencer register: {:02X}", byte);
                self.sequencer_register_selected
            }
        }
    }

    /// Handle a write to the Sequencer Data register.
    ///
    /// Will write to the internal register selected by the Sequencer Address Register.
    pub fn write_sequencer_data(&mut self, byte: u8) {
        match self.sequencer_register_selected {
            SequencerRegister::Reset => {
                self.sequencer_reset = byte & 0x03;

                trace!(self, "Write to Sequencer::Reset register: {:02X}", byte);
                log::trace!("Write to Sequencer::Reset register: {:02X}", byte);

                // expirmental: reset latches?
                //for i in 0..4 {
                //    self.planes[i].latch = 0x00;
                //}
            }
            SequencerRegister::ClockingMode => {
                self.sequencer_clocking_mode = SClockingModeRegister::from_bytes([byte]);

                trace!(self, "Write to Sequencer::ClockingMode register: {:02X}", byte);
                log::trace!("Write to Sequencer::ClockingMode register: {:02X}", byte);
            }
            SequencerRegister::MapMask => {
                self.sequencer_map_mask = byte & 0x0F;
                // Warning: noisy
                trace!(self, "Write to Sequencer::MapMask register: {:02X}", byte);
                //log::trace!("Write to Sequencer::MapMask register: {:02X}", byte);
            }
            SequencerRegister::CharacterMapSelect => {
                self.sequencer_character_map_select = SCharacterMapSelect::from_bytes([byte]);
                // Calculate actual values from extra bits
                self.sequencer_character_map_a = self.sequencer_character_map_select.select_generator_a() as u8
                    | self.sequencer_character_map_select.sah_bit() << 2;
                self.sequencer_character_map_b = self.sequencer_character_map_select.select_generator_b() as u8
                    | self.sequencer_character_map_select.sbh_bit() << 2;

                trace!(self, "Write to Sequencer::CharacterMapSelect register: {:02X}", byte);
                log::trace!("Write to Sequencer::CharacterMapSelect register: {:02X}", byte);
            }
            SequencerRegister::MemoryMode => {
                self.sequencer_memory_mode = SMemoryMode::from_bytes([byte]);

                trace!(self, "Write to Sequencer::MemoryMode register: {:02X}", byte);
                log::trace!("Write to Sequencer::MemoryMode register: {:02X}", byte);
            }
        }
        self.recalculate_mode();
        self.recalculate_timings();
    }

    /// Handle a read from the Sequencer Data register. 0x3C5 (VGA Only)
    ///
    /// The Sequencer Data registers are only readable on the VGA.
    pub fn read_sequencer_data(&mut self) -> u8 {
        let byte = match self.sequencer_register_selected {
            SequencerRegister::Reset => self.sequencer_reset,
            SequencerRegister::ClockingMode => self.sequencer_clocking_mode.into_bytes()[0],
            SequencerRegister::MapMask => self.sequencer_map_mask,
            SequencerRegister::CharacterMapSelect => self.sequencer_character_map_select.into_bytes()[0],
            SequencerRegister::MemoryMode => self.sequencer_memory_mode.into_bytes()[0],
        };
        byte
    }
}

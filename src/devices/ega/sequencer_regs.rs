/*
    Marty PC Emulator 
    (C)2023 Daniel Balsom
    https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

    ---------------------------------------------------------------------------

    ega::sequencer_regs.rs

    Implement the EGA Sequencer registers.

*/

use modular_bitfield::prelude::*;
use crate::devices::ega::EGACard;

#[derive(Copy, Clone, Debug)]
pub enum SequencerRegister {
    Reset,
    ClockingMode,
    MapMask,
    CharacterMapSelect,
    MemoryMode
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
    unused: B4
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum CharacterClock {
    EightDots,
    NineDots
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum DotClock {
    Native,
    HalfClock,
}


impl EGACard {
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
                log::debug!("Select to invalid sequencer register: {:02X}", byte);
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
                log::trace!("Write to Sequencer::Reset register: {:02X}", byte);
            }
            SequencerRegister::ClockingMode => {
                self.sequencer_clocking_mode = SClockingModeRegister::from_bytes([byte]);
                log::trace!("Write to Sequencer::ClockingMode register: {:02X}", byte);
            }
            SequencerRegister::MapMask => {
                self.sequencer_map_mask = byte & 0x0F;
                // Warning: noisy
                //log::trace!("Write to Sequencer::MapMask register: {:02X}", byte);
            }
            SequencerRegister::CharacterMapSelect => {
                self.sequencer_character_map_select = byte & 0x0F;
                log::trace!("Write to Sequencer::CharacterMapSelect register: {:02X}", byte);
            }
            SequencerRegister::MemoryMode => {
                self.sequencer_memory_mode = byte & 0x07;
                log::trace!("Write to Sequencer::MemoryMode register: {:02X}", byte);
            }
        }
        self.recalculate_mode();
    }

}
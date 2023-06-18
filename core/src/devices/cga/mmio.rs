/*
    MartyPC Emulator 
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

    --------------------------------------------------------------------------

    devices::cga::mmio.rs

    Implementation of the MMIO interface for the IBM CGA.

*/

use crate::devices::cga::*;
use crate::bus::{MemoryMappedDevice};

/// Unlike the EGA or VGA the CGA doesn't do any operations on video memory on read/write,
/// but we handle the mirroring of VRAM this way, and for consistency with other devices
impl MemoryMappedDevice for CGACard {

    fn get_read_wait(&mut self, _address: usize, cycles: u32) -> u32 {
        // Look up wait states given the last ticked clock cycle + elapsed cycles
        // passed in.
        let phase = (self.cycles + cycles as u64 + 1) as usize & (0x0F as usize);
        let waits = WAIT_TABLE[phase];

        trace!(
            self, 
            "READ_U8 (T2): PHASE: {:02X}, WAITS: {}", 
            phase,
            waits
        );
        waits
    }

    fn get_write_wait(&mut self, _address: usize, cycles: u32) -> u32 {
        // Look up wait states given the last ticked clock cycle + elapsed cycles
        // passed in.
        let phase = (self.cycles + cycles as u64 + 1) as usize & (0x0F as usize);
        let waits = WAIT_TABLE[phase];

        trace!(
            self, 
            "WRITE_U8 (T2): PHASE: {:02X}, WAITS: {}", 
            phase,
            waits
        );
        waits
    }

    fn mmio_read_u8(&mut self, address: usize, cycles: u32) -> (u8, u32) {

        let a_offset = (address & CGA_MEM_MASK) - CGA_MEM_ADDRESS;
        if a_offset < CGA_MEM_SIZE {
            // Read within memory range
            
            // Look up wait states given the last ticked clock cycle + elapsed cycles
            // passed in.
            let phase = (self.cycles + cycles as u64 + 1) as usize & (0x0F as usize);
            let waits = WAIT_TABLE[phase];

            trace!(
                self, 
                "READ_U8: {:04X}:{:02X} PHASE: {:02X}, WAITS: {}", 
                a_offset, 
                self.mem[a_offset],
                phase,
                waits
            );
            (self.mem[a_offset], waits)

            //(self.mem[a_offset], 0)
        }
        else {
            // Read out of range, shouldn't happen...
            (0xFF, 0)
        }
    }

    fn mmio_write_u8(&mut self, address: usize, byte: u8, cycles: u32) -> u32 {
        let a_offset = (address & CGA_MEM_MASK) - CGA_MEM_ADDRESS;
        if a_offset < CGA_MEM_SIZE {
            self.mem[a_offset] = byte;

            // Look up wait states given the last ticked clock cycle + elapsed cycles
            // passed in.
            let phase = (self.cycles + cycles as u64 + 1) as usize & (0x0F as usize);
            trace!(
                self, 
                "WRITE_U8: {:04X}:{:02X} PHASE: {:02X}, WAITS: {}", 
                a_offset, 
                byte,
                phase,
                WAIT_TABLE[phase]
            );            
            WAIT_TABLE[phase]
        }
        else {
            // Write out of range, shouldn't happen...
            0
        }
    }

    fn mmio_read_u16(&mut self, address: usize, _cycles: u32) -> (u16, u32) {

        let (lo_byte, wait1) = MemoryMappedDevice::mmio_read_u8(self, address, 0);
        let (ho_byte, wait2) = MemoryMappedDevice::mmio_read_u8(self, address + 1, 0);

        log::warn!("Unsupported 16 bit read from VRAM");
        return ((ho_byte as u16) << 8 | lo_byte as u16, wait1 + wait2)
    }    

    fn mmio_write_u16(&mut self, _address: usize, _data: u16, _cycles: u32) -> u32 {
        //trace!(self, "16 byte write to VRAM, {:04X} -> {:05X} ", data, address);
        log::warn!("Unsupported 16 bit write to VRAM");
        0
    }

}
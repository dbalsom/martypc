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
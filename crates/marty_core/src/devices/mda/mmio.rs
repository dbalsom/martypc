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

    devices::mda::mmio.rs

    Implementation of the MMIO interface for the IBM MDA.

*/

use super::*;
use crate::bus::{MemRangeDescriptor, MemoryMappedDevice};

/// Unlike the EGA or VGA the CGA doesn't do any operations on video memory on read/write,
/// but we handle the mirroring of VRAM this way, and for consistency with other devices
impl MemoryMappedDevice for MDACard {
    fn get_read_wait(&mut self, _address: usize, cycles: u32) -> u32 {
        // Look up wait states given the last ticked clock cycle + elapsed cycles
        // passed in.
        let phase = (self.cycles + cycles as u64 + 1) as usize & 0x0F_usize;
        let waits = WAIT_TABLE[phase];

        trace!(self, "READ_U8 (T2): PHASE: {:02X}, WAITS: {}", phase, waits);
        waits
    }

    fn mmio_read_u8(&mut self, address: usize, _cycles: u32, _cpumem: Option<&[u8]>) -> (u8, u32) {
        let a_offset = address & self.mem_mask;

        trace!(self, "READ_U8: {:04X}:{:02X}", a_offset, self.mem[a_offset]);
        (self.mem[a_offset], 0)
    }

    fn mmio_read_u16(&mut self, address: usize, _cycles: u32, cpumem: Option<&[u8]>) -> (u16, u32) {
        let (lo_byte, wait1) = MemoryMappedDevice::mmio_read_u8(self, address, 0, cpumem);
        let (ho_byte, wait2) = MemoryMappedDevice::mmio_read_u8(self, address + 1, 0, cpumem);

        log::warn!("Unsupported 16 bit read from VRAM");
        ((ho_byte as u16) << 8 | lo_byte as u16, wait1 + wait2)
    }

    fn mmio_peek_u8(&self, address: usize, _cpumem: Option<&[u8]>) -> u8 {
        let a_offset = address & self.mem_mask;

        self.mem[a_offset]
    }

    fn mmio_peek_u16(&self, address: usize, _cpumem: Option<&[u8]>) -> u16 {
        let a_offset = address & self.mem_mask;

        (self.mem[a_offset] as u16) << 8 | self.mem[(a_offset + 1) & self.mem_mask] as u16
    }

    fn get_write_wait(&mut self, _address: usize, cycles: u32) -> u32 {
        // Look up wait states given the last ticked clock cycle + elapsed cycles
        // passed in.
        let phase = (self.cycles + cycles as u64 + 1) as usize & 0x0F_usize;
        let waits = WAIT_TABLE[phase];

        trace!(self, "WRITE_U8 (T2): PHASE: {:02X}, WAITS: {}", phase, waits);
        waits
    }

    fn mmio_write_u8(&mut self, address: usize, byte: u8, _cycles: u32, _cpumem: Option<&mut [u8]>) -> u32 {
        let a_offset = address & self.mem_mask;

        self.mem[a_offset] = byte;
        trace!(self, "WRITE_U8: {:04X}:{:02X}", a_offset, byte);
        0
    }

    fn mmio_write_u16(&mut self, _address: usize, _data: u16, _cycles: u32, _cpumem: Option<&mut [u8]>) -> u32 {
        //trace!(self, "16 byte write to VRAM, {:04X} -> {:05X} ", data, address);
        log::warn!("Unsupported 16 bit write to VRAM");
        0
    }

    fn get_mapping(&self) -> Vec<MemRangeDescriptor> {
        let mut mapping = Vec::new();

        match self.subtype {
            VideoCardSubType::None => {
                mapping.push(MemRangeDescriptor {
                    address: 0xB0000,
                    size: MDA_MEM_APERTURE,
                    cycle_cost: 0,
                    read_only: false,
                    priority: 0,
                });
            }
            VideoCardSubType::Hercules => {
                log::debug!("MDA get_mapping(): Using Hercules memory map");
                mapping.push(MemRangeDescriptor {
                    address: 0xB0000,
                    size: HGC_MEM_APERTURE_HALF,
                    cycle_cost: 0,
                    read_only: false,
                    priority: 3, // Allow another MDA card to override this
                });
                mapping.push(MemRangeDescriptor {
                    address: 0xB8000,
                    size: HGC_MEM_APERTURE_HALF,
                    cycle_cost: 0,
                    read_only: false,
                    priority: 0,
                });
            }
            _ => {
                panic!("Bad subtype for MDA!")
            }
        }

        mapping
    }
}

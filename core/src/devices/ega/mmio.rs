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

    ega::mmio.rs

    Implement the EGA MMIO Interface

*/

use super::*;
use crate::bus::MemoryMappedDevice;

impl MemoryMappedDevice for EGACard {
    fn get_read_wait(&mut self, _address: usize, _cycles: u32) -> u32 {
        0
    }

    fn mmio_read_u8(&mut self, address: usize, _cycles: u32) -> (u8, u32) {
        // RAM Enable disables memory mapped IO
        if !self.misc_output_register.enable_ram() {
            return (0, 0);
        }

        let byte = self.gc.cpu_read_u8(&self.sequencer, address);
        (byte, 0)
    }

    fn mmio_read_u16(&mut self, address: usize, cycles: u32) -> (u16, u32) {
        let (lo_byte, wait1) = MemoryMappedDevice::mmio_read_u8(self, address, cycles);
        let (ho_byte, wait2) = MemoryMappedDevice::mmio_read_u8(self, address + 1, cycles);

        //log::warn!("Unsupported 16 bit read from VRAM");
        ((ho_byte as u16) << 8 | lo_byte as u16, wait1 + wait2)
    }

    fn mmio_peek_u8(&self, address: usize) -> u8 {
        // RAM Enable disables memory mapped IO
        if !self.misc_output_register.enable_ram() {
            return 0;
        }

        self.gc.cpu_peek_u8(&self.sequencer, address)
    }

    fn mmio_peek_u16(&self, address: usize) -> u16 {
        // RAM Enable disables memory mapped IO
        if !self.misc_output_register.enable_ram() {
            return 0;
        }

        // Validate address is within current memory map and get the offset into VRAM
        let offset = match self.gc.plane_bounds_check(address) {
            Some(offset) => offset,
            None => return 0,
        };

        //(self.sequencer.peek_u8(0, offset, address & 0x01) as u16) << 8 | self.sequencer.peek_u8(0, offset + 1) as u16
        (self.mmio_peek_u8(address) as u16) << 8 | self.mmio_peek_u8(address + 1) as u16
    }

    fn get_write_wait(&mut self, _address: usize, _cycles: u32) -> u32 {
        0
    }

    #[rustfmt::skip]
    fn mmio_write_u8(&mut self, address: usize, byte: u8, _cycles: u32) -> u32 {
        // RAM Enable disables memory mapped IO
        if !self.misc_output_register.enable_ram() {
            return 0;
        }

        self.gc.cpu_write_u8(&mut self.sequencer, address, byte);
        0
    }

    fn mmio_write_u16(&mut self, _address: usize, _data: u16, _cycles: u32) -> u32 {
        log::warn!("Unsupported 16 bit write to VRAM");
        0
    }
}

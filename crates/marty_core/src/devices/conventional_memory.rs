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
*/

///! A simple conventional memory region, that may be read-only or read-write.
use crate::bus::{MemRangeDescriptor, MemoryMappedDevice, OPEN_BUS_BYTE};

pub struct ConventionalMemory {
    base_address: usize,
    size: usize,      // Size in bytes
    data: Vec<u8>,    // Data buffer
    wait_states: u32, // Number of wait states
    read_only: bool,
}

impl ConventionalMemory {
    pub fn new(base_address: usize, size: usize, wait_states: u32, read_only: bool) -> Self {
        let data = vec![0; size];
        Self {
            base_address,
            size,
            data,
            wait_states,
            read_only,
        }
    }

    pub fn new_rom(base_address: usize, size: usize, wait_states: u32, data: &[u8]) -> Self {
        Self {
            base_address,
            size,
            data: data.to_vec(),
            wait_states,
            read_only: true,
        }
    }
}

impl MemoryMappedDevice for ConventionalMemory {
    #[inline]
    fn get_read_wait(&mut self, _address: usize, _cycles: u32) -> u32 {
        self.wait_states
    }

    fn mmio_read_u8(&mut self, offset: usize, _cycles: u32, _cpumem: Option<&[u8]>) -> (u8, u32) {
        if offset < self.size {
            (self.data[offset], self.wait_states)
        }
        else {
            (OPEN_BUS_BYTE, self.wait_states)
        }
    }

    fn mmio_read_u16(&mut self, _address: usize, _cycles: u32, _cpumem: Option<&[u8]>) -> (u16, u32) {
        (0, self.wait_states)
    }

    #[inline]
    fn mmio_peek_u8(&self, offset: usize, _cpumem: Option<&[u8]>) -> u8 {
        if offset < self.size {
            self.data[offset]
        }
        else {
            OPEN_BUS_BYTE
        }
    }

    fn mmio_peek_u16(&self, _address: usize, _cpumem: Option<&[u8]>) -> u16 {
        0
    }

    #[inline]
    fn get_write_wait(&mut self, _address: usize, _cycles: u32) -> u32 {
        self.wait_states
    }

    fn mmio_write_u8(&mut self, offset: usize, data: u8, _cycles: u32, _cpumem: Option<&mut [u8]>) -> u32 {
        if !self.read_only && offset < self.size {
            self.data[offset] = data;
            self.wait_states
        }
        else {
            self.wait_states
        }
    }

    fn mmio_write_u16(&mut self, _address: usize, _data: u16, _cycles: u32, _cpumem: Option<&mut [u8]>) -> u32 {
        0
    }

    fn get_mapping(&self) -> Vec<MemRangeDescriptor> {
        let mut mapping = Vec::new();

        mapping.push(MemRangeDescriptor {
            address: self.base_address,
            size: self.size,
            cycle_cost: self.wait_states,
            read_only: false,
            priority: 0,
        });

        mapping
    }
}

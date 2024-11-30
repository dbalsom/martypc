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

    --------------------------------------------------------------------------

    devices::lotech_ems.rs

    Implementation of the LoTech 2MB EMS Board
    https://texelec.com/product/lo-tech-ems-2-mb/

*/

use crate::bus::{BusInterface, DeviceRunTimeUnit, IoDevice, MemRangeDescriptor, MemoryMappedDevice, NO_IO_BYTE};

pub const LOTECH_DEFAULT_IO_BASE: u16 = 0x260;
pub const LOTECH_IO_MASK: u16 = !0x03;
pub const LOTECH_DEFAULT_EMS_WINDOW_SEG: usize = 0xE000;
pub const LOTECH_EMS_WINDOW_SIZE: usize = 0x10000;
pub const LOTECH_EMS_SIZE: usize = 0x200000;
pub const LOTECH_EMS_PAGE_SIZE: usize = 0x4000;

pub const LOTECH_PAGE_MASK: usize = 0b1100_0000_0000_0000;
pub const LOTECH_BASE_MASK: usize = 0b0011_1111_1111_1111;
pub const LOTECH_PAGE_SHIFT: usize = 14;

#[derive(Debug, Clone, Copy, Default)]
pub struct PageRegister {
    page_addr: usize,
}

pub struct LotechEmsCard {
    port_base: u16,
    window_addr: usize,
    pages: [PageRegister; 4],
    mem: Vec<u8>,
}

impl Default for LotechEmsCard {
    fn default() -> Self {
        LotechEmsCard {
            port_base: LOTECH_DEFAULT_IO_BASE,
            window_addr: LOTECH_DEFAULT_EMS_WINDOW_SEG << 4,
            pages: [PageRegister::default(); 4],
            mem: vec![0xAA; LOTECH_EMS_SIZE],
        }
    }
}

impl LotechEmsCard {
    pub fn new(port_base: Option<u16>, window_seg: Option<usize>) -> Self {
        LotechEmsCard {
            port_base: port_base.unwrap_or(LOTECH_DEFAULT_IO_BASE),
            window_addr: window_seg.unwrap_or(LOTECH_DEFAULT_EMS_WINDOW_SEG) << 4,
            ..Default::default()
        }
    }

    pub fn page_reg_write(&mut self, port_num: u16, data: u8) {
        self.pages[port_num as usize].page_addr = ((data & 0x7F) as usize) << 14;
    }
}

impl IoDevice for LotechEmsCard {
    fn read_u8(&mut self, port: u16, _delta: DeviceRunTimeUnit) -> u8 {
        // Catch up to CPU state.
        //let _ticks = self.catch_up(delta, false);
        //self.rw_op(ticks, 0, port as u32, RwSlotType::Io);

        if (port & LOTECH_IO_MASK) == self.port_base {
            //self.port_read(port)
            NO_IO_BYTE
        }
        else {
            NO_IO_BYTE
        }
    }

    fn write_u8(&mut self, port: u16, data: u8, _bus: Option<&mut BusInterface>, _delta: DeviceRunTimeUnit) {
        if (port & LOTECH_IO_MASK) == self.port_base {
            // Read is from LPT port.
            let port_num = port & 0x03;
            self.page_reg_write(port_num, data);
        }
    }

    fn port_list(&self) -> Vec<(String, u16)> {
        vec![
            ("EMS Page Register 0".to_string(), self.port_base),
            ("EMS Page Register 1".to_string(), self.port_base + 1),
            ("EMS Page Register 2".to_string(), self.port_base + 2),
            ("EMS Page Register 3".to_string(), self.port_base + 3),
        ]
    }
}

/// Unlike the EGA or VGA the CGA doesn't do any operations on video memory on read/write,
/// but we handle the mirroring of VRAM this way, and for consistency with other devices
impl MemoryMappedDevice for LotechEmsCard {
    fn get_read_wait(&mut self, _address: usize, _cycles: u32) -> u32 {
        0
    }

    fn mmio_read_u8(&mut self, address: usize, _cycles: u32, _cpumem: Option<&[u8]>) -> (u8, u32) {
        let page = (address & LOTECH_PAGE_MASK) >> LOTECH_PAGE_SHIFT;
        let ems_addr = self.pages[page].page_addr + (address & LOTECH_BASE_MASK);

        (self.mem[ems_addr], 0)
    }

    fn mmio_read_u16(&mut self, address: usize, _cycles: u32, cpumem: Option<&[u8]>) -> (u16, u32) {
        let (lo_byte, wait1) = MemoryMappedDevice::mmio_read_u8(self, address, 0, cpumem);
        let (ho_byte, wait2) = MemoryMappedDevice::mmio_read_u8(self, address + 1, 0, cpumem);

        log::warn!("Unsupported 16 bit read from VRAM");
        return ((ho_byte as u16) << 8 | lo_byte as u16, wait1 + wait2);
    }

    fn mmio_peek_u8(&self, address: usize, _cpumem: Option<&[u8]>) -> u8 {
        let page = (address & LOTECH_PAGE_MASK) >> LOTECH_PAGE_SHIFT;
        let ems_addr = self.pages[page].page_addr + (address & LOTECH_BASE_MASK);

        self.mem[ems_addr]
    }

    fn mmio_peek_u16(&self, address: usize, _cpumem: Option<&[u8]>) -> u16 {
        let a_offset = (address & LOTECH_PAGE_MASK) >> LOTECH_PAGE_SHIFT;

        (self.mem[a_offset] as u16) << 8 | self.mem[a_offset + 1] as u16
    }

    fn get_write_wait(&mut self, _address: usize, _cycles: u32) -> u32 {
        0
    }

    fn mmio_write_u8(&mut self, address: usize, byte: u8, _cycles: u32, _cpumem: Option<&mut [u8]>) -> u32 {
        let page = (address & LOTECH_PAGE_MASK) >> LOTECH_PAGE_SHIFT;
        let ems_addr = self.pages[page].page_addr + (address & LOTECH_BASE_MASK);

        self.mem[ems_addr] = byte;
        0
    }

    fn mmio_write_u16(&mut self, _address: usize, _data: u16, _cycles: u32, _cpumem: Option<&mut [u8]>) -> u32 {
        //trace!(self, "16 byte write to VRAM, {:04X} -> {:05X} ", data, address);
        log::warn!("Unsupported 16 bit write to VRAM");
        0
    }

    fn get_mapping(&self) -> Vec<MemRangeDescriptor> {
        let mut mapping = Vec::new();

        mapping.push(MemRangeDescriptor {
            address: self.window_addr,
            size: LOTECH_EMS_WINDOW_SIZE,
            cycle_cost: 0,
            read_only: false,
            priority: 0,
        });

        mapping
    }
}

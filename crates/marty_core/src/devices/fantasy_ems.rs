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

    devices::fantasy_ems.rs

    Implementation of a non-existent 'fantasy' 4MB EMS 4.0 Board
     with conventional backfill, loosely based on the VLSI SCAMP
      motherboard's register scheme.

      Pages 0-3 are the pages in the page frame beginning at
        0xC000, 0xD000, or 0xE000 as per setting.
      Pages 4-27 are the conventional page registers (0x4000 thru 0x9C00 )



*/

use crate::{
    bus::{BusInterface, DeviceRunTimeUnit, IoDevice, MemRangeDescriptor, MemoryMappedDevice, NO_IO_BYTE},
    cpu_common::LogicAnalyzer,
};

// todos:
// proper config
// unmapped state for page frames
// Port EB
// ? reset pages on reset?

pub const FANTASY_DEFAULT_IO_BASE: u16 = 0x260;
pub const FANTASY_IO_MASK: u16 = !0x03;
pub const FANTASY_DEFAULT_EMS_WINDOW_SEG: usize = 0xD000;
pub const FANTASY_EMS_WINDOW_SIZE: usize = 0x10000;

pub const FANTASY_NON_PAGEABLE_CONVENTIONAL_WINDOW_START_SEG: usize = 0x0000;
pub const FANTASY_NON_PAGEABLE_CONVENTIONAL_WINDOW_START_SEG_16K: usize = 0x0400;

pub const FANTASY_NON_PAGEABLE_CONVENTIONAL_WINDOW_SIZE: usize = 0x40000;

pub const FANTASY_PAGEABLE_CONVENTIONAL_WINDOW_START_SEG: usize = 0x4000;
// todo stylistic: 9FFF or A000 (inclusive or exclusive)
pub const FANTASY_PAGEABLE_CONVENTIONAL_WINDOW_END_SEG: usize = 0x9FFF;
pub const FANTASY_PAGEABLE_CONVENTIONAL_WINDOW_END_ADDRESS: usize = 0x9FFFF;
pub const FANTASY_PAGEABLE_CONVENTIONAL_WINDOW_SIZE: usize = 0x60000;  // 0xA0000 - 0x40000
pub const FANTASY_EMS_SIZE: usize = 0x400000;

pub const FANTASY_PAGE_MASK: usize                  = 0b1111_1100_0000_0000_0000;pub const FANTASY_BASE_MASK: usize                  = 0b0000_0011_1111_1111_1111;
pub const FANTASY_PAGE_SHIFT: usize = 14;

pub const FANTASY_PAGE_SELECT_REGISTER: u16 = 0xE8;
pub const FANTASY_PAGE_SET_REGISTER: u16 = 0xEA;
// todo make this EA+EB to commit the write. Support 8 MB for simplicity?
pub const FANTASY_AUTOINCREMENT_PAGE_FLAG: u8 = 0x40;
pub const FANTASY_PAGE_SET_MASK: u8 = 0x3F;
// pages above 36 are not port-accessible and are read only for the sake of page_lookup_table
pub const FANTASY_WRITABLE_PAGE_COUNT: u8 = 36;
pub const FANTASY_PAGE_COUNT: u8 = 52;

// translates the 0x400 of the memory address into the appropriate page
static PAGE_LOOKUP_TABLE: &'static [u8] = &[
    36, 37, 38, 39,     // 0x00000 (inaccessible)
    40, 41, 42, 43,     // 0x10000 (inaccessible)
    44, 45, 46, 47,     // 0x20000 (inaccessible)
    48, 49, 50, 51,     // 0x30000 (inaccessible)
    12, 13, 14, 15, // 0x40000
    16, 17, 18, 19, // 0x50000
    20, 21, 22, 23, // 0x60000
    24, 25, 26, 27, // 0x70000
    28, 29, 30, 31, // 0x80000
    32, 33, 34, 35, // 0x90000
    0, 0, 0, 0,     // 0xA0000
    0, 0, 0, 0,     // 0xB0000
    0, 1, 2, 3,     // 0xC0000
    4, 5, 6, 7,     // 0xD0000
    8, 9, 10, 11,   // 0xE0000
    0, 0, 0, 0      // 0xF0000

];


#[derive(Debug, Clone, Copy, Default)]
pub struct PageRegister {
    page_addr: usize,
    unmapped_default: u8
}

pub struct FantasyEmsCard {
    window_addr: usize,
    non_pageable_conventional_base_addr: usize,
    pageable_conventional_base_addr: usize,
    pages: [PageRegister; FANTASY_PAGE_COUNT as usize],
    mem: Vec<u8>,
    page_index_auto_increment_on: bool,
    current_page_index: u8,
}

impl Default for FantasyEmsCard {
    fn default() -> Self {
        FantasyEmsCard {
            window_addr: FANTASY_DEFAULT_EMS_WINDOW_SEG << 4,
            non_pageable_conventional_base_addr: 0,
            pageable_conventional_base_addr: FANTASY_PAGEABLE_CONVENTIONAL_WINDOW_START_SEG << 4,
//            pages: [PageRegister::default(); FANTASY_PAGE_COUNT
            // todo there's got to be a better way
            page_index_auto_increment_on: false,
            current_page_index: 0,
            pages: [
                // first four pages (the page frame) point to later pages, such that the
                // conventional page frame points to the first pages on the device
                PageRegister {
                    page_addr: 0x28 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x28
                },
                PageRegister {
                    page_addr: 0x29 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x29
                },
                PageRegister {
                    page_addr: 0x2A << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x2A
                },
                PageRegister {
                    page_addr: 0x2B << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x2B
                },
                PageRegister {
                    page_addr: 0x2C << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x2C
                },
                PageRegister {
                    page_addr: 0x2D << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x2D
                },
                PageRegister {
                    page_addr: 0x2E << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x2E
                },
                PageRegister {
                    page_addr: 0x2F << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x2F
                },
                PageRegister {
                    page_addr: 0x30 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x30
                },
                PageRegister {
                    page_addr: 0x31 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x31
                },
                PageRegister {
                    page_addr: 0x32 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x32
                },
                PageRegister {
                    page_addr: 0x33 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x33
                },
// conventional here

                PageRegister {
                    page_addr: 0x10 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x10
                },
                PageRegister {
                    page_addr: 0x11 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x11
                },
                PageRegister {
                    page_addr: 0x12 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x12
                },
                PageRegister {
                    page_addr: 0x13 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x13
                },
                PageRegister {
                    page_addr: 0x14 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x14
                },
                PageRegister {
                    page_addr: 0x15 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x15
                },
                PageRegister {
                    page_addr: 0x16 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x16
                },
                PageRegister {
                    page_addr: 0x17 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x17
                },
                PageRegister {
                    page_addr: 0x18 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x18
                },
                PageRegister {
                    page_addr: 0x19 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x19
                },
                PageRegister {
                    page_addr: 0x1A << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x1A
                },
                PageRegister {
                    page_addr: 0x1B << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x1B
                },
                PageRegister {
                    page_addr: 0x1C << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x1C
                },
                PageRegister {
                    page_addr: 0x1D << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x1D
                },
                PageRegister {
                    page_addr: 0x1E << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x1E
                },
                PageRegister {
                    page_addr: 0x1F << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x1F
                },
                PageRegister {
                    page_addr: 0x20 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x20
                },
                PageRegister {
                    page_addr: 0x21 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x21
                },
                PageRegister {
                    page_addr: 0x22 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x22
                },
                PageRegister {
                    page_addr: 0x23 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x23
                },
                PageRegister {
                    page_addr: 0x24 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x24
                },
                PageRegister {
                    page_addr: 0x25 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x25
                },
                PageRegister {
                    page_addr: 0x26 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x26
                },
                PageRegister {
                    page_addr: 0x27 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x27
                },
// non-pageable conventional
                PageRegister {
                    page_addr: 0x00 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x00
                },
                PageRegister {
                    page_addr: 0x01 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x01
                },
                PageRegister {
                    page_addr: 0x02 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x02
                },
                PageRegister {
                    page_addr: 0x03 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x03
                },
                PageRegister {
                    page_addr: 0x04 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x04
                },
                PageRegister {
                    page_addr: 0x05 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x05
                },
                PageRegister {
                    page_addr: 0x06 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x06
                },
                PageRegister {
                    page_addr: 0x07 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x07
                },
                PageRegister {
                    page_addr: 0x08 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x08
                },
                PageRegister {
                    page_addr: 0x09 << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x09
                },
                PageRegister {
                    page_addr: 0x0A << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x0A
                },
                PageRegister {
                    page_addr: 0x0B << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x0B
                },
                PageRegister {
                    page_addr: 0x0C << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x0C
                },
                PageRegister {
                    page_addr: 0x0D << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x0D
                },
                PageRegister {
                    page_addr: 0x0E << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x0E
                },
                PageRegister {
                    page_addr: 0x0F << FANTASY_PAGE_SHIFT,
                    unmapped_default : 0x0F
                },

            ],
            mem: vec![0xAA; FANTASY_EMS_SIZE],
        }
    }
}

impl FantasyEmsCard {
    pub fn new(window_seg: Option<usize>, base_addr: Option<usize>) -> Self {
        FantasyEmsCard {
            window_addr: window_seg.unwrap_or(FANTASY_DEFAULT_EMS_WINDOW_SEG) << 4,
            non_pageable_conventional_base_addr: base_addr.unwrap_or(FANTASY_NON_PAGEABLE_CONVENTIONAL_WINDOW_START_SEG),
            ..Default::default()
        }
    }

    pub fn page_reg_write(&mut self, port_num: u8, data: u8) {
        //self.pages[port_num as usize].page_addr = ((data & 0x7F) as usize) << 14;
        self.pages[port_num as usize].page_addr = (data as usize) << FANTASY_PAGE_SHIFT;
    }

    pub fn page_reg_unmap(&mut self, port_num: u8) {
        //self.pages[port_num as usize].page_addr = ((data & 0x7F) as usize) << 14;
        self.pages[port_num as usize].page_addr = (self.pages[port_num as usize].unmapped_default as usize) << FANTASY_PAGE_SHIFT;
    }

}

impl IoDevice for FantasyEmsCard {
    fn read_u8(&mut self, port: u16, _delta: DeviceRunTimeUnit) -> u8 {
        // Catch up to CPU state.
        //let _ticks = self.catch_up(delta, false);
        //self.rw_op(ticks, 0, port as u32, RwSlotType::Io);


        if (port == FANTASY_PAGE_SELECT_REGISTER) {
            self.current_page_index
        } else if (port == FANTASY_PAGE_SET_REGISTER) {
            (self.pages[self.current_page_index as usize].page_addr >> FANTASY_PAGE_SHIFT) as u8
        } else {
            NO_IO_BYTE

        }
    }

    fn write_u8(
        &mut self,
        port: u16,
        data: u8,
        _bus: Option<&mut BusInterface>,
        _delta: DeviceRunTimeUnit,
        _analyzer: Option<&mut LogicAnalyzer>,
    ) {
        if (port == FANTASY_PAGE_SELECT_REGISTER) {
            if (data > FANTASY_WRITABLE_PAGE_COUNT){
                log::warn!("Out of range page select register write! {}", data);
                self.current_page_index = 0;
            } else {
                self.current_page_index = data;
            }

            if ((data & FANTASY_AUTOINCREMENT_PAGE_FLAG) == FANTASY_AUTOINCREMENT_PAGE_FLAG){
                self.page_index_auto_increment_on = true;
            } else {
                self.page_index_auto_increment_on = false;
            }
        }
        else if (port == FANTASY_PAGE_SET_REGISTER) {
            if (data == 0xFF){
                //log::warn!("Page {} Unset!", self.current_page_index);
                self.page_reg_unmap(self.current_page_index);
            } else {
                //log::warn!("Page set! {} as {}", self.current_page_index, data);
                self.page_reg_write(self.current_page_index, data);
            }

            if (self.page_index_auto_increment_on){
                self.current_page_index += 1;
                if (self.current_page_index > FANTASY_WRITABLE_PAGE_COUNT){
                    self.current_page_index = 0;
                }

            }
        }
    }

    fn port_list(&self) -> Vec<(String, u16)> {
        vec![
            ("EMS Page Select Register".to_string(), FANTASY_PAGE_SELECT_REGISTER),
            ("EMS Page Set Register".to_string(), FANTASY_PAGE_SET_REGISTER),
        ]
    }
}

impl MemoryMappedDevice for FantasyEmsCard {
    fn get_read_wait(&mut self, _address: usize, _cycles: u32) -> u32 {
        0
    }

    fn mmio_read_u8(&mut self, address: usize, _cycles: u32, _cpumem: Option<&[u8]>) -> (u8, u32) {
        let page = PAGE_LOOKUP_TABLE[(address & FANTASY_PAGE_MASK) >> FANTASY_PAGE_SHIFT] as usize;
        let ems_addr = self.pages[page].page_addr + (address & FANTASY_BASE_MASK);

        if (ems_addr == 0x9C000){

        }

        (self.mem[ems_addr], 0)
    }

    fn mmio_read_u16(&mut self, address: usize, _cycles: u32, cpumem: Option<&[u8]>) -> (u16, u32) {
        let (lo_byte, wait1) = MemoryMappedDevice::mmio_read_u8(self, address, 0, cpumem);
        let (ho_byte, wait2) = MemoryMappedDevice::mmio_read_u8(self, address + 1, 0, cpumem);

        log::warn!("Unsupported 16 bit read from VRAM");
        ((ho_byte as u16) << 8 | lo_byte as u16, wait1 + wait2)
    }

    fn mmio_peek_u8(&self, address: usize, _cpumem: Option<&[u8]>) -> u8 {
        let page = PAGE_LOOKUP_TABLE[(address & FANTASY_PAGE_MASK) >> FANTASY_PAGE_SHIFT] as usize;
        let ems_addr = self.pages[page].page_addr + (address & FANTASY_BASE_MASK);
        
        self.mem[ems_addr]
    }

    fn mmio_peek_u16(&self, address: usize, _cpumem: Option<&[u8]>) -> u16 {
        // todo im pretty sure this is wrong.
        let a_offset = (address & FANTASY_PAGE_MASK) >> FANTASY_PAGE_SHIFT;

        (self.mem[a_offset] as u16) << 8 | self.mem[a_offset + 1] as u16
    }

    fn get_write_wait(&mut self, _address: usize, _cycles: u32) -> u32 {
        0
    }

    fn mmio_write_u8(&mut self, address: usize, byte: u8, _cycles: u32, _cpumem: Option<&mut [u8]>) -> u32 {
        let page = PAGE_LOOKUP_TABLE[(address & FANTASY_PAGE_MASK) >> FANTASY_PAGE_SHIFT] as usize;
        let ems_addr = self.pages[page].page_addr + (address & FANTASY_BASE_MASK);

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
            size: FANTASY_EMS_WINDOW_SIZE,
            cycle_cost: 0,
            read_only: false,
            priority: 0,
        });

        // should this be its own mapping for clarity reasons?
        // or merged with the pageable one below?
        mapping.push(MemRangeDescriptor {
            address: self.non_pageable_conventional_base_addr,
            size: FANTASY_NON_PAGEABLE_CONVENTIONAL_WINDOW_SIZE - self.non_pageable_conventional_base_addr,
            cycle_cost: 0,
            read_only: false,
            priority: 0,
        });


        mapping.push(MemRangeDescriptor {
            address: self.pageable_conventional_base_addr,
            size: FANTASY_PAGEABLE_CONVENTIONAL_WINDOW_SIZE,
            cycle_cost: 0,
            read_only: false,
            priority: 0,
        });

        mapping
    }
}

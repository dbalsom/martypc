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

//! An implementation of the ATA Drive Identification structure.

use std::str::FromStr;

use crate::{devices::ata::ata_string::AtaString, vhd::VHDGeometry};

use crate::device_types::geometry::DriveGeometry;
use binrw::binrw;

pub const CAPABILITIES_LBA: u16 = 0b0000_0010_0000_0001;
pub const CAPABILITIES_DMA: u16 = 0b0000_0001_0000_0000;

#[binrw]
#[derive(Default)]
#[brw(little)]
pub struct AtaDriveIdentification {
    pub general: u16,
    pub cylinders: u16,
    pub specific_configuration: u16,
    pub num_heads: u16,
    pub unformatted_bytes_per_track: u16,
    pub unformatted_bytes_per_sector: u16,
    pub sectors_per_track: u16,
    pub vendor_unique: [u16; 3],
    pub serial_no: [u8; 20],
    pub buffer_type: u16,
    pub buffer_size: u16,
    pub long_cmd_bytes: u16,
    pub firmware_revision: [u8; 8],
    pub model_number: AtaString<40>,
    pub maximum_block_transfer: u8,
    pub vendor_unique2: u8,
    pub double_word_io: u16,
    pub capabilities: u16,
    pub reserved: u16,
    pub pio_timing: u16,
    pub dma_timing: u16,
    pub field_validity: u16,
    pub current_cylinders: u16,
    pub current_heads: u16,
    pub current_sectors_per_track: u16,
    pub current_capacity_low: u16,
    pub current_capacity_high: u16,
    pub multiple_sector: u16,
    pub user_addressable_sectors: u32,
    pub single_word_dma: u16,
    pub multi_word_dma: u16,
}

impl AtaDriveIdentification {
    pub fn new(vhdgeometry: &DriveGeometry, sector_size: usize, lba: bool, dma: bool) -> Self {
        let current_capacity: u32 = vhdgeometry.c as u32 * vhdgeometry.h as u32 * vhdgeometry.s as u32;

        let mut capabilities = 0;
        if lba {
            capabilities |= CAPABILITIES_LBA;
        }
        if dma {
            capabilities |= CAPABILITIES_DMA;
        }

        AtaDriveIdentification {
            general: 0b0000_0000_0100_0000, // Fixed Disk
            cylinders: vhdgeometry.c,
            num_heads: vhdgeometry.h as u16,
            unformatted_bytes_per_track: sector_size as u16 * vhdgeometry.s as u16,
            unformatted_bytes_per_sector: sector_size as u16,
            sectors_per_track: vhdgeometry.s as u16,
            current_cylinders: vhdgeometry.c,
            current_heads: vhdgeometry.h as u16,
            current_sectors_per_track: vhdgeometry.s as u16,
            serial_no: "0000000000000MARTYPC".as_bytes().try_into().unwrap(),
            model_number: AtaString::from_str("MartyPC Hard Drive").unwrap(),
            firmware_revision: "1.2.3.4 ".as_bytes().try_into().unwrap(),
            maximum_block_transfer: 1,
            field_validity: 1,
            current_capacity_low: current_capacity as u16,
            current_capacity_high: (current_capacity >> 16) as u16,
            user_addressable_sectors: current_capacity,
            capabilities,
            ..Default::default()
        }
    }
}

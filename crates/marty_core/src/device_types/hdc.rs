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

    devices::types::hdc.rs

    Defines types common to implementations of a Hard Disk Controller
*/

use lazy_static::lazy_static;
use std::fmt::{Display, Formatter};

pub const HDC_SECTOR_SIZE: usize = 512;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HardDiskFormat {
    pub max_cylinders: u16,
    pub max_heads: u8,
    pub max_sectors: u8,
    pub wpc: Option<u16>,
    pub desc: String,
}

impl HardDiskFormat {
    pub fn get_size(&self) -> usize {
        (self.max_cylinders as usize) * (self.max_heads as usize) * (self.max_sectors as usize) * HDC_SECTOR_SIZE
    }
}

impl Display for HardDiskFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let size = self.get_size() as f32;
        let size_in_mb = size / 1024.0 / 1024.0;

        write!(
            f,
            "c:{} h:{} s:{} wpc:{} ({:.1})",
            self.max_cylinders,
            self.max_heads,
            self.max_sectors,
            match self.wpc {
                Some(wpc) => wpc.to_string(),
                None => "N/A".to_string(),
            },
            size_in_mb
        )
    }
}

lazy_static! {
    static ref XT_HARD_DISK_TYPES: [Option<HardDiskFormat>; 5] = [
        None,
        // "Type 1"
        Some(HardDiskFormat {
            max_cylinders: 306,
            max_heads: 4,
            max_sectors: 17,
            wpc: Some(306),
            desc: "10,653,696 bytes (10MB)".to_string(),
        }),
        // "Type 2"
        Some(HardDiskFormat {
            max_cylinders: 615,
            max_heads: 4,
            max_sectors: 17,
            wpc: Some(300),
            desc: "21,377,024 Bytes (20MB)".to_string(),
        }),
        // "Type 3"
        Some(HardDiskFormat {
            max_cylinders: 306,
            max_heads: 8,
            max_sectors: 17,
            wpc: Some(128),
            desc: "21,307,392 Bytes (20MB)".to_string(),
        }),
        // "Type 4"
        Some(HardDiskFormat {
            max_cylinders: 612,
            max_heads: 4,
            max_sectors: 17,
            wpc: Some(0),
            desc: "21,307,392 Bytes (20MB)".to_string(),
        }),
    ];

    static ref AT_HARD_DISK_TYPES: [Option<HardDiskFormat>; 16] = [
        None,
        // "Type 1"
        Some(HardDiskFormat {
            max_cylinders: 306,
            max_heads: 4,
            max_sectors: 17,
            wpc: Some(128),
            desc: "10MB".to_string(),
        }),
        // "Type 2"
        Some(HardDiskFormat {
            max_cylinders: 615,
            max_heads: 4,
            max_sectors: 17,
            wpc: Some(300),
            desc: "20MB".to_string(),
        }),
        // "Type 3"
        Some(HardDiskFormat {
            max_cylinders: 615,
            max_heads: 6,
            max_sectors: 17,
            wpc: Some(300),
            desc: "30MB".to_string(),
        }),
        // "Type 4"
        Some(HardDiskFormat {
            max_cylinders: 940,
            max_heads: 8,
            max_sectors: 17,
            wpc: Some(512),
            desc: "62MB".to_string(),
        }),
        // "Type 5"
        Some(HardDiskFormat {
            max_cylinders: 940,
            max_heads: 6,
            max_sectors: 17,
            wpc: Some(512),
            desc: "40MB".to_string(),
        }),
        // "Type 6"
        Some(HardDiskFormat {
            max_cylinders: 615,
            max_heads: 4,
            max_sectors: 17,
            wpc: None,
            desc: "20MB".to_string(),
        }),
        // "Type 7"
        Some(HardDiskFormat {
            max_cylinders: 462,
            max_heads: 8,
            max_sectors: 17,
            wpc: Some(256),
            desc: "30MB".to_string(),
        }),
        // "Type 8"
        Some(HardDiskFormat {
            max_cylinders: 733,
            max_heads: 5,
            max_sectors: 17,
            wpc: None,
            desc: "30MB".to_string(),
        }),
        // "Type 9"
        Some(HardDiskFormat {
            max_cylinders: 900,
            max_heads: 15,
            max_sectors: 17,
            wpc: None,
            desc: "112MB".to_string(),
        }),
        // "Type 10"
        Some(HardDiskFormat {
            max_cylinders: 820,
            max_heads: 3,
            max_sectors: 17,
            wpc: None,
            desc: "20MB".to_string(),
        }),
        // "Type 11"
        Some(HardDiskFormat {
            max_cylinders: 855,
            max_heads: 5,
            max_sectors: 17,
            wpc: None,
            desc: "35MB".to_string(),
        }),
        // "Type 12"
        Some(HardDiskFormat {
            max_cylinders: 855,
            max_heads: 8,
            max_sectors: 17,
            wpc: None,
            desc: "49MB".to_string(),
        }),
        // "Type 13"
        Some(HardDiskFormat {
            max_cylinders: 306,
            max_heads: 8,
            max_sectors: 17,
            wpc: Some(128),
            desc: "20MB".to_string(),
        }),
        // "Type 14"
        Some(HardDiskFormat {
            max_cylinders: 306,
            max_heads: 4,
            max_sectors: 17,
            wpc: Some(128),
            desc: "10MB".to_string(),
        }),
        // "Type 15"
        None,
    ];
}

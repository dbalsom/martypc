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

    devices::types::fdc.rs

    Defines types common to implementations of a Floppy Disk Controller
*/

use crate::machine_types::FloppyDriveType;
use fluxfox::prelude::*;
use lazy_static::lazy_static;
use std::collections::HashMap;

pub struct DiskFormat {
    pub chs: DiskChs,
}

#[derive(Copy, Clone, Debug)]
pub enum FloppyImageType {
    Image160K,
    Image180K,
    Image320K,
    Image360K,
    Image720K,
    Image12M,
    Image144M,
}

impl TryFrom<FloppyImageType> for StandardFormat {
    type Error = &'static str;

    fn try_from(value: FloppyImageType) -> Result<Self, Self::Error> {
        match value {
            FloppyImageType::Image160K => Ok(StandardFormat::PcFloppy160),
            FloppyImageType::Image180K => Ok(StandardFormat::PcFloppy180),
            FloppyImageType::Image320K => Ok(StandardFormat::PcFloppy320),
            FloppyImageType::Image360K => Ok(StandardFormat::PcFloppy360),
            FloppyImageType::Image720K => Ok(StandardFormat::PcFloppy720),
            FloppyImageType::Image12M => Ok(StandardFormat::PcFloppy1200),
            FloppyImageType::Image144M => Ok(StandardFormat::PcFloppy1440),
        }
    }
}

lazy_static! {
    /// Define the drive capabilities for each floppy drive type.
    /// Drives can seek a bit beyond the end of the traditional media sizes.
    /// TODO: Determine accurate values
    pub static ref DRIVE_CAPABILITIES: HashMap<FloppyDriveType, DiskFormat> = {
        let mut map = HashMap::new();
        map.insert(
            FloppyDriveType::Floppy360K,
            DiskFormat {
                chs: DiskChs::new(45, 2, 9),
            },
        );
        map.insert(
            FloppyDriveType::Floppy720K,
            DiskFormat {
                chs: DiskChs::new(85, 2, 9),
            },
        );
        map.insert(
            FloppyDriveType::Floppy12M,
            DiskFormat {
                chs: DiskChs::new(85, 2, 15),
            },
        );
        map.insert(
            FloppyDriveType::Floppy144M,
            DiskFormat {
                chs: DiskChs::new(85, 2, 18),
            },
        );
        map
    };
}

lazy_static! {
    pub static ref DISK_FORMATS: HashMap<usize, DiskFormat> = {
        
        HashMap::from([
            (
                163_840,
                DiskFormat {
                    chs: DiskChs::new(40, 1, 8),
                },
            ),
            (
                184_320,
                DiskFormat {
                    chs: DiskChs::new(40, 1, 9),
                },
            ),
            (
                327_680,
                DiskFormat {
                    chs: DiskChs::new(40, 2, 8),
                },
            ),
            (
                368_640,
                DiskFormat {
                    chs: DiskChs::new(40, 2, 9),
                },
            ),
            (
                737_280,
                DiskFormat {
                    chs: DiskChs::new(80, 2, 9),
                },
            ),
            (
                1_228_800,
                DiskFormat {
                    chs: DiskChs::new(80, 2, 15),
                },
            ),
            (
                1_474_560,
                DiskFormat {
                    chs: DiskChs::new(80, 2, 18),
                },
            ),
        ])
    };
}

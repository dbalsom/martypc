/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

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

    device_types::fdc.rs
*/

//! Defines types common to implementations of a Floppy Disk Controller

use crate::machine_types::FloppyDriveType;
use fluxfox::prelude::*;
use lazy_static::lazy_static;
pub use marty_common::types::floppy::FloppyImageType;
use marty_common::MartyHashMap;
use serde_derive::Deserialize;
use std::fmt;

/// Policy for handling image insertion with technically mismatched image types.
/// Strict will enforce physical diskette dimensions. Lenient will allow insertion of 5.25" DD
/// images into 3.5" drives.
#[derive(Copy, Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub enum ImageInsertionPolicy {
    #[default]
    Strict,
    Lenient,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FloppyDataRate {
    Rate125Kbps,
    Rate250Kbps,
    Rate300Kbps,
    Rate500Kbps,
    Rate1000Kbps,
    Nonstandard(u32),
}

impl From<TrackDataRate> for FloppyDataRate {
    fn from(value: TrackDataRate) -> Self {
        match value {
            TrackDataRate::Rate125Kbps(_) => Self::Rate125Kbps,
            TrackDataRate::Rate250Kbps(_) => Self::Rate250Kbps,
            TrackDataRate::Rate300Kbps(_) => Self::Rate300Kbps,
            TrackDataRate::Rate500Kbps(_) => Self::Rate500Kbps,
            TrackDataRate::Rate1000Kbps(_) => Self::Rate1000Kbps,
            TrackDataRate::RateNonstandard(rate) => Self::Nonstandard(rate),
        }
    }
}

impl fmt::Display for FloppyDataRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rate125Kbps => write!(f, "125Kbps"),
            Self::Rate250Kbps => write!(f, "250Kbps"),
            Self::Rate300Kbps => write!(f, "300Kbps"),
            Self::Rate500Kbps => write!(f, "500Kbps"),
            Self::Rate1000Kbps => write!(f, "1000Kbps"),
            Self::Nonstandard(rate) => write!(f, "{}Kbps", rate / 1000),
        }
    }
}

pub struct DriveCapabilities {
    pub chs: DiskChs,
    pub data_rates: &'static [FloppyDataRate],
}

pub struct DiskFormat {
    pub chs: DiskChs,
}

#[derive(Copy, Clone, Debug)]
pub struct CoreFloppyImageType(pub FloppyImageType);

impl From<FloppyImageType> for CoreFloppyImageType {
    fn from(value: FloppyImageType) -> Self {
        Self(value)
    }
}

impl TryFrom<CoreFloppyImageType> for StandardFormat {
    type Error = &'static str;

    fn try_from(value: CoreFloppyImageType) -> Result<Self, Self::Error> {
        match value.0 {
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
    pub static ref DRIVE_CAPABILITIES: MartyHashMap<FloppyDriveType, DriveCapabilities> = {
        let mut map = MartyHashMap::default();
        map.insert(
            FloppyDriveType::Floppy360K,
            DriveCapabilities {
                chs: DiskChs::new(45, 2, 9),
                data_rates: &[FloppyDataRate::Rate250Kbps],
            },
        );
        map.insert(
            FloppyDriveType::Floppy720K,
            DriveCapabilities {
                chs: DiskChs::new(85, 2, 9),
                data_rates: &[FloppyDataRate::Rate250Kbps],
            },
        );
        map.insert(
            FloppyDriveType::Floppy12M,
            DriveCapabilities {
                chs: DiskChs::new(85, 2, 15),
                data_rates: &[
                    FloppyDataRate::Rate250Kbps,
                    FloppyDataRate::Rate300Kbps,
                    FloppyDataRate::Rate500Kbps,
                ],
            },
        );
        map.insert(
            FloppyDriveType::Floppy144M,
            DriveCapabilities {
                chs: DiskChs::new(85, 2, 18),
                data_rates: &[FloppyDataRate::Rate250Kbps, FloppyDataRate::Rate500Kbps],
            },
        );
        map
    };
}

lazy_static! {
    pub static ref DISK_FORMATS: MartyHashMap<usize, DiskFormat> = {
        [
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
        ]
        .into_iter()
        .collect()
    };
}

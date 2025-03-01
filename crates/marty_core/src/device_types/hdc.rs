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

use std::fmt::{Debug, Display, Formatter};

use crate::device_types::geometry::DriveGeometry;

pub const HDC_SECTOR_SIZE: usize = 512;

#[derive(Clone, Default, Eq, PartialEq)]
pub struct HardDiskFormat {
    pub geometry: DriveGeometry,
    pub wpc: Option<u16>,
    pub desc: String,
}

impl HardDiskFormat {
    pub fn total_size(&self) -> usize {
        self.geometry.total_sectors() * HDC_SECTOR_SIZE
    }
}

impl Display for HardDiskFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let size = self.total_size() as f32;
        let size_in_mb = (size / 1024.0 / 1024.0).floor() as u32;
        write!(
            f,
            "{}MB: (CHS: {}, {}, {})",
            size_in_mb,
            self.geometry.c(),
            self.geometry.h(),
            self.geometry.s()
        )
    }
}

impl Debug for HardDiskFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let size = self.total_size() as f32;
        let size_in_mb = size / 1024.0 / 1024.0;

        write!(
            f,
            "geometry: {} wpc:{} ({:.1})",
            self.geometry,
            match self.wpc {
                Some(wpc) => wpc.to_string(),
                None => "N/A".to_string(),
            },
            size_in_mb
        )
    }
}

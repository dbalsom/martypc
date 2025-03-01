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

//! Define a [DriveGeometry] that represents cylinder, head, and sector based
//! drive geometry.
//! This is only used for hard drives - for floppy disks we use fluxfox's
//! `SectorLayout`, from which this is copied.

use crate::{
    device_types::chs::{DiskChs, DiskChsIterator},
    devices::hdc::DEFAULT_SECTOR_SIZE,
};
use std::fmt::Display;

/// A structure representing how sectors are laid out on a disk (assuming standard format)
///  - Cylinder (c)
///  - Head (h)
///  - Sector count (s)
///
/// Plus a sector ID offset (s_off) to represent whether a standard sector id starts at 0 or 1.
///
/// A DiskChs may represent a Sector ID, where size is ignored, or an overall disk geometry.
#[derive(Copy, Clone, Debug, Default, Hash, Eq, PartialEq)]
pub struct DriveGeometry {
    pub(crate) c: u16,
    pub(crate) h: u8,
    pub(crate) s: u8,
    pub(crate) s_off: u8,
    pub(crate) size: usize,
}

impl Display for DriveGeometry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[c:{:2} h:{} s:{:2} s_off:{}]", self.c, self.h, self.s, self.s_off)
    }
}

impl TryFrom<usize> for DriveGeometry {
    type Error = &'static str;

    fn try_from(size: usize) -> Result<Self, Self::Error> {
        let sector_size = DEFAULT_SECTOR_SIZE;
        let total_sectors = size / sector_size;
        if total_sectors % sector_size != 0 {
            return Err("Invalid sector size");
        }
        let c = 80;
        let h = 2;
        let s = 10;
        let s_off = 1;
        Ok(Self { c, h, s, s_off, size })
    }
}

impl DriveGeometry {
    /// Create a new [DriveGeometry] structure from cylinder, head and sector id components.
    pub fn new(c: u16, h: u8, s: u8, s_off: u8, size: usize) -> Self {
        Self { c, h, s, s_off, size }
    }
    pub fn get(&self) -> (u16, u8, u8, u8, usize) {
        (self.c, self.h, self.s, self.s_off, self.size)
    }
    /// Return the cylinder (c) field.
    #[inline]
    pub fn c(&self) -> u16 {
        self.c
    }
    /// Return the head (h) field.
    #[inline]
    pub fn h(&self) -> u8 {
        self.h
    }
    /// Return the sector count (s) field.
    #[inline]
    pub fn s(&self) -> u8 {
        self.s
    }
    /// Return the sector id offset (s_off) field.
    #[inline]
    pub fn s_off(&self) -> u8 {
        self.s_off
    }
    #[inline]
    /// Return the size of a sector in bytes.
    pub fn size(&self) -> usize {
        self.size
    }
    /// Return a [DiskChs] structure representing the cylinder, head and sector count components of a [DriveGeometry].
    #[inline]
    pub fn chs(&self) -> DiskChs {
        DiskChs::new(self.c, self.h, self.s)
    }
    /// Set the cylinder count (c) component of a [DriveGeometry].
    #[inline]
    pub fn set_c(&mut self, c: u16) {
        self.c = c;
    }
    /// Set the head count (h) component of a [DriveGeometry].
    #[inline]
    pub fn set_h(&mut self, h: u8) {
        self.h = h;
    }
    /// Set the sector count (s) component of a [DriveGeometry].
    #[inline]
    pub fn set_s(&mut self, s: u8) {
        self.s = s;
    }
    /// Set the sector id offset (s_off) component of a [DriveGeometry].
    #[inline]
    pub fn set_s_off(&mut self, s_off: u8) {
        self.s_off = s_off;
    }
    /// Return the number of sectors represented by a [DriveGeometry].
    pub fn total_sectors(&self) -> usize {
        (self.c as usize) * (self.h as usize) * (self.s as usize)
    }
    /// Return a boolean indicating whether this [DriveGeometry] contains the specified [DiskChs]
    /// representing a sector id.
    pub fn contains(&self, chs: impl Into<DiskChs>) -> bool {
        let chs = chs.into();
        self.c > chs.c && self.h > chs.h && self.s > (chs.s.saturating_sub(self.s_off))
    }

    pub fn chs_iter(&self) -> DiskChsIterator {
        DiskChs::new(self.c, self.h, self.s).iter(*self)
    }

    fn derive_matches(size: usize, sector_size: Option<usize>) -> Result<Vec<Self>, &'static str> {
        // Overall cylinder range is 39-85
        // We allow one less cylinder than normal, this is sometimes seen in ST files
        let cylinder_range = 39usize..=85;
        // Consider anything from 45-79 as an invalid cylinder range. Would indicate under-dumped image.
        let invalid_cylinders = 45usize..79;
        let sector_size = sector_size.unwrap_or(DEFAULT_SECTOR_SIZE);
        let total_sectors = size / sector_size;
        if size % sector_size != 0 {
            return Err("Raw size must be multiple of sector size");
        }

        //let mut layout_match = None;
        let mut layout_matches = Vec::with_capacity(2);

        for spt in 8..=18 {
            // Iterate over possible sectors per track
            if total_sectors % spt != 0 {
                continue; // Skip if total_sectors is not divisible by spt
            }

            let total_tracks = total_sectors / spt; // Calculate total tracks

            // Determine the number of heads (1 or 2) and corresponding track count
            let heads = if total_tracks % 2 == 0 { 2 } else { 1 };

            let tracks = total_tracks / heads;
            if cylinder_range.contains(&tracks) && !invalid_cylinders.contains(&tracks) {
                layout_matches.push(DriveGeometry {
                    c: tracks as u16,
                    h: heads as u8,
                    s: spt as u8,
                    s_off: 0,
                    size: sector_size,
                });
            }
        }

        if !layout_matches.is_empty() {
            layout_matches
                .sort_by(|a, b| Self::normal_cylinder_distance(a.c).cmp(&Self::normal_cylinder_distance(b.c)));

            let vec = layout_matches.iter().flat_map(|layout| layout.equivalents()).collect();
            Ok(vec)
        }
        else {
            Err("No match for raw image size")
        }
    }

    fn normal_cylinder_distance(c: u16) -> u16 {
        if c < 60 {
            40i16.abs_diff(c as i16)
        }
        else {
            80i16.abs_diff(c as i16)
        }
    }

    fn equivalents(&self) -> Vec<Self> {
        let mut equivalents = Vec::with_capacity(2);
        let mut layout = *self;
        // Add the original layout
        equivalents.push(layout);

        // If the track count is >= 79, we could have either a double-sided 5.25" disk or a
        // single sided 3.5" disk. We can't determine which from the raw size alone.
        if layout.c >= 79 && layout.c % 2 == 0 && layout.h == 1 {
            layout.c /= 2;
            layout.h = 2;
            equivalents.push(layout);
        }
        else if layout.c <= 45 && layout.h == 2 {
            // Otherwise, if the track count is small enough to be a 48TPI 5.25" disk with two
            // sides, it might also be a 96tpi 3.5" disk with one side.
            layout.c *= 2;
            layout.h = 1;
            equivalents.push(layout);
        }
        equivalents
    }
}

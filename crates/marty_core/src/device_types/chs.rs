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

//! Define a [DiskChs] that represents cylinder, head, sector addressing.
//! This is only used for hard drives - for floppy disks we use fluxfox's
//! DiskChs, from which this is copied.

use crate::device_types::geometry::DriveGeometry;
use std::fmt::Display;

/// A structure representing a cylinder, head, sector address
///  - Cylinder (c)
///  - Head (h)
///  - Sector ID (s)
///
/// A DiskChs may represent a Sector ID, where size is ignored, or an overall disk geometry.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct DiskChs {
    pub(crate) c: u16,
    pub(crate) h: u8,
    pub(crate) s: u8,
}

impl Default for DiskChs {
    fn default() -> Self {
        Self { c: 0, h: 0, s: 1 }
    }
}

impl From<(u16, u8, u8)> for DiskChs {
    fn from((c, h, s): (u16, u8, u8)) -> Self {
        Self { c, h, s }
    }
}

impl From<DiskChs> for (u16, u8, u8) {
    fn from(chs: DiskChs) -> Self {
        (chs.c, chs.h, chs.s)
    }
}

impl Display for DiskChs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[c:{:2} h:{} s:{:3}]", self.c, self.h, self.s)
    }
}

impl DiskChs {
    /// Create a new `DiskChs` structure from cylinder, head and sector id components.
    pub fn new(c: u16, h: u8, s: u8) -> Self {
        Self { c, h, s }
    }
    /// Return the cylinder, head and sector id components in a tuple.
    #[inline]
    pub fn get(&self) -> (u16, u8, u8) {
        (self.c, self.h, self.s)
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
    /// Return the sector id (s) field.
    #[inline]
    pub fn s(&self) -> u8 {
        self.s
    }
    /// Set the three components of a `DiskChs`
    pub fn set(&mut self, c: u16, h: u8, s: u8) {
        self.c = c;
        self.h = h;
        self.s = s;
    }
    /// Set the cylinder (c) component of a `DiskChs`
    #[inline]
    pub fn set_c(&mut self, c: u16) {
        self.c = c;
    }
    /// Set the head (h) component of a `DiskChs`
    #[inline]
    pub fn set_h(&mut self, h: u8) {
        self.h = h;
    }
    /// Set the sector ID (s) component of a `DiskChs`
    #[inline]
    pub fn set_s(&mut self, s: u8) {
        self.s = s;
    }

    /// Seek to the specified CHS.
    /// This function is deprecated. Seeking cannot be performed directly on a `DiskChs` structure,
    /// as sector IDs are not always sequential.
    #[deprecated]
    #[allow(deprecated)]
    pub fn seek(&mut self, c: u16, h: u8, s: u8) {
        self.seek_to(&DiskChs::from((c, h, s)));
    }

    /// Seek to the specified CHS.
    /// This function is deprecated. Seeking cannot be performed directly on a `DiskChs` structure,
    /// as sector IDs are not always sequential.
    #[deprecated]
    pub fn seek_to(&mut self, dst_chs: &DiskChs) {
        self.c = dst_chs.c;
        self.h = dst_chs.h;
        self.s = dst_chs.s;
    }

    /// Return the number of sectors represented by a DiskChs structure, interpreted as drive geometry.
    pub fn sector_count(&self) -> u32 {
        (self.c as u32) * (self.h as u32) * (self.s as u32)
    }

    /// Return the number of sectors represented by a DiskChs structure, interpreted as drive geometry.
    pub fn total_sectors(&self) -> usize {
        (self.c as usize) * (self.h as usize) * (self.s as usize)
    }

    /// Return a boolean indicating whether this `DiskChs`, interpreted as drive geometry, contains
    /// the specified `DiskChs` representing a sector.
    pub fn contains(&self, other: impl Into<DiskChs>) -> bool {
        let other = other.into();
        self.c > other.c && self.h > other.h && self.s >= other.s
    }

    /// Convert a [DiskChs] struct to an LBA sector address.
    /// A reference [SectorLayout] is required to calculate the address.
    /// Only valid for standard disk formats.
    pub fn to_lba(&self, geom: &DriveGeometry) -> usize {
        let hpc = geom.h() as usize;
        let spt = geom.s() as usize;
        (self.c as usize * hpc + (self.h as usize)) * spt + (self.s.saturating_sub(geom.s_off) as usize)
    }

    /// Convert an LBA sector address into a [DiskChs] struct and byte offset into the resulting sector.
    /// A reference drive geometry is required to calculate the address.
    /// Only valid for standard disk formats.
    /// # Arguments:
    /// * `lba` - The LBA sector address to convert.
    /// * `geom` - A [SectorLayout], representing the number of heads and cylinders on the disk.
    /// # Returns:
    /// * `Some(DiskChs)` representing the resulting CHS address.
    /// * `None` if the LBA address is invalid for the specified geometry.
    pub fn from_lba(lba: usize, geom: &DriveGeometry) -> Option<DiskChs> {
        let hpc = geom.h() as usize;
        let spt = geom.s() as usize;
        let c = lba / (hpc * spt);
        let h = (lba / spt) % hpc;
        let s = (lba % spt) + geom.s_off as usize;

        if c >= geom.c() as usize || h >= hpc || s > spt {
            return None;
        }
        Some(DiskChs::from((c as u16, h as u8, s as u8)))
    }

    /// Convert a raw byte offset into a `DiskChs` struct and byte offset into the resulting sector.
    /// A reference standard disk geometry is required to calculate the address.
    /// Only valid for standard disk formats. This function is intended to assist seeking within a raw sector view.
    /// # Arguments:
    /// * `lba` - The LBA sector address to convert.
    /// * `lba` - The LBA sector address to convert.
    /// * `geom` - A [SectorLayout], representing the number of heads and cylinders on the disk.
    /// # Returns:
    /// A tuple containing the resulting `DiskChs` and the byte offset into the sector.
    pub fn from_raw_offset(offset: usize, geom: &DriveGeometry) -> Option<(DiskChs, usize)> {
        let lba = offset / geom.size();
        DiskChs::from_lba(lba, geom).map(|chs| (chs, offset % geom.size()))
    }

    /// Convert a `DiskChs` into a raw byte offset
    /// A reference drive geometry is required to calculate the address.
    /// Only valid for standard disk formats. This function is intended to assist seeking within a raw sector view.
    /// # Arguments:
    /// * `lba` - The LBA sector address to convert.
    /// * `geom` - A [SectorLayout], representing the number of heads and cylinders on the disk.
    /// # Returns:
    /// A tuple containing the resulting `DiskChs` and the byte offset into the sector.
    pub fn to_raw_offset(&self, geom: &DriveGeometry) -> Option<usize> {
        geom.contains(*self).then_some(self.to_lba(geom) * geom.size())
    }

    /// Return a new `DiskChs` that is the next sector on the disk, according to the specified
    /// geometry.
    /// Returns None if the current `DiskChs` represents the last sector of the specified geometry.
    /// This function should only be used for iterating through sectors in a standard disk format.
    /// It will not work correctly for non-standard disk formats.
    /// # Arguments:
    /// * `geom` - A [SectorLayout], representing the number of heads and cylinders on the disk.
    pub fn next_sector(&self, geom: &DriveGeometry) -> Option<DiskChs> {
        if self.s < (geom.s() - 1 + geom.s_off) {
            // println!(
            //     "Geometry: {} current sector: {}, spt: {}, last valid sector:{} Next sector: {}",
            //     geom,
            //     self.s,
            //     geom.s(),
            //     geom.s() - 1 + geom.s_off,
            //     self.s + 1
            // );

            // Not at last sector, just return next sector
            Some(DiskChs::from((self.c, self.h, self.s + 1)))
        }
        else if self.h < geom.h().saturating_sub(1) {
            // At last sector, but not at last head, go to next head, same cylinder, sector 1
            Some(DiskChs::from((self.c, self.h + 1, geom.s_off)))
        }
        else if self.c < geom.c().saturating_sub(1) {
            // At last sector and last head, go to next cylinder, head 0, sector (s_off)
            Some(DiskChs::from((self.c + 1, 0, geom.s_off)))
        }
        else {
            // At end of disk.
            None
        }
    }

    /// Return a new `Option<DiskChs>` that is `sectors` number of sectors advanced from the current
    /// `DiskChs`, according to a provided geometry.
    /// Returns None if advanced past the end of the disk.
    /// # Arguments:
    /// * `geom` - A [SectorLayout], representing the number of heads and cylinders on the disk.
    pub fn offset_sectors(&mut self, sectors: u32, geom: &DriveGeometry) -> Option<DiskChs> {
        let mut start_chs = *self;
        for _ in 0..sectors {
            start_chs = start_chs.next_sector(geom)?;
        }
        Some(start_chs)
    }

    /// Return a `DiskChsIterator` that will iterate through all sectors in order, interpreting the `DiskChs` as a standard disk geometry.
    /// This should only be used for standard disk formats. It will skip non-standard sectors, and may access sectors out of physical order.
    pub fn iter(&self, geom: DriveGeometry) -> DiskChsIterator {
        DiskChsIterator { geom, chs: None }
    }
}

pub struct DiskChsIterator {
    geom: DriveGeometry,
    chs:  Option<DiskChs>,
}

impl Iterator for DiskChsIterator {
    type Item = DiskChs;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(chs) = &mut self.chs {
            *chs = chs.next_sector(&self.geom)?;
        }
        else {
            self.chs = Some(DiskChs::new(0, 0, self.geom.s_off));
        }
        self.chs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diskchs_iter_works() {
        let geom = DriveGeometry::new(1024, 5, 26, 1, 512);
        let total_sectors = geom.total_sectors();

        let first_chs = geom.chs_iter().next().unwrap();
        assert_eq!(first_chs, DiskChs::new(0, 0, 1));

        let last_chs = geom.chs_iter().last().unwrap();
        println!("Last CHS: {}", last_chs);
        assert_eq!(last_chs, DiskChs::new(geom.c() - 1, geom.h() - 1, geom.s()));

        let iter_ct = geom.chs_iter().count();
        assert_eq!(iter_ct, total_sectors);
    }
}

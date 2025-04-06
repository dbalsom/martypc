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

//! A generic representation of a disk drive.

use crate::{
    device_types::{chs::DiskChs, geometry::DriveGeometry},
    vhd::VirtualHardDisk,
};
use core::fmt;
use std::fmt::Debug;

pub struct Disk {
    position: DiskChs,
    geometry: DriveGeometry,
    vhd: Option<VirtualHardDisk>,
}

impl Debug for Disk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HardDisk")
            .field("position", &self.position)
            .field("geometry", &self.geometry)
            .finish()
    }
}

impl Disk {
    pub fn new(geometry: DriveGeometry) -> Self {
        Self {
            position: DiskChs::new(0, 0, 1),
            geometry,
            vhd: None,
        }
    }

    pub fn from_vhd(vhd: VirtualHardDisk) -> Self {
        let geometry = vhd.geometry().into();
        Self {
            position: DiskChs::new(0, 0, 1),
            geometry,
            vhd: Some(vhd),
        }
    }

    pub fn set_vhd(&mut self, vhd: VirtualHardDisk) {
        self.geometry = vhd.geometry().into();
        self.vhd = Some(vhd);
    }

    pub fn vhd(&self) -> Option<&VirtualHardDisk> {
        self.vhd.as_ref()
    }

    pub fn vhd_mut(&mut self) -> Option<&mut VirtualHardDisk> {
        self.vhd.as_mut()
    }

    pub fn geometry(&self) -> DriveGeometry {
        self.geometry
    }

    pub fn set_geometry(&mut self, geometry: DriveGeometry) {
        self.geometry = geometry;
    }

    pub fn position(&self) -> DiskChs {
        self.position
    }

    /// Returns the current position in VHD format CHS (0-indexed)
    pub fn position_vhd(&self) -> DiskChs {
        DiskChs::new(self.position.c, self.position.h, self.position.s.saturating_sub(1))
    }

    pub fn seek(&mut self, chs: DiskChs) {
        if self.geometry.contains(chs) {
            log::debug!("Disk::seek(): Seeking to CHS: {}", chs);
            self.position = chs;
        }
        else {
            log::error!(
                "Disk::seek(): Attempted to seek to invalid CHS: {} for geometry: {}",
                chs,
                self.geometry
            );
        }
    }

    pub fn next_sector(&mut self) -> Option<DiskChs> {
        self.position.next_sector(&self.geometry)
    }
}

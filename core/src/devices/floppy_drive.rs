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

    devices::implementations::floppy_drive.rs

    Implements a floppy drive
*/

use crate::{
    device_types::{chs::DiskChs, fdc::DISK_FORMATS},
    devices::fdc::SECTOR_SIZE,
};
use anyhow::{anyhow, Error};

pub struct FloppyDiskDrive {
    pub(crate) error_signal: bool,

    pub(crate) chs: DiskChs,
    media_geom: DiskChs,
    drive_geom: DiskChs,

    pub(crate) max_cylinders: u8,
    pub(crate) max_heads: u8,
    pub(crate) max_sectors: u8,
    pub(crate) ready: bool,
    pub(crate) motor_on: bool,
    pub(crate) positioning: bool,
    pub(crate) have_disk: bool,
    pub(crate) write_protected: bool,
    pub(crate) disk_image: Vec<u8>,
}

impl Default for FloppyDiskDrive {
    fn default() -> Self {
        Self {
            error_signal: false,
            chs: Default::default(),
            media_geom: Default::default(),
            drive_geom: Default::default(),
            max_cylinders: 0,
            max_heads: 0,
            max_sectors: 0,
            ready: false,
            motor_on: false,
            positioning: false,
            have_disk: false,
            write_protected: true,
            disk_image: Vec::new(),
        }
    }
}
impl FloppyDiskDrive {
    pub fn new() -> Self {
        Default::default()
    }

    /// Reset the drive to default state. Like other device patterns we use default after preserving persistent state.
    /// Called when FDC itself is reset.
    pub fn reset(&mut self) {
        // Preserve the disk image before defaulting the drive
        let image = std::mem::replace(&mut self.disk_image, Vec::new());

        *self = Self {
            ready: self.have_disk,
            have_disk: self.have_disk,
            write_protected: self.write_protected,
            max_cylinders: self.max_cylinders,
            max_heads: self.max_heads,
            max_sectors: self.max_sectors,
            motor_on: false,
            positioning: false,
            disk_image: image,
            ..Default::default()
        };
    }

    /// Load a disk into the specified drive
    pub fn load_image_from(&mut self, src_vec: Vec<u8>) -> Result<(), Error> {
        let image_len: usize = src_vec.len();

        // Disk images must contain whole sectors
        if image_len % SECTOR_SIZE > 0 {
            return Err(anyhow!("Invalid image length"));
        }

        // Look up disk parameters based on image size
        if let Some(fmt) = DISK_FORMATS.get(&image_len) {
            self.max_cylinders = fmt.chs.c();
            self.max_heads = fmt.chs.h();
            self.max_sectors = fmt.chs.s();
        }
        else {
            // No image format found.
            if image_len < 163_840 {
                // If image is smaller than single sided disk, assume single sided disk, 8 sectors per track
                // This is useful for loading things like boot sector images without having to copy them to
                // a full disk image
                self.max_cylinders = 40;
                self.max_heads = 1;
                self.max_sectors = 8;
            }
            else {
                return Err(anyhow!("Invalid image length"));
            }
        }

        self.have_disk = true;
        self.disk_image = src_vec;

        log::debug!(
            "Loaded floppy image, size: {} c: {} h: {} s: {}",
            self.disk_image.len(),
            self.max_cylinders,
            self.max_heads,
            self.max_sectors
        );

        Ok(())
    }
}

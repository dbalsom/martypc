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
    device_types::{
        chs::DiskChs,
        fdc::{DISK_FORMATS, DRIVE_CAPABILITIES},
    },
    devices::fdc::SECTOR_SIZE,
    machine_types::FloppyDriveType,
};
use anyhow::{anyhow, Error};

pub enum FloppyDriveMechanicalState {
    MotorOff,
    MotorSpinningUp,
    MotorOnIdle,
    MotorSpinningDown,
    HeadSeeking,
}

pub struct FloppyDiskDrive {
    drive_type: FloppyDriveType,
    drive_n: usize,
    pub(crate) error_signal: bool,

    pub(crate) chs: DiskChs,
    drive_geom: DiskChs,
    pub(crate) media_geom: DiskChs,

    pub(crate) ready: bool,
    pub(crate) motor_on: bool,
    pub(crate) positioning: bool,
    pub(crate) disk_present: bool,
    pub(crate) write_protected: bool,
    pub(crate) disk_image: Vec<u8>,
}

impl Default for FloppyDiskDrive {
    fn default() -> Self {
        Self {
            drive_type: Default::default(),
            drive_n: 0,
            error_signal: false,
            chs: Default::default(),
            drive_geom: Default::default(),
            media_geom: Default::default(),
            ready: false,
            motor_on: false,
            positioning: false,
            disk_present: false,
            write_protected: true,
            disk_image: Vec::new(),
        }
    }
}
impl FloppyDiskDrive {
    pub fn new(drive_n: usize, drive_type: FloppyDriveType) -> Self {
        // Should be safe to unwrap as we are limited by valid drive type enums.
        let drive_geom = DRIVE_CAPABILITIES.get(&drive_type).unwrap().chs;

        FloppyDiskDrive {
            drive_type,
            drive_n,
            drive_geom,
            ..Default::default()
        }
    }

    /// Reset the drive to default state. Like other device patterns we use default after preserving persistent state.
    /// Called when FDC itself is reset.
    pub fn reset(&mut self) {
        // Preserve the disk image before defaulting the drive
        let image = std::mem::replace(&mut self.disk_image, Vec::new());

        *self = Self {
            drive_type: self.drive_type,
            drive_n: self.drive_n,
            ready: self.disk_present,
            disk_present: self.disk_present,
            write_protected: self.write_protected,
            media_geom: self.media_geom,
            drive_geom: self.drive_geom,
            motor_on: false,
            positioning: false,
            disk_image: image,
            ..Default::default()
        };
    }

    /// Load a disk into the specified drive
    pub fn load_image_from(&mut self, src_vec: Vec<u8>, write_protect: bool) -> Result<(), Error> {
        let image_len: usize = src_vec.len();

        // Disk images must contain whole sectors
        if image_len % SECTOR_SIZE > 0 {
            return Err(anyhow!("Invalid image length"));
        }

        // Look up disk parameters based on image size
        if let Some(fmt) = DISK_FORMATS.get(&image_len) {
            self.media_geom = fmt.chs;
        }
        else {
            // No image format found.
            if image_len < 163_840 {
                // If image is smaller than single sided disk, assume single sided disk, 8 sectors per track
                // This is useful for loading things like boot sector images without having to copy them to
                // a full disk image

                self.media_geom = DiskChs::new(40, 1, 8);
            }
            else {
                return Err(anyhow!("Invalid image length"));
            }
        }

        self.disk_present = true;
        self.disk_image = src_vec;
        self.write_protected = write_protect;

        log::debug!(
            "Loaded floppy image, size: {} chs: {}",
            self.disk_image.len(),
            self.media_geom,
        );

        Ok(())
    }

    /// Unload (eject) the disk in the specified drive
    pub fn unload_image(&mut self) {
        self.chs.set_c(0);
        self.chs.set_h(0);
        self.chs.set_s(1);

        self.media_geom = DiskChs::default();
        self.disk_present = false;
        self.disk_image.clear();
    }

    pub fn motor_on(&mut self) {
        if self.disk_present {
            self.motor_on = true;
            self.ready = true;
        }
    }

    pub fn motor_off(&mut self) {
        if self.motor_on {
            log::trace!("Drive {}: turning motor off.", self.drive_n);
        }
        self.motor_on = false;
    }

    /// Return whether the specified chs is valid for the disk in the drive.
    /// Note this is different from checking if the id is valid for a seek, for which there is a
    /// separate function. We can seek a bit beyond the end of a disk, as well as seek with no
    /// disk in the drive.
    pub fn is_id_valid(&self, chs: DiskChs) -> bool {
        if chs.c() >= self.media_geom.c() {
            log::warn!("is_id_valid: c {} >= media_geom.c {}", chs.c(), self.media_geom.c());
            return false;
        }
        if chs.h() >= self.media_geom.h() {
            log::warn!("is_id_valid: h {} >= media_geom.h {}", chs.h(), self.media_geom.h());
            return false;
        }
        if chs.s() > self.media_geom.s() {
            // Note sectors are 1 based, so we can seek to the last sector
            log::warn!("is_id_valid: s {} > media_geom.s {}", chs.s(), self.media_geom.s());
            return false;
        }
        true
    }

    /// Return whether the drive is physically capable of seeking to the specified chs.
    pub fn is_seek_valid(&self, chs: DiskChs) -> bool {
        if chs.c() >= self.drive_geom.c() {
            return false;
        }
        if chs.h() >= self.drive_geom.h() {
            return false;
        }
        if chs.s() > self.drive_geom.s() {
            // Note sectors are 1 based, so we can seek to the last sector
            return false;
        }
        true
    }

    pub fn seek(&mut self, chs: DiskChs) {
        if !self.is_seek_valid(chs) {
            return;
        }
        self.chs.seek_to(&chs);
    }

    pub fn get_next_sector(&self, chs: DiskChs) -> DiskChs {
        if chs.s < self.media_geom.s {
            // Not at last sector, just return next sector
            DiskChs::from((chs.c, chs.h, chs.s + 1))
        }
        else if chs.h < self.media_geom.h - 1 {
            // TODO: should this be media heads or drive heads?
            // At last sector, but not at last head, go to next head, same cylinder, sector 1
            DiskChs::from((chs.c, chs.h + 1, 1))
        }
        else if chs.h < self.media_geom.c - 1 {
            // At last sector and last head, go to next cylinder, head 0, sector 1
            DiskChs::from((chs.c + 1, 0, 1))
        }
        else {
            // Return end of drive? What does this do on real hardware
            DiskChs::from((self.media_geom.c, 0, 1))
        }
    }

    pub fn get_chs_sector_offset(&self, sector_offset: u32, chs: DiskChs) -> DiskChs {
        let mut new_chs = chs;
        for _ in 0..sector_offset {
            new_chs = self.get_next_sector(new_chs);
        }

        new_chs
    }

    pub fn get_image_address(&self, chs: DiskChs) -> usize {
        if chs.s == 0 {
            log::warn!("Invalid sector == 0");
            return 0;
        }
        let hpc = self.media_geom.h as usize;
        let spt = self.media_geom.s as usize;
        let lba: usize = (chs.c as usize * hpc + (chs.h as usize)) * spt + (chs.s as usize - 1);
        lba * SECTOR_SIZE
    }

    pub fn disk_present(&self) -> bool {
        self.disk_present
    }
}

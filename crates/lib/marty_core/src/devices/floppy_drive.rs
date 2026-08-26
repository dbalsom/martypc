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

    devices::implementations::floppy_drive.rs

    Implements a floppy drive
*/

use std::{
    fmt,
    io::Cursor,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use crate::{
    device_types::fdc::{FloppyDataRate, FloppyImageType, ImageInsertionPolicy, DRIVE_CAPABILITIES},
    machine_types::FloppyDriveType,
};
use anyhow::{anyhow, Error};
use fluxfox::{file_system::FileSystemType, prelude::*, types::ReadSectorResult, DiskSectorMap};

#[allow(unused)]
macro_rules! read_lock {
    ($arc_lock:expr) => {{
        match $arc_lock.try_read() {
            Ok(guard) => guard,
            Err(_) => anyhow::bail!("Failed to acquire read lock"),
        }
    }};
}

macro_rules! read_lock_opt {
    ($arc_lock:expr) => {{
        match $arc_lock.try_read() {
            Ok(guard) => guard,
            Err(_) => return None,
        }
    }};
}

macro_rules! write_lock {
    ($arc_lock:expr) => {{
        match $arc_lock.try_write() {
            Ok(guard) => guard,
            Err(_) => anyhow::bail!("Failed to acquire write lock"),
        }
    }};
}

#[derive(Copy, Clone, Debug, Default)]
pub enum FloppyDriveOperation {
    #[default]
    NoOperation,
    ReadData,
    WriteData,
}

pub enum FloppyDriveMechanicalState {
    MotorOff,
    MotorSpinningUp,
    MotorOnIdle,
    MotorSpinningDown,
    HeadSeeking,
}

#[derive(Copy, Clone, Default)]
pub struct OperationStatus {
    pub(crate) op_type: FloppyDriveOperation,
    pub(crate) sector_not_found: bool,
    pub(crate) address_crc_error: bool,
    pub(crate) data_crc_error: bool,
    pub(crate) deleted_mark: bool,
    pub(crate) no_dam: bool,
    pub(crate) wrong_cylinder: bool,
    pub(crate) wrong_head: bool,
}

impl OperationStatus {
    pub fn reset(&mut self, op_type: FloppyDriveOperation) {
        *self = Self {
            op_type,
            ..Default::default()
        };
    }
}

pub struct DriveSectorReadResult {
    pub(crate) not_found: bool,
    pub(crate) data: Vec<u8>,
    pub(crate) status: OperationStatus,
}

pub struct DriveTrackReadResult {
    pub(crate) not_found: bool,
    pub(crate) sectors_read: u16,
    pub(crate) data: Vec<u8>,
    pub(crate) status: OperationStatus,
}

pub struct DriveSectorWriteResult {
    pub(crate) not_found: bool,
    pub(crate) status:    OperationStatus,
}

pub struct DriveFormatResult {
    pub(crate) sectors_formatted: u8,
    pub(crate) new_sid: u8,
}

fn sector_not_found(read_result: &ReadSectorResult) -> bool {
    read_result.not_found && read_result.id_chsn.is_none()
}

pub struct FloppyImageState {
    pub format: Option<StandardFormat>,
    pub heads: u8,
    pub sector_map: DiskSectorMap,
}

impl FloppyImageState {
    pub fn get_head_ct(&self) -> usize {
        self.sector_map.len()
    }
    pub fn get_track_ct(&self, head: usize) -> usize {
        self.sector_map.get(head).map_or(0, |tracks| tracks.len())
    }
    pub fn get_sector_ct(&self, head: usize, track: usize) -> usize {
        self.sector_map
            .get(head)
            .and_then(|tracks| tracks.get(track))
            .map_or(0, |sectors| sectors.len())
    }
}

#[derive(Clone, Debug)]
pub enum FloppyMediaIncompatibility {
    GeometryExceedsDrive {
        media_cylinders: u16,
        media_heads: u8,
        drive_cylinders: u16,
        drive_heads: u8,
    },
    UnsupportedDataRate {
        data_rate:  FloppyDataRate,
        drive_type: FloppyDriveType,
    },
    UnsupportedFormat {
        format: Option<StandardFormat>,
        drive_type: FloppyDriveType,
    },
}

impl fmt::Display for FloppyMediaIncompatibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GeometryExceedsDrive {
                media_cylinders,
                media_heads,
                drive_cylinders,
                drive_heads,
            } if media_cylinders > drive_cylinders && media_heads <= drive_heads => write!(
                f,
                "image has {} cylinders; drive supports {}",
                media_cylinders, drive_cylinders
            ),
            Self::GeometryExceedsDrive {
                media_cylinders,
                media_heads,
                drive_cylinders,
                drive_heads,
            } if media_cylinders <= drive_cylinders && media_heads > drive_heads => {
                write!(f, "image has {} heads; drive supports {}", media_heads, drive_heads)
            }
            Self::GeometryExceedsDrive {
                media_cylinders,
                media_heads,
                drive_cylinders,
                drive_heads,
            } => write!(
                f,
                "image geometry {}c/{}h; drive supports {}c/{}h",
                media_cylinders, media_heads, drive_cylinders, drive_heads
            ),
            Self::UnsupportedDataRate { data_rate, drive_type } => {
                write!(
                    f,
                    "disk image data rate {} is not supported by a {} drive",
                    data_rate, drive_type
                )
            }
            Self::UnsupportedFormat { format, drive_type } => match format {
                Some(format) => write!(
                    f,
                    "disk image format {} is not supported by a {} drive",
                    format, drive_type
                ),
                None => write!(
                    f,
                    "disk image does not match a standard format supported by a {} drive",
                    drive_type
                ),
            },
        }
    }
}

impl std::error::Error for FloppyMediaIncompatibility {}

pub struct FloppyDiskDrive {
    drive_type: FloppyDriveType,
    drive_n: usize,
    pub(crate) error_signal: bool,

    cylinder: u16,
    pub(crate) chsn: DiskChsn,
    drive_geom: DiskChs,
    pub(crate) media_geom: DiskChs,

    pub(crate) ready: bool,
    motor_on: bool,
    pub(crate) disk_present: bool,
    pub(crate) write_protected: bool,
    pub(crate) disk_image: Option<Arc<RwLock<DiskImage>>>,

    operation_status: OperationStatus,
    /// Candidate image formats for autofloppy creation, ordered by capacity. This allows
    /// autofloppy to select an appropriate image size when mounting a directory.
    pub(crate) autofloppy_formats: Vec<FloppyImageType>,

    ref_write: u64,
}

impl Default for FloppyDiskDrive {
    fn default() -> Self {
        Self {
            drive_type: Default::default(),
            drive_n: 0,
            error_signal: false,
            cylinder: 0,
            chsn: Default::default(),
            drive_geom: Default::default(),
            media_geom: Default::default(),
            ready: false,
            motor_on: false,
            disk_present: false,
            write_protected: true,
            disk_image: None,

            operation_status: Default::default(),

            autofloppy_formats: Vec::new(),

            ref_write: 0,
        }
    }
}
impl FloppyDiskDrive {
    pub fn new(drive_n: usize, drive_type: FloppyDriveType) -> Self {
        // Should be safe to unwrap as we are limited by valid drive type enums.
        let drive_geom = DRIVE_CAPABILITIES.get(&drive_type).unwrap().chs;

        let autofloppy_formats = match drive_type {
            FloppyDriveType::Floppy360K => vec![FloppyImageType::Image360K],
            FloppyDriveType::Floppy720K => vec![FloppyImageType::Image720K],
            FloppyDriveType::Floppy12M => vec![FloppyImageType::Image360K, FloppyImageType::Image12M],
            FloppyDriveType::Floppy144M => vec![FloppyImageType::Image720K, FloppyImageType::Image144M],
        };

        FloppyDiskDrive {
            drive_type,
            drive_n,
            drive_geom,
            autofloppy_formats,
            ..Default::default()
        }
    }

    /// Reset the drive to default state. Like other device patterns we use default after preserving persistent state.
    /// Called when FDC itself is reset.
    pub fn reset(&mut self) {
        // Preserve the disk image before defaulting the drive
        let image = self.disk_image.take();
        // The motor-enable line is controlled externally by the FDC's DOR.
        // Its state is preserved here; callers that need it off must use
        // FloppyController::motor_off() before resetting the drive.
        let motor_on = self.motor_on;

        *self = Self {
            drive_type: self.drive_type,
            drive_n: self.drive_n,
            // IBM DOS wants to see ready in ST3 even if no disk in drive.
            ready: true,
            disk_present: self.disk_present,
            write_protected: self.write_protected,
            media_geom: self.media_geom,
            drive_geom: self.drive_geom,
            motor_on,
            disk_image: image,
            autofloppy_formats: self.autofloppy_formats.clone(),
            ..Default::default()
        };
    }

    pub fn get_largest_supported_image_format(&self) -> FloppyImageType {
        self.autofloppy_formats[self.autofloppy_formats.len().saturating_sub(1)]
    }

    pub fn get_type(&self) -> FloppyDriveType {
        self.drive_type
    }

    /// Check that the requested [DiskImage] can be inserted into this drive.
    pub fn validate_image_compatibility(
        &self,
        image: &DiskImage,
        policy: ImageInsertionPolicy,
    ) -> Result<(), FloppyMediaIncompatibility> {
        let image_format = image.image_format();
        let media_cylinders = image_format.geometry.c();
        let media_heads = image_format.geometry.h();

        // If the image requires more heads or cylinders than the drive supports, fail.
        if media_cylinders > self.drive_geom.c() || media_heads > self.drive_geom.h() {
            return Err(FloppyMediaIncompatibility::GeometryExceedsDrive {
                media_cylinders,
                media_heads,
                drive_cylinders: self.drive_geom.c(),
                drive_heads: self.drive_geom.h(),
            });
        }

        let data_rate = FloppyDataRate::from(image_format.data_rate);
        let drive_capabilities = DRIVE_CAPABILITIES
            .get(&self.drive_type)
            .expect("all floppy drive types must define capabilities");
        if !drive_capabilities.data_rates.contains(&data_rate) {
            return Err(FloppyMediaIncompatibility::UnsupportedDataRate {
                data_rate,
                drive_type: self.drive_type,
            });
        }

        // Additionally match physical media type if policy is set to Strict
        if matches!(policy, ImageInsertionPolicy::Strict) {
            let format = image.closest_format(false);
            let format_supported = format
                .as_ref()
                .is_some_and(|format| self.drive_type.get_compatible_formats().contains(format));

            if !format_supported {
                return Err(FloppyMediaIncompatibility::UnsupportedFormat {
                    format,
                    drive_type: self.drive_type,
                });
            }
        }

        Ok(())
    }

    /// Load a disk into the specified drive
    pub fn load_image_from(
        &mut self,
        src_vec: Vec<u8>,
        path: Option<&Path>,
        write_protect: bool,
        policy: ImageInsertionPolicy,
    ) -> Result<Arc<RwLock<DiskImage>>, Error> {
        let mut image_buffer = Cursor::new(src_vec);
        let image = DiskImage::load(&mut image_buffer, path, None, None)?;
        self.attach_image(image, path.map(Path::to_path_buf), write_protect, policy)
    }

    pub fn attach_image(
        &mut self,
        image: DiskImage,
        _path: Option<PathBuf>,
        write_protect: bool,
        policy: ImageInsertionPolicy,
    ) -> Result<Arc<RwLock<DiskImage>>, Error> {
        self.validate_image_compatibility(&image, policy)?;

        self.media_geom = DiskChs::from((
            image.image_format().geometry.c(),
            image.image_format().geometry.h(),
            0u8,
        ));

        log::debug!("Attached floppy image, CHS: {}", self.media_geom);
        self.disk_present = true;
        self.write_protected = write_protect;
        let image_arc = image.into_arc();
        let image_clone = image_arc.clone();
        self.disk_image = Some(image_arc);

        Ok(image_clone)
    }

    pub fn get_image(&mut self) -> (Option<Arc<RwLock<DiskImage>>>, u64) {
        self.ref_write = self.disk_image.as_mut().map_or(0, |image| match image.try_read() {
            Ok(image) => image.write_ct(),
            Err(_) => 0,
        });
        (self.disk_image.clone(), self.ref_write)
    }

    /// Unload (eject) the disk in the specified drive
    pub fn unload_image(&mut self) {
        self.chsn = Default::default();
        self.media_geom = DiskChs::default();
        self.disk_present = false;
        self.disk_image = None;
    }

    pub fn create_new_image(
        &mut self,
        format: StandardFormat,
        formatted: bool,
    ) -> Result<Arc<RwLock<DiskImage>>, Error> {
        self.unload_image();

        let mut builder = ImageBuilder::new()
            .with_standard_format(format)
            .with_resolution(TrackDataResolution::BitStream)
            .with_creator_tag(b"MartyPC");

        if formatted {
            builder = builder.with_filesystem(FileSystemType::Fat12);
        }

        let image = builder.build()?;
        self.chsn = Default::default();
        self.media_geom = format.chs();
        self.disk_present = true;

        let image_arc = image.into_arc();
        let image_clone = image_arc.clone();
        self.disk_image = Some(image_arc);

        Ok(image_clone)
    }

    pub fn patch_image_bpb(&mut self, standard_format: StandardFormat) -> Result<(), Error> {
        if self.disk_image.is_none() {
            return Err(anyhow!("No media in drive"));
        }

        if let Some(image_lock) = &self.disk_image {
            match image_lock.try_write() {
                Ok(mut image) => {
                    image.update_standard_boot_sector(standard_format)?;
                    Ok(())
                }
                Err(_) => {
                    log::error!("patch_image_bpb(): failed to acquire write lock");
                    Err(anyhow!("Failed to acquire write lock"))
                }
            }
        }
        else {
            log::error!("patch_image_bpb(): no disk image");
            Err(anyhow!("No media in drive"))
        }
    }

    pub fn write_sector(
        &mut self,
        h: u8,
        id_chs: DiskChs,
        n: u8,
        sector_data: &[u8],
        deleted: bool,
    ) -> Result<DriveSectorWriteResult, Error> {
        if self.disk_image.is_none() {
            return Err(anyhow!("No media in drive"));
        }

        let chsn = DiskChsn::from((id_chs, n));
        let sector_data_size = chsn.n_size();
        if sector_data.len() != sector_data_size {
            return Err(anyhow!(
                "Data buffer size: {} does not match sector size: {}",
                sector_data.len(),
                sector_data_size
            ));
        }

        log::trace!(
            "write_sector(): phys_c: {} phys_h: {} id_chs: {} n: {} bytes: {}",
            self.cylinder,
            h,
            id_chs,
            n,
            sector_data.len()
        );

        let image_lock = self.disk_image.as_ref().unwrap();
        let mut image = write_lock!(image_lock);
        self.operation_status.reset(FloppyDriveOperation::WriteData);

        let write_sector_result = image.write_sector(
            DiskCh::new(self.cylinder, h),
            DiskChsnQuery::new(chsn.c(), chsn.h(), chsn.s(), n),
            None,
            sector_data,
            RwScope::DataOnly,
            deleted,
            false,
        )?;

        self.operation_status.sector_not_found = write_sector_result.not_found;
        self.operation_status.no_dam = write_sector_result.no_dam;
        self.operation_status.address_crc_error = write_sector_result.address_crc_error;
        self.operation_status.wrong_cylinder = write_sector_result.wrong_cylinder;
        self.operation_status.wrong_head = write_sector_result.wrong_head;

        if write_sector_result.not_found {
            log::warn!("write_sector(): sector not found: {}", id_chs);
        }
        else {
            log::debug!(
                "write_sector(): wrote sector: {} bytes, wrong cylinder: {}",
                sector_data_size,
                write_sector_result.wrong_cylinder
            );
        }

        Ok(DriveSectorWriteResult {
            not_found: write_sector_result.not_found,
            status:    self.operation_status,
        })
    }

    pub fn read_sector(&mut self, h: u8, id_chs: DiskChs, n: u8) -> Result<DriveSectorReadResult, Error> {
        if self.disk_image.is_none() {
            return Err(anyhow!("No media in drive"));
        }

        log::trace!(
            "read_sector(): phys_c: {} phys_h: {} id_chs: {} n: {}",
            self.cylinder,
            h,
            id_chs,
            n
        );

        let image_lock = self.disk_image.as_ref().unwrap();
        let mut image = write_lock!(image_lock);

        self.operation_status.reset(FloppyDriveOperation::ReadData);

        let read_sector_result = match image.read_sector(
            DiskCh::new(self.cylinder, h),
            DiskChsnQuery::new(id_chs.c(), id_chs.h(), id_chs.s(), n),
            None,
            None,
            RwScope::DataOnly,
            false,
        ) {
            Ok(result) => result,
            Err(DiskImageError::DataError) => {
                self.operation_status.sector_not_found = true;
                return Ok(DriveSectorReadResult {
                    not_found: true,
                    data: Vec::new(),
                    status: self.operation_status,
                });
            }
            Err(e) => return Err(e.into()),
        };

        let not_found = sector_not_found(&read_sector_result);
        self.operation_status.sector_not_found = not_found;
        self.operation_status.wrong_cylinder = read_sector_result.wrong_cylinder;
        self.operation_status.wrong_head = read_sector_result.wrong_head;
        self.operation_status.no_dam = read_sector_result.no_dam;
        self.operation_status.address_crc_error = read_sector_result.address_crc_error;
        self.operation_status.data_crc_error = read_sector_result.data_crc_error;
        self.operation_status.deleted_mark = read_sector_result.deleted_mark;

        let data = if not_found || read_sector_result.no_dam || read_sector_result.address_crc_error {
            Vec::new()
        }
        else {
            read_sector_result.read_buf[read_sector_result.data_range].to_vec()
        };

        Ok(DriveSectorReadResult {
            not_found,
            data,
            status: self.operation_status,
        })
    }

    pub fn read_track(&mut self, h: u8, id_ch: DiskCh, n: u8, eot: u8) -> Result<DriveTrackReadResult, Error> {
        if self.disk_image.is_none() {
            return Err(anyhow!("No media in drive"));
        }

        let image_lock = self.disk_image.as_ref().unwrap();
        let mut image = write_lock!(image_lock);

        self.operation_status.reset(FloppyDriveOperation::ReadData);

        let phys_ch = DiskCh::new(self.cylinder, h);
        let read_track_result = image.read_all_sectors(phys_ch, id_ch, n, eot)?;

        if read_track_result.not_found {
            log::debug!("read_track(): sector not found");
            self.operation_status.sector_not_found = true;
            return Ok(DriveTrackReadResult {
                not_found: true,
                sectors_read: 0,
                data: Vec::new(),
                status: self.operation_status,
            });
        }
        else {
            log::debug!(
                "read_track(): read {} sectors, {} bytes, address_crc_error: {}, data_crc_error: {}, deleted_mark: {}",
                read_track_result.sectors_read,
                read_track_result.read_buf.len(),
                read_track_result.address_crc_error,
                read_track_result.data_crc_error,
                read_track_result.deleted_mark
            );
        }

        self.operation_status.address_crc_error = read_track_result.address_crc_error;
        self.operation_status.data_crc_error = read_track_result.data_crc_error;
        self.operation_status.deleted_mark = read_track_result.deleted_mark;

        let read_len = read_track_result.read_len_bytes.min(read_track_result.read_buf.len());
        let mut data = read_track_result.read_buf;
        data.truncate(read_len);

        Ok(DriveTrackReadResult {
            not_found: false,
            sectors_read: read_track_result.sectors_read,
            data,
            status: self.operation_status,
        })
    }

    pub fn format_track(
        &mut self,
        ch: DiskCh,
        format_buffer: &[u8],
        gap3_len: u8,
        fill_byte: u8,
    ) -> Result<DriveFormatResult, Error> {
        if self.disk_image.is_none() {
            return Err(anyhow!("No media in drive"));
        }

        let image_lock = self.disk_image.as_ref().unwrap();
        let mut image = write_lock!(image_lock);

        let mut fox_format_buffer = Vec::new();
        for buf_entry in format_buffer.chunks_exact(4) {
            let c = buf_entry[0] as u16;
            let h = buf_entry[1];
            let s = buf_entry[2];
            let n = buf_entry[3];

            let chsn = DiskChsn::new(c, h, s, n);
            fox_format_buffer.push(chsn);
        }

        let sector_ct = fox_format_buffer.len();

        log::trace!("format_track(): formatting track: {}: {} sectors", ch, sector_ct);
        match image.format_track(ch, fox_format_buffer, &[fill_byte], gap3_len as usize) {
            Ok(_) => Ok(DriveFormatResult {
                sectors_formatted: sector_ct as u8,
                new_sid: (sector_ct + 1) as u8,
            }),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_operation_status(&self) -> OperationStatus {
        self.operation_status
    }

    pub fn motor_on(&mut self) {
        self.motor_on = true;
        self.ready = self.disk_present;
    }

    pub fn is_motor_on(&self) -> bool {
        self.motor_on
    }

    pub fn motor_off(&mut self) {
        if self.motor_on {
            log::trace!("Drive {}: turning motor off.", self.drive_n);
        }
        self.motor_on = false;
    }

    // Return whether the specified chs is valid for the disk in the drive.
    // Note this is different from checking if the id is valid for a seek, for which there is a
    // separate function. We can seek a bit beyond the end of a disk, as well as seek with no
    // disk in the drive.
    // pub fn is_id_valid(&self, chs: DiskChs) -> bool {
    //     if let Some(image) = &self.disk_image {
    //         image.is_id_valid(chs)
    //     }
    //     else {
    //         log::warn!("is_id_valid(): no disk image");
    //         false
    //     }
    // }

    /// Return whether the drive is physically capable of seeking to the specified cylinder
    pub fn is_seek_valid(&self, c: u16) -> bool {
        if c >= self.drive_geom.c() {
            return false;
        }
        true
    }

    pub fn seek(&mut self, c: u16) {
        if !self.is_seek_valid(c) {
            return;
        }
        self.cylinder = c;
        self.chsn.set_c(c);
    }

    pub fn advance_sector(&mut self) {
        if let Some(next_sector) = self.get_next_sector(self.chsn.into()) {
            log::warn!(
                "advance_sector(): advancing from sector {} to {}",
                self.chsn.s(),
                next_sector.s()
            );
            self.chsn = next_sector;
        }
        else {
            log::error!("advance_sector(): no next sector found");
        }
    }

    pub fn get_next_sector(&self, chs: DiskChs) -> Option<DiskChsn> {
        if let Some(image_lock) = &self.disk_image {
            if let Some(chsn) = read_lock_opt!(image_lock).get_next_id(chs) {
                return Some(chsn);
            }
            else {
                log::error!("get_next_sector(): no next sector found");
            }
        }
        else {
            log::error!("get_next_sector(): no image loaded");
        }
        None
    }

    pub fn get_chs_sector_offset(&self, sector_offset: usize, chs: DiskChs) -> DiskChs {
        let mut new_chs = chs;
        for _ in 0..sector_offset {
            if let Some(next_chs) = self.get_next_sector(new_chs) {
                new_chs = next_chs.into();
            }
        }

        new_chs
    }

    pub fn disk_present(&self) -> bool {
        self.disk_present
    }

    pub fn image_state(&self) -> Option<FloppyImageState> {
        if let Some(image_lock) = &self.disk_image {
            let image = read_lock_opt!(image_lock);
            let sector_map = image.sector_map();

            Some(FloppyImageState {
                format: None,
                heads: image.heads(),
                sector_map,
            })
        }
        else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_image(format: StandardFormat) -> DiskImage {
        ImageBuilder::new()
            .with_standard_format(format)
            .with_resolution(TrackDataResolution::BitStream)
            .build()
            .unwrap()
    }

    #[test]
    fn matched_sector_with_bad_address_crc_is_not_reported_as_not_found() {
        let result = ReadSectorResult {
            id_chsn: Some(DiskChsn::new(0, 0, 96, 2)),
            not_found: true,
            address_crc_error: true,
            ..Default::default()
        };

        assert!(!sector_not_found(&result));
    }

    #[test]
    fn strict_policy_rejects_a_different_physical_diskette_format() {
        let drive = FloppyDiskDrive::new(0, FloppyDriveType::Floppy720K);
        let image = build_image(StandardFormat::PcFloppy360);

        assert!(matches!(
            drive.validate_image_compatibility(&image, ImageInsertionPolicy::Strict),
            Err(FloppyMediaIncompatibility::UnsupportedFormat {
                format: Some(StandardFormat::PcFloppy360),
                ..
            })
        ));
    }

    #[test]
    fn lenient_policy_allows_a_different_physical_diskette_format() {
        let drive = FloppyDiskDrive::new(0, FloppyDriveType::Floppy720K);
        let image = build_image(StandardFormat::PcFloppy360);

        assert!(drive
            .validate_image_compatibility(&image, ImageInsertionPolicy::Lenient)
            .is_ok());
    }

    #[test]
    fn unsupported_data_rate_is_rejected_in_both_policies() {
        let drive = FloppyDiskDrive::new(0, FloppyDriveType::Floppy720K);

        for policy in [ImageInsertionPolicy::Strict, ImageInsertionPolicy::Lenient] {
            let image = build_image(StandardFormat::PcFloppy1440);
            assert!(matches!(
                drive.validate_image_compatibility(&image, policy),
                Err(FloppyMediaIncompatibility::UnsupportedDataRate {
                    data_rate: FloppyDataRate::Rate500Kbps,
                    ..
                })
            ));
        }
    }

    #[test]
    fn excessive_cylinder_count_is_rejected_before_attachment() {
        let mut drive = FloppyDiskDrive::new(0, FloppyDriveType::Floppy360K);
        let image = build_image(StandardFormat::PcFloppy720);

        assert!(drive
            .attach_image(image, None, false, ImageInsertionPolicy::Lenient)
            .is_err());
        assert!(!drive.disk_present());
        assert!(drive.disk_image.is_none());
    }

    #[test]
    fn cylinder_incompatibility_has_a_concise_message() {
        let error = FloppyMediaIncompatibility::GeometryExceedsDrive {
            media_cylinders: 80,
            media_heads: 2,
            drive_cylinders: 40,
            drive_heads: 2,
        };

        assert_eq!(error.to_string(), "image has 80 cylinders; drive supports 40");
    }
}

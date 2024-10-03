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
    device_types::fdc::{DiskFormat, FloppyImageType, DISK_FORMATS, DRIVE_CAPABILITIES},
    machine_types::FloppyDriveType,
};
use anyhow::{anyhow, Error};
use fluxfox::{
    diskimage::{RwSectorScope, SectorMapEntry},
    DiskCh,
    DiskChs,
    DiskChsn,
    DiskImage,
    DiskImageError,
    DiskSectorMap,
    StandardFormat,
};
use std::io::{Cursor, Read, Seek};

#[derive(Copy, Clone, Debug, Default)]
pub enum FloppyDriveOperation {
    #[default]
    NoOperation,
    ReadSector,
    WriteSector,
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
}

pub struct DriveReadResult {
    pub(crate) not_found: bool,
    pub(crate) sectors_read: u16,
    pub(crate) new_sid: u8,
    pub(crate) deleted_mark: bool,
}

pub struct DriveWriteResult {
    pub(crate) sectors_written: u8,
    pub(crate) new_sid: u8,
}

pub struct DriveFormatResult {
    pub(crate) sectors_formatted: u8,
    pub(crate) new_sid: u8,
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

pub struct FloppyDiskDrive {
    drive_type: FloppyDriveType,
    drive_n: usize,
    pub(crate) error_signal: bool,

    pub(crate) chsn: DiskChsn,
    drive_geom: DiskChs,
    pub(crate) media_geom: DiskChs,

    pub(crate) ready: bool,
    pub(crate) motor_on: bool,
    pub(crate) positioning: bool,
    pub(crate) disk_present: bool,
    pub(crate) write_protected: bool,
    pub(crate) disk_image: Option<DiskImage>,

    operation_status: OperationStatus,
    operation_buf: Cursor<Vec<u8>>,
    /// We keep a list of supported formats for this drive, primarily so we can query the largest
    /// supported format. This is used for building the appropriate size image when mounting a
    /// directory.
    pub(crate) supported_formats: Vec<FloppyImageType>,

    ref_write: u64,
}

impl Default for FloppyDiskDrive {
    fn default() -> Self {
        Self {
            drive_type: Default::default(),
            drive_n: 0,
            error_signal: false,
            chsn: Default::default(),
            drive_geom: Default::default(),
            media_geom: Default::default(),
            ready: false,
            motor_on: false,
            positioning: false,
            disk_present: false,
            write_protected: true,
            disk_image: None,

            operation_status: Default::default(),
            operation_buf:    Cursor::new(Vec::with_capacity(512 * 2)),

            supported_formats: Vec::new(),

            ref_write: 0,
        }
    }
}
impl FloppyDiskDrive {
    pub fn new(drive_n: usize, drive_type: FloppyDriveType) -> Self {
        // Should be safe to unwrap as we are limited by valid drive type enums.
        let drive_geom = DRIVE_CAPABILITIES.get(&drive_type).unwrap().chs;

        let supported_formats = match drive_type {
            FloppyDriveType::Floppy360K => vec![FloppyImageType::Image360K],
            FloppyDriveType::Floppy720K => vec![FloppyImageType::Image720K],
            FloppyDriveType::Floppy12M => vec![FloppyImageType::Image360K, FloppyImageType::Image12M],
            FloppyDriveType::Floppy144M => vec![FloppyImageType::Image720K, FloppyImageType::Image144M],
        };

        FloppyDiskDrive {
            drive_type,
            drive_n,
            drive_geom,
            supported_formats,
            ..Default::default()
        }
    }

    /// Reset the drive to default state. Like other device patterns we use default after preserving persistent state.
    /// Called when FDC itself is reset.
    pub fn reset(&mut self) {
        // Preserve the disk image before defaulting the drive
        let image = self.disk_image.take();

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
            supported_formats: self.supported_formats.clone(),
            ..Default::default()
        };
    }

    pub fn get_largest_supported_image_format(&self) -> FloppyImageType {
        self.supported_formats[self.supported_formats.len().saturating_sub(1)]
    }

    pub fn get_type(&self) -> FloppyDriveType {
        self.drive_type
    }

    /// Load a disk into the specified drive
    pub fn load_image_from(&mut self, src_vec: Vec<u8>, write_protect: bool) -> Result<(), Error> {
        /*        let image_len: usize = src_vec.len();

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
        }*/

        let mut image_buffer = Cursor::new(src_vec);
        let image = DiskImage::load(&mut image_buffer)?;

        self.media_geom = DiskChs::from((
            image.image_format().geometry.c(),
            image.image_format().geometry.h(),
            0u8,
        ));

        self.disk_present = true;
        self.disk_image = Some(image);
        self.write_protected = write_protect;

        log::debug!("Loaded floppy image, CHS: {}", self.media_geom,);

        Ok(())
    }

    pub fn get_image(&mut self) -> (Option<(&DiskImage)>, u64) {
        self.ref_write = self.disk_image.as_mut().map_or(0, |image| image.write_ct());
        //log::trace!("get_image(): ref_write: {}", self.ref_write);
        (self.disk_image.as_ref(), self.ref_write)
    }

    /// Unload (eject) the disk in the specified drive
    pub fn unload_image(&mut self) {
        self.chsn = Default::default();
        self.media_geom = DiskChs::default();
        self.disk_present = false;
        self.disk_image = None;
    }

    pub fn create_new_image(&mut self, format: StandardFormat, formatted: bool) -> Result<&DiskImage, Error> {
        self.unload_image();

        let builder = fluxfox::ImageBuilder::new()
            .with_standard_format(format)
            .with_resolution(fluxfox::DiskDataResolution::BitStream)
            .with_creator_tag(b"MartyPC")
            .with_formatted(formatted);

        let image = builder.build()?;
        self.chsn = Default::default();
        self.media_geom = format.get_chs();
        self.disk_present = true;
        self.disk_image = Some(image);

        Ok(&self.disk_image.as_ref().unwrap())
    }

    pub fn patch_image_bpb(&mut self, standard_format: StandardFormat) -> Result<(), Error> {
        if self.disk_image.is_none() {
            return Err(anyhow!("No media in drive"));
        }

        let image = self.disk_image.as_mut().unwrap();
        image.update_standard_boot_sector(standard_format)?;

        Ok(())
    }

    pub fn command_write_data(
        &mut self,
        chs: DiskChs,
        ct: usize,
        n: u8,
        sector_data: &[u8],
        _skip_flag: bool,
    ) -> Result<DriveWriteResult, Error> {
        if self.disk_image.is_none() {
            return Err(anyhow!("No media in drive"));
        }

        let image = self.disk_image.as_mut().unwrap();
        let chsn = fluxfox::DiskChsn::from((chs, n));

        let sector_data_size = chsn.n_size();
        if sector_data.len() != sector_data_size * ct {
            return Err(anyhow!(
                "Data buffer size: {} does not match (sector_size:{} * ct:{})",
                sector_data.len(),
                sector_data_size,
                ct
            ));
        }

        self.operation_status.sector_not_found = false;
        self.operation_status.address_crc_error = false;
        self.operation_status.data_crc_error = false;
        self.operation_status.deleted_mark = false;

        let mut sid = chsn.s();
        let mut sectors_written = 0;
        let mut write_buf_idx = 0;

        while sectors_written < ct {
            let data_slice = &sector_data[write_buf_idx..(write_buf_idx + sector_data_size)];
            log::trace!(
                "command_write_data(): writing sector: {} n: {} bytes: {}",
                sid,
                n,
                data_slice.len()
            );

            let write_sector_result = image.write_sector(
                fluxfox::DiskChs::from((chsn.c(), chsn.h(), sid)),
                Some(n),
                data_slice,
                RwSectorScope::DataOnly,
                false,
                false,
            )?;

            log::debug!(
                "command_write_data(): wrote sector: {} bytes, wrong cylinder: {}",
                sector_data_size,
                write_sector_result.wrong_cylinder
            );

            write_buf_idx += sector_data_size;
            sid += 1;
            sectors_written += 1;
        }

        Ok(DriveWriteResult {
            sectors_written: sectors_written as u8,
            new_sid: sid,
        })
    }

    pub fn command_read_data(
        &mut self,
        chs: DiskChs,
        ct: usize,
        n: u8,
        _track_len: u8,
        _gap3_len: u8,
        _data_len: u8,
        skip_flag: bool,
    ) -> Result<DriveReadResult, Error> {
        if self.disk_image.is_none() {
            return Err(anyhow!("No media in drive"));
        }

        let image = self.disk_image.as_mut().unwrap();

        let sector_size = fluxfox::DiskChsn::n_to_bytes(n);

        let mut operation_buf = Vec::with_capacity(sector_size * ct);
        let ff_chs = fluxfox::DiskChs::from((chs.c() as u16, chs.h(), chs.s()));

        self.operation_status.sector_not_found = false;
        self.operation_status.address_crc_error = false;
        self.operation_status.data_crc_error = false;
        self.operation_status.deleted_mark = false;
        self.operation_status.no_dam = false;

        let mut sid = ff_chs.s();
        let mut sectors_read = 0;

        while sectors_read < ct {
            let read_sector_result = match image.read_sector(
                fluxfox::DiskChs::from((ff_chs.c(), ff_chs.h(), sid)),
                Some(n),
                RwSectorScope::DataOnly,
                false,
            ) {
                Ok(result) => result,
                Err(DiskImageError::DataError) => {
                    self.operation_status.sector_not_found = true;
                    return Ok(DriveReadResult {
                        not_found: true,
                        sectors_read: 0,
                        new_sid: sid,
                        deleted_mark: false,
                    });
                }
                Err(e) => return Err(e.into()),
            };

            if read_sector_result.no_dam {
                self.operation_status.no_dam = true;
                return Ok(DriveReadResult {
                    not_found: false,
                    sectors_read: 0,
                    new_sid: sid,
                    deleted_mark: false,
                });
            }

            log::debug!(
                "command_read_sector(): read {} bytes, address_crc_error: {}, data_crc_error: {}, deleted_mark: {} no_dam: {}",
                read_sector_result.read_buf.len(),
                read_sector_result.address_crc_error,
                read_sector_result.data_crc_error,
                read_sector_result.deleted_mark,
                read_sector_result.no_dam
            );

            match (skip_flag, read_sector_result.deleted_mark) {
                (_, false) => {
                    // Normal mark read, skip flag irrelevant. Read current sector and continue.
                    if read_sector_result.address_crc_error {
                        self.operation_status.address_crc_error = true;
                        break;
                    }
                    if read_sector_result.data_crc_error {
                        self.operation_status.data_crc_error = true;
                    }
                    log::trace!(
                        "Extending operation buffer by {} bytes",
                        read_sector_result.read_buf.len()
                    );
                    operation_buf.extend(read_sector_result.read_buf);
                    sid = sid.wrapping_add(1);
                    sectors_read += 1;
                    continue;
                }
                (false, true) => {
                    // Deleted mark read, and skip flag not set. Read current sector and stop.
                    self.operation_status.deleted_mark = true;

                    if read_sector_result.address_crc_error {
                        // If address mark is bad, we do not read data
                        self.operation_status.address_crc_error = true;
                        break;
                    }
                    operation_buf.extend(read_sector_result.read_buf);
                    sid = sid.wrapping_add(1);
                    sectors_read += 1;
                    self.operation_status.data_crc_error |= read_sector_result.data_crc_error;

                    break;
                }
                (true, true) => {
                    // Deleted mark read, skip flag true. Skip the current sector and continue.
                    sid = sid.wrapping_add(1);
                    self.operation_status.deleted_mark = true;
                    if read_sector_result.address_crc_error {
                        self.operation_status.address_crc_error = true;
                        break;
                    }
                    // Since we are skipping, we don't care about data crc errors
                    continue;
                }
            }
        }

        self.operation_buf = Cursor::new(operation_buf);
        Ok(DriveReadResult {
            not_found: false,
            sectors_read: sectors_read as u16,
            new_sid: sid,
            deleted_mark: self.operation_status.deleted_mark,
        })
    }

    pub fn command_read_track(
        &mut self,
        ch: DiskCh,
        n: u8,
        eot: u8,
        xfer_size: Option<usize>,
    ) -> Result<DriveReadResult, Error> {
        if self.disk_image.is_none() {
            return Err(anyhow!("No media in drive"));
        }

        let image = self.disk_image.as_mut().unwrap();
        let sector_size = fluxfox::DiskChsn::n_to_bytes(n);
        let capacity = match xfer_size {
            Some(size) => size,
            None => sector_size * 9,
        };

        self.operation_status.sector_not_found = false;
        self.operation_status.address_crc_error = false;
        self.operation_status.data_crc_error = false;
        self.operation_status.deleted_mark = false;

        let mut sectors_read = 0;

        let read_track_result = image.read_all_sectors(ch, n, eot)?;

        if read_track_result.not_found {
            log::debug!("command_read_track(): sector not found");
            self.operation_status.sector_not_found = true;
            return Ok(DriveReadResult {
                not_found: true,
                sectors_read: 0,
                new_sid: 1,
                deleted_mark: false,
            });
        }
        else {
            log::debug!(
                "command_read_track(): read {} sectors, {} bytes, address_crc_error: {}, data_crc_error: {}, deleted_mark: {}",
                read_track_result.sectors_read,
                read_track_result.read_buf.len(),
                read_track_result.address_crc_error,
                read_track_result.data_crc_error,
                read_track_result.deleted_mark
            );
        }

        self.operation_buf = Cursor::new(read_track_result.read_buf);
        Ok(DriveReadResult {
            not_found: false,
            sectors_read: read_track_result.sectors_read,
            new_sid: (read_track_result.sectors_read + 1) as u8,
            deleted_mark: self.operation_status.deleted_mark,
        })
    }

    pub fn command_format_track(
        &mut self,
        ch: DiskCh,
        format_buffer: &[u8],
        gap3_len: u8,
        fill_byte: u8,
    ) -> Result<DriveFormatResult, Error> {
        if self.disk_image.is_none() {
            return Err(anyhow!("No media in drive"));
        }

        let image = self.disk_image.as_mut().unwrap();

        let mut fox_format_buffer = Vec::new();
        for buf_entry in format_buffer.chunks_exact(4) {
            let c = buf_entry[0] as u16;
            let h = buf_entry[1];
            let s = buf_entry[2];
            let n = buf_entry[3];

            let chsn = fluxfox::DiskChsn::new(c, h, s, n);
            fox_format_buffer.push(chsn);
        }

        let sector_ct = fox_format_buffer.len();

        log::trace!(
            "command_format_track(): formatting track: {}: {} sectors",
            ch,
            sector_ct
        );
        match image.format_track(ch, fox_format_buffer, fill_byte, gap3_len as usize) {
            Ok(_) => Ok(DriveFormatResult {
                sectors_formatted: sector_ct as u8,
                new_sid: (sector_ct + 1) as u8,
            }),
            Err(e) => Err(e.into()),
        }
    }

    pub fn read_operation_buf(&mut self) -> u8 {
        let byte_buf = &mut [0u8];
        self.operation_buf.read(byte_buf).unwrap();

        byte_buf[0]
    }

    pub fn get_operation_byte(&mut self, offset: usize) -> u8 {
        self.operation_buf
            .seek(std::io::SeekFrom::Start(offset as u64))
            .unwrap();
        let byte_buf = &mut [0u8];
        self.operation_buf.read(byte_buf).unwrap();

        byte_buf[0]
    }

    pub fn get_operation_status(&self) -> OperationStatus {
        self.operation_status
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
        if let Some(image) = &self.disk_image {
            image.is_id_valid(chs)
        }
        else {
            log::warn!("is_id_valid(): no disk image");
            false
        }
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
        self.chsn.seek(&chs);
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
        if let Some(image) = &self.disk_image {
            if let Some(chsn) = image.get_next_id(chs) {
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
        if let Some(image) = &self.disk_image {
            let sector_map = image.get_sector_map();

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

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

    devices::implementations::floppy_drive.rs

    Implements a floppy drive
*/

use crate::{
    device_types::fdc::{FloppyImageType, DRIVE_CAPABILITIES},
    machine_types::FloppyDriveType,
};
use anyhow::{anyhow, Error};
use fluxfox::{file_system::FileSystemType, prelude::*, DiskSectorMap};
use std::{
    io::{Cursor, Read, Seek},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

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

pub struct DriveReadResult {
    pub(crate) not_found: bool,
    pub(crate) sectors_read: u16,
    pub(crate) new_chs: DiskChs,
    pub(crate) deleted_mark: bool,
}

pub struct DriveWriteResult {
    pub(crate) not_found: bool,
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

    cylinder: u16,
    pub(crate) chsn: DiskChsn,
    drive_geom: DiskChs,
    pub(crate) media_geom: DiskChs,

    pub(crate) ready: bool,
    pub(crate) motor_on: bool,
    pub(crate) positioning: bool,
    pub(crate) disk_present: bool,
    pub(crate) write_protected: bool,
    pub(crate) disk_image: Option<Arc<RwLock<DiskImage>>>,

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
            cylinder: 0,
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
            // IBM DOS wants to see ready in ST3 even if no disk in drive.
            ready: true,
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
    pub fn load_image_from(
        &mut self,
        src_vec: Vec<u8>,
        path: Option<&Path>,
        write_protect: bool,
    ) -> Result<Arc<RwLock<DiskImage>>, Error> {
        let mut image_buffer = Cursor::new(src_vec);
        let image = DiskImage::load(&mut image_buffer, path, None, None)?;

        self.media_geom = DiskChs::from((
            image.image_format().geometry.c(),
            image.image_format().geometry.h(),
            0u8,
        ));

        log::debug!("Loaded floppy image, CHS: {}", self.media_geom,);
        self.disk_present = true;
        self.write_protected = write_protect;
        let image_arc = image.into_arc();
        let image_clone = image_arc.clone();
        self.disk_image = Some(image_arc);

        Ok(image_clone)
    }

    pub fn attach_image(
        &mut self,
        image: DiskImage,
        _path: Option<PathBuf>,
        write_protect: bool,
    ) -> Result<Arc<RwLock<DiskImage>>, Error> {
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

    pub fn command_write_data(
        &mut self,
        h: u8,
        id_chs: DiskChs,
        ct: usize,
        n: u8,
        sector_data: &[u8],
        _skip_flag: bool,
        deleted: bool,
    ) -> Result<DriveWriteResult, Error> {
        if self.disk_image.is_none() {
            return Err(anyhow!("No media in drive"));
        }

        let image_lock = self.disk_image.as_ref().unwrap();
        let mut image = write_lock!(image_lock);
        let chsn = DiskChsn::from((id_chs, n));

        let sector_data_size = chsn.n_size();
        if sector_data.len() != sector_data_size * ct {
            return Err(anyhow!(
                "Data buffer size: {} does not match (sector_size:{} * ct:{})",
                sector_data.len(),
                sector_data_size,
                ct
            ));
        }

        self.operation_status.reset(FloppyDriveOperation::WriteData);

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
                DiskCh::new(self.cylinder, h),
                DiskChsnQuery::new(chsn.c(), chsn.h(), sid, n),
                None,
                data_slice,
                RwScope::DataOnly,
                deleted,
                false,
            )?;

            self.operation_status.wrong_cylinder |= write_sector_result.wrong_cylinder;
            self.operation_status.address_crc_error |= write_sector_result.address_crc_error;
            self.operation_status.wrong_head |= write_sector_result.wrong_head;

            if write_sector_result.not_found {
                log::warn!("command_write_data(): sector not found");
                self.operation_status.sector_not_found = true;

                return Ok(DriveWriteResult {
                    not_found: true,
                    sectors_written: sectors_written as u8,
                    new_sid: sid,
                });
            }
            else {
                log::debug!(
                    "command_write_data(): wrote sector: {} bytes, wrong cylinder: {}",
                    sector_data_size,
                    write_sector_result.wrong_cylinder
                );
            }

            write_buf_idx += sector_data_size;
            sid += 1;
            sectors_written += 1;
        }

        Ok(DriveWriteResult {
            not_found: false,
            sectors_written: sectors_written as u8,
            new_sid: sid,
        })
    }

    pub fn command_read_data(
        &mut self,
        mut mt: bool,
        mut h: u8,
        id_chs: DiskChs,
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

        log::trace!(
            "command_read_data(): phys_c: {} h: {} id_chs: {}",
            self.cylinder,
            h,
            id_chs
        );

        let image_lock = self.disk_image.as_ref().unwrap();
        let mut image = write_lock!(image_lock);

        let sector_size = DiskChsn::n_to_bytes(n);

        let mut operation_buf = Vec::with_capacity(sector_size * ct);

        self.operation_status.reset(FloppyDriveOperation::ReadData);

        // Ignore multi-track if head is not 0. MT will only continue a read from head 0 to head 1,
        // it will not flip from head 1 back to head 0.
        if h > 0 || id_chs.h() > 0 {
            mt = false;
        }

        // Just disable mt entirely
        //mt = false;

        let mut op_chs = id_chs;
        let mut sectors_read = 0;
        let mut not_found_count = 0;

        while sectors_read < ct {
            let read_sector_result = match image.read_sector(
                DiskCh::new(self.cylinder, h),
                DiskChsnQuery::new(op_chs.c(), op_chs.h(), op_chs.s(), n),
                None,
                None,
                RwScope::DataOnly,
                false,
            ) {
                Ok(result) => result,
                Err(DiskImageError::DataError) => {
                    log::warn!("command_read_data(): no sectors found on this track");

                    self.operation_status.sector_not_found = true;
                    return Ok(DriveReadResult {
                        not_found: true,
                        sectors_read: 0,
                        new_chs: op_chs,
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
                    new_chs: op_chs,
                    deleted_mark: false,
                });
            }

            if read_sector_result.not_found {
                if mt && sectors_read > 0 && not_found_count == 0 {
                    // If we are in multi-track mode, and this is the first sector not found, we
                    // will attempt to read the next head instead of failing.

                    // TODO: use the last-sector flag from fluxfox instead of flipping heads on
                    //       the first sector not found
                    not_found_count += 1;
                    op_chs.set_h(1);
                    h = 1;
                    op_chs.set_s(1);
                    log::warn!(
                        "command_read_data(): sector not found with multi-track enabled, trying new phys_h: {} chs: {}",
                        h,
                        op_chs
                    );
                    continue;
                }
                else if mt && not_found_count > 0 {
                    // If we are in multi-track mode, and this is the second sector not found, we
                    // will stop reading and return the sectors read so far.
                    log::warn!("command_read_data(): sector not found with multi-track enabled, stopping read");
                    self.operation_status.sector_not_found = true;
                    return Ok(DriveReadResult {
                        not_found: true,
                        sectors_read: sectors_read as u16,
                        new_chs: op_chs,
                        deleted_mark: false,
                    });
                }
            }

            log::debug!(
                "command_read_data(): read sector id: {}, {} bytes, address_crc_error: {}, data_crc_error: {}, deleted_mark: {} no_dam: {}",
                op_chs,
                read_sector_result.data_range.len(),
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
                        read_sector_result.data_range.len()
                    );
                    operation_buf.extend(&read_sector_result.read_buf[read_sector_result.data_range]);
                    op_chs.set_s(op_chs.s().wrapping_add(1));
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
                    operation_buf.extend(&read_sector_result.read_buf[read_sector_result.data_range]);
                    op_chs.set_s(op_chs.s().wrapping_add(1));
                    sectors_read += 1;
                    self.operation_status.data_crc_error |= read_sector_result.data_crc_error;

                    break;
                }
                (true, true) => {
                    // Deleted mark read, skip flag true. Skip the current sector and continue.
                    op_chs.set_s(op_chs.s().wrapping_add(1));
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
            new_chs: op_chs,
            deleted_mark: self.operation_status.deleted_mark,
        })
    }

    pub fn command_read_track(
        &mut self,
        h: u8,
        id_ch: DiskCh,
        n: u8,
        eot: u8,
        _xfer_size: Option<usize>,
    ) -> Result<DriveReadResult, Error> {
        if self.disk_image.is_none() {
            return Err(anyhow!("No media in drive"));
        }

        let image_lock = self.disk_image.as_ref().unwrap();
        let mut image = write_lock!(image_lock);

        self.operation_status.sector_not_found = false;
        self.operation_status.address_crc_error = false;
        self.operation_status.data_crc_error = false;
        self.operation_status.deleted_mark = false;

        let phys_ch = DiskCh::new(self.cylinder, h);
        let read_track_result = image.read_all_sectors(phys_ch, id_ch, n, eot)?;

        if read_track_result.not_found {
            log::debug!("command_read_track(): sector not found");
            self.operation_status.sector_not_found = true;
            return Ok(DriveReadResult {
                not_found: true,
                sectors_read: 0,
                new_chs: DiskChs::from((id_ch, 1)),
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
            new_chs: DiskChs::from((id_ch, (read_track_result.sectors_read + 1) as u8)),
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

        log::trace!(
            "command_format_track(): formatting track: {}: {} sectors",
            ch,
            sector_ct
        );
        match image.format_track(ch, fox_format_buffer, &[fill_byte], gap3_len as usize) {
            Ok(_) => Ok(DriveFormatResult {
                sectors_formatted: sector_ct as u8,
                new_sid: (sector_ct + 1) as u8,
            }),
            Err(e) => Err(e.into()),
        }
    }

    pub fn read_operation_buf(&mut self) -> u8 {
        let byte_buf = &mut [0u8];
        _ = self.operation_buf.read_exact(byte_buf);

        byte_buf[0]
    }

    pub fn get_operation_byte(&mut self, offset: usize) -> u8 {
        self.operation_buf
            .seek(std::io::SeekFrom::Start(offset as u64))
            .unwrap();
        let byte_buf = &mut [0u8];
        _ = self.operation_buf.read_exact(byte_buf);

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

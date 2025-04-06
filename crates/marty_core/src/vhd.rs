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

    vhd.rs

    Implements VHD support including reading and writing to VHD images.

*/

use core::fmt::Display;
use std::{
    error::Error,
    ffi::OsString,
    fs,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    str,
};

pub const SECTOR_SIZE: usize = 512;

use anyhow::{bail, Result};
use uuid::Uuid;

use crate::bytebuf::{ByteBuf, ByteBufWriter};

/// A trait alias for objects that support reading, writing, and seeking.
pub trait VhdIO: Read + Write + Seek {}

/// Implement VhdIO for all types that satisfy Read + Write + Seek.
impl<T: Read + Write + Seek> VhdIO for T {}

pub const VHD_FOOTER_LEN: usize = 512;
pub const VHD_SECTOR_SIZE: usize = 512;
pub const VHD_VERSION: u32 = 0x00010000;
pub const VHD_DATA_OFFSET: u64 = 0xFFFFFFFFFFFFFFFF;
pub const VHD_FEATURE_RESERVED: u32 = 0x02;
pub const VHD_CHECKSUM_OFFSET: usize = 64;
pub const VHD_DISK_TYPE: u32 = 0x02;

// TODO: Refactor this with thiserror
#[derive(Debug)]
pub enum VirtualHardDiskError {
    FileExists,
    InvalidLength,
    InvalidFooter,
    InvalidVersion,
    InvalidType,
    InvalidSeek,
    WriteFailure,
}
impl Error for VirtualHardDiskError {}
impl Display for VirtualHardDiskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VirtualHardDiskError::FileExists => write!(
                f,
                "Creation of VHD failed as the file already exists (Will not overwrite)."
            ),
            VirtualHardDiskError::InvalidLength => write!(f, "The VHD file was an invalid size."),
            VirtualHardDiskError::InvalidFooter => {
                write!(f, "The VHD footer was invalid or contained an invalid value.")
            }
            VirtualHardDiskError::InvalidVersion => {
                write!(f, "The VHD file is an unsupported version.")
            }
            VirtualHardDiskError::InvalidType => write!(f, "The VHD file is not a supported type."),
            VirtualHardDiskError::InvalidSeek => {
                write!(f, "An IO operation was requested out of bounds.")
            }
            VirtualHardDiskError::WriteFailure => {
                write!(f, "An error occurred while writing to the VHD file.")
            }
        }
    }
}

#[allow(dead_code)]
pub struct VirtualHardDisk {
    vhd_file:  Box<dyn VhdIO>,
    read_only: bool,
    footer:    VHDFileFooter,

    size: u64,
    checksum: u32,

    pub max_cylinders: u32,
    pub max_heads: u32,
    pub max_sectors: u32,

    cur_cylinder: u32,
    cur_head: u32,
    cur_sector: u32,
}

#[derive(Clone, Debug, Default)]
pub struct VHDGeometry {
    pub c: u16,
    pub h: u8,
    pub s: u8,
}

// TODO: Refactor this with binrw
#[derive(Default)]
pub struct VHDFileFooter {
    cookie: [u8; 8],
    features: u32,
    version: u32,
    offset: u64,
    timestamp: u32,
    creator_app: [u8; 4],
    creator_version: u32,
    creator_os: [u8; 4],
    original_size: u64,
    current_size: u64,
    geometry: VHDGeometry,
    disk_type: u32,
    checksum: u32,
    uuid: Uuid,
    saved_state: u8,
    // There is 427 bytes of padding here, but we can't Default it
}
impl VHDFileFooter {
    pub fn new(c: u16, h: u8, s: u8, id: Uuid) -> Self {
        let mut cookie: [u8; 8] = [0; 8];
        cookie.copy_from_slice("conectix".as_bytes());

        let mut app: [u8; 4] = [0; 4];
        app.copy_from_slice("mrty".as_bytes());

        let mut os: [u8; 4] = [0; 4];
        os.copy_from_slice("Wi2k".as_bytes());

        let size: u64 = c as u64 * h as u64 * s as u64 * VHD_SECTOR_SIZE as u64;

        let geom = VHDGeometry { c, h, s };

        Self {
            cookie,
            features: VHD_FEATURE_RESERVED,
            version: VHD_VERSION,
            offset: VHD_DATA_OFFSET,
            timestamp: 0,
            creator_app: app,
            creator_version: VHD_VERSION,
            creator_os: os,
            original_size: size,
            current_size: size,
            geometry: geom,
            disk_type: VHD_DISK_TYPE,
            checksum: 0,
            uuid: id,
            saved_state: 0,
        }
    }

    /// Write the fields of a VHD footer into the specified buffer which should be 512 bytes long.
    fn make_vhd_footer_bytes(buf: &mut [u8], footer: VHDFileFooter) {
        {
            let mut bytebuf = ByteBufWriter::from_slice(buf);
            bytebuf.write_bytes("conectix".as_bytes(), 8).unwrap();
            bytebuf.write_u32_be(footer.features).unwrap();
            bytebuf.write_u32_be(footer.version).unwrap();
            bytebuf.write_u64_be(VHD_DATA_OFFSET).unwrap();
            bytebuf.write_u32_be(footer.timestamp).unwrap();
            bytebuf.write_bytes(&footer.creator_app, 4).unwrap();
            bytebuf.write_u32_be(footer.creator_version).unwrap();
            bytebuf.write_bytes(&footer.creator_os, 4).unwrap();
            bytebuf.write_u64_be(footer.original_size).unwrap();
            bytebuf.write_u64_be(footer.current_size).unwrap();
            bytebuf.write_u16_be(footer.geometry.c).unwrap();
            bytebuf.write_u8(footer.geometry.h).unwrap();
            bytebuf.write_u8(footer.geometry.s).unwrap();
            bytebuf.write_u32_be(footer.disk_type).unwrap();
            bytebuf.write_u32_be(0).unwrap(); // Checksum calculated later
            bytebuf.write_bytes(&footer.uuid.into_bytes(), 16).unwrap();
            bytebuf.write_u8(footer.saved_state).unwrap();
            // Bytebuf dropped here so we can compute checksum
        }
        let checksum = VHDFileFooter::calculate_footer_checksum(buf);

        // Write checksum
        let mut bytebuf = ByteBufWriter::from_slice(buf);
        bytebuf.seek(VHD_CHECKSUM_OFFSET).unwrap();
        bytebuf.write_u32_be(checksum).unwrap();
    }

    /// Parse the footer of a VHD file.
    ///
    /// We could do this a lot faster with some unsafe magic, but I'm doing it the 'safe' way.
    fn parse_vhd_footer(buf: &[u8]) -> Result<VHDFileFooter, anyhow::Error> {
        let mut footer = VHDFileFooter::default();
        let mut bytebuf = ByteBuf::from_slice(buf);

        bytebuf.read_bytes(&mut footer.cookie, 8)?;
        if footer.cookie != "conectix".as_bytes() {
            bail!(VirtualHardDiskError::InvalidFooter);
        }

        footer.features = bytebuf.read_u32_be()?;
        if footer.features != VHD_FEATURE_RESERVED {
            log::warn!("VHD may contain unsupported features.")
        }

        footer.version = bytebuf.read_u32_be()?;
        if footer.version != VHD_VERSION {
            bail!(VirtualHardDiskError::InvalidVersion);
        }

        footer.offset = bytebuf.read_u64_be()?;
        if footer.offset != 0xFFFFFFFFFFFFFFFFu64 {
            bail!(VirtualHardDiskError::InvalidFooter);
        }

        footer.timestamp = bytebuf.read_u32_be()?;

        bytebuf.read_bytes(&mut footer.creator_app, 4)?;
        // These aren't technically utf-8 strings but there's not from_ascii in std soo...
        let creator_app_str = str::from_utf8(&footer.creator_app).unwrap_or("(invalid)");
        log::info!("VHD Creator: {:?} ({})", footer.creator_app, creator_app_str);

        footer.creator_version = bytebuf.read_u32_be()?;
        log::info!("VHD Creator Version: {:08X}", footer.creator_version);

        bytebuf.read_bytes(&mut footer.creator_os, 4)?;
        let creator_os_str = str::from_utf8(&footer.creator_os).unwrap_or("(invalid)");
        log::info!("VHD Creator OS: {:?} ({})", footer.creator_os, creator_os_str);

        footer.original_size = bytebuf.read_u64_be()?;
        footer.current_size = bytebuf.read_u64_be()?;

        footer.geometry.c = bytebuf.read_u16_be()?;
        footer.geometry.h = bytebuf.read_u8()?;
        footer.geometry.s = bytebuf.read_u8()?;

        log::info!(
            "VHD Geometry: c:{} h:{} s:{}",
            footer.geometry.c,
            footer.geometry.h,
            footer.geometry.s
        );

        footer.disk_type = bytebuf.read_u32_be()?;
        if footer.disk_type != 0x02 {
            bail!(VirtualHardDiskError::InvalidType);
        }

        footer.checksum = bytebuf.read_u32_be()?;

        if footer.checksum != VHDFileFooter::calculate_footer_checksum(buf) {
            log::warn!("VHD Checksum incorrect");
        }

        // Parse the UUID
        let mut uuid_buf: [u8; 16] = [0; 16];
        bytebuf.read_bytes(&mut uuid_buf, 16)?;

        footer.uuid = uuid::Builder::from_bytes(uuid_buf).into_uuid();
        log::info!("VHD UUID: {}", footer.uuid.to_string());

        footer.saved_state = bytebuf.read_u8()?;
        Ok(footer)
    }

    fn calculate_footer_checksum(buf: &[u8]) -> u32 {
        let mut sum: u32 = 0;

        for byte in buf.iter().take(VHD_CHECKSUM_OFFSET) {
            sum += *byte as u32;
        }
        // Skip checksum field
        for byte in buf.iter().take(VHD_FOOTER_LEN).skip(VHD_CHECKSUM_OFFSET + 4) {
            sum += *byte as u32;
        }

        // Return one's compliment of sum
        !sum
    }

    fn geometry(&self) -> VHDGeometry {
        self.geometry.clone()
    }
}

impl VirtualHardDisk {
    pub fn parse(mut vhd_file: Box<dyn VhdIO>, read_only: bool) -> Result<VirtualHardDisk, anyhow::Error> {
        let vhd_file_size = vhd_file.seek(SeekFrom::End(0))?;

        // Check that the file is long enough to even read the footer in. Such a small file will fail
        // for other reasons later such as not containing the proper chs
        if vhd_file_size <= VHD_FOOTER_LEN as u64 {
            bail!(VirtualHardDiskError::InvalidLength);
        }

        let mut trailer_buf = vec![0u8; VHD_FOOTER_LEN];

        vhd_file.seek(SeekFrom::End(-(VHD_FOOTER_LEN as i64)))?;
        // Read in the entire footer
        vhd_file.read_exact(&mut trailer_buf)?;

        let footer = VHDFileFooter::parse_vhd_footer(&trailer_buf)?;

        Ok(VirtualHardDisk {
            vhd_file,
            read_only,

            size: vhd_file_size,
            checksum: 0,

            max_cylinders: footer.geometry.c as u32,
            max_heads: footer.geometry.h as u32,
            max_sectors: footer.geometry.s as u32,

            cur_cylinder: 0,
            cur_head: 0,
            cur_sector: 0,

            footer,
        })
    }

    pub fn size(&mut self) -> Result<u64, anyhow::Error> {
        // Get the size of the VHD reader, restore the stream position after
        let pos = self.vhd_file.stream_position()?;
        let size = self.vhd_file.seek(SeekFrom::End(0))?;
        self.vhd_file.seek(SeekFrom::Start(pos))?;
        Ok(size)
    }

    /// Return a byte offset given a CHS (Cylinder, Head, Sector) address
    ///
    /// Hard drive sectors are allowed to start at 0
    pub fn get_chs_offset(&self, cylinder: u16, head: u8, sector: u8) -> u64 {
        let lba: u64 = ((cylinder as u32 * self.max_heads + (head as u32)) * self.max_sectors + (sector as u32)) as u64;

        //log::trace!(">>>>>>>>>> Computed offset for c: {} h: {} s: {} of {:08X}", cylinder, head, sector, lba * SECTOR_SIZE);
        lba * SECTOR_SIZE as u64
    }

    pub fn read_sector(&mut self, buf: &mut [u8], cylinder: u16, head: u8, sector: u8) -> Result<(), anyhow::Error> {
        let read_offset = self.get_chs_offset(cylinder, head, sector);
        if read_offset > self.size()? - VHD_FOOTER_LEN as u64 - VHD_SECTOR_SIZE as u64 {
            // Read requested past last sector in file
            bail!(VirtualHardDiskError::InvalidSeek);
        }
        self.vhd_file.seek(SeekFrom::Start(read_offset))?;
        self.vhd_file.read_exact(buf)?;

        //log::debug!("Read sector from VHD at offset: {} read buf: {:X?}", read_offset, buf);
        Ok(())
    }

    pub fn write_sector(&mut self, buf: &[u8], cylinder: u16, head: u8, sector: u8) -> Result<(), anyhow::Error> {
        let write_offset = self.get_chs_offset(cylinder, head, sector);
        if write_offset > self.size()? - VHD_FOOTER_LEN as u64 - VHD_SECTOR_SIZE as u64 {
            // Write requested past last sector in file
            bail!(VirtualHardDiskError::InvalidSeek);
        }

        self.vhd_file.seek(SeekFrom::Start(write_offset))?;

        let write_len = self.vhd_file.write(buf)?;
        if write_len != VHD_SECTOR_SIZE {
            bail!(VirtualHardDiskError::WriteFailure);
        }

        Ok(())
    }

    pub fn geometry(&self) -> VHDGeometry {
        self.footer.geometry()
    }
}

pub fn create_vhd(filename: OsString, c: u16, h: u8, s: u8) -> Result<File, anyhow::Error> {
    assert_eq!(VHD_FOOTER_LEN, VHD_SECTOR_SIZE);

    // Don't overwrite an existing file
    if fs::metadata(&filename).is_ok() {
        log::warn!("Requested VHD file already exists: {:?}", filename);
        bail!(VirtualHardDiskError::FileExists);
    }

    // Create the requested file
    let mut vhd_file = File::create(filename)?;

    // Generate a new UUID for our VHD
    let uuid = Uuid::new_v4();

    let mut write_buf = vec![0; VHD_SECTOR_SIZE];

    // Write all 0's by sector buf size
    let n_sectors = c as u32 * h as u32 * s as u32;

    for _ in 0..n_sectors {
        vhd_file.write_all(&write_buf)?;
    }

    let footer = VHDFileFooter::new(c, h, s, uuid);

    // Since the length of a VHD footer == a sector size, re-use sector buf
    VHDFileFooter::make_vhd_footer_bytes(&mut write_buf, footer);

    vhd_file.write_all(&write_buf)?;

    Ok(vhd_file)
}

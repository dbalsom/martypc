use std::io::{Cursor, Read, Seek, SeekFrom, Write};

#[cfg(feature = "builder")]
use std::{fs::File, path::Path};

use binrw::{BinRead, BinWrite};
use thiserror::Error;

#[cfg(feature = "builder")]
use uuid::Uuid;

use crate::geometry::{Geometry, SECTOR_SIZE};

const FOOTER_LEN: usize = 512;
const CHECKSUM_OFFSET: usize = 64;
const VHD_VERSION: u32 = 0x0001_0000;
const VHD_DATA_OFFSET: u64 = u64::MAX;
const FIXED_DISK_TYPE: u32 = 2;
const FEATURE_TEMPORARY: u32 = 0x0000_0001;
const FEATURE_RESERVED: u32 = 0x0000_0002;
const SUPPORTED_FEATURES: u32 = FEATURE_TEMPORARY | FEATURE_RESERVED;

/// A backing object that can be used for VHD sector I/O.
pub trait VhdIo: Read + Write + Seek {}

impl<T: Read + Write + Seek> VhdIo for T {}

/// Controls whether a mounted VHD permits writes through [`VirtualHardDisk`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VhdAccess {
    ReadOnly,
    ReadWrite,
}

/// Errors produced while opening or accessing a VHD.
#[derive(Debug, Error)]
pub enum VhdError {
    #[error("VHD I/O failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to encode or decode the VHD footer: {0}")]
    FooterCodec(#[from] binrw::Error),

    #[error("file is too small to contain a fixed VHD")]
    InvalidLength,

    #[error("invalid VHD footer cookie")]
    InvalidCookie,

    #[error("unsupported VHD features: {0:#010X}")]
    UnsupportedFeatures(u32),

    #[error("unsupported VHD version: {0:#010X}")]
    UnsupportedVersion(u32),

    #[error("VHD is not a fixed-size image")]
    UnsupportedDiskType,

    #[error("invalid VHD footer checksum: expected {expected:#010X}, found {actual:#010X}")]
    InvalidChecksum { expected: u32, actual: u32 },

    #[error("invalid VHD geometry: {0}")]
    InvalidGeometry(String),

    #[error("invalid VHD virtual size: {0}")]
    InvalidVirtualSize(u64),

    #[error("VHD file length is {actual} bytes, but its footer declares {expected} bytes")]
    InvalidFileLength { actual: u64, expected: u64 },

    #[error("sector buffer must be exactly {expected} bytes, but is {actual} bytes")]
    InvalidSectorBufferLength { expected: usize, actual: usize },

    #[error("LBA {lba} is outside the VHD's {sector_count} sectors")]
    LbaOutOfRange { lba: u64, sector_count: u64 },

    #[error("CHS address {cylinder}:{head}:{sector} is outside geometry {geometry:?}")]
    ChsOutOfRange { cylinder: u16, head: u8, sector: u8, geometry: Geometry },

    #[error("cannot write to a read-only VHD")]
    ReadOnly,
}

/// Validated metadata from a fixed VHD footer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VhdMetadata {
    features: u32,
    timestamp: u32,
    creator_application: [u8; 4],
    creator_version: u32,
    creator_host_os: [u8; 4],
    original_size: u64,
    current_size: u64,
    geometry: Geometry,
    unique_id: [u8; 16],
    saved_state: bool,
}

impl VhdMetadata {
    pub fn features(&self) -> u32 {
        self.features
    }

    pub fn timestamp(&self) -> u32 {
        self.timestamp
    }

    pub fn creator_application(&self) -> [u8; 4] {
        self.creator_application
    }

    pub fn creator_version(&self) -> u32 {
        self.creator_version
    }

    pub fn creator_host_os(&self) -> [u8; 4] {
        self.creator_host_os
    }

    pub fn original_size(&self) -> u64 {
        self.original_size
    }

    pub fn current_size(&self) -> u64 {
        self.current_size
    }

    pub fn geometry(&self) -> Geometry {
        self.geometry
    }

    pub fn unique_id(&self) -> [u8; 16] {
        self.unique_id
    }

    pub fn saved_state(&self) -> bool {
        self.saved_state
    }
}

/// A validated fixed VHD backed by an arbitrary readable, writable, seekable object.
pub struct VirtualHardDisk {
    io: Box<dyn VhdIo>,
    metadata: VhdMetadata,
    access: VhdAccess,
}

impl VirtualHardDisk {
    /// Open and validate a fixed VHD.
    pub fn open(mut io: Box<dyn VhdIo>, access: VhdAccess) -> Result<Self, VhdError> {
        let metadata = read_metadata(&mut *io)?;
        Ok(Self { io, metadata, access })
    }

    pub fn metadata(&self) -> &VhdMetadata {
        &self.metadata
    }

    pub fn geometry(&self) -> Geometry {
        self.metadata.geometry()
    }

    pub fn virtual_size(&self) -> u64 {
        self.metadata.current_size()
    }

    pub fn sector_count(&self) -> u64 {
        self.virtual_size() / SECTOR_SIZE as u64
    }

    pub fn access(&self) -> VhdAccess {
        self.access
    }

    pub fn read_sector_lba(&mut self, lba: u64, buffer: &mut [u8]) -> Result<(), VhdError> {
        validate_sector_buffer(buffer.len())?;
        let offset = self.lba_offset(lba)?;
        self.io.seek(SeekFrom::Start(offset))?;
        self.io.read_exact(buffer)?;
        Ok(())
    }

    pub fn write_sector_lba(&mut self, lba: u64, buffer: &[u8]) -> Result<(), VhdError> {
        validate_sector_buffer(buffer.len())?;
        if self.access == VhdAccess::ReadOnly {
            return Err(VhdError::ReadOnly);
        }
        let offset = self.lba_offset(lba)?;
        self.io.seek(SeekFrom::Start(offset))?;
        self.io.write_all(buffer)?;
        Ok(())
    }

    /// Read a sector using a zero-based CHS address.
    pub fn read_sector_chs(&mut self, cylinder: u16, head: u8, sector: u8, buffer: &mut [u8]) -> Result<(), VhdError> {
        let lba = self.chs_lba(cylinder, head, sector)?;
        self.read_sector_lba(lba, buffer)
    }

    /// Write a sector using a zero-based CHS address.
    pub fn write_sector_chs(&mut self, cylinder: u16, head: u8, sector: u8, buffer: &[u8]) -> Result<(), VhdError> {
        let lba = self.chs_lba(cylinder, head, sector)?;
        self.write_sector_lba(lba, buffer)
    }

    pub fn flush(&mut self) -> Result<(), VhdError> {
        self.io.flush()?;
        Ok(())
    }

    pub fn into_inner(self) -> Box<dyn VhdIo> {
        self.io
    }

    fn chs_lba(&self, cylinder: u16, head: u8, sector: u8) -> Result<u64, VhdError> {
        self.geometry()
            .chs_to_lba(cylinder, head, sector)
            .map(u64::from)
            .ok_or(VhdError::ChsOutOfRange {
                cylinder,
                head,
                sector,
                geometry: self.geometry(),
            })
    }

    fn lba_offset(&self, lba: u64) -> Result<u64, VhdError> {
        let sector_count = self.sector_count();
        if lba >= sector_count {
            return Err(VhdError::LbaOutOfRange { lba, sector_count });
        }
        Ok(lba * SECTOR_SIZE as u64)
    }
}

#[derive(BinRead, BinWrite, Clone, Debug)]
#[brw(big)]
struct VhdFooter {
    cookie: [u8; 8],
    features: u32,
    version: u32,
    data_offset: u64,
    timestamp: u32,
    creator_app: [u8; 4],
    creator_version: u32,
    creator_host_os: [u8; 4],
    original_size: u64,
    current_size: u64,
    cylinders: u16,
    heads: u8,
    sectors_per_track: u8,
    disk_type: u32,
    checksum: u32,
    unique_id: [u8; 16],
    saved_state: u8,
    reserved: [u8; 427],
}

impl VhdFooter {
    #[cfg(any(feature = "builder", test))]
    fn new(geometry: Geometry, unique_id: [u8; 16]) -> Self {
        Self {
            cookie: *b"conectix",
            features: FEATURE_RESERVED,
            version: VHD_VERSION,
            data_offset: VHD_DATA_OFFSET,
            // Zero represents the VHD epoch and avoids encoding the host clock.
            timestamp: 0,
            creator_app: *b"mrty",
            creator_version: VHD_VERSION,
            creator_host_os: *b"Wi2k",
            original_size: geometry.byte_len(),
            current_size: geometry.byte_len(),
            cylinders: geometry.cylinders(),
            heads: geometry.heads(),
            sectors_per_track: geometry.sectors(),
            disk_type: FIXED_DISK_TYPE,
            checksum: 0,
            unique_id,
            saved_state: 0,
            reserved: [0; 427],
        }
    }

    #[cfg(any(feature = "builder", test))]
    fn calculated_checksum(&self) -> Result<u32, VhdError> {
        let mut footer = self.clone();
        footer.checksum = 0;
        let mut encoded = Cursor::new([0u8; FOOTER_LEN]);
        footer.write_be(&mut encoded)?;
        Ok(calculate_footer_checksum(&encoded.into_inner()))
    }

    fn metadata(&self) -> Result<VhdMetadata, VhdError> {
        let geometry = Geometry::new(self.cylinders, self.heads, self.sectors_per_track)
            .map_err(|error| VhdError::InvalidGeometry(error.to_string()))?;
        Ok(VhdMetadata {
            features: self.features,
            timestamp: self.timestamp,
            creator_application: self.creator_app,
            creator_version: self.creator_version,
            creator_host_os: self.creator_host_os,
            original_size: self.original_size,
            current_size: self.current_size,
            geometry,
            unique_id: self.unique_id,
            saved_state: self.saved_state != 0,
        })
    }
}

fn read_metadata(reader: &mut (impl Read + Seek + ?Sized)) -> Result<VhdMetadata, VhdError> {
    let file_len = reader.seek(SeekFrom::End(0))?;
    if file_len <= FOOTER_LEN as u64 {
        return Err(VhdError::InvalidLength);
    }

    reader.seek(SeekFrom::End(-(FOOTER_LEN as i64)))?;
    let mut footer_bytes = [0u8; FOOTER_LEN];
    reader.read_exact(&mut footer_bytes)?;
    let footer = VhdFooter::read_be(&mut Cursor::new(footer_bytes))?;

    if footer.cookie != *b"conectix" {
        return Err(VhdError::InvalidCookie);
    }
    if footer.features & FEATURE_RESERVED == 0 || footer.features & !SUPPORTED_FEATURES != 0 {
        return Err(VhdError::UnsupportedFeatures(footer.features));
    }
    if footer.version != VHD_VERSION {
        return Err(VhdError::UnsupportedVersion(footer.version));
    }
    if footer.data_offset != VHD_DATA_OFFSET || footer.disk_type != FIXED_DISK_TYPE {
        return Err(VhdError::UnsupportedDiskType);
    }

    let expected_checksum = calculate_footer_checksum(&footer_bytes);
    if footer.checksum != expected_checksum {
        return Err(VhdError::InvalidChecksum {
            expected: expected_checksum,
            actual:   footer.checksum,
        });
    }
    if footer.current_size == 0 || footer.current_size % SECTOR_SIZE as u64 != 0 {
        return Err(VhdError::InvalidVirtualSize(footer.current_size));
    }

    let expected_file_len = footer
        .current_size
        .checked_add(FOOTER_LEN as u64)
        .ok_or(VhdError::InvalidVirtualSize(footer.current_size))?;
    if file_len != expected_file_len {
        return Err(VhdError::InvalidFileLength {
            actual:   file_len,
            expected: expected_file_len,
        });
    }

    footer.metadata()
}

fn calculate_footer_checksum(footer: &[u8; FOOTER_LEN]) -> u32 {
    let sum = footer
        .iter()
        .enumerate()
        .filter(|(index, _)| !(*index >= CHECKSUM_OFFSET && *index < CHECKSUM_OFFSET + 4))
        .map(|(_, byte)| u32::from(*byte))
        .sum::<u32>();
    !sum
}

fn validate_sector_buffer(actual: usize) -> Result<(), VhdError> {
    if actual != SECTOR_SIZE {
        return Err(VhdError::InvalidSectorBufferLength {
            expected: SECTOR_SIZE,
            actual,
        });
    }
    Ok(())
}

#[cfg(feature = "builder")]
pub fn initialize(file: &mut File, geometry: Geometry) -> Result<(), VhdError> {
    file.set_len(geometry.byte_len() + FOOTER_LEN as u64)?;
    Ok(())
}

#[cfg(feature = "builder")]
pub fn write_footer(file: &mut (impl Write + Seek), geometry: Geometry) -> Result<(), VhdError> {
    let mut footer = VhdFooter::new(geometry, *Uuid::new_v4().as_bytes());
    footer.checksum = footer.calculated_checksum()?;
    file.seek(SeekFrom::Start(geometry.byte_len()))?;
    footer.write_be(file)?;
    Ok(())
}

#[cfg(feature = "builder")]
pub fn read_geometry(path: &Path) -> Result<Geometry, VhdError> {
    let mut file = File::open(path)?;
    Ok(read_metadata(&mut file)?.geometry())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_image(geometry: Geometry) -> Vec<u8> {
        make_image_with_declared_size(geometry, geometry.byte_len())
    }

    fn make_image_with_declared_size(geometry: Geometry, current_size: u64) -> Vec<u8> {
        let mut image = vec![0u8; geometry.byte_len() as usize + FOOTER_LEN];
        let mut footer = VhdFooter::new(geometry, [0x5a; 16]);
        footer.current_size = current_size;
        footer.checksum = footer.calculated_checksum().unwrap();
        let footer_offset = image.len() - FOOTER_LEN;
        footer.write_be(&mut Cursor::new(&mut image[footer_offset..])).unwrap();
        image
    }

    fn open_image(image: Vec<u8>, access: VhdAccess) -> Result<VirtualHardDisk, VhdError> {
        VirtualHardDisk::open(Box::new(Cursor::new(image)), access)
    }

    #[test]
    fn opens_cursor_backed_fixed_vhd() {
        let geometry = Geometry::new(2, 2, 4).unwrap();
        let disk = open_image(make_image(geometry), VhdAccess::ReadWrite).unwrap();

        assert_eq!(disk.geometry(), geometry);
        assert_eq!(disk.virtual_size(), geometry.byte_len());
        assert_eq!(disk.sector_count(), u64::from(geometry.total_sectors()));
        assert_eq!(disk.metadata().unique_id(), [0x5a; 16]);
    }

    #[test]
    fn reads_and_writes_lba_sectors() {
        let geometry = Geometry::new(2, 2, 4).unwrap();
        let mut disk = open_image(make_image(geometry), VhdAccess::ReadWrite).unwrap();
        let written = [0xa5; SECTOR_SIZE];
        let mut read = [0; SECTOR_SIZE];

        disk.write_sector_lba(3, &written).unwrap();
        disk.read_sector_lba(3, &mut read).unwrap();

        assert_eq!(read, written);
    }

    #[test]
    fn reads_and_writes_zero_based_chs_sectors() {
        let geometry = Geometry::new(2, 2, 4).unwrap();
        let mut disk = open_image(make_image(geometry), VhdAccess::ReadWrite).unwrap();
        let written = [0x3c; SECTOR_SIZE];
        let mut read = [0; SECTOR_SIZE];

        disk.write_sector_chs(1, 0, 2, &written).unwrap();
        disk.read_sector_lba(10, &mut read).unwrap();

        assert_eq!(read, written);
    }

    #[test]
    fn rejects_writes_to_read_only_vhd() {
        let geometry = Geometry::new(1, 1, 1).unwrap();
        let mut disk = open_image(make_image(geometry), VhdAccess::ReadOnly).unwrap();

        let error = disk.write_sector_lba(0, &[0xff; SECTOR_SIZE]).unwrap_err();

        assert!(matches!(error, VhdError::ReadOnly));
        let mut read = [0xff; SECTOR_SIZE];
        disk.read_sector_lba(0, &mut read).unwrap();
        assert_eq!(read, [0; SECTOR_SIZE]);
    }

    #[test]
    fn rejects_invalid_sector_buffer_lengths_before_io() {
        let geometry = Geometry::new(1, 1, 1).unwrap();
        let mut disk = open_image(make_image(geometry), VhdAccess::ReadWrite).unwrap();

        let error = disk.read_sector_lba(0, &mut [0; SECTOR_SIZE - 1]).unwrap_err();

        match error {
            VhdError::InvalidSectorBufferLength { expected, actual } => {
                assert_eq!(expected, SECTOR_SIZE);
                assert_eq!(actual, SECTOR_SIZE - 1);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn rejects_out_of_range_lba_and_chs_addresses() {
        let geometry = Geometry::new(2, 2, 4).unwrap();
        let mut disk = open_image(make_image(geometry), VhdAccess::ReadWrite).unwrap();
        let mut buffer = [0; SECTOR_SIZE];

        assert!(matches!(
            disk.read_sector_lba(16, &mut buffer),
            Err(VhdError::LbaOutOfRange { .. })
        ));
        assert!(matches!(
            disk.read_sector_chs(0, 2, 0, &mut buffer),
            Err(VhdError::ChsOutOfRange { .. })
        ));
        assert!(matches!(
            disk.read_sector_chs(0, 0, 4, &mut buffer),
            Err(VhdError::ChsOutOfRange { .. })
        ));
    }

    #[test]
    fn rejects_invalid_footer_checksum() {
        let geometry = Geometry::new(1, 1, 1).unwrap();
        let mut image = make_image(geometry);
        let footer_offset = image.len() - FOOTER_LEN;
        image[footer_offset + 28] ^= 0xff;

        let error = open_image(image, VhdAccess::ReadWrite).err().unwrap();

        assert!(matches!(error, VhdError::InvalidChecksum { .. }));
    }

    #[test]
    fn rejects_file_length_that_disagrees_with_footer() {
        let geometry = Geometry::new(2, 1, 1).unwrap();
        let image = make_image_with_declared_size(geometry, SECTOR_SIZE as u64);

        let error = open_image(image, VhdAccess::ReadWrite).err().unwrap();

        assert!(matches!(error, VhdError::InvalidFileLength { .. }));
    }
}

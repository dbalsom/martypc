use std::{
    fs,
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use binrw::{BinRead, BinWrite};
use fatfs::{
    DefaultTimeProvider,
    FatType,
    FileAttributes,
    FileSystem,
    FormatVolumeOptions,
    FsOptions,
    LossyOemCpConverter,
    StdIoWrapper,
    format_volume,
};

use crate::{
    bios::FatBootSectorPrefix,
    geometry::Geometry,
    partition::{Partition, PartitionIo},
};

const BOOT_SECTOR_SIZE: usize = 512;
const BPB_OFFSET: usize = 11;
const COMMON_BPB_END: usize = 36;
const FAT12_16_EXTENDED_BPB_SIGNATURE_OFFSET: usize = 38;
const FAT12_16_BOOT_CODE_OFFSET: usize = 62;
const FAT32_BOOT_CODE_OFFSET: usize = 90;
const MICROSOFT_BOOT_NAMES: &[u8; 22] = b"IO      SYSMSDOS   SYS";
const IBM_BOOT_NAMES: &[u8; 22] = b"IBMBIO  SYSIBMDOS  SYS";
const INVALID_FAT_NAME_CHARACTERS: &[u8] = b"\"*+,./:;<=>?[\\]|";

pub fn parse_volume_label(label: &str) -> Result<[u8; 11]> {
    let label = label.trim();
    ensure!(!label.is_empty(), "volume label cannot be empty");
    ensure!(label.len() <= 11, "volume label cannot exceed 11 characters");
    ensure!(label.is_ascii(), "volume label must contain only ASCII characters");
    ensure!(
        label
            .bytes()
            .all(|byte| (0x20..=0x7e).contains(&byte) && !INVALID_FAT_NAME_CHARACTERS.contains(&byte)),
        "volume label contains a character not supported by FAT"
    );

    let mut encoded = [b' '; 11];
    encoded[..label.len()].copy_from_slice(label.to_ascii_uppercase().as_bytes());
    Ok(encoded)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemFileKind {
    Microsoft,
    Ibm,
}

#[derive(Debug)]
pub struct SystemFiles {
    kind:   SystemFileKind,
    first:  PathBuf,
    second: PathBuf,
}

impl SystemFiles {
    fn destination_names(&self) -> (&'static str, &'static str) {
        match self.kind {
            SystemFileKind::Microsoft => ("IO.SYS", "MSDOS.SYS"),
            SystemFileKind::Ibm => ("IBMBIO.SYS", "IBMDOS.SYS"),
        }
    }
}

pub fn find_system_files(source: &Path) -> Result<SystemFiles> {
    let mut io_sys = None;
    let mut msdos_sys = None;
    let mut ibmbio_sys = None;
    let mut ibmdos_sys = None;

    for entry in fs::read_dir(source).with_context(|| format!("could not read {}", source.display()))? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_ascii_uppercase)
        else {
            continue;
        };
        let slot = match name.as_str() {
            "IO.SYS" => &mut io_sys,
            "MSDOS.SYS" => &mut msdos_sys,
            "IBMBIO.SYS" => &mut ibmbio_sys,
            "IBMDOS.SYS" => &mut ibmdos_sys,
            _ => continue,
        };
        ensure!(
            entry.file_type()?.is_file(),
            "system file {} is not a regular file",
            entry.path().display()
        );
        ensure!(slot.is_none(), "multiple source files match system filename {name}");
        *slot = Some(entry.path());
    }

    let microsoft_complete = io_sys.is_some() && msdos_sys.is_some();
    let microsoft_present = io_sys.is_some() || msdos_sys.is_some();
    let ibm_complete = ibmbio_sys.is_some() && ibmdos_sys.is_some();
    let ibm_present = ibmbio_sys.is_some() || ibmdos_sys.is_some();

    if microsoft_complete && !ibm_present {
        return Ok(SystemFiles {
            kind:   SystemFileKind::Microsoft,
            first:  io_sys.expect("pair was checked"),
            second: msdos_sys.expect("pair was checked"),
        });
    }
    if ibm_complete && !microsoft_present {
        return Ok(SystemFiles {
            kind:   SystemFileKind::Ibm,
            first:  ibmbio_sys.expect("pair was checked"),
            second: ibmdos_sys.expect("pair was checked"),
        });
    }

    let mut found = Vec::new();
    for (name, present) in [
        ("IO.SYS", io_sys.is_some()),
        ("MSDOS.SYS", msdos_sys.is_some()),
        ("IBMBIO.SYS", ibmbio_sys.is_some()),
        ("IBMDOS.SYS", ibmdos_sys.is_some()),
    ] {
        if present {
            found.push(name);
        }
    }
    bail!(
        "system files in {} must be exactly one complete pair: IO.SYS + MSDOS.SYS or \
         IBMBIO.SYS + IBMDOS.SYS (found {})",
        source.display(),
        if found.is_empty() {
            "none".to_owned()
        }
        else {
            found.join(", ")
        }
    )
}

pub fn patch_boot_sector_system_names(
    boot_sector: &mut [u8; BOOT_SECTOR_SIZE],
    system_files: &SystemFiles,
) -> Result<()> {
    let code = &boot_sector[FAT12_16_BOOT_CODE_OFFSET..BOOT_SECTOR_SIZE - 2];
    let microsoft_matches = code
        .windows(MICROSOFT_BOOT_NAMES.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == MICROSOFT_BOOT_NAMES).then_some(offset))
        .collect::<Vec<_>>();
    let ibm_matches = code
        .windows(IBM_BOOT_NAMES.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == IBM_BOOT_NAMES).then_some(offset))
        .collect::<Vec<_>>();
    ensure!(
        microsoft_matches.len() <= 1 && ibm_matches.len() <= 1,
        "boot sector contains multiple system-file name pairs"
    );
    ensure!(
        microsoft_matches.is_empty() || ibm_matches.is_empty(),
        "boot sector contains both Microsoft and IBM system-file name pairs"
    );

    let (from, to, match_offset) = match system_files.kind {
        SystemFileKind::Microsoft => {
            if let Some(offset) = ibm_matches.first() {
                (IBM_BOOT_NAMES, MICROSOFT_BOOT_NAMES, Some(*offset))
            }
            else {
                (IBM_BOOT_NAMES, MICROSOFT_BOOT_NAMES, None)
            }
        }
        SystemFileKind::Ibm => {
            if let Some(offset) = microsoft_matches.first() {
                (MICROSOFT_BOOT_NAMES, IBM_BOOT_NAMES, Some(*offset))
            }
            else if ibm_matches.is_empty() {
                bail!("custom boot sector does not contain the IO.SYS + MSDOS.SYS filename pair");
            }
            else {
                (MICROSOFT_BOOT_NAMES, IBM_BOOT_NAMES, None)
            }
        }
    };
    if let Some(offset) = match_offset {
        let start = FAT12_16_BOOT_CODE_OFFSET + offset;
        boot_sector[start..start + from.len()].copy_from_slice(to);
    }
    Ok(())
}

pub fn find_boot_sector(source: &Path) -> Result<Option<[u8; BOOT_SECTOR_SIZE]>> {
    find_sector_file(source, "boot sector", is_boot_sector_name)
}

pub fn find_mbr(source: &Path) -> Result<Option<[u8; BOOT_SECTOR_SIZE]>> {
    find_sector_file(source, "MBR", is_mbr_name)
}

fn find_sector_file(
    source: &Path,
    description: &str,
    matches_name: fn(&str) -> bool,
) -> Result<Option<[u8; BOOT_SECTOR_SIZE]>> {
    let mut matches = Vec::new();
    for entry in fs::read_dir(source).with_context(|| format!("could not read {}", source.display()))? {
        let entry = entry?;
        if entry.file_type()?.is_file() && matches_name(&entry.file_name().to_string_lossy()) {
            matches.push(entry.path());
        }
    }
    matches.sort();
    ensure!(
        matches.len() <= 1,
        "multiple {description} files found in {}: {}",
        source.display(),
        matches
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let Some(path) = matches.pop()
    else {
        return Ok(None);
    };
    let bytes = fs::read(&path).with_context(|| format!("could not read {description} {}", path.display()))?;
    ensure!(
        bytes.len() == BOOT_SECTOR_SIZE,
        "{description} {} must be exactly 512 bytes (found {})",
        path.display(),
        bytes.len()
    );
    Ok(Some(bytes.try_into().expect("length was checked")))
}

fn is_boot_sector_name(name: &str) -> bool {
    name == "bootsector.bin" || name.ends_with("_bootsector.bin")
}

fn is_mbr_name(name: &str) -> bool {
    name == "mbr.bin" || name.ends_with("_mbr.bin")
}

fn is_build_metadata_name(name: &str) -> bool {
    is_boot_sector_name(name) || is_mbr_name(name)
}

pub fn format_and_copy<T: Read + Write + Seek>(
    disk: &mut T,
    partition: &Partition,
    source: Option<&Path>,
    geometry: Geometry,
    boot_sector: Option<&[u8; BOOT_SECTOR_SIZE]>,
    system_files: Option<&SystemFiles>,
    volume_label: Option<&[u8; 11]>,
) -> Result<FatType> {
    let mut volume = PartitionIo::new(disk, partition)?;
    {
        let mut storage = StdIoWrapper::new(&mut volume);
        let mut format_options = FormatVolumeOptions::new()
            .sectors_per_track(u16::from(geometry.sectors()))
            .heads(u16::from(geometry.heads()));
        if let Some(label) = volume_label {
            format_options = format_options.volume_label(*label);
        }
        format_volume(&mut storage, format_options)?;
    }
    volume.rewind()?;
    let fs = FileSystem::new(&mut volume, FsOptions::new())?;
    let fat_type = fs.fat_type();
    if let Some(system_files) = system_files {
        copy_system_files(system_files, &fs.root_dir())?;
    }
    if let Some(source) = source {
        copy_directory(source, &fs.root_dir(), true)?;
    }
    fs.unmount()?;
    if volume_label.is_some() && system_files.is_some() {
        move_volume_label_after_system_files(&mut volume, fat_type)?;
    }

    // fatfs formats a standalone volume and leaves BPB_HiddSec at zero. DOS
    // requires it to contain the partition's absolute starting LBA.
    let mut volume = PartitionIo::new(disk, partition)?;
    patch_and_write_boot_sector(&mut volume, 0, partition.start_lba, fat_type, boot_sector)?;
    if fat_type == FatType::Fat32 {
        // fatfs uses the conventional backup boot sector at relative sector 6.
        patch_and_write_boot_sector(
            &mut volume,
            6 * BOOT_SECTOR_SIZE as u64,
            partition.start_lba,
            fat_type,
            boot_sector,
        )?;
    }
    Ok(fat_type)
}

fn move_volume_label_after_system_files<T: Read + Write + Seek>(volume: &mut T, fat_type: FatType) -> Result<()> {
    volume.rewind()?;
    let mut boot_sector = [0u8; BOOT_SECTOR_SIZE];
    volume.read_exact(&mut boot_sector)?;
    let bytes_per_sector = u64::from(u16::from_le_bytes(boot_sector[11..13].try_into()?));
    let reserved_sectors = u32::from(u16::from_le_bytes(boot_sector[14..16].try_into()?));
    let fats = u32::from(boot_sector[16]);
    let sectors_per_fat = match fat_type {
        FatType::Fat12 | FatType::Fat16 => u32::from(u16::from_le_bytes(boot_sector[22..24].try_into()?)),
        FatType::Fat32 => u32::from_le_bytes(boot_sector[36..40].try_into()?),
    };
    let mut root_sector = reserved_sectors + fats * sectors_per_fat;
    if fat_type == FatType::Fat32 {
        let sectors_per_cluster = u32::from(boot_sector[13]);
        let root_cluster = u32::from_le_bytes(boot_sector[44..48].try_into()?);
        ensure!(root_cluster >= 2, "invalid FAT32 root-directory cluster");
        root_sector += (root_cluster - 2) * sectors_per_cluster;
    }

    let root_offset = u64::from(root_sector) * bytes_per_sector;
    volume.seek(std::io::SeekFrom::Start(root_offset))?;
    let mut entries = [[0u8; 32]; 3];
    for entry in &mut entries {
        volume.read_exact(entry)?;
    }
    ensure!(
        entries[0][11] == FileAttributes::VOLUME_ID.bits(),
        "formatted FAT volume-label entry is missing"
    );
    ensure!(
        entries[1][11] == (FileAttributes::READ_ONLY | FileAttributes::HIDDEN | FileAttributes::SYSTEM).bits()
            && entries[2][11] == (FileAttributes::READ_ONLY | FileAttributes::HIDDEN | FileAttributes::SYSTEM).bits(),
        "DOS system files do not immediately follow the FAT volume-label entry"
    );

    volume.seek(std::io::SeekFrom::Start(root_offset))?;
    volume.write_all(&entries[1])?;
    volume.write_all(&entries[2])?;
    volume.write_all(&entries[0])?;
    Ok(())
}

fn copy_system_files<T: Read + Write + Seek>(
    system_files: &SystemFiles,
    destination: &fatfs::Dir<'_, StdIoWrapper<&mut PartitionIo<'_, T>>, DefaultTimeProvider, LossyOemCpConverter>,
) -> Result<()> {
    let (first_name, second_name) = system_files.destination_names();
    let attributes = FileAttributes::READ_ONLY | FileAttributes::HIDDEN | FileAttributes::SYSTEM;
    copy_file(&system_files.first, destination, first_name, attributes)?;
    copy_file(&system_files.second, destination, second_name, attributes)?;
    Ok(())
}

fn patch_and_write_boot_sector<T: Read + Write + Seek>(
    volume: &mut T,
    boot_sector_offset: u64,
    start_lba: u32,
    fat_type: FatType,
    supplied: Option<&[u8; BOOT_SECTOR_SIZE]>,
) -> Result<()> {
    volume.seek(std::io::SeekFrom::Start(boot_sector_offset))?;
    let mut formatted = [0u8; BOOT_SECTOR_SIZE];
    volume.read_exact(&mut formatted)?;

    let mut prefix = FatBootSectorPrefix::read_le(&mut std::io::Cursor::new(&formatted))?;
    prefix.bpb3.hidden_sectors = start_lba;
    prefix.write_le(&mut std::io::Cursor::new(&mut formatted[..]))?;

    let mut patched = supplied.copied().unwrap_or(formatted);
    if let Some(supplied) = supplied {
        let boot_code_offset = match fat_type {
            // DOS 3.x boot sectors predate the FAT12/FAT16 extended BPB and
            // use bytes 36 onward for bootstrap data. Preserve that region
            // unless the supplied sector advertises an extended BPB.
            FatType::Fat12 | FatType::Fat16
                if matches!(supplied[FAT12_16_EXTENDED_BPB_SIGNATURE_OFFSET], 0x28 | 0x29) =>
            {
                FAT12_16_BOOT_CODE_OFFSET
            }
            FatType::Fat12 | FatType::Fat16 => COMMON_BPB_END,
            FatType::Fat32 => FAT32_BOOT_CODE_OFFSET,
        };
        patched[BPB_OFFSET..boot_code_offset].copy_from_slice(&formatted[BPB_OFFSET..boot_code_offset]);
    }
    volume.seek(std::io::SeekFrom::Start(boot_sector_offset))?;
    volume.write_all(&patched)?;
    Ok(())
}

fn copy_directory<T: Read + Write + Seek>(
    source: &Path,
    destination: &fatfs::Dir<'_, StdIoWrapper<&mut PartitionIo<'_, T>>, DefaultTimeProvider, LossyOemCpConverter>,
    source_root: bool,
) -> Result<()> {
    for entry in fs::read_dir(source).with_context(|| format!("could not read {}", source.display()))? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("FAT filenames must be Unicode: {}", entry.path().display()))?;
        if source_root && entry.file_type()?.is_file() && is_build_metadata_name(&name) {
            continue;
        }
        if source_root && is_system_file_name(&name) {
            continue;
        }
        ensure!(name != "." && name != "..", "invalid FAT filename: {name}");
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            destination.create_dir(&name)?;
            let child = destination.open_dir(&name)?;
            copy_directory(&entry.path(), &child, false)?;
        }
        else if file_type.is_file() {
            copy_file(&entry.path(), destination, &name, FileAttributes::empty())?;
        }
    }
    Ok(())
}

fn is_system_file_name(name: &str) -> bool {
    matches!(
        name.to_ascii_uppercase().as_str(),
        "IO.SYS" | "MSDOS.SYS" | "IBMBIO.SYS" | "IBMDOS.SYS"
    )
}

fn copy_file<T: Read + Write + Seek>(
    source: &Path,
    destination: &fatfs::Dir<'_, StdIoWrapper<&mut PartitionIo<'_, T>>, DefaultTimeProvider, LossyOemCpConverter>,
    name: &str,
    attributes: FileAttributes,
) -> Result<()> {
    let mut input =
        fs::File::open(source).with_context(|| format!("could not open source file {}", source.display()))?;
    let mut output = destination
        .create_file(name)
        .with_context(|| format!("could not create FAT file {name}"))?;
    output.set_attributes(attributes);
    std::io::copy(&mut input, &mut output)
        .with_context(|| format!("could not copy {} to FAT file {name}", source.display()))?;
    output.flush()?;
    Ok(())
}

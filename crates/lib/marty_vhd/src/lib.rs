#[cfg(feature = "builder")]
mod bios;
#[cfg(feature = "builder")]
mod fat;
mod geometry;
#[cfg(feature = "builder")]
mod partition;
mod vhd;

#[cfg(feature = "builder")]
use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};

#[cfg(feature = "builder")]
use anyhow::{Context, Result, ensure};
pub use geometry::{Geometry, SECTOR_SIZE};
#[cfg(feature = "builder")]
use partition::{Mbr, Partition, PartitionKind};
pub use vhd::{VhdAccess, VhdError, VhdIo, VhdMetadata, VirtualHardDisk};

#[cfg(feature = "builder")]
const DEFAULT_BOOT_SECTOR: &[u8; 512] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/dos5_bootsector.bin"));
#[cfg(feature = "builder")]
const DEFAULT_MBR: &[u8; 512] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/dos5_mbr.bin"));

/// Builder for creating a fixed-size FAT VHD from a host directory.
#[cfg(feature = "builder")]
pub struct VhdBuilder {
    output: PathBuf,
    geometry: Geometry,
    partitioned: bool,
    formatted: bool,
    source: Option<PathBuf>,
    label: Option<String>,
}

#[cfg(feature = "builder")]
impl VhdBuilder {
    pub fn new(output: impl Into<PathBuf>, geometry: Geometry) -> Self {
        Self {
            output: output.into(),
            geometry,
            partitioned: false,
            formatted: false,
            source: None,
            label: None,
        }
    }

    #[must_use]
    pub fn partitioned(mut self, state: bool) -> Self {
        self.partitioned = state;
        self
    }

    /// Format the VHD as FAT, optionally populating it from a host directory.
    #[must_use]
    pub fn formatted(mut self, source: Option<PathBuf>) -> Self {
        self.formatted = true;
        self.source = source;
        self
    }

    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn build(self) -> Result<()> {
        ensure!(
            self.partitioned || !self.formatted,
            "cannot format an unpartitioned VHD"
        );
        ensure!(
            self.formatted || self.label.is_none(),
            "a volume label requires a formatted VHD"
        );
        if let Some(source) = &self.source {
            ensure!(source.is_dir(), "source is not a directory: {}", source.display());
        }

        let system_files = self.source.as_deref().map(fat::find_system_files).transpose()?;
        let volume_label = self.label.as_deref().map(fat::parse_volume_label).transpose()?;
        let mut boot_sector = match &self.source {
            Some(source) => fat::find_boot_sector(source)?.unwrap_or(*DEFAULT_BOOT_SECTOR),
            None => *DEFAULT_BOOT_SECTOR,
        };
        if let Some(system_files) = &system_files {
            fat::patch_boot_sector_system_names(&mut boot_sector, system_files)?;
        }
        let mbr = match (self.partitioned, &self.source) {
            (true, Some(source)) => fat::find_mbr(source)?.unwrap_or(*DEFAULT_MBR),
            _ => *DEFAULT_MBR,
        };

        let mut output = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&self.output)
            .with_context(|| format!("could not create {}", self.output.display()))?;

        let result = build_into(
            &mut output,
            &self,
            &boot_sector,
            &mbr,
            system_files.as_ref(),
            volume_label.as_ref(),
        );
        if result.is_err() {
            drop(output);
            let _ = std::fs::remove_file(&self.output);
        }
        result
    }
}

#[cfg(feature = "builder")]
pub fn update(source: PathBuf, output: PathBuf, label: Option<String>) -> Result<()> {
    ensure!(source.is_dir(), "source is not a directory: {}", source.display());
    ensure!(output.is_file(), "VHD does not exist: {}", output.display());

    let geometry = vhd::read_geometry(&output)
        .with_context(|| format!("could not read VHD geometry from {}", output.display()))?;
    let temp_parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let temp_dir = tempfile::Builder::new()
        .prefix(".marty_vhd-update-")
        .tempdir_in(temp_parent)
        .with_context(|| format!("could not create temporary directory in {}", temp_parent.display()))?;
    let temp_output = temp_dir.path().join("replacement.vhd");

    let mut builder = VhdBuilder::new(temp_output.clone(), geometry)
        .partitioned(true)
        .formatted(Some(source));
    if let Some(label) = label {
        builder = builder.with_label(label);
    }
    builder.build()?;

    // Re-read the generated footer before touching the existing image.
    let replacement_geometry = vhd::read_geometry(&temp_output)?;
    ensure!(
        replacement_geometry == geometry,
        "replacement VHD geometry changed unexpectedly"
    );
    fs::copy(&temp_output, &output).with_context(|| format!("could not replace {}", output.display()))?;
    Ok(())
}

#[cfg(feature = "builder")]
fn build_into(
    output: &mut std::fs::File,
    builder: &VhdBuilder,
    boot_sector: &[u8; 512],
    supplied_mbr: &[u8; 512],
    system_files: Option<&fat::SystemFiles>,
    volume_label: Option<&[u8; 11]>,
) -> Result<()> {
    vhd::initialize(output, builder.geometry)?;

    // DOS-compatible partitioned images reserve the first track for the MBR and boot tools.
    let start_lba = if builder.partitioned {
        u32::from(builder.geometry.sectors())
    }
    else {
        0
    };
    ensure!(
        builder.geometry.total_sectors() > start_lba,
        "geometry is too small to contain a partition"
    );
    let sector_count = builder.geometry.total_sectors() - start_lba;
    let mut partition = Partition::new(start_lba, sector_count, PartitionKind::Fat16);

    if builder.formatted {
        let fat_type = fat::format_and_copy(
            output,
            &partition,
            builder.source.as_deref(),
            builder.geometry,
            Some(boot_sector),
            system_files,
            volume_label,
        )?;
        partition.kind = PartitionKind::from_fat_type(fat_type, sector_count);
    }
    if builder.partitioned {
        Mbr::new(vec![partition]).write(output, builder.geometry, Some(supplied_mbr))?;
    }
    vhd::write_footer(output, builder.geometry)?;
    output.sync_all()?;
    Ok(())
}

#[cfg(all(test, feature = "builder"))]
mod tests {
    use std::{
        fs,
        io::{Read, Seek, SeekFrom},
        path::{Path, PathBuf},
    };

    use fatfs::{FileSystem, FsOptions};

    use super::{
        DEFAULT_BOOT_SECTOR,
        DEFAULT_MBR,
        Geometry,
        VhdBuilder,
        partition::{Partition, PartitionIo, PartitionKind},
        update,
    };

    fn make_source(parent: &Path, name: &str, contents: &[u8]) -> PathBuf {
        let source = parent.join(name);
        fs::create_dir(&source).unwrap();
        write_microsoft_system_files(&source);
        fs::write(source.join("CONTENT.TXT"), contents).unwrap();
        source
    }

    fn write_microsoft_system_files(source: &Path) {
        fs::write(source.join("IO.SYS"), b"Microsoft BIOS").unwrap();
        fs::write(source.join("MSDOS.SYS"), b"Microsoft DOS").unwrap();
    }

    fn write_ibm_system_files(source: &Path) {
        fs::write(source.join("IBMBIO.SYS"), b"IBM BIOS").unwrap();
        fs::write(source.join("IBMDOS.SYS"), b"IBM DOS").unwrap();
    }

    fn root_entries(disk: &mut fs::File, start_lba: u32, count: usize) -> Vec<[u8; 32]> {
        let partition_offset = u64::from(start_lba) * 512;
        disk.seek(SeekFrom::Start(partition_offset)).unwrap();
        let mut boot_sector = [0u8; 512];
        disk.read_exact(&mut boot_sector).unwrap();
        let bytes_per_sector = u16::from_le_bytes(boot_sector[11..13].try_into().unwrap());
        let reserved_sectors = u16::from_le_bytes(boot_sector[14..16].try_into().unwrap());
        let fats = boot_sector[16];
        let sectors_per_fat = u16::from_le_bytes(boot_sector[22..24].try_into().unwrap());
        assert_ne!(sectors_per_fat, 0, "test helper requires FAT12 or FAT16");
        let root_sector = u32::from(reserved_sectors) + u32::from(fats) * u32::from(sectors_per_fat);
        disk.seek(SeekFrom::Start(
            partition_offset + u64::from(root_sector) * u64::from(bytes_per_sector),
        ))
        .unwrap();
        let mut entries = vec![0u8; count * 32];
        disk.read_exact(&mut entries).unwrap();
        entries
            .chunks_exact(32)
            .map(|entry| entry.try_into().unwrap())
            .collect()
    }

    fn first_root_names(disk: &mut fs::File, start_lba: u32) -> [[u8; 11]; 2] {
        let entries = root_entries(disk, start_lba, 2);
        [
            entries[0][..11].try_into().unwrap(),
            entries[1][..11].try_into().unwrap(),
        ]
    }

    fn first_root_attributes(disk: &mut fs::File, start_lba: u32) -> [u8; 2] {
        let entries = root_entries(disk, start_lba, 2);
        [entries[0][11], entries[1][11]]
    }

    #[test]
    fn builds_unpartitioned_unformatted_vhd() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("disk.vhd");
        let geometry = Geometry::new(306, 4, 17).unwrap();

        VhdBuilder::new(output.clone(), geometry).build().unwrap();

        assert_eq!(fs::metadata(&output).unwrap().len(), geometry.byte_len() + 512);
        assert_eq!(super::vhd::read_geometry(&output).unwrap(), geometry);
        let mut disk = fs::File::open(output).unwrap();
        let mut first_sector = [0u8; 512];
        disk.read_exact(&mut first_sector).unwrap();
        assert_eq!(first_sector, [0u8; 512]);
    }

    #[test]
    fn builds_partitioned_unformatted_vhd() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("disk.vhd");
        let geometry = Geometry::new(306, 4, 17).unwrap();

        VhdBuilder::new(output.clone(), geometry)
            .partitioned(true)
            .build()
            .unwrap();

        let mut disk = fs::File::open(output).unwrap();
        let mut mbr = [0u8; 512];
        disk.read_exact(&mut mbr).unwrap();
        assert_eq!(&mbr[510..], &[0x55, 0xaa]);
        assert_eq!(mbr[450], 0x06);
        assert_eq!(u32::from_le_bytes(mbr[454..458].try_into().unwrap()), 17);
        assert_eq!(
            u32::from_le_bytes(mbr[458..462].try_into().unwrap()),
            geometry.total_sectors() - 17
        );
    }

    #[test]
    fn builds_empty_formatted_filesystem() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("disk.vhd");
        let geometry = Geometry::new(306, 4, 17).unwrap();

        VhdBuilder::new(output.clone(), geometry)
            .partitioned(true)
            .formatted(None)
            .with_label("EMPTY")
            .build()
            .unwrap();

        let start_lba = u32::from(geometry.sectors());
        let mut disk = fs::OpenOptions::new().read(true).write(true).open(output).unwrap();
        let partition = Partition::new(start_lba, geometry.total_sectors() - start_lba, PartitionKind::Fat16);
        let volume = PartitionIo::new(&mut disk, &partition).unwrap();
        let filesystem = FileSystem::new(volume, FsOptions::new()).unwrap();
        assert_eq!(filesystem.volume_label_as_bytes(), b"EMPTY");
        assert!(filesystem.root_dir().open_file("IO.SYS").is_err());
        assert!(filesystem.root_dir().open_file("CONTENT.TXT").is_err());
    }

    #[test]
    fn rejects_formatting_an_unpartitioned_vhd() {
        let temp = tempfile::tempdir().unwrap();
        let source = make_source(temp.path(), "source", b"contents");
        let output = temp.path().join("disk.vhd");
        let geometry = Geometry::new(306, 4, 17).unwrap();

        let error = VhdBuilder::new(output.clone(), geometry)
            .partitioned(false)
            .formatted(Some(source))
            .build()
            .unwrap_err();

        assert!(error.to_string().contains("cannot format an unpartitioned VHD"));
        assert!(!output.exists());
    }

    #[test]
    fn writes_volume_label_after_system_file_entries() {
        let temp = tempfile::tempdir().unwrap();
        let source = make_source(temp.path(), "source", b"contents");
        let output = temp.path().join("disk.vhd");
        let geometry = Geometry::new(306, 4, 17).unwrap();

        VhdBuilder::new(output.clone(), geometry)
            .partitioned(true)
            .formatted(Some(source))
            .with_label("My Disk")
            .build()
            .unwrap();

        let start_lba = u32::from(geometry.sectors());
        let mut disk = fs::OpenOptions::new().read(true).write(true).open(output).unwrap();
        let entries = root_entries(&mut disk, start_lba, 3);
        assert_eq!(&entries[0][..11], b"IO      SYS");
        assert_eq!(&entries[1][..11], b"MSDOS   SYS");
        assert_eq!(&entries[2][..11], b"MY DISK    ");
        assert_eq!(entries[2][11], 0x08);

        let partition = Partition::new(start_lba, geometry.total_sectors() - start_lba, PartitionKind::Fat16);
        let volume = PartitionIo::new(&mut disk, &partition).unwrap();
        let filesystem = FileSystem::new(volume, FsOptions::new()).unwrap();
        assert_eq!(filesystem.volume_label_as_bytes(), b"MY DISK");
        assert_eq!(
            filesystem.read_volume_label_from_root_dir_as_bytes().unwrap(),
            Some(*b"MY DISK    ")
        );
    }

    #[test]
    fn rejects_invalid_volume_label_without_creating_output() {
        let temp = tempfile::tempdir().unwrap();
        let source = make_source(temp.path(), "source", b"contents");
        let output = temp.path().join("disk.vhd");

        let error = VhdBuilder::new(output.clone(), Geometry::new(306, 4, 17).unwrap())
            .partitioned(true)
            .formatted(Some(source))
            .with_label("TOO-LONG-LABEL")
            .build()
            .unwrap_err();

        assert!(error.to_string().contains("cannot exceed 11 characters"));
        assert!(!output.exists());
    }

    #[test]
    fn update_replaces_contents_and_reuses_geometry() {
        let temp = tempfile::tempdir().unwrap();
        let old_source = make_source(temp.path(), "old", b"old contents");
        let new_source = make_source(temp.path(), "new", b"new contents");
        let output = temp.path().join("disk.vhd");
        let geometry = Geometry::new(306, 4, 17).unwrap();
        VhdBuilder::new(output.clone(), geometry)
            .partitioned(true)
            .formatted(Some(old_source))
            .build()
            .unwrap();

        update(new_source, output.clone(), None).unwrap();

        assert_eq!(super::vhd::read_geometry(&output).unwrap(), geometry);
        let mut disk = fs::OpenOptions::new().read(true).write(true).open(output).unwrap();
        let partition = Partition::new(
            u32::from(geometry.sectors()),
            geometry.total_sectors() - u32::from(geometry.sectors()),
            PartitionKind::Fat16,
        );
        let volume = PartitionIo::new(&mut disk, &partition).unwrap();
        let filesystem = FileSystem::new(volume, FsOptions::new()).unwrap();
        let mut file = filesystem.root_dir().open_file("CONTENT.TXT").unwrap();
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).unwrap();
        assert_eq!(contents, b"new contents");
    }

    #[test]
    fn failed_update_preserves_existing_vhd() {
        let temp = tempfile::tempdir().unwrap();
        let old_source = make_source(temp.path(), "old", b"keep me");
        let oversized_source = temp.path().join("oversized");
        fs::create_dir(&oversized_source).unwrap();
        write_microsoft_system_files(&oversized_source);
        let oversized = fs::File::create(oversized_source.join("TOO_BIG.BIN")).unwrap();
        oversized.set_len(20 * 1024 * 1024).unwrap();
        let output = temp.path().join("disk.vhd");
        VhdBuilder::new(output.clone(), Geometry::new(306, 4, 17).unwrap())
            .partitioned(true)
            .formatted(Some(old_source))
            .build()
            .unwrap();
        let before = fs::read(&output).unwrap();

        assert!(update(oversized_source, output.clone(), None).is_err());

        assert_eq!(fs::read(output).unwrap(), before);
    }

    #[test]
    fn injects_mbr_and_boot_sector_while_patching_disk_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let source = make_source(temp.path(), "source", b"contents");
        let boot_sector_name = "dos5_bootsector.bin";
        let mut supplied = [0xcc; 512];
        supplied[..11].copy_from_slice(b"\xeb\x3c\x90CUSTOM  ");
        supplied[38] = 0x29;
        supplied[510..].copy_from_slice(&[0x55, 0xaa]);
        fs::write(source.join(boot_sector_name), supplied).unwrap();
        let mbr_name = "dos5_mbr.bin";
        let mut supplied_mbr = [0xa5; 512];
        supplied_mbr[510..].copy_from_slice(&[0x12, 0x34]);
        fs::write(source.join(mbr_name), supplied_mbr).unwrap();
        let output = temp.path().join("disk.vhd");
        let geometry = Geometry::new(306, 4, 17).unwrap();

        VhdBuilder::new(output.clone(), geometry)
            .partitioned(true)
            .formatted(Some(source))
            .build()
            .unwrap();

        let start_lba = u32::from(geometry.sectors());
        let partition_sectors = geometry.total_sectors() - start_lba;
        let mut disk = fs::OpenOptions::new().read(true).write(true).open(output).unwrap();
        let mut actual_mbr = [0u8; 512];
        disk.read_exact(&mut actual_mbr).unwrap();
        assert_eq!(&actual_mbr[..446], &supplied_mbr[..446]);
        assert_eq!(&actual_mbr[510..], &supplied_mbr[510..]);
        assert_eq!(actual_mbr[446], 0x80);
        assert_eq!(u32::from_le_bytes(actual_mbr[454..458].try_into().unwrap()), 17);
        assert_eq!(
            u32::from_le_bytes(actual_mbr[458..462].try_into().unwrap()),
            partition_sectors
        );
        disk.seek(SeekFrom::Start(u64::from(start_lba) * 512)).unwrap();
        let mut actual = [0u8; 512];
        disk.read_exact(&mut actual).unwrap();

        assert_eq!(&actual[..11], &supplied[..11]);
        assert_eq!(&actual[62..], &supplied[62..]);
        assert_eq!(u16::from_le_bytes(actual[11..13].try_into().unwrap()), 512);
        assert_eq!(
            u16::from_le_bytes(actual[19..21].try_into().unwrap()),
            partition_sectors as u16
        );
        assert_eq!(u16::from_le_bytes(actual[24..26].try_into().unwrap()), 17);
        assert_eq!(u16::from_le_bytes(actual[26..28].try_into().unwrap()), 4);
        assert_eq!(u32::from_le_bytes(actual[28..32].try_into().unwrap()), 17);
        assert_eq!(
            first_root_names(&mut disk, start_lba),
            [*b"IO      SYS", *b"MSDOS   SYS"]
        );
        assert_eq!(first_root_attributes(&mut disk, start_lba), [0x07, 0x07]);

        let partition = Partition::new(start_lba, partition_sectors, PartitionKind::Fat16);
        let volume = PartitionIo::new(&mut disk, &partition).unwrap();
        let filesystem = FileSystem::new(volume, FsOptions::new()).unwrap();
        assert!(filesystem.root_dir().open_file(boot_sector_name).is_err());
        assert!(filesystem.root_dir().open_file(mbr_name).is_err());
    }

    #[test]
    fn preserves_dos33_bootstrap_data_while_patching_common_bpb() {
        let temp = tempfile::tempdir().unwrap();
        let source = make_source(temp.path(), "source", b"contents");
        let supplied = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/dos33_bootsector.bin"));
        fs::write(source.join("dos33_bootsector.bin"), supplied).unwrap();
        let output = temp.path().join("disk.vhd");
        let geometry = Geometry::new(615, 4, 26).unwrap();

        VhdBuilder::new(output.clone(), geometry)
            .partitioned(true)
            .formatted(Some(source))
            .build()
            .unwrap();

        let start_lba = u32::from(geometry.sectors());
        let mut disk = fs::File::open(output).unwrap();
        disk.seek(SeekFrom::Start(u64::from(start_lba) * 512)).unwrap();
        let mut actual = [0u8; 512];
        disk.read_exact(&mut actual).unwrap();

        assert_eq!(&actual[..11], &supplied[..11]);
        assert_eq!(&actual[36..], &supplied[36..]);
        assert_eq!(u16::from_le_bytes(actual[24..26].try_into().unwrap()), 26);
        assert_eq!(u16::from_le_bytes(actual[26..28].try_into().unwrap()), 4);
        assert_eq!(u32::from_le_bytes(actual[28..32].try_into().unwrap()), start_lba);
    }

    #[test]
    fn rejects_wrong_sized_boot_sector_without_creating_output() {
        let temp = tempfile::tempdir().unwrap();
        let source = make_source(temp.path(), "source", b"contents");
        fs::write(source.join("bootsector.bin"), [0u8; 511]).unwrap();
        let output = temp.path().join("disk.vhd");

        let error = VhdBuilder::new(output.clone(), Geometry::new(306, 4, 17).unwrap())
            .partitioned(true)
            .formatted(Some(source))
            .build()
            .unwrap_err();

        assert!(error.to_string().contains("must be exactly 512 bytes"));
        assert!(!output.exists());
    }

    #[test]
    fn patches_ibm_boot_names_and_places_ibm_system_files_first() {
        let temp = tempfile::tempdir().unwrap();
        let source = make_source(temp.path(), "source", b"contents");
        fs::remove_file(source.join("IO.SYS")).unwrap();
        fs::remove_file(source.join("MSDOS.SYS")).unwrap();
        write_ibm_system_files(&source);

        let mut supplied = [0xcc; 512];
        supplied[..11].copy_from_slice(b"\xeb\x3c\x90MSDOS5.0");
        supplied[0x1e6..0x1fc].copy_from_slice(b"IO      SYSMSDOS   SYS");
        supplied[510..].copy_from_slice(&[0x55, 0xaa]);
        fs::write(source.join("bootsector.bin"), supplied).unwrap();

        let output = temp.path().join("disk.vhd");
        let geometry = Geometry::new(306, 4, 17).unwrap();
        VhdBuilder::new(output.clone(), geometry)
            .partitioned(true)
            .formatted(Some(source))
            .build()
            .unwrap();

        let start_lba = u32::from(geometry.sectors());
        let mut disk = fs::OpenOptions::new().read(true).write(true).open(output).unwrap();
        disk.seek(SeekFrom::Start(u64::from(start_lba) * 512)).unwrap();
        let mut actual = [0u8; 512];
        disk.read_exact(&mut actual).unwrap();
        assert_eq!(&actual[0x1e6..0x1fc], b"IBMBIO  SYSIBMDOS  SYS");
        assert_eq!(
            first_root_names(&mut disk, start_lba),
            [*b"IBMBIO  SYS", *b"IBMDOS  SYS"]
        );
        assert_eq!(first_root_attributes(&mut disk, start_lba), [0x07, 0x07]);
    }

    #[test]
    fn rejects_both_system_file_pairs_without_creating_output() {
        let temp = tempfile::tempdir().unwrap();
        let source = make_source(temp.path(), "source", b"contents");
        write_ibm_system_files(&source);
        let output = temp.path().join("disk.vhd");

        let error = VhdBuilder::new(output.clone(), Geometry::new(306, 4, 17).unwrap())
            .partitioned(true)
            .formatted(Some(source))
            .build()
            .unwrap_err();

        assert!(error.to_string().contains("exactly one complete pair"));
        assert!(!output.exists());
    }

    #[test]
    fn rejects_mixed_system_file_pair_without_creating_output() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("IO.SYS"), b"Microsoft BIOS").unwrap();
        fs::write(source.join("IBMDOS.SYS"), b"IBM DOS").unwrap();
        let output = temp.path().join("disk.vhd");

        let error = VhdBuilder::new(output.clone(), Geometry::new(306, 4, 17).unwrap())
            .partitioned(true)
            .formatted(Some(source))
            .build()
            .unwrap_err();

        assert!(error.to_string().contains("exactly one complete pair"));
        assert!(!output.exists());
    }

    #[test]
    fn rejects_missing_system_file_pair_without_creating_output() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("CONTENT.TXT"), b"contents").unwrap();
        let output = temp.path().join("disk.vhd");

        let error = VhdBuilder::new(output.clone(), Geometry::new(306, 4, 17).unwrap())
            .partitioned(true)
            .formatted(Some(source))
            .build()
            .unwrap_err();

        assert!(error.to_string().contains("exactly one complete pair"));
        assert!(!output.exists());
    }

    #[test]
    fn uses_embedded_defaults_and_patches_boot_for_ibm_pair() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        write_ibm_system_files(&source);
        let output = temp.path().join("disk.vhd");
        let geometry = Geometry::new(306, 4, 17).unwrap();

        VhdBuilder::new(output.clone(), geometry)
            .partitioned(true)
            .formatted(Some(source))
            .build()
            .unwrap();

        let mut disk = fs::OpenOptions::new().read(true).write(true).open(output).unwrap();
        let mut actual_mbr = [0u8; 512];
        disk.read_exact(&mut actual_mbr).unwrap();
        assert_eq!(&actual_mbr[..446], &DEFAULT_MBR[..446]);
        assert_eq!(&actual_mbr[510..], &DEFAULT_MBR[510..]);

        let start_lba = u32::from(geometry.sectors());
        disk.seek(SeekFrom::Start(u64::from(start_lba) * 512)).unwrap();
        let mut actual_boot_sector = [0u8; 512];
        disk.read_exact(&mut actual_boot_sector).unwrap();
        let mut expected_boot_sector = *DEFAULT_BOOT_SECTOR;
        expected_boot_sector[0x1e6..0x1fc].copy_from_slice(b"IBMBIO  SYSIBMDOS  SYS");
        assert_eq!(&actual_boot_sector[..11], &expected_boot_sector[..11]);
        assert_eq!(&actual_boot_sector[62..], &expected_boot_sector[62..]);
        assert_eq!(
            first_root_names(&mut disk, start_lba),
            [*b"IBMBIO  SYS", *b"IBMDOS  SYS"]
        );
    }

    #[test]
    fn rejects_wrong_sized_mbr_without_creating_output() {
        let temp = tempfile::tempdir().unwrap();
        let source = make_source(temp.path(), "source", b"contents");
        fs::write(source.join("mbr.bin"), [0u8; 513]).unwrap();
        let output = temp.path().join("disk.vhd");

        let error = VhdBuilder::new(output.clone(), Geometry::new(306, 4, 17).unwrap())
            .partitioned(true)
            .formatted(Some(source))
            .build()
            .unwrap_err();

        assert!(error.to_string().contains("MBR"));
        assert!(error.to_string().contains("must be exactly 512 bytes"));
        assert!(!output.exists());
    }
}

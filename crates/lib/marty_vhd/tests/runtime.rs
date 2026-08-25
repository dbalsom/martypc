#![cfg(feature = "builder")]

use std::fs::OpenOptions;

use marty_vhd::{Geometry, SECTOR_SIZE, VhdAccess, VhdBuilder, VirtualHardDisk};

#[test]
fn opens_and_accesses_builder_output() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("runtime.vhd");
    let geometry = Geometry::new(306, 4, 17).unwrap();
    VhdBuilder::new(output.clone(), geometry).build().unwrap();

    let file = OpenOptions::new().read(true).write(true).open(output).unwrap();
    let mut disk = VirtualHardDisk::open(Box::new(file), VhdAccess::ReadWrite).unwrap();
    let written = [0x96; SECTOR_SIZE];
    let mut read = [0; SECTOR_SIZE];

    disk.write_sector_lba(geometry.total_sectors().into(), &written)
        .unwrap_err();
    disk.write_sector_lba(u64::from(geometry.total_sectors() - 1), &written)
        .unwrap();
    disk.read_sector_lba(u64::from(geometry.total_sectors() - 1), &mut read)
        .unwrap();

    assert_eq!(disk.geometry(), geometry);
    assert_eq!(read, written);
}

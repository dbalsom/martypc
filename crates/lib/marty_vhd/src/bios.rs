use binrw::{BinRead, BinWrite};

/// BIOS Parameter Block introduced in MS-DOS 2.0.
#[derive(BinRead, BinWrite, Copy, Clone, Debug, Default)]
#[brw(little)]
pub struct BiosParameterBlock2 {
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub number_of_fats: u8,
    pub root_entries: u16,
    pub total_sectors: u16,
    pub media_descriptor: u8,
    pub sectors_per_fat: u16,
}

/// BIOS Parameter Block extensions introduced in MS-DOS 3.0.
#[derive(BinRead, BinWrite, Copy, Clone, Debug, Default)]
#[brw(little)]
pub struct BiosParameterBlock3 {
    pub sectors_per_track: u16,
    pub number_of_heads:   u16,
    pub hidden_sectors:    u32,
}

/// BIOS Parameter Block extension introduced in MS-DOS 3.31.
#[derive(BinRead, BinWrite, Copy, Clone, Debug, Default)]
#[brw(little)]
pub struct BiosParameterBlock3_1 {
    pub total_sectors_32: u32,
}

/// The common prefix of a FAT boot sector.
///
/// FAT-specific extended fields immediately follow this structure and are
/// intentionally left under fatfs's control.
#[derive(BinRead, BinWrite, Debug)]
#[brw(little)]
pub struct FatBootSectorPrefix {
    pub jump_instruction: [u8; 3],
    pub oem_name: [u8; 8],
    pub bpb2: BiosParameterBlock2,
    pub bpb3: BiosParameterBlock3,
    pub bpb3_1: BiosParameterBlock3_1,
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Seek};

    use binrw::BinRead;

    use super::{BiosParameterBlock2, BiosParameterBlock3, BiosParameterBlock3_1, FatBootSectorPrefix};

    #[test]
    fn bpb2_is_exactly_13_bytes() {
        let mut bytes = Cursor::new([0u8; 13]);
        BiosParameterBlock2::read_le(&mut bytes).unwrap();
        assert_eq!(bytes.stream_position().unwrap(), 13);
    }

    #[test]
    fn bpb3_is_exactly_8_bytes() {
        let mut bytes = Cursor::new([0u8; 8]);
        BiosParameterBlock3::read_le(&mut bytes).unwrap();
        assert_eq!(bytes.stream_position().unwrap(), 8);
    }

    #[test]
    fn bpb3_1_is_exactly_4_bytes() {
        let mut bytes = Cursor::new([0u8; 4]);
        BiosParameterBlock3_1::read_le(&mut bytes).unwrap();
        assert_eq!(bytes.stream_position().unwrap(), 4);
    }

    #[test]
    fn fat_boot_sector_prefix_is_exactly_36_bytes() {
        let mut bytes = Cursor::new([0u8; 36]);
        FatBootSectorPrefix::read_le(&mut bytes).unwrap();
        assert_eq!(bytes.stream_position().unwrap(), 36);
    }
}

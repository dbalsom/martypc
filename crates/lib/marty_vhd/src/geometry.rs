use anyhow::{Result, ensure};

pub const SECTOR_SIZE: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Geometry {
    cylinders: u16,
    heads: u8,
    sectors: u8,
}

impl Geometry {
    pub fn new(cylinders: u16, heads: u8, sectors: u8) -> Result<Self> {
        ensure!(cylinders > 0, "cylinders must be non-zero");
        ensure!(heads > 0, "heads must be non-zero");
        ensure!(sectors > 0 && sectors <= 63, "sectors must be in 1..=63");
        ensure!(heads <= 16, "heads must be in 1..=16 for legacy CHS addressing");
        Ok(Self {
            cylinders,
            heads,
            sectors,
        })
    }

    pub fn cylinders(self) -> u16 {
        self.cylinders
    }

    pub fn heads(self) -> u8 {
        self.heads
    }

    pub fn sectors(self) -> u8 {
        self.sectors
    }

    pub fn total_sectors(self) -> u32 {
        u32::from(self.cylinders) * u32::from(self.heads) * u32::from(self.sectors)
    }

    pub fn byte_len(self) -> u64 {
        u64::from(self.total_sectors()) * SECTOR_SIZE as u64
    }

    /// Convert a zero-based CHS address to an LBA sector number.
    pub fn chs_to_lba(self, cylinder: u16, head: u8, sector: u8) -> Option<u32> {
        if cylinder >= self.cylinders || head >= self.heads || sector >= self.sectors {
            return None;
        }
        Some(
            (u32::from(cylinder) * u32::from(self.heads) + u32::from(head)) * u32::from(self.sectors)
                + u32::from(sector),
        )
    }

    pub fn lba_to_mbr_chs(self, lba: u32) -> [u8; 3] {
        let sectors_per_cylinder = u32::from(self.heads) * u32::from(self.sectors);
        let cylinder = (lba / sectors_per_cylinder).min(1023);
        let within = lba % sectors_per_cylinder;
        let head = (within / u32::from(self.sectors)).min(254) as u8;
        let sector = (within % u32::from(self.sectors)) + 1;
        [head, (sector as u8) | (((cylinder >> 8) as u8) << 6), cylinder as u8]
    }
}

#[cfg(test)]
mod tests {
    use super::Geometry;

    #[test]
    fn converts_lba_to_packed_mbr_chs() {
        let geometry = Geometry::new(306, 4, 17).unwrap();
        assert_eq!(geometry.lba_to_mbr_chs(0), [0, 1, 0]);
        assert_eq!(geometry.lba_to_mbr_chs(17), [1, 1, 0]);
        assert_eq!(geometry.lba_to_mbr_chs(68), [0, 1, 1]);
    }

    #[test]
    fn converts_zero_based_chs_to_lba() {
        let geometry = Geometry::new(306, 4, 17).unwrap();
        assert_eq!(geometry.chs_to_lba(0, 0, 0), Some(0));
        assert_eq!(geometry.chs_to_lba(1, 0, 0), Some(68));
        assert_eq!(geometry.chs_to_lba(305, 3, 16), Some(20_807));
        assert_eq!(geometry.chs_to_lba(306, 0, 0), None);
        assert_eq!(geometry.chs_to_lba(0, 4, 0), None);
        assert_eq!(geometry.chs_to_lba(0, 0, 17), None);
    }
}

use std::io::{Read, Seek, SeekFrom, Write};

use anyhow::{Result, ensure};
use binrw::BinWrite;
use fatfs::FatType;

use crate::geometry::{Geometry, SECTOR_SIZE};

#[derive(Clone, Copy, Debug)]
pub enum PartitionKind {
    Fat12,
    Fat16Small,
    Fat16,
    Fat32Lba,
}

impl PartitionKind {
    fn id(self) -> u8 {
        match self {
            Self::Fat12 => 0x01,
            Self::Fat16Small => 0x04,
            Self::Fat16 => 0x06,
            Self::Fat32Lba => 0x0c,
        }
    }
    pub fn from_fat_type(kind: FatType, sectors: u32) -> Self {
        match kind {
            FatType::Fat12 => Self::Fat12,
            FatType::Fat16 if sectors < 65_536 => Self::Fat16Small,
            FatType::Fat16 => Self::Fat16,
            FatType::Fat32 => Self::Fat32Lba,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Partition {
    pub start_lba: u32,
    pub sector_count: u32,
    pub kind: PartitionKind,
    pub bootable: bool,
}

impl Partition {
    pub fn new(start_lba: u32, sector_count: u32, kind: PartitionKind) -> Self {
        Self {
            start_lba,
            sector_count,
            kind,
            bootable: true,
        }
    }
    pub fn byte_offset(&self) -> u64 {
        u64::from(self.start_lba) * SECTOR_SIZE as u64
    }
    pub fn byte_len(&self) -> u64 {
        u64::from(self.sector_count) * SECTOR_SIZE as u64
    }
}

pub struct Mbr {
    partitions: Vec<Partition>,
}

#[derive(BinWrite, Clone, Copy, Debug, Default)]
#[bw(little)]
struct MbrPartitionEntry {
    boot_indicator: u8,
    start_chs: [u8; 3],
    partition_type: u8,
    end_chs: [u8; 3],
    start_lba: u32,
    sector_count: u32,
}

#[derive(BinWrite)]
#[bw(little)]
struct MbrSector {
    bootstrap_code: [u8; 446],
    partitions: [MbrPartitionEntry; 4],
    signature: u16,
}

impl Mbr {
    pub fn new(partitions: Vec<Partition>) -> Self {
        Self { partitions }
    }
    pub fn write(
        &self,
        disk: &mut (impl Write + Seek),
        geometry: Geometry,
        supplied: Option<&[u8; 512]>,
    ) -> Result<()> {
        ensure!(
            self.partitions.len() <= 4,
            "an MBR supports at most four primary partitions"
        );
        let mut entries = [MbrPartitionEntry::default(); 4];
        for (index, p) in self.partitions.iter().enumerate() {
            ensure!(
                p.sector_count > 0 && p.start_lba + p.sector_count <= geometry.total_sectors(),
                "partition is outside the disk"
            );
            entries[index] = MbrPartitionEntry {
                boot_indicator: if p.bootable { 0x80 } else { 0 },
                start_chs: geometry.lba_to_mbr_chs(p.start_lba),
                partition_type: p.kind.id(),
                end_chs: geometry.lba_to_mbr_chs(p.start_lba + p.sector_count - 1),
                start_lba: p.start_lba,
                sector_count: p.sector_count,
            };
        }
        let mbr = MbrSector {
            bootstrap_code: [0; 446],
            partitions: entries,
            signature: 0xaa55,
        };
        let mut generated = [0u8; 512];
        mbr.write_le(&mut std::io::Cursor::new(&mut generated[..]))?;
        let mut sector = supplied.copied().unwrap_or(generated);
        if supplied.is_some() {
            // The partition table is always derived from the actual image layout.
            // Bootstrap code and the signature remain supplied by the caller.
            sector[446..510].copy_from_slice(&generated[446..510]);
        }
        disk.seek(SeekFrom::Start(0))?;
        disk.write_all(&sector)?;
        Ok(())
    }
}

pub struct PartitionIo<'a, T> {
    inner: &'a mut T,
    start: u64,
    len:   u64,
    pos:   u64,
}

impl<'a, T: Seek> PartitionIo<'a, T> {
    pub fn new(inner: &'a mut T, partition: &Partition) -> Result<Self> {
        let start = partition.byte_offset();
        inner.seek(SeekFrom::Start(start))?;
        Ok(Self {
            inner,
            start,
            len: partition.byte_len(),
            pos: 0,
        })
    }
    fn seek_to(&mut self, pos: i128) -> std::io::Result<u64> {
        if !(0..=i128::from(self.len)).contains(&pos) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "seek outside partition",
            ));
        }
        self.pos = pos as u64;
        self.inner.seek(SeekFrom::Start(self.start + self.pos))?;
        Ok(self.pos)
    }
}
impl<T: Read + Seek> Read for PartitionIo<'_, T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let count = buf.len().min((self.len - self.pos) as usize);
        let n = self.inner.read(&mut buf[..count])?;
        self.pos += n as u64;
        Ok(n)
    }
}
impl<T: Write + Seek> Write for PartitionIo<'_, T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.len() as u64 > self.len - self.pos {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "write outside partition",
            ));
        }
        let n = self.inner.write(buf)?;
        self.pos += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
impl<T: Seek> Seek for PartitionIo<'_, T> {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        let pos = match from {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::Current(n) => i128::from(self.pos) + i128::from(n),
            SeekFrom::End(n) => i128::from(self.len) + i128::from(n),
        };
        self.seek_to(pos)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{Mbr, Partition, PartitionKind};
    use crate::Geometry;

    #[test]
    fn writes_dos_partition_entry_and_signature() {
        let geometry = Geometry::new(306, 4, 17).unwrap();
        let partition = Partition::new(17, geometry.total_sectors() - 17, PartitionKind::Fat16);
        let mut disk = Cursor::new(vec![0; 512]);

        Mbr::new(vec![partition]).write(&mut disk, geometry, None).unwrap();

        let bytes = disk.into_inner();
        assert_eq!(&bytes[510..512], &[0x55, 0xaa]);
        assert_eq!(bytes[446], 0x80);
        assert_eq!(bytes[450], 0x06);
        assert_eq!(u32::from_le_bytes(bytes[454..458].try_into().unwrap()), 17);
        assert_eq!(
            u32::from_le_bytes(bytes[458..462].try_into().unwrap()),
            geometry.total_sectors() - 17
        );
    }
}

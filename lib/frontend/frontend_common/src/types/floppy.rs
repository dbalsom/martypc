use std::{ffi::OsString, path::PathBuf};

#[derive(Clone)]
pub struct RelativeDirectory {
    pub full: PathBuf,
    pub relative: PathBuf,
    pub name: OsString,
}

pub enum FloppyImageSource {
    DiskImage(Vec<u8>),
    ZipArchive(Vec<u8>),
    KryoFluxSet(Vec<u8>, PathBuf),
}

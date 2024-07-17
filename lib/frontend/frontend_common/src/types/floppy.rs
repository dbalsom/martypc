use std::{ffi::OsString, path::PathBuf};

#[derive(Clone)]
pub struct RelativeDirectory {
    pub full: PathBuf,
    pub relative: PathBuf,
    pub name: OsString,
}

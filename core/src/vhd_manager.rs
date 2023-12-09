/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

    vhd_manager.rs

    Enumerate VHD images in the 'hdd' directory to allow disk image selection
    in the GUI.
*/

const DRIVE_MAX: usize = 4;

use core::fmt::Display;
use std::{
    collections::HashMap,
    ffi::OsString,
    fs,
    fs::File,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum VHDManagerError {
    DirNotFound,
    FileNotFound,
    FileReadError,
    InvalidDrive,
    DriveAlreadyLoaded,
}
impl std::error::Error for VHDManagerError {}
impl Display for VHDManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &*self {
            VHDManagerError::DirNotFound => write!(f, "The VHD directory was not found."),
            VHDManagerError::FileNotFound => {
                write!(f, "File not found error scanning VHD directory.")
            }
            VHDManagerError::FileReadError => write!(f, "File read error scanning VHD directory."),
            VHDManagerError::InvalidDrive => write!(f, "Specified drive out of range."),
            VHDManagerError::DriveAlreadyLoaded => write!(f, "Specified drive already loaded!"),
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct VHDFile {
    path: PathBuf,
    size: u64,
}

pub struct VHDManager {
    file_vec: Vec<VHDFile>,
    file_map: HashMap<OsString, VHDFile>,
    files_loaded: [Option<OsString>; DRIVE_MAX],
}

impl VHDManager {
    pub fn new() -> Self {
        Self {
            file_vec: Vec::new(),
            file_map: HashMap::new(),
            files_loaded: [None, None, None, None],
        }
    }

    pub fn scan_dir(&mut self, path: &Path) -> Result<bool, VHDManagerError> {
        // Read in directory entries within the provided path
        let dir = match fs::read_dir(path) {
            Ok(dir) => dir,
            Err(_) => return Err(VHDManagerError::DirNotFound),
        };

        let extensions = ["vhd"];

        // Scan through all entries in the directory
        for entry in dir {
            if let Ok(entry) = entry {
                if entry.path().is_file() {
                    if let Some(extension) = entry.path().extension() {
                        if extensions.contains(&extension.to_string_lossy().to_lowercase().as_ref()) {
                            println!(
                                "Found VHD image: {:?} size: {}",
                                entry.path(),
                                entry.metadata().unwrap().len()
                            );
                            self.file_vec.push(VHDFile {
                                path: entry.path(),
                                size: entry.metadata().unwrap().len(),
                            });

                            self.file_map.insert(
                                entry.file_name(),
                                VHDFile {
                                    path: entry.path(),
                                    size: entry.metadata().unwrap().len(),
                                },
                            );
                        }
                    }
                }
            }
        }
        Ok(true)
    }

    pub fn get_vhd_names(&self) -> Vec<OsString> {
        let mut vec: Vec<OsString> = Vec::new();
        for key in self.file_map.keys() {
            vec.push(key.clone());
        }
        vec.sort_by(|a, b| a.cmp(b));
        vec
    }

    pub fn is_vhd_loaded(&self, name: &OsString) -> Option<usize> {
        for i in 0..DRIVE_MAX {
            if let Some(drive) = &self.files_loaded[i] {
                if name.as_os_str() == drive.as_os_str() {
                    return Some(i);
                }
            }
        }
        None
    }

    pub fn load_vhd_file(&mut self, drive: usize, name: &OsString) -> Result<File, VHDManagerError> {
        if drive > 3 {
            return Err(VHDManagerError::InvalidDrive);
        }

        if let Some(vhd) = self.file_map.get(name) {
            let vhd_file_result = File::options().read(true).write(true).open(&vhd.path);

            match vhd_file_result {
                Ok(file) => {
                    log::debug!("Associating vhd: {} to drive: {}", name.to_string_lossy(), drive);

                    if let Some(_) = &self.files_loaded[drive] {
                        log::error!("VHD drive slot {} not empty!", drive);
                        return Err(VHDManagerError::DriveAlreadyLoaded);
                    }

                    if let Some(d) = self.is_vhd_loaded(name) {
                        log::error!("VHD already associated with drive {}! Release drive first.", d);
                        return Err(VHDManagerError::DriveAlreadyLoaded);
                    }

                    self.files_loaded[drive] = Some(name.clone());

                    return Ok(file);
                }
                Err(_e) => {
                    return Err(VHDManagerError::FileReadError);
                }
            }
        }
        Err(VHDManagerError::FileNotFound)
    }

    pub fn release_vhd(&mut self, drive: usize) {
        self.files_loaded[drive] = None;
    }
}

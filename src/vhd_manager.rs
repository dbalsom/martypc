/*
    MartyPC Emulator 
    (C)2023 Daniel Balsom
    https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

    --------------------------------------------------------------------------

    vhd_manager.rs

    Enumerate VHD images in the 'hdd' directory to allow disk image selection
    in the GUI.
*/

const DRIVE_MAX: usize = 4;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    ffi::OsString,
    fs,
    fs::File
};
use core::fmt::Display;

#[derive (Debug)]
pub enum VHDManagerError {
    DirNotFound,
    FileNotFound,
    FileReadError,
    InvalidDrive,
    DriveAlreadyLoaded,
}
impl std::error::Error for VHDManagerError{}
impl Display for VHDManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &*self {
            VHDManagerError::DirNotFound => write!(f, "The VHD directory was not found."),
            VHDManagerError::FileNotFound => write!(f, "File not found error scanning VHD directory."),
            VHDManagerError::FileReadError => write!(f, "File read error scanning VHD directory."),
            VHDManagerError::InvalidDrive => write!(f, "Specified drive out of range."),
            VHDManagerError::DriveAlreadyLoaded => write!(f, "Specified drive already loaded!"),
        }
    }
}

#[allow(dead_code)]
#[derive (Clone, Debug)]
pub struct VHDFile {
    path: PathBuf,
    size: u64
}

pub struct VHDManager {
    file_vec: Vec<VHDFile>,
    file_map: HashMap<OsString, VHDFile>,
    files_loaded: [Option<OsString>; DRIVE_MAX]
}

impl VHDManager {
    pub fn new() -> Self {
        Self {
            file_vec: Vec::new(),
            file_map: HashMap::new(),
            files_loaded: [ None, None, None, None ]
        }
    }

    pub fn scan_dir(&mut self, path: &Path) -> Result<bool, VHDManagerError> {

        // Read in directory entries within the provided path
        let dir = match fs::read_dir(path) {
            Ok(dir) => dir,
            Err(_) => return Err(VHDManagerError::DirNotFound)
        };

        let extensions = ["vhd"];

        // Scan through all entries in the directory
        for entry in dir {
            if let Ok(entry) = entry {
                if entry.path().is_file() {
                    if let Some(extension) = entry.path().extension() {
                        if extensions.contains(&extension.to_string_lossy().to_lowercase().as_ref()) {
                            println!("Found VHD image: {:?} size: {}", entry.path(), entry.metadata().unwrap().len());
                            self.file_vec.push( VHDFile {
                                path: entry.path(),
                                size: entry.metadata().unwrap().len()
                            });
        
                            self.file_map.insert(entry.file_name(), 
                                VHDFile { 
                                    path: entry.path(),
                                    size: entry.metadata().unwrap().len()
                                 });
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

                    return Some(i)
                }
            }
        }
        None
    }

    pub fn load_vhd_file(&mut self, drive: usize, name: &OsString ) -> Result<File, VHDManagerError> {

        if drive > 3 {
            return Err(VHDManagerError::InvalidDrive);
        }

        if let Some(vhd) = self.file_map.get(name) {

            let vhd_file_result = 
                File::options()
                    .read(true)
                    .write(true)
                    .open(&vhd.path);

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
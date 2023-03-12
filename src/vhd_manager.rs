/*
    vhd_manager.rs
    Enumerate VHD images in the /hdd directory to allow disk image selection

*/

use std::collections::HashMap;
use std::path::PathBuf;
use std::ffi::OsString;
use std::fs;
use std::fs::File;
use core::fmt::Display;

#[derive (Debug)]
pub enum VHDManagerError {
    DirNotFound,
    FileNotFound,
    FileReadError,
}
impl std::error::Error for VHDManagerError{}
impl Display for VHDManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &*self {
            VHDManagerError::DirNotFound => write!(f, "The VHD directory was not found."),
            VHDManagerError::FileNotFound => write!(f, "File not found error scanning VHD directory."),
            VHDManagerError::FileReadError => write!(f, "File read error scanning VHD directory."),
        }
    }
}

#[allow(dead_code)]
pub struct VHDFile {
    path: PathBuf,
    size: u64
}

pub struct VHDManager {
    file_vec: Vec<VHDFile>,
    file_map: HashMap<OsString, VHDFile>
}

impl VHDManager {
    pub fn new() -> Self {
        Self {
            file_vec: Vec::new(),
            file_map: HashMap::new()
        }
    }

    pub fn scan_dir(&mut self, path: &str) -> Result<bool, VHDManagerError> {

        // Read in directory entries within the provided path
        let dir = match fs::read_dir(path) {
            Ok(dir) => dir,
            Err(_) => return Err(VHDManagerError::DirNotFound)
        };

        // Scan through all entries in the directory
        for entry in dir {
            if let Ok(entry) = entry {
                if entry.path().is_file() {

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

    pub fn get_vhd_file(&self, name: &OsString ) -> Result<File, VHDManagerError> {

        if let Some(vhd) = self.file_map.get(name) {

            let vhd_file_result = File::options()
                .read(true)
                .write(true)
                .open(&vhd.path);
            match vhd_file_result {
                Ok(file) => {
                    return Ok(file);
                }
                Err(_e) => {
                    return Err(VHDManagerError::FileReadError);
                }
            }
        }
        Err(VHDManagerError::FileNotFound)
    }

}
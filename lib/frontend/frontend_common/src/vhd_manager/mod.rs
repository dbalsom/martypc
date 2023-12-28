/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2024 Daniel Balsom

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

    frontend_common::vhd_manager.rs

    Discover Vhd images in the 'hdd' resource and provide an interface
    for enumerating and loading them.

    Unlike most other resources, the core writes to Vhd files directly.
    Therefore, the Vhd manager is primarily responsible for enumerating file
    paths.

    Eventually I would like to have the front ends give the core a handle to an
    object implementing the Read and Write traits so that the core doesn't need
    to know whether it is operating on an in-memory image or file.
*/

const DRIVE_MAX: usize = 4;

use crate::resource_manager::{PathTreeNode, ResourceItem, ResourceManager};
use std::{
    collections::{BTreeMap, HashMap},
    ffi::OsString,
    fmt::Display,
    fs,
    fs::File,
    path::{Path, PathBuf},
};

use anyhow::Error;

#[derive(Debug)]
pub enum VhdManagerError {
    DirNotFound,
    FileNotFound,
    FileReadError,
    InvalidDrive,
    DriveAlreadyLoaded,
}
impl std::error::Error for VhdManagerError {}
impl Display for VhdManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &*self {
            VhdManagerError::DirNotFound => write!(f, "The Vhd directory was not found."),
            VhdManagerError::FileNotFound => {
                write!(f, "File not found error scanning Vhd directory.")
            }
            VhdManagerError::FileReadError => {
                write!(f, "File read error scanning Vhd directory.")
            }
            VhdManagerError::InvalidDrive => write!(f, "Specified drive out of range."),
            VhdManagerError::DriveAlreadyLoaded => {
                write!(f, "Specified drive already loaded!")
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct VhdFile {
    idx:  usize,
    name: OsString,
    path: PathBuf,
    size: u64,
}

pub struct VhdManager {
    files: Vec<ResourceItem>,
    image_vec: Vec<VhdFile>,
    image_map: HashMap<OsString, usize>,
    images_loaded: BTreeMap<usize, OsString>,
    extensions: Vec<OsString>,
}

impl VhdManager {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            image_vec: Vec::new(),
            image_map: HashMap::new(),
            images_loaded: BTreeMap::new(),
            extensions: vec![OsString::from("vhd")],
        }
    }

    pub fn set_extensions(&mut self, extensions: Option<Vec<String>>) {
        if let Some(extensions) = extensions {
            self.extensions = extensions
                .iter()
                .map(|ext| OsString::from(ext.to_lowercase()))
                .collect();
        }
    }

    pub fn scan_resource(&mut self, rm: &ResourceManager) -> Result<bool, Error> {
        // Clear and rebuild image lists.
        self.image_vec.clear();
        self.image_map.clear();

        // Retrieve all items from the floppy resource paths.
        let floppy_items = rm.enumerate_items("floppy", true, true, Some(self.extensions.clone()))?;

        // Index mapping between 'files' vec and 'image_vec' should be maintained.
        for item in floppy_items.iter() {
            let idx = self.image_vec.len();
            self.image_vec.push(VhdFile {
                idx,
                name: item.full_path.file_name().unwrap().to_os_string(),
                path: item.full_path.clone(),
                size: item.full_path.metadata().unwrap().len(),
            });

            self.image_map
                .insert(item.full_path.file_name().unwrap().to_os_string(), idx);
        }

        self.files = floppy_items;

        Ok(true)
    }

    pub fn make_tree(&mut self, rm: &ResourceManager) -> Result<PathTreeNode, Error> {
        let tree = rm.items_to_tree("floppy", &self.files)?;
        Ok(tree)
    }

    pub fn scan_paths(&mut self, paths: Vec<PathBuf>) -> Result<bool, crate::floppy_manager::FloppyError> {
        // Clear and rebuild image lists.
        self.image_vec.clear();
        self.image_map.clear();

        // Scan through all entries in the directory and find all files with matching extension
        for path in paths {
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if self.extensions.contains(&extension.to_ascii_lowercase()) {
                        println!(
                            "Found floppy image: {:?} size: {}",
                            path,
                            path.metadata().unwrap().len()
                        );

                        let idx = self.image_vec.len();
                        self.image_vec.push(VhdFile {
                            idx,
                            name: path.file_name().unwrap().to_os_string(),
                            path: path.clone(),
                            size: path.metadata().unwrap().len(),
                        });

                        self.image_map.insert(path.file_name().unwrap().to_os_string(), idx);
                    }
                }
            }
        }
        Ok(true)
    }

    pub fn get_vhd_names(&self) -> Vec<OsString> {
        let mut vec: Vec<OsString> = Vec::new();
        for (key, _val) in &self.image_map {
            vec.push(key.clone());
        }
        //vec.sort_by(|a, b| a.to_ascii_uppercase().cmp(&b.to_ascii_uppercase()));
        vec
    }

    pub fn get_vhd_name(&self, idx: usize) -> Option<OsString> {
        if idx >= self.image_vec.len() {
            return None;
        }
        Some(self.image_vec[idx].name.clone())
    }

    pub fn is_vhd_loaded(&self, name: &OsString) -> bool {
        if let Some(_entry) = self.image_map.get(name).and_then(|idx| self.image_vec.get(*idx)) {
            return true;
        }
        false
    }

    pub fn is_drive_loaded(&self, drive: usize) -> bool {
        if let Some(_entry) = self.images_loaded.get(&drive) {
            return true;
        }
        false
    }

    pub fn load_vhd_file_by_name(&mut self, drive: usize, name: &OsString) -> Result<File, VhdManagerError> {
        if let Some(vhd_idx) = self.image_map.get(name) {
            return self.load_vhd_file(drive, *vhd_idx);
        }
        Err(VhdManagerError::FileNotFound)
    }

    pub fn load_vhd_file(&mut self, drive: usize, idx: usize) -> Result<File, VhdManagerError> {
        if let Some(vhd) = self.image_vec.get(idx) {
            let vhd_file_result = File::options().read(true).write(true).open(&vhd.path);

            match vhd_file_result {
                Ok(file) => {
                    log::debug!("Associating vhd: {} to drive: {}", vhd.name.to_string_lossy(), drive);

                    if self.is_drive_loaded(drive) {
                        log::error!("VHD drive slot {} not empty!", drive);
                        return Err(VhdManagerError::DriveAlreadyLoaded);
                    }

                    if self.is_vhd_loaded(&vhd.name) {
                        log::error!("VHD already associated with drive! Release drive first.");
                        return Err(VhdManagerError::DriveAlreadyLoaded);
                    }

                    self.images_loaded.insert(drive, vhd.name.clone());

                    return Ok(file);
                }
                Err(_e) => {
                    return Err(VhdManagerError::FileReadError);
                }
            }
        }
        Err(VhdManagerError::FileNotFound)
    }

    pub fn release_vhd(&mut self, drive: usize) {
        self.images_loaded.remove(&drive);
    }
}

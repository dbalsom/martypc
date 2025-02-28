/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2025 Daniel Balsom

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

use crate::resource_manager::{PathTreeNode, ResourceItem, ResourceManager};
use anyhow::Error;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    ffi::OsString,
    fmt::Display,
    fs::File,
    path::PathBuf,
};

#[derive(Debug)]
pub enum VhdManagerError {
    DirNotFound,
    FileNotFound,
    FileReadError,
    InvalidDrive,
    DriveAlreadyLoaded,
    NameNotFound,
    IndexNotFound,
}
impl std::error::Error for VhdManagerError {}
impl Display for VhdManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &*self {
            VhdManagerError::DirNotFound => write!(f, "The Vhd directory was not found."),
            VhdManagerError::FileNotFound => {
                write!(f, "File not found scanning Vhd directory.")
            }
            VhdManagerError::FileReadError => {
                write!(f, "File read error scanning Vhd directory.")
            }
            VhdManagerError::InvalidDrive => write!(f, "Specified drive out of range."),
            VhdManagerError::DriveAlreadyLoaded => {
                write!(f, "Specified drive already loaded!")
            }
            VhdManagerError::NameNotFound => write!(f, "Specified VHD name not found."),
            VhdManagerError::IndexNotFound => write!(f, "Specified VHD index not found."),
        }
    }
}

#[derive(Clone, Debug)]
pub struct VhdFile {
    name: OsString,
    path: PathBuf,
    #[allow(unused)]
    size: u64,
}

pub struct VhdManager {
    files: Vec<ResourceItem>,
    image_vec: Vec<VhdFile>,
    image_map: HashMap<PathBuf, usize>,
    drives_loaded: BTreeMap<usize, PathBuf>,
    images_loaded: BTreeSet<PathBuf>,
    extensions: Vec<OsString>,
}

impl VhdManager {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            image_vec: Vec::new(),
            image_map: HashMap::new(),
            drives_loaded: BTreeMap::new(),
            images_loaded: BTreeSet::new(),
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

    pub fn scan_resource(&mut self, rm: &mut ResourceManager) -> Result<bool, Error> {
        // TODO: the *_loaded maps will be invalidated on scan, we should handle this properly.

        // Clear and rebuild image lists.
        self.image_vec.clear();
        self.image_map.clear();

        // Retrieve all items from the floppy resource paths.
        let floppy_items = rm.enumerate_items("hdd", None, true, true, Some(self.extensions.clone()))?;

        // Index mapping between 'files' vec and 'image_vec' should be maintained.
        for item in floppy_items.iter() {
            let idx = self.image_vec.len();
            self.image_vec.push(VhdFile {
                name: item.location.file_name().unwrap().to_os_string(),
                path: item.location.clone(),
                size: item.size.unwrap_or(0),
            });

            self.image_map.insert(item.location.clone(), idx);
        }

        self.files = floppy_items;

        Ok(true)
    }

    pub fn make_tree(&mut self, rm: &ResourceManager) -> Result<PathTreeNode, Error> {
        let tree = rm.items_to_tree("hdd", &self.files)?;
        Ok(tree)
    }

    pub fn get_vhd_names(&self) -> Vec<OsString> {
        let mut vec: Vec<OsString> = Vec::new();
        for image in self.image_vec.iter() {
            vec.push(image.name.clone());
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

    pub fn get_vhd_path(&self, idx: usize) -> Option<PathBuf> {
        if idx >= self.image_vec.len() {
            return None;
        }
        Some(self.image_vec[idx].path.clone())
    }

    pub fn is_vhd_available(&self, name: &PathBuf) -> bool {
        if let Some(entry) = self.image_map.get(name).and_then(|idx| self.image_vec.get(*idx)) {
            log::debug!("is_vhd_loaded(): confirming entry {}", entry.name.to_string_lossy());
            return true;
        }
        log::debug!("is_vhd_loaded(): vhd {} not loaded", name.to_string_lossy());
        false
    }

    pub fn is_vhd_loaded(&self, name: &PathBuf) -> bool {
        if self.images_loaded.contains(name) {
            log::debug!("is_vhd_loaded(): confirming entry {}", name.to_string_lossy());
            return true;
        }
        log::debug!("is_vhd_loaded(): vhd {:?} not loaded", name.file_name());
        false
    }

    pub fn is_drive_loaded(&self, drive: usize) -> bool {
        if let Some(_entry) = self.drives_loaded.get(&drive) {
            return true;
        }
        false
    }

    // pub fn load_vhd_file_by_name(&mut self, drive: usize, name: &OsString) -> Result<(File, usize), VhdManagerError> {
    //     if let Some(path) = self.find_first_name(name.clone()) {
    //         if let Some(vhd_idx) = self.image_map.get(&path) {
    //             match self.load_vhd_file(drive, vhd_idx) {
    //                 Ok(file) => {
    //                     return Ok((file, vhd_idx));
    //                 }
    //                 Err(e) => {
    //                     log::error!("Error loading VHD file: {}", e);
    //                     return Err(e);
    //                 }
    //             }
    //         }
    //     }
    //     Err(VhdManagerError::FileNotFound)
    // }

    /// Load a VHD file by its resource name and return a rust File handle.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_vhd_file_by_name(&mut self, drive: usize, name: &OsString) -> Result<(File, usize), VhdManagerError> {
        if let Some(path) = self.find_first_name(name.clone()) {
            let img_idx;

            if let Some(vhd_idx) = self.image_map.get(&path) {
                img_idx = *vhd_idx;
            }
            else {
                return Err(VhdManagerError::IndexNotFound);
            }

            match self.load_vhd_file(drive, img_idx) {
                Ok(file) => {
                    return Ok((file, img_idx));
                }
                Err(e) => {
                    log::error!("Error loading VHD file: {}", e);
                    return Err(e);
                }
            }
        }
        Err(VhdManagerError::FileNotFound)
    }

    /// Load a VHD file by its resource name and return a Vec<u8>.
    #[cfg(target_arch = "wasm32")]
    pub fn load_vhd_file_by_name(
        &mut self,
        rm: &mut ResourceManager,
        drive: usize,
        name: &OsString,
    ) -> Result<Vec<u8>, VhdManagerError> {
        if let Some(path) = self.find_first_name(name.clone()) {
            match rm.read_resource_from_path_blocking(path.clone()) {
                Ok(file) => {
                    if self.is_drive_loaded(drive) {
                        log::error!("VHD drive slot {} not empty!", drive);
                        return Err(VhdManagerError::DriveAlreadyLoaded);
                    }

                    if self.is_vhd_loaded(&path) {
                        log::error!("VHD already associated with drive! Release drive first.");
                        return Err(VhdManagerError::DriveAlreadyLoaded);
                    }

                    self.drives_loaded.insert(drive, path.clone());
                    self.images_loaded.insert(path.clone());
                    Ok(file)
                }
                Err(e) => {
                    log::error!("Error loading VHD file: {}", e);
                    Err(VhdManagerError::FileReadError)
                }
            }
        }
        else {
            Err(VhdManagerError::FileNotFound)
        }
    }

    pub fn find_first_name(&mut self, name: OsString) -> Option<PathBuf> {
        for path in self.image_map.keys() {
            if let Some(filename) = path.file_name() {
                if filename == name {
                    return Some(path.clone());
                }
            }
        }
        None
    }

    pub fn load_vhd_file(&mut self, drive: usize, idx: usize) -> Result<File, VhdManagerError> {
        if let Some(vhd) = self.image_vec.get(idx) {
            let vhd_file_result = File::options().read(true).write(true).open(&vhd.path);

            return match vhd_file_result {
                Ok(file) => {
                    log::debug!("Associating vhd: {} to drive: {}", vhd.name.to_string_lossy(), drive);

                    if self.is_drive_loaded(drive) {
                        log::error!("VHD drive slot {} not empty!", drive);
                        return Err(VhdManagerError::DriveAlreadyLoaded);
                    }

                    if self.is_vhd_loaded(&vhd.path) {
                        log::error!("VHD already associated with drive! Release drive first.");
                        return Err(VhdManagerError::DriveAlreadyLoaded);
                    }

                    self.drives_loaded.insert(drive, vhd.path.clone());
                    self.images_loaded.insert(vhd.path.clone());

                    Ok(file)
                }
                Err(e) => {
                    log::error!("load_vhd_file(): error opening file: {}", e);
                    Err(VhdManagerError::FileReadError)
                }
            };
        }
        Err(VhdManagerError::FileNotFound)
    }

    pub fn release_vhd(&mut self, drive: usize) {
        if let Some(image) = self.drives_loaded.remove(&drive) {
            log::debug!("Releasing VHD {:?} from drive {}", image, drive);
            self.images_loaded.remove(&image);
        }
    }
}

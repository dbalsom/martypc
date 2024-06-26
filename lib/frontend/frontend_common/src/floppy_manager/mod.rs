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

    frontend_common::floppy_manager.rs

    Discover floppy images in the 'floppy' resource and provide an interface
    for enumerating and loading them.

*/

use crate::resource_manager::{PathTreeNode, ResourceItem, ResourceManager};
use std::{
    collections::HashMap,
    ffi::OsString,
    fmt::Display,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Error;

#[derive(Debug)]
pub enum FloppyError {
    DirNotFound,
    ImageNotFound,
    FileReadError,
    FileWriteError,
}
impl std::error::Error for FloppyError {}
impl Display for FloppyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &*self {
            FloppyError::DirNotFound => write!(f, "Couldn't find the requested directory."),
            FloppyError::ImageNotFound => write!(f, "Specified image name could not be found in floppy manager."),
            FloppyError::FileReadError => write!(f, "A file read error occurred."),
            FloppyError::FileWriteError => write!(f, "A file write error occurred."),
        }
    }
}

#[allow(dead_code)]
pub struct FloppyImage {
    idx:  usize,
    name: OsString,
    path: PathBuf,
    size: u64,
}

pub struct FloppyManager {
    files: Vec<ResourceItem>,
    image_vec: Vec<FloppyImage>,
    image_map: HashMap<OsString, usize>,
    extensions: Vec<OsString>,
}

impl FloppyManager {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            image_vec: Vec::new(),
            image_map: HashMap::new(),
            extensions: vec![OsString::from("img"), OsString::from("ima")],
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
            self.image_vec.push(FloppyImage {
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

    pub fn scan_paths(&mut self, paths: Vec<PathBuf>) -> Result<bool, FloppyError> {
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
                        self.image_vec.push(FloppyImage {
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

    pub fn scan_dir(&mut self, path: &Path) -> Result<bool, FloppyError> {
        // Read in directory entries within the provided path
        let dir = match fs::read_dir(path) {
            Ok(dir) => dir,
            Err(_) => return Err(FloppyError::DirNotFound),
        };

        // Clear and rebuild image lists.
        self.image_vec.clear();
        self.image_map.clear();

        // Scan through all entries in the directory and find all files with matching extension
        for entry in dir {
            if let Ok(entry) = entry {
                if entry.path().is_file() {
                    if let Some(extension) = entry.path().extension() {
                        if self.extensions.contains(&extension.to_ascii_lowercase()) {
                            println!(
                                "Found floppy image: {:?} size: {}",
                                entry.path(),
                                entry.metadata().unwrap().len()
                            );

                            let idx = self.image_vec.len();
                            self.image_vec.push(FloppyImage {
                                idx,
                                name: entry.file_name(),
                                path: entry.path(),
                                size: entry.metadata().unwrap().len(),
                            });

                            self.image_map.insert(entry.file_name(), idx);
                        }
                    }
                }
            }
        }
        Ok(true)
    }

    pub fn get_floppy_names(&self) -> Vec<OsString> {
        let mut vec: Vec<OsString> = Vec::new();
        for (key, _val) in &self.image_map {
            vec.push(key.clone());
        }
        //vec.sort_by(|a, b| a.to_ascii_uppercase().cmp(&b.to_ascii_uppercase()));
        vec
    }

    pub fn get_floppy_name(&self, idx: usize) -> Option<OsString> {
        if idx >= self.image_vec.len() {
            return None;
        }
        Some(self.image_vec[idx].name.clone())
    }

    pub fn load_floppy_data(&self, idx: usize, rm: &ResourceManager) -> Result<Vec<u8>, FloppyError> {
        let floppy_vec;

        if idx >= self.image_vec.len() {
            return Err(FloppyError::ImageNotFound);
        }
        let floppy_path = self.image_vec[idx].path.clone();
        floppy_vec = match rm.read_resource_from_path(&floppy_path) {
            Ok(vec) => vec,
            Err(_e) => {
                return Err(FloppyError::FileReadError);
            }
        };
        Ok(floppy_vec)
    }

    /*
    pub fn load_floppy_data(&self, name: &OsString) -> Result<Vec<u8>, FloppyError> {
        let mut floppy_vec = Vec::new();
        if let Some(idx) = self.image_map.get(name) {
            if *idx >= self.image_vec.len() {
                return Err(FloppyError::ImageNotFound);
            }
            let floppy_path = self.image_vec[*idx].path.clone();
            floppy_vec = match std::fs::read(&floppy_path) {
                Ok(vec) => vec,
                Err(_e) => {
                    return Err(FloppyError::FileReadError);
                }
            };
        }
        Ok(floppy_vec)
    }*/

    // pub fn save_floppy_data(&self, data: &[u8], name: &OsString) -> Result<(), FloppyError> {
    //     if let Some(idx) = self.image_map.get(name) {
    //         if *idx >= self.image_vec.len() {
    //             return Err(FloppyError::ImageNotFound);
    //         }
    //         let floppy_path = self.image_vec[*idx].path.clone();
    //         match std::fs::write(&floppy_path, data) {
    //             Ok(_) => Ok(()),
    //             Err(_e) => {
    //                 return Err(FloppyError::FileWriteError);
    //             }
    //         }
    //     }
    //     else {
    //         Err(FloppyError::ImageNotFound)
    //     }
    // }

    pub fn save_floppy_data(&self, data: &[u8], idx: usize, _rm: &ResourceManager) -> Result<PathBuf, FloppyError> {
        if idx >= self.image_vec.len() {
            return Err(FloppyError::ImageNotFound);
        }

        let floppy_path = self.image_vec[idx].path.clone();
        // TODO: Implement write through resource manager instead of direct file access.
        match std::fs::write(&floppy_path, data) {
            Ok(_) => Ok(floppy_path.clone()),
            Err(_e) => {
                return Err(FloppyError::FileWriteError);
            }
        }
    }
}

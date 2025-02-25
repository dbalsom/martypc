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
*/

use crate::resource_manager::{ResourceFsType, ResourceItem, ResourceItemType, RESOURCE_READONLY};
use std::{
    io::{Read, Seek},
    path::{Path, PathBuf},
};

use anyhow::Error;
use zip::ZipArchive;

pub struct ArchiveOverlay<R> {
    index:   usize,
    archive: ZipArchive<R>,
}

impl<R: Read + Seek> ArchiveOverlay<R> {
    pub fn new(reader: R) -> std::io::Result<Self> {
        let archive = ZipArchive::new(reader)?;
        Ok(Self { index: 0, archive })
    }

    pub fn set_index(&mut self, index: usize) {
        self.index = index;
    }

    // pub fn list_resources(&mut self) -> Vec<ResourceItem> {
    //     let mut resources = Vec::new();
    //
    //     for i in 0..self.archive.len() {
    //         if let Ok(file) = self.archive.by_index(i) {
    //             let path = PathBuf::from(file.name()); // Zip paths are relative
    //             let filename_only = path.file_name().map(|s| s.to_os_string());
    //             let size = Some(file.size());
    //
    //             resources.push(ResourceItem {
    //                 rtype: ResourceItemType::File(ResourceFsType::Overlay(self.index)), // Assume single overlay for now
    //                 location: path.clone(),
    //                 relative_path: Some(path),
    //                 filename_only,
    //                 size,
    //                 flags: RESOURCE_READONLY, // Set any flags if necessary
    //             });
    //         }
    //     }
    //
    //     resources
    // }

    pub fn read(&mut self, path: &Path) -> Result<Vec<u8>, Error> {
        let mut path_string = path.to_str().unwrap();
        if let Some(trimmed) = path_string.strip_prefix("./") {
            path_string = trimmed;
        }

        let mut file = self.archive.by_name(path_string)?;
        let mut data = Vec::new();
        let _bytes_read = file.read_to_end(&mut data)?;
        Ok(data)
    }

    pub fn list_resources(&mut self) -> Vec<ResourceItem> {
        let mut resources = Vec::new();
        let mut seen_dirs = std::collections::HashSet::new(); // Track known directories

        for i in 0..self.archive.len() {
            if let Ok(file) = self.archive.by_index(i) {
                #[cfg(target_arch = "wasm32")]
                let mut base_path = PathBuf::new();
                #[cfg(not(target_arch = "wasm32"))]
                let mut base_path = PathBuf::from("./"); // Start with a relative path

                let zip_path = PathBuf::from(file.name()); // Zip paths are relative
                let path = base_path.join(zip_path.clone());
                // log::debug!(
                //     "list_resources: joined path is: {:?} from base {:?} and filename {:?}",
                //     path.display(),
                //     zip_path.display()
                // );
                let filename_only = path.file_name().map(|s| s.to_os_string());
                let size = Some(file.size());

                if file.is_dir() || file.name().ends_with('/') || size == Some(0) {
                    // Explicitly mark as a directory
                    log::debug!("Enumerating overlay directory: {:?}", path);
                    resources.push(ResourceItem {
                        rtype: ResourceItemType::Directory(ResourceFsType::Overlay(self.index)),
                        location: path.clone(),
                        relative_path: Some(path.clone()),
                        filename_only,
                        size: None, // Directories don't have a meaningful size
                        flags: 0,   // No special flags
                    });
                    seen_dirs.insert(path);
                }
                else {
                    // Ensure parent directories exist
                    if let Some(parent) = path.parent() {
                        if !seen_dirs.contains(parent) {
                            log::debug!("Enumerating overlay directory: {:?}", parent);
                            resources.push(ResourceItem {
                                rtype: ResourceItemType::Directory(ResourceFsType::Overlay(self.index)),
                                location: parent.to_path_buf(),
                                relative_path: Some(parent.to_path_buf()),
                                filename_only: parent.file_name().map(|s| s.to_os_string()),
                                size: None,
                                flags: 0,
                            });
                            seen_dirs.insert(parent.to_path_buf());
                        }
                    }

                    // Insert the file
                    log::debug!("Enumerating overlay file: {:?}", &path);
                    resources.push(ResourceItem {
                        rtype: ResourceItemType::File(ResourceFsType::Overlay(self.index)), // Assume single overlay for now
                        location: path.clone(),
                        relative_path: Some(path),
                        filename_only,
                        size,
                        flags: RESOURCE_READONLY, // Set any flags if necessary
                    });
                }
            }
        }

        resources
    }
}

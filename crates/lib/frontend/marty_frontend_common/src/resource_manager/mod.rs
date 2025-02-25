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

    frontend_common::resource_manager::mod.rs

    File and path services for frontends. File operations are abstracted
    to support both local and web filesystems (for wasm compilation).

    Eventually archive support will be added as well.

*/

mod archive_overlay;
#[cfg(not(target_arch = "wasm32"))]
mod local_fs;
mod manifest;
mod path_manager;
pub mod tree;
#[cfg(target_arch = "wasm32")]
mod wasm;

use std::{
    collections::HashSet,
    ffi::OsString,
    future::Future,
    path::{Path, PathBuf},
};

#[cfg(target_arch = "wasm32")]
use crate::resource_manager::manifest::{ManifestFile, ResourceManifest};

use crate::{resource_manager::archive_overlay::ArchiveOverlay, types::resource_location::ResourceLocation};

#[cfg(target_arch = "wasm32")]
use marty_web_helpers::fetch_file;
pub use path_manager::PathConfigItem;
use path_manager::PathManager;

use anyhow::Error;
use regex::Regex;
pub use tree::TreeNode as PathTreeNode;
use url::Url;

pub type AsyncResourceReadResult = dyn Future<Output = Result<Vec<u8>, anyhow::Error>> + Send;

// Resource flags
const RESOURCE_READONLY: u32 = 0x00000001;

#[derive(Copy, Clone, Debug)]
pub enum ResourceItemType {
    Directory(ResourceFsType),
    File(ResourceFsType),
}

#[derive(Copy, Clone, Debug)]
pub enum ResourceFsType {
    Native,
    Overlay(usize),
}

#[derive(Clone, Debug)]
pub struct ResourceItem {
    pub(crate) rtype: ResourceItemType,
    pub(crate) location: PathBuf,
    pub(crate) relative_path: Option<PathBuf>,
    pub(crate) filename_only: Option<OsString>,
    pub(crate) size: Option<u64>,
    flags: u32,
}

impl ResourceItem {
    pub fn from_filename(filename: &str) -> Self {
        let mut new_path: PathBuf = PathBuf::from(".");
        new_path.push(filename.replace("/", "\\"));
        Self {
            rtype: ResourceItemType::File(ResourceFsType::Native),
            location: new_path.clone(),
            relative_path: None,
            filename_only: new_path.file_name().map(|s| s.to_os_string()),
            size: None,
            flags: 0,
        }
    }
}

pub struct ResourceManager {
    pub pm: PathManager,
    pub base_url: Option<Url>,
    pub ignore_dirs: Vec<String>,
    pub overlays: Vec<ArchiveOverlay<std::io::Cursor<Vec<u8>>>>,
    #[cfg(target_arch = "wasm32")]
    manifest: ResourceManifest,
}

impl ResourceManager {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            pm: PathManager::new(base_path),
            base_url: None,
            ignore_dirs: Vec::new(),
            overlays: Vec::new(),
            #[cfg(target_arch = "wasm32")]
            manifest: ResourceManifest::default(),
        }
    }

    pub fn set_base_url(&mut self, base_url: &Url) {
        self.base_url = Some(base_url.clone());
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn load_manifest(&mut self, manifest: ResourceLocation) -> Result<(), Error> {
        match manifest {
            ResourceLocation::Url(url) => {
                // Load the manifest from the URL
                let manifest_data = marty_web_helpers::fetch_url(&url)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to fetch manifest from URL '{}': {}", url, e))?;

                // Parse the manifest
                let manifest_str = String::from_utf8(manifest_data)?;

                // Deserialize the manifest FROM TOML
                let manifest_file: ManifestFile = toml::from_str(&manifest_str)?;

                self.manifest = ResourceManifest::new(&manifest_file.entries);
                self.manifest.debug();
            }
            ResourceLocation::FilePath(_) => {
                panic!("Don't use FilePath for wasm32 targets!");
            }
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn load_manifest(&mut self, _manifest: ResourceLocation) -> Result<(), Error> {
        log::debug!("load_manifest(): Not implemented for native build");
        Ok(())
    }

    pub fn resolve_path_from_filename(
        &mut self,
        resource: &str,
        file_name: impl AsRef<Path>,
    ) -> Result<PathBuf, Error> {
        let file_extension = file_name.as_ref().extension().map(|s| s.to_os_string());

        let mut extensions = Vec::new();
        if let Some(ext) = file_extension {
            extensions.push(ext);
        }

        let items = self.enumerate_items(resource, None, false, true, Some(extensions))?;

        for item in items {
            if item.filename_only == Some(file_name.as_ref().as_os_str().to_os_string()) {
                return Ok(item.location.clone());
            }
        }

        Err(anyhow::anyhow!(
            "Failed to resolve path for file: {}",
            file_name.as_ref().to_string_lossy()
        ))
    }

    pub fn from_config(base_path: PathBuf, config: &[PathConfigItem]) -> Result<Self, Error> {
        let mut rm = Self::new(base_path);
        for item in config {
            rm.pm.add_path(&item.resource, &item.path, item.create)?;
        }
        //rm.pm.create_paths()?;
        Ok(rm)
    }

    pub fn set_ignore_dirs(&mut self, dirs: Vec<String>) {
        self.ignore_dirs = dirs;
    }

    pub fn resource_path(&self, resource: &str) -> Option<PathBuf> {
        self.pm.resource_path(resource)
    }

    /// Return a unique filename for the given resource, base name, and extension.
    /// Names will be generated by appending digits to the base name until a unique name is found.
    pub fn get_available_filename(
        &mut self,
        resource: &str,
        base_name: &str,
        extension: Option<&str>,
    ) -> Result<PathBuf, Error> {
        let mut path = self
            .pm
            .resource_path(resource)
            .ok_or(anyhow::anyhow!("Resource path not found: {}", resource))?;

        // Generate a regex to extract a sequence of digits from a filename
        let re = Regex::new(r"(\d+)")?;
        let mut largest_num = 0;

        log::debug!("Finding unique filename in: {:?}", path);

        // First, generate a map of all items starting with 'base_name'
        let mut existing_basenames: HashSet<OsString> = HashSet::new();
        match self.enumerate_items(resource, None, false, false, None) {
            Ok(items) => {
                for item in items {
                    //log::debug!("Item: {:?}", item);
                    if let Some(filename) = item.filename_only.clone() {
                        if filename.to_string_lossy().contains(base_name) {
                            //log::debug!("Found matching basename: {:?}", filename);

                            // Extract any number sequence from the filename
                            re.captures(
                                filename
                                    .to_str()
                                    .ok_or(anyhow::anyhow!("Failed to convert filename to string"))?,
                            )
                            .and_then(|caps| caps.get(1))
                            .and_then(|match_| match_.as_str().parse::<u32>().ok())
                            .map(|num| {
                                if num > largest_num {
                                    largest_num = num
                                }
                            });
                            existing_basenames.insert(filename);
                        }
                    }
                }
            }
            Err(err) => {
                return Err(anyhow::anyhow!(
                    "Failed to enumerate items in resource '{}': {}",
                    resource,
                    err
                ));
            }
        }

        // Generate unique names and check them against the map. We can start searching at
        // 'largest_num'
        let mut i = largest_num;
        let mut test_name_path = PathBuf::from(format!("{}{:04}", base_name, i));
        if let Some(ext) = extension {
            test_name_path.set_extension(ext);
        }
        let mut test_name = test_name_path.into_os_string();

        // for name in existing_basenames.iter() {
        //     log::debug!("Existing name: {} largest num: {}", name.to_str().unwrap(), largest_num);
        // }

        while existing_basenames.contains(&test_name) {
            i += 1;
            test_name_path = PathBuf::from(format!("{}{:04}", base_name, i));
            if let Some(ext) = extension {
                test_name_path.set_extension(ext);
            }
            test_name = test_name_path.into_os_string();
        }

        log::debug!("Found unique filename: {}", test_name.to_str().unwrap());

        path.push(test_name.clone());
        if let Some(ext) = extension {
            path.set_extension(ext);
        }

        // We should have a unique filename now. Check that the file exists before we return it
        // as one last sanity check.
        if ResourceManager::path_exists(&path) {
            log::error!(
                "Failed to create unique filename: File already exists: {}",
                path.to_str().unwrap()
            );
            return Err(anyhow::anyhow!(
                "Failed to create unique filename: File already exists: {}",
                path.to_str().unwrap()
            ));
        }
        Ok(path)
    }

    pub fn path_contains_dir(path: &PathBuf, dir: &str) -> bool {
        path.iter().any(|component| component == dir)
    }

    pub fn path_contains_dirs(path: &PathBuf, dirs: &Vec<&str>) -> bool {
        dirs.iter().any(|&dir| path.iter().any(|component| component == dir))
    }

    pub fn set_relative_paths_for_items(base: PathBuf, items: &mut Vec<ResourceItem>) {
        // Strip the base path from all items.
        for item in items.iter_mut() {
            item.relative_path = Some(
                item.location
                    .strip_prefix(&base)
                    .unwrap_or(&item.location)
                    .to_path_buf(),
            );
        }
    }
}

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

    frontend_common::resource_manager::local_fs.rs

    Method implementations for wasm "virtual filesystem" operations.

*/

use std::{
    ffi::{OsStr, OsString},
    io::Cursor,
    path::{Path, PathBuf},
};

use crate::resource_manager::{
    tree::{build_tree, merge_items, new_tree, TreeNode},
    ArchiveOverlay,
    ResourceFsType,
    ResourceItem,
    ResourceItemType,
    ResourceManager,
};

use anyhow::Error;
use marty_common::MartyHashMap;
use url::Url;

impl ResourceManager {
    /// On wasm targets we return the list of items read from either a file manifest or an
    /// [ArchiveOverlay].
    pub fn enumerate_items(
        &mut self,
        resource: &str,
        subdir: Option<&OsStr>,
        multipath: bool,
        _resursive: bool,
        _extension_filter: Option<Vec<OsString>>,
    ) -> Result<Vec<ResourceItem>, Error> {
        let mut items: Vec<ResourceItem> = Vec::new();

        log::debug!("Enumerating items for resource: {}", resource);
        let mut roots = self
            .pm
            .get_resource_paths(resource)
            .ok_or(anyhow::anyhow!("Resource path not found: {}", resource))?;

        if !multipath {
            // If multipath is false, only use the first path
            roots.truncate(1);
        }

        if roots.is_empty() {
            return Err(anyhow::anyhow!("No paths defined for resource: {}", resource));
        }

        let mut item_map = MartyHashMap::default();

        log::debug!("Got {} path(s) for resource: {}", roots.len(), resource);
        for path in roots.iter() {
            let mut path = PathBuf::from(&path);
            //let mut path = path.clone().canonicalize()?;

            if let Some(subdir) = subdir.clone() {
                path.push(subdir);
            }

            log::debug!("Descending into directory: {}", path.display());
            for entry in self.manifest.read_dir(&String::from(path.clone().to_string_lossy())) {
                if entry.path().is_dir() {
                    log::debug!("Enumerating Directory entry {:?}", entry);
                    let resource_item = ResourceItem {
                        rtype: ResourceItemType::Directory(ResourceFsType::Native),
                        location: entry.path().clone(),
                        relative_path: None,
                        filename_only: Some(entry.path().file_name().unwrap_or_default().to_os_string()),
                        flags: 0,
                        size: entry.size(),
                    };
                    item_map.insert(entry.path().clone(), resource_item);
                }
                else {
                    let foo = entry.path().file_name().unwrap_or_default().to_os_string();
                    let resource_item = ResourceItem {
                        rtype: ResourceItemType::File(ResourceFsType::Native),
                        location: entry.path().clone(),
                        relative_path: None,
                        filename_only: Some(entry.path().file_name().unwrap_or_default().to_os_string()),
                        flags: 0,
                        size: entry.size(),
                    };
                    item_map.insert(entry.path().clone(), resource_item);
                }
            }
        }

        log::debug!("enumerate_items(): Found {} overlays", self.overlays.len());
        for overlay in &mut self.overlays {
            let overlay_items = overlay.list_resources();

            for overlay_item in overlay_items {
                let mut overlay_path = &overlay_item.location;

                //log::debug!("Have roots: {:?}, item: {:?}", roots, overlay_path);

                roots.iter().any(|root| {
                    if overlay_path.starts_with(root) {
                        log::debug!("Item {:?} matched resource root {:?}", overlay_item, root.display());

                        if item_map.contains_key(overlay_path) {
                            log::debug!(
                                "Item {:?} already exists, local fs takes precedence. skipping.",
                                overlay_item.location.display()
                            );
                        }
                        else {
                            log::debug!("Adding new overlay item {:?}", overlay_item.location.display());
                            item_map.insert(overlay_path.clone(), overlay_item.clone());
                        };
                        true
                    }
                    else {
                        log::debug!(
                            "Item {:?} did not match root {:?}",
                            overlay_item.location.display(),
                            root.display()
                        );
                        false
                    }
                });
            }
        }

        items.extend(item_map.into_values());
        log::debug!(
            "enumerate_items(): Found {} items for resource: {}",
            items.len(),
            resource
        );
        Ok(items)
    }

    pub fn items_to_tree(&self, resource: &str, items: &Vec<ResourceItem>) -> Result<TreeNode, Error> {
        // TODO: support multipath
        let root_path = self
            .pm
            .resource_path(resource)
            .ok_or(anyhow::anyhow!("Resource path not found: {}", resource))?;

        log::debug!(
            "wasm::items_to_tree(): building tree for resource: {} over {} items: root_path: {}",
            resource,
            items.len(),
            root_path.display()
        );

        let skip_size = root_path.components().count();

        build_tree(String::from(root_path.to_string_lossy()), items, skip_size)
    }

    pub fn items_to_tree_raw(&self, _items: &Vec<ResourceItem>) -> Result<TreeNode, Error> {
        log::warn!("items_to_tree_raw() not implemented for wasm target");
        Ok(TreeNode::default())
    }

    /// Stub implementation for wasm targets. We cannot create urls.
    pub fn create_path(path: &Path) -> Result<(), Error> {
        log::warn!("create_path() not implemented for wasm target");
        Ok(())
    }

    /// On wasm targets, we don't have access to the filesystem to overwrite anything,
    /// so we just return false
    pub fn path_exists(_path: &Path) -> bool {
        log::warn!("path_exists() not implemented for wasm target");
        false
    }

    /// On wasm targets, we don't have access to the filesystem. We can probably detect what is
    /// a directory based on the path string, but for now we just return false.
    pub fn path_is_dir(_path: &Path) -> bool {
        log::warn!("path_is_dir() not implemented for wasm target");
        false
    }

    /// On wasm, a blocking read is only possible if the file is in the overlay. Otherwise, we
    /// return an error.
    pub fn read_resource_from_path_blocking(&mut self, path: impl AsRef<Path>) -> Result<Vec<u8>, Error> {
        let path = path.as_ref();

        // Attempt to read the file from the overlay.
        for overlay in &mut self.overlays {
            log::debug!("Attempting to read file {:?} from fs overlay...", path.display());
            return match overlay.read(path) {
                Ok(buf) => Ok(buf),
                Err(e) => {
                    log::warn!("Failed to read file {:?} from fs overlay: {}", path.display(), e);
                    Err(anyhow::anyhow!("Failed to read file from overlay: {}", e))
                }
            };
        }

        Err(anyhow::anyhow!("File not found in overlay: {}", path.display()))
    }

    /// Mount an ArchiveOverlay from a specified path, or return an error.
    pub async fn mount_overlay(&mut self, path: impl AsRef<Path>) -> Result<(), Error> {
        if self.base_url.is_none() {
            return Err(anyhow::anyhow!("Base URL not set"));
        }

        let path_str = path.as_ref().to_string_lossy().to_string();

        log::debug!("Fetching overlay file: {}", path_str);
        let url = self.base_url.clone().unwrap().join(&path_str)?;
        let file = marty_web_helpers::fetch_url(&url)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch file: {}", e))?;

        let mut new_archive = ArchiveOverlay::new(Cursor::new(file))
            .map_err(|e| anyhow::anyhow!("Failed to create archive overlay: {}", e))?;

        let new_idx = self.overlays.len();
        new_archive.set_index(new_idx);

        // Create a new tree for the overlay.
        let mut new_tree = new_tree(self.base_url.clone().unwrap().to_string());
        // Enumerate the items in the overlay and merge them into the tree.
        let items = new_archive.list_resources();
        merge_items(&mut new_tree, &items, 0);

        self.overlays.push(new_archive);

        Ok(())
    }

    /// Reads the contents of a resource from a specified file system path into a byte vector, or returns an error.
    pub async fn read_resource_from_path(&mut self, path: impl AsRef<Path>) -> Result<Vec<u8>, Error> {
        if self.base_url.is_none() {
            return Err(anyhow::anyhow!("Base URL not set"));
        }
        let path = path.as_ref();

        // First, attempt to read the file from overlays before fetching.
        for overlay in &mut self.overlays {
            log::debug!("Attempting to read file {:?} from fs overlay...", path.display());
            match overlay.read(path) {
                Ok(buf) => return Ok(buf),
                Err(e) => {
                    log::warn!("Failed to read file {:?} from fs overlay: {}", path.display(), e);
                    continue;
                }
            }
        }

        let path_str = path.to_string_lossy().to_string();

        let entry = self
            .manifest
            .map
            .get(&path_str)
            .map_or(Err(anyhow::anyhow!("Path not found: {}", path_str)), |v| Ok(v.clone()))?;

        let url = self.base_url.clone().unwrap().join(&entry.path)?;
        let file = marty_web_helpers::fetch_url(&url)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch file: {}", e))?;

        Ok(file)
    }

    pub async fn read_string_from_path(&mut self, path: impl AsRef<Path>) -> Result<String, Error> {
        let file = self.read_resource_from_path(path).await?;
        let file_str =
            String::from_utf8(file).map_err(|e| anyhow::anyhow!("Failed to convert file to string: {}", e))?;
        Ok(file_str)
    }
}

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

    frontend_common::resource_manager::local_fs.rs

    Method implementations for local filesystem operations.

*/
use crate::resource_manager::{
    tree::{build_tree, TreeNode},
    ResourceItem,
    ResourceItemType,
    ResourceManager,
};
use anyhow::Error;
use std::{
    collections::HashSet,
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
};

impl ResourceManager {
    /// Enumerates resource items for a given resource, optionally using multiple paths and recursion.
    ///
    /// This function provides an interface to collect resource items based on the specified `resource`.
    /// It can operate in either a recursive or non-recursive manner and can use either a single path or
    /// multiple paths to locate resources, depending on the provided arguments.
    ///
    /// If `recursive` is `true`, the function delegates to `enumerate_items_recursive` to perform a recursive
    /// search. Otherwise, it conducts a non-recursive search, adding each found item to a `Vec<ResourceItem>`.
    /// A `ResourceItem` includes details like resource type, full path, filename, and flags.
    ///
    /// # Arguments
    /// * `resource`  - A string slice representing the resource for which items are to be enumerated.
    /// * `multipath` - A boolean flag indicating whether to search across multiple paths. If `false`,
    ///                 only the first found path is used.
    /// * `recursive` - A boolean flag indicating whether the search should be recursive. If `true`, the
    ///                 function calls `enumerate_items_recursive`.
    ///
    /// # Returns
    /// Returns a `Result<Vec<ResourceItem>, Error>`. On success, it provides a vector of `ResourceItem` objects,
    /// each representing a found resource item. On failure, such as when a resource path is not found or a path
    /// cannot be canonicalized, it returns an `Error`.
    ///
    /// # Errors
    /// The function may return an error if the resource path is not found or if there's an issue in canonicalization
    /// the path.
    pub fn enumerate_items(
        &self,
        resource: &str,
        subdir: Option<&OsStr>,
        multipath: bool,
        resursive: bool,
        extension_filter: Option<Vec<OsString>>,
    ) -> Result<Vec<ResourceItem>, Error> {
        let mut items = if resursive {
            self.enumerate_items_recursive(multipath, resource, subdir)?
        }
        else {
            let mut items: Vec<ResourceItem> = Vec::new();

            let mut paths = self
                .pm
                .get_resource_paths(resource)
                .ok_or(anyhow::anyhow!("Resource path not found: {}", resource))?;

            if !multipath {
                // If multipath is false, only use the first path
                paths.truncate(1);
            }

            if paths.is_empty() {
                return Err(anyhow::anyhow!("No paths defined for resource: {}", resource));
            }

            log::debug!("Got {} path(s) for resource: {}", paths.len(), resource);
            for path in paths.iter() {
                let mut path = path.clone().canonicalize()?;

                if let Some(subdir) = subdir.clone() {
                    path.push(subdir);
                }

                log::debug!("Descending into directory: {}", path.display());
                for entry in fs::read_dir(path.clone())? {
                    match entry {
                        Ok(entry) => {
                            if entry.path().is_dir() {
                                items.push(ResourceItem {
                                    rtype: ResourceItemType::Directory,
                                    location: entry.path().clone(),
                                    relative_path: None,
                                    filename_only: Some(entry.path().file_name().unwrap_or_default().to_os_string()),
                                    flags: 0,
                                    size: None,
                                });
                            }
                            else {
                                items.push(ResourceItem {
                                    rtype: ResourceItemType::LocalFile,
                                    location: entry.path().clone(),
                                    relative_path: None,
                                    filename_only: Some(entry.path().file_name().unwrap_or_default().to_os_string()),
                                    flags: 0,
                                    size: Some(entry.path().metadata()?.len() as usize),
                                });
                            }
                        }
                        Err(e) => {
                            log::error!("Error reading directory entry: {}", e);
                        }
                    }
                }
            }
            items
        };

        // If extension filter was provided, filter items by extension
        if let Some(extension_filter) = extension_filter {
            items = items
                .iter()
                .filter_map(|item| {
                    if item.location.is_file() {
                        if let Some(extension) = item.location.extension() {
                            if extension_filter.contains(&extension.to_ascii_lowercase()) {
                                return Some(item);
                            }
                        }
                    }
                    return None;
                })
                .cloned()
                .collect::<Vec<_>>();
        }

        // Convert paths to relative paths
        let path_prefix = self
            .pm
            .get_resource_path(resource)
            .ok_or(anyhow::anyhow!("Resource path not found: {}", resource))?;

        ResourceManager::set_relative_paths_for_items(path_prefix, &mut items);

        if items.is_empty() {
            log::warn!("No items found for resource: {}", resource);
        }
        else {
            log::debug!(
                "enumerate_items(): Found {} items for resource: {}",
                items.len(),
                resource
            );
        }

        Ok(items)
    }

    /// Recursively enumerates resource items based on the provided resource path.
    ///
    /// This function searches for resource items starting from the paths associated with the given `resource`.
    /// The paths are obtained from a resource manager (`self.pm`). If `multipath` is `true`, it explores all
    /// available paths; otherwise, it only explores the first path. The search avoids directories listed in
    /// `self.ignore_dirs`.
    fn enumerate_items_recursive(
        &self,
        multipath: bool,
        resource: &str,
        subdir: Option<&OsStr>,
    ) -> Result<Vec<ResourceItem>, Error> {
        let mut roots = self
            .pm
            .get_resource_paths(resource)
            .ok_or(anyhow::anyhow!("Resource path not found: {}", resource))?;

        if !multipath {
            // If multipath is false, only use the first path
            roots.truncate(1);

            if let Some(subdir) = subdir {
                roots[0].push(subdir);
            }
        }

        if roots.is_empty() {
            return Err(anyhow::anyhow!("No paths defined for resource: {}", resource));
        }

        let mut items: Vec<ResourceItem> = Vec::new();
        let mut visited = HashSet::new();

        for root in roots.iter() {
            let ignore_dirs = self.ignore_dirs.iter().map(|s| s.as_str()).collect();
            ResourceManager::visit_dirs(&root, &mut visited, &ignore_dirs, &mut |entry: &fs::DirEntry| {
                items.push(ResourceItem {
                    rtype: ResourceItemType::LocalFile,
                    location: entry.path(),
                    relative_path: None,
                    filename_only: Some(entry.path().file_name().unwrap_or_default().to_os_string()),
                    flags: 0,
                });
            })?
        }

        Ok(items)
    }

    pub fn enumerate_items_from_path(&self, path: &PathBuf) -> Result<Vec<ResourceItem>, Error> {
        let mut items: Vec<ResourceItem> = Vec::new();
        let mut visited = HashSet::new();

        let ignore_dirs = self.ignore_dirs.iter().map(|s| s.as_str()).collect();
        ResourceManager::visit_dirs(&path, &mut visited, &ignore_dirs, &mut |entry: &fs::DirEntry| {
            items.push(ResourceItem {
                rtype: ResourceItemType::LocalFile,
                location: entry.path(),
                relative_path: None,
                filename_only: Some(entry.path().file_name().unwrap_or_default().to_os_string()),
                flags: 0,
            });
        })?;

        Ok(items)
    }

    pub fn enumerate_dirs(&self, multipath: bool, resource: &str) -> Result<Vec<ResourceItem>, Error> {
        let mut dir_items = Vec::new();

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

        for root in roots.iter() {
            if root.is_dir() {
                for entry in fs::read_dir(root)? {
                    let entry = entry?;
                    let path = entry.path();

                    if path.is_dir() {
                        dir_items.push(ResourceItem {
                            rtype: ResourceItemType::Directory,
                            location: path.clone(),
                            relative_path: None,
                            filename_only: Some(path.file_name().unwrap_or_default().to_os_string()),
                            flags: 0,
                        });
                    }
                }
            }
        }

        Ok(dir_items)
    }

    fn visit_dirs(
        dir: &Path,
        visited: &mut HashSet<PathBuf>,
        ignore_dirs: &Vec<&str>,
        cb: &mut dyn FnMut(&fs::DirEntry),
    ) -> std::io::Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                // Resolve the symlink (if any) and check if it's already visited
                let canonical_path = fs::canonicalize(&path)?;
                if visited.contains(&canonical_path) {
                    continue;
                }

                if ResourceManager::path_contains_dirs(&canonical_path, ignore_dirs) {
                    continue;
                }
                visited.insert(canonical_path);

                if path.is_dir() {
                    ResourceManager::visit_dirs(&path, visited, ignore_dirs, cb)?;
                }
                else {
                    cb(&entry);
                }
            }
        }
        Ok(())
    }

    /// Converts a list of resource items into a tree structure.
    pub fn items_to_tree(&self, resource: &str, items: &Vec<ResourceItem>) -> Result<TreeNode, Error> {
        // TODO: support multipath
        let root_path = self
            .pm
            .get_resource_path(resource)
            .ok_or(anyhow::anyhow!("Resource path not found: {}", resource))?;

        log::debug!(
            "items_to_tree(): building tree for resource: {} over {} items: root_path: {}",
            resource,
            items.len(),
            root_path.display()
        );

        let skip_size = root_path.components().count();

        build_tree(String::from(root_path.to_string_lossy()), items, skip_size)
    }

    pub fn items_to_tree_raw(&self, items: &Vec<ResourceItem>) -> Result<TreeNode, Error> {
        build_tree(".".to_string(), items, 0)
    }

    /// Return whether the specified path exists.
    pub fn path_exists(path: impl AsRef<Path>) -> bool {
        path.as_ref().exists()
    }

    /// Create the specified path if it does not exist.
    pub fn create_path<P: AsRef<Path>>(path: P) -> Result<(), Error> {
        if !ResourceManager::path_exists(&path) {
            fs::create_dir_all(path)?;
        }
        Ok(())
    }

    /// Returns whether the specified path is a directory.
    pub fn path_is_dir(path: &PathBuf) -> bool {
        let canonical_path = path.canonicalize();
        if let Ok(path) = canonical_path {
            //log::debug!("Path: {} dir?: {}", path.to_str().unwrap_or_default(), path.is_dir());
            return path.is_dir();
        }
        false
    }

    /// Reads the contents of a resource from a specified file system path into a byte vector, or returns an error.
    pub fn read_resource_from_path_blocking(&self, path: impl AsRef<Path>) -> Result<Vec<u8>, Error> {
        let buffer = std::fs::read(path)?;
        Ok(buffer)
    }

    /// Reads the contents of a resource from a specified file system path into a byte vector, or returns an error.
    pub async fn read_resource_from_path(&self, path: impl AsRef<Path>) -> Result<Vec<u8>, Error> {
        let buffer = std::fs::read(path)?;
        Ok(buffer)
    }

    pub async fn read_string_from_path(&self, path: impl AsRef<Path>) -> Result<String, Error> {
        let file = self.read_resource_from_path(path).await?;
        let file_str =
            String::from_utf8(file).map_err(|e| anyhow::anyhow!("Failed to convert file to string: {}", e))?;
        Ok(file_str)
    }
}

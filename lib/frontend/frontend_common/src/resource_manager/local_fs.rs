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
    fs,
    path::{Path, PathBuf},
};

impl ResourceManager {
    pub fn enumerate_items(&self, resource: &str, resursive: bool) -> Result<Vec<ResourceItem>, Error> {
        if resursive {
            return self.enumerate_items_recursive(resource);
        }

        let mut items: Vec<ResourceItem> = Vec::new();

        let path = self
            .pm
            .get_path(resource)
            .ok_or(anyhow::anyhow!("Resource path not found: {}", resource))?;

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                items.push(ResourceItem {
                    rtype: ResourceItemType::Directory,
                    full_path: path.clone(),
                    filename_only: Some(PathBuf::from(path.file_name().unwrap_or_default())),
                    flags: 0,
                });
            }
            else {
                items.push(ResourceItem {
                    rtype: ResourceItemType::LocalFile,
                    full_path: path.clone(),
                    filename_only: Some(PathBuf::from(path.file_name().unwrap_or_default())),
                    flags: 0,
                });
            }
        }

        Ok(items)
    }

    fn enumerate_items_recursive(&self, resource: &str) -> Result<Vec<ResourceItem>, Error> {
        let root = self
            .pm
            .get_path(resource)
            .ok_or(anyhow::anyhow!("Resource path not found: {}", resource))?;

        let mut items: Vec<ResourceItem> = Vec::new();
        let mut visited = HashSet::new();

        let ignore_dirs = self.ignore_dirs.iter().map(|s| s.as_str()).collect();
        ResourceManager::visit_dirs(&root, &mut visited, &ignore_dirs, &mut |entry: &fs::DirEntry| {
            items.push(ResourceItem {
                rtype: ResourceItemType::LocalFile,
                full_path: entry.path(),
                filename_only: Some(PathBuf::from(entry.path().file_name().unwrap_or_default())),
                flags: 0,
            });
        })?;
        Ok(items)
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

    pub fn items_to_tree(&self, resource: &str, items: Vec<ResourceItem>) -> Result<TreeNode, Error> {
        let mut root_path = self
            .pm
            .get_path(resource)
            .ok_or(anyhow::anyhow!("Resource path not found: {}", resource))?;

        build_tree(String::from(root_path.to_string_lossy()), items)
    }

    /// Return whether the specified path exists.
    pub fn path_exists(path: &PathBuf) -> bool {
        path.exists()
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

    pub fn read_resource_from_path(&self, path: &PathBuf) -> Result<Vec<u8>, Error> {
        let mut buffer = std::fs::read(path)?;
        Ok(buffer)
    }
}

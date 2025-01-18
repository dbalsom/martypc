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

    Method implementations for wasm "virtual filesystem" operations.

*/

use crate::resource_manager::{tree::TreeNode, ResourceItem, ResourceItemType, ResourceManager};

use std::{
    ffi::{OsStr, OsString},
    fs,
    path::Path,
};

use anyhow::Error;

impl ResourceManager {
    /// On wasm targets, we can't get a directory listing, so return an empty vector.
    /// TODO: Eventually we might load a manifest file that can provide a virtual directory listing.
    pub fn enumerate_items(
        &self,
        resource: &str,
        _subdir: Option<&OsStr>,
        _multipath: bool,
        _resursive: bool,
        _extension_filter: Option<Vec<OsString>>,
    ) -> Result<Vec<ResourceItem>, Error> {
        let mut items: Vec<ResourceItem> = Vec::new();
        Ok(items)
    }

    pub fn items_to_tree(&self, _resource: &str, _items: &Vec<ResourceItem>) -> Result<TreeNode, Error> {
        Ok(TreeNode::default())
    }

    pub fn items_to_tree_raw(&self, _items: &Vec<ResourceItem>) -> Result<TreeNode, Error> {
        Ok(TreeNode::default())
    }

    /// Stub implementation for wasm targets. We cannot create urls.
    pub fn create_path(path: &Path) -> Result<(), Error> {
        Ok(())
    }

    /// On wasm targets, we don't have access to the filesystem to overwrite anything,
    /// so we just return false
    pub fn path_exists(_path: &Path) -> bool {
        false
    }

    /// On wasm targets, we don't have access to the filesystem. We can probably detect what is
    /// a directory based on the path string, but for now we just return false.
    pub fn path_is_dir(_path: &Path) -> bool {
        false
    }

    /// Reads the contents of a resource from a specified file system path into a byte vector, or returns an error.
    pub fn read_resource_from_path(&self, _path: &Path) -> Result<Vec<u8>, Error> {
        Ok(Vec::new())
    }
}

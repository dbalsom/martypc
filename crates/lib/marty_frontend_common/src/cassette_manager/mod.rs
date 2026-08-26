/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the "Software"),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.
*/

use std::{ffi::OsString, path::PathBuf};

use anyhow::{anyhow, Error};

use crate::resource_manager::{PathTreeNode, ResourceItem, ResourceManager};

const CASSETTE_RESOURCE: &str = "cassette";

#[derive(Debug)]
pub struct CassetteMedia {
    pub name: OsString,
    pub path: PathBuf,
    pub data: Vec<u8>,
}

pub struct CassetteManager {
    files: Vec<ResourceItem>,
    loaded: Option<CassetteMedia>,
}

impl CassetteManager {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            loaded: None,
        }
    }

    pub fn scan_resource(&mut self, rm: &mut ResourceManager) -> Result<bool, Error> {
        self.files = rm.enumerate_items(
            CASSETTE_RESOURCE,
            None,
            true,
            true,
            Some(vec![OsString::from("wav")]),
        )?;
        Ok(true)
    }

    pub fn make_tree(&self, rm: &ResourceManager) -> Result<PathTreeNode, Error> {
        rm.items_to_tree(CASSETTE_RESOURCE, &self.files)
    }

    pub fn load_resource(&mut self, idx: usize, rm: &mut ResourceManager) -> Result<&CassetteMedia, Error> {
        let path = self
            .files
            .get(idx)
            .ok_or_else(|| anyhow!("Cassette WAV index {idx} was not found"))?
            .location
            .clone();
        let data = rm.read_resource_from_path_blocking(&path)?;
        Ok(self.load_data(path, data))
    }

    pub fn load_data(&mut self, path: PathBuf, data: Vec<u8>) -> &CassetteMedia {
        let name = path.file_name().unwrap_or(path.as_os_str()).to_os_string();
        self.loaded = Some(CassetteMedia { name, path, data });
        self.loaded.as_ref().unwrap()
    }

    pub fn loaded(&self) -> Option<&CassetteMedia> {
        self.loaded.as_ref()
    }

    pub fn eject(&mut self) {
        self.loaded = None;
    }
}

impl Default for CassetteManager {
    fn default() -> Self {
        Self::new()
    }
}

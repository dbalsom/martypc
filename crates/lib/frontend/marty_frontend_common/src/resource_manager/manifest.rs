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
#![allow(dead_code)]
//! Define a [ResourceManifest], a file used for wasm builds to inform the emulator of what
//! resources it has available to fetch. This implements a crude virtual file system.

use std::path::PathBuf;

use marty_common::MartyHashMap;
use serde_derive::Deserialize;

#[derive(Copy, Clone, Debug, Deserialize)]
pub enum ManifestFileType {
    Directory,
    File,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ManifestEntry {
    pub(crate) path: String,
    #[serde(rename = "type")]
    pub(crate) kind: ManifestFileType,
    pub(crate) size: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ManifestFile {
    pub(crate) version: String,
    pub(crate) entries: Vec<ManifestEntry>,
}

#[derive(Default)]
pub struct ResourceManifest {
    pub(crate) entries: Vec<ManifestEntry>,
    pub(crate) map: MartyHashMap<String, ManifestEntry>,
}

impl ResourceManifest {
    pub fn new(entries: &[ManifestEntry]) -> Self {
        let mut manifest = Self::default();
        for entry in entries.iter() {
            manifest.map.insert(entry.path.clone(), entry.clone());
        }
        manifest.entries = entries.to_vec();
        manifest
    }

    pub fn entry(&self, key: &str) -> Option<&ManifestEntry> {
        self.map.get(key)
    }

    pub fn debug(&self) {
        for entry in self.entries.iter() {
            log::debug!("Manifest entry: {:?}", entry);
        }
    }

    pub fn read_dir(&self, dir: &String) -> Vec<ManifestDirEntry> {
        let mut entries = Vec::new();
        for file in &self.entries {
            log::debug!("Checking dir: {} against manifest entry {:?}", dir, file.path);
            if file.path.starts_with(dir) && matches!(file.kind, ManifestFileType::File) {
                entries.push(ManifestDirEntry {
                    path:   file.path.clone(),
                    is_dir: false,
                    size:   file.size,
                });
            }
        }
        entries
    }
}

#[derive(Clone, Debug)]
pub struct ManifestDirEntry {
    path:   String,
    is_dir: bool,
    size:   Option<u64>,
}

impl ManifestDirEntry {
    pub fn path(&self) -> PathBuf {
        PathBuf::from(&self.path)
    }

    pub fn is_dir(&self) -> bool {
        self.is_dir
    }

    pub fn size(&self) -> Option<u64> {
        self.size
    }
}

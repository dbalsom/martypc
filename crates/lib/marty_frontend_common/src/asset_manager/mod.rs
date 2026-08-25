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

    --------------------------------------------------------------------------
*/

//! Discovery and classification of frontend asset libraries.

use std::{
    ffi::OsString,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

use crate::resource_manager::ResourceManager;

use anyhow::{Context, Error};
use serde::Deserialize;
use zip::ZipArchive;

const ASSET_RESOURCE: &str = "asset";
const ASSET_MANIFEST_FILENAME: &str = "manifest.toml";
pub const OSD_KEYBOARD_DEFINITION_FILENAME: &str = "keyboard_def.toml";

#[derive(Copy, Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub enum AssetType {
    SoundLibrary,
    OsdKeyboard,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct AssetManifest {
    pub asset_type: AssetType,
    pub asset_subtype: String,
    pub asset_name: String,
    pub asset_specifier: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SoundLibrary {
    pub path: PathBuf,
    pub manifest: AssetManifest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OsdKeyboardAsset {
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Asset {
    SoundLibrary(SoundLibrary),
    OsdKeyboard(OsdKeyboardAsset),
}

impl Asset {
    pub fn asset_type(&self) -> AssetType {
        match self {
            Self::SoundLibrary(_) => AssetType::SoundLibrary,
            Self::OsdKeyboard(_) => AssetType::OsdKeyboard,
        }
    }

    pub fn manifest(&self) -> Option<&AssetManifest> {
        match self {
            Self::SoundLibrary(library) => Some(&library.manifest),
            Self::OsdKeyboard(_) => None,
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            Self::SoundLibrary(library) => &library.path,
            Self::OsdKeyboard(keyboard) => &keyboard.path,
        }
    }
}

#[derive(Default)]
pub struct AssetManager {
    assets: Vec<Asset>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn scan_resource(&mut self, rm: &mut ResourceManager) -> Result<usize, Error> {
        self.assets.clear();

        if rm.resource_path(ASSET_RESOURCE).is_none() {
            log::debug!("No '{}' resource configured; skipping asset scan.", ASSET_RESOURCE);
            return Ok(0);
        }

        let mut items = rm.enumerate_items(
            ASSET_RESOURCE,
            None,
            true,
            true,
            Some(vec![OsString::from("zip"), OsString::from("toml")]),
        )?;

        items.sort_by(|a, b| a.location.cmp(&b.location));

        for item in items {
            match Self::classify_file(rm, &item.location).await {
                Ok(Some(asset)) => {
                    if let Some(manifest) = asset.manifest() {
                        log::debug!(
                            "Discovered {:?} asset '{}' at {}",
                            asset.asset_type(),
                            manifest.asset_name,
                            asset.path().display()
                        );
                    }
                    else {
                        log::debug!(
                            "Discovered {:?} asset at {}",
                            asset.asset_type(),
                            asset.path().display()
                        );
                    }
                    self.assets.push(asset);
                }
                Ok(None) => {
                    log::debug!("Unrecognized asset file: {}", item.location.display());
                }
                Err(err) => {
                    log::warn!("Failed to classify asset file {}: {}", item.location.display(), err);
                }
            }
        }

        self.assets.sort_by(|a, b| a.path().cmp(b.path()));
        Ok(self.assets.len())
    }

    pub async fn classify_file(rm: &mut ResourceManager, path: impl AsRef<Path>) -> Result<Option<Asset>, Error> {
        let path = path.as_ref();
        if path
            .file_name()
            .is_some_and(|file_name| file_name == OSD_KEYBOARD_DEFINITION_FILENAME)
        {
            return Ok(Some(Asset::OsdKeyboard(OsdKeyboardAsset {
                path: path.to_path_buf(),
            })));
        }

        let is_zip = path
            .extension()
            .is_some_and(|extension| extension.to_string_lossy().eq_ignore_ascii_case("zip"));
        if !is_zip {
            return Ok(None);
        }

        let data = rm
            .read_resource_from_path(path)
            .await
            .with_context(|| format!("Failed to read asset file {}", path.display()))?;
        Self::classify_zip(path, data)
    }

    fn classify_zip(path: &Path, data: Vec<u8>) -> Result<Option<Asset>, Error> {
        let mut archive = ZipArchive::new(Cursor::new(data))
            .with_context(|| format!("Failed to open asset ZIP {}", path.display()))?;

        let mut manifest_file = match archive.by_name(ASSET_MANIFEST_FILENAME) {
            Ok(file) => file,
            Err(zip::result::ZipError::FileNotFound) => return Ok(None),
            Err(err) => {
                return Err(err).with_context(|| format!("Failed to read asset manifest from {}", path.display()));
            }
        };

        let mut manifest_toml = String::new();
        manifest_file
            .read_to_string(&mut manifest_toml)
            .with_context(|| format!("Failed to read asset manifest from {}", path.display()))?;
        let manifest: AssetManifest = toml::from_str(&manifest_toml)
            .with_context(|| format!("Failed to parse asset manifest from {}", path.display()))?;

        let asset = match manifest.asset_type {
            AssetType::SoundLibrary => Asset::SoundLibrary(SoundLibrary {
                path: path.to_path_buf(),
                manifest,
            }),
            AssetType::OsdKeyboard => {
                return Err(anyhow::anyhow!(
                    "OSD keyboard assets must be defined by a {} file",
                    OSD_KEYBOARD_DEFINITION_FILENAME
                ));
            }
        };

        Ok(Some(asset))
    }

    pub fn assets(&self) -> &[Asset] {
        &self.assets
    }

    pub fn assets_of_type(&self, asset_type: AssetType) -> impl Iterator<Item = &Asset> {
        self.assets.iter().filter(move |asset| asset.asset_type() == asset_type)
    }

    pub fn sound_libraries(&self) -> impl Iterator<Item = &SoundLibrary> {
        self.assets.iter().filter_map(|asset| match asset {
            Asset::SoundLibrary(library) => Some(library),
            Asset::OsdKeyboard(_) => None,
        })
    }

    pub fn osd_keyboards(&self) -> impl Iterator<Item = &OsdKeyboardAsset> {
        self.assets.iter().filter_map(|asset| match asset {
            Asset::SoundLibrary(_) => None,
            Asset::OsdKeyboard(keyboard) => Some(keyboard),
        })
    }

    pub fn len(&self) -> usize {
        self.assets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use zip::{write::SimpleFileOptions, ZipWriter};

    const SOUND_LIBRARY_MANIFEST: &str = r#"
asset_type = "SoundLibrary"
asset_subtype = "FloppyDriveSounds"
asset_name = "TEAC FD-55 Sound Effects"
asset_specifier = "TEAC FD-55"
"#;

    fn sound_library_zip() -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(ASSET_MANIFEST_FILENAME, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(SOUND_LIBRARY_MANIFEST.as_bytes()).unwrap();
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn classifies_sound_library_manifest_from_zip() {
        let asset = AssetManager::classify_zip(Path::new("teac_fd55.zip"), sound_library_zip())
            .unwrap()
            .unwrap();

        let Asset::SoundLibrary(library) = asset
        else {
            panic!("expected a sound library asset");
        };
        assert_eq!(library.path, PathBuf::from("teac_fd55.zip"));
        assert_eq!(library.manifest.asset_type, AssetType::SoundLibrary);
        assert_eq!(library.manifest.asset_subtype, "FloppyDriveSounds");
        assert_eq!(library.manifest.asset_name, "TEAC FD-55 Sound Effects");
        assert_eq!(library.manifest.asset_specifier, "TEAC FD-55");
    }

    #[test]
    fn ignores_zip_without_asset_manifest() {
        let writer = ZipWriter::new(Cursor::new(Vec::new()));
        let data = writer.finish().unwrap().into_inner();

        assert!(AssetManager::classify_zip(Path::new("unrelated.zip"), data)
            .unwrap()
            .is_none());
    }

    #[test]
    fn classifies_keyboard_definition_by_filename() {
        let asset = pollster::block_on(AssetManager::classify_file(
            &mut ResourceManager::new(PathBuf::new()),
            Path::new("keyboards/modelf/keyboard_def.toml"),
        ))
        .unwrap()
        .unwrap();

        let Asset::OsdKeyboard(keyboard) = asset
        else {
            panic!("expected an OSD keyboard asset");
        };
        assert_eq!(keyboard.path, PathBuf::from("keyboards/modelf/keyboard_def.toml"));
    }
}

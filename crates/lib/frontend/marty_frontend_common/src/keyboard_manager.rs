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

//! Loading of core keyboard mappings and validation of on-screen keyboard assets.

use std::{path::Path, str::FromStr};

use anyhow::{Context, Error};
use marty_common::types::keys::MartyKey;
use serde::Deserialize;

use crate::{asset_manager::AssetManager, resource_manager::ResourceManager};

#[derive(Clone, Debug)]
pub struct OsdKeyboard {
    pub keyboard_name: String,
    pub layout_name:   String,
    pub bitmap:        OsdKeyboardBitmap,
    pub keys:          Vec<OsdKey>,
    pub source_image:  Vec<u8>,
    pub bezel_image:   Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct OsdKeyboardBitmap {
    pub width:      u32,
    pub height:     u32,
    pub base_color: [u8; 4],
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct OsdKey {
    pub code:        MartyKey,
    pub source:      [u32; 4],
    pub destination: [u32; 2],
}

#[derive(Debug, Deserialize)]
struct KeyboardDefinitionFile {
    osd_keyboard:        KeyboardDefinition,
    osd_keyboard_layout: Vec<KeyboardLayoutReference>,
}

#[derive(Debug, Deserialize)]
struct KeyboardDefinition {
    keyboard_name: String,
    source_image:  String,
    bezel_image:   String,
}

#[derive(Clone, Debug, Deserialize)]
struct KeyboardLayoutReference {
    name:   String,
    config: String,
}

#[derive(Debug, Deserialize)]
struct KeyboardLayoutFile {
    bitmap: BitmapDefinition,
    key:    Vec<KeyDefinition>,
}

#[derive(Debug, Deserialize)]
struct BitmapDefinition {
    width:      u32,
    height:     u32,
    origin:     String,
    base_color: u32,
}

#[derive(Debug, Deserialize)]
struct KeyDefinition {
    code:        String,
    source:      [u32; 4],
    destination: [u32; 2],
}

#[derive(Default)]
pub struct KeyboardManager;

impl KeyboardManager {
    pub fn new() -> Self {
        Self
    }

    pub async fn load_mapping(
        &mut self,
        rm: &mut ResourceManager,
        layout_name: &str,
    ) -> Result<Option<String>, Error> {
        let Some(mut layout_path) = rm.resource_path("keyboard_layout") else {
            log::debug!("No keyboard_layout resource is configured.");
            return Ok(None);
        };

        layout_path.push(format!("keyboard_{}.toml", layout_name));
        let mapping = rm
            .read_string_from_path(&layout_path)
            .await
            .with_context(|| format!("Failed to read keyboard mapping {}", layout_path.display()))?;

        log::debug!(
            "Loaded '{}' keyboard mapping from {}",
            layout_name,
            layout_path.display()
        );
        Ok(Some(mapping))
    }

    pub async fn load_layout(
        &mut self,
        asset_manager: &AssetManager,
        rm: &mut ResourceManager,
        layout_name: &str,
    ) -> Result<Option<OsdKeyboard>, Error> {
        for keyboard_asset in asset_manager.osd_keyboards() {
            let definition_data = match rm.read_resource_from_path(&keyboard_asset.path).await {
                Ok(data) => data,
                Err(err) => {
                    log::warn!(
                        "Failed to read OSD keyboard definition {}: {}",
                        keyboard_asset.path.display(),
                        err
                    );
                    continue;
                }
            };
            let definition_text = match String::from_utf8(definition_data) {
                Ok(text) => text,
                Err(err) => {
                    log::warn!(
                        "OSD keyboard definition {} is not valid UTF-8: {}",
                        keyboard_asset.path.display(),
                        err
                    );
                    continue;
                }
            };
            let definition: KeyboardDefinitionFile = match toml::from_str(&definition_text) {
                Ok(definition) => definition,
                Err(err) => {
                    log::warn!(
                        "Failed to parse OSD keyboard definition {}: {}",
                        keyboard_asset.path.display(),
                        err
                    );
                    continue;
                }
            };

            let Some(layout_reference) = definition
                .osd_keyboard_layout
                .iter()
                .find(|layout| layout.name.eq_ignore_ascii_case(layout_name))
                .cloned()
            else {
                continue;
            };

            let keyboard = Self::load_keyboard(rm, &keyboard_asset.path, definition, layout_reference).await?;
            log::debug!(
                "Loaded OSD keyboard '{}' layout '{}' from {}",
                keyboard.keyboard_name,
                keyboard.layout_name,
                keyboard_asset.path.display()
            );
            return Ok(Some(keyboard));
        }

        log::debug!("No OSD keyboard asset provides layout '{}'.", layout_name);
        Ok(None)
    }

    async fn load_keyboard(
        rm: &mut ResourceManager,
        definition_path: &Path,
        definition_file: KeyboardDefinitionFile,
        layout_reference: KeyboardLayoutReference,
    ) -> Result<OsdKeyboard, Error> {
        let asset_directory = definition_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("OSD keyboard definition has no parent directory"))?;
        let layout_path = asset_directory.join(&layout_reference.config);
        let source_image_path = asset_directory.join(&definition_file.osd_keyboard.source_image);
        let bezel_image_path = asset_directory.join(&definition_file.osd_keyboard.bezel_image);

        let layout_data = rm
            .read_resource_from_path(&layout_path)
            .await
            .with_context(|| format!("Failed to read OSD keyboard layout {}", layout_path.display()))?;
        let layout_text = String::from_utf8(layout_data)
            .with_context(|| format!("OSD keyboard layout {} is not valid UTF-8", layout_path.display()))?;
        let layout_file: KeyboardLayoutFile = toml::from_str(&layout_text)
            .with_context(|| format!("Failed to parse OSD keyboard layout {}", layout_path.display()))?;
        let (bitmap, keys) = Self::validate_layout(layout_file)
            .with_context(|| format!("Invalid OSD keyboard layout {}", layout_path.display()))?;

        let source_image = rm
            .read_resource_from_path(&source_image_path)
            .await
            .with_context(|| format!("Failed to read OSD keyboard image {}", source_image_path.display()))?;
        let bezel_image = rm
            .read_resource_from_path(&bezel_image_path)
            .await
            .with_context(|| format!("Failed to read OSD keyboard bezel {}", bezel_image_path.display()))?;

        Ok(OsdKeyboard {
            keyboard_name: definition_file.osd_keyboard.keyboard_name,
            layout_name: layout_reference.name,
            bitmap,
            keys,
            source_image,
            bezel_image,
        })
    }

    fn validate_layout(layout: KeyboardLayoutFile) -> Result<(OsdKeyboardBitmap, Vec<OsdKey>), Error> {
        if layout.bitmap.width == 0 || layout.bitmap.height == 0 {
            return Err(anyhow::anyhow!("bitmap dimensions must be non-zero"));
        }
        if layout.bitmap.origin != "top-left" {
            return Err(anyhow::anyhow!(
                "unsupported bitmap origin '{}'; expected 'top-left'",
                layout.bitmap.origin
            ));
        }

        let base_color = rgb24_to_rgba(layout.bitmap.base_color)?;
        let mut keys = Vec::with_capacity(layout.key.len());
        for key in layout.key {
            let code = MartyKey::from_str(&key.code)
                .map_err(|_| anyhow::anyhow!("unknown MartyKey code '{}'", key.code))?;
            let [source_x, source_y, width, height] = key.source;
            if width == 0 || height == 0 {
                return Err(anyhow::anyhow!("key '{}' has an empty source rectangle", key.code));
            }
            if source_x.checked_add(width).is_none_or(|right| right > layout.bitmap.width)
                || source_y.checked_add(height).is_none_or(|bottom| bottom > layout.bitmap.height)
            {
                return Err(anyhow::anyhow!("key '{}' source rectangle is outside the bitmap", key.code));
            }
            if key.destination[0]
                .checked_add(width)
                .is_none_or(|right| right > layout.bitmap.width)
                || key.destination[1]
                    .checked_add(height)
                    .is_none_or(|bottom| bottom > layout.bitmap.height)
            {
                return Err(anyhow::anyhow!(
                    "key '{}' destination rectangle is outside the bitmap",
                    key.code
                ));
            }
            keys.push(OsdKey {
                code,
                source: key.source,
                destination: key.destination,
            });
        }

        Ok((
            OsdKeyboardBitmap {
                width: layout.bitmap.width,
                height: layout.bitmap.height,
                base_color,
            },
            keys,
        ))
    }
}

fn rgb24_to_rgba(color: u32) -> Result<[u8; 4], Error> {
    if color > 0x00FF_FFFF {
        return Err(anyhow::anyhow!(
            "base_color must be a 24-bit RGB value in the range 0x000000..=0xFFFFFF"
        ));
    }

    Ok([
        ((color >> 16) & 0xFF) as u8,
        ((color >> 8) & 0xFF) as u8,
        (color & 0xFF) as u8,
        0xFF,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    #[cfg(not(target_arch = "wasm32"))]
    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[cfg(not(target_arch = "wasm32"))]
    struct TestDirectory {
        path: std::path::PathBuf,
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "martypc-keyboard-manager-{}-{}",
                std::process::id(),
                NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self { path }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn converts_rgb24_to_opaque_rgba() {
        assert_eq!(rgb24_to_rgba(0x292929).unwrap(), [0x29, 0x29, 0x29, 0xFF]);
        assert_eq!(rgb24_to_rgba(0x123456).unwrap(), [0x12, 0x34, 0x56, 0xFF]);
        assert!(rgb24_to_rgba(0x01000000).is_err());
    }

    #[test]
    fn validates_key_codes_and_rectangles() {
        let layout = KeyboardLayoutFile {
            bitmap: BitmapDefinition {
                width: 100,
                height: 50,
                origin: "top-left".to_string(),
                base_color: 0x000000,
            },
            key: vec![KeyDefinition {
                code: "KeyA".to_string(),
                source: [10, 10, 20, 20],
                destination: [12, 12],
            }],
        };

        let (_, keys) = KeyboardManager::validate_layout(layout).unwrap();
        assert_eq!(keys[0].code, MartyKey::KeyA);
        assert_eq!(keys[0].source, [10, 10, 20, 20]);
        assert_eq!(keys[0].destination, [12, 12]);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn loads_mapping_from_keyboard_layout_resource() {
        let test_directory = TestDirectory::new();
        let layout_directory = test_directory.path.join("keyboard_layouts");
        fs::create_dir_all(&layout_directory).unwrap();
        let mapping = "[keyboard.modelf]\nkeycode_mappings = []\n";
        fs::write(layout_directory.join("keyboard_UK.toml"), mapping).unwrap();

        let mut resource_manager = ResourceManager::new(test_directory.path.clone());
        resource_manager
            .pm
            .add_path(
                "keyboard_layout",
                layout_directory.to_str().unwrap(),
                false,
                true,
            )
            .unwrap();

        let mut keyboard_manager = KeyboardManager::new();
        let loaded = pollster::block_on(keyboard_manager.load_mapping(&mut resource_manager, "UK"))
            .unwrap()
            .unwrap();

        assert_eq!(loaded, mapping);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn discovers_and_loads_matching_keyboard_asset() {
        let test_directory = TestDirectory::new();
        let keyboard_directory = test_directory.path.join("nested/modelf");
        fs::create_dir_all(&keyboard_directory).unwrap();
        fs::write(
            keyboard_directory.join("keyboard_def.toml"),
            r#"
[osd_keyboard]
keyboard_name = "modelf"
source_image = "keyboard.png"
bezel_image = "bezel.png"

[[osd_keyboard_layout]]
name = "us"
config = "layout.toml"
"#,
        )
        .unwrap();
        fs::write(
            keyboard_directory.join("layout.toml"),
            r##"
[bitmap]
width = 100
height = 50
origin = "top-left"
base_color = 0x292929

[[key]]
code = "KeyA"
source = [10, 10, 20, 20]
destination = [12, 12]
"##,
        )
        .unwrap();
        fs::write(keyboard_directory.join("keyboard.png"), b"source image").unwrap();
        fs::write(keyboard_directory.join("bezel.png"), b"bezel image").unwrap();

        let mut resource_manager = ResourceManager::new(test_directory.path.clone());
        resource_manager
            .pm
            .add_path("asset", test_directory.path.to_str().unwrap(), false, true)
            .unwrap();

        let mut asset_manager = AssetManager::new();
        pollster::block_on(asset_manager.scan_resource(&mut resource_manager)).unwrap();
        let mut keyboard_manager = KeyboardManager::new();
        let keyboard = pollster::block_on(keyboard_manager.load_layout(
            &asset_manager,
            &mut resource_manager,
            "US",
        ))
        .unwrap()
        .unwrap();

        assert_eq!(keyboard.keyboard_name, "modelf");
        assert_eq!(keyboard.layout_name, "us");
        assert_eq!(keyboard.bitmap.width, 100);
        assert_eq!(keyboard.bitmap.height, 50);
        assert!(keyboard.keys.iter().any(|key| key.code == MartyKey::KeyA));
        assert_eq!(keyboard.source_image, b"source image");
        assert_eq!(keyboard.bezel_image, b"bezel image");
    }
}

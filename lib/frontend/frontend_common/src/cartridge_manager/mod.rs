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

    frontend_common::floppy_manager.rs

    Discover floppy images in the 'floppy' resource and provide an interface
    for enumerating and loading them.

*/

use std::{
    collections::HashMap,
    ffi::OsString,
    fmt::Display,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Error};

use crate::resource_manager::{PathTreeNode, ResourceItem, ResourceManager};
use marty_common::{bytebuf::*, CartImage};

pub const JRIPCART_HEADER_LEN: usize = 512;
pub const JRIPCART_SIG_LEN: usize = 27;
pub const JRIPCART_SIG_STR: &str = "PCjr Cartridge image file";
pub const JRIPCART_CREATOR_LEN: usize = 30;
pub const JRIPCART_COMMENT_LEN: usize = 400;

pub enum CartImageType {
    JRipCart,
    PCJrCart,
}

#[derive(Debug)]
pub enum CartridgeError {
    DirNotFound,
    ImageNotFound,
    ImageFormatError,
    ImageReadError,
    FileReadError,
    FileWriteError,
}
impl std::error::Error for CartridgeError {}
impl Display for CartridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &*self {
            CartridgeError::DirNotFound => write!(f, "Couldn't find the requested directory."),
            CartridgeError::ImageNotFound => write!(f, "Specified image name could not be found in cartridge manager."),
            CartridgeError::ImageFormatError => write!(f, "Unable to detect image format."),
            CartridgeError::ImageReadError => write!(f, "Error reading cartridge image."),
            CartridgeError::FileReadError => write!(f, "A file read error occurred."),
            CartridgeError::FileWriteError => write!(f, "A file write error occurred."),
        }
    }
}

#[allow(dead_code)]
pub struct CartImageMeta {
    idx:  usize,
    name: OsString,
    path: PathBuf,
    size: u64,
}

pub struct CartridgeManager {
    files: Vec<ResourceItem>,
    image_vec: Vec<CartImageMeta>,
    image_map: HashMap<OsString, usize>,
    extensions: Vec<OsString>,
}

impl CartridgeManager {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            image_vec: Vec::new(),
            image_map: HashMap::new(),
            extensions: vec![OsString::from("jrc")],
        }
    }

    pub fn set_extensions(&mut self, extensions: Option<Vec<String>>) {
        if let Some(extensions) = extensions {
            self.extensions = extensions
                .iter()
                .map(|ext| OsString::from(ext.to_lowercase()))
                .collect();
        }
    }

    pub fn scan_resource(&mut self, rm: &ResourceManager) -> Result<bool, Error> {
        // Clear and rebuild image lists.
        self.image_vec.clear();
        self.image_map.clear();

        // Retrieve all items from the floppy resource paths.
        let floppy_items = rm.enumerate_items("cartridge", None, true, true, Some(self.extensions.clone()))?;

        // Index mapping between 'files' vec and 'image_vec' should be maintained.
        for item in floppy_items.iter() {
            let idx = self.image_vec.len();
            self.image_vec.push(CartImageMeta {
                idx,
                name: item.location.file_name().unwrap().to_os_string(),
                path: item.location.clone(),
                size: item.location.metadata().unwrap().len(),
            });

            self.image_map
                .insert(item.location.file_name().unwrap().to_os_string(), idx);
        }

        self.files = floppy_items;

        Ok(true)
    }

    pub fn make_tree(&mut self, rm: &ResourceManager) -> Result<PathTreeNode, Error> {
        let tree = rm.items_to_tree("floppy", &self.files)?;
        Ok(tree)
    }

    pub fn scan_paths(&mut self, paths: Vec<PathBuf>) -> Result<bool, CartridgeError> {
        // Clear and rebuild image lists.
        self.image_vec.clear();
        self.image_map.clear();

        // Scan through all entries in the directory and find all files with matching extension
        for path in paths {
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if self.extensions.contains(&extension.to_ascii_lowercase()) {
                        println!(
                            "Found floppy image: {:?} size: {}",
                            path,
                            path.metadata().unwrap().len()
                        );

                        let idx = self.image_vec.len();
                        self.image_vec.push(CartImageMeta {
                            idx,
                            name: path.file_name().unwrap().to_os_string(),
                            path: path.clone(),
                            size: path.metadata().unwrap().len(),
                        });

                        self.image_map.insert(path.file_name().unwrap().to_os_string(), idx);
                    }
                }
            }
        }
        Ok(true)
    }

    pub fn scan_dir(&mut self, path: &Path) -> Result<bool, CartridgeError> {
        // Read in directory entries within the provided path
        let dir = match fs::read_dir(path) {
            Ok(dir) => dir,
            Err(_) => return Err(CartridgeError::DirNotFound),
        };

        // Clear and rebuild image lists.
        self.image_vec.clear();
        self.image_map.clear();

        // Scan through all entries in the directory and find all files with matching extension
        for entry in dir {
            if let Ok(entry) = entry {
                if entry.path().is_file() {
                    if let Some(extension) = entry.path().extension() {
                        if self.extensions.contains(&extension.to_ascii_lowercase()) {
                            println!(
                                "Found floppy image: {:?} size: {}",
                                entry.path(),
                                entry.metadata().unwrap().len()
                            );

                            let idx = self.image_vec.len();
                            self.image_vec.push(CartImageMeta {
                                idx,
                                name: entry.file_name(),
                                path: entry.path(),
                                size: entry.metadata().unwrap().len(),
                            });

                            self.image_map.insert(entry.file_name(), idx);
                        }
                    }
                }
            }
        }
        Ok(true)
    }

    pub fn get_cart_names(&self) -> Vec<OsString> {
        let mut vec: Vec<OsString> = Vec::new();
        for (key, _val) in &self.image_map {
            vec.push(key.clone());
        }
        //vec.sort_by(|a, b| a.to_ascii_uppercase().cmp(&b.to_ascii_uppercase()));
        vec
    }

    pub fn get_cart_name(&self, idx: usize) -> Option<OsString> {
        if idx >= self.image_vec.len() {
            return None;
        }
        Some(self.image_vec[idx].name.clone())
    }

    pub async fn load_cart_data(&self, idx: usize, rm: &ResourceManager) -> Result<CartImage, Error> {
        let cart_vec;

        if idx >= self.image_vec.len() {
            return Err(anyhow!(CartridgeError::ImageNotFound));
        }
        let floppy_path = self.image_vec[idx].path.clone();
        cart_vec = match rm.read_resource_from_path(&floppy_path).await {
            Ok(vec) => vec,
            Err(_e) => {
                return Err(anyhow!(CartridgeError::FileReadError));
            }
        };

        let cart_image_type = scan_cart(&cart_vec);

        match cart_image_type {
            Some(CartImageType::JRipCart) => {
                let cart = read_jripcart_image(&cart_vec)?;
                Ok(cart)
            }
            Some(CartImageType::PCJrCart) => {
                log::error!("PCJrCart images currently unsupported.");
                Err(anyhow!(CartridgeError::ImageFormatError))
            }
            _ => {
                log::error!("Unknown cart image format!");
                Err(anyhow!(CartridgeError::ImageFormatError))
            }
        }
    }
}

pub fn scan_cart(bytes: &[u8]) -> Option<CartImageType> {
    let mut buf = ByteBuf::from_slice(bytes);

    let mut sig = [0u8; JRIPCART_SIG_LEN];
    match buf.read_bytes(&mut sig, JRIPCART_SIG_LEN) {
        Ok(_) => {
            let sig_str = std::str::from_utf8(&sig[0..(JRIPCART_SIG_LEN - 2)]).unwrap();

            log::debug!("scan_cart(): sig_str: {}", sig_str);
            if sig_str == JRIPCART_SIG_STR {
                return Some(CartImageType::JRipCart);
            }
        }
        Err(_e) => return None,
    }

    None
}

pub fn read_jripcart_image(bytes: &[u8]) -> Result<CartImage, Error> {
    let mut buf = ByteBuf::from_slice(bytes);

    // +  0  signature DB "PCjr Cartridge image file",0Dh,0Ah ;file signature
    if buf.len() <= JRIPCART_HEADER_LEN {
        // No image data!
        return Err(anyhow!(CartridgeError::ImageReadError));
    }

    // + 27  creator   DB 30 DUP (20h) ;creator signature
    let mut sig = [0u8; JRIPCART_SIG_LEN];
    buf.read_bytes(&mut sig, JRIPCART_SIG_LEN)?;
    let sig_str = std::str::from_utf8(&sig[0..(JRIPCART_SIG_LEN - 2)]).unwrap();
    if sig_str != JRIPCART_SIG_STR {
        return Err(anyhow!(CartridgeError::ImageReadError));
    }

    let mut creator_buf = [0u8; JRIPCART_CREATOR_LEN];
    buf.read_bytes(&mut creator_buf, JRIPCART_CREATOR_LEN)?;

    let creator = ascii_string_from_bytes(&creator_buf);
    _ = buf.read_u16_le()?; // Skip CR

    let mut comment_buf = [0u8; JRIPCART_COMMENT_LEN];
    buf.read_bytes(&mut comment_buf, JRIPCART_COMMENT_LEN)?;

    for i in 0..JRIPCART_COMMENT_LEN {
        if comment_buf[i] == 0x1A {
            comment_buf[i] = 0;
        }
    }
    let comment = ascii_string_from_bytes(&comment_buf);

    let eof = buf.read_u8()?;
    if eof != 0x1A {
        log::error!("Expected EOF (0x1A), got: {:02X}", eof);
        return Err(anyhow!(CartridgeError::ImageReadError));
    }

    let version_major = buf.read_u8()?;
    let version_minor = buf.read_u8()?;

    let address_seg = buf.read_u16_le()?;
    let address_mask = buf.read_u16_le()?;

    let image = bytes[JRIPCART_HEADER_LEN..].to_vec();

    Ok(CartImage {
        creator,
        comment,
        version_major,
        version_minor,
        address_seg,
        address_mask,
        image,
    })
}

fn ascii_string_from_bytes(bytes: &[u8]) -> String {
    let mut ascii_bytes = Vec::new();
    for &byte in bytes {
        if byte.is_ascii() {
            ascii_bytes.push(byte);
        }
        else {
            ascii_bytes.push(b' '); // ASCII space
        }
    }
    String::from_utf8(ascii_bytes).unwrap()
}

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

use crate::{
    resource_manager::{
        tree::{NodeType, TreeNode},
        PathTreeNode,
        ResourceItem,
        ResourceItemType,
        ResourceManager,
    },
    types::floppy::{FloppyImageSource, RelativeDirectory},
};
use anyhow::Error;
use fatfs::{Dir, OemCpConverter, TimeProvider};
use marty_core::device_types::fdc::FloppyImageType;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ffi::OsString,
    fmt::{write, Display},
    fs,
    fs::File,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    rc::Rc,
};
use zip::ZipArchive;

#[derive(Debug)]
pub enum FloppyError {
    DirNotFound,
    ImageNotFound,
    FileReadError,
    FileWriteError,
    ImageBuildError,
}
impl std::error::Error for FloppyError {}
impl Display for FloppyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &*self {
            FloppyError::DirNotFound => write!(f, "Couldn't find the requested directory."),
            FloppyError::ImageNotFound => write!(f, "Specified image name could not be found in floppy manager."),
            FloppyError::FileReadError => write!(f, "A file read error occurred."),
            FloppyError::FileWriteError => write!(f, "A file write error occurred."),
            FloppyError::ImageBuildError => write!(f, "Error building floppy image."),
        }
    }
}

#[allow(dead_code)]
pub struct FloppyImage {
    idx:  usize,
    name: OsString,
    path: PathBuf,
    size: u64,
}

pub struct FloppyManager {
    files: Vec<ResourceItem>,
    image_vec: Vec<FloppyImage>,
    image_map: HashMap<OsString, usize>,
    autofloppy_dir_vec: Vec<RelativeDirectory>,
    extensions: Vec<OsString>,
}

impl FloppyManager {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            image_vec: Vec::new(),
            image_map: HashMap::new(),
            autofloppy_dir_vec: Vec::new(),
            extensions: vec![OsString::from("img"), OsString::from("ima"), OsString::from("zip")],
        }
    }

    pub fn set_extensions(&mut self, extensions: Option<Vec<String>>) {
        if let Some(extensions) = extensions {
            self.extensions = extensions
                .iter()
                .map(|ext| OsString::from(ext.to_lowercase()))
                .collect();

            // Include zip files if not specified
            if !self.extensions.contains(&OsString::from("zip")) {
                self.extensions.push(OsString::from("zip"));
            }
        }
    }

    pub fn scan_resource(&mut self, rm: &ResourceManager) -> Result<bool, Error> {
        // Clear and rebuild image lists.
        self.image_vec.clear();
        self.image_map.clear();

        // Retrieve all items from the floppy resource paths.
        let floppy_items = rm.enumerate_items("floppy", true, true, Some(self.extensions.clone()))?;

        // Index mapping between 'files' vec and 'image_vec' should be maintained.
        for item in floppy_items.iter() {
            let idx = self.image_vec.len();
            self.image_vec.push(FloppyImage {
                idx,
                name: item.full_path.file_name().unwrap().to_os_string(),
                path: item.full_path.clone(),
                size: item.full_path.metadata().unwrap().len(),
            });

            self.image_map
                .insert(item.full_path.file_name().unwrap().to_os_string(), idx);
        }

        self.files = floppy_items;

        Ok(true)
    }

    pub fn scan_autofloppy(&mut self, rm: &ResourceManager) -> Result<bool, Error> {
        // Clear and rebuild autofloppy list.
        self.autofloppy_dir_vec.clear();

        // Retrieve all items from the floppy resource paths.
        let autofloppy_dirs = rm.enumerate_items("autofloppy", false, false, None)?;

        // Index mapping between 'files' vec and 'image_vec' should be maintained.
        for item in autofloppy_dirs.iter() {
            let idx = self.image_vec.len();

            if matches!(item.rtype, ResourceItemType::Directory) {
                self.autofloppy_dir_vec.push(RelativeDirectory {
                    full: item.full_path.clone(),
                    relative: item.relative_path.clone().unwrap_or(PathBuf::new()),
                    name: item.filename_only.clone().unwrap_or(OsString::new()),
                });
            }
        }

        for dir in &self.autofloppy_dir_vec {
            println!("Found autofloppy directory: {:?}", dir.name);
        }

        Ok(true)
    }

    pub fn make_tree(&mut self, rm: &ResourceManager) -> Result<PathTreeNode, Error> {
        let tree = rm.items_to_tree("floppy", &self.files)?;
        Ok(tree)
    }

    pub fn scan_paths(&mut self, paths: Vec<PathBuf>) -> Result<bool, Error> {
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
                        self.image_vec.push(FloppyImage {
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

    pub fn scan_dir(&mut self, path: &Path) -> Result<bool, FloppyError> {
        // Read in directory entries within the provided path
        let dir = match fs::read_dir(path) {
            Ok(dir) => dir,
            Err(_) => return Err(FloppyError::DirNotFound),
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
                            self.image_vec.push(FloppyImage {
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

    pub fn get_autofloppy_paths(&self) -> Vec<RelativeDirectory> {
        self.autofloppy_dir_vec.clone()
    }

    pub fn get_floppy_names(&self) -> Vec<OsString> {
        let mut vec: Vec<OsString> = Vec::new();
        for (key, _val) in &self.image_map {
            vec.push(key.clone());
        }
        //vec.sort_by(|a, b| a.to_ascii_uppercase().cmp(&b.to_ascii_uppercase()));
        vec
    }

    pub fn get_floppy_name(&self, idx: usize) -> Option<OsString> {
        if idx >= self.image_vec.len() {
            return None;
        }
        Some(self.image_vec[idx].name.clone())
    }

    pub fn load_floppy_data(&self, idx: usize, rm: &ResourceManager) -> Result<FloppyImageSource, FloppyError> {
        let floppy_vec;

        if idx >= self.image_vec.len() {
            return Err(FloppyError::ImageNotFound);
        }
        let floppy_path = self.image_vec[idx].path.clone();
        floppy_vec = match rm.read_resource_from_path(&floppy_path) {
            Ok(vec) => vec,
            Err(_e) => {
                return Err(FloppyError::FileReadError);
            }
        };

        if floppy_path.extension().unwrap().to_ascii_lowercase() == "zip" {
            Ok(FloppyImageSource::ZipArchive(floppy_vec))
        }
        else {
            Ok(FloppyImageSource::RawSectorImage(floppy_vec))
        }
    }

    pub fn build_autofloppy_image_from_zip(
        &self,
        archive: Vec<u8>,
        format: Option<FloppyImageType>,
        rm: &ResourceManager,
    ) -> Result<Vec<u8>, Error> {
        let format = format.unwrap_or(FloppyImageType::Image360K);

        let formatted_image = create_formatted_image("MartyPC", format)?;
        let mut floppy_buf = Cursor::new(formatted_image);

        let vfat12 = match fatfs::FileSystem::new(&mut floppy_buf, fatfs::FsOptions::new()) {
            Ok(fs) => fs,
            Err(err) => {
                println!("Error creating FAT filesystem: {:?}", err);
                return Err(FloppyError::ImageBuildError.into());
            }
        };

        let archive_reader = Cursor::new(archive);
        let mut zip = ZipArchive::new(archive_reader)?;

        let zip_items: Vec<ResourceItem> = zip
            .file_names()
            .filter_map(|f| {
                log::debug!("Found zip entry: {:?}", f);
                if !f.ends_with('/') {
                    Some(ResourceItem::from_filename(f))
                }
                else {
                    log::debug!("Skipping directory: {:?}", f);
                    None
                }
            })
            .collect();

        //return Err(FloppyError::ImageBuildError.into());

        let mut files_visited: HashSet<PathBuf> = HashSet::new();
        let mut io_sys: Option<PathBuf> = None;
        let mut dos_sys: Option<PathBuf> = None;

        // First, scan for the special files IO.SYS and MSDOS.SYS, as these need to be the first two files in the root directory.
        for item in &zip_items {
            let filename_only = item.filename_only.as_ref().unwrap();
            let filename = filename_only.to_str().unwrap().clone();

            if filename == "IO.SYS" {
                files_visited.insert(item.full_path.clone());
                io_sys = Some(item.full_path.clone());
            }
            else if filename == "IBMBIO.COM" {
                files_visited.insert(item.full_path.clone());
                io_sys = Some(item.full_path.clone());
            }
            else if filename == "MSDOS.SYS" {
                files_visited.insert(item.full_path.clone());
                dos_sys = Some(item.full_path.clone());
            }
            else if filename == "IBMDOS.COM" {
                files_visited.insert(item.full_path.clone());
                dos_sys = Some(item.full_path.clone());
            }
            else if filename == "KERNEL.SYS" {
                // FreeDOS only has one file - KERNEL.SYS. If we find it, use it as the IO SYS file.
                files_visited.insert(item.full_path.clone());
                io_sys = Some(item.full_path.clone());
            }
        }

        /*        // If we found IO.SYS, write it first.
        if let Some(io_sys_path) = io_sys {
            let io_sys_vec = rm.read_resource_from_path(&io_sys_path)?;
            let filename_only = io_sys_path.file_name().unwrap().to_str().unwrap();
            let mut io_sys_file = vfat12.root_dir().create_file(filename_only)?;
            log::debug!("Installing IO SYS: {}", filename_only);
            io_sys_file.write_all(&io_sys_vec)?;
            io_sys_file.flush().unwrap();
        }

        // If we found MSDOS.SYS, write it second.
        if let Some(dos_sys_path) = dos_sys {
            let dos_sys_vec = rm.read_resource_from_path(&dos_sys_path)?;
            let filename_only = dos_sys_path.file_name().unwrap().to_str().unwrap();
            let mut dos_sys_file = vfat12.root_dir().create_file(filename_only)?;
            log::debug!("Installing DOS SYS: {}", filename_only);
            dos_sys_file.write_all(&dos_sys_vec)?;
            dos_sys_file.flush().unwrap();
        }*/

        // Build tree from the rest of the files
        let file_tree = rm.items_to_tree_raw(&zip_items)?;

        log::debug!("File tree node: {:?}", file_tree);
        let src_root_node_opt = file_tree.descend(".");

        //let mut bootsector_opt = None;

        {
            let mut dst_root_dir = vfat12.root_dir();

            if let Some(src_root_node) = src_root_node_opt {
                if let Err(err) = build_autofloppy_dir(&src_root_node, dst_root_dir, rm, &mut |path: &Path| {
                    // This callback strips the first directory entry back off the tree (.\\)
                    // and converts backslashes to forward slashes.

                    let mut buf = path.to_path_buf();
                    if let Some(first_component) = buf.components().next() {
                        if first_component.as_os_str() == "." {
                            buf = buf.components().skip(1).collect();
                        }
                    }

                    if let Some(path_str) = buf.to_str() {
                        let zip_path = path_str.replace("\\", "/");

                        log::debug!("Reading file from zip: {}", zip_path);
                        let mut file = zip.by_name(&zip_path)?;
                        let mut file_vec = Vec::new();
                        file.read_to_end(&mut file_vec)?;
                        Ok(file_vec)
                    }
                    else {
                        Err(FloppyError::ImageBuildError.into())
                    }
                }) {
                    log::error!("Error building autofloppy directory: {}", err);
                }
            }
        }

        vfat12.unmount()?;

        let mut buf = floppy_buf.into_inner();

        /*        // Did we find a boot sector file? if so, load it now
        if let Some(bootsector_path) = bootsector_opt {
            let mut bootsector_vec = rm.read_resource_from_path(&bootsector_path)?;

            if bootsector_vec.len() > 0 {
                if bootsector_vec.len() < 512 {
                    bootsector_vec.extend(vec![0u8; 512 - bootsector_vec.len()]);
                }
                else if bootsector_vec.len() > 512 {
                    bootsector_vec.truncate(512);
                }

                log::debug!(
                    "Installing bootsector of len: {} into autofloppy image...",
                    bootsector_vec.len()
                );
                buf[..512].copy_from_slice(&bootsector_vec);
            }
        }*/

        //log::debug!("Created image of size: {}", image_buf.len());

        let mut file = std::fs::File::create("fat_dump.img").map_err(|_| FloppyError::ImageBuildError)?;
        file.write_all(&buf).map_err(|_| FloppyError::ImageBuildError)?;

        Ok(buf.clone())
    }

    pub fn build_autofloppy_image_from_dir(
        &self,
        path: &PathBuf,
        format: Option<FloppyImageType>,
        rm: &ResourceManager,
    ) -> Result<Vec<u8>, Error> {
        let format = format.unwrap_or(FloppyImageType::Image360K);

        let formatted_image = create_formatted_image("MartyPC", format)?;
        let mut floppy_buf = Cursor::new(formatted_image);

        let vfat12 = match fatfs::FileSystem::new(&mut floppy_buf, fatfs::FsOptions::new()) {
            Ok(fs) => fs,
            Err(err) => {
                println!("Error creating FAT filesystem: {:?}", err);
                return Err(FloppyError::ImageBuildError.into());
            }
        };

        let dir_items = rm.enumerate_items("autofloppy", false, true, None)?;

        let mut files_visited: HashSet<PathBuf> = HashSet::new();
        let mut io_sys: Option<PathBuf> = None;
        let mut dos_sys: Option<PathBuf> = None;
        let mut bootsector_opt = None;

        // First, scan for the special files IO.SYS and MSDOS.SYS, as these need to be the first two files in the root directory.
        for item in &dir_items {
            let filename_only = item.filename_only.as_ref().unwrap();
            let filename = filename_only.to_str().unwrap().clone();

            if filename == "IO.SYS" {
                files_visited.insert(item.full_path.clone());
                io_sys = Some(item.full_path.clone());
            }
            else if filename == "IBMBIO.COM" {
                files_visited.insert(item.full_path.clone());
                io_sys = Some(item.full_path.clone());
            }
            else if filename == "MSDOS.SYS" {
                files_visited.insert(item.full_path.clone());
                dos_sys = Some(item.full_path.clone());
            }
            else if filename == "IBMDOS.COM" {
                files_visited.insert(item.full_path.clone());
                dos_sys = Some(item.full_path.clone());
            }
            else if filename == "KERNEL.SYS" {
                // FreeDOS only has one file - KERNEL.SYS. If we find it, use it as the IO SYS file.
                files_visited.insert(item.full_path.clone());
                io_sys = Some(item.full_path.clone());
            }
            else if filename.to_lowercase() == "bootsector.bin" {
                files_visited.insert(item.full_path.clone());
                bootsector_opt = Some(item.full_path.clone());
            }
        }

        // If we found IO.SYS, write it first.
        if let Some(io_sys_path) = io_sys {
            let io_sys_vec = rm.read_resource_from_path(&io_sys_path)?;
            let filename_only = io_sys_path.file_name().unwrap().to_str().unwrap();
            let mut io_sys_file = vfat12.root_dir().create_file(filename_only)?;
            log::debug!("Installing IO SYS: {}", filename_only);
            io_sys_file.write_all(&io_sys_vec)?;
            io_sys_file.flush().unwrap();
        }

        // If we found MSDOS.SYS, write it second.
        if let Some(dos_sys_path) = dos_sys {
            let dos_sys_vec = rm.read_resource_from_path(&dos_sys_path)?;
            let filename_only = dos_sys_path.file_name().unwrap().to_str().unwrap();
            let mut dos_sys_file = vfat12.root_dir().create_file(filename_only)?;
            log::debug!("Installing DOS SYS: {}", filename_only);
            dos_sys_file.write_all(&dos_sys_vec)?;
            dos_sys_file.flush().unwrap();
        }

        // Build tree from the rest of the files
        let file_tree = rm.items_to_tree("autofloppy", &dir_items)?;
        let src_root_node_opt = file_tree.descend(path.file_name().unwrap().to_str().unwrap());

        {
            let mut dst_root_dir = vfat12.root_dir();

            if let Some(src_root_node) = src_root_node_opt {
                if let Err(err) = build_autofloppy_dir(&src_root_node, dst_root_dir, rm, &mut |path: &Path| {
                    rm.read_resource_from_path(path)
                }) {
                    log::error!("Error building autofloppy directory: {:?}", err);
                }
            }
        }
        /*
        for item in &dir_items {
            let filename_only = item.filename_only.as_ref().unwrap();
            let filename = filename_only.to_str().unwrap().clone();

            if filename.to_lowercase() == "bootsector.bin" {
                bootsector_opt = Some(item.full_path.clone());
                continue;
            }

            let file_vec = rm.read_resource_from_path(&item.full_path)?;

            if files_visited.get(&item.full_path).is_some() {
                // Skip files we have already processed, like IO.SYS and MSDOS.SYS
                log::debug!("Skipping previously installed file: {:?}", item.full_path);
                continue;
            }

            log::debug!("Writing file: {:?} size: {}", item.full_path.display(), file_vec.len());

            let mut file = vfat12.root_dir().create_file(filename)?;
            file.write_all(&file_vec)?;
            file.flush().unwrap();
        }*/

        vfat12.unmount()?;

        let mut buf = floppy_buf.into_inner();

        // Did we find a boot sector file? if so, load it now
        if let Some(bootsector_path) = bootsector_opt {
            let mut bootsector_vec = rm.read_resource_from_path(&bootsector_path)?;

            if bootsector_vec.len() > 0 {
                if bootsector_vec.len() < 512 {
                    bootsector_vec.extend(vec![0u8; 512 - bootsector_vec.len()]);
                }
                else if bootsector_vec.len() > 512 {
                    bootsector_vec.truncate(512);
                }

                log::debug!(
                    "Installing bootsector of len: {} into autofloppy image...",
                    bootsector_vec.len()
                );
                buf[..512].copy_from_slice(&bootsector_vec);
            }
        }

        //log::debug!("Created image of size: {}", image_buf.len());

        let mut file = std::fs::File::create("fat_dump.img").map_err(|_| FloppyError::ImageBuildError)?;
        file.write_all(&buf).map_err(|_| FloppyError::ImageBuildError)?;

        Ok(buf.clone())
    }

    /*
    pub fn load_floppy_data(&self, name: &OsString) -> Result<Vec<u8>, FloppyError> {
        let mut floppy_vec = Vec::new();
        if let Some(idx) = self.image_map.get(name) {
            if *idx >= self.image_vec.len() {
                return Err(FloppyError::ImageNotFound);
            }
            let floppy_path = self.image_vec[*idx].path.clone();
            floppy_vec = match std::fs::read(&floppy_path) {
                Ok(vec) => vec,
                Err(_e) => {
                    return Err(FloppyError::FileReadError);
                }
            };
        }
        Ok(floppy_vec)
    }*/

    // pub fn save_floppy_data(&self, data: &[u8], name: &OsString) -> Result<(), FloppyError> {
    //     if let Some(idx) = self.image_map.get(name) {
    //         if *idx >= self.image_vec.len() {
    //             return Err(FloppyError::ImageNotFound);
    //         }
    //         let floppy_path = self.image_vec[*idx].path.clone();
    //         match std::fs::write(&floppy_path, data) {
    //             Ok(_) => Ok(()),
    //             Err(_e) => {
    //                 return Err(FloppyError::FileWriteError);
    //             }
    //         }
    //     }
    //     else {
    //         Err(FloppyError::ImageNotFound)
    //     }
    // }

    pub fn save_floppy_data(&self, data: &[u8], idx: usize, _rm: &ResourceManager) -> Result<PathBuf, FloppyError> {
        if idx >= self.image_vec.len() {
            return Err(FloppyError::ImageNotFound);
        }

        let floppy_path = self.image_vec[idx].path.clone();
        // TODO: Implement write through resource manager instead of direct file access.
        match std::fs::write(&floppy_path, data) {
            Ok(_) => Ok(floppy_path.clone()),
            Err(_e) => {
                return Err(FloppyError::FileWriteError);
            }
        }
    }
}

fn create_formatted_image(label: &str, format: FloppyImageType) -> Result<Vec<u8>, Error> {
    let (bps, bpc, mrde, spt, heads, media_byte, image_size) = match format {
        FloppyImageType::Image360K => (512, 2 * 512, 0x70, 9, 2, 0xFD, 368_640),
        FloppyImageType::Image720K => (512, 2 * 512, 0x70, 9, 2, 0xF9, 737_280),
        FloppyImageType::Image12M => (512, 2 * 512, 0xE0, 15, 2, 0xF9, 1_228_800),
        FloppyImageType::Image144M => (512, 2 * 512, 0xE0, 18, 2, 0xF0, 1_474_560),
        _ => {
            return Err(anyhow::anyhow!("Unsupported floppy image format: {:?}", format));
        }
    };

    log::debug!("Formatting an {:?} format floppy with label: {}", format, label);

    let mut floppy_buf = fatfs::StdIoWrapper::new(Cursor::new(vec![0u8; image_size]));
    let label = create_drive_label(label);

    fatfs::format_volume(
        &mut floppy_buf,
        fatfs::FormatVolumeOptions::new()
            .fat_type(fatfs::FatType::Fat12)
            .volume_label(label)
            .bytes_per_sector(bps)
            .bytes_per_cluster(bpc)
            .max_root_dir_entries(mrde)
            .sectors_per_track(spt)
            .heads(heads)
            .media(media_byte)
            .drive_num(0),
    )?;

    Ok(floppy_buf.into_inner().into_inner())
}

fn create_drive_label(input: &str) -> [u8; 11] {
    let max_length = 11;
    let trimmed = if input.len() > max_length {
        input[..max_length].to_string().to_ascii_uppercase()
    }
    else {
        input.to_string().to_ascii_uppercase()
    };

    format!("{:<width$}", trimmed, width = max_length)
        .bytes()
        .collect::<Vec<u8>>()
        .try_into()
        .unwrap()
}

pub fn build_autofloppy_dir<IO: fatfs::Read + fatfs::Write + fatfs::Seek, TP, OCC>(
    dir_node: &TreeNode,
    fs: Dir<IO, TP, OCC>,
    rm: &ResourceManager,
    file_callback: &mut dyn FnMut(&Path) -> Result<Vec<u8>, Error>,
) -> Result<(), Error>
where
    OCC: OemCpConverter,
    TP: TimeProvider,
{
    log::debug!(
        "build_autofloppy_dir: Processing {} children...",
        dir_node.children().len()
    );
    for (i, entry) in dir_node.children().iter().enumerate() {
        let filename = entry.name();

        log::debug!("Processing child {}", i);
        match entry.node_type() {
            NodeType::File(path) => {
                log::debug!("build_autofloppy_dir: file_name: {}", path.display());

                let file_vec = file_callback(Path::new(path))?;
                let mut file = fs
                    .create_file(&filename)
                    .map_err(|_| anyhow::anyhow!("Failed to create file: {}", filename))?;
                use fatfs::Write;
                file.write_all(&file_vec)
                    .map_err(|_| anyhow::anyhow!("Failed to write file: {}", filename))?;
                file.flush()
                    .map_err(|_| anyhow::anyhow!("Failed to flush file: {}", filename))?;
            }
            NodeType::Directory(_) => {
                log::debug!("build_autofloppy_dir: dir_name: {}", filename);

                let new_dir = fs
                    .create_dir(&filename)
                    .map_err(|_| anyhow::anyhow!("Failed to create directory: {}", filename))?;
                build_autofloppy_dir(entry, new_dir, rm, file_callback)?;
            }
        }
        log::debug!("Completed processing child {}", i);
    }

    Ok(())
}

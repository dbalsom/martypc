/*
    MartyPC Emulator 
    (C)2023 Daniel Balsom
    https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

    --------------------------------------------------------------------------

    floppy_manager.rs

    Enumerate images in the 'floppy' directory to allow floppy selection 
    from within the GUI.

*/

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    ffi::OsString,
    fs,
    error::Error,
    fmt::Display
};

#[derive(Debug)]
pub enum FloppyError {
    DirNotFound,
    ImageNotFound,
    FileReadError,
    FileWriteError,
}
impl Error for FloppyError {}
impl Display for FloppyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &*self {
            FloppyError::DirNotFound => write!(f, "Couldn't find the requested directory."),
            FloppyError::ImageNotFound => write!(f, "Specified image name could not be found in floppy manager."),
            FloppyError::FileReadError => write!(f, "A file read error occurred."),
            FloppyError::FileWriteError => write!(f, "A file write error occurred."),
        }
    }
}

#[allow(dead_code)]
pub struct FloppyImage {
    path: PathBuf,
    size: u64
}

pub struct FloppyManager {
    image_vec: Vec<FloppyImage>,
    image_map: HashMap<OsString, FloppyImage>
}

impl FloppyManager {
    pub fn new() -> Self {
        Self {
            image_vec: Vec::new(),
            image_map: HashMap::new()
        }
    }

    pub fn scan_dir(&mut self, path: &Path) -> Result<bool, FloppyError> {

        // Read in directory entries within the provided path
        let dir = match fs::read_dir(path) {
            Ok(dir) => dir,
            Err(_) => return Err(FloppyError::DirNotFound)
        };

        let extensions = ["img", "ima"];

        // Clear and rebuild image lists.
        self.image_vec.clear();
        self.image_map.clear();

        // Scan through all entries in the directory and find all files with matching extension
        for entry in dir {
            if let Ok(entry) = entry {
                if entry.path().is_file() {
                    if let Some(extension) = entry.path().extension() {
                        if extensions.contains(&extension.to_string_lossy().to_lowercase().as_ref()) {

                            println!("Found floppy image: {:?} size: {}", entry.path(), entry.metadata().unwrap().len());
                            
                            self.image_vec.push( 
                                FloppyImage {
                                    path: entry.path(),
                                    size: entry.metadata().unwrap().len()
                                }
                            );
                        
                            self.image_map.insert(entry.file_name(), 
                                FloppyImage { 
                                    path: entry.path(),
                                    size: entry.metadata().unwrap().len()
                                 }
                            );
                        }
                    }
                }
            }
        }
        Ok(true)
    }


    pub fn get_floppy_names(&self) -> Vec<OsString> {
        let mut vec: Vec<OsString> = Vec::new();
        for (key, _val) in &self.image_map {
            vec.push(key.clone());
        }
        vec.sort_by(|a, b| a.to_ascii_uppercase().cmp(&b.to_ascii_uppercase()));
        vec
    }

    pub fn load_floppy_data(&self, name: &OsString ) -> Result<Vec<u8>, FloppyError> {

        let mut floppy_vec = Vec::new();
        if let Some(floppy) = self.image_map.get(name) {
            floppy_vec = match std::fs::read(&floppy.path) {
                Ok(vec) => vec,
                Err(e) => {
                    eprintln!("Couldn't open floppy image: {}", e);
                    return Err(FloppyError::FileReadError);
                }
            };
        }
        Ok(floppy_vec)
    }

    pub fn save_floppy_data(&self, data: &[u8], name: &OsString ) -> Result<(), FloppyError> {

        if let Some(floppy) = self.image_map.get(name) {

            match std::fs::write(&floppy.path, data) {
                Ok(_) => Ok(()),
                Err(e) => {
                    eprintln!("Couldn't save floppy image: {}", e);
                    return Err(FloppyError::FileWriteError)
                }
            }
        }
        else {
            Err(FloppyError::ImageNotFound)
        }
    }    

}

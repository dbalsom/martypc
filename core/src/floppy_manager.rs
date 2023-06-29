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

/*
    floppy_browser.rc
    Enumerate images in the /floppy directory to allow floppy selection

*/

use std::collections::HashMap;
use std::path::PathBuf;
use std::ffi::OsString;
use std::fs;

pub enum FloppyError {
    DirNotFound,
    FileReadError,
}

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

    pub fn scan_dir(&mut self, path: &str) -> Result<bool, FloppyError> {

        // Read in directory entries within the provided path
        let dir = match fs::read_dir(path) {
            Ok(dir) => dir,
            Err(_) => return Err(FloppyError::DirNotFound)
        };

        // Scan through all entries in the directory
        for entry in dir {
            if let Ok(entry) = entry {
                if entry.path().is_file() {

                    println!("Found floppy image: {:?} size: {}", entry.path(), entry.metadata().unwrap().len());
                    self.image_vec.push( FloppyImage {
                        path: entry.path(),
                        size: entry.metadata().unwrap().len()
                    });

                    self.image_map.insert(entry.file_name(), 
                        FloppyImage { 
                            path: entry.path(),
                            size: entry.metadata().unwrap().len()
                         });
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
        vec.sort_by(|a, b| a.cmp(b));
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

}

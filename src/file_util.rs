
use std::path::{Path, PathBuf};

pub fn find_unique_filename(path: &Path, base: &str, ext: &str) -> PathBuf {
    
    let mut i = 1;
    let mut test_path = path.join(format!("{}{:03}.{}", base, i, ext));

    while test_path.exists() {
        i += 1;
        test_path = path.join(format!("{}{:03}.{}", base, i, ext));
    }

    test_path
}
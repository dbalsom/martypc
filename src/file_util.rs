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

    file_util.rs

    Miscellaneous file utility routines.
*/

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
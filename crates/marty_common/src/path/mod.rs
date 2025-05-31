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

   ---------------------------------------------------------------------------

   common::src/path/mod.rs

    Common path routines
*/

use std::path::{Path, PathBuf};

/// Find a unique filename in the specified directory, using the specified base name and extension.
/// If `last_file` is provided, it will be used to determine the starting index for the search, which
/// will drastically speed things up as without it a binary search must be performed.
pub fn find_unique_filename<P>(dir: &Path, base: &str, ext: &str, last_file: Option<&P>) -> PathBuf
where
    P: AsRef<Path>,
{
    // Helper: parse index from filename like "baseNNN.ext"
    fn parse_index_from_path(path: &Path, base: &str) -> Option<usize> {
        let file_stem = path.file_stem()?.to_str()?;
        if !file_stem.starts_with(base) {
            return None;
        }
        let idx_str = &file_stem[base.len()..];
        idx_str.parse::<usize>().ok()
    }

    fn make_path(dir: &Path, base: &str, ext: &str, index: usize) -> PathBuf {
        dir.join(format!("{base}{index:04}.{ext}"))
    }

    // Get last index hint if possible
    let last_index = last_file.as_ref().and_then(|p| parse_index_from_path(p.as_ref(), base));
    // Determine starting index to check:
    // - If hint is given and valid, start from one past that index
    // - Otherwise, start from 0
    let mut lower = match last_index {
        Some(idx) => idx + 1,
        None => 0,
    };

    let candidate = make_path(dir, base, ext, lower);
    // if the candidate file at `lower` index does not exist, we can return it immediately
    if !candidate.exists() {
        return candidate;
    }

    // Find an upper bound index where the file does NOT exist by doubling `upper`
    let mut upper = if lower == 0 { 1 } else { lower * 2 };
    while make_path(dir, base, ext, upper).exists() {
        lower = upper;
        upper *= 2;
    }

    // Perform binary search between lower and upper bounds to find the smallest free index
    while lower + 1 < upper {
        let mid = (lower + upper) / 2;
        let mid_path = make_path(dir, base, ext, mid);
        if mid_path.exists() {
            lower = mid;
        }
        else {
            upper = mid;
        }
    }

    make_path(dir, base, ext, upper)
}

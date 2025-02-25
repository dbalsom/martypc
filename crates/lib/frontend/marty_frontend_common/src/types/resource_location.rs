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

    --------------------------------------------------------------------------
*/

//! A [ResourceLocation] enum allows specifying a resource location as either a local file path or
//! a URL.
//! Conversion methods are provided that will default to a `FilePath` variant.  URL support is gated
//! by the `url` feature, and `Url` variants must be specifically constructed.

use std::path::PathBuf;

#[cfg(feature = "use_url")]
use url::Url;

#[derive(Clone, Debug)]
pub enum ResourceLocation {
    FilePath(PathBuf), // Native paths
    #[cfg(feature = "use_url")]
    Url(Url), // Absolute or relative URLs
}

// Implement From<&str> to allow easy conversion
impl From<&str> for ResourceLocation {
    fn from(s: &str) -> Self {
        ResourceLocation::FilePath(PathBuf::from(s))
    }
}

// Optionally support From<String> too
impl From<String> for ResourceLocation {
    fn from(s: String) -> Self {
        ResourceLocation::FilePath(PathBuf::from(s))
    }
}

// Allow conversion from PathBuf directly
impl From<PathBuf> for ResourceLocation {
    fn from(path: PathBuf) -> Self {
        ResourceLocation::FilePath(path)
    }
}

// Allow conversion from Url directly
#[cfg(feature = "use_url")]
impl From<Url> for ResourceLocation {
    fn from(url: Url) -> Self {
        ResourceLocation::Url(url)
    }
}

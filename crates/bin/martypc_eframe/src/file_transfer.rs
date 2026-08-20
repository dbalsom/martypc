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

    --------------------------------------------------------------------------
*/

use marty_frontend_common::resource_manager::ResourceManager;

pub(crate) enum NonInteractiveFileLoadError {
    NotFound(String),
    Other(String),
}

pub(crate) fn file_transfer_basename(filename: &str) -> Result<&str, String> {
    filename
        .rsplit(['/', '\\', ':'])
        .next()
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .ok_or_else(|| format!("Invalid file transfer filename: {filename}"))
}

pub(crate) fn load_non_interactive_file(
    resource_manager: &mut ResourceManager,
    filename: &str,
) -> Result<(String, Vec<u8>), NonInteractiveFileLoadError> {
    let basename = file_transfer_basename(filename)
        .map_err(NonInteractiveFileLoadError::Other)?
        .to_string();
    if resource_manager.resource_path("file_transfer").is_none() {
        return Err(NonInteractiveFileLoadError::Other(
            "Resource path not found: file_transfer".to_string(),
        ));
    }
    let path = resource_manager
        .resolve_path_from_filename("file_transfer", &basename)
        .map_err(|error| NonInteractiveFileLoadError::NotFound(error.to_string()))?;
    let data = resource_manager
        .read_resource_from_path_blocking(&path)
        .map_err(|error| NonInteractiveFileLoadError::Other(error.to_string()))?;
    Ok((basename, data))
}

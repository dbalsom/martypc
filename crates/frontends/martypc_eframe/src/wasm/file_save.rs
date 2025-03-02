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

use anyhow::{anyhow, Result};
use wasm_bindgen::prelude::*;

use web_sys::{js_sys, window, Blob, BlobPropertyBag, Url};

/// Initiate a file save operation from the browser, saving the provided `bytes` with the given
/// suggested filename as `path`.
pub fn save_file_dialog(path: &str, bytes: &[u8]) -> Result<()> {
    let filename = path.rsplit('/').next().ok_or_else(|| anyhow!("Invalid path"))?;

    // Convert the bytes to a `Uint8Array` for compatibility with JavaScript
    log::debug!("Saving file as: {}, byte dump: {:0X?}", path, &bytes[0..16]);

    // I don't really understand this sequence of operations, but attempting to use the uint8_array
    // directly doesn't seem to work. Working code shamelessly taken from:
    // https://stackoverflow.com/questions/69556755/web-sysurlcreate-object-url-with-blobblob-not-formatting-binary-data-co
    let uint8_array = js_sys::Uint8Array::new(&unsafe { js_sys::Uint8Array::view(bytes) }.into());
    let array = js_sys::Array::new();
    array.push(&uint8_array.buffer());

    // Create a new `Blob` from the `Uint8Array`
    let bag = BlobPropertyBag::new();
    bag.set_type("application/octet-stream");
    let blob =
        Blob::new_with_u8_array_sequence_and_options(&array, &bag).map_err(|_| anyhow!("Failed to create Blob"))?;

    // Create an object URL for the Blob
    let url = Url::create_object_url_with_blob(&blob).map_err(|_| anyhow!("Failed to create object URL for Blob"))?;

    log::debug!("url: {:?}", url);

    // Use the window object to create an `a` element
    let window = window().ok_or_else(|| anyhow!("Failed to get window object"))?;
    let document = window
        .document()
        .ok_or_else(|| anyhow!("Failed to get document object"))?;
    let a = document
        .create_element("a")
        .map_err(|_| anyhow!("Failed to create anchor element"))?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| anyhow!("Failed to cast element to HtmlAnchorElement"))?;

    // Set the href attribute to the Blob URL and the download attribute to the desired file name
    a.set_href(&url);
    a.set_download(filename);

    // Programmatically click the `a` element to trigger the download
    a.click();

    // Revoke the Blob URL to free resources
    //Url::revoke_object_url(&url).map_err(|_| anyhow!("Failed to revoke object URL"))?;

    Ok(())
}

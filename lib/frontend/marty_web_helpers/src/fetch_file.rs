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
*/

use url::Url;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};

pub fn fetch_file(url: &Url) -> Result<Vec<u8>, String> {
    let runtime = wasm_futures::local_pool::LocalPool::new();
    runtime.run_until(async {
        // Create a GET request
        let mut opts = RequestInit::new();
        opts.method("GET");
        opts.mode(RequestMode::Cors);

        let request = Request::new_with_str_and_init(url.as_str(), &opts)
            .map_err(|e| format!("Failed to create request: {:?}", e))?;

        // Perform the fetch operation
        let window = web_sys::window().ok_or("No window object available")?;
        let response = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|e| format!("Network error: {:?}", e))?;

        let response: Response = response
            .dyn_into()
            .map_err(|e| format!("Failed to cast response: {:?}", e))?;

        // Check HTTP status code
        if !response.ok() {
            return Err(format!("HTTP error: {} {}", response.status(), response.status_text()));
        }

        // Read the response body as an ArrayBuffer
        let data = JsFuture::from(response.array_buffer().map_err(|e| format!("{:?}", e))?)
            .await
            .map_err(|e| format!("Failed to read response body: {:?}", e))?;

        let buffer = js_sys::Uint8Array::new(&data);
        Ok(buffer.to_vec())
    })
}

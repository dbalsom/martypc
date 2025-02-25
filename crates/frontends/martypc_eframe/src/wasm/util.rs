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

use eframe::{egui::Image, wasm_bindgen::prelude::wasm_bindgen};
use std::env;
use url::Url;

#[wasm_bindgen(module = "/assets/base_url.js")]
extern "C" {
    fn getBaseURL() -> String;
}

pub fn get_base_url() -> Url {
    let base_url_string = getBaseURL();

    Url::parse(&base_url_string).unwrap_or_else(|_| {
        Url::parse(&env::var("MARTYPC_BASE_URL").unwrap_or("http://localhost:8080".to_string())).unwrap()
    })
}

pub fn construct_full_url(relative_path: &str) -> String {
    let mut path_components = Vec::new();
    let base_url = getBaseURL();

    path_components.push(base_url.trim_end_matches('/'));
    let base_path = option_env!("URL_PATH");
    if let Some(base_path) = base_path {
        path_components.push(base_path.trim_start_matches('/').trim_end_matches('/'));
    }
    path_components.push(relative_path);

    let url = path_components.join("/");
    url
}

pub(crate) fn get_logo_image<'a>() -> Image<'a> {
    let url = construct_full_url("assets/fluxfox_logo.png");
    egui::Image::new(url).fit_to_original_size(1.0)
}

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

use crossbeam_channel::Sender;
use gloo_timers::callback::Timeout;
use web_time::{Duration, Instant};

use crate::FetchResult;
use url::Url;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{js_sys::Uint8Array, Request, RequestInit, RequestMode, Response};

pub async fn fetch_url(url: &Url) -> Result<Vec<u8>, String> {
    fetch_file(url.as_str()).await
}

pub async fn fetch_file(url: &str) -> Result<Vec<u8>, String> {
    // Create a GET request
    let mut opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(RequestMode::Cors);

    let request = Request::new_with_str_and_init(url, &opts)
        .map_err(|e| format!("fetch_file(): Failed to create request: {:?}", e))?;

    // Fetch the resource
    let window = web_sys::window().ok_or("No window object available")?;
    log::debug!("fetch_file(): Making Request for url {}...", url);
    let response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("fetch_file(): Network error: {:?}", e))?;

    let response: Response = response
        .dyn_into()
        .map_err(|_| "fetch_file(): Failed to cast response".to_string())?;

    // Check the HTTP status
    if !response.ok() {
        log::error!(
            "fetch_file(): HTTP error: {} {}",
            response.status(),
            response.status_text()
        );
        return Err(format!("HTTP error: {} {}", response.status(), response.status_text()));
    }

    log::debug!("fetch_file(): Got response code {}", response.status());

    // Read the response body as an ArrayBuffer
    let data = JsFuture::from(response.array_buffer().map_err(|e| format!("{:?}", e))?)
        .await
        .map_err(|e| format!("fetch_file(): Failed to read response body: {:?}", e))?;

    let buffer = Uint8Array::new(&data);
    let vec = buffer.to_vec();
    log::debug!("fetch_file(): Read {} bytes from response body.", vec.len());

    Ok(vec)
}

pub fn fetch_via_sender(url: &str, sender: Sender<FetchResult>) {
    // Must clone the sender or we'll lose it!
    let inner_sender = sender.clone();
    let inner_url = url.to_string();

    wasm_bindgen_futures::spawn_local(async move {
        log::debug!("fetch(): Async task started");
        let bytes = fetch_file(&inner_url).await;
        log::debug!("fetch(): fetch_file() awaited");

        // Send the result back to the main thread
        match bytes {
            Ok(bytes) => {
                log::debug!("fetch(): Sending result to main thread");
                inner_sender.send(FetchResult::Ok(bytes)).unwrap();
            }
            Err(e) => {
                log::error!("fetch(): Failed to fetch file: {:?}", e);
                inner_sender.send(FetchResult::Err(e)).unwrap();
            }
        }
    });
}

pub fn fetch_file_blocking(url: &str, timeout_f32: f32) -> Result<Vec<u8>, String> {
    // Create a mpsc channel to communicate between async and non-async parts
    let (sender, receiver) = crossbeam_channel::unbounded();

    // Spawn the async task
    fetch_via_sender(url, sender.clone());

    _ = yield_to_browser();

    // Wait for the result with a timeout
    let timeout = Duration::from_secs_f32(timeout_f32);
    let start = Instant::now();

    loop {
        if let Ok(result) = receiver.try_recv() {
            return match result {
                FetchResult::Ok(bytes) => {
                    log::debug!("fetch_file_blocking(): Got result");
                    Ok(bytes)
                }
                FetchResult::Err(e) => {
                    log::error!("fetch_file_blocking(): Failed to fetch file: {:?}", e);
                    Err(e)
                }
            };
        }
        else {
            log::debug!("fetch_file_blocking(): Waiting for result...");
        }

        if Instant::now() - start >= timeout {
            return Err("Fetch operation timed out".to_string());
        }
        else {
            log::debug!(
                "timeout left: {:.3}",
                (timeout - (Instant::now() - start)).as_secs_f32()
            );
        }

        // Yield to the browser's event loop to allow async tasks to progress
        _ = yield_to_browser();
    }
}

async fn yield_to_browser() {
    sleep_ms(1).await;
}

// Hack to get async sleep on wasm
// from EGUI source
async fn sleep_ms(millis: i32) {
    let mut cb = |resolve: web_sys::js_sys::Function, _reject: web_sys::js_sys::Function| {
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, millis)
            .expect("Failed to call set_timeout");
    };
    let p = web_sys::js_sys::Promise::new(&mut cb);
    wasm_bindgen_futures::JsFuture::from(p).await.unwrap();
}

/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

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

use std::sync::Arc;

use marty_frontend_common::thread_events::{FileOpenContext, FileSelectionContext, FrontendThreadEvent};

use fluxfox::DiskImage;

use anyhow::{anyhow, Error};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{js_sys::Uint8Array, window};

pub fn open_file(
    context: FileOpenContext,
    sender: crossbeam_channel::Sender<FrontendThreadEvent<Arc<DiskImage>>>,
) -> Result<(), Error> {
    let path = match context {
        FileOpenContext::ServiceHostFile { .. } => {
            return Err(anyhow!("ServiceHostFile not supported by URL file open"));
        }
        FileOpenContext::CassetteImage { ref fsc } => match fsc {
            FileSelectionContext::Path(path) => path,
            FileSelectionContext::Index(_) => return Err(anyhow!("Index context not supported on wasm")),
            FileSelectionContext::Uninitialized => return Err(anyhow!("Uninitialized context!")),
        },
        FileOpenContext::FloppyDiskImage { drive_select, ref fsc } => match fsc {
            FileSelectionContext::Path(path) => path,
            FileSelectionContext::Index(index) => return Err(anyhow!("Index context not supported on wasm")),
            FileSelectionContext::Uninitialized => return Err(anyhow!("Uninitialized context!")),
        },
        FileOpenContext::CartridgeImage { slot_select, fsc } => {
            return Err(anyhow!("Cartridge image not supported on wasm"));
        }
    };

    // Convert path to a URL
    let url = path.to_string_lossy().to_string();

    let inner_path = path.clone();
    let inner_context = context.clone();

    // Fetch the file using web_sys
    spawn_local(async move {
        let window = window().expect("No global `window` exists");
        let fetch_promise = window.fetch_with_str(&url);
        let response = match JsFuture::from(fetch_promise).await {
            Ok(resp) => resp.dyn_into::<web_sys::Response>().unwrap(),
            Err(err) => {
                log::error!("Failed to fetch file: {:?}", err);
                return;
            }
        };

        if !response.ok() {
            log::error!("Failed to fetch file: HTTP status {}", response.status());
            return;
        }

        let array_buffer_promise = response.array_buffer().unwrap();
        let array_buffer = match JsFuture::from(array_buffer_promise).await {
            Ok(buffer) => buffer,
            Err(err) => {
                log::error!("Failed to read file as ArrayBuffer: {:?}", err);
                return;
            }
        };

        let data = Uint8Array::new(&array_buffer);
        let bytes = data.to_vec();

        // Send the data through the channel
        if let Err(err) = sender.send(FrontendThreadEvent::FileOpenDialogComplete {
            context: inner_context,
            path: Some(inner_path),
            contents: bytes,
        }) {
            log::error!("Failed to send file data to channel: {:?}", err);
        }
    });

    Ok(())
}

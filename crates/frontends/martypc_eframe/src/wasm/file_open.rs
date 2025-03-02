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

use std::sync::Arc;

use marty_frontend_common::thread_events::{FileOpenContext, FileSelectionContext, FrontendThreadEvent};

use fluxfox::DiskImage;

use anyhow::{anyhow, Error};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{js_sys::Uint8Array, window, Event, FileReader, HtmlInputElement};

pub fn open_file(
    context: FileOpenContext,
    sender: crossbeam_channel::Sender<FrontendThreadEvent<Arc<DiskImage>>>,
) -> Result<(), Error> {
    let path = match context {
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

/// For WebAssembly, this function opens the browser file dialog by:
/// 1. Creating an <input type="file"> element.
/// 2. Attaching a `change` event listener.
/// 3. Triggering `.click()`.
///
/// This doesn't seem to work on Safari. Safari requires a user gesture to open the file dialog.
pub fn open_file_dialog(
    context: FileOpenContext,
    sender: crossbeam_channel::Sender<FrontendThreadEvent<Arc<DiskImage>>>,
) {
    use wasm_bindgen::{closure::Closure, JsCast};
    use web_sys::{Event, HtmlInputElement};

    let window = web_sys::window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");
    let body = document.body().expect("document should have a body");

    // Create <input type="file">
    let file_input: HtmlInputElement = document.create_element("input").unwrap().dyn_into().unwrap();
    file_input.set_type("file");
    file_input.set_id("file_input");
    // If you want multiple file selection, use: file_input.set_multiple(true);
    // If you want to limit file types, e.g. images only:
    // file_input.set_accept("image/*");

    // Create a closure to handle the file change event
    let inner_context = context.clone();
    let change_handler = Closure::wrap(Box::new(move |event: Event| {
        let input = event.target().unwrap().dyn_into::<HtmlInputElement>().unwrap();

        if let Some(file_list) = input.files() {
            // You can iterate over the file_list here or read them using File APIs
            // For instance:
            for i in 0..file_list.length() {
                if let Some(file) = file_list.item(i) {
                    web_sys::console::log_1(&format!("Selected file: {:?}", file.name()).into());
                    // We'll create a second closure for the FileReader 'load' event
                    let sender_clone = sender.clone();
                    let mut inner_inner_context = inner_context.clone();

                    let inner_name = file.name();
                    let onload_handler = Closure::wrap(Box::new(move |e: Event| {
                        let reader = e.target().unwrap().dyn_into::<FileReader>().unwrap();
                        // The result is an ArrayBuffer if we used read_as_array_buffer
                        if let Ok(array_buf) = reader.result() {
                            // Convert JSValue -> ArrayBuffer -> TypedArray -> Vec<u8>
                            let array = web_sys::js_sys::Uint8Array::new(&array_buf);
                            let mut bytes = vec![0u8; array.length() as usize];
                            array.copy_to(&mut bytes[..]);

                            let new_context = match inner_inner_context.clone() {
                                FileOpenContext::FloppyDiskImage { drive_select, fsc } => {
                                    FileOpenContext::FloppyDiskImage {
                                        drive_select,
                                        fsc: FileSelectionContext::Path(inner_name.clone().into()),
                                    }
                                }
                                FileOpenContext::CartridgeImage { slot_select, fsc } => {
                                    FileOpenContext::CartridgeImage {
                                        slot_select,
                                        fsc: FileSelectionContext::Path(inner_name.clone().into()),
                                    }
                                }
                            };

                            // Send the file bytes back via our channel
                            let _ = sender_clone.send(FrontendThreadEvent::FileOpenDialogComplete {
                                context: new_context,
                                path: Some(inner_name.clone().into()),
                                contents: bytes,
                            });
                        }
                    }) as Box<dyn FnMut(Event)>);

                    let reader = FileReader::new().unwrap();
                    // Attach onload
                    reader.set_onload(Some(onload_handler.as_ref().unchecked_ref()));
                    // Actually read the file
                    reader
                        .read_as_array_buffer(&file)
                        .expect("failed to read file as array buffer");

                    // Important: if we do not leak (forget) the closure, it’ll drop prematurely
                    onload_handler.forget();
                }
            }
        }
    }) as Box<dyn FnMut(_)>);

    // Attach the event listener
    file_input
        .add_event_listener_with_callback("change", change_handler.as_ref().unchecked_ref())
        .unwrap();
    change_handler.forget(); // Important: we must leak the closure to keep it alive

    // We must attach the file input to the DOM for click() to work reliably in all browsers
    body.append_child(&file_input).unwrap();

    // Programmatically click the hidden input
    file_input.click();
}

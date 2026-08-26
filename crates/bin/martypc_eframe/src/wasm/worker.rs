/*
    FluxFox
    https://github.com/dbalsom/fluxfox

    Copyright 2024-2025 Daniel Balsom

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

//! Spawn a Rust closure in a web worker.
//! This is really a ridiculous, unsafe hack, but it works.
//! Code adapted from:
//! https://www.tweag.io/blog/2022-11-24-wasm-threads-and-messages/

use eframe::{
    wasm_bindgen,
    wasm_bindgen::{closure::Closure, prelude::wasm_bindgen, JsCast, JsValue},
};

use fluxfox_egui::RenderCallback;

#[derive(Default)]
pub struct PlatformRenderCallback {}

impl RenderCallback for PlatformRenderCallback {
    fn spawn(&self, f: Box<dyn FnOnce() + Send + 'static>) {
        spawn_closure_worker(f);
    }
}

pub fn spawn(f: impl FnOnce() + Send + 'static) {
    match spawn_closure_worker(f) {
        Ok(worker) => {
            log::debug!("spawn(): worker spawned successfully");
        }
        Err(e) => {
            log::error!("spawn(): failed to spawn worker: {:?}", e);
        }
    }
}

// Spawn a worker and communicate with it.
pub fn spawn_closure_worker(f: impl FnOnce() + Send + 'static) -> Result<web_sys::Worker, JsValue> {
    let worker_opts = web_sys::WorkerOptions::new();
    worker_opts.set_type(web_sys::WorkerType::Module);
    let worker = web_sys::Worker::new_with_options("./worker.js", &worker_opts)?;

    // Double-boxing because `dyn FnOnce` is unsized and so `Box<dyn FnOnce()>` is a fat pointer.
    // But `Box<Box<dyn FnOnce()>>` is just a plain pointer, and since wasm has 32-bit pointers,
    // we can cast it to a `u32` and back.
    let ptr = Box::into_raw(Box::new(Box::new(f) as Box<dyn FnOnce()>));
    let msg = web_sys::js_sys::Array::new();

    // Send the worker a reference to our memory chunk, so it can initialize a wasm module
    // using the same memory.
    msg.push(&wasm_bindgen::memory());

    // Also send the worker the address of the closure we want to execute.
    msg.push(&JsValue::from(ptr as u32));

    // Send the data to the worker.
    log::debug!("spawn_closure_worker(): posting message to worker");
    worker.post_message(&msg)?;

    Ok(worker)
}

#[wasm_bindgen]
pub fn closure_worker_entry_point(ptr: u32) {
    // Interpret the address we were given as a pointer to a closure to call.
    let closure = unsafe { Box::from_raw(ptr as *mut Box<dyn FnOnce()>) };
    (*closure)();
}

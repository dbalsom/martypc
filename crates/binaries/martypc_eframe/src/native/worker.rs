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
use anyhow::Error;
use std::thread::JoinHandle;

use fluxfox_egui::RenderCallback;

#[derive(Default)]
pub struct PlatformRenderCallback {}

impl RenderCallback for PlatformRenderCallback {
    fn spawn(&self, f: Box<dyn FnOnce() + Send + 'static>) {
        std::thread::spawn(move || {
            f();
        });
    }
}

pub fn spawn_closure_worker<F, T>(f: F) -> Result<JoinHandle<T>, Error>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    Ok(std::thread::spawn(f))
}

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

    event_loop/thread_events.rs

    Handle events received from background threads spawned by the frontend.
*/
use crate::emulator::Emulator;
use fluxfox::DiskImage;
use std::path::PathBuf;

pub enum FrontendThreadEvent {
    FloppyImageLoadComplete(DiskImage, PathBuf),
    FloppyImageSaveComplete(PathBuf),
}

pub fn handle_thread_event(emu: &mut Emulator) {
    while let Ok(event) = emu.receiver.try_recv() {
        match event {
            FrontendThreadEvent::FloppyImageLoadComplete(disk_image, path) => {
                log::info!("Floppy image loaded: {:?}", path);
            }
            FrontendThreadEvent::FloppyImageSaveComplete(path) => {
                log::info!("Floppy image saved: {:?}", path);
            }
        }
    }
}

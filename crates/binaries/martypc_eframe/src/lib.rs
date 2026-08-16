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
#![warn(clippy::all, rust_2018_idioms)]
pub mod app;
pub mod counter;
pub mod display_power;
pub mod emulator;
pub mod emulator_builder;
pub mod event_loop;
mod file_transfer;
pub mod floppy;
pub mod input;
pub mod sound;
pub mod timestep_update;

pub mod build_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[cfg(not(target_arch = "wasm32"))]
pub mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::worker::{self, PlatformRenderCallback};
#[cfg(target_arch = "wasm32")]
pub mod wasm;
#[cfg(target_arch = "wasm32")]
pub use app::MartyApp;
#[cfg(target_arch = "wasm32")]
pub use wasm::worker::{self, PlatformRenderCallback};

// Embed default icon
pub const MARTY_ICON: &[u8] = include_bytes!("../../../../assets/martypc_icon_small.png");

pub fn build_id() -> &'static str {
    build_info::GIT_COMMIT_HASH
        .and_then(|hash| hash.get(..6))
        .unwrap_or("000000")
}

pub fn version_string() -> String {
    format!("{} [{}]", env!("CARGO_PKG_VERSION"), build_id())
}

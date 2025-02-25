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

//! On the web, we treat the URL query parameters as command line arguments.
//! This module is conditionally compiled when building for the web wasm target.

use std::path::PathBuf;

use marty_core::{cpu_common::CpuType, cpu_validator::ValidatorType};

use url::Url;
use wasm_bindgen::prelude::*;
use web_sys::window;

#[derive(Default)]
pub struct CmdLineArgs {
    pub config_file: Option<PathBuf>,
    // Ignored on wasm
    pub base_dir: Option<PathBuf>,
    // Ignored on wasm
    pub benchmark_mode: bool,
    pub no_sound: bool,
    // Ignored on wasm
    pub headless: bool,
    // Ignored on wasm
    pub fuzzer: bool,
    // Ignored on wasm
    pub romscan: bool,
    // Ignored on wasm
    pub machinescan: bool,
    // Ignored on wasm
    pub auto_poweron: bool,
    // Ignored on wasm
    pub warpspeed: bool,
    pub title_hacks: bool,
    pub off_rails_detection: bool,
    pub reverse_mouse_buttons: bool,
    pub machine_config_name: Option<String>,
    pub machine_config_overlays: Option<String>,
    pub turbo: bool,
    // Ignored on wasm
    pub validator: Option<ValidatorType>,
    pub debug_mode: bool,
    pub debug_keyboard: bool,
    pub no_roms: bool,

    // Everything below ignored on wasm
    // --------------------------------
    pub run_bin: Option<String>,
    pub run_bin_seg: Option<u16>,
    pub run_bin_ofs: Option<u16>,
    pub vreset_bin_seg: Option<u16>,
    pub vreset_bin_ofs: Option<u16>,

    // Test stuff
    pub test_cpu_type: Option<CpuType>,
    pub test_path: Option<PathBuf>,
}

/// Parse the URL query parameters into a [CmdLineArgs] struct.
pub fn parse_query_params() -> CmdLineArgs {
    let mut args = CmdLineArgs::default();

    if let Some(window) = window() {
        if let Ok(url) = Url::parse(&window.location().href().unwrap_or_default()) {
            let query_pairs = url.query_pairs();

            for (key, value) in query_pairs {
                log::debug!("Read query parameter: {}={}", key, value);
                match key.as_ref() {
                    "configfile" => args.config_file = Some(PathBuf::from(value.into_owned())),
                    "no_sound" => args.no_sound = true,
                    "machine_config_name" => args.machine_config_name = Some(String::from(value.into_owned())),
                    "machine_config_overlays" => args.machine_config_name = Some(String::from(value.into_owned())),
                    "no_roms" => args.no_roms = true,
                    "turbo" => args.turbo = true,
                    _ => {} // Ignore unknown parameters
                }
            }
        }
    }

    args
}

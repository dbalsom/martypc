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

use std::path::PathBuf;

use marty_core::{cpu_common::CpuType, cpu_validator::ValidatorType};

#[derive(Default)]
pub struct CmdLineArgs {
    pub configfile: Option<PathBuf>,
    pub basedir: Option<PathBuf>,
    pub benchmark_mode: bool,
    pub noaudio: bool,

    // Emulator options
    pub headless: bool,
    pub fuzzer:   bool,

    // Emulator options
    pub romscan: bool,
    pub machinescan: bool,
    pub auto_poweron: bool,
    pub warpspeed: bool,
    pub title_hacks: bool,
    pub off_rails_detection: bool,

    pub reverse_mouse_buttons: bool,
    pub machine_config_name: Option<String>,
    pub machine_config_overlays: Option<String>,
    pub turbo: bool,
    pub validator: Option<ValidatorType>,
    pub debug_mode: bool,
    pub debug_keyboard: bool,
    pub no_roms: bool,

    //#[bpaf(long)]
    //pub video_type: Option<VideoType>,

    //#[bpaf(long, switch)]
    //pub video_frame_debug: bool,
    pub run_bin: Option<String>,
    pub run_bin_seg: Option<u16>,
    pub run_bin_ofs: Option<u16>,
    pub vreset_bin_seg: Option<u16>,
    pub vreset_bin_ofs: Option<u16>,

    // Test stuff
    pub test_cpu_type: Option<CpuType>,
    pub test_path: Option<PathBuf>,
}

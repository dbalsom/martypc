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

use std::path::PathBuf;

use crate::mount::MountSpec;
use marty_common::types::joystick::ControllerLayout;
use marty_core::{cpu_common::CpuType, cpu_validator::ValidatorType};

use bpaf::{Bpaf, Parser};

fn mount_arg() -> impl Parser<Vec<MountSpec>> {
    bpaf::short('m')
        .long("mount")
        .help("Mount media with syntax: fd:0:disk.img")
        .argument::<String>("mountspec")
        .parse(|s| s.parse::<MountSpec>())
        .many()
}

#[cfg_attr(feature = "use_bpaf", derive(Bpaf))]
#[cfg_attr(feature = "use_bpaf", bpaf(options, version, generate(cli_args)))]
#[derive(Debug, Default)]
pub struct CmdLineArgs {
    #[bpaf(long("config_file"), long("configfile"))]
    pub config_file: Option<PathBuf>,

    #[bpaf(long("base_dir"), long("basedir"))]
    pub base_dir: Option<PathBuf>,

    #[bpaf(long, switch)]
    pub benchmark_mode: bool,

    #[bpaf(long, switch)]
    pub prefer_oem: bool,

    #[bpaf(long("no_sound"), long("nosound"), long("noaudio"), switch)]
    pub no_sound: bool,

    #[bpaf(long("demo_mode"), long("demomode"), switch)]
    pub demo_mode: bool,

    #[bpaf(long("full_screen"), long("fullscreen"), switch)]
    pub fullscreen: bool,

    // Emulator options
    #[bpaf(long, switch)]
    pub headless: bool,

    #[bpaf(long, switch)]
    pub fuzzer: bool,

    // Emulator options
    #[bpaf(long("rom_scan"), long("romscan"), switch)]
    pub romscan: bool,

    #[bpaf(long("machine_scan"), long("machinescan"), switch)]
    pub machinescan: bool,

    #[bpaf(long, switch)]
    pub auto_poweron: bool,

    #[bpaf(long, switch)]
    pub title_hacks: bool,

    #[bpaf(long, switch)]
    pub off_rails_detection: bool,

    //#[bpaf(long, switch)]
    //pub scaler_aspect_correction: bool,
    #[bpaf(long, switch)]
    pub reverse_mouse_buttons: bool,

    #[bpaf(long)]
    pub controller_layout: Option<ControllerLayout>,

    #[bpaf(long)]
    pub machine_config_name: Option<String>,
    #[bpaf(long)]
    pub machine_config_overlays: Option<String>,

    #[bpaf(long)]
    pub turbo: bool,

    #[bpaf(long)]
    pub validator: Option<ValidatorType>,

    #[bpaf(long, switch)]
    pub debug_mode: bool,

    #[bpaf(long, switch)]
    pub debug_keyboard: bool,

    #[bpaf(long, switch)]
    pub no_roms: bool,

    //#[bpaf(long)]
    //pub video_type: Option<VideoType>,

    //#[bpaf(long, switch)]
    //pub video_frame_debug: bool,
    #[bpaf(long)]
    pub run_bin: Option<String>,
    #[bpaf(long)]
    pub run_bin_seg: Option<u16>,
    #[bpaf(long)]
    pub run_bin_ofs: Option<u16>,
    #[bpaf(long)]
    pub vreset_bin_seg: Option<u16>,
    #[bpaf(long)]
    pub vreset_bin_ofs: Option<u16>,

    // Test stuff
    #[bpaf(long)]
    pub test_cpu_type: Option<CpuType>,
    #[bpaf(long)]
    pub test_path: Option<PathBuf>,

    #[bpaf(external(mount_arg))]
    pub mounts: Vec<MountSpec>,
}

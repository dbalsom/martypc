/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

    bpaf_toml_cnofig::lib.rs

    Routines to parse configuration file and command line arguments.

    This library implements CoreConfig for BPAF & TOML parsing.

*/

use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use marty_core::{
    coreconfig::VideoCardDefinition,
    cpu_common::TraceMode,
    cpu_validator::ValidatorType,
    devices::implementations::keyboard::KeyboardType,
    machine_types::{HardDiskControllerType, MachineType},
};

use frontend_common::{display_scaler::ScalerPreset, resource_manager::PathConfigItem};
use marty_common::VideoDimensions;

use bpaf::Bpaf;
use marty_core::cpu_common::HaltMode;
use serde_derive::Deserialize;

const fn _default_true() -> bool {
    true
}
const fn _default_false() -> bool {
    true
}

mod coreconfig;

#[derive(Copy, Clone, Debug, Bpaf, Deserialize, PartialEq)]
pub enum TestMode {
    None,
    Generate,
    Run,
    Validate,
    Process,
}

impl Default for TestMode {
    fn default() -> Self {
        TestMode::None
    }
}

impl FromStr for TestMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        match s.to_lowercase().as_str() {
            "none" => Ok(TestMode::None),
            "generate" => Ok(TestMode::Generate),
            "validate" => Ok(TestMode::Validate),
            "process" => Ok(TestMode::Process),
            _ => Err("Bad value for testmode".to_string()),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Media {
    pub raw_sector_image_extensions: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct Audio {
    #[serde(default = "_default_true")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct Emulator {
    pub basedir: PathBuf,
    pub paths: Vec<PathConfigItem>,
    pub ignore_dirs: Option<Vec<String>>,
    #[serde(default = "_default_true")]
    pub auto_poweron: bool,
    #[serde(default = "_default_true")]
    pub cpu_autostart: bool,
    #[serde(default)]
    pub headless: bool,
    #[serde(default)]
    pub romscan: bool,
    #[serde(default)]
    pub machinescan: bool,
    #[serde(default)]
    pub fuzzer: bool,
    #[serde(default)]
    pub warpspeed: bool,
    #[serde(default)]
    pub debug_mode: bool,
    #[serde(default = "_default_true")]
    pub debug_warn: bool,
    #[serde(default)]
    pub debug_keyboard: bool,
    pub media: Media,
    pub audio: Audio,
    pub run_bin: Option<String>,
    pub run_bin_seg: Option<u16>,
    pub run_bin_ofs: Option<u16>,

    #[serde(default)]
    pub video_trace_file: Option<PathBuf>,
    //pub video_frame_debug: bool,
    #[serde(default)]
    pub pit_output_file: Option<PathBuf>,
    #[serde(default)]
    pub pit_output_int_trigger: bool,

    pub window: Vec<WindowDefinition>,
    pub scaler_preset: Vec<ScalerPreset>,
}

#[derive(Debug, Deserialize)]
pub struct Gui {
    #[serde(default)]
    pub disabled: bool,
    pub zoom: Option<f32>,
    #[serde(default)]
    pub theme_dark: bool,
    pub theme_color: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct Validator {
    #[serde(rename = "type")]
    pub vtype: Option<ValidatorType>,
    pub trigger_address: Option<u32>,
    pub trace_file: Option<PathBuf>,
    pub baud_rate: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct Tests {
    pub test_mode: Option<TestMode>,
    pub test_seed: Option<u64>,
    pub test_dir: Option<String>,
    pub test_opcode_range: Option<Vec<u8>>,
    pub test_extension_range: Option<Vec<u8>>,
    pub test_opcode_exclude_list: Option<Vec<u8>>,
    pub test_opcode_gen_count: Option<u32>,
    pub test_opcode_gen_append: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct Cpu {
    pub wait_states: Option<bool>,
    pub off_rails_detection: Option<bool>,
    pub on_halt: Option<HaltMode>,
    pub instruction_history: Option<bool>,
    pub service_interrupt: Option<bool>,
    #[serde(default)]
    pub trace_on: bool,
    pub trace_mode: Option<TraceMode>,
    pub trace_file: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub struct Machine {
    pub config_name: String,
    //pub model: MachineType,
    #[serde(default)]
    pub no_bios: bool,
    #[serde(default)]
    pub raw_rom: bool,
    #[serde(default)]
    pub turbo: bool,
    pub cpu: Cpu,
    pub videocard: Option<Vec<VideoCardDefinition>>,
    pub pit_phase: Option<u32>,
    pub keyboard_type: Option<KeyboardType>,
    pub keyboard_layout: Option<String>,
    pub hdc: Option<HardDiskControllerType>,
    pub drive0: Option<String>,
    pub drive1: Option<String>,
    pub floppy0: Option<String>,
    pub floppy1: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Input {
    pub reverse_mouse_buttons: bool,
}
#[derive(Debug, Deserialize)]
pub struct WindowDefinition {
    #[serde(default)]
    pub enabled: bool,
    pub name: String,
    pub size: Option<VideoDimensions>,
    #[serde(default)]
    pub resizable: bool,
    pub card_id: Option<usize>,
    pub card_scale: Option<f32>,
    #[serde(default)]
    pub always_on_top: bool,
    pub scaler_preset: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigFileParams {
    pub emulator: Emulator,
    pub gui: Gui,
    pub input: Input,
    pub machine: Machine,
    pub validator: Validator,
    pub tests: Tests,
}

#[derive(Debug, Bpaf)]
#[bpaf(options, version, generate(cli_args))]
pub struct CmdLineArgs {
    #[bpaf(long)]
    pub configfile: Option<PathBuf>,

    #[bpaf(long)]
    pub basedir: Option<PathBuf>,

    #[bpaf(long, switch)]
    pub noaudio: bool,

    // Emulator options
    #[bpaf(long, switch)]
    pub headless: bool,

    #[bpaf(long, switch)]
    pub fuzzer: bool,

    // Emulator options
    #[bpaf(long, switch)]
    pub romscan: bool,

    #[bpaf(long, switch)]
    pub machinescan: bool,

    #[bpaf(long, switch)]
    pub auto_poweron: bool,

    #[bpaf(long, switch)]
    pub warpspeed: bool,

    #[bpaf(long, switch)]
    pub off_rails_detection: bool,

    //#[bpaf(long, switch)]
    //pub scaler_aspect_correction: bool,
    #[bpaf(long, switch)]
    pub reverse_mouse_buttons: bool,

    #[bpaf(long)]
    pub machine_config_name: Option<String>,

    #[bpaf(long)]
    pub turbo: bool,

    #[bpaf(long)]
    pub validator: Option<ValidatorType>,

    #[bpaf(long, switch)]
    pub debug_mode: bool,

    #[bpaf(long, switch)]
    pub debug_keyboard: bool,

    #[bpaf(long, switch)]
    pub no_bios: bool,

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
}

impl ConfigFileParams {
    pub fn overlay(&mut self, shell_args: CmdLineArgs) {
        if let Some(config_name) = shell_args.machine_config_name {
            self.machine.config_name = config_name;
        }
        if let Some(validator) = shell_args.validator {
            self.validator.vtype = Some(validator);
        }

        if let Some(basedir) = shell_args.basedir {
            self.emulator.basedir = basedir;
        }

        self.emulator.headless |= shell_args.headless;
        self.emulator.fuzzer |= shell_args.fuzzer;
        self.emulator.auto_poweron |= shell_args.auto_poweron;
        self.emulator.warpspeed |= shell_args.warpspeed;
        self.emulator.audio.enabled &= !shell_args.noaudio;

        //self.emulator.scaler_aspect_correction |= shell_args.scaler_aspect_correction;
        self.emulator.debug_mode |= shell_args.debug_mode;
        //self.emulator.video_frame_debug |= shell_args.video_frame_debug;
        self.emulator.debug_keyboard |= shell_args.debug_keyboard;
        self.machine.no_bios |= shell_args.no_bios;

        /*
        if let Some(video) = shell_args.video_type {
            self.machine.primary_video = Some(video);
        }
         */

        if let Some(run_bin) = shell_args.run_bin {
            self.emulator.run_bin = Some(run_bin);
        }

        if let Some(run_bin_seg) = shell_args.run_bin_seg {
            self.emulator.run_bin_seg = Some(run_bin_seg);
        }

        if let Some(run_bin_ofs) = shell_args.run_bin_ofs {
            self.emulator.run_bin_ofs = Some(run_bin_ofs);
        }

        self.machine.turbo |= shell_args.turbo;

        if let Some(ref mut off_rails_detection) = self.machine.cpu.off_rails_detection {
            *off_rails_detection |= shell_args.off_rails_detection;
        }

        self.input.reverse_mouse_buttons |= shell_args.reverse_mouse_buttons;

        self.emulator.romscan = shell_args.romscan;
        self.emulator.machinescan = shell_args.romscan;
    }
}

pub fn get_config<P>(default_path: P) -> Result<ConfigFileParams, anyhow::Error>
where
    P: AsRef<Path>,
{
    let shell_args: CmdLineArgs = cli_args().run();
    let mut toml_args: ConfigFileParams;

    // Allow configuration file path to be overridden by command line argument 'configfile'

    if let Some(configfile_path) = shell_args.configfile.as_ref() {
        let toml_slice = std::fs::read(configfile_path)?;
        toml_args = toml::from_slice(&toml_slice)?;
    }
    else {
        let toml_slice = std::fs::read(default_path)?;
        toml_args = toml::from_slice(&toml_slice)?;
    }

    //log::debug!("toml_config: {:?}", toml_args);

    // Command line arguments override config file arguments
    toml_args.overlay(shell_args);

    Ok(toml_args)
}

pub fn get_config_from_str(toml_text: &str) -> Result<ConfigFileParams, anyhow::Error> {
    let toml_args: ConfigFileParams;

    toml_args = toml::from_str(toml_text)?;

    //log::debug!("toml_config: {:?}", toml_args);

    Ok(toml_args)
}

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

    config_toml_bpaf::lib.rs

    Routines to parse configuration file and command line arguments.

    This library implements CoreConfig for bpaf & TOML parsing.

*/

use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use frontend_common::{
    display_scaler::ScalerPreset,
    resource_manager::PathConfigItem,
    types::window::WindowDefinition,
    BenchmarkEndCondition,
    HotkeyConfigEntry,
    JoyKeyEntry,
    MartyGuiTheme,
};

use marty_core::{
    cpu_common::{CpuSubType, CpuType, TraceMode},
    cpu_validator::ValidatorType,
    machine_types::OnHaltBehavior,
};

use bpaf::Bpaf;
use serde_derive::Deserialize;

const fn _default_true() -> bool {
    true
}
const fn _default_false() -> bool {
    false
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
pub struct VhdConfigEntry {
    pub drive:    usize,
    pub filename: String,
}

#[derive(Debug, Deserialize)]
pub struct Media {
    pub raw_sector_image_extensions: Option<Vec<String>>,
    #[serde(default)]
    pub write_protect_default: bool,
    pub vhd: Option<Vec<VhdConfigEntry>>,
}

#[derive(Debug, Deserialize)]
pub struct Audio {
    #[serde(default = "_default_true")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct Debugger {
    pub checkpoint_notify_level: Option<u32>,
    #[serde(default)]
    pub breakpoint_notify: bool,
}

#[derive(Debug, Deserialize)]
pub struct Backend {
    #[serde(default)]
    pub vsync: bool,
    #[serde(default)]
    pub macos_stripe_fix: bool,
}

#[derive(Debug, Deserialize)]
pub struct Emulator {
    pub basedir: PathBuf,
    pub paths: Vec<PathConfigItem>,
    pub ignore_dirs: Option<Vec<String>>,
    pub benchmark_mode: bool,
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
    pub title_hacks: bool,
    #[serde(default)]
    pub debug_mode: bool,
    #[serde(default = "_default_true")]
    pub debug_warn: bool,
    pub media: Media,
    pub debugger: Debugger,
    pub audio: Audio,
    pub run_bin: Option<String>,
    pub run_bin_seg: Option<u16>,
    pub run_bin_ofs: Option<u16>,
    pub vreset_bin_seg: Option<u16>,
    pub vreset_bin_ofs: Option<u16>,

    pub backend: Backend,

    #[serde(default)]
    pub video_trace_file: Option<PathBuf>,
    //pub video_frame_debug: bool,
    #[serde(default)]
    pub pit_output_file: Option<PathBuf>,
    #[serde(default)]
    pub pit_output_int_trigger: bool,

    pub window: Vec<WindowDefinition>,
    pub scaler_preset: Vec<ScalerPreset>,
    pub input: EmulatorInput,
    pub benchmark: Benchmark,
}

#[derive(Debug, Deserialize)]
pub struct Gui {
    #[serde(default)]
    pub disabled: bool,
    pub theme: Option<MartyGuiTheme>,
    pub menu_theme: Option<MartyGuiTheme>,
    pub zoom: Option<f32>,
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
pub struct Benchmark {
    pub config_name: String,
    pub config_overlays: Option<Vec<String>>,
    #[serde(default)]
    pub prefer_oem: bool,
    pub end_condition: BenchmarkEndCondition,
    pub timeout: Option<u32>,
    pub cycles: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct Tests {
    pub test_cpu_type: Option<CpuType>,
    pub test_cpu_subtype: Option<CpuSubType>,
    pub test_mode: Option<TestMode>,
    pub test_seed: Option<u64>,
    pub test_start: Option<u32>,
    pub test_path: Option<PathBuf>,
    pub test_output_path: Option<PathBuf>,
    pub test_opcode_prefix: Option<u8>,
    pub test_opcode_range: Option<Vec<u8>>,
    pub test_extension_range: Option<Vec<u8>>,
    pub test_opcode_exclude_list: Option<Vec<u8>>,
    pub test_gen_opcode_count: Option<u32>,
    pub test_gen_append: Option<bool>,
    pub test_gen_stop_on_error: Option<bool>,
    pub test_gen_version: Option<u32>,
    pub test_gen_ignore_underflow: Option<bool>,
    pub test_gen_validate_cycles: Option<bool>,
    pub test_gen_validate_memops: Option<bool>,
    pub test_gen_validate_registers: Option<bool>,
    pub test_gen_validate_flags: Option<bool>,
    pub test_run_summary_file: Option<PathBuf>,
    pub test_run_version: Option<u32>,
    pub test_run_limit: Option<usize>,
    pub test_run_validate_cycles: Option<bool>,
    pub test_run_validate_memops: Option<bool>,
    pub test_run_validate_registers: Option<bool>,
    pub test_run_validate_flags: Option<bool>,
    pub test_run_validate_undefined_flags: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct Cpu {
    pub wait_states: Option<bool>,
    pub off_rails_detection: Option<bool>,
    pub on_halt: Option<OnHaltBehavior>,
    pub instruction_history: Option<bool>,
    pub service_interrupt: Option<bool>,
    #[serde(default)]
    pub trace_on: bool,
    pub trace_mode: Option<TraceMode>,
    pub trace_file: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub struct MachineInput {
    pub keyboard_layout: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Machine {
    pub config_name: String,
    pub config_overlays: Option<Vec<String>>,
    #[serde(default = "_default_true")]
    pub prefer_oem: bool,
    //pub model: MachineType,
    #[serde(default)]
    pub reload_roms: bool,
    #[serde(default)]
    pub patch_roms: bool,
    #[serde(default)]
    pub no_roms: bool,
    #[serde(default)]
    pub raw_rom: bool,
    #[serde(default)]
    pub turbo: bool,
    pub cpu: Cpu,
    pub pit_phase: Option<u32>,
    pub input: MachineInput,
    pub disassembly_recording: Option<bool>,
    pub disassembly_file: Option<PathBuf>,
    pub terminal_port: Option<u16>,
}

#[derive(Debug, Deserialize)]
pub struct EmulatorInput {
    #[serde(default)]
    pub reverse_mouse_buttons: bool,
    pub hotkeys: Vec<HotkeyConfigEntry>,
    pub joystick_keys: Vec<JoyKeyEntry>,
    #[serde(default)]
    pub keyboard_joystick: bool,
    #[serde(default)]
    pub debug_keyboard: bool,
}

#[derive(Debug, Deserialize)]
pub struct ConfigFileParams {
    pub emulator: Emulator,
    pub gui: Gui,
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
    pub benchmark_mode: bool,

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
    pub title_hacks: bool,

    #[bpaf(long, switch)]
    pub off_rails_detection: bool,

    //#[bpaf(long, switch)]
    //pub scaler_aspect_correction: bool,
    #[bpaf(long, switch)]
    pub reverse_mouse_buttons: bool,

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
}

impl ConfigFileParams {
    pub fn overlay(&mut self, shell_args: CmdLineArgs) {
        if let Some(config_name) = shell_args.machine_config_name {
            self.machine.config_name = config_name;
        }
        if let Some(config_overlay_string) = shell_args.machine_config_overlays {
            // Split comma-separated list of overlays into vector of strings
            let config_overlays: Vec<String> = config_overlay_string.split(',').map(|s| s.trim().to_string()).collect();
            self.machine.config_overlays = Some(config_overlays);
        }

        if let Some(validator) = shell_args.validator {
            self.validator.vtype = Some(validator);
        }

        if let Some(basedir) = shell_args.basedir {
            self.emulator.basedir = basedir;
        }

        self.emulator.benchmark_mode |= shell_args.benchmark_mode;
        self.emulator.headless |= shell_args.headless;
        self.emulator.fuzzer |= shell_args.fuzzer;
        self.emulator.auto_poweron |= shell_args.auto_poweron;
        self.emulator.warpspeed |= shell_args.warpspeed;
        self.emulator.title_hacks |= shell_args.title_hacks;
        self.emulator.audio.enabled &= !shell_args.noaudio;

        //self.emulator.scaler_aspect_correction |= shell_args.scaler_aspect_correction;
        self.emulator.debug_mode |= shell_args.debug_mode;
        //self.emulator.video_frame_debug |= shell_args.video_frame_debug;
        self.emulator.input.debug_keyboard |= shell_args.debug_keyboard;
        self.machine.no_roms |= shell_args.no_roms;

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

        if let Some(vreset_bin_seg) = shell_args.vreset_bin_seg {
            self.emulator.vreset_bin_seg = Some(vreset_bin_seg);
        }

        if let Some(vreset_bin_ofs) = shell_args.vreset_bin_ofs {
            self.emulator.vreset_bin_ofs = Some(vreset_bin_ofs);
        }

        // Test stuff
        if let Some(test_cpu_type) = shell_args.test_cpu_type {
            self.tests.test_cpu_type = Some(test_cpu_type);
        }
        if let Some(test_path) = shell_args.test_path {
            self.tests.test_path = Some(test_path);
        }

        self.machine.turbo |= shell_args.turbo;

        if let Some(ref mut off_rails_detection) = self.machine.cpu.off_rails_detection {
            *off_rails_detection |= shell_args.off_rails_detection;
        }

        self.emulator.input.reverse_mouse_buttons |= shell_args.reverse_mouse_buttons;

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
        let toml_string = std::fs::read_to_string(configfile_path)?;
        toml_args = toml::from_str(&toml_string)?;
    }
    else {
        let toml_string = std::fs::read_to_string(default_path)?;
        toml_args = toml::from_str(&toml_string)?;
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

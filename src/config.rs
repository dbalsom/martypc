use std::path::{Path, PathBuf};
use std::str::FromStr;

use bpaf::{Bpaf};
use serde_derive::{Deserialize};

const fn _default_true() -> bool { true }
const fn _default_false() -> bool { true }

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Bpaf, Deserialize, Hash, Eq, PartialEq)] 
pub enum MachineType {
    FUZZER_8088,
    IBM_PC_5150,
    IBM_XT_5160
}

impl FromStr for MachineType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        match s {
            "IBM_PC_5150" => Ok(MachineType::IBM_PC_5150),
            "IBM_XT_5160" => Ok(MachineType::IBM_XT_5160),
            _ => Err("Bad value for model".to_string()),
        }
    }
}

#[allow (dead_code)]
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Bpaf, Deserialize, PartialEq)] 
pub enum VideoType {
    MDA,
    CGA,
    EGA,
    VGA
}

impl FromStr for VideoType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        match s {
            "MDA" => Ok(VideoType::MDA),
            "CGA" => Ok(VideoType::CGA),
            "EGA" => Ok(VideoType::EGA),
            "VGA" => Ok(VideoType::VGA),
            _ => Err("Bad value for videotype".to_string()),
        }
    }
}

#[derive(Copy, Clone, Debug, Bpaf, Deserialize, PartialEq)] 
pub enum HardDiskControllerType {
    None,
    Xebec
}

impl FromStr for HardDiskControllerType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        match s.to_lowercase().as_str() {
            "xebec" => Ok(HardDiskControllerType::Xebec),
            _ => Err("Bad value for videotype".to_string()),
        }
    }
}

#[derive(Copy, Clone, Debug, Bpaf, Deserialize, PartialEq)] 
pub enum ValidatorType {
    None,
    Pi8088,
    Arduino8088
}

impl FromStr for ValidatorType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        match s.to_lowercase().as_str() {
            "pi8088" => Ok(ValidatorType::Pi8088),
            "arduino8088" => Ok(ValidatorType::Arduino8088),
            _ => Err("Bad value for validatortype".to_string()),
        }
    }
}

#[derive(Copy, Clone, Debug, Bpaf, Deserialize, PartialEq)] 
pub enum TraceMode {
    None,
    Cycle,
    Instruction
}

impl Default for TraceMode {
    fn default() -> Self { 
        TraceMode::None
    }
}

impl FromStr for TraceMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        match s.to_lowercase().as_str() {
            "none" => Ok(TraceMode::None),
            "cycle" => Ok(TraceMode::Cycle),
            "instruction" => Ok(TraceMode::Instruction),
            _ => Err("Bad value for tracemode".to_string()),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Emulator {

    pub basedir: PathBuf,

    #[serde(default = "_default_true")]
    pub autostart: bool,

    #[serde(default = "_default_false")]
    pub headless: bool,

    #[serde(default = "_default_false")]
    pub fuzzer: bool,    

    #[serde(default = "_default_false")]
    pub warpspeed: bool,    

    #[serde(default = "_default_false")]
    pub correct_aspect: bool,    

    #[serde(default)]
    pub debug_mode: bool,

    #[serde(default)]
    pub no_bios: bool,

    pub run_bin: Option<String>,
    pub run_bin_seg: Option<u16>,
    pub run_bin_ofs: Option<u16>,

    #[serde(default)]
    pub trace_on: bool,
    pub trace_mode: TraceMode,
    pub trace_file: Option<String>,

    #[serde(default)]
    pub video_trace_file: Option<String>,

    pub video_frame_debug: bool,

    #[serde(default)]
    pub pit_output_file: Option<String>,
    #[serde(default = "_default_false")]
    pub pit_output_int_trigger: bool

}

#[derive(Debug, Deserialize)]
pub struct Gui {
    #[serde(default)]
    pub gui_disabled: bool,
    pub theme_color: Option<u32>
}

#[derive(Debug, Deserialize)]
pub struct Validator {
    #[serde(rename = "type")]
    pub vtype: Option<ValidatorType>,
    pub trigger_address: Option<u32>,
    pub trace_file: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Machine {
    pub model: MachineType,
    pub turbo: bool,
    pub video: VideoType,
    pub hdc: HardDiskControllerType,
    pub drive0: Option<String>,
    pub drive1: Option<String>,
}


#[derive(Debug, Deserialize)]
pub struct Cpu {
    pub wait_states_enabled: bool,
    pub off_rails_detection: bool,
    pub instruction_history: bool,
}

#[derive(Debug, Deserialize)]
pub struct Input {
    pub reverse_mouse_buttons: bool,
}

#[derive(Debug, Deserialize)]
pub struct ConfigFileParams {
    pub emulator: Emulator,
    pub gui: Gui,
    pub input: Input,
    pub machine: Machine,
    pub cpu: Cpu,
    pub validator: Validator
}

#[derive(Debug, Bpaf)]
#[bpaf(options, version, generate(cli_args))]
pub struct CmdLineArgs {

    #[bpaf(long)]
    pub configfile: Option<PathBuf>,

    #[bpaf(long)]
    pub basedir: Option<PathBuf>,

    // Emulator options
    #[bpaf(long, switch)]
    pub headless: bool,

    #[bpaf(long, switch)]
    pub fuzzer: bool,

    #[bpaf(long, switch)]
    pub autostart: bool,

    #[bpaf(long, switch)]
    pub warpspeed: bool,

    #[bpaf(long, switch)]
    pub off_rails_detection: bool,

    #[bpaf(long, switch)]
    pub correct_aspect: bool,      

    #[bpaf(long, switch)]
    pub reverse_mouse_buttons: bool,    

    #[bpaf(long)]
    pub machine_model: Option<MachineType>,

    #[bpaf(long)]
    pub turbo: bool,

    #[bpaf(long)]
    pub validator: Option<ValidatorType>,

    #[bpaf(long, switch)]
    pub debug_mode: bool,

    #[bpaf(long, switch)]
    pub no_bios: bool,

    #[bpaf(long, switch)]
    pub video_frame_debug: bool,

    #[bpaf(long)]
    pub run_bin: Option<String>,
    #[bpaf(long)]
    pub run_bin_seg: Option<u16>,
    #[bpaf(long)]
    pub run_bin_ofs: Option<u16>,    
}

impl ConfigFileParams {
    pub fn overlay(&mut self, shell_args: CmdLineArgs) {

        if let Some(machine_model) = shell_args.machine_model { 
            self.machine.model = machine_model;
        }
        if let Some(validator) = shell_args.validator { 
            self.validator.vtype = Some(validator);
        }       

        if let Some(basedir) = shell_args.basedir {
            self.emulator.basedir = basedir;
        }
        self.emulator.headless |= shell_args.headless;
        self.emulator.fuzzer |= shell_args.fuzzer;
        self.emulator.autostart |= shell_args.autostart;
        self.emulator.warpspeed |= shell_args.warpspeed;
        self.emulator.correct_aspect |= shell_args.correct_aspect;
        self.emulator.debug_mode |= shell_args.debug_mode;
        self.emulator.no_bios |= shell_args.no_bios;
        self.emulator.video_frame_debug |= shell_args.video_frame_debug;

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

        self.cpu.off_rails_detection |= shell_args.off_rails_detection;

        self.input.reverse_mouse_buttons |= shell_args.reverse_mouse_buttons;
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
    
    log::debug!("toml_config: {:?}", toml_args);

    // Command line arguments override config file arguments
    toml_args.overlay(shell_args);

    Ok(toml_args)
}

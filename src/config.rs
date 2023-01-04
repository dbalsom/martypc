use std::path::{Path, PathBuf};
use std::str::FromStr;

use bpaf::{Bpaf, Parser};
use serde_derive::{Deserialize};

const fn _default_true() -> bool { true }
const fn _default_false() -> bool { true }

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Bpaf, Deserialize, PartialEq)] 
pub enum MachineType {
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

#[derive(Debug, Deserialize)]
pub struct Emulator {
    #[serde(default = "_default_true")]
    pub autostart: bool,

    #[serde(default = "_default_false")]
    pub headless: bool
}

#[derive(Debug, Deserialize)]
pub struct Validator {
    #[serde(rename = "type")]
    pub vtype: Option<ValidatorType>,
    pub trigger_address: Option<u32>
}

#[derive(Debug, Deserialize)]
pub struct Machine {
    pub model: MachineType,
    pub video: VideoType,
    pub hdc: HardDiskControllerType,
    pub drive0: Option<String>,
    pub drive1: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigFileParams {
    pub emulator: Emulator,
    pub machine: Machine,
    pub validator: Validator
}

#[derive(Debug, Bpaf)]
#[bpaf(options, version, generate(cli_args))]
pub struct CmdLineArgs {

    #[bpaf(long)]
    pub configfile: Option<PathBuf>,

    // Emulator options
    #[bpaf(long)]
    pub headless: Option<bool>,

    #[bpaf(long)]
    pub autostart: Option<bool>,

    #[bpaf(long)]
    pub machine_model: Option<MachineType>,


    #[bpaf(long)]
    pub validator: Option<ValidatorType>,
}

impl ConfigFileParams {
    pub fn overlay(&mut self, shell_args: CmdLineArgs) {
        if shell_args.machine_model.is_some() { 
            self.machine.model = shell_args.machine_model.unwrap();
        }
        if shell_args.headless.is_some() { 
            self.emulator.headless = shell_args.headless.unwrap()
        }
        if shell_args.autostart.is_some() { 
            self.emulator.autostart = shell_args.autostart.unwrap()
        }          
    }
}

pub fn get_config<P>(default_path: P) -> Result<ConfigFileParams, anyhow::Error>
where 
    P: AsRef<Path>,
{
    let shell_args: CmdLineArgs = cli_args().run();
    let mut toml_args: ConfigFileParams;

    // Allow configuration file path to be overridden by command line argument 'configfile'
    if shell_args.configfile.is_some() {
        let config_pathbuf = shell_args.configfile.clone();

        let toml_slice = std::fs::read(config_pathbuf.unwrap())?;
        toml_args = toml::from_slice(&toml_slice )?;
    }
    else {
        let toml_slice = std::fs::read(default_path)?;
        toml_args = toml::from_slice(&toml_slice)?;
    }
    
    println!("toml: {:?}", toml_args);

    // Command line arguments override config file arguments
    toml_args.overlay(shell_args);

    Ok(toml_args)
}

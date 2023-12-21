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

    frontend_common::machine_manager::mod.rs

    Machine configuration services for frontends.
*/

use crate::{
    resource_manager::ResourceManager,
    rom_manager::{RomCheckpoint, RomDefinitionFile, RomManager, RomPatch, RomSet, RomSetDefinition},
};
use anyhow::Error;
use marty_core::{
    machine_config::{
        FloppyControllerConfig,
        HardDriveControllerConfig,
        KeyboardConfig,
        MachineConfiguration,
        MemoryConfig,
        SerialControllerConfig,
        SerialMouseConfig,
        VideoCardConfig,
    },
    machine_types::{HardDiskControllerType, MachineType},
    videocard::VideoType,
};
use serde;
use serde_derive::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

#[derive(Clone, Debug, Deserialize)]
pub struct MachineConfigFile {
    machine: Vec<MachineConfigFileEntry>,
}

pub struct MachineConfigContext<'a> {
    config: &'a MachineConfiguration,
    roms_required: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MachineConfigFileEntry {
    name: String,
    #[serde(rename = "type")]
    machine_type: MachineType,
    rom_set: String,
    memory: MemoryConfig,
    #[serde(default)]
    speaker: bool,
    fdc: Option<FloppyControllerConfig>,
    hdc: Option<HardDriveControllerConfig>,
    serial: Option<Vec<SerialControllerConfig>>,
    video: Option<Vec<VideoCardConfig>>,
    keyboard: Option<KeyboardConfig>,
    serial_mouse: Option<SerialMouseConfig>,
}

/*
#[derive(Clone, Debug, Deserialize)]
pub struct ParallelControllerConfig {
    type: ParallelControllerType,
    port: Vec<ParallelPortConfig>,
}
 */

pub struct MachineManager {
    active_config_name: String,
    active_config: Option<MachineConfigFileEntry>,
    config_names: HashSet<String>,
    configs: HashMap<String, MachineConfigFileEntry>,
    features_requested: HashSet<String>,
    features_provided: HashSet<String>,
    rom_sets_required: Vec<usize>,
}

impl Default for MachineManager {
    fn default() -> Self {
        Self {
            active_config_name: String::new(),
            active_config: None,
            config_names: HashSet::new(),
            configs: HashMap::new(),
            features_requested: HashSet::new(),
            features_provided: HashSet::new(),
            rom_sets_required: Vec::new(),
        }
    }
}

impl MachineManager {
    pub fn new() -> Self {
        let slf = Self::default();

        slf
    }

    pub fn load_configs(&mut self, rm: &ResourceManager) -> Result<(), Error> {
        let mut machine_configs: Vec<MachineConfigFileEntry> = Vec::new();

        // Get a file listing of the rom directory.
        let items = rm.enumerate_items("machine", true)?;

        // Filter out any non-toml files.
        let toml_configs: Vec<_> = items
            .iter()
            .filter_map(|item| {
                log::debug!("item: {:?}", item.full_path);
                if item.full_path.extension().is_some_and(|ext| ext == "toml") {
                    return Some(item);
                }
                None
            })
            .collect();

        log::debug!(
            "load_configs(): Found {} Machine Configuration files",
            toml_configs.len()
        );

        // Attempt to load each toml file as a rom definition file.
        for config in toml_configs {
            let mut loaded_config = self.parse_config_file(&config.full_path)?;
            machine_configs.append(&mut loaded_config.machine);
        }

        // Check for duplicate names
        for config in machine_configs {
            if self.configs.contains_key(&config.name) {
                return Err(anyhow::anyhow!("Duplicate machine name: {}", config.name));
            }
            self.configs.insert(config.name.clone(), config);
        }

        self.print_config_stats();
        Ok(())
    }

    /// Parse the given toml file as a MachineConfigFile.
    fn parse_config_file(&mut self, toml_path: &PathBuf) -> Result<MachineConfigFile, Error> {
        let toml_str = std::fs::read_to_string(toml_path)?;
        let config = toml::from_str::<MachineConfigFile>(&toml_str)?;

        log::debug!("Rom definition file loaded: {:?}", toml_path);
        Ok(config)
    }

    fn print_config_stats(&mut self) {
        println!("Have {} Machine Configurations:", self.configs.len());
        for (name, config) in self.configs.iter() {
            println!(" {}", name);

            for (i, card) in config.video.as_ref().unwrap_or(&Vec::new()).iter().enumerate() {
                println!("  videocard {}: type: {:?}", i, card.video_type,);
            }
        }
    }

    /// Return a list of strings representing the names of all machine configurations parsed.
    pub fn get_config_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for name in self.configs.keys() {
            names.push(name.clone());
        }
        names
    }

    /// Return the machine configuration with the given name, if present.
    pub fn get_config(&self, config_name: &str) -> Option<&MachineConfigFileEntry> {
        self.configs.get(config_name)
    }

    /*
    pub fn resolve_sets(&self, config_name: &str, rom_manager: &RomManager) -> Result<MachineConfigContext, Error> {
        let config = self
            .configs
            .get(config_name)
            .ok_or(anyhow::anyhow!("Machine configuration not found: {}", config_name))?;

        // The ROM Set resolution process is a bit complicated.

        // First, resolve any ROMS referenced by name to their hashes.
        Ok(Default::default())
    }

     */
}

impl MachineConfigFileEntry {
    /// Return a vector of strings representing the ROM feature requirements for this configuration
    pub fn get_rom_requirements(&self) -> Result<Vec<String>, Error> {
        let mut req_set: HashSet<String> = HashSet::new();

        if let Some(features) = marty_core::machine_config::get_base_rom_features(self.machine_type) {
            for feature in features {
                req_set.insert(feature.to_string());
            }
        }

        if let Some(hdc) = self.hdc.as_ref() {
            match hdc.hdc_type {
                HardDiskControllerType::IbmXebec => {
                    req_set.insert(String::from("expansion"));
                    req_set.insert(String::from("ibm_xebec"));
                }
            }
        }

        if let Some(cards) = self.video.as_ref() {
            for card in cards {
                match card.video_type {
                    VideoType::EGA => {
                        req_set.insert(String::from("expansion"));
                        req_set.insert(String::from("ibm_ega"));
                    }
                    VideoType::VGA => {
                        req_set.insert(String::from("expansion"));
                        req_set.insert(String::from("ibm_vga"));
                    }
                    _ => {}
                }
            }
        }

        let mut req_list = Vec::from_iter(req_set.into_iter());
        req_list.sort();
        Ok(req_list)
    }

    pub fn to_machine_config(&self) -> MachineConfiguration {
        MachineConfiguration {
            speaker: self.speaker,
            machine_type: self.machine_type,
            memory: self.memory.clone(),
            fdc: self.fdc.clone(),
            hdc: self.hdc.clone(),
            serial: self.serial.clone().unwrap_or_default(),
            video: self.video.clone().unwrap_or_default(),
            keyboard: self.keyboard.clone(),
            serial_mouse: self.serial_mouse.clone(),
        }
    }
}

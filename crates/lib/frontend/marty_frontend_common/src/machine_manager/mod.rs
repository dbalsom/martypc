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

    frontend_common::machine_manager::mod.rs

    Machine configuration services for frontends.
*/

use crate::resource_manager::ResourceManager;
use anyhow::Error;
use marty_core::{
    device_traits::videocard::VideoType,
    machine_config::{
        CpuConfig,
        EmsMemoryConfig,
        FloppyControllerConfig,
        GamePortConfig,
        HardDriveControllerConfig,
        KeyboardConfig,
        MachineConfiguration,
        MediaConfig,
        MemoryConfig,
        SerialControllerConfig,
        SerialMouseConfig,
        SoundDeviceConfig,
        VideoCardConfig,
    },
    machine_types::{HardDiskControllerType, MachineType},
};

use serde_derive::Deserialize;
use std::{
    collections::{BTreeMap, HashSet},
    ffi::OsString,
};

#[derive(Clone, Debug, Deserialize)]
pub struct MachineConfigFile {
    machine: Option<Vec<MachineConfigFileEntry>>,
    overlay: Option<Vec<MachineConfigFileOverlayEntry>>,
}

// pub struct MachineConfigContext<'a> {
//     config: &'a MachineConfiguration,
//     roms_required: Vec<String>,
// }

#[derive(Clone, Debug, Deserialize)]
pub struct MachineConfigFileEntry {
    name: String,
    #[serde(rename = "type")]
    machine_type: MachineType,
    rom_set: String,
    overlays: Option<Vec<String>>,
    cpu: Option<CpuConfig>,
    memory: MemoryConfig,
    ems: Option<EmsMemoryConfig>,
    #[serde(default)]
    speaker: bool,
    ppi_turbo: Option<bool>, // This bool is an option so that it is tri-state - missing means no turbo feature, true means ppi high = turbo, false means ppi low = turbo.
    fdc: Option<FloppyControllerConfig>,
    hdc: Option<HardDriveControllerConfig>,
    serial: Option<Vec<SerialControllerConfig>>,
    video: Option<Vec<VideoCardConfig>>,
    sound: Option<Vec<SoundDeviceConfig>>,
    keyboard: Option<KeyboardConfig>,
    serial_mouse: Option<SerialMouseConfig>,
    game_port: Option<GamePortConfig>,
    media: Option<MediaConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MachineConfigFileOverlayEntry {
    name: String,
    cpu: Option<CpuConfig>,
    memory: Option<MemoryConfig>,
    ems: Option<EmsMemoryConfig>,
    fdc: Option<FloppyControllerConfig>,
    hdc: Option<HardDriveControllerConfig>,
    serial: Option<Vec<SerialControllerConfig>>,
    video: Option<Vec<VideoCardConfig>>,
    sound: Option<Vec<SoundDeviceConfig>>,
    keyboard: Option<KeyboardConfig>,
    serial_mouse: Option<SerialMouseConfig>,
    game_port: Option<GamePortConfig>,
    // TODO: Support media in overlay?
    #[allow(unused)]
    media: Option<MediaConfig>,
}

/*
#[derive(Clone, Debug, Deserialize)]
pub struct ParallelControllerConfig {
    type: ParallelControllerType,
    port: Vec<ParallelPortConfig>,
}
 */

pub struct MachineManager {
    active_config: Option<MachineConfigFileEntry>,
    config_names: HashSet<String>,
    overlay_names: HashSet<String>,
    configs: BTreeMap<String, MachineConfigFileEntry>,
    overlays: BTreeMap<String, MachineConfigFileOverlayEntry>,
    features_requested: HashSet<String>,
    features_provided: HashSet<String>,
    rom_sets_required: Vec<usize>,
}

impl Default for MachineManager {
    fn default() -> Self {
        Self {
            active_config: None,
            config_names: HashSet::new(),
            overlay_names: HashSet::new(),
            configs: BTreeMap::new(),
            overlays: BTreeMap::new(),
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

    pub async fn load_configs(&mut self, rm: &mut ResourceManager) -> Result<(), Error> {
        let mut machine_configs: Vec<MachineConfigFileEntry> = Vec::new();
        let mut overlay_configs: Vec<MachineConfigFileOverlayEntry> = Vec::new();

        log::debug!("load_configs(): Loading machine configurations...");

        // Get a file listing of 'toml' files in the machine configuration directory.
        let toml_configs = rm.enumerate_items("machine", None, false, true, Some(vec![OsString::from("toml")]))?;

        log::debug!(
            "load_configs(): Found {} Machine Configuration files:",
            toml_configs.len()
        );
        for item in toml_configs.iter() {
            log::debug!("  {:?}", item.location);
        }

        // Attempt to parse each toml file as a machine configuration or overlay file.
        for config in toml_configs {
            println!("Reading machine configuration file: {:?}", config.location);

            let toml_str = rm.read_string_from_path(&config.location).await?;

            let mut loaded_config = match self.parse_config_file(&toml_str) {
                Ok(config) => config,
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Error parsing machine configuration file '{:?}':\n{}",
                        config.location,
                        e
                    ))
                }
            };

            if let Some(machine_vec) = loaded_config.machine.as_mut() {
                machine_configs.append(machine_vec);
            }
            if let Some(overlay_vec) = loaded_config.overlay.as_mut() {
                overlay_configs.append(overlay_vec);
            }
        }

        // Check for duplicate names
        for config in machine_configs {
            if self.configs.contains_key(&config.name) {
                return Err(anyhow::anyhow!("Duplicate machine name: {}", config.name));
            }
            self.configs.insert(config.name.clone(), config);
        }
        for overlay in overlay_configs {
            if self.overlays.contains_key(&overlay.name) {
                return Err(anyhow::anyhow!("Duplicate overlay name: {}", overlay.name));
            }
            self.overlays.insert(overlay.name.clone(), overlay);
        }

        self.print_config_stats();
        Ok(())
    }

    fn parse_config_file(&mut self, toml_str: &str) -> Result<MachineConfigFile, Error> {
        let config = toml::from_str::<MachineConfigFile>(toml_str)?;

        //log::debug!("Machine definition file loaded: {:?}", toml_path);
        Ok(config)
    }

    fn print_config_stats(&mut self) {
        println!("Found {} Machine Configurations:", self.configs.len());
        for (name, _config) in self.configs.iter() {
            println!(" {}", name);

            /*
            for (i, card) in config.video.as_ref().unwrap_or(&Vec::new()).iter().enumerate() {
                println!("  videocard {}: type: {:?}", i, card.video_type,);
            }
            */
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

    /// Return the machine configuration with the given name, after applying the specified overlays. If the machine
    /// name or one of the overlays is not found, an error is returned.
    pub fn get_config_with_overlays(
        &mut self,
        config_name: &str,
        additional_overlays: &Vec<String>,
    ) -> Result<&MachineConfigFileEntry, Error> {
        let mut config = self
            .configs
            .get(config_name)
            .ok_or(anyhow::anyhow!("Machine configuration not found: {}", config_name))?
            .clone();

        // Populate overlay list with the overlays specified in base configuration.
        let mut total_overlays = config.overlays.as_ref().unwrap_or(&Vec::new()).clone();
        total_overlays.extend(additional_overlays.clone());

        for overlay_name in total_overlays {
            let overlay = self.overlays.get(&overlay_name).ok_or(anyhow::anyhow!(
                "Machine configuration overlay not found: {}",
                overlay_name
            ))?;
            config.apply_overlay(overlay.clone());
        }

        self.active_config = Some(config);
        Ok(&self.active_config.as_ref().unwrap())
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
    pub fn get_specified_rom_set(&self) -> Option<String> {
        if self.rom_set.contains("auto") {
            return None;
        }
        Some(self.rom_set.clone())
    }

    /// Returns a a tuple of vectors of strings representing the required and optional ROM features for this
    /// configuration
    pub fn get_rom_requirements(&self) -> Result<(Vec<String>, Vec<String>), Error> {
        let mut req_set: HashSet<String> = HashSet::new();
        let mut req_vec: Vec<String> = Vec::new();
        let mut opt_vec: Vec<String> = Vec::new();

        if let Some(features) = marty_core::machine_config::get_base_rom_features(self.machine_type) {
            for feature in features {
                if req_set.insert(feature.to_string()) {
                    req_vec.push(feature.to_string());
                }
            }
        }

        if let Some(features) = marty_core::machine_config::get_optional_rom_features(self.machine_type) {
            for feature in features {
                if req_set.insert(feature.to_string()) {
                    opt_vec.push(feature.to_string());
                }
            }
        }

        if let Some(hdc) = &self.hdc {
            match hdc.hdc_type {
                HardDiskControllerType::IbmXebec => {
                    if req_set.insert(String::from("expansion")) {
                        req_vec.push(String::from("expansion"));
                    }
                    if req_set.insert(String::from("ibm_xebec")) {
                        req_vec.push(String::from("ibm_xebec"));
                    }
                }
                HardDiskControllerType::XtIde => {
                    if req_set.insert(String::from("expansion")) {
                        req_vec.push(String::from("expansion"));
                    }
                    if req_set.insert(String::from("xtide")) {
                        req_vec.push(String::from("xtide"));
                    }
                }
            }
        }

        if let Some(cards) = self.video.as_ref() {
            for card in cards {
                match card.video_type {
                    #[cfg(feature = "ega")]
                    VideoType::EGA => {
                        log::debug!("Adding EGA ROM requirements");
                        if req_set.insert(String::from("expansion")) {
                            req_vec.push(String::from("expansion"));
                        }
                        if req_set.insert(String::from("ibm_ega")) {
                            req_vec.push(String::from("ibm_ega"));
                        }
                    }
                    #[cfg(feature = "vga")]
                    VideoType::VGA => {
                        log::debug!("Adding VGA ROM requirements");
                        if req_set.insert(String::from("expansion")) {
                            req_vec.push(String::from("expansion"));
                        }
                        if req_set.insert(String::from("ibm_vga")) {
                            req_vec.push(String::from("ibm_vga"));
                        }
                    }
                    _ => {}
                }
            }
        }
        else {
            log::warn!("Config has no video cards specified. Skipping video ROM requirements.");
        }

        Ok((req_vec, opt_vec))
    }

    /// Apply a Machine Config Overlay to this configuration. Every option that is Some within the overlay is
    /// copied into this configuration.
    pub fn apply_overlay(&mut self, overlay: MachineConfigFileOverlayEntry) {
        if let Some(cpu) = overlay.cpu {
            log::debug!("Applying CPU overlay: {:?}", cpu);
            self.cpu = Some(cpu);
        }
        if let Some(memory) = overlay.memory {
            log::debug!("Applying memory overlay: {:?}", memory);
            self.memory = memory;
        }
        if let Some(ems) = overlay.ems {
            log::debug!("Applying EMS overlay: {:?}", ems);
            self.ems = Some(ems);
        }
        if let Some(fdc) = overlay.fdc {
            log::debug!("Applying FDC overlay: {:?}", fdc);
            self.fdc = Some(fdc);
        }
        if let Some(hdc) = overlay.hdc {
            log::debug!("Applying HDC overlay: {:?}", hdc);
            self.hdc = Some(hdc);
        }
        if let Some(serial) = overlay.serial {
            log::debug!("Applying serial overlay: {:?}", serial);
            self.serial = Some(serial);
        }
        if let Some(video) = overlay.video {
            log::debug!("Applying video overlay: {:?}", video);
            self.video = Some(video);
        }
        if let Some(sound) = overlay.sound {
            log::debug!("Applying sound overlay: {:?}", sound);
            self.sound = Some(sound);
        }
        if let Some(keyboard) = overlay.keyboard {
            log::debug!("Applying keyboard overlay: {:?}", keyboard);
            self.keyboard = Some(keyboard);
        }
        if let Some(serial_mouse) = overlay.serial_mouse {
            log::debug!("Applying serial mouse overlay: {:?}", serial_mouse);
            self.serial_mouse = Some(serial_mouse);
        }
        if let Some(game_port) = overlay.game_port {
            log::debug!("Applying game port overlay: {:?}", game_port);
            self.game_port = Some(game_port);
        }
    }

    pub fn to_machine_config(&self) -> MachineConfiguration {
        MachineConfiguration {
            speaker: self.speaker,
            ppi_turbo: self.ppi_turbo,
            machine_type: self.machine_type,
            cpu: self.cpu.clone(),
            memory: self.memory.clone(),
            ems: self.ems.clone(),
            fdc: self.fdc.clone(),
            hdc: self.hdc.clone(),
            serial: self.serial.clone().unwrap_or_default(),
            video: self.video.clone().unwrap_or_default(),
            sound: self.sound.clone().unwrap_or_default(),
            keyboard: self.keyboard.clone(),
            serial_mouse: self.serial_mouse.clone(),
            game_port: self.game_port.clone(),
            media: self.media.clone(),
        }
    }
}

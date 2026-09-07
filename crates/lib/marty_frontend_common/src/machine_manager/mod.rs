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

    frontend_common::machine_manager::mod.rs

    Machine configuration services for frontends.
*/

use std::{collections::BTreeMap, ffi::OsString};

use crate::resource_manager::ResourceManager;
use marty_common::MartyHashSet;
use marty_core::{
    machine_config::{
        ConventionalExpansionConfig,
        CpuConfig,
        EmsMemoryConfig,
        FloppyControllerConfig,
        FloppyDriveConfig,
        GamePortConfig,
        HardDriveControllerConfig,
        KeyboardConfig,
        MachineConfiguration,
        MediaConfig,
        MemoryConfig,
        ParallelControllerConfig,
        SerialControllerConfig,
        SerialMouseConfig,
        SoundDeviceConfig,
        VideoCardConfig,
        VirtualMouseConfig,
    },
    machine_types::{HardDiskControllerType, MachineType},
};

#[cfg(any(feature = "ega", feature = "vga"))]
use marty_core::device_traits::videocard::VideoType;

use anyhow::Error;
use serde_derive::Deserialize;

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
    #[serde(default)]
    cassette: bool,
    ppi_turbo: Option<bool>, // This bool is an option so that it is tri-state - missing means no turbo feature, true means ppi high = turbo, false means ppi low = turbo.
    fdc: Option<FloppyControllerConfig>,
    hdc: Option<HardDriveControllerConfig>,
    serial: Option<Vec<SerialControllerConfig>>,
    parallel: Option<Vec<ParallelControllerConfig>>,
    video: Option<Vec<VideoCardConfig>>,
    sound: Option<Vec<SoundDeviceConfig>>,
    keyboard: Option<KeyboardConfig>,
    serial_mouse: Option<SerialMouseConfig>,
    virtual_mouse: Option<VirtualMouseConfig>,
    game_port: Option<GamePortConfig>,
    media: Option<MediaConfig>,
    conventional_expansion: Option<Vec<ConventionalExpansionConfig>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MachineConfigFileOverlayEntry {
    name: String,
    #[serde(default)]
    operation: MachineConfigOverlayOperation,
    target: Option<MachineConfigOverlayTarget>,
    selector: Option<String>,
    #[serde(default)]
    parameters: Vec<MachineConfigOverlayParameter>,
    value: Option<toml::Value>,
    cpu: Option<CpuConfig>,
    memory: Option<MemoryConfig>,
    ems: Option<EmsMemoryConfig>,
    fdc: Option<FloppyControllerConfig>,
    hdc: Option<HardDriveControllerConfig>,
    serial: Option<Vec<SerialControllerConfig>>,
    parallel: Option<Vec<ParallelControllerConfig>>,
    video: Option<Vec<VideoCardConfig>>,
    sound: Option<Vec<SoundDeviceConfig>>,
    keyboard: Option<KeyboardConfig>,
    serial_mouse: Option<SerialMouseConfig>,
    virtual_mouse: Option<VirtualMouseConfig>,
    game_port: Option<GamePortConfig>,
    conventional_expansion: Option<Vec<ConventionalExpansionConfig>>,
    // TODO: Support media in overlay?
    #[allow(unused)]
    media: Option<MediaConfig>,
}

#[derive(Copy, Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum MachineConfigOverlayOperation {
    #[default]
    Replace,
    Merge,
}

#[derive(Copy, Clone, Debug, Deserialize, Eq, PartialEq)]
enum MachineConfigOverlayTarget {
    #[serde(rename = "fdc.drive")]
    FdcDrive,
    #[serde(rename = "video")]
    Video,
}

impl MachineConfigOverlayTarget {
    fn name(self) -> &'static str {
        match self {
            Self::FdcDrive => "fdc.drive",
            Self::Video => "video",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum MachineConfigOverlayParameter {
    Integer {
        name: String,
        min: Option<i64>,
        max: Option<i64>,
        #[serde(default = "default_true")]
        required: bool,
        default: Option<i64>,
        #[allow(unused)]
        label: Option<String>,
    },
}

impl MachineConfigOverlayParameter {
    fn name(&self) -> &str {
        match self {
            Self::Integer { name, .. } => name,
        }
    }

    fn validate_definition(&self, overlay_name: &str) -> Result<(), Error> {
        match self {
            Self::Integer {
                name,
                min,
                max,
                default,
                ..
            } => {
                if name.is_empty() {
                    return Err(anyhow::anyhow!(
                        "Overlay '{}': parameter name cannot be empty",
                        overlay_name
                    ));
                }
                if let (Some(min), Some(max)) = (*min, *max) {
                    if min > max {
                        return Err(anyhow::anyhow!(
                            "Overlay '{}': parameter '{}' has minimum {} greater than maximum {}",
                            overlay_name,
                            name,
                            min,
                            max
                        ));
                    }
                }
                if let Some(default) = default {
                    self.validate_integer(overlay_name, *default)?;
                }
            }
        }
        Ok(())
    }

    fn parse_value(&self, overlay_name: &str, raw_value: &str) -> Result<MachineConfigOverlayValue, Error> {
        match self {
            Self::Integer { .. } => {
                let value = raw_value.parse::<i64>().map_err(|_| {
                    anyhow::anyhow!(
                        "Overlay '{}': parameter '{}' requires an integer, received '{}'",
                        overlay_name,
                        self.name(),
                        raw_value
                    )
                })?;
                self.validate_integer(overlay_name, value)?;
                Ok(MachineConfigOverlayValue::Integer(value))
            }
        }
    }

    fn default_value(&self) -> Option<MachineConfigOverlayValue> {
        match self {
            Self::Integer { default, .. } => default.map(MachineConfigOverlayValue::Integer),
        }
    }

    fn is_required(&self) -> bool {
        match self {
            Self::Integer { required, .. } => *required,
        }
    }

    fn validate_integer(&self, overlay_name: &str, value: i64) -> Result<(), Error> {
        let Self::Integer { name, min, max, .. } = self;
        if min.is_some_and(|min| value < min) || max.is_some_and(|max| value > max) {
            let expected = match (min, max) {
                (Some(min), Some(max)) => format!("between {} and {}", min, max),
                (Some(min), None) => format!("at least {}", min),
                (None, Some(max)) => format!("at most {}", max),
                (None, None) => unreachable!(),
            };
            return Err(anyhow::anyhow!(
                "Overlay '{}': parameter '{}' must be {}, received {}",
                overlay_name,
                name,
                expected,
                value
            ));
        }
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum MachineConfigOverlayValue {
    Integer(i64),
}

#[derive(Debug, Eq, PartialEq)]
struct MachineConfigOverlayInvocation<'a> {
    name: &'a str,
    arguments: Option<&'a str>,
}

impl<'a> MachineConfigOverlayInvocation<'a> {
    fn parse(spec: &'a str) -> Result<Self, Error> {
        let spec = spec.trim();
        if spec.is_empty() {
            return Err(anyhow::anyhow!("Machine configuration overlay name cannot be empty"));
        }

        let (name, arguments) = match spec.split_once(':') {
            Some((name, arguments)) => {
                let name = name.trim();
                let arguments = arguments.trim();
                if name.is_empty() {
                    return Err(anyhow::anyhow!("Machine configuration overlay name cannot be empty"));
                }
                if arguments.is_empty() {
                    return Err(anyhow::anyhow!("Overlay '{}': argument list cannot be empty", name));
                }
                (name, Some(arguments))
            }
            None => (spec, None),
        };

        Ok(Self { name, arguments })
    }
}

fn default_true() -> bool {
    true
}

impl MachineConfigFileOverlayEntry {
    fn validate_definition(&self) -> Result<(), Error> {
        if self.name.is_empty() {
            return Err(anyhow::anyhow!("Overlay name cannot be empty"));
        }
        if self.name.contains(':') {
            return Err(anyhow::anyhow!(
                "Overlay name '{}' contains reserved argument delimiter ':'",
                self.name
            ));
        }

        let mut parameter_names = MartyHashSet::default();
        for parameter in &self.parameters {
            parameter.validate_definition(&self.name)?;
            if !parameter_names.insert(parameter.name()) {
                return Err(anyhow::anyhow!(
                    "Overlay '{}': duplicate parameter '{}'",
                    self.name,
                    parameter.name()
                ));
            }
        }

        match self.operation {
            MachineConfigOverlayOperation::Replace => {
                if self.target.is_some()
                    || self.selector.is_some()
                    || self.value.is_some()
                    || !self.parameters.is_empty()
                {
                    return Err(anyhow::anyhow!(
                        "Overlay '{}': target, selector, parameters, and value are only valid for merge operations",
                        self.name
                    ));
                }
            }
            MachineConfigOverlayOperation::Merge => {
                let target = self
                    .target
                    .ok_or_else(|| anyhow::anyhow!("Overlay '{}': merge operation requires a target", self.name))?;
                let target_name = target.name();
                let selector = self
                    .selector
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Overlay '{}': merge operation requires a selector", self.name))?;
                if self.parameters.len() != 1 {
                    return Err(anyhow::anyhow!(
                        "Overlay '{}': {} merge requires exactly one parameter",
                        self.name,
                        target_name
                    ));
                }
                let parameter = self
                    .parameters
                    .iter()
                    .find(|parameter| parameter.name() == selector)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Overlay '{}': selector '{}' does not name a declared parameter",
                            self.name,
                            selector
                        )
                    })?;
                if !matches!(parameter, MachineConfigOverlayParameter::Integer { .. }) {
                    return Err(anyhow::anyhow!(
                        "Overlay '{}': {} selector '{}' must be an integer parameter",
                        self.name,
                        target_name,
                        selector
                    ));
                }
                let value = self
                    .value
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Overlay '{}': merge operation requires a value", self.name))?;
                match target {
                    MachineConfigOverlayTarget::FdcDrive => {
                        value.clone().try_into::<FloppyDriveConfig>().map_err(|err| {
                            anyhow::anyhow!("Overlay '{}': invalid fdc.drive value: {}", self.name, err)
                        })?;
                    }
                    MachineConfigOverlayTarget::Video => {
                        value
                            .clone()
                            .try_into::<VideoCardConfig>()
                            .map_err(|err| anyhow::anyhow!("Overlay '{}': invalid video value: {}", self.name, err))?;
                    }
                }
                if self.has_replacement_payload() {
                    return Err(anyhow::anyhow!(
                        "Overlay '{}': merge operation cannot also contain replacement fields",
                        self.name
                    ));
                }
            }
        }
        Ok(())
    }

    fn has_replacement_payload(&self) -> bool {
        self.cpu.is_some()
            || self.memory.is_some()
            || self.ems.is_some()
            || self.fdc.is_some()
            || self.hdc.is_some()
            || self.serial.is_some()
            || self.parallel.is_some()
            || self.video.is_some()
            || self.sound.is_some()
            || self.keyboard.is_some()
            || self.serial_mouse.is_some()
            || self.virtual_mouse.is_some()
            || self.game_port.is_some()
            || self.conventional_expansion.is_some()
            || self.media.is_some()
    }

    fn bind_arguments(
        &self,
        invocation: &MachineConfigOverlayInvocation<'_>,
    ) -> Result<BTreeMap<String, MachineConfigOverlayValue>, Error> {
        let mut raw_arguments: BTreeMap<&str, &str> = BTreeMap::new();

        if let Some(argument_list) = invocation.arguments {
            if self.parameters.is_empty() {
                return Err(anyhow::anyhow!("Overlay '{}' does not accept parameters", self.name));
            }

            if !argument_list.contains('=') {
                if self.parameters.len() != 1 {
                    return Err(anyhow::anyhow!(
                        "Overlay '{}': positional syntax requires exactly one declared parameter",
                        self.name
                    ));
                }
                raw_arguments.insert(self.parameters[0].name(), argument_list.trim());
            }
            else {
                for argument in argument_list.split(';') {
                    let (name, value) = argument.split_once('=').ok_or_else(|| {
                        anyhow::anyhow!("Overlay '{}': invalid named argument '{}'", self.name, argument)
                    })?;
                    let name = name.trim();
                    let value = value.trim();
                    if name.is_empty() || value.is_empty() {
                        return Err(anyhow::anyhow!(
                            "Overlay '{}': invalid named argument '{}'",
                            self.name,
                            argument
                        ));
                    }
                    if raw_arguments.insert(name, value).is_some() {
                        return Err(anyhow::anyhow!(
                            "Overlay '{}': parameter '{}' was specified more than once",
                            self.name,
                            name
                        ));
                    }
                }
            }
        }

        for name in raw_arguments.keys() {
            if !self.parameters.iter().any(|parameter| parameter.name() == *name) {
                return Err(anyhow::anyhow!("Overlay '{}': unknown parameter '{}'", self.name, name));
            }
        }

        let mut bound_arguments = BTreeMap::new();
        for parameter in &self.parameters {
            let value = match raw_arguments.get(parameter.name()) {
                Some(raw_value) => parameter.parse_value(&self.name, raw_value)?,
                None => match parameter.default_value() {
                    Some(default) => default,
                    None if parameter.is_required() => {
                        return Err(anyhow::anyhow!(
                            "Overlay '{}': missing required parameter '{}'",
                            self.name,
                            parameter.name()
                        ));
                    }
                    None => continue,
                },
            };
            bound_arguments.insert(parameter.name().to_string(), value);
        }
        Ok(bound_arguments)
    }
}

/*
#[derive(Clone, Debug, Deserialize)]
pub struct ParallelControllerConfig {
    type: ParallelControllerType,
    port: Vec<ParallelPortConfig>,
}
 */

#[derive(Default)]
pub struct MachineManager {
    active_config: Option<MachineConfigFileEntry>,
    config_names: MartyHashSet<String>,
    overlay_names: MartyHashSet<String>,
    configs: BTreeMap<String, MachineConfigFileEntry>,
    overlays: BTreeMap<String, MachineConfigFileOverlayEntry>,
    features_requested: MartyHashSet<String>,
    features_provided: MartyHashSet<String>,
    rom_sets_required: Vec<usize>,
}

impl MachineManager {
    pub fn new() -> Self {
        Self::default()
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
            overlay.validate_definition()?;
            if self.overlays.contains_key(&overlay.name) {
                return Err(anyhow::anyhow!("Duplicate overlay name: {}", overlay.name));
            }
            self.overlay_names.insert(overlay.name.clone());
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

        // Filter empty strings
        total_overlays.retain(|overlay| !overlay.is_empty());

        for overlay_spec in total_overlays {
            let invocation = MachineConfigOverlayInvocation::parse(&overlay_spec)?;
            let overlay = self.overlays.get(invocation.name).ok_or(anyhow::anyhow!(
                "Machine configuration overlay not found: {}",
                invocation.name
            ))?;
            let arguments = overlay.bind_arguments(&invocation)?;
            config.apply_overlay(overlay.clone(), &arguments)?;
        }

        self.active_config = Some(config);
        Ok(self.active_config.as_ref().unwrap())
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

    /// Returns a tuple of vectors of strings representing the required and optional ROM features for this
    /// configuration
    pub fn get_rom_requirements(&self, load_custom_roms: bool) -> Result<(Vec<String>, Vec<String>), Error> {
        let mut req_set: MartyHashSet<String> = MartyHashSet::default();
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
                if *feature == "custom" && !load_custom_roms {
                    log::debug!("Skipping custom ROM feature as load_custom_roms is false.");
                    continue;
                }
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
                HardDiskControllerType::JrIde => {
                    if req_set.insert(String::from("expansion")) {
                        req_vec.push(String::from("expansion"));
                    }
                    if req_set.insert(String::from("jride")) {
                        req_vec.push(String::from("jride"));
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

    /// Apply either a replacement overlay or a parameterized merge overlay to this configuration.
    fn apply_overlay(
        &mut self,
        overlay: MachineConfigFileOverlayEntry,
        arguments: &BTreeMap<String, MachineConfigOverlayValue>,
    ) -> Result<(), Error> {
        match overlay.operation {
            MachineConfigOverlayOperation::Replace => self.apply_replacement_overlay(overlay),
            MachineConfigOverlayOperation::Merge => self.apply_merge_overlay(overlay, arguments),
        }
    }

    fn apply_replacement_overlay(&mut self, overlay: MachineConfigFileOverlayEntry) -> Result<(), Error> {
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
        if let Some(parallel) = overlay.parallel {
            log::debug!("Applying parallel overlay: {:?}", parallel);
            self.parallel = Some(parallel);
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
            self.virtual_mouse = None;
        }
        if let Some(virtual_mouse) = overlay.virtual_mouse {
            log::debug!("Applying virtual mouse overlay: {:?}", virtual_mouse);
            self.virtual_mouse = Some(virtual_mouse);
            self.serial_mouse = None;
        }
        if let Some(game_port) = overlay.game_port {
            log::debug!("Applying game port overlay: {:?}", game_port);
            self.game_port = Some(game_port);
        }
        if let Some(conventional_expansion) = overlay.conventional_expansion {
            log::debug!("Applying conventional expansion overlay: {:?}", conventional_expansion);
            self.conventional_expansion = Some(conventional_expansion);
        }
        Ok(())
    }

    fn apply_merge_overlay(
        &mut self,
        overlay: MachineConfigFileOverlayEntry,
        arguments: &BTreeMap<String, MachineConfigOverlayValue>,
    ) -> Result<(), Error> {
        let MachineConfigFileOverlayEntry {
            name,
            target,
            selector,
            value,
            ..
        } = overlay;
        let target = target.ok_or_else(|| anyhow::anyhow!("Overlay '{}': merge operation requires a target", name))?;
        let selector =
            selector.ok_or_else(|| anyhow::anyhow!("Overlay '{}': merge operation requires a selector", name))?;
        let value = value.ok_or_else(|| anyhow::anyhow!("Overlay '{}': merge operation requires a value", name))?;
        let target_name = target.name();
        let target_index = match arguments.get(&selector) {
            Some(MachineConfigOverlayValue::Integer(value)) => usize::try_from(*value).map_err(|_| {
                anyhow::anyhow!(
                    "Overlay '{}': parameter '{}' cannot be used as a {} index: {}",
                    name,
                    selector,
                    target_name,
                    value
                )
            })?,
            None => {
                return Err(anyhow::anyhow!(
                    "Overlay '{}': selector parameter '{}' was not bound",
                    name,
                    selector
                ));
            }
        };

        match target {
            MachineConfigOverlayTarget::FdcDrive => {
                let drive_index = target_index;
                let drive = value
                    .try_into::<FloppyDriveConfig>()
                    .map_err(|err| anyhow::anyhow!("Overlay '{}': invalid fdc.drive value: {}", name, err))?;
                let fdc = self.fdc.as_mut().ok_or_else(|| {
                    anyhow::anyhow!(
                        "Overlay '{}': cannot merge fdc.drive[{}] because the machine has no floppy controller",
                        name,
                        drive_index
                    )
                })?;
                let maximum = fdc.fdc_type.max_drives();
                if maximum == 0 {
                    return Err(anyhow::anyhow!(
                        "Overlay '{}': floppy controller {:?} does not support any drives",
                        name,
                        fdc.fdc_type
                    ));
                }
                if fdc.drive.len() > maximum {
                    return Err(anyhow::anyhow!(
                        "Floppy controller {:?} has {} configured drives but supports at most {}",
                        fdc.fdc_type,
                        fdc.drive.len(),
                        maximum
                    ));
                }
                if drive_index >= maximum {
                    return Err(anyhow::anyhow!(
                        "Overlay '{}': floppy controller {:?} supports drive indices 0 through {}, received {}",
                        name,
                        fdc.fdc_type,
                        maximum - 1,
                        drive_index
                    ));
                }

                match drive_index.cmp(&fdc.drive.len()) {
                    std::cmp::Ordering::Less => {
                        log::debug!(
                            "Overlay '{}': replacing fdc.drive[{}] with {:?}",
                            name,
                            drive_index,
                            drive
                        );
                        fdc.drive[drive_index] = drive;
                    }
                    std::cmp::Ordering::Equal => {
                        log::debug!(
                            "Overlay '{}': appending fdc.drive[{}] as {:?}",
                            name,
                            drive_index,
                            drive
                        );
                        fdc.drive.push(drive);
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(anyhow::anyhow!(
                            "Overlay '{}': cannot create fdc.drive[{}] before fdc.drive[{}] is configured",
                            name,
                            drive_index,
                            fdc.drive.len()
                        ));
                    }
                }
            }
            MachineConfigOverlayTarget::Video => {
                let card = value
                    .try_into::<VideoCardConfig>()
                    .map_err(|err| anyhow::anyhow!("Overlay '{}': invalid video value: {}", name, err))?;
                let video = self.video.get_or_insert_default();

                match target_index.cmp(&video.len()) {
                    std::cmp::Ordering::Less => {
                        log::debug!("Overlay '{}': replacing video[{}] with {:?}", name, target_index, card);
                        video[target_index] = card;
                    }
                    std::cmp::Ordering::Equal => {
                        log::debug!("Overlay '{}': appending video[{}] as {:?}", name, target_index, card);
                        video.push(card);
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(anyhow::anyhow!(
                            "Overlay '{}': cannot create video[{}] before video[{}] is configured",
                            name,
                            target_index,
                            video.len()
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn to_machine_config(&self) -> MachineConfiguration {
        MachineConfiguration {
            speaker: self.speaker,
            cassette: self.cassette,
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
            parallel: self.parallel.clone().unwrap_or_default(),
            keyboard: self.keyboard.clone(),
            serial_mouse: self.serial_mouse.clone(),
            virtual_mouse: self.virtual_mouse.clone(),
            game_port: self.game_port.clone(),
            controller_layout: self.game_port.as_ref().map(|gp| gp.controller_layout.clone()).flatten(),
            conventional_expansion: self.conventional_expansion.clone().unwrap_or_default(),
            media: self.media.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use marty_core::{
        device_traits::videocard::{VideoCardSubType, VideoType},
        machine_types::FloppyDriveType,
    };

    const TEST_CONFIG: &str = r#"
[[machine]]
name = "xt"
type = "Ibm5160"
rom_set = "auto"
overlays = ["ibm_nec_fdc"]

[machine.memory]
conventional.size = 0xA0000
conventional.wait_states = 0

[[machine]]
name = "pcjr"
type = "IbmPCJr"
rom_set = "auto"
cassette = true
overlays = []

[machine.memory]
conventional.size = 0x20000
conventional.wait_states = 0

[machine.fdc]
type = "IbmPCJrNec"

[[machine]]
name = "bare"
type = "Ibm5160"
rom_set = "auto"
overlays = []

[machine.memory]
conventional.size = 0xA0000
conventional.wait_states = 0

[[overlay]]
name = "ibm_nec_fdc"
operation = "replace"

[overlay.fdc]
type = "IbmNec"

[[overlay]]
name = "floppy_360k"
operation = "merge"
target = "fdc.drive"
selector = "drive"

[[overlay.parameters]]
name = "drive"
type = "integer"
min = 0
max = 3
required = true

[overlay.value]
type = "360k"

[[overlay]]
name = "floppy_720k"
operation = "merge"
target = "fdc.drive"
selector = "drive"

[[overlay.parameters]]
name = "drive"
type = "integer"
min = 0
max = 3
required = true

[overlay.value]
type = "720k"

[[overlay]]
name = "ibm_cga"
operation = "replace"

[[overlay.video]]
type = "CGA"

[[overlay]]
name = "ibm_mda"
operation = "merge"
target = "video"
selector = "card"

[[overlay.parameters]]
name = "card"
type = "integer"
min = 0
required = false
default = 0

[overlay.value]
type = "MDA"

[[overlay]]
name = "hercules"
operation = "merge"
target = "video"
selector = "card"

[[overlay.parameters]]
name = "card"
type = "integer"
min = 0
required = false
default = 0

[overlay.value]
type = "MDA"
subtype = "Hercules"

[[overlay]]
name = "serial_mouse"

[overlay.serial_mouse]
type = "Microsoft"
port = 1

[[overlay]]
name = "virtmouse"

[overlay.virtual_mouse]
irq = 5
"#;

    fn test_manager() -> MachineManager {
        let mut file = toml::from_str::<MachineConfigFile>(TEST_CONFIG).expect("test configuration should parse");
        let mut manager = MachineManager::new();

        for config in file.machine.take().unwrap_or_default() {
            manager.configs.insert(config.name.clone(), config);
        }
        for overlay in file.overlay.take().unwrap_or_default() {
            overlay.validate_definition().expect("test overlay should validate");
            manager.overlays.insert(overlay.name.clone(), overlay);
        }
        manager
    }

    fn test_overlay(source: &str) -> MachineConfigFileOverlayEntry {
        toml::from_str::<MachineConfigFile>(source)
            .expect("test overlay should parse")
            .overlay
            .expect("test overlay file should contain an overlay")
            .remove(0)
    }

    #[test]
    fn indexed_floppy_overlays_build_mixed_drive_configuration() {
        let mut manager = test_manager();
        let overlays = vec!["floppy_360k:0".to_string(), "floppy_720k:drive=1".to_string()];
        let config = manager.get_config_with_overlays("xt", &overlays).unwrap();
        let drives = &config.fdc.as_ref().unwrap().drive;

        assert_eq!(drives.len(), 2);
        assert_eq!(drives[0].fd_type, FloppyDriveType::Floppy360K);
        assert_eq!(drives[1].fd_type, FloppyDriveType::Floppy720K);
    }

    #[test]
    fn indexed_floppy_overlay_replaces_an_existing_slot() {
        let mut manager = test_manager();
        let overlays = vec![
            "floppy_720k:0".to_string(),
            "floppy_720k:1".to_string(),
            "floppy_360k:0".to_string(),
        ];
        let config = manager.get_config_with_overlays("xt", &overlays).unwrap();
        let drives = &config.fdc.as_ref().unwrap().drive;

        assert_eq!(drives.len(), 2);
        assert_eq!(drives[0].fd_type, FloppyDriveType::Floppy360K);
        assert_eq!(drives[1].fd_type, FloppyDriveType::Floppy720K);
    }

    #[test]
    fn indexed_video_overlay_defaults_to_primary_slot() {
        let mut manager = test_manager();
        let overlays = vec!["ibm_cga".to_string(), "ibm_mda".to_string()];
        let config = manager.get_config_with_overlays("bare", &overlays).unwrap();
        let cards = config.video.as_ref().unwrap();

        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].video_type, VideoType::MDA);
    }

    #[test]
    fn indexed_video_overlay_builds_dual_adapter_configuration() {
        let mut manager = test_manager();
        let overlays = vec!["ibm_cga".to_string(), "ibm_mda:1".to_string()];
        let config = manager.get_config_with_overlays("bare", &overlays).unwrap();
        let cards = config.video.as_ref().unwrap();

        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].video_type, VideoType::CGA);
        assert_eq!(cards[1].video_type, VideoType::MDA);
    }

    #[test]
    fn indexed_hercules_overlay_builds_dual_adapter_configuration() {
        let mut manager = test_manager();
        let overlays = vec!["ibm_cga".to_string(), "hercules:1".to_string()];
        let config = manager.get_config_with_overlays("bare", &overlays).unwrap();
        let cards = config.video.as_ref().unwrap();

        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].video_type, VideoType::CGA);
        assert_eq!(cards[1].video_type, VideoType::MDA);
        assert_eq!(cards[1].video_subtype, Some(VideoCardSubType::Hercules));
    }

    #[test]
    fn indexed_video_overlay_rejects_gaps() {
        let mut manager = test_manager();
        let error = manager
            .get_config_with_overlays("bare", &vec!["ibm_mda:1".to_string()])
            .unwrap_err()
            .to_string();

        assert!(error.contains("cannot create video[1] before video[0] is configured"));
    }

    #[test]
    fn indexed_floppy_overlay_rejects_missing_and_invalid_parameters() {
        let mut manager = test_manager();

        let missing = manager
            .get_config_with_overlays("xt", &vec!["floppy_360k".to_string()])
            .unwrap_err()
            .to_string();
        assert!(missing.contains("missing required parameter 'drive'"));

        let out_of_range = manager
            .get_config_with_overlays("xt", &vec!["floppy_360k:4".to_string()])
            .unwrap_err()
            .to_string();
        assert!(out_of_range.contains("must be between 0 and 3"));

        let unknown = manager
            .get_config_with_overlays("xt", &vec!["floppy_360k:slot=0".to_string()])
            .unwrap_err()
            .to_string();
        assert!(unknown.contains("unknown parameter 'slot'"));
    }

    #[test]
    fn malformed_merge_overlays_return_errors_instead_of_panicking() {
        let manager = test_manager();
        let mut config = manager.configs.get("xt").unwrap().clone();
        let arguments = BTreeMap::from([("drive".to_string(), MachineConfigOverlayValue::Integer(0))]);

        let missing_target = test_overlay(
            r#"
[[overlay]]
name = "missing_target"
operation = "merge"
"#,
        );
        assert!(config
            .apply_overlay(missing_target, &arguments)
            .unwrap_err()
            .to_string()
            .contains("requires a target"));

        let missing_selector = test_overlay(
            r#"
[[overlay]]
name = "missing_selector"
operation = "merge"
target = "fdc.drive"
"#,
        );
        assert!(config
            .apply_overlay(missing_selector, &arguments)
            .unwrap_err()
            .to_string()
            .contains("requires a selector"));

        let missing_value = test_overlay(
            r#"
[[overlay]]
name = "missing_value"
operation = "merge"
target = "fdc.drive"
selector = "drive"
"#,
        );
        assert!(config
            .apply_overlay(missing_value, &arguments)
            .unwrap_err()
            .to_string()
            .contains("requires a value"));
    }

    #[test]
    fn indexed_floppy_overlay_rejects_gaps_and_missing_controllers() {
        let mut manager = test_manager();

        let gap = manager
            .get_config_with_overlays("xt", &vec!["floppy_360k:1".to_string()])
            .unwrap_err()
            .to_string();
        assert!(gap.contains("before fdc.drive[0] is configured"));

        let no_controller = manager
            .get_config_with_overlays("bare", &vec!["floppy_360k:0".to_string()])
            .unwrap_err()
            .to_string();
        assert!(no_controller.contains("machine has no floppy controller"));
    }

    #[test]
    fn pcjr_controller_rejects_a_second_drive() {
        let mut manager = test_manager();
        let overlays = vec!["floppy_360k:0".to_string(), "floppy_360k:1".to_string()];
        let error = manager
            .get_config_with_overlays("pcjr", &overlays)
            .unwrap_err()
            .to_string();

        assert!(error.contains("supports drive indices 0 through 0"));
    }

    #[test]
    fn cassette_defaults_to_disabled_and_propagates_when_enabled() {
        let manager = test_manager();

        let xt = manager.configs.get("xt").unwrap();
        assert!(!xt.cassette);
        assert!(!xt.to_machine_config().cassette);

        let pcjr = manager.configs.get("pcjr").unwrap();
        assert!(pcjr.cassette);
        assert!(pcjr.to_machine_config().cassette);
    }

    #[test]
    fn mouse_overlays_replace_the_other_mouse_transport() {
        let mut manager = test_manager();
        let virtual_config = manager
            .get_config_with_overlays("bare", &vec!["serial_mouse".to_string(), "virtmouse".to_string()])
            .unwrap();
        assert!(virtual_config.serial_mouse.is_none());
        assert!(virtual_config.virtual_mouse.is_some());

        let serial_config = manager
            .get_config_with_overlays("bare", &vec!["virtmouse".to_string(), "serial_mouse".to_string()])
            .unwrap();
        assert!(serial_config.serial_mouse.is_some());
        assert!(serial_config.virtual_mouse.is_none());
    }

    #[cfg(all(feature = "ega", feature = "vga"))]
    #[test]
    fn installed_machine_configurations_resolve_with_parameterized_overlays() {
        let machine_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../install/configs/machines");
        let mut manager = MachineManager::new();
        let mut machine_configs = Vec::new();
        let mut overlay_configs = Vec::new();

        for entry in std::fs::read_dir(&machine_dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("toml") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();

            let file = toml::from_str::<MachineConfigFile>(&source)
                .unwrap_or_else(|err| panic!("Failed to parse {}: {}", path.display(), err));
            machine_configs.append(&mut file.machine.unwrap_or_default());
            overlay_configs.append(&mut file.overlay.unwrap_or_default());
        }

        for config in machine_configs {
            assert!(manager.configs.insert(config.name.clone(), config).is_none());
        }
        for overlay in overlay_configs {
            overlay.validate_definition().unwrap();
            assert!(manager.overlays.insert(overlay.name.clone(), overlay).is_none());
        }

        let names = manager.get_config_names();
        for name in names {
            manager
                .get_config_with_overlays(&name, &Vec::new())
                .unwrap_or_else(|err| panic!("Failed to resolve machine '{}': {}", name, err));
        }
    }
}

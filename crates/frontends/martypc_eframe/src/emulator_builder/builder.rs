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

//! An [EmulatorBuilder] implements the Builder pattern for an [Emulator].
//! The primary goal is to be able to share code between different frontends
//! that need flexibility in how they configure and build their emulator
//! instances.

use std::{
    cell::RefCell,
    io::Write,
    path::{Path, PathBuf},
    rc::Rc,
};

use crate::{
    counter::Counter,
    emulator::{
        joystick_state::JoystickData,
        keyboard_state::KeyboardData,
        mouse_state::MouseData,
        EmuFlags,
        Emulator,
    },
    input::HotkeyManager,
};

use marty_config::ConfigFileParams;

// This module will export either a rodio or null sound interface depending on the `sound` feature.
use crate::sound::SoundInterface;

#[cfg(feature = "cpu_validator")]
use marty_core::cpu_validator::ValidatorType;
use marty_core::{
    machine::{ExecutionControl, ExecutionState, MachineBuilder},
    supported_floppy_extensions,
};
use marty_egui::state::GuiState;
use marty_frontend_common::{
    cartridge_manager::CartridgeManager,
    floppy_manager::FloppyManager,
    machine_manager::MachineManager,
    resource_manager::ResourceManager,
    rom_manager::RomManager,
    types::resource_location::ResourceLocation,
    vhd_manager::VhdManager,
};

use anyhow::{anyhow, Error};
use url::Url;

#[derive(thiserror::Error, Debug)]
pub enum EmuBuilderError {
    #[error("Configuration file '{0}' could not be found")]
    ConfigNotFound(String),
    #[error("IO Error reading configuration file '{0}': {1}")]
    ConfigIOError(String, String),
    #[error("Error parsing configuration file '{0}': {1}")]
    ConfigParseError(String, String),
    #[error("An operation was attempted that was not supported on the current platform: {0}")]
    UnsupportedPlatform(String),
    #[error("Failed to open sound device: {0}")]
    AudioDeviceError(String),
    #[error("Failed to open sound stream: {0}")]
    AudioStreamError(String),
    #[error(
        "MartyPC was compiled with CPU validation enabled, but no validator type was specified in the configuration."
    )]
    ValidatorNotSpecified,
    #[error("No Resource paths found")]
    NoResourcePaths,
    #[error("An error occurred reading or scanning emulator resources: {0}")]
    ResourceError(String),
    #[error("An error occurred reading Machine Configuration files: {0}")]
    MachineConfigError(String),
    #[error("No Machine Configuration was found for the specified name: {0}")]
    BadMachineConfig(String),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error), // source and Display delegate to anyhow::Error
}

#[derive(Default, Debug)]
pub enum BuildPlatform {
    #[default]
    Native,
    Web,
}

#[derive(Default)]
pub struct EmulatorBuilder {
    platform: BuildPlatform,
    toml_config_path: Option<PathBuf>,
    toml_config_url: Option<Url>,
    toml_manifest_url: Option<Url>,
    base_url: Option<Url>,
    #[cfg(feature = "cpu_validator")]
    validator: Option<ValidatorType>,

    enable_floppy_manager: bool,
    enable_vhd_manager: bool,
    enable_cart_manager: bool,
    enable_sound: bool,
    enable_keyboard: bool,
    enable_mouse: bool,
    enable_serial: bool,
    enable_gui: bool,
}

impl EmulatorBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_platform(mut self, platform: BuildPlatform) -> Self {
        self.platform = platform;
        self
    }

    pub fn with_base_url(mut self, url: &Url) -> Self {
        self.base_url = Some(url.clone());
        self
    }

    pub fn with_toml_config_path(mut self, path: impl AsRef<Path>) -> Self {
        self.toml_config_path = Some(path.as_ref().to_owned());
        self
    }

    pub fn with_toml_config_url(mut self, url: &Url) -> Self {
        self.toml_config_url = Some(url.to_owned());
        self
    }

    pub fn with_toml_manifest_url(mut self, url: &Url) -> Self {
        self.toml_manifest_url = Some(url.to_owned());
        self
    }

    #[cfg(feature = "cpu_validator")]
    pub fn with_validator(mut self, v_type: Option<ValidatorType>) -> Self {
        self.validator = v_type;
        self
    }

    pub fn enable_floppy_manager(mut self, enabled: bool) -> Self {
        self.enable_floppy_manager = enabled;
        self
    }

    pub fn enable_vhd_manager(mut self, enabled: bool) -> Self {
        self.enable_vhd_manager = enabled;
        self
    }

    pub fn enable_cart_manager(mut self, enabled: bool) -> Self {
        self.enable_cart_manager = enabled;
        self
    }

    pub fn enable_gui(mut self, enabled: bool) -> Self {
        self.enable_gui = enabled;
        self
    }

    pub fn enable_sound(mut self, enabled: bool) -> Self {
        self.enable_sound = enabled;
        self
    }

    pub fn enable_keyboard(mut self, enabled: bool) -> Self {
        self.enable_keyboard = enabled;
        self
    }

    pub fn enable_mouse(mut self, enabled: bool) -> Self {
        self.enable_mouse = enabled;
        self
    }

    pub fn enable_serial(mut self, enabled: bool) -> Self {
        self.enable_serial = enabled;
        self
    }

    /// Load and resolve the configuration. This uses the `marty_config` crate to parse TOML,
    /// and merge with either command line arguments (native) or query parameters (web).
    /// The result is a [ConfigFileParams] struct that can be used to build the emulator.
    async fn resolve_config(&self) -> Result<ConfigFileParams, EmuBuilderError> {
        // One of toml_config_path or toml_config_url must be set, but not both.
        if self.toml_config_path.is_none() && self.toml_config_url.is_none() {
            panic!("No configuration file specified");
        }

        if self.toml_config_path.is_some() && self.toml_config_url.is_some() {
            panic!("Do not specify both file and url config paths");
        }

        let config_location = if self.toml_config_path.is_some() {
            ResourceLocation::FilePath(self.toml_config_path.as_ref().unwrap().clone())
        }
        else {
            ResourceLocation::Url(self.toml_config_url.as_ref().unwrap().clone())
        };

        use EmuBuilderError::*;
        match config_location {
            ResourceLocation::FilePath(path) => match marty_config::read_config_file(&path) {
                Ok(config) => Ok(config),
                Err(e) => match e.downcast_ref::<std::io::Error>() {
                    Some(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        Err(ConfigNotFound(path.to_string_lossy().as_ref().to_string()))
                    }
                    Some(e) => Err(ConfigIOError(
                        path.to_string_lossy().as_ref().to_string(),
                        e.to_string(),
                    )),
                    None => Err(ConfigParseError(
                        path.to_string_lossy().as_ref().to_string(),
                        e.to_string(),
                    )),
                },
            },
            ResourceLocation::Url(url) => {
                #[cfg(target_arch = "wasm32")]
                {
                    let url_string = url.as_str().to_string();
                    let config = marty_web_helpers::fetch_file(url.as_str())
                        .await
                        .map_err(|e| ConfigIOError(url_string.clone(), e.to_string()))?;

                    match marty_config::read_config_string(
                        &std::str::from_utf8(&config).expect("TOML contained invalid UTF-8"),
                    ) {
                        Ok(config) => return Ok(config),
                        Err(e) => return Err(ConfigParseError(url.as_str().to_string(), e.to_string())),
                    }
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    _ = url;
                    Err(UnsupportedPlatform(
                        "URL configuration not supported in native builds".to_string(),
                    ))
                }
            }
        }
    }

    /// Build the [Emulator] using the previously supplied parameters from the call chain.
    /// This function will return an [Emulator] instance if successful, or an [Error] if
    /// something went wrong.
    ///
    /// Since it performs async operations, this function is async, but it does not
    /// return a future.
    ///
    /// This function may print debug info that could be useful to the user. We provide two Write
    /// streams for this purpose: one for normal output, and one for error output. This can be
    /// regular stdout and stderr, or we could capture into a Cursor or file.
    ///
    /// # Arguments
    /// - `stdout` - A mutable reference to an implementation of `Write` that will be used to log
    ///     normal startup messages. This is typically a file or stdout.
    /// - `stderr` - A mutable reference to an implementation of `Write` that will be used to log
    ///     error messages. This is typically a file or stderr.
    pub async fn build<W, WE>(self, stdout: &mut W, stderr: &mut WE) -> Result<Emulator, EmuBuilderError>
    where
        W: Write,
        WE: Write,
    {
        use EmuBuilderError::*;

        // First we need to resolve a configuration. On native this typically comes from a TOML file.
        let config = self.resolve_config().await?;

        // Initialize sound early as we have limited time after a user click event to start audio
        // on web.
        let mut sound_config = Default::default();
        let mut sound_player = if self.enable_sound | config.emulator.audio.enabled {
            let mut sound_player = SoundInterface::new(config.emulator.audio.enabled);

            match sound_player.open_device() {
                Ok(_) => {
                    stdout.write_fmt(format_args!("Opened audio device: {}", sound_player.device_name()))?;
                }
                Err(e) => {
                    return Err(AudioDeviceError(e.to_string()));
                }
            }

            match sound_player.open_stream() {
                Ok(_) => {
                    stdout.write_fmt(format_args!("Opened audio stream."))?;
                }
                Err(e) => {
                    return Err(AudioStreamError(e.to_string()));
                }
            }

            sound_config = sound_player.config();
            Some(sound_player)
        }
        else {
            None
        };

        // First we can check that we have a validator type, if the validation feature is enabled.
        #[cfg(feature = "cpu_validator")]
        match config.validator.vtype {
            Some(ValidatorType::None) | None => {
                return Err(ValidatorNotSpecified);
            }
            _ => {}
        }

        log::debug!("Creating ResourceManager...");
        // Now that we have our configuration, we can instantiate a ResourceManager.
        let mut resource_manager =
            match ResourceManager::from_config(config.emulator.basedir.clone(), &config.emulator.paths) {
                Ok(rm) => rm,
                Err(e) => {
                    return Err(ResourceError(e.to_string()));
                }
            };

        // Set the base url for resources loaded by the resource manager
        if let Some(base_url) = self.base_url {
            resource_manager.set_base_url(&base_url);
        }

        // Load the file manifest if specified.
        // The file manifest provides the wasm build with information about what files it can
        // fetch, creating a sort of virtual filesystem.
        if let Some(manifest_url) = &self.toml_manifest_url {
            log::debug!("Loading file manifest from {:?}...", manifest_url.as_str());
            resource_manager
                .load_manifest(ResourceLocation::Url(manifest_url.clone()))
                .await?;
        }

        // Mount the virtual filesystem if specified
        if let Some(ref virtual_fs) = config.emulator.virtual_fs {
            log::debug!(
                "Loading virtual filesystem from {:?}",
                virtual_fs.to_string_lossy().into_owned()
            );

            resource_manager.mount_overlay(&virtual_fs).await?;
        }

        let resolved_paths = resource_manager.pm.dump_paths();
        if resolved_paths.is_empty() {
            return Err(NoResourcePaths);
        }
        for path in &resolved_paths {
            log::debug!("Resolved resource path: {:?}", path);
            stdout.write_fmt(format_args!("Resolved resource path: {:?}", path))?;
            stdout.flush()?;
        }

        // Tell the resource manager to ignore specified dirs
        if let Some(ignore_dirs) = &config.emulator.ignore_dirs {
            resource_manager.set_ignore_dirs(ignore_dirs.clone());
        }

        // Instantiate the new machine manager to load Machine configurations.
        log::debug!("Creating MachineManager...");
        let mut machine_manager = MachineManager::new();
        if let Err(e) = machine_manager.load_configs(&mut resource_manager).await {
            let err_str = format!("Error loading Machine configuration files: {}", e);
            stderr.write(err_str.as_bytes())?;
            return Err(MachineConfigError(e.to_string()));
        }

        // Initialize machine configuration name, options and prefer_oem flag.
        // If benchmark_mode is true, we use the values from the benchmark configuration section. This
        // gives us the ability to run benchmarks with a consistent, static configuration.
        let mut init_config_name = config.machine.config_name.clone();
        let mut init_prefer_oem = config.machine.prefer_oem;
        let mut init_config_overlays = config.machine.config_overlays.clone().unwrap_or_default();

        if config.emulator.benchmark_mode {
            init_config_name = config.emulator.benchmark.config_name.clone();
            init_prefer_oem = config.emulator.benchmark.prefer_oem;
            init_config_overlays = config.emulator.benchmark.config_overlays.clone().unwrap_or_default();

            stdout.write_fmt(format_args!(
                "Benchmark mode enabled. Using machine config: {} config overlays: [{}] prefer_oem: {}",
                init_config_name,
                init_config_overlays.join(", "),
                init_prefer_oem
            ))?;
        }

        // Get a list of machine configuration names
        let machine_names = machine_manager.get_config_names();
        let have_machine_config = machine_names.contains(&init_config_name);

        // Do --machinescan commandline argument. We print machine info (and ROM info if --romscan
        // was also specified), then quit.
        if config.emulator.machinescan {
            // Print the list of machine configurations and their rom requirements
            for machine in machine_names {
                stdout.write_fmt(format_args!("Machine: {}", machine))?;
                if let Some(reqs) = machine_manager
                    .get_config(&machine)
                    .and_then(|config| Some(config.get_rom_requirements()))
                {
                    stdout.write_fmt(format_args!("  Requires: {:?}", reqs))?;
                }
            }

            if !have_machine_config {
                let err_str = format!("Warning! No matching configuration found for: {}", init_config_name);
                stderr.write(err_str.as_bytes())?;
                std::process::exit(1);
            }

            // Exit unless we will also have --romscan
            if !config.emulator.romscan {
                std::process::exit(0);
            }
        }

        // We should have a resolved machine configuration now. If we don't, we should exit.
        if !have_machine_config {
            let err_str = format!(
                "No machine configuration for specified config name: {}",
                &init_config_name
            );
            stderr.write(err_str.as_bytes())?;
            return Err(BadMachineConfig(init_config_name));
        }

        // Instantiate the ROM Manager
        log::debug!("Creating RomManager...");
        let mut rom_manager = RomManager::new(init_prefer_oem);
        // Load ROM definitions
        rom_manager.load_defs(&mut resource_manager).await?;

        // Get the ROM requirements for the requested machine type
        let machine_config_file = {
            for overlay in init_config_overlays.iter() {
                log::debug!("Have machine config overlay from global config: {}", overlay);
            }
            let overlay_vec = init_config_overlays.clone();

            machine_manager.get_config_with_overlays(&init_config_name, &overlay_vec)?
        };

        // Collect the ROM requirements for the machine configuration
        let (required_features, optional_features) = machine_config_file.get_rom_requirements()?;

        // Scan the rom resource director(ies)
        rom_manager.scan(&mut resource_manager).await?;
        // Determine what complete ROM sets we have
        rom_manager.resolve_rom_sets()?;

        // Do --romscan option.  We print rom and machine info and quit.
        if config.emulator.romscan {
            rom_manager.print_rom_stats();
            rom_manager.print_romset_stats();
            std::process::exit(0);
        }

        // Output ROM feature requirements and optional requests
        stdout.write_fmt(format_args!(
            "Selected machine config {} requires the following ROM features:",
            init_config_name
        ))?;
        for rom_feature in &required_features {
            stdout.write_fmt(format_args!("  {}", rom_feature))?;
        }
        stdout.write_fmt(format_args!(
            "Selected machine config {} optionally requests the following ROM features:",
            init_config_name
        ))?;
        for rom_feature in &optional_features {
            stdout.write_fmt(format_args!("  {}", rom_feature))?;
        }

        // Determine if the machine configuration specifies a particular ROM set
        let specified_rom_set = machine_config_file.get_specified_rom_set();

        // Resolve the ROM requirements for the requested ROM features
        let rom_sets_resolved =
            rom_manager.resolve_requirements(required_features, optional_features, specified_rom_set)?;

        // Output resolved ROM sets
        stdout.write_fmt(format_args!(
            "Selected machine config {} has resolved the following ROM sets:",
            init_config_name
        ))?;
        for rom_set in &rom_sets_resolved {
            stdout.write_fmt(format_args!("  {}", rom_set))?;
        }

        // Create the ROM manifest to pass to the emulator core
        let rom_manifest = rom_manager
            .create_manifest_async(rom_sets_resolved.clone(), &mut resource_manager)
            .await?;

        log::debug!("Created manifest!");
        for (i, rom) in rom_manifest.roms.iter().enumerate() {
            log::debug!("  rom {}: md5: {} length: {}", i, rom.md5, rom.data.len());
        }

        // Instantiate the floppy manager
        log::debug!("Creating FloppyManager...");
        let mut floppy_manager = FloppyManager::new();

        // Get a combined list of the floppy extensions we should recognize from the config,
        // and the extensions we natively support via fluxfox
        let mut floppy_extensions = config
            .emulator
            .media
            .raw_sector_image_extensions
            .clone()
            .unwrap_or_default();
        let managed_extensions = supported_floppy_extensions();
        log::debug!(
            "marty_core reports native support for the following extensions: {:?}",
            managed_extensions
        );
        managed_extensions.iter().for_each(|ext| {
            if !floppy_extensions.contains(&ext.to_string()) {
                floppy_extensions.push(ext.to_string());
            }
        });
        // Update the floppy manager with the extension list
        floppy_manager.set_extensions(Some(floppy_extensions));
        // Scan the "floppy" resource
        floppy_manager.scan_resource(&mut resource_manager)?;
        log::debug!("Floppy resource scan complete!");
        // Scan the "autofloppy" resource
        floppy_manager.scan_autofloppy(&mut resource_manager)?;

        // Instantiate the VHD manager
        log::debug!("Creating VhdManager...");
        let mut vhd_manager = VhdManager::new();
        // Scan the 'hdd' resource
        vhd_manager.scan_resource(&mut resource_manager)?;

        // Instantiate the cartridge manager
        log::debug!("Creating CartridgeManager...");
        let mut cart_manager = CartridgeManager::new();
        // Scan the 'cartridge' resource
        cart_manager.scan_resource(&mut resource_manager)?;

        // Enumerate the host's serial ports if the feature is enabled
        #[cfg(feature = "use_serialport")]
        let serial_ports = {
            let ports = serialport::available_ports().unwrap_or_else(|e| {
                log::warn!("Didn't find any serial ports: {:?}", e);
                Vec::new()
            });

            for port in &ports {
                log::debug!("Found serial port: {:?}", port);
            }
            ports
        };

        // If headless mode was specified, we would try to run headless mode here.
        // I'm not sure how to implement this yet, so for now we'll just print an error and exit.
        // A true headless mode is probably best implemented as a separate front-end.
        if config.emulator.headless {
            let err_str = "Headless mode requested, but not implemented".to_string();
            return Err(UnsupportedPlatform(err_str));
        }

        // ----------------------------------------------------------------------------------------
        // From this point forward, it is assumed we are starting a graphical frontend
        // This may or may not have an internal GUI depending on the platform and features
        // ----------------------------------------------------------------------------------------

        // Instantiate hotkey manager
        let mut hotkey_manager = HotkeyManager::new();
        hotkey_manager.add_hotkeys(config.emulator.input.hotkeys.clone());

        // ExecutionControl is shared via RefCell with GUI so that state can be updated by control widget
        let exec_control = Rc::new(RefCell::new(ExecutionControl::new()));

        // Set CPU state to Running if cpu_autostart option was set in config
        if config.emulator.cpu_autostart {
            exec_control.borrow_mut().set_state(ExecutionState::Running);
        }

        // Initialize input device state.
        let kb_data = KeyboardData::new();
        let mouse_data = MouseData::new(config.emulator.input.reverse_mouse_buttons);
        log::debug!(
            "Reverse mouse buttons is: {}",
            config.emulator.input.reverse_mouse_buttons
        );
        let joy_data = JoystickData::new(
            config.emulator.input.joystick_keys.clone(),
            config.emulator.input.keyboard_joystick,
        );

        // Make a statistics counter
        let stat_counter = Counter::new();

        // Create a MachineConfiguration for core initialization
        let machine_config = machine_config_file.to_machine_config();

        let trace_file_base = resource_manager.resource_path("trace").unwrap_or_default();
        let mut trace_file_path = None;
        if let Some(trace_file) = &config.machine.cpu.trace_file {
            stdout.write_fmt(format_args!("Using CPU trace log file: {:?}", trace_file))?;
            trace_file_path = Some(trace_file_base.join(trace_file));
        }

        // Calculate the path to the keyboard layout file
        let mut kb_layout_file_path = None;
        let mut kb_string = "US".to_string();

        if let Some(global_kb_string) = &config.machine.input.keyboard_layout {
            kb_string = global_kb_string.clone()
        }
        else {
            if let Some(keyboard) = machine_config.keyboard.as_ref() {
                kb_string = keyboard.layout.clone();
            }
        }

        // Get the 'keyboard_layout' resource path and append the calculated keyboard layout file name
        if let Some(mut kb_layout_resource_path) = resource_manager.resource_path("keyboard_layout") {
            kb_layout_resource_path.push(format!("keyboard_{}.toml", kb_string));
            kb_layout_file_path = Some(kb_layout_resource_path);
        }

        // Load the keyboard layout file
        let mut kb_layout = None;
        if let Some(path) = kb_layout_file_path {
            let kb_str = resource_manager.read_string_from_path(&path).await?;
            kb_layout = Some(kb_str);
        }

        // Get the file path to use for the disassembly log
        let mut disassembly_file_path = None;
        if let Some(disassembly_file) = config.machine.disassembly_file.as_ref() {
            disassembly_file_path = Some(trace_file_base.join(disassembly_file));
            stdout.write_fmt(format_args!(
                "Using disassembly log file: {:?}",
                disassembly_file_path.clone().unwrap_or(PathBuf::from("None"))
            ))?;
        }

        // Construct the core Machine instance
        log::debug!("Creating MachineBuilder...");
        let mut machine_builder = MachineBuilder::new()
            .with_core_config(Box::new(&config))
            .with_machine_config(&machine_config)
            .with_roms(rom_manifest)
            .with_trace_mode(config.machine.cpu.trace_mode.unwrap_or_default())
            .with_trace_log(trace_file_path)
            .with_keyboard_layout(kb_layout)
            .with_listing_file(disassembly_file_path);

        #[cfg(feature = "sound")]
        {
            log::debug!("Sound is enabled. Adding sound configuration to MachineBuilder...");
            machine_builder = machine_builder.with_sound_config(sound_config);
        }

        // Build the Machine instance
        log::debug!("Building Machine...");
        let machine = machine_builder.build()?;

        // Now that we have a Machine, we can query it for sound sources (devices that produce sound)
        // For each sound source we will create a source in the SoundInterface, to give it
        // volume/mute controls.
        #[cfg(feature = "sound")]
        {
            let sound_sources = machine.get_sound_sources();

            // If we have a SoundInterface, create player resources for each machine source
            if let Some(si) = sound_player.as_mut() {
                log::debug!("Machine configuration reported {} sound sources", sound_sources.len());
                for source in sound_sources.iter() {
                    log::debug!("Adding sound source: {}", source.name);
                    if let Err(e) = si.add_source(source) {
                        log::error!("Failed to add sound source: {:?}", e);
                        std::process::exit(1);
                    }
                }

                // PC Speaker is always first sound source. Set its volume to 25%.
                si.set_volume(0, 0.25);
            }
        }

        // A DisplayManager is front-end specific, so we'll expect the front-end to create one
        // after we have built the emulator.

        // Create a channel for receiving thread events (File open requests, etc.)
        let (sender, receiver) = crossbeam_channel::unbounded();

        // Create a GUI state object
        let mut gui = GuiState::new(exec_control.clone(), sender.clone());

        // Set list of virtual serial ports
        gui.set_serial_ports(machine.bus().enumerate_serial_ports());

        // Set floppy drives.
        let drive_ct = machine.bus().floppy_drive_ct();
        let mut drive_types = Vec::new();
        for i in 0..drive_ct {
            if let Some(fdc) = machine.bus().fdc() {
                drive_types.push(fdc.drive(i).get_type());
            }
        }

        gui.set_floppy_drives(drive_types);

        // Set default floppy path. This is used to set the default path for Save As dialogs.
        gui.set_paths(resource_manager.resource_path("floppy").unwrap());

        // Set hard drives.
        gui.set_hdds(machine.bus().hdd_ct());

        // Set cartridge slots
        gui.set_cart_slots(machine.bus().cart_ct());

        // Set autofloppy paths
        #[cfg(not(target_arch = "wasm32"))]
        {
            gui.set_autofloppy_paths(floppy_manager.get_autofloppy_paths());
        }

        // Request initial events from GUI.
        gui.initialize();

        // Create a queue for machine events.
        // TODO: This should probably be converted into a channel
        let machine_events = Vec::new();

        Ok(Emulator {
            rm: resource_manager,
            romm: rom_manager,
            romsets: rom_sets_resolved.clone(),
            config,
            machine,
            machine_events,
            exec_control,
            mouse_data,
            kb_data,
            joy_data,
            stat_counter,
            gui,
            floppy_manager,
            vhd_manager,
            cart_manager,
            perf: Default::default(),
            flags: EmuFlags {
                render_gui: self.enable_gui,
                debug_keyboard: false,
            },
            hkm: hotkey_manager,
            si: sound_player,
            sender,
            receiver,
        })
    }
}

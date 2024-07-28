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

    lib.rs

    MartyPC Desktop front-end main library component.

    MartyPC Desktop includes the full GUI and debugger interface.

*/

#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::too_many_arguments)]
#![forbid(unsafe_code)]

mod cpu_test;
mod emulator;
mod event_loop;
mod input;
mod run_benchmark;
mod run_headless;
mod sound_player;

#[cfg(feature = "arduino_validator")]
mod run_fuzzer;

use rodio::cpal::traits::HostTrait;
use std::{
    cell::RefCell,
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

use crate::run_benchmark::run_benchmark;

#[cfg(feature = "arduino_validator")]
use crate::{cpu_test::gen_tests::run_gentests, cpu_test::process_tests::run_processtests, run_fuzzer::run_fuzzer};

#[cfg(feature = "cpu_validator")]
use crate::cpu_test::run_tests::run_runtests;
#[cfg(feature = "cpu_validator")]
use marty_core::cpu_validator::ValidatorType;

use config_toml_bpaf::TestMode;

use marty_core::{
    devices::keyboard::KeyboardModifiers,
    machine::{ExecutionControl, ExecutionState, MachineBuilder},
};

use display_manager_wgpu::{DisplayBackend, DisplayManager, DisplayManagerGuiOptions, WgpuDisplayManagerBuilder};
use frontend_common::{
    cartridge_manager::CartridgeManager,
    floppy_manager::FloppyManager,
    resource_manager::ResourceManager,
    timestep_manager::TimestepManager,
    types::joykeys::JoyKeyInput,
    vhd_manager::VhdManager,
    JoyKeyEntry,
};
use marty_core::keys::MartyKey;
use marty_egui::state::GuiState;

use crate::{
    emulator::{EmuFlags, Emulator},
    event_loop::handle_event,
    input::HotkeyManager,
    sound_player::SoundInterface,
};

pub const FPS_TARGET: f64 = 60.0;

// Embed default icon
const MARTY_ICON: &[u8] = include_bytes!("../../../assets/martypc_icon_small.png");

// Rendering Stats
pub struct Counter {
    pub frame_count: u64,
    pub cycle_count: u64,
    pub instr_count: u64,

    pub current_ups: u32,
    pub current_cps: u64,
    pub current_fps: u32,
    pub current_ips: u64,
    pub emulated_fps: u32,
    pub current_emulated_frames: u64,
    pub emulated_frames: u64,

    pub ups: u32,
    pub fps: u32,
    pub last_frame: Instant,
    #[allow(dead_code)]
    pub last_sndbuf: Instant,
    pub last_second: Instant,
    pub last_cpu_cycles: u64,
    pub current_cpu_cps: u64,
    pub last_system_ticks: u64,
    pub last_pit_ticks: u64,
    pub current_sys_tps: u64,
    pub current_pit_tps: u64,
    pub emulation_time: Duration,
    pub render_time: Duration,
    pub accumulated_us: u128,
    pub cpu_mhz: f64,
    pub cycles_per_frame: u32,
    pub cycle_target: u32,
}

impl Counter {
    fn new() -> Self {
        Self {
            frame_count: 0,
            cycle_count: 0,
            instr_count: 0,

            current_ups: 0,
            current_cps: 0,
            current_fps: 0,
            current_ips: 0,

            emulated_fps: 0,
            current_emulated_frames: 0,
            emulated_frames: 0,

            ups: 0,
            fps: 0,
            last_second: Instant::now(),
            last_sndbuf: Instant::now(),
            last_frame: Instant::now(),
            last_cpu_cycles: 0,
            current_cpu_cps: 0,
            last_system_ticks: 0,
            last_pit_ticks: 0,
            current_sys_tps: 0,
            current_pit_tps: 0,
            emulation_time: Duration::ZERO,
            render_time: Duration::ZERO,
            accumulated_us: 0,
            cpu_mhz: 0.0,
            cycles_per_frame: 0,
            cycle_target: 0,
        }
    }
}

#[allow(dead_code)]
pub struct MouseData {
    pub reverse_buttons: bool,
    pub l_button_id: u32,
    pub r_button_id: u32,
    pub is_captured: bool,
    pub have_update: bool,
    pub l_button_was_pressed: bool,
    pub l_button_was_released: bool,
    pub l_button_is_pressed: bool,
    pub r_button_was_pressed: bool,
    pub r_button_was_released: bool,
    pub r_button_is_pressed: bool,
    pub frame_delta_x: f64,
    pub frame_delta_y: f64,
}

impl MouseData {
    fn new(reverse_buttons: bool) -> Self {
        Self {
            reverse_buttons,
            l_button_id: input::get_mouse_buttons(reverse_buttons).0,
            r_button_id: input::get_mouse_buttons(reverse_buttons).1,
            is_captured: false,
            have_update: false,
            l_button_was_pressed: false,
            l_button_was_released: false,
            l_button_is_pressed: false,
            r_button_was_pressed: false,
            r_button_was_released: false,
            r_button_is_pressed: false,
            frame_delta_x: 0.0,
            frame_delta_y: 0.0,
        }
    }
    pub fn reset(&mut self) {
        if !self.l_button_is_pressed {
            self.l_button_was_pressed = false;
        }
        if !self.r_button_is_pressed {
            self.r_button_was_pressed = false;
        }

        self.l_button_was_released = false;
        self.r_button_was_released = false;

        self.frame_delta_x = 0.0;
        self.frame_delta_y = 0.0;
        self.have_update = false;
    }
}

/// This structure is only used to maintain the state for keyboard joystick emulation.
/// Actual joysticks will be read directly via a controller input library.
#[allow(dead_code)]
#[derive(Default)]
pub struct JoystickData {
    pub enabled:   bool,
    pub key_state: HashMap<MartyKey, (JoyKeyInput, bool)>,
    pub joy_state: HashMap<JoyKeyInput, bool>,
}
impl JoystickData {
    fn new(keys: Vec<JoyKeyEntry>, enabled: bool) -> Self {
        let mut jd = JoystickData::default();

        for key in keys {
            jd.key_state.insert(key.key, (key.input, false));
            jd.joy_state.insert(key.input, false);
        }
        jd.enabled = enabled;
        jd
    }

    fn get_xy(&self) -> (f64, f64) {
        let x = if *self.joy_state.get(&JoyKeyInput::JoyLeft).unwrap() {
            -1.0
        }
        else if *self.joy_state.get(&JoyKeyInput::JoyRight).unwrap() {
            1.0
        }
        else {
            0.0
        };
        let y = if *self.joy_state.get(&JoyKeyInput::JoyUp).unwrap() {
            -1.0
        }
        else if *self.joy_state.get(&JoyKeyInput::JoyDown).unwrap() {
            1.0
        }
        else {
            0.0
        };

        //log::debug!("Joystick XY: ({}, {})", x, y);
        (x, y)
    }
}

pub struct KeyboardData {
    pub modifiers:    KeyboardModifiers,
    pub ctrl_pressed: bool,
}
impl KeyboardData {
    fn new() -> Self {
        Self {
            modifiers:    KeyboardModifiers::default(),
            ctrl_pressed: false,
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    // Dummy main for wasm32 target
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run() {
    env_logger::init();

    // TODO: Move most of everything from here into an EmulatorBuilder

    // First we resolve the emulator configuration by parsing the configuration toml and merging it with
    // command line arguments. For the desktop frontend, this is handled by the config_toml_bpaf front end
    // library.
    let config = match config_toml_bpaf::get_config("./martypc.toml") {
        Ok(config) => config,
        Err(e) => match e.downcast_ref::<std::io::Error>() {
            Some(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!(
                    "Configuration file not found! Please create martypc.toml in the emulator directory \
                               or provide the path to configuration file with --configfile."
                );

                std::process::exit(1);
            }
            Some(e) => {
                eprintln!("Unknown IO error reading configuration file:\n{}", e);
                std::process::exit(1);
            }
            None => {
                eprintln!(
                    "Failed to parse configuration file. There may be a typo or otherwise invalid toml:\n{}",
                    e
                );
                std::process::exit(1);
            }
        },
    };

    // Now that we have our configuration, we can instantiate a ResourceManager.
    let mut resource_manager = ResourceManager::from_config(config.emulator.basedir.clone(), &config.emulator.paths)
        .unwrap_or_else(|e| {
            log::error!("Failed to create resource manager: {:?}", e);
            std::process::exit(1);
        });

    let resolved_paths = resource_manager.pm.dump_paths();
    for path in &resolved_paths {
        println!("Resolved resource path: {:?}", path);
    }

    // Tell the resource manager to ignore specified dirs
    if let Some(ignore_dirs) = &config.emulator.ignore_dirs {
        resource_manager.set_ignore_dirs(ignore_dirs.clone());
    }

    #[cfg(feature = "cpu_validator")]
    match config.validator.vtype {
        Some(ValidatorType::None) | None => {
            eprintln!("Compiled with validator but no validator specified");
            std::process::exit(1);
        }
        _ => {}
    }

    // Instantiate the new machine manager to load Machine configurations.
    let mut machine_manager = frontend_common::machine_manager::MachineManager::new();
    if let Err(err) = machine_manager.load_configs(&resource_manager) {
        eprintln!("Error loading Machine configuration files: {}", err);
        std::process::exit(1);
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

        println!(
            "Benchmark mode enabled. Using machine config: {} config overlays: [{}] prefer_oem: {}",
            init_config_name,
            init_config_overlays.join(", "),
            init_prefer_oem
        );
    }

    // Get a list of machine configuration names
    let machine_names = machine_manager.get_config_names();
    let have_machine_config = machine_names.contains(&init_config_name);

    // Do --machinescan commandline argument. We print machine info (and rom info if --romscan
    // was also specified) and then quit.
    if config.emulator.machinescan {
        // Print the list of machine configurations and their rom requirements
        for machine in machine_names {
            println!("Machine: {}", machine);
            if let Some(reqs) = machine_manager
                .get_config(&machine)
                .and_then(|config| Some(config.get_rom_requirements()))
            {
                println!("  Requires: {:?}", reqs);
            }
        }

        if !have_machine_config {
            println!("Warning! No matching configuration found for: {}", init_config_name);
            std::process::exit(1);
        }

        // Exit unless we will also run romscan
        if !config.emulator.romscan {
            std::process::exit(0);
        }
    }

    if !have_machine_config {
        eprintln!(
            "No machine configuration for specified config name: {}",
            init_config_name
        );
        std::process::exit(1);
    }

    // Instantiate the new rom manager to load roms
    let mut rom_manager = frontend_common::rom_manager::RomManager::new(init_prefer_oem);
    if let Err(err) = rom_manager.load_defs(&resource_manager) {
        eprintln!("Error loading ROM definition files: {}", err);
        std::process::exit(1);
    }

    // Get the ROM requirements for the requested machine type
    let machine_config_file = {
        for overlay in init_config_overlays.iter() {
            log::debug!("Have machine config overlay from global config: {}", overlay);
        }
        let overlay_vec = init_config_overlays.clone();

        match machine_manager.get_config_with_overlays(&init_config_name, &overlay_vec) {
            Ok(config) => config,
            Err(err) => {
                eprintln!("Error getting machine config: {}", err);
                std::process::exit(1);
            }
        }
    };
    let (required_features, optional_features) = machine_config_file.get_rom_requirements().unwrap_or_else(|e| {
        eprintln!("Error getting ROM requirements for machine: {}", e);
        std::process::exit(1);
    });

    // Scan the rom resource director(ies)
    if let Err(err) = rom_manager.scan(&resource_manager) {
        eprintln!("Error scanning ROM resource directories: {}", err);
        std::process::exit(1);
    }

    // Determine what complete ROM sets we have
    if let Err(err) = rom_manager.resolve_rom_sets() {
        eprintln!("Error resolving ROM sets: {}", err);
        std::process::exit(1);
    }

    // Do --romscan option.  We print rom and machine info and quit.
    if config.emulator.romscan {
        rom_manager.print_rom_stats();
        rom_manager.print_romset_stats();
        std::process::exit(0);
    }

    println!(
        "Selected machine config {} requires the following ROM features:",
        init_config_name
    );
    for rom_feature in &required_features {
        println!("  {}", rom_feature);
    }

    println!(
        "Selected machine config {} optionally requests the following ROM features:",
        init_config_name
    );
    for rom_feature in &optional_features {
        println!("  {}", rom_feature);
    }

    // Determine if the machine configuration specifies a particular ROM set
    let specified_rom_set = machine_config_file.get_specified_rom_set();

    // Resolve the ROM requirements for the requested ROM features
    let rom_sets_resolved = rom_manager
        .resolve_requirements(required_features, optional_features, specified_rom_set)
        .unwrap_or_else(|err| {
            eprintln!("Error resolving ROM sets for machine: {}", err);
            std::process::exit(1);
        });

    println!(
        "Selected machine config {} has resolved the following ROM sets:",
        init_config_name
    );
    for rom_set in &rom_sets_resolved {
        println!("  {}", rom_set);
    }

    // Create the ROM manifest
    let rom_manifest = rom_manager
        .create_manifest(rom_sets_resolved.clone(), &resource_manager)
        .unwrap_or_else(|err| {
            eprintln!("Error loading ROM set: {}", err);
            std::process::exit(1);
        });

    log::debug!("Created manifest!");
    for (i, rom) in rom_manifest.roms.iter().enumerate() {
        log::debug!("  rom {}: md5: {} length: {}", i, rom.md5, rom.data.len());
    }

    // Instantiate the floppy manager
    let mut floppy_manager = FloppyManager::new();

    floppy_manager.set_extensions(config.emulator.media.raw_sector_image_extensions.clone());

    // Scan the "floppy" resource
    if let Err(e) = floppy_manager.scan_resource(&resource_manager) {
        eprintln!("Failed to read floppy path: {:?}", e);
        std::process::exit(1);
    }

    // Scan the "autofloppy" resource
    if let Err(e) = floppy_manager.scan_autofloppy(&resource_manager) {
        eprintln!("Failed to read autofloppy path: {:?}", e);
        std::process::exit(1);
    }

    // Instantiate the VHD manager
    let mut vhd_manager = VhdManager::new();

    // Scan the "hdd" resource
    if let Err(e) = vhd_manager.scan_resource(&resource_manager) {
        eprintln!("Failed to read hdd path: {:?}", e);
        std::process::exit(1);
    }

    // Instantiate the cartridge manager
    let mut cart_manager = CartridgeManager::new();

    // Scan the "cartridge" resource
    if let Err(e) = cart_manager.scan_resource(&resource_manager) {
        eprintln!("Failed to read cartridge path: {:?}", e);
        std::process::exit(1);
    }

    // Enumerate host serial ports
    let serial_ports = serialport::available_ports().unwrap_or_else(|e| {
        log::warn!("Didn't find any serial ports: {:?}", e);
        Vec::new()
    });

    for port in &serial_ports {
        log::debug!("Found serial port: {:?}", port);
    }

    log::debug!("Test mode: {:?}", config.tests.test_mode);

    // If fuzzer mode was specified, run the emulator in fuzzer mode now
    #[cfg(feature = "arduino_validator")]
    if config.emulator.fuzzer {
        return run_fuzzer(&config);
    }

    // If test generate mode was specified, run the emulator in test generation mode now
    #[cfg(feature = "cpu_validator")]
    match config.tests.test_mode {
        #[cfg(feature = "arduino_validator")]
        Some(TestMode::Generate) => return run_gentests(&config),
        #[cfg(not(feature = "arduino_validator"))]
        Some(TestMode::Generate) => panic!("Test generation not supported without a validator backend."),
        Some(TestMode::Run) | Some(TestMode::Validate) => return run_runtests(config),
        #[cfg(feature = "arduino_validator")]
        Some(TestMode::Process) => return run_processtests(config),
        Some(TestMode::None) | None | _ => {}
    }
    #[cfg(not(feature = "cpu_validator"))]
    {
        if !matches!(config.tests.test_mode, None | Some(TestMode::None)) {
            eprintln!("Test mode not supported without validator feature.");
            std::process::exit(1);
        }
    }

    if config.emulator.benchmark_mode {
        return run_benchmark(
            &config,
            machine_config_file,
            rom_manifest,
            resource_manager,
            rom_manager,
            floppy_manager,
        );
    }

    // If headless mode was specified, run the emulator in headless mode now
    if config.emulator.headless {
        //return run_headless::run_headless(&config, rom_manager, floppy_manager);
    }

    // ----------------------------------------------------------------------------
    // From this point forward, it is assumed we are staring the full GUI frontend!
    // ----------------------------------------------------------------------------

    let mut hotkey_manager = HotkeyManager::new();
    hotkey_manager.add_hotkeys(config.emulator.input.hotkeys.clone());

    // ExecutionControl is shared via RefCell with GUI so that state can be updated by control widget
    let exec_control = Rc::new(RefCell::new(ExecutionControl::new()));

    // Set CPU state to Running if cpu_autostart option was set in config
    if config.emulator.cpu_autostart {
        exec_control.borrow_mut().set_state(ExecutionState::Running);
    }

    let stat_counter = Counter::new();

    // KB modifiers
    let kb_data = KeyboardData::new();

    // Mouse event struct
    let mouse_data = MouseData::new(config.emulator.input.reverse_mouse_buttons);
    log::debug!(
        "Reverse mouse buttons is: {}",
        config.emulator.input.reverse_mouse_buttons
    );

    let mut sound_config = Default::default();
    let mut sound_player = if config.emulator.audio.enabled {
        let mut sound_player = SoundInterface::new(config.emulator.audio.enabled);

        match sound_player.open_device() {
            Ok(_) => {
                log::info!("Opened audio device: {}", sound_player.device_name());
            }
            Err(e) => {
                eprintln!("Failed to open audio device: {:?}", e);
                std::process::exit(1);
            }
        }

        match sound_player.open_stream() {
            Ok(_) => {
                log::info!("Opened audio stream.");
            }
            Err(e) => {
                eprintln!("Failed to open audio stream: {:?}", e);
                std::process::exit(1);
            }
        }

        sound_config = sound_player.config();
        Some(sound_player)
    }
    else {
        None
    };

    // Init sound
    /*
    let sound_player_opt = {
        if config.emulator.audio.enabled {
            // The cpal sound library uses generics to initialize depending on the SampleFormat type.
            // On Windows at least a sample type of f32 is typical, but just in case...
            let (audio_device, sample_fmt) = SoundPlayer::get_device();
            let sp = match sample_fmt {
                cpal::SampleFormat::F32 => SoundPlayer::new::<f32>(audio_device),
                cpal::SampleFormat::I16 => SoundPlayer::new::<i16>(audio_device),
                cpal::SampleFormat::U16 => SoundPlayer::new::<u16>(audio_device),
            };
            Some(sp)
        }
        else {
            None
        }
    };*/

    let machine_config = machine_config_file.to_machine_config();

    let trace_file_base = resource_manager.get_resource_path("trace").unwrap_or_else(|| {
        eprintln!("Failed to retrieve 'trace' resource path.");
        std::process::exit(1);
    });

    let mut trace_file_path = None;
    if let Some(trace_file) = &config.machine.cpu.trace_file {
        log::info!("Using CPU trace log file: {:?}", trace_file);
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

    if let Some(mut kb_layout_resource_path) = resource_manager.get_resource_path("keyboard_layout") {
        kb_layout_resource_path.push(format!("keyboard_{}.toml", kb_string));
        kb_layout_file_path = Some(kb_layout_resource_path);
    }

    let mut disassembly_file_path = None;
    if let Some(disassembly_file) = config.machine.disassembly_file.as_ref() {
        disassembly_file_path = Some(trace_file_base.join(disassembly_file));
        log::info!(
            "Using disassembly file: {:?}",
            disassembly_file_path.clone().unwrap_or(PathBuf::from("None"))
        );
    }

    let machine_builder = MachineBuilder::new()
        .with_core_config(Box::new(&config))
        .with_machine_config(&machine_config)
        .with_roms(rom_manifest)
        .with_trace_mode(config.machine.cpu.trace_mode.unwrap_or_default())
        .with_trace_log(trace_file_path)
        .with_sound_config(sound_config)
        .with_keyboard_layout(kb_layout_file_path)
        .with_listing_file(disassembly_file_path);

    let machine = machine_builder.build().unwrap_or_else(|e| {
        log::error!("Failed to build machine: {:?}", e);
        std::process::exit(1);
    });

    let sound_sources = machine.get_sound_sources();

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

    // Get a list of video devices from machine.
    let cardlist = machine.bus().enumerate_videocards();

    let mut highest_rate = 50;
    for card in cardlist.iter() {
        let rate = machine.bus().video(&card).unwrap().get_refresh_rate();
        if rate > highest_rate {
            highest_rate = rate;
        }
    }

    // Create Timestep Manager
    let mut timestep_manager = TimestepManager::new();
    timestep_manager.set_cpu_mhz(machine.get_cpu_mhz());
    timestep_manager.set_emu_update_rate(highest_rate);
    timestep_manager.set_emu_render_rate(highest_rate);

    let gui_options = DisplayManagerGuiOptions {
        enabled: !config.gui.disabled,
        theme: config.gui.theme,
        menu_theme: config.gui.menu_theme,
        menubar_h: 24, // TODO: Dynamically measure the height of the egui menu bar somehow
        zoom: config.gui.zoom.unwrap_or(1.0),
        debug_drawing: false,
    };

    // Create displays.
    let mut display_manager = WgpuDisplayManagerBuilder::build(
        &config,
        cardlist,
        &config.emulator.scaler_preset,
        None,
        Some(MARTY_ICON),
        &gui_options,
    )
    .unwrap_or_else(|e| {
        log::error!("Failed to create displays: {:?}", e);
        std::process::exit(1);
    });

    // Create joystick data
    let joy_data = JoystickData::new(
        config.emulator.input.joystick_keys.clone(),
        config.emulator.input.keyboard_joystick,
    );

    // Create GUI state
    let render_egui = true;
    let gui = GuiState::new(exec_control.clone());

    // Get main GUI context from Display Manager
    let _gui_ctx = display_manager
        .get_main_gui_mut()
        .expect("Couldn't get main gui context!");

    let machine_events = Vec::new();

    // Put everything we want to handle in event loop into an Emulator struct
    let mut emu = Emulator {
        rm: resource_manager,
        dm: display_manager,
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
            render_gui: render_egui,
            debug_keyboard: false,
        },
        hkm: hotkey_manager,
        si: sound_player,
    };

    // Resize video cards
    emu.post_dm_build_init();

    // Set list of host serial ports
    emu.gui.set_host_serial_ports(serial_ports);

    let adapter_info = emu.dm.get_main_backend().and_then(|backend| backend.get_adapter_info());

    let (backend_str, adapter_name_str) = {
        let backend_str;
        let adapter_name_str;

        if let Some(adapter_info) = adapter_info {
            backend_str = format!("{:?}", adapter_info.backend);
            adapter_name_str = format!("{}", adapter_info.name);
            (backend_str, adapter_name_str)
        }
        else {
            log::error!("Failed to get adapter info from backend.");
            std::process::exit(1);
        }
    };

    if adapter_name_str.contains("llvmpipe") {
        emu.gui.show_warning(
            &"MartyPC was unable to initialize a hardware accellerated backend.\n\
                MartyPC is running under software rasterization (llvmpipe).\n\
                Performance will be poor. (Do you have libx11-dev installed?)"
                .to_string(),
        );
    }

    log::debug!("wgpu using adapter: {}, backend: {}", adapter_name_str, backend_str);

    if let Err(e) = emu.apply_config() {
        log::error!("Failed to apply configuration to Emulator state: {}", e);
        std::process::exit(1);
    }

    if let Err(_e) = emu.mount_vhds() {
        log::error!("Failed to mount VHDs!");
        std::process::exit(1);
    }

    // Start emulator
    emu.start();

    let event_loop = emu.dm.take_event_loop();

    // Run the winit event loop
    if let Err(_e) = event_loop.run(move |event, elwt| {
        handle_event(&mut emu, &mut timestep_manager, event, elwt);
    }) {
        log::error!("Failed to start event loop!");
        std::process::exit(1);
    }

    std::process::exit(0);
}

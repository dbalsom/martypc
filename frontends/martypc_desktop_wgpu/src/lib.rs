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
mod run_headless;

#[cfg(feature = "arduino_validator")]
mod run_fuzzer;

use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

#[cfg(feature = "arduino_validator")]
use crate::{cpu_test::gen_tests::run_gentests, cpu_test::run_tests, run_fuzzer::run_fuzzer};

use config_toml_bpaf::TestMode;

use marty_core::{
    cpu_validator::ValidatorType,
    devices::keyboard::KeyboardModifiers,
    machine::{ExecutionControl, ExecutionState, MachineBuilder},
    sound::SoundPlayer,
};

use display_manager_wgpu::{DisplayBackend, DisplayManager, DisplayManagerGuiOptions, WgpuDisplayManagerBuilder};
use frontend_common::{
    floppy_manager::FloppyManager,
    resource_manager::ResourceManager,
    timestep_manager::TimestepManager,
    vhd_manager::VhdManager,
};
use marty_egui::state::GuiState;

#[cfg(feature = "cpu_validator")]
use run_tests::run_runtests;

use crate::{
    cpu_test::process_tests::run_processtests,
    emulator::{EmuFlags, Emulator},
    event_loop::handle_event,
};

pub const FPS_TARGET: f64 = 60.0;
const MICROS_PER_FRAME: f64 = 1.0 / FPS_TARGET * 1000000.0;

// Remove static frequency references
//const CYCLES_PER_FRAME: u32 = (cpu_808x::CPU_MHZ * 1000000.0 / FPS_TARGET) as u32;

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

    // Get a list of machine configuration names
    let machine_names = machine_manager.get_config_names();
    let have_machine_config = machine_names.contains(&config.machine.config_name);

    // Do 'romscan' commandline argument. We print machine info (and rom info if --romscan
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
            println!(
                "Warning! No matching configuration found for: {}",
                config.machine.config_name
            );
        }

        // Exit unless we will also run romscan
        if !config.emulator.romscan {
            std::process::exit(0);
        }
    }

    if !have_machine_config {
        eprintln!(
            "No machine configuration for specified config name: {}",
            config.machine.config_name
        );
        std::process::exit(1);
    }

    // Instantiate the new rom manager to load roms
    let mut rom_manager = frontend_common::rom_manager::RomManager::new(config.machine.prefer_oem);
    if let Err(err) = rom_manager.load_defs(&resource_manager) {
        eprintln!("Error loading ROM definition files: {}", err);
        std::process::exit(1);
    }

    // Get the ROM requirements for the requested machine type
    let machine_config_file = {
        if let Some(overlay_vec) = &config.machine.config_overlays {
            for overlay in overlay_vec.iter() {
                log::debug!("Have machine config overlay: {}", overlay);
            }
            machine_manager
                .get_config_with_overlays(&config.machine.config_name, overlay_vec)
                .unwrap()
        }
        else {
            machine_manager.get_config(&config.machine.config_name).unwrap()
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
        config.machine.config_name
    );
    for rom_feature in &required_features {
        println!("  {}", rom_feature);
    }

    println!(
        "Selected machine config {} optionally requests the following ROM features:",
        config.machine.config_name
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
        config.machine.config_name
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

    // Instantiate the VHD manager
    let mut vhd_manager = VhdManager::new();

    // Scan the "hdd" resource
    if let Err(e) = vhd_manager.scan_resource(&resource_manager) {
        eprintln!("Failed to read hdd path: {:?}", e);
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
    #[cfg(feature = "cpu_validator")]
    if config.emulator.fuzzer {
        return run_fuzzer(&config);
    }

    // If test generate mode was specified, run the emulator in test generation mode now
    #[cfg(feature = "cpu_validator")]
    match config.tests.test_mode {
        Some(TestMode::Generate) => return run_gentests(&config),
        Some(TestMode::Run) | Some(TestMode::Validate) => return run_runtests(config),
        Some(TestMode::Process) => return run_processtests(config),
        Some(TestMode::None) | None => {}
    }
    #[cfg(not(feature = "cpu_validator"))]
    {
        if let Some(TestMode::Run) = config.tests.test_mode {
            eprintln!("Test generation mode not supported in this build.");
            std::process::exit(1);
        }
    }

    // If headless mode was specified, run the emulator in headless mode now
    if config.emulator.headless {
        //return run_headless::run_headless(&config, rom_manager, floppy_manager);
    }

    // ExecutionControl is shared via RefCell with GUI so that state can be updated by control widget
    let exec_control = Rc::new(RefCell::new(ExecutionControl::new()));

    // Set CPU state to Running if cpu_autostart option was set in config
    if config.emulator.cpu_autostart {
        exec_control.borrow_mut().set_state(ExecutionState::Running);
    }

    // Create the logical GUI.
    let _gui = GuiState::new(exec_control.clone());

    let stat_counter = Counter::new();

    // KB modifiers
    let kb_data = KeyboardData::new();

    // Mouse event struct
    let mouse_data = MouseData::new(config.input.reverse_mouse_buttons);

    // Init sound
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
    };

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

    let machine_builder = MachineBuilder::new()
        .with_core_config(Box::new(&config))
        .with_machine_config(&machine_config)
        .with_roms(rom_manifest)
        .with_trace_mode(config.machine.cpu.trace_mode.unwrap_or_default())
        .with_trace_log(trace_file_path)
        .with_sound_player(sound_player_opt);

    let machine = machine_builder.build().unwrap_or_else(|e| {
        log::error!("Failed to build machine: {:?}", e);
        std::process::exit(1);
    });

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
        stat_counter,
        gui,
        floppy_manager,
        vhd_manager,
        perf: Default::default(),
        flags: EmuFlags {
            render_gui: render_egui,
            debug_keyboard: false,
        },
    };

    // Resize video cards
    emu.post_dm_build_init();

    // Set list of serial ports
    emu.gui.update_serial_ports(serial_ports);

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

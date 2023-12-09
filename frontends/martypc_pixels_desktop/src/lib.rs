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

    lib.rs

    MartyPC Desktop front-end main library component.

    MartyPC Desktop includes the full GUI and debugger interface.

*/

#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::too_many_arguments)]
#![forbid(unsafe_code)]

use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

mod cpu_test;
mod emulator;
mod event_loop;
mod input;
#[cfg(feature = "arduino_validator")]
mod run_fuzzer;
#[cfg(feature = "arduino_validator")]
mod run_gentests;
#[cfg(feature = "arduino_validator")]
mod run_runtests;

mod run_headless;
mod run_processtests;

use crate::emulator::{EmuFlags, Emulator};

use marty_egui::GuiState;

#[cfg(feature = "arduino_validator")]
use crate::run_fuzzer::run_fuzzer;

#[cfg(feature = "arduino_validator")]
use crate::run_gentests::run_gentests;

#[cfg(feature = "arduino_validator")]
use crate::run_runtests::run_runtests;

use marty_core::{
    devices::{hdc::HardDiskControllerType, keyboard::KeyboardModifiers},
    floppy_manager::{FloppyError, FloppyManager},
    machine::{ExecutionControl, ExecutionState, Machine},
    machine_manager::MACHINE_DESCS,
    rom_manager::{RomError, RomFeature, RomManager},
    sound::SoundPlayer,
    vhd_manager::{VHDManager, VHDManagerError},
    videocard::{ClockingMode, VideoType},
};

use display_manager_wgpu::{DisplayBackend, DisplayManager, DisplayManagerGuiOptions, WgpuDisplayManagerBuilder};
use marty_core::coreconfig::CoreConfig;

use crate::event_loop::handle_event;

const EGUI_MENU_BAR: u32 = 25;

const WINDOW_MIN_WIDTH: u32 = 640;
const WINDOW_MIN_HEIGHT: u32 = 480;

const WINDOW_WIDTH: u32 = WINDOW_MIN_WIDTH;
const WINDOW_HEIGHT: u32 = WINDOW_MIN_HEIGHT + EGUI_MENU_BAR * 2;

const MIN_RENDER_WIDTH: u32 = 160;
const MIN_RENDER_HEIGHT: u32 = 200;
//const RENDER_ASPECT: f32 = 0.75;

pub const FPS_TARGET: f64 = 60.0;
const MICROS_PER_FRAME: f64 = 1.0 / FPS_TARGET * 1000000.0;

// Remove static frequency references
//const CYCLES_PER_FRAME: u32 = (cpu_808x::CPU_MHZ * 1000000.0 / FPS_TARGET) as u32;

// Embed default icon
const MARTY_ICON: &[u8] = include_bytes!("../../../assets/martypc_icon_small.png");

// Rendering Stats
struct Counter {
    frame_count: u64,
    cycle_count: u64,
    instr_count: u64,

    current_ups: u32,
    current_cps: u64,
    current_fps: u32,
    current_ips: u64,
    emulated_fps: u32,
    current_emulated_frames: u64,
    emulated_frames: u64,

    ups: u32,
    fps: u32,
    last_frame: Instant,
    #[allow(dead_code)]
    last_sndbuf: Instant,
    last_second: Instant,
    last_cpu_cycles: u64,
    current_cpu_cps: u64,
    last_system_ticks: u64,
    last_pit_ticks: u64,
    current_sys_tps: u64,
    current_pit_tps: u64,
    emulation_time: Duration,
    render_time: Duration,
    accumulated_us: u128,
    cpu_mhz: f64,
    cycles_per_frame: u32,
    cycle_target: u32,
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
struct MouseData {
    reverse_buttons: bool,
    l_button_id: u32,
    r_button_id: u32,
    is_captured: bool,
    have_update: bool,
    l_button_was_pressed: bool,
    l_button_was_released: bool,
    l_button_is_pressed: bool,
    r_button_was_pressed: bool,
    r_button_was_released: bool,
    r_button_is_pressed: bool,
    frame_delta_x: f64,
    frame_delta_y: f64,
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

struct KeyboardData {
    modifiers:    KeyboardModifiers,
    ctrl_pressed: bool,
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

    let mut features = Vec::new();

    // Read config file
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

    let (video_type, _clock_mode, _video_debug) = {
        let mut video_type: Option<VideoType> = None;
        let mut clock_mode: Option<ClockingMode> = None;
        let mut video_debug = false;
        let video_cards = config.get_video_cards();
        if video_cards.len() > 0 {
            clock_mode = video_cards[0].clocking_mode;
            video_type = Some(video_cards[0].video_type); // Videotype is not optional
            video_debug = video_cards[0].debug.unwrap_or(false);
        }
        (video_type, clock_mode.unwrap_or_default(), video_debug)
    };

    // Determine required ROM features from configuration options
    match video_type {
        Some(VideoType::EGA) => {
            // an EGA BIOS ROM is required for EGA
            features.push(RomFeature::EGA);
        }
        Some(VideoType::VGA) => {
            // a VGA BIOS ROM is required for VGA
            features.push(RomFeature::VGA);
        }
        _ => {}
    }

    match config.machine.hdc {
        Some(HardDiskControllerType::Xebec) => {
            // The Xebec controller ROM is required for Xebec HDC
            features.push(RomFeature::XebecHDC);
        }
        _ => {}
    }

    #[cfg(feature = "cpu_validator")]
    match config.validator.vtype {
        Some(ValidatorType::None) | None => {
            eprintln!("Compiled with validator but no validator specified");
            std::process::exit(1);
        }
        _ => {}
    }

    // Instantiate the rom manager to load roms for the requested machine type
    let mut rom_manager = RomManager::new(config.machine.model, features, config.machine.rom_override.clone());

    let mut rom_path = PathBuf::new();
    rom_path.push(config.emulator.basedir.clone());
    rom_path.push("roms");

    if let Err(e) = rom_manager.try_load_from_dir(&rom_path) {
        match e {
            RomError::DirNotFound => {
                eprintln!("ROM directory not found: {}", rom_path.display())
            }
            RomError::RomNotFoundForMachine => {
                eprintln!("No valid ROM found for specified machine type.")
            }
            RomError::RomNotFoundForFeature(feature) => {
                eprintln!("No valid ROM found for requested feature: {:?}", feature)
            }
            _ => {
                eprintln!("Error loading ROM file.")
            }
        }
        std::process::exit(1);
    }

    // Verify that our ROM prerequisites are met for any machine features
    //let features = rom_manager.get_available_features();
    //
    //if let VideoType::EGA = video_type {
    //    if !features.contains(&RomFeature::EGA) {
    //        eprintln!("To enable EGA graphics, an EGA adapter ROM must be present.");
    //        std::process::exit(1);
    //    }
    //}

    // Instantiate the floppy manager
    let mut floppy_manager = FloppyManager::new();

    floppy_manager.set_extensions(config.emulator.media.raw_sector_image_extensions.clone());

    // Scan the floppy directory
    let mut floppy_path = PathBuf::new();
    floppy_path.push(config.emulator.basedir.clone());
    floppy_path.push("floppy");

    if let Err(e) = floppy_manager.scan_dir(&floppy_path) {
        match e {
            FloppyError::DirNotFound => {
                eprintln!("Floppy directory not found: {}", floppy_path.display())
            }
            _ => {
                eprintln!("Error reading floppy directory: {}", floppy_path.display())
            }
        }
        std::process::exit(1);
    }

    // Instantiate the VHD manager
    let mut vhd_manager = VHDManager::new();

    // Scan the HDD directory
    let mut hdd_path = PathBuf::new();
    hdd_path.push(config.emulator.basedir.clone());
    hdd_path.push("hdd");
    if let Err(e) = vhd_manager.scan_dir(&hdd_path) {
        match e {
            VHDManagerError::DirNotFound => {
                eprintln!("HDD directory not found")
            }
            _ => {
                eprintln!("Error reading floppy directory")
            }
        }
        std::process::exit(1);
    }

    // Enumerate host serial ports
    let serial_ports = match serialport::available_ports() {
        Ok(ports) => ports,
        Err(e) => {
            log::error!("Didn't find any serial ports: {:?}", e);
            Vec::new()
        }
    };

    for port in &serial_ports {
        log::debug!("Found serial port: {:?}", port);
    }

    log::debug!("Test mode: {:?}", config.tests.test_mode);

    // If test generate mode was specified, run the emulator in test generation mode now
    #[cfg(feature = "cpu_validator")]
    match config.tests.test_mode {
        Some(TestMode::Generate) => return run_gentests(&config),
        Some(TestMode::Run) | Some(TestMode::Validate) => return run_runtests(config),
        Some(TestMode::Process) => return run_processtests(config),
        Some(TestMode::None) | None => {}
    }

    // If fuzzer mode was specified, run the emulator in fuzzer mode now
    #[cfg(feature = "cpu_validator")]
    if config.emulator.fuzzer {
        return run_fuzzer(&config, rom_manager, floppy_manager);
    }

    // If headless mode was specified, run the emulator in headless mode now
    if config.emulator.headless {
        return run_headless::run_headless(&config, rom_manager, floppy_manager);
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
    // The cpal sound library uses generics to initialize depending on the SampleFormat type.
    // On Windows at least a sample type of f32 is typical, but just in case...
    let sample_fmt = SoundPlayer::get_sample_format();
    let sp = match sample_fmt {
        cpal::SampleFormat::F32 => SoundPlayer::new::<f32>(),
        cpal::SampleFormat::I16 => SoundPlayer::new::<i16>(),
        cpal::SampleFormat::U16 => SoundPlayer::new::<u16>(),
    };

    // Look up the machine description given the machine type in the configuration file
    let machine_desc_opt = MACHINE_DESCS.get(&config.machine.model);
    if let Some(machine_desc) = machine_desc_opt {
        log::debug!(
            "Given machine type {:?} got machine description: {:?}",
            config.machine.model,
            machine_desc
        );
    }
    else {
        log::error!("Couldn't get machine description for {:?}", config.machine.model);

        eprintln!(
            "Couldn't get machine description for machine type {:?}. \
             Check that you have a valid machine type specified in configuration file.",
            config.machine.model
        );
        std::process::exit(1);
    }

    // Instantiate the main Machine data struct
    // Machine coordinates all the parts of the emulated computer
    let mut machine = Machine::new(
        &config,
        config.machine.model,
        *machine_desc_opt.unwrap(),
        config.emulator.trace_mode.unwrap_or_default(),
        video_type.unwrap_or_default(),
        sp,
        rom_manager,
    );

    // Get a list of video devices from machine.
    let cardlist = machine.enumerate_video_cards();

    let gui_options = DisplayManagerGuiOptions {
        enabled: !config.gui.disabled,
        theme_color: config.gui.theme_color,
        theme_dark: config.gui.theme_dark,
        menubar_h: 24, // TODO: Dynamically measure the height of the egui menu bar somehow
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

    // Put everything we want to handle in event loop into an Emulator struct
    let mut emu = Emulator {
        dm: display_manager,
        config,
        machine,
        exec_control,
        mouse_data,
        kb_data,
        stat_counter,
        gui,
        floppy_manager,
        vhd_manager,
        hdd_path,
        floppy_path,
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
        handle_event(&mut emu, event, elwt);
    }) {
        log::error!("Failed to start event loop!");
        std::process::exit(1);
    }
}

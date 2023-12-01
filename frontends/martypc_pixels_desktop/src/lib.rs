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

use display_backend_pixels::PixelsBackend;
use std::{
    time::{Duration, Instant},
    cell::RefCell,
    rc::Rc,
    ffi::OsString,
    path::PathBuf
};

mod event_loop;
mod input;
mod cpu_test;
#[cfg(feature = "arduino_validator")]
mod run_fuzzer;
#[cfg(feature = "arduino_validator")]
mod run_gentests;
#[cfg(feature = "arduino_validator")]
mod run_runtests;

mod run_processtests;
mod run_headless;

use input::TranslateKey;
use marty_egui::GuiState;
use config_toml_bpaf::ConfigFileParams;

#[cfg(feature = "arduino_validator")]
use crate::run_fuzzer::run_fuzzer;

#[cfg(feature = "arduino_validator")]
use crate::run_gentests::run_gentests;

#[cfg(feature = "arduino_validator")]
use crate::run_runtests::run_runtests;

use crate::run_processtests::run_processtests;

use marty_core::{
    machine::{self, Machine, MachineState, ExecutionControl, ExecutionState},
    cpu_common::CpuOption,
    devices::{
        keyboard::KeyboardModifiers,
        hdc::HardDiskControllerType,
    },
    rom_manager::{RomManager, RomError, RomFeature},
    floppy_manager::{FloppyManager, FloppyError},
    machine_manager::MACHINE_DESCS,
    vhd_manager::{VHDManager, VHDManagerError},
    vhd::{self, VirtualHardDisk},
    videocard::{VideoType, ClockingMode},
    bytequeue::ByteQueue,
    sound::SoundPlayer,
};

use marty_egui::{GuiBoolean, GuiWindow};
use display_manager_wgpu::{
    DisplayBackend,
    DisplayManager,
    WgpuDisplayManager,
    WgpuDisplayManagerBuilder,
    DisplayManagerGuiOptions,
};
use marty_core::coreconfig::CoreConfig;

use videocard_renderer::{
    AspectRatio,
    SCALING_MODES,
    VideoRenderer,
    AspectCorrectionMode,
};

use marty_pixels_scaler::DisplayScaler;

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

/// Define the main Emulator struct for this frontend.
/// All the items that the winit event loop closure needs should be set here so that
/// we can call an event handler in a different file.
pub struct Emulator {
    dm: WgpuDisplayManager,
    config: ConfigFileParams,
    machine: Machine,
    exec_control: Rc<RefCell<ExecutionControl>>,
    mouse_data: MouseData,
    kb_data: KeyboardData,
    stat_counter: Counter,
    gui: GuiState,
    //context: &'a mut GuiRenderContext,
    floppy_manager: FloppyManager,
    vhd_manager: VHDManager,
    hdd_path: PathBuf,
    floppy_path: PathBuf,
    flags: EmuFlags
}

/// Define flags to be used by emulator.
pub struct EmuFlags {
    render_gui: bool,
    debug_keyboard: bool,
}

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
    #[allow (dead_code)]
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
    frame_delta_y: f64
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
            frame_delta_y: 0.0
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
    modifiers: KeyboardModifiers,
    ctrl_pressed: bool
}
impl KeyboardData {
    fn new() -> Self {
        Self { 
            modifiers: KeyboardModifiers::default(),    
            ctrl_pressed: false 
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
    let mut config = match config_toml_bpaf::get_config("./martypc.toml"){
        Ok(config) => config,
        Err(e) => {
            match e.downcast_ref::<std::io::Error>() {
                Some(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    eprintln!("Configuration file not found! Please create martypc.toml in the emulator directory \
                               or provide the path to configuration file with --configfile.");

                    std::process::exit(1);
                }
                Some(e) => {
                    eprintln!("Unknown IO error reading configuration file:\n{}", e);
                    std::process::exit(1);
                }                
                None => {
                    eprintln!("Failed to parse configuration file. There may be a typo or otherwise invalid toml:\n{}", e);
                    std::process::exit(1);
                }
            }
        }
    };

    let (video_type, clock_mode, video_debug) = {
        let mut video_type: Option<VideoType> = None;
        let mut clock_mode: Option<ClockingMode> = None;
        let mut video_debug = false;
        let video_cards = config.get_video_cards();
        if video_cards.len() > 0 {
            clock_mode = video_cards[0].clocking_mode;
            video_type = Some(video_cards[0].video_type); // Videotype is not optional
            video_debug = video_cards[0].debug.unwrap_or(false);
        }
        (
            video_type,
            clock_mode.unwrap_or_default(),
            video_debug
        )
    };

    // Determine required ROM features from configuration options
    match video_type {
        Some(VideoType::EGA) => {
            // an EGA BIOS ROM is required for EGA
            features.push(RomFeature::EGA);
        },
        Some(VideoType::VGA) => {
            // a VGA BIOS ROM is required for VGA
            features.push(RomFeature::VGA);
        },
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
            eprintln!("Compiled with validator but no validator specified" );
            std::process::exit(1);
        }
        _=> {}
    }

    // Instantiate the rom manager to load roms for the requested machine type    
    let mut rom_manager = 
        RomManager::new(
            config.machine.model, 
            features,
            config.machine.rom_override.clone(),
        );

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

    // Init graphics & GUI
    // let event_loop = EventLoop::new();




    /*
    let render_window = {
        let size = LogicalSize::new(WINDOW_WIDTH as f64, WINDOW_HEIGHT as f64);
        WindowBuilder::new()
            .with_title(format!("MartyPC {}", env!("CARGO_PKG_VERSION")))
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };
    */


    // ExecutionControl is shared via RefCell with GUI so that state can be updated by control widget
    let exec_control = Rc::new(RefCell::new(ExecutionControl::new()));

    // Set CPU state to Running if cpu_autostart option was set in config
    if config.emulator.cpu_autostart {
        exec_control.borrow_mut().set_state(ExecutionState::Running);
    }

    // Create the logical GUI.
    let gui = GuiState::new(exec_control.clone());

    /*
    let primary_video = if let Some(video) = config.machine.primary_video {
        video
    }
    else {
        panic!("No primary video type specified.")
    };

     */

    /*
    // Create pixels & egui backend
    let (pixels, mut framework) = {

        let render_window_opt = window_manager.get_render_window(primary_video);

        if let Some(mw_render) = render_window_opt {
            let window_size = mw_render.window.inner_size();
            let scale_factor = mw_render.window.scale_factor() as f32;
            let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &mw_render.window);
            //let (pixels_w, pixels_h) = {
            //    (video.params().aspect_w, video.params().aspect_h)
            //};
            let pixels =
                Pixels::new(
                    window::WINDOW_MIN_WIDTH,

                    window::WINDOW_MIN_HEIGHT,
                    surface_texture).unwrap();
            let framework =
                Framework::new(

                    &window_manager.get_event_loop().unwrap(),
                    window_size.width,
                    window_size.height,
                    scale_factor,
                    &pixels,
                    exec_control.clone(),
                    config.gui.theme_color
                );

            (pixels, framework)
        }
        else {
            panic!("Couldn't get marty_render window target.");
        }
    };

     */



    /*
    let fill_color = Color { r: 0.03, g: 0.03, b: 0.03, a: 1.0 }; // Dark grey.

    let marty_scaler = MartyScaler::new(
        ScalerMode::Integer,
        &pixels,
        640,480,
        640, 480,
        640, 480,
        24, // margin_y == egui menu height
        true,
        fill_color
    );
    */

    //let adapter_info = pixels.adapter().get_info();


    /*
    // Create the video renderer
    let mut video = 
        VideoRenderer::new(
            config.machine.primary_video.unwrap_or_default(),
            ScalerMode::Integer, 
            pixels,
            marty_scaler,
        );
    */

    //let pixels_arc = video.get_backend();





    let mut stat_counter = Counter::new();

    // KB modifiers
    let mut kb_data = KeyboardData::new();

    // Mouse event struct
    let mut mouse_data = MouseData::new(config.input.reverse_mouse_buttons);

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
        log::debug!("Given machine type {:?} got machine description: {:?}", config.machine.model, machine_desc);
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
        rom_manager
    );

    // Get a list of video devices from machine.
    let cardlist = machine.enumerate_video_cards();

    // Calculate icon path for window manager.
    let mut icon_path = PathBuf::new();
    icon_path.push(config.emulator.basedir.clone());
    icon_path.push("icon.png");

    let gui_options = DisplayManagerGuiOptions{
        theme_color: config.gui.theme_color,
        theme_dark: config.gui.theme_dark
    };

    // Create displays.
    let mut display_manager =
        WgpuDisplayManagerBuilder::build(
            &config,
            cardlist,
            icon_path,
            &gui_options,
        )
            .unwrap_or_else(|e| {
                log::error!("Failed to create displays: {:?}", e);
                std::process::exit(1);
            });

    let mut render_egui = true;
    let mut gui = GuiState::new(exec_control.clone());

    // Set list of serial ports
    gui.update_serial_ports(serial_ports);

    let adapter_info =
        display_manager
            .get_main_backend()
            .and_then(|backend| {
                backend.get_adapter_info()
            });

    let (backend_str, adapter_name_str) = {
        let backend_str;
        let adapter_name_str;

        if let Some(adapter_info) = adapter_info {
            backend_str = format!("{:?}", adapter_info.backend);
            adapter_name_str =  format!("{}", adapter_info.name);
            (backend_str, adapter_name_str)
        }
        else {
            log::error!("Failed to get adapter info from backend.");
            std::process::exit(1);
        }
    };

    log::debug!("wgpu using adapter: {}, backend: {}", adapter_name_str, backend_str);


    // Set the inital power-on state.
    if config.emulator.auto_poweron {
        machine.change_state(MachineState::On);
    }
    else {
        machine.change_state(MachineState::Off);
    }

    let debug_keyboard = config.emulator.debug_keyboard;

    // Do PIT phase offset option
    machine.pit_adjust(config.machine.pit_phase.unwrap_or(0) & 0x03);

    // Set options from config. We do this now so that we can set the same state for both GUI and machine

    // TODO: Add GUI for these two options?
    machine.set_cpu_option(CpuOption::OffRailsDetection(config.cpu.off_rails_detection.unwrap_or(false)));
    machine.set_cpu_option(CpuOption::EnableServiceInterrupt(config.cpu.service_interrupt_enabled.unwrap_or(false)));

    // TODO: Reenable these
    //gui.set_option(GuiBoolean::EnableSnow, config.machine.cga_snow.unwrap_or(false));
    //machine.set_video_option(VideoOption::EnableSnow(config.machine.cga_snow.unwrap_or(false)));
    //gui.set_option(GuiBoolean::CorrectAspect, config.emulator.scaler_aspect_correction);


    //if config.emulator.scaler_aspect_correction {
        // Default to hardware aspect correction.
       //video.set_aspect_mode(AspectCorrectionMode::Hardware);
        display_manager.for_each_target(|dt| {
            dt.set_aspect_mode(AspectCorrectionMode::Hardware);
        });
    //}

    gui.set_option(GuiBoolean::CpuEnableWaitStates, config.cpu.wait_states_enabled.unwrap_or(true));
    machine.set_cpu_option(CpuOption::EnableWaitStates(config.cpu.wait_states_enabled.unwrap_or(true)));

    gui.set_option(GuiBoolean::CpuInstructionHistory, config.cpu.instruction_history.unwrap_or(false));
    machine.set_cpu_option(CpuOption::InstructionHistory(config.cpu.instruction_history.unwrap_or(false)));

    gui.set_option(GuiBoolean::CpuTraceLoggingEnabled, config.emulator.trace_on);
    machine.set_cpu_option(CpuOption::TraceLoggingEnabled(config.emulator.trace_on));

    gui.set_option(GuiBoolean::TurboButton, config.machine.turbo);

    //TODO: renable these.
    //gui.set_option(GuiBoolean::CompositeDisplay, config.machine.composite.unwrap_or(false));

    if let Some(video_card) = machine.primary_videocard() {
        // Update display aperture options in GUI
        gui.set_display_apertures(video_card.list_display_apertures());
    }

    gui.set_scaler_modes((SCALING_MODES.to_vec(), Default::default()));

    // Disable warpspeed feature if 'devtools' flag not on.
    #[cfg(not(feature = "devtools"))]
    {
        config.emulator.warpspeed = false;
    }

    // Debug mode on? 
    if config.emulator.debug_mode {
        // Open default debug windows
        gui.set_window_open(GuiWindow::CpuControl, true);
        gui.set_window_open(GuiWindow::DisassemblyViewer, true);
        gui.set_window_open(GuiWindow::CpuStateViewer, true);

        // Override CpuInstructionHistory
        gui.set_option(GuiBoolean::CpuInstructionHistory, true);
        machine.set_cpu_option(CpuOption::InstructionHistory(true));

        // Disable autostart
        config.emulator.cpu_autostart = false;
    }

    #[cfg(debug_assertions)]
    if config.emulator.debug_warn {
        // User compiled MartyPC in debug mode, let them know...
        gui.show_warning(
            &"MartyPC has been compiled in debug mode and will be extremely slow.\n \
                    To compile in release mode, use 'cargo build -r'\n \
                    To disable this error, set debug_warn=false in martypc.toml.".to_string()
        );
    }

    // Load program binary if one was specified in config options
    if let Some(prog_bin) = config.emulator.run_bin.clone() {

        if let Some(prog_seg) = config.emulator.run_bin_seg {
            if let Some(prog_ofs) = config.emulator.run_bin_ofs {
                let prog_vec = match std::fs::read(prog_bin.clone()) {
                    Ok(vec) => vec,
                    Err(e) => {
                        eprintln!("Error opening filename {:?}: {}", prog_bin, e);
                        std::process::exit(1);
                    }
                };

                if let Err(_) = machine.load_program(&prog_vec, prog_seg, prog_ofs) {
                    eprintln!("Error loading program into memory at {:04X}:{:04X}.", prog_seg, prog_ofs);
                    std::process::exit(1);
                };
            }
            else {
                eprintln!("Must specify program load offset.");
                std::process::exit(1);
            }
        }
        else {
            eprintln!("Must specify program load segment.");
            std::process::exit(1);  
        }
    }

    /*
    // Resize window if video card is in Direct mode and specifies a display aperture
    {
        if let Some(card) = machine.videocard() {
            if let RenderMode::Direct = card.get_render_mode() {
                if let Some(render_window) = window_manager.get_render_window(card.get_video_type()) {
                    let extents = card.get_display_extents();
                    let (aper_x, mut aper_y) = card.get_display_aperture();
                    assert!(aper_x != 0 && aper_y !=0 );

                    if extents.double_scan {
                        video.set_double_scan(true);
                        aper_y *= 2;
                    }
                    else {
                        video.set_double_scan(false);
                    }

                    let aspect_ratio = if config.emulator.scaler_aspect_correction {
                        Some(marty_render::AspectRatio{ h: 4, v: 3 })
                    }
                    else {
                        None
                    };

                    video.set_aspect_ratio(aspect_ratio);

                    let (aper_correct_x, aper_correct_y) = {
                        let dim = video.get_display_dimensions();
                        (dim.w, dim.h)
                    };

                    let mut double_res = false;


                    // Get the current monitor resolution.
                    if let Some(monitor) = render_window.window.current_monitor() {
                        let monitor_size = monitor.size();
                        let dip_scale = monitor.scale_factor();

                        log::debug!("Current monitor resolution: {}x{} scale factor: {}", monitor_size.width, monitor_size.height, dip_scale);

                        // Take into account DPI scaling for window-fit.
                        let scaled_width = ((aper_correct_x * 2) as f64 * dip_scale) as u32;
                        let scaled_height = ((aper_correct_y * 2) as f64 * dip_scale) as u32;
                        log::debug!("Target resolution after aspect correction and DPI scaling: {}x{}", scaled_width, scaled_height);

                        if (scaled_width <= monitor_size.width) && (scaled_height <= monitor_size.height) {
                            // Monitor is large enough to double the display window
                            double_res = true;
                        }
                    }

                    let window_resize_w = if double_res { aper_correct_x * 2 } else { aper_correct_x };
                    let window_resize_h = if double_res { aper_correct_y * 2 } else { aper_correct_y };

                    log::debug!("Resizing window to {}x{}", window_resize_w, window_resize_h);
                    //resize_h = if card.get_scanline_double() { resize_h * 2 } else { resize_h };

                    render_window.window.set_inner_size(winit::dpi::LogicalSize::new(window_resize_w, window_resize_h));

                    log::debug!("Resizing marty_render buffer to {}x{}", aper_x, aper_y);

                    video.resize((aper_x, aper_y).into());

                    /*
                    let pixel_res = video.get_display_dimensions();

                    if (pixel_res.w > 0) && (pixel_res.h > 0) {
                        log::debug!("Resizing pixel buffer to {}x{}", pixel_res.w, pixel_res.h);
                        pixels.resize_buffer(pixel_res.w, pixel_res.h).expect("Failed to resize Pixels buffer.");
                    }
                    */

                    //VideoRenderer::set_alpha(pixels.frame_mut(), pixel_res.w, pixel_res.h, 255);

                    // Recalculate sampling parameters.
                    //resample_context.precalc(aper_x, aper_y, aper_correct_x, aper_correct_y);

                    // Update internal state and request a redraw
                    render_window.window.request_redraw();
                }

            }
        }
    }

     */

    let mut vhd_names: Vec<Option<String>> = Vec::new();

    vhd_names.push(config.machine.drive0.clone());
    vhd_names.push(config.machine.drive1.clone());

    let mut vhd_idx: usize = 0;
    for vhd_name in vhd_names.into_iter().filter_map(|x| x) {
        let vhd_os_name: OsString = vhd_name.into();
        match vhd_manager.load_vhd_file(vhd_idx, &vhd_os_name) {
            Ok(vhd_file) => {
                match VirtualHardDisk::from_file(vhd_file) {
                    Ok(vhd) => {
                        if let Some(hdc) = machine.hdc() {
                            match hdc.set_vhd(0_usize, vhd) {
                                Ok(_) => {
                                    log::info!("VHD image {:?} successfully loaded into virtual drive: {}", vhd_os_name, 0);
                                }
                                Err(err) => {
                                    log::error!("Error mounting VHD: {}", err);
                                }
                            }
                        }
                        else {
                            log::error!("Couldn't load VHD: No Hard Disk Controller present!");
                        }
                    },
                    Err(err) => {
                        log::error!("Error loading VHD: {}", err);
                    }
                }
            }
            Err(err) => {
                log::error!("Failed to load VHD image {:?}: {}", vhd_os_name, err);
            }
        }
        vhd_idx += 1;
    }

    // Start buffer playback
    machine.play_sound_buffer();

    let gui_ctx =
        display_manager
            .get_main_gui_mut()
            .expect("Couldn't get main gui context!");

    // Put everything we want to handle in event loop into an Emulator struct
    let mut emulator = Emulator {
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
            debug_keyboard
        }
    };

    let event_loop = emulator.dm.take_event_loop();

    // Run the winit event loop
    event_loop.run(move |event, elwt| {
        handle_event(&mut emulator, event, elwt);
    });
}

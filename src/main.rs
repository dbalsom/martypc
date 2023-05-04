#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::too_many_arguments)]
#![forbid(unsafe_code)]

use std::{
    time::{Duration, Instant},
    cell::RefCell,
    rc::Rc,
    path::Path,
    ffi::OsString
};

use crate::egui::Framework;

use log::error;
use pixels::{Pixels, SurfaceTexture};

use winit::{
    dpi::LogicalSize,
    error::ExternalError,
    event::{
        Event, 
        WindowEvent, 
        DeviceEvent, 
        ElementState, 
        StartCause, 
        VirtualKeyCode,
    },
    event_loop::{
        ControlFlow,
        EventLoop
    },
    window::WindowBuilder
};

use winit_input_helper::WinitInputHelper;

#[path = "./devices/ega/mod.rs"]
mod ega;
#[path = "./devices/vga/mod.rs"]
mod vga;
mod breakpoints;
mod bus;
mod bytebuf;
mod bytequeue;
mod cga;
mod config;
mod cpu_common;
mod cpu_808x;
mod dma;
mod fdc;
mod floppy_manager;
mod egui;
mod hdc;
mod bus_io;
mod interrupt;
mod machine;
mod machine_manager;
mod mc6845;
mod memerror;
mod mouse;
mod pic;
mod pit;
mod ppi;
mod rom_manager;
mod serial;
mod sound;
mod syntax_token;
mod tracelogger;
mod updatable;
mod util;

mod vhd;
mod vhd_manager;
mod render;
mod videocard; // VideoCard trait
mod input;

mod cpu_validator; // CpuValidator trait
#[cfg(feature = "pi_validator")]
mod pi_cpu_validator;
#[cfg(feature = "arduino_validator")]
#[macro_use]
mod arduino8088_client;
#[cfg(feature = "arduino_validator")]
mod arduino8088_validator;

use input::MouseButton;
use breakpoints::BreakPointType;
use config::*;
use machine::{Machine, ExecutionState};
use cpu_808x::{Cpu, CpuAddress};
use cpu_common::CpuOption;
use rom_manager::{RomManager, RomError, RomFeature};
use floppy_manager::{FloppyManager, FloppyError};
use machine_manager::MACHINE_DESCS;
use vhd_manager::{VHDManager, VHDManagerError};
use vhd::{VirtualHardDisk};
use videocard::{RenderMode};
use bytequeue::ByteQueue;
use crate::egui::{GuiEvent, GuiOption , GuiWindow};
use render::{VideoRenderer, CompositeParams};
use sound::SoundPlayer;
use syntax_token::SyntaxToken;

const EGUI_MENU_BAR: u32 = 25;
const WINDOW_WIDTH: u32 = 1280;
const WINDOW_HEIGHT: u32 = 960 + EGUI_MENU_BAR * 2;

const DEFAULT_RENDER_WIDTH: u32 = 640;
const DEFAULT_RENDER_HEIGHT: u32 = 400;

const MIN_RENDER_WIDTH: u32 = 160;
const MIN_RENDER_HEIGHT: u32 = 200;
const RENDER_ASPECT: f32 = 0.75;

pub const FPS_TARGET: f64 = 60.0;
const MICROS_PER_FRAME: f64 = 1.0 / FPS_TARGET * 1000000.0;

// Remove static frequency references
//const CYCLES_PER_FRAME: u32 = (cpu_808x::CPU_MHZ * 1000000.0 / FPS_TARGET) as u32;


pub struct PerformanceStats {
    emulation_time: Duration,
    render_time: Duration,
    fps: u32,
    emulated_fps: u32,
    cps: u64,
    ips: u64,
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
    last_pit_ticks: u64,
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
            last_pit_ticks: 0,
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
    ctrl_pressed: bool
}
impl KeyboardData {
    fn new() -> Self {
        Self { ctrl_pressed: false }
    }
}

#[derive (Copy, Clone, Default)]
struct VideoData {
    render_w: u32,
    render_h: u32,
    aspect_w: u32,
    aspect_h: u32,
    aspect_correction_enabled: bool,
    composite_params: CompositeParams
}

fn main() {

    env_logger::init();

    let mut features = Vec::new();

    // Read config file
    let mut config = match config::get_config("./marty.toml"){
        Ok(config) => config,
        Err(e) => {
            match e.downcast_ref::<std::io::Error>() {
                Some(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    eprintln!("Configuration file not found! Please create marty.toml in the emulator directory \
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

    // Determine required ROM features from configuration options
    match config.machine.video {
        VideoType::EGA => {
            // an EGA BIOS ROM is required for EGA
            features.push(RomFeature::EGA);
        },
        VideoType::VGA => {
            // a VGA BIOS ROM is required for VGA
            features.push(RomFeature::VGA);
        },
        _ => {}
    }

    match config.machine.hdc {
        HardDiskControllerType::Xebec => {
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
    let mut rom_manager = RomManager::new(config.machine.model, features);

    if let Err(e) = rom_manager.try_load_from_dir("./rom") {
        match e {
            RomError::DirNotFound => {
                eprintln!("ROM directory not found")
            }
            RomError::RomNotFoundForMachine => {
                eprintln!("No valid ROM found for specified machine type")
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
    if let Err(e) = floppy_manager.scan_dir("./floppy") {
        match e {
            FloppyError::DirNotFound => {
                eprintln!("Floppy directory not found")
            }
            _ => {
                eprintln!("Error reading floppy directory")
            }
        }
        std::process::exit(1);
    }

    // Instantiate the VHD manager
    let mut vhd_manager = VHDManager::new();

    // Scan the HDD directory
    if let Err(e) = vhd_manager.scan_dir("./hdd") {
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


    // If fuzzer mode was specified, run the emulator in fuzzer mode now
    #[cfg(feature = "cpu_validator")]
    if config.emulator.fuzzer {
        return main_fuzzer(&config, rom_manager, floppy_manager);
    }

    // If headless mode was specified, run the emulator in headless mode now
    if config.emulator.headless {
        return main_headless(&config, rom_manager, floppy_manager);
    }

    // Create the video renderer
    let mut video = render::VideoRenderer::new(config.machine.video);

    // Init graphics & GUI 
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let window = {
        let size = LogicalSize::new(WINDOW_WIDTH as f64, WINDOW_HEIGHT as f64);
        WindowBuilder::new()
            .with_title("MartyPC")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    // ExecutionControl is shared via RefCell with GUI so that state can be updated by control widget
    let exec_control = Rc::new(RefCell::new(machine::ExecutionControl::new()));

    // Set machine state to Running if autostart option was set in config
    if config.emulator.autostart {
        exec_control.borrow_mut().set_state(ExecutionState::Running);
    }

    // Create render buf
    let mut render_src = vec![0; (DEFAULT_RENDER_WIDTH * DEFAULT_RENDER_HEIGHT * 4) as usize];
    let mut video_data = VideoData {
        render_w: DEFAULT_RENDER_WIDTH,
        render_h: DEFAULT_RENDER_HEIGHT,
        aspect_w: 640,
        aspect_h: 480,
        aspect_correction_enabled: false,
        composite_params: Default::default(),
    };

    let (mut pixels, mut framework) = {
        let window_size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        let pixels = 
            Pixels::new(video_data.aspect_w, video_data.aspect_h, surface_texture).unwrap();
        let framework =
            Framework::new(
                &event_loop,
                window_size.width, 
                window_size.height, 
                scale_factor, 
                &pixels, 
                exec_control.clone()
            );

        (pixels, framework)
    };

    // Set list of serial ports
    framework.gui.update_serial_ports(serial_ports);

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
        config.emulator.trace_mode,
        config.machine.video, 
        sp, 
        rom_manager, 
        floppy_manager,
    );

    // Set options from config. We do this now so that we can set the same state for both GUI and machine
    framework.gui.set_option(GuiOption::CorrectAspect, config.emulator.correct_aspect);

    framework.gui.set_option(GuiOption::CpuEnableWaitStates, config.cpu.wait_states_enabled);
    machine.set_cpu_option(CpuOption::EnableWaitStates(config.cpu.wait_states_enabled));

    framework.gui.set_option(GuiOption::CpuInstructionHistory, config.cpu.instruction_history);
    machine.set_cpu_option(CpuOption::InstructionHistory(config.cpu.instruction_history));

    framework.gui.set_option(GuiOption::CpuTraceLoggingEnabled, config.emulator.trace_on);
    machine.set_cpu_option(CpuOption::TraceLoggingEnabled(config.emulator.trace_on));

    // Debug mode on? 
    if config.emulator.debug_mode {
        // Open default debug windows
        framework.gui.set_window_open(GuiWindow::CpuControl, true);
        framework.gui.set_window_open(GuiWindow::DiassemblyViewer, true);
        framework.gui.set_window_open(GuiWindow::CpuStateViewer, true);

        // Override CpuInstructionHistory
        framework.gui.set_option(GuiOption::CpuInstructionHistory, true);
        machine.set_cpu_option(CpuOption::InstructionHistory(true));

        // Disable autostart
        config.emulator.autostart = false;
    }

    // Load program binary if one was specified in config options
    if let Some(prog_bin) = config.emulator.run_bin {

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
                eprintln!("Must specifiy program load offset.");
                std::process::exit(1);
            }
        }
        else {
            eprintln!("Must specifiy program load segment.");
            std::process::exit(1);  
        }
    }

    // Resize window if video card is in Direct mode and specifies a display aperature
    {
        if let Some(card) = machine.videocard() {
            if let RenderMode::Direct = card.get_render_mode() {

                let (aper_x, mut aper_y) = card.get_display_aperture();

                if card.get_scanline_double() {
                    aper_y *= 2;
                }

                let (aper_correct_x, aper_correct_y) = 
                    VideoRenderer::get_aspect_corrected_res(
                        (aper_x, aper_y),
                        render::AspectRatio{ h: 4, v: 3 }
                    );

                let mut double_res = false;

                // Get the current monitor resolution. 
                if let Some(monitor) = window.current_monitor() {
                    let monitor_size = monitor.size();
                    
                    log::debug!("Current monitor resolution: {}x{}", monitor_size.width, monitor_size.height);

                    if ((aper_correct_x * 2) <= monitor_size.width) && ((aper_correct_y * 2) <= monitor_size.height) {
                        // Monitor is large enough to double the display window
                        double_res = true;
                    }
                }

                let window_resize_w = if double_res { aper_correct_x * 2 } else { aper_correct_x };
                let window_resize_h = if double_res { aper_correct_y * 2 } else { aper_correct_y };

                log::debug!("Resizing window to {}x{}", window_resize_w, window_resize_h);
                //resize_h = if card.get_scanline_double() { resize_h * 2 } else { resize_h };

                window.set_inner_size(winit::dpi::LogicalSize::new(window_resize_w, window_resize_h));

                log::debug!("Reiszing render buffer to {}x{}", aper_x, aper_y);

                render_src.resize((aper_x * aper_y * 4) as usize, 0);
                render_src.fill(0);

                let (pixel_buf_w, pixel_buf_h) = if config.emulator.correct_aspect {
                    (aper_x, aper_correct_y)
                }
                else {
                    (aper_x, aper_y)
                };
                
                log::debug!("Resizing pixel buffer to {}x{}", pixel_buf_w, pixel_buf_h);
                pixels.resize_buffer(pixel_buf_w, pixel_buf_h).expect("Failed to resize Pixels buffer.");

                // Pixels will resize itself from window size event
                /*
                if pixels.resize_surface(aper_correct_x, aper_correct_y).is_err() {
                    // Some error occured but not much we can do about it.
                    // Errors get thrown when the window minimizes.
                    log::error!("Unable to resize pixels surface!");
                }

                framework.resize(window_resize_w, window_resize_h);
                */

                video_data.render_w = aper_x;
                video_data.render_h = aper_y;
                video_data.aspect_w = aper_correct_x;
                video_data.aspect_h = aper_correct_y;

                // Update internal state and request a redraw
                window.request_redraw();
            }
        }
    }
        
    // Try to load default vhd
    if let Some(vhd_name) = config.machine.drive0 {
        let vhd_os_name: OsString = vhd_name.into();
        match vhd_manager.get_vhd_file(&vhd_os_name) {
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
    }   

    // Start buffer playback
    machine.play_sound_buffer();
    
    // Run the winit event loop
    event_loop.run(move |event, _, control_flow| {

        //*control_flow = ControlFlow::Poll;
    
        // Handle input events
        if input.update(&event) {
            // Close events
            
            if input.quit() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // Update the scale factor
            if let Some(scale_factor) = input.scale_factor() {
                framework.scale_factor(scale_factor);
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                log::debug!("Resizing pixel surface to {}x{}", size.width, size.height);
                if pixels.resize_surface(size.width, size.height).is_err() {
                    // Some error occured but not much we can do about it.
                    // Errors get thrown when the window minimizes.
                }
                framework.resize(size.width, size.height);
            }

            // Update internal state and request a redraw
            window.request_redraw();
        }

        match event {
            Event::NewEvents(StartCause::Init) => {
                // Initialization stuff here?
                stat_counter.last_second = Instant::now();
            }
            Event::DeviceEvent{ event, .. } => {
                match event {
                    DeviceEvent::MouseMotion {
                        delta: (x, y)
                    } => {
                        // We can get a lot more mouse updates than we want to send to the virtual mouse,
                        // so add up all deltas between each mouse polling period
                        mouse_data.have_update = true;
                        mouse_data.frame_delta_x += x;
                        mouse_data.frame_delta_y += y;
                    },
                    DeviceEvent::Button { 
                        button,
                        state 
                    } => {
                        // Button ID is a raw u32. It appears that the id's for relative buttons are not consistent
                        // accross platforms. 1 == left button on windows, 3 == left button on macos. So we resolve
                        // button ids to button enums based on platform. There is a config option to override button 
                        // order.

                        // Resolve the winit button id to a button enum based on platform and reverse flag.
                        let mbutton = input::button_from_id(button, mouse_data.reverse_buttons);

                        // A mouse click could be faster than one frame (pressed & released in 16.6ms), therefore mouse 
                        // clicks are 'sticky', if a button was pressed during the last update period it will be sent as
                        // pressed during virtual mouse update.

                        match (mbutton, state) {
                            (MouseButton::Left, ElementState::Pressed) => {
                                mouse_data.l_button_was_pressed = true;
                                mouse_data.l_button_is_pressed = true;
                                mouse_data.have_update = true;
                            },
                            (MouseButton::Left, ElementState::Released) => {
                                mouse_data.l_button_is_pressed = false;
                                mouse_data.l_button_was_released = true;
                                mouse_data.have_update = true;
                            },
                            (MouseButton::Right, ElementState::Pressed) => {
                                mouse_data.r_button_was_pressed = true;
                                mouse_data.r_button_is_pressed = true;
                                mouse_data.have_update = true;
                            },
                            (MouseButton::Right, ElementState::Released) => {
                                mouse_data.r_button_is_pressed = false;
                                mouse_data.r_button_was_released = true;
                                mouse_data.have_update = true;
                            }                              
                            _=> {}
                        }
                        //log::debug!("Mouse button: {:?} state: {:?}", button, state);
                    }
                    _ => {

                    }
                }
            }
            Event::WindowEvent{ event, .. } => {

                match event {
                    WindowEvent::ModifiersChanged(modifier_state) => {
                        kb_data.ctrl_pressed = modifier_state.ctrl();
                    }
                    WindowEvent::KeyboardInput {
                        input: winit::event::KeyboardInput {
                            virtual_keycode: Some(keycode),
                            state,
                            ..
                        },
                        ..
                    } => {

                        // Match global hotkeys regardless of egui focus
                        match (state, keycode) {
                            (winit::event::ElementState::Pressed, VirtualKeyCode::F10 ) => {
                                if kb_data.ctrl_pressed {
                                    // Ctrl-F10 pressed. Toggle mouse capture.
                                    log::info!("Control F10 pressed. Capturing mouse cursor.");
                                    if !mouse_data.is_captured {
                                        let mut grab_success = false;
                                        match window.set_cursor_grab(winit::window::CursorGrabMode::Confined) {
                                            Ok(_) => {
                                                mouse_data.is_captured = true;
                                                grab_success = true;
                                            }
                                            Err(_) => {
                                                // Try alternate grab mode (Windows/Mac require opposite modes)
                                                match window.set_cursor_grab(winit::window::CursorGrabMode::Locked) {
                                                    Ok(_) => {
                                                        mouse_data.is_captured = true;
                                                        grab_success = true;
                                                    } 
                                                    Err(e) => log::error!("Couldn't set cursor grab mode: {:?}", e)
                                                }
                                            }
                                        }
                                        // Hide mouse cursor if grab successful
                                        if grab_success {
                                            window.set_cursor_visible(false);
                                        }
                                    }
                                    else {
                                        // Cursor is grabbed, ungrab
                                        match window.set_cursor_grab(winit::window::CursorGrabMode::None) {
                                            Ok(_) => mouse_data.is_captured = false,
                                            Err(e) => log::error!("Couldn't set cursor grab mode: {:?}", e)
                                        }
                                        window.set_cursor_visible(true);
                                    }
                                    
                                }
                            }
                            _=>{}
                        }

                        if !framework.has_focus() {
                            // An egui widget doesn't have focus, so send an event to the emulated machine
                            // TODO: widget seems to lose focus before 'enter' is processed in a text entry, passing that 
                            // enter to the emulator
                            match state {
                                winit::event::ElementState::Pressed => {
                                    
                                    if let Some(keycode) = input::match_virtual_keycode(keycode) {
                                        //log::debug!("Key pressed, keycode: {:?}: xt: {:02X}", keycode, keycode);
                                        machine.key_press(keycode);
                                    };
                                },
                                winit::event::ElementState::Released => {
                                    if let Some(keycode) = input::match_virtual_keycode(keycode) {
                                        //log::debug!("Key released, keycode: {:?}: xt: {:02X}", keycode, keycode);
                                        machine.key_release(keycode);
                                    };
                                }
                            }
                        }
                        else {
                            // Egui widget has focus, so send keyboard event to egui
                            framework.handle_event(&event);
                        }
                    },
                    _ => {
                        framework.handle_event(&event);
                    }
                }
            },

            // Draw the current frame
            Event::MainEventsCleared => {

                stat_counter.current_ups += 1;

                // Calculate FPS
                let elapsed_ms = stat_counter.last_second.elapsed().as_millis();
                if elapsed_ms > 1000 {
                    // One second elapsed, calculate FPS/CPS
                    let pit_ticks = machine.pit_cycles();
                    let cpu_cycles = machine.cpu_cycles();

                    stat_counter.current_cpu_cps = cpu_cycles - stat_counter.last_cpu_cycles;
                    stat_counter.last_cpu_cycles = cpu_cycles;

                    stat_counter.current_pit_tps = pit_ticks - stat_counter.last_pit_ticks;
                    stat_counter.last_pit_ticks = pit_ticks;

                    //println!("fps: {} | cps: {} | pit tps: {}", 
                    //    stat_counter.current_fps,
                    //    stat_counter.current_cpu_cps, 
                    //    stat_counter.current_pit_tps);

                    stat_counter.ups = stat_counter.current_ups;
                    stat_counter.current_ups = 0;
                    stat_counter.fps = stat_counter.current_fps;
                    stat_counter.current_fps = 0;

                    // Update IPS and reset instruction count for next second

                    stat_counter.current_cps = stat_counter.cycle_count;
                    stat_counter.cycle_count = 0;

                    stat_counter.emulated_fps = stat_counter.current_emulated_frames as u32;
                    stat_counter.current_emulated_frames = 0;

                    stat_counter.current_ips = stat_counter.instr_count;
                    stat_counter.instr_count = 0;
                    stat_counter.last_second = Instant::now();
                } 

                // Decide whether to draw a frame
                let elapsed_us = stat_counter.last_frame.elapsed().as_micros();
                stat_counter.last_frame = Instant::now();

                stat_counter.accumulated_us += elapsed_us;

                while stat_counter.accumulated_us > MICROS_PER_FRAME as u128 {

                    stat_counter.accumulated_us -= MICROS_PER_FRAME as u128;
                    stat_counter.last_frame = Instant::now();
                    stat_counter.frame_count += 1;
                    stat_counter.current_fps += 1;
                    //println!("frame: {} elapsed: {}", world.current_fps, elapsed_us);

                    // Get single step flag from GUI and either step or run CPU
                    // TODO: This logic is messy, figure out a better way to control CPU state 
                    //       via gui

                    //if framework.gui.get_cpu_single_step() {
                    //    if framework.gui.get_cpu_step_flag() {
                    //        machine.run(CYCLES_PER_FRAME, &exec_control.borrow(), 0);
                    //    }
                    //}
                    //else {
                    //    machine.run(CYCLES_PER_FRAME, &exec_control.borrow(), bp_addr);
                    //    // Check for breakpoint
                    //    if machine.cpu().get_flat_address() == bp_addr && bp_addr != 0 {
                    //        log::debug!("Breakpoint hit at {:06X}", bp_addr);
                    //        framework.gui.set_cpu_single_step();
                    //    }
                    //}

                    if let Some(mouse) = machine.mouse_mut() {
                        // Send any pending mouse update to machine if mouse is captured
                        if mouse_data.is_captured && mouse_data.have_update {
                            mouse.update(
                                mouse_data.l_button_was_pressed,
                                mouse_data.r_button_was_pressed,
                                mouse_data.frame_delta_x,
                                mouse_data.frame_delta_y
                            );

                            // Handle release event
                            let l_release_state = 
                                if mouse_data.l_button_was_released {
                                    false
                                }
                                else {
                                    mouse_data.l_button_was_pressed
                                };
                            
                            let r_release_state = 
                                if mouse_data.r_button_was_released {
                                    false
                                }
                                else {
                                    mouse_data.r_button_was_pressed
                                };

                            if mouse_data.l_button_was_released || mouse_data.r_button_was_released {
                                // Send release event
                                mouse.update(
                                    l_release_state,
                                    r_release_state,
                                    0.0,
                                    0.0
                                );                            
                            }

                            // Reset mouse for next frame
                            mouse_data.reset();
                        }
                    }

                    // Emulate a frame worth of instructions
                    // ---------------------------------------------------------------------------

                    // Recalculate cycle target based on current CPU speed if it has changed (or uninitialized)
                    let mhz = machine.get_cpu_mhz();
                    if mhz != stat_counter.cpu_mhz {
                        stat_counter.cycles_per_frame = (machine.get_cpu_mhz() * 1000000.0 / FPS_TARGET) as u32;
                        stat_counter.cycle_target = stat_counter.cycles_per_frame;
                        log::info!("CPU clock has changed to {}Mhz; new cycle target: {}", mhz, stat_counter.cycle_target);
                        stat_counter.cpu_mhz = mhz;
                    }
                    
                    let emulation_start = Instant::now();
                    stat_counter.instr_count += machine.run(stat_counter.cycle_target, &mut exec_control.borrow_mut());
                    stat_counter.emulation_time = Instant::now() - emulation_start;

                    // Add instructions to IPS counter
                    stat_counter.cycle_count += stat_counter.cycle_target as u64;

                    // Add emulated frames from video card device to emulated frame counter
                    let mut frame_count = 0;
                    if let Some(video_card) = machine.videocard() {
                        // We have a video card to query
                        frame_count = video_card.get_frame_count()
                    }
                    let elapsed_frames = frame_count - stat_counter.emulated_frames;
                    stat_counter.emulated_frames += elapsed_frames;
                    stat_counter.current_emulated_frames += elapsed_frames;

                    // Emulation time budget is 16ms - render time in ms - fudge factor
                    let render_time = stat_counter.render_time.as_micros();
                    let emulation_time = stat_counter.emulation_time.as_millis();
                    let mut emulation_time_allowed_ms = 16;
                    if render_time < 16 {
                        // Rendering time has left us some emulation headroom
                        emulation_time_allowed_ms = 16_u128.saturating_sub(render_time);
                    }
                    else {
                        // Rendering is too long to run at 60fps. Just ignore render time for now.
                    }                    

                    // If emulation time took too long, reduce CYCLE_TARGET
                    if emulation_time > emulation_time_allowed_ms {
                        // Emulation running slower than 60fps
                        let factor: f64 = (stat_counter.emulation_time.as_millis() as f64) / emulation_time_allowed_ms as f64;
                        // Decrease speed by half of scaling factor

                        let old_target = stat_counter.cycle_target;
                        let new_target = (stat_counter.cycle_target as f64 / factor) as u32;
                        stat_counter.cycle_target -= (old_target - new_target) / 2;

                        /*
                        log::trace!("Emulation speed slow: ({}ms > {}ms). Reducing cycle target: {}->{}", 
                            emulation_time,
                            emulation_time_allowed_ms,
                            old_target,
                            stat_counter.cycle_target
                        );
                        */
                    }
                    else if (emulation_time > 0) && (emulation_time < emulation_time_allowed_ms) {
                        // Emulation could run faster
                            
                        // Increase speed by half of scaling factor
                        let factor: f64 = (stat_counter.emulation_time.as_millis() as f64) / emulation_time_allowed_ms as f64;

                        let old_target = stat_counter.cycle_target;
                        let new_target = (stat_counter.cycle_target as f64 / factor) as u32;
                        stat_counter.cycle_target += (new_target - old_target) / 2;

                        if stat_counter.cycle_target > stat_counter.cycles_per_frame {
                            // Warpspeed runs entire emulator as fast as possible 
                            // TODO: Limit cycle target based on render/gui time to maintain 60fps GUI updates
                            if !config.emulator.warpspeed {
                                stat_counter.cycle_target = stat_counter.cycles_per_frame;
                            }
                        }
                        else {
                            /*
                            log::trace!("Emulation speed recovering. ({}ms < {}ms). Increasing cycle target: {}->{}" ,
                                emulation_time,
                                emulation_time_allowed_ms,
                                old_target,
                                stat_counter.cycle_target
                            );
                            */
                        }
                    }

                    /*
                    log::debug!(
                        "Cycle target: {} emulation time: {} allowed_ms: {}", 
                        stat_counter.cycle_target, 
                        emulation_time,
                        emulation_time_allowed_ms
                    );
                    */

                    // Do per-frame updates (Serial port emulation)
                    machine.frame_update();

                    // Check if there was a resolution change, if a video card is present
                    if let Some(video_card) = machine.videocard() {

                        let new_w;
                        let mut new_h;

                        match video_card.get_render_mode() {
                            RenderMode::Direct => {
                                /*
                                let extents = video_card.get_display_extents();

                                new_w = extents.visible_w;
                                new_h = extents.visible_h;
                                */

                                (new_w, new_h) = video_card.get_display_aperture();

                                // Set a sane maximum
                                if new_h > 240 { 
                                    new_h = 240;
                                }
                            }
                            RenderMode::Indirect => {
                                (new_w, new_h) = video_card.get_display_size();
                            }
                        }

                        // If CGA, we will double scanlines later in the renderer, so make our buffer twice
                        // as high.
                        if video_card.get_scanline_double() {
                            new_h = new_h * 2;
                        }
                        
                        if new_w >= MIN_RENDER_WIDTH && new_h >= MIN_RENDER_HEIGHT {

                            let vertical_delta = (video_data.render_h as i32).wrapping_sub(new_h as i32).abs();

                            // Hack for 8088mph. If vertical resolution is decreasing by less than N, do not
                            // make a new buffer. 8088mph alternates between 239 and 240 scanlines when displaying
                            // its 1024 color mode. 
                            if (new_w != video_data.render_w) || ((new_h != video_data.render_h) && (vertical_delta <= 2)) {
                                // Resize buffers
                                log::debug!("Setting internal resolution to ({},{})", new_w, new_h);
                                video_card.write_trace_log(format!("Setting internal resolution to ({},{})", new_w, new_h));

                                // Calculate new aspect ratio (make this option)
                                video_data.render_w = new_w;
                                video_data.render_h = new_h;
                                render_src.resize((new_w * new_h * 4) as usize, 0);                                
                                render_src.fill(0);
    
                                video_data.aspect_w = video_data.render_w;
                                let aspect_corrected_h = f32::floor(video_data.render_w as f32 * RENDER_ASPECT) as u32;
                                // Don't make height smaller
                                let new_height = std::cmp::max(video_data.render_h, aspect_corrected_h);
                                video_data.aspect_h = new_height;
    
                                pixels.get_frame_mut().fill(0);
                                if let Err(e) = pixels.resize_buffer(video_data.aspect_w, video_data.aspect_h) {
                                    log::error!("Failed to resize pixel pixel buffer: {}", e);
                                }
                            }
                        }
                    }

                    // -- Draw video memory --
                    let composite_enabled = framework.gui.get_composite_enabled();
                    let aspect_correct = framework.gui.get_option(GuiOption::CorrectAspect).unwrap_or(false);

                    let render_start = Instant::now();

                    // Draw video if there is a video card present
                    let bus = machine.bus_mut();

                    if let Some(video_card) = bus.video() {

                        if composite_enabled {
                            video_data.composite_params = framework.gui.composite_adjust.get_params().clone();
                        }

                        let video_buffer;
                        // Get the appropriate buffer depending on run mode. If execution is paused 
                        // (debugging) show the back buffer instead of front buffer. 
                        // TODO: Discriminate between paused in debug mode vs user paused state
                        // TODO: buffer and extents may not match due to extents being for front buffer
                        if let ExecutionState::Paused = exec_control.borrow_mut().get_state() {
                            video_buffer = video_card.get_back_buf();
                        }
                        else {
                            video_buffer = video_card.get_display_buf();
                        }

                        // Get the render mode from the device and render appropriately
                        match (video_card.get_video_type(), video_card.get_render_mode()) {

                            (VideoType::CGA, RenderMode::Direct) => {
                                // Draw device's back buffer in direct mode (CGA only for now)

                                match aspect_correct {
                                    true => {
                                        video.draw_cga_direct(
                                            &mut render_src,
                                            video_data.render_w, 
                                            video_data.render_h,                                             
                                            video_buffer,
                                            video_card.get_display_extents(),
                                            composite_enabled,
                                            &video_data.composite_params
                                        );

                                        render::resize_linear(
                                            &render_src, 
                                            video_data.render_w, 
                                            video_data.render_h, 
                                            pixels.get_frame_mut(), 
                                            video_data.aspect_w, 
                                            video_data.aspect_h);                            
                                    }
                                    false => {
                                        video.draw_cga_direct(
                                            pixels.get_frame_mut(),
                                            video_data.render_w, 
                                            video_data.render_h,                                                                                         
                                            video_buffer,
                                            video_card.get_display_extents(),
                                            composite_enabled,
                                            &video_data.composite_params                                            
                                        );
                                    }
                                }
                            }
                            (_, RenderMode::Indirect) => {
                                // Draw VRAM in indirect mode
                                match aspect_correct {
                                    true => {
                                        video.draw(&mut render_src, video_card, bus, composite_enabled);
                                        render::resize_linear(
                                            &render_src, 
                                            video_data.render_w, 
                                            video_data.render_h, 
                                            pixels.get_frame_mut(), 
                                            video_data.aspect_w, 
                                            video_data.aspect_h);                            
                                    }
                                    false => {
                                        video.draw(pixels.get_frame_mut(), video_card, bus, composite_enabled);
                                    }
                                }                                
                            }
                            _ => panic!("Invalid combination of VideoType and RenderMode")
                        }
                    }
                    stat_counter.render_time = Instant::now() - render_start;

                    // Update egui data

                    // Is the machine in an error state? If so, display an error dialog.
                    if let Some(err) = machine.get_error_str() {
                        framework.gui.show_error(err);
                        framework.gui.show_window(GuiWindow::DiassemblyViewer);
                    }
                    else {
                        // No error? Make sure we close the error dialog.
                        framework.gui.clear_error();
                    }

                    // Handle custom user events received from our gui windows
                    loop {
                        match framework.gui.get_event() {

                            Some(GuiEvent::OptionChanged(opt, val)) => {
                                match (opt, val) {
                                    (GuiOption::CorrectAspect, false) => {
                                        // Aspect correction was turned off. We want to clear the render buffer as the 
                                        // display buffer is shrinking vertically.
                                        let surface = pixels.get_frame_mut();
                                        surface.fill(0);
                                    }
                                    (GuiOption::CpuEnableWaitStates, state) => {
                                        machine.set_cpu_option(CpuOption::EnableWaitStates(state));
                                    }
                                    (GuiOption::CpuInstructionHistory, state) => {
                                        machine.set_cpu_option(CpuOption::InstructionHistory(state));
                                    }
                                    (GuiOption::CpuTraceLoggingEnabled, state) => {
                                        machine.set_cpu_option(CpuOption::TraceLoggingEnabled(state));
                                    }
                                    (GuiOption::TurboButton, state) => {
                                        machine.set_turbo_mode(state);
                                    }
                                    _ => {}
                                }
                            }

                            Some(GuiEvent::CreateVHD(filename, fmt)) => {
                                log::info!("Got CreateVHD event: {:?}, {:?}", filename, fmt);

                                let vhd_path = Path::new("./hdd").join(filename);

                                match vhd::create_vhd(
                                    vhd_path.into_os_string(), 
                                    fmt.max_cylinders, 
                                    fmt.max_heads, 
                                    fmt.max_sectors) {

                                    Ok(_) => {
                                        // We don't actually do anything with the newly created file

                                        // Rescan dir to show new file in list
                                        if let Err(e) = vhd_manager.scan_dir("./hdd") {
                                            log::error!("Error scanning hdd directory: {}", e);
                                        };
                                    }
                                    Err(err) => {
                                        log::error!("Error creating VHD: {}", err);
                                    }
                                }
                            }
                            Some(GuiEvent::LoadFloppy(drive_select, filename)) => {
                                log::debug!("Load floppy image: {:?} into drive: {}", filename, drive_select);

                                match machine.floppy_manager().load_floppy_data(&filename) {
                                    Ok(vec) => {
                                        
                                        if let Some(fdc) = machine.fdc() {
                                            match fdc.load_image_from(drive_select, vec) {
                                                Ok(()) => {
                                                    log::info!("Floppy image successfully loaded into virtual drive.");
                                                }
                                                Err(err) => {
                                                    log::warn!("Floppy image failed to load: {}", err);
                                                }
                                            }
                                        }
                                    } 
                                    Err(e) => {
                                        log::error!("Failed to load floppy image: {:?} Error: {}", filename, e);
                                        // TODO: Some sort of GUI indication of failure
                                        eprintln!("Failed to read floppy image file: {:?} Error: {}", filename, e);
                                    }
                                }                                
                            }
                            Some(GuiEvent::EjectFloppy(drive_select)) => {
                                log::info!("Ejecting floppy in drive: {}", drive_select);
                                if let Some(fdc) = machine.fdc() {
                                    fdc.unload_image(drive_select);
                                }
                            }
                            Some(GuiEvent::BridgeSerialPort(port_name)) => {

                                log::info!("Bridging serial port: {}", port_name);
                                machine.bridge_serial_port(1, port_name);
                            }
                            Some(GuiEvent::DumpVRAM) => {
                                if let Some(video_card) = machine.videocard() {
                                    video_card.dump_mem();
                                }
                            }
                            Some(GuiEvent::DumpCS) => {
                                machine.cpu().dump_cs();
                            }
                            Some(GuiEvent::EditBreakpoint) => {
                                // Get breakpoints from GUI
                                let (bp_str, bp_mem_str) = framework.gui.get_breakpoints();

                                let mut breakpoints = Vec::new();

                                // Push exec breakpoint to list if valid hex
                                if let Ok(addr) = u32::from_str_radix(bp_str, 16) {
                                    if addr > 0 && addr < 0x100000 {
                                        breakpoints.push(BreakPointType::ExecuteFlat(addr));
                                    }
                                }
                            
                                // Push mem breakpoint to list if valid hex
                                if let Ok(addr) = u32::from_str_radix(bp_mem_str, 16) {
                                    if addr > 0 && addr < 0x100000 {
                                        breakpoints.push(BreakPointType::MemAccessFlat(addr));
                                    }
                                }                                     
                            
                                machine.set_breakpoints(breakpoints);
                            }
                            Some(GuiEvent::MemoryUpdate) => {
                                // The address bar for the memory viewer was updated. We need to 
                                // evaluate the expression and set a new row value for the control.
                                // The memory contents will be updated in the normal frame update.
                                let mem_dump_addr_str = framework.gui.memory_viewer.get_address();
                                // Show address 0 if expression evail fails
                                let mem_dump_addr: u32 = match machine.cpu().eval_address(&mem_dump_addr_str) {
                                    Some(i) => {
                                        let addr: u32 = i.into();
                                        addr & !0x0F
                                    }
                                    None => 0
                                };
                                framework.gui.memory_viewer.set_row(mem_dump_addr as usize);                                    
                            }
                            Some(GuiEvent::TokenHover(addr)) => {
                                // Hovered over a token in a TokenListView.
                                let debug = machine.bus_mut().get_memory_debug(addr);
                                framework.gui.memory_viewer.set_hover_text(format!("{}", debug));
                            }
                            Some(GuiEvent::FlushLogs) => {
                                // Request to flush trace logs.
                                machine.flush_trace_logs();
                            }
                            None => break,
                            _ => {
                                // Unhandled event?
                            }
                        }
                    }

                    // -- Update list of floppies
                    let name_vec = machine.floppy_manager().get_floppy_names();
                    framework.gui.set_floppy_names(name_vec);

                    // -- Update VHD Creator window
                    if framework.gui.is_window_open(egui::GuiWindow::VHDCreator) {
                        if let Some(hdc) = machine.hdc() {
                            framework.gui.update_vhd_formats(hdc.get_supported_formats());
                        }
                        else {
                            log::error!("Couldn't query available formats: No Hard Disk Controller present!");
                        }
                    }

                    // -- Update list of VHD images
                    let name_vec = vhd_manager.get_vhd_names();
                    framework.gui.set_vhd_names(name_vec);

                    // -- Do we have a new VHD image to load?
                    for i in 0..machine::NUM_HDDS {
                        if let Some(new_vhd_name) = framework.gui.get_new_vhd_name(i) {
                            log::debug!("Load new VHD image: {:?} in device: {}", new_vhd_name, i);

                            match vhd_manager.get_vhd_file(&new_vhd_name) {
                                Ok(vhd_file) => {

                                    match VirtualHardDisk::from_file(vhd_file) {
                                        Ok(vhd) => {

                                            if let Some(hdc) = machine.hdc() {
                                                match hdc.set_vhd(i as usize, vhd) {
                                                    Ok(_) => {
                                                        log::info!("VHD image {:?} successfully loaded into virtual drive: {}", new_vhd_name, i);
                                                    }
                                                    Err(err) => {
                                                        log::error!("Error mounting VHD: {}", err);
                                                    }
                                                }
                                            }
                                            else {
                                                log::error!("No Hard Disk Controller present!");
                                            }
                                        },
                                        Err(err) => {
                                            log::error!("Error loading VHD: {}", err);
                                        }
                                    }
                                }
                                Err(err) => {
                                    log::error!("Failed to load VHD image {:?}: {}", new_vhd_name, err);
                                }                                
                            }
                        }
                    }

                    // Update performance viewer
                    if framework.gui.is_window_open(egui::GuiWindow::PerfViewer) {
                        framework.gui.update_video_data(video_data);
                        framework.gui.update_perf_view(
                            stat_counter.ups,
                            stat_counter.fps,
                            stat_counter.emulated_fps,
                            stat_counter.current_cps,
                            stat_counter.current_ips,
                            stat_counter.emulation_time,
                            stat_counter.render_time
                        )
                    }

                    // -- Update memory viewer window if open
                    if framework.gui.is_window_open(egui::GuiWindow::MemoryViewer) {
                        let mem_dump_addr_str = framework.gui.memory_viewer.get_address();
                        // Show address 0 if expression evail fails
                        let mem_dump_addr: u32 = match machine.cpu().eval_address(&mem_dump_addr_str) {
                            Some(i) => {
                                let addr: u32 = i.into();
                                addr & !0x0F
                            }
                            None => 0
                        };

                        let mem_dump_vec = machine.bus().dump_flat_tokens(mem_dump_addr as usize, 256);
                    
                        //framework.gui.memory_viewer.set_row(mem_dump_addr as usize);
                        framework.gui.memory_viewer.set_memory(mem_dump_vec);
                    }   

                    // -- Update register viewer window
                    if framework.gui.is_window_open(egui::GuiWindow::CpuStateViewer) {
                        let cpu_state = machine.cpu().get_string_state();
                        framework.gui.cpu_viewer.update_state(cpu_state);
                    }

                    // -- Update PIT viewer window
                    if framework.gui.is_window_open(egui::GuiWindow::PitViewer) {
                        let pit_state = machine.pit_state();
                        framework.gui.update_pit_state(&pit_state);

                        let pit_data = machine.get_pit_buf();
                        framework.gui.pit_viewer.update_channel_data(2, &pit_data);
                    }

                    // -- Update PIC viewer window
                    if framework.gui.is_window_open(egui::GuiWindow::PicViewer) {
                        let pic_state = machine.pic_state();
                        framework.gui.update_pic_state(pic_state);
                    }

                    // -- Update PPI viewer window
                    if framework.gui.is_window_open(egui::GuiWindow::PpiViewer) {
                        let ppi_state_opt = machine.ppi_state();
                        if let Some(ppi_state) = ppi_state_opt {
                            framework.gui.update_ppi_state(ppi_state);  
                            // TODO: If no PPI, disable debug window
                        }
                    }

                    // -- Update DMA viewer window
                    if framework.gui.is_window_open(egui::GuiWindow::DmaViewer) {
                        let dma_state = machine.dma_state();
                        framework.gui.dma_viewer.update_state(dma_state);
                    }
                    
                    // -- Update VideoCard Viewer (Replace CRTC Viewer)
                    if framework.gui.is_window_open(egui::GuiWindow::VideoCardViewer) {
                        // Only have an update if we have a videocard to update.
                        if let Some(videocard_state) = machine.videocard_state() {
                            framework.gui.update_videocard_state(videocard_state);
                        }
                    }

                    // -- Update Instruction Trace window
                    if framework.gui.is_window_open(egui::GuiWindow::HistoryViewer) {
                        let trace = machine.cpu().dump_instruction_history_tokens();
                        framework.gui.trace_viewer.set_content(trace);
                    }

                    // -- Update Call Stack window
                    if framework.gui.is_window_open(egui::GuiWindow::CallStack) {
                        let stack = machine.cpu().dump_call_stack();
                        framework.gui.update_call_stack_state(stack);
                    }

                    // -- Update cycle trace viewer window
                    if framework.gui.is_window_open(egui::GuiWindow::CycleTraceViewer) {

                        if machine.get_cpu_option(CpuOption::TraceLoggingEnabled(true)) {
                            let trace_vec = machine.cpu().get_cycle_trace();
                            framework.gui.cycle_trace_viewer.update(trace_vec);
                        }
                    }

                    // -- Update disassembly viewer window
                    if framework.gui.is_window_open(egui::GuiWindow::DiassemblyViewer) {
                        let start_addr_str = framework.gui.disassembly_viewer.get_address();

                        // The expression evaluation could result in a segment:offset address or a flat address.
                        // The behavior of the viewer will differ slightly depending on whether we have segment:offset 
                        // information. Wrapping of segments can't be detected if the expression evaluates to a flat
                        // address.
                        let start_addr = machine.cpu().eval_address(&start_addr_str);
                        let start_addr_flat: u32 = match start_addr {
                            Some(i) => i.into(),
                            None => 0
                        };

                        let bus = machine.bus_mut();
                        
                        let mut listview_vec = Vec::new();

                        //let mut disassembly_string = String::new();
                        let mut disassembly_addr_flat = start_addr_flat as usize;
                        let mut disassembly_addr_seg = start_addr;

                        for _ in 0..24 {

                            if disassembly_addr_flat < machine::MAX_MEMORY_ADDRESS {

                                bus.seek(disassembly_addr_flat);

                                let mut decode_vec = Vec::new();

                                match Cpu::decode(bus) {
                                    Ok(i) => {
                                    
                                        let instr_slice = bus.get_slice_at(disassembly_addr_flat, i.size as usize);
                                        let instr_bytes_str = util::fmt_byte_array(instr_slice);
                                        
                                        decode_vec.push(SyntaxToken::MemoryAddressFlat(disassembly_addr_flat as u32, format!("{:05X}", disassembly_addr_flat)));

                                        let mut instr_vec = Cpu::tokenize_instruction(&i);

                                        //let decode_str = format!("{:05X} {:012} {}\n", disassembly_addr, instr_bytes_str, i);
                                        
                                        disassembly_addr_flat += i.size as usize;

                                        // If we have cs:ip, advance the offset. Wrapping of segment may provide different results 
                                        // from advancing flat address, so if a wrap is detected, adjust the flat address.
                                        if let Some(CpuAddress::Segmented(segment, offset)) = disassembly_addr_seg {

                                            decode_vec.push(SyntaxToken::MemoryAddressSeg16(segment, offset, format!("{:04X}:{:04X}", segment, offset)));

                                            let new_offset = offset.wrapping_add(i.size as u16);
                                            if new_offset < offset {
                                                // A wrap of the code segment occurred. Update the linear address to match.
                                                disassembly_addr_flat = Cpu::calc_linear_address(segment, new_offset) as usize;
                                            }

                                            disassembly_addr_seg = Some(CpuAddress::Segmented(segment, new_offset));
                                            //*offset = new_offset;
                                        }
                                        decode_vec.push(SyntaxToken::InstructionBytes(format!("{:012}", instr_bytes_str)));
                                        decode_vec.append(&mut instr_vec);
                                    }
                                    Err(_) => {
                                        decode_vec.push(SyntaxToken::ErrorString("INVALID".to_string()));
                                    }
                                };

                                //disassembly_string.push_str(&decode_str);
                                listview_vec.push(decode_vec);
                            }
                        }

                        //framework.gui.update_dissassembly_view(disassembly_string);
                        framework.gui.disassembly_viewer.set_content(listview_vec);
                    }

                    // Prepare egui
                    framework.prepare(&window);

                    // Render everything together
                    let render_result = pixels.render_with(|encoder, render_target, context| {

                        // Render the world texture
                        context.scaling_renderer.render(encoder, render_target);

                        // Render egui
                        #[cfg(not(feature = "pi_validator"))]
                        framework.render(encoder, render_target, context);

                        Ok(())
                    });

                    // Basic error handling
                    if render_result
                        .map_err(|e| error!("pixels.render() failed: {}", e))
                        .is_err()
                    {
                        *control_flow = ControlFlow::Exit;
                    }   
                }
            }
            
            Event::RedrawRequested(_) => {


            }
            _ => (),
        }
    });
}

pub fn main_headless(
    config: &ConfigFileParams,
    rom_manager: RomManager,
    floppy_manager: FloppyManager
) {

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
        config,
        config.machine.model,
        *machine_desc_opt.unwrap(),
        config.emulator.trace_mode,
        config.machine.video, 
        sp, 
        rom_manager, 
        floppy_manager,
    );

    // Load program binary if one was specified in config options
    if let Some(prog_bin) = &config.emulator.run_bin {

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
                eprintln!("Must specifiy program load offset.");
                std::process::exit(1);
            }
        }
        else {
            eprintln!("Must specifiy program load segment.");
            std::process::exit(1);  
        }
    }

    let mut exec_control = machine::ExecutionControl::new();
    exec_control.set_state(ExecutionState::Running);

    loop {
        // This should really return a Result
        machine.run(1000, &mut exec_control);
    }
    
    //std::process::exit(0);
}


#[cfg(feature = "cpu_validator")]
use std::{
    fs::File,
    io::{BufWriter, Write},
};
#[cfg(feature = "cpu_validator")]
use cpu_808x::{
    *,
    mnemonic::Mnemonic,
    CpuValidatorState
};
#[cfg(feature = "cpu_validator")]
use cpu_common::CpuType;

#[cfg(feature = "cpu_validator")]
pub fn main_fuzzer <'a>(
    config: &ConfigFileParams,
    _rom_manager: RomManager,
    _floppy_manager: FloppyManager
) {

    let mut trace_file_option: Box<dyn Write + 'a> = Box::new(std::io::stdout());
    if config.emulator.trace_mode != TraceMode::None {
        // Open the trace file if specified
        if let Some(filename) = &config.emulator.trace_file {
            match File::create(filename) {
                Ok(file) => {
                    trace_file_option = Box::new(BufWriter::new(file));
                },
                Err(e) => {
                    eprintln!("Couldn't create specified tracelog file: {}", e);
                }
            }
        }
    }

    //let mut io_bus = IoBusInterface::new();
    let pic = Rc::new(RefCell::new(pic::Pic::new()));    

    let mut cpu = Cpu::new(
        CpuType::Intel8088,
        config.emulator.trace_mode,
        Some(trace_file_option),
        #[cfg(feature = "cpu_validator")]
        config.validator.vtype.unwrap()
    );

    cpu.randomize_seed(1234);
    cpu.randomize_mem();

    let mut test_num = 0;

    'testloop: loop {

        test_num += 1;
        cpu.randomize_regs();

        if cpu.get_register16(Register16::IP) > 0xFFF0 {
            // Avoid IP wrapping issues for now
            continue;
        }

        // Generate specific opcodes (optional)

        // ALU ops
        
        /*
        cpu.random_inst_from_opcodes(
            &[
                0x00, 0x01, 0x02, 0x03, 0x04, 0x05, // ADD
                0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, // OR
                0x10, 0x11, 0x12, 0x13, 0x14, 0x15, // ADC
                0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, // SBB
                0x20, 0x21, 0x22, 0x23, 0x24, 0x25, // AND
                0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, // SUB
                0x30, 0x31, 0x32, 0x33, 0x34, 0x35, // XOR
                0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, // CMP
            ]
        );
        */
        // Completed 5000 tests
        

        //cpu.random_inst_from_opcodes(&[0x06, 0x07, 0x0E, 0x0F, 0x16, 0x17, 0x1E, 0x1F]); // PUSH/POP - completed 5000 tests
        //cpu.random_inst_from_opcodes(&[0x27, 0x2F, 0x37, 0x3F]); // DAA, DAS, AAA, AAS

        //cpu.random_inst_from_opcodes(&[0x90]);

        /*
        // INC & DEC
        cpu.random_inst_from_opcodes(
            &[
                0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47,
                0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F,
            ]
        );
        */

        /*
        // PUSH & POP
        cpu.random_inst_from_opcodes(
            &[
                0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57,
                0x58, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F,
            ]
        );
        */

        /*
        // Relative jumps
        cpu.random_inst_from_opcodes(
            &[
                0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77,
                0x78, 0x79, 0x7A, 0x7B, 0x7C, 0x7D, 0x7E, 0x7F,
            ]
        );
        */
        
        //cpu.random_inst_from_opcodes(&[0x80, 0x81, 82, 83]); // ALU imm8, imm16, and imm8s
        //cpu.random_inst_from_opcodes(&[0x84, 0x85]); // TEST 8 & 16 bit
        //cpu.random_inst_from_opcodes(&[0x86, 0x87]); // XCHG 8 & 16 bit
        //cpu.random_inst_from_opcodes(&[0x88, 0x89, 0x8A, 0x8B]); // MOV various
        //cpu.random_inst_from_opcodes(&[0x8D]); // LEA
        //cpu.random_inst_from_opcodes(&[0x8C, 0x8E]); // MOV Sreg

        //cpu.random_inst_from_opcodes(&[0x8F]); // POP  (Weird behavior when REG != 0)

        cpu.random_inst_from_opcodes(&[0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97]); // XCHG reg, ax
        //cpu.random_inst_from_opcodes(&[0x98, 0x99]); // CBW, CWD
        //cpu.random_inst_from_opcodes(&[0x9A]); // CALLF
        //cpu.random_inst_from_opcodes(&[0x9C, 0x9D]); // PUSHF, POPF
        //cpu.random_inst_from_opcodes(&[0x9E, 0x9F]); // SAHF, LAHF
        //cpu.random_inst_from_opcodes(&[0xA0, 0xA1, 0xA2, 0xA3]); // MOV offset
        
        //cpu.random_inst_from_opcodes(&[0xA4, 0xA5]); // MOVS
        //cpu.random_inst_from_opcodes(&[0xAC, 0xAD]); // LODS

        //cpu.random_inst_from_opcodes(&[0xA6, 0xA7]); // CMPS
        //cpu.random_inst_from_opcodes(&[0xAE, 0xAF]); // SCAS

        //cpu.random_inst_from_opcodes(&[0xA8, 0xA9]); // TEST
        
        //cpu.random_inst_from_opcodes(&[0xAA, 0xAB]); // STOS
        
        // MOV imm
        /*
        cpu.random_inst_from_opcodes(
            &[
                0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 
                0xB8, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE, 0xBF
            ]
        );
        */

        //cpu.random_inst_from_opcodes(&[0xC0, 0xC1, 0xC2, 0xC3]); // RETN
        //cpu.random_inst_from_opcodes(&[0xC4]); // LES
        //cpu.random_inst_from_opcodes(&[0xC5]); // LDS
        //cpu.random_inst_from_opcodes(&[0xC6, 0xC7]); // MOV r/m, imm
        //cpu.random_inst_from_opcodes(&[0xC8, 0xC9, 0xCA, 0xCB]); // RETF
        //cpu.random_inst_from_opcodes(&[0xCC]); // INT3
        //cpu.random_inst_from_opcodes(&[0xCD]); // INT
        //cpu.random_inst_from_opcodes(&[0xCE]); // INT0
        //cpu.random_inst_from_opcodes(&[0xCF]); // IRET  ** unaccounted for cycle after FLUSH
        
        //cpu.random_inst_from_opcodes(&[0xD0, 0xD1]); // Misc bitshift ops, 1
        //cpu.random_inst_from_opcodes(&[0xD2]); // Misc bitshift ops, cl

        //cpu.random_inst_from_opcodes(&[0xD4]); // AAM
        //cpu.random_inst_from_opcodes(&[0xD5]); // AAD
        //cpu.random_inst_from_opcodes(&[0xD6]); // SALC
        //cpu.random_inst_from_opcodes(&[0xD7]); // XLAT
        //cpu.random_inst_from_opcodes(&[0xD8, 0xD9, 0xDA, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF]); // ESC

        //cpu.random_inst_from_opcodes(&[0xE0, 0xE1, 0xE2, 0xE3]); // LOOP & JCXZ
        //cpu.random_inst_from_opcodes(&[0xE8, 0xE9, 0xEA, 0xEB]); // CALL & JMP

        //cpu.random_inst_from_opcodes(&[0xF5]); // CMC

        //cpu.random_grp_instruction(0xF6, &[0, 1, 2, 3]); // 8 bit TEST, NOT & NEG
        //cpu.random_grp_instruction(0xF7, &[0, 1, 2, 3]); // 16 bit TEST, NOT & NEG
        //cpu.random_grp_instruction(0xF6, &[4, 5]); // 8 bit MUL & IMUL
        //cpu.random_grp_instruction(0xF7, &[4, 5]); // 16 bit MUL & IMUL
          
        //cpu.random_grp_instruction(0xF6, &[6, 7]); // 8 bit DIV & IDIV
        //cpu.random_grp_instruction(0xF7, &[6, 7]); // 16 bit DIV & IDIV

        //cpu.random_inst_from_opcodes(&[0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD]); // CLC, STC, CLI, STI, CLD, STD

        //cpu.random_grp_instruction(0xFE, &[0, 1]); // 8 bit INC & DEC
        //cpu.random_grp_instruction(0xFF, &[0, 1]); // 16 bit INC & DEC
        
        //cpu.random_grp_instruction(0xFE, &[2, 3]); // CALL & CALLF
        //cpu.random_grp_instruction(0xFF, &[2, 3]); // CALL & CALLF
        //cpu.random_grp_instruction(0xFE, &[4, 5]); // JMP & JMPF
        //cpu.random_grp_instruction(0xFF, &[4, 5]); // JMP & JMPF
        //cpu.random_grp_instruction(0xFE, &[6, 7]); // 8-bit broken PUSH & POP
        //cpu.random_grp_instruction(0xFF, &[6, 7]); // PUSH & POP

        // Decode this instruction
        let instruction_address = 
            Cpu::calc_linear_address(
                cpu.get_register16(Register16::CS),  
                cpu.get_register16(Register16::IP)
            );

        cpu.bus_mut().seek(instruction_address as usize);
        let (opcode, _cost) = cpu.bus_mut().read_u8(instruction_address as usize, 0).expect("mem err");

        let mut i = match Cpu::decode(cpu.bus_mut()) {
            Ok(i) => i,
            Err(_) => {
                log::error!("Instruction decode error, skipping...");
                continue;
            }                
        };
        
        // Skip N successful instructions

        // was at 13546
        if test_num < 0 {
            continue;
        }


        if test_num > 3 {
            return;

        }
        match i.opcode {
            0xFE | 0xD2 | 0xD3 | 0x8F => {
                continue;
            }
            _ => {}
        }

        let mut rep = false;
        match i.mnemonic {
            Mnemonic::INT | Mnemonic::INT3 | Mnemonic::INTO | Mnemonic::IRET => {
                continue;
            },
            Mnemonic::FWAIT => {
                continue;
            }
            Mnemonic::POPF => {
                // POPF can set trap flag which messes up the validator
                continue;
            }
            Mnemonic::LDS | Mnemonic::LES | Mnemonic::LEA => {
                if let OperandType::Register16(_) = i.operand2_type {
                    // Invalid forms end up using the last calculated EA. However this will differ between
                    // the validator and CPU due to the validator setup routine.
                    continue;
                }
            }
            Mnemonic::HLT => {
                // For obvious reasons
                continue;
            }
            /*
            Mnemonic::AAM | Mnemonic::DIV | Mnemonic::IDIV => {
                // Timings on these will take some work 
                continue;
            }
            */
            Mnemonic::MOVSB | Mnemonic::MOVSW | Mnemonic::CMPSB | Mnemonic::CMPSW | Mnemonic::STOSB | 
            Mnemonic::STOSW | Mnemonic::LODSB | Mnemonic::LODSW | Mnemonic::SCASB | Mnemonic::SCASW => {
                // limit cx to 31.
                cpu.set_register16(Register16::CX, cpu.get_register16(Register16::CX) % 32);

                rep = true;
            }
            
            Mnemonic::SETMO | Mnemonic::SETMOC | Mnemonic::ROL | Mnemonic::ROR | 
            Mnemonic::RCL | Mnemonic::RCR | Mnemonic::SHL | Mnemonic::SHR | Mnemonic::SAR => {
                // Limit cl to 0-31.
                cpu.set_register8(Register8::CL, cpu.get_register8(Register8::CL) % 32);
            }
            _=> {}
        }

        i.address = instruction_address;
   
        log::trace!("Test {}: Validating instruction: {} op:{:02X} @ [{:05X}]", test_num, i, opcode, i.address);
        
        // Set terminating address for CPU validator.
        cpu.set_end_address((i.address + i.size) as usize);

        // We loop here to handle REP string instructions, which are broken up into 1 effective instruction
        // execution per iteration. The 8088 makes no such distinction.
        loop {
            match cpu.step(false) {
                Ok((_, cycles)) => {
                    log::trace!("Instruction reported {} cycles", cycles);

                    if rep & cpu.in_rep() {
                        continue
                    }
                    break;
                },
                Err(err) => {
                    log::error!("CPU Error: {}\n", err);
                    break 'testloop;
                } 
            }
        }

        cpu.reset();

    }
    
    //std::process::exit(0);
}
/*
    MartyPC Emulator 
    (C)2023 Daniel Balsom
    https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

    --------------------------------------------------------------------------

    main.rs

    Main emulator entrypoint

*/

#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::too_many_arguments)]
#![forbid(unsafe_code)]

use std::{
    time::{Duration, Instant},
    cell::RefCell,
    rc::Rc,
    ffi::OsString,
    path::PathBuf
};

mod egui;

#[cfg(feature = "arduino_validator")]
mod main_fuzzer;

use crate::egui::{Framework, DeviceSelection};

use log::error;
use pixels::{Pixels, SurfaceTexture};

use winit::{
    dpi::LogicalSize,
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

#[cfg(feature = "arduino_validator")]
use crate::main_fuzzer::main_fuzzer;

use marty_core::{
    breakpoints::BreakPointType,
    config::{self, *},
    machine::{self, Machine, MachineState, ExecutionControl, ExecutionState},
    cpu_808x::{Cpu, CpuAddress},
    cpu_common::CpuOption,
    rom_manager::{RomManager, RomError, RomFeature},
    floppy_manager::{FloppyManager, FloppyError},
    machine_manager::MACHINE_DESCS,
    vhd_manager::{VHDManager, VHDManagerError},
    vhd::{self, VirtualHardDisk},
    videocard::{RenderMode},
    bytequeue::ByteQueue,
    sound::SoundPlayer,
    syntax_token::SyntaxToken,
    input::{
        self,
        MouseButton
    },
    util
};


use crate::egui::{GuiEvent, GuiOption , GuiWindow, PerformanceStats};
use marty_render::{VideoData, VideoRenderer, CompositeParams, ResampleContext};

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

fn main() {

    env_logger::init();

    let mut features = Vec::new();

    // Read config file
    let mut config = match config::get_config("./martypc.toml"){
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
    let mut video = VideoRenderer::new(config.machine.video);

    // Init graphics & GUI 
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let window = {
        let size = LogicalSize::new(WINDOW_WIDTH as f64, WINDOW_HEIGHT as f64);
        WindowBuilder::new()
            .with_title(format!("MartyPC {}", env!("CARGO_PKG_VERSION")))
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };
    
    // Load icon file.
    let mut icon_path = PathBuf::new();
    icon_path.push(config.emulator.basedir.clone());
    icon_path.push("icon.png");

    if let Ok(image) = image::open(icon_path.clone()) {

        let rgba8 = image.into_rgba8();
        let (width, height) = rgba8.dimensions();
        let icon_raw = rgba8.into_raw();
        
        let icon = winit::window::Icon::from_rgba(icon_raw, width, height).unwrap();
        window.set_window_icon(Some(icon));
    }
    else {
        log::error!("Couldn't load icon: {}", icon_path.display());
    }

    // ExecutionControl is shared via RefCell with GUI so that state can be updated by control widget
    let exec_control = Rc::new(RefCell::new(ExecutionControl::new()));

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

    // Create resampling context
    let mut resample_context = ResampleContext::new();

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
                exec_control.clone(),
                config.gui.theme_color
            );

        (pixels, framework)
    };

    let adapter_info = pixels.adapter().get_info();
    let backend_str = format!("{:?}", adapter_info.backend);
    let adapter_name_str =  format!("{}", adapter_info.name);
    log::debug!("wgpu using adapter: {}, backend: {}", adapter_name_str, backend_str);
    
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
        rom_manager
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
        framework.gui.set_window_open(GuiWindow::DisassemblyViewer, true);
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
                        marty_render::AspectRatio{ h: 4, v: 3 }
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

                VideoRenderer::set_alpha(pixels.frame_mut(), pixel_buf_w, pixel_buf_h, 255);
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

                // Recalculate sampling parameters.
                resample_context.precalc(aper_x, aper_y, aper_correct_x, aper_correct_y);

                // Update internal state and request a redraw
                window.request_redraw();
            }
        }
    }
        
    // Try to load default vhd for drive0: 
    if let Some(vhd_name) = config.machine.drive0 {
        let vhd_os_name: OsString = vhd_name.into();
        match vhd_manager.load_vhd_file(0, &vhd_os_name) {
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

    // Try to load default vhd for drive1: 
    // TODO: refactor this to func or put in vhd_manager
    if let Some(vhd_name) = config.machine.drive1 {
        let vhd_os_name: OsString = vhd_name.into();
        match vhd_manager.load_vhd_file(1, &vhd_os_name) {
            Ok(vhd_file) => {
                match VirtualHardDisk::from_file(vhd_file) {
                    Ok(vhd) => {
                        if let Some(hdc) = machine.hdc() {
                            match hdc.set_vhd(1_usize, vhd) {
                                Ok(_) => {
                                    log::info!("VHD image {:?} successfully loaded into virtual drive: {}", vhd_os_name, 1);
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
                    let system_ticks = machine.system_ticks();

                    stat_counter.current_cpu_cps = cpu_cycles - stat_counter.last_cpu_cycles;
                    stat_counter.last_cpu_cycles = cpu_cycles;

                    stat_counter.current_pit_tps = pit_ticks - stat_counter.last_pit_ticks;
                    stat_counter.last_pit_ticks = pit_ticks;

                    stat_counter.current_sys_tps = system_ticks - stat_counter.last_system_ticks;
                    stat_counter.last_system_ticks = system_ticks;

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

                            // TODO: The vertical delta hack was used for area 8088mph for the old style of rendering.
                            // Now that we render into a fixed frame, we should refactor this
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
                                
                                // Recalculate sampling factors
                                resample_context.precalc(
                                    video_data.render_w, 
                                    video_data.render_h, 
                                    video_data.aspect_w,
                                    video_data.aspect_h
                                );

                                pixels.frame_mut().fill(0);

                                if let Err(e) = pixels.resize_buffer(video_data.aspect_w, video_data.aspect_h) {
                                    log::error!("Failed to resize pixel pixel buffer: {}", e);
                                }

                                VideoRenderer::set_alpha(pixels.frame_mut(), video_data.aspect_w, video_data.aspect_h, 255);
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

                        let beam_pos;
                        let video_buffer;
                        // Get the appropriate buffer depending on run mode. If execution is paused 
                        // (debugging) show the back buffer instead of front buffer. 
                        // TODO: Discriminate between paused in debug mode vs user paused state
                        // TODO: buffer and extents may not match due to extents being for front buffer
                        match exec_control.borrow_mut().get_state() {
                            ExecutionState::Paused | ExecutionState::BreakpointHit | ExecutionState::Halted => {
                                if framework.gui.get_option(GuiOption::ShowBackBuffer).unwrap_or(false) {
                                    video_buffer = video_card.get_back_buf();
                                }
                                else {
                                    video_buffer = video_card.get_display_buf();
                                }
                                beam_pos = video_card.get_beam_pos();
                            }
                            _ => {
                                video_buffer = video_card.get_display_buf();
                                beam_pos = None;
                            }
                        }

                        // Get the render mode from the device and render appropriately
                        match (video_card.get_video_type(), video_card.get_render_mode()) {

                            (VideoType::CGA, RenderMode::Direct) => {
                                // Draw device's front buffer in direct mode (CGA only for now)

                                match aspect_correct {
                                    true => {
                                        video.draw_cga_direct(
                                            &mut render_src,
                                            video_data.render_w, 
                                            video_data.render_h,                                             
                                            video_buffer,
                                            video_card.get_display_extents(),
                                            composite_enabled,
                                            &video_data.composite_params,
                                            beam_pos
                                        );

                                        /*
                                        marty_render::resize_linear(
                                            &render_src, 
                                            video_data.render_w, 
                                            video_data.render_h, 
                                            pixels.frame_mut(), 
                                            video_data.aspect_w, 
                                            video_data.aspect_h,
                                            &resample_context
                                        );
                                        */
                                        marty_render::resize_linear_fast(
                                            &mut render_src, 
                                            video_data.render_w, 
                                            video_data.render_h, 
                                            pixels.frame_mut(), 
                                            video_data.aspect_w, 
                                            video_data.aspect_h,
                                            &mut resample_context
                                        );

                                    }
                                    false => {
                                        video.draw_cga_direct(
                                            pixels.frame_mut(),
                                            video_data.render_w, 
                                            video_data.render_h,                                                                                         
                                            video_buffer,
                                            video_card.get_display_extents(),
                                            composite_enabled,
                                            &video_data.composite_params,
                                            beam_pos                                         
                                        );
                                    }
                                }
                            }
                            (_, RenderMode::Indirect) => {
                                // Draw VRAM in indirect mode
                                match aspect_correct {
                                    true => {
                                        video.draw(&mut render_src, video_card, bus, composite_enabled);
                                        marty_render::resize_linear(
                                            &render_src, 
                                            video_data.render_w, 
                                            video_data.render_h, 
                                            pixels.frame_mut(), 
                                            video_data.aspect_w, 
                                            video_data.aspect_h,
                                            &resample_context
                                        );                            
                                    }
                                    false => {
                                        video.draw(pixels.frame_mut(), video_card, bus, composite_enabled);
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
                        framework.gui.show_window(GuiWindow::DisassemblyViewer);
                    }
                    else {
                        // No error? Make sure we close the error dialog.
                        framework.gui.clear_error();
                    }

                    // Handle custom events received from our GUI
                    loop {
                        if let Some(gui_event) = framework.gui.get_event() {
                            match gui_event {
                                GuiEvent::Exit => {
                                    // User chose exit option from menu. Shut down.
                                    // TODO: Add a timeout from last VHD write for safety?
                                    println!("Thank you for using MartyPC!");
                                    *control_flow = ControlFlow::Exit;
                                }
                                GuiEvent::SetNMI(state) => {
                                    // User wants to crash the computer. Sure, why not.
                                    machine.set_nmi(state);
                                }
                                GuiEvent::OptionChanged(opt, val) => {
                                    match (opt, val) {
                                        (GuiOption::CorrectAspect, false) => {
                                            // Aspect correction was turned off. We want to clear the render buffer as the 
                                            // display buffer is shrinking vertically.
                                            let surface = pixels.frame_mut();
                                            surface.fill(0);
                                            VideoRenderer::set_alpha(surface, video_data.aspect_w, video_data.aspect_h, 255);
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
    
                                GuiEvent::CreateVHD(filename, fmt) => {
                                    log::info!("Got CreateVHD event: {:?}, {:?}", filename, fmt);
    
                                    let vhd_path = hdd_path.join(filename);
    
                                    match vhd::create_vhd(
                                        vhd_path.into_os_string(), 
                                        fmt.max_cylinders, 
                                        fmt.max_heads, 
                                        fmt.max_sectors) {
    
                                        Ok(_) => {
                                            // We don't actually do anything with the newly created file
    
                                            // Rescan dir to show new file in list
                                            if let Err(e) = vhd_manager.scan_dir(&hdd_path) {
                                                log::error!("Error scanning hdd directory: {}", e);
                                            };
                                        }
                                        Err(err) => {
                                            log::error!("Error creating VHD: {}", err);
                                        }
                                    }
                                }
                                GuiEvent::RescanMediaFolders => {
                                    if let Err(e) = floppy_manager.scan_dir(&floppy_path) {
                                        log::error!("Error scanning floppy directory: {}", e);
                                    }
                                    if let Err(e) = vhd_manager.scan_dir(&hdd_path) {
                                        log::error!("Error scanning hdd directory: {}", e);
                                    };
                                }
                                GuiEvent::LoadFloppy(drive_select, filename) => {
                                    log::debug!("Load floppy image: {:?} into drive: {}", filename, drive_select);
    
                                    match floppy_manager.load_floppy_data(&filename) {
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
                                GuiEvent::EjectFloppy(drive_select) => {
                                    log::info!("Ejecting floppy in drive: {}", drive_select);
                                    if let Some(fdc) = machine.fdc() {
                                        fdc.unload_image(drive_select);
                                    }
                                }
                                GuiEvent::BridgeSerialPort(port_name) => {
    
                                    log::info!("Bridging serial port: {}", port_name);
                                    machine.bridge_serial_port(1, port_name);
                                }
                               GuiEvent::DumpVRAM => {
                                    if let Some(video_card) = machine.videocard() {
                                        let mut dump_path = PathBuf::new();
                                        dump_path.push(config.emulator.basedir.clone());
                                        dump_path.push("dumps");
                                        video_card.dump_mem(&dump_path);
                                    }
                                }
                                GuiEvent::DumpCS => {
                                    let mut dump_path = PathBuf::new();
                                    dump_path.push(config.emulator.basedir.clone());
                                    dump_path.push("dumps");
                                                                    
                                    machine.cpu().dump_cs(&dump_path);
                                }
                                GuiEvent::DumpAllMem => {
                                    let mut dump_path = PathBuf::new();
                                    dump_path.push(config.emulator.basedir.clone());
                                    dump_path.push("dumps");
                                                                                                    
                                    machine.bus().dump_mem(&dump_path);
                                }
                                GuiEvent::EditBreakpoint => {
                                    // Get breakpoints from GUI
                                    let (bp_str, bp_mem_str, bp_int_str) = framework.gui.get_breakpoints();
    
                                    let mut breakpoints = Vec::new();
    
                                    // Push exec breakpoint to list if valid expression
                                    if let Some(addr) = machine.cpu().eval_address(&bp_str) {
                                        let flat_addr = u32::from(addr);
                                        if flat_addr > 0 && flat_addr < 0x100000 {
                                            breakpoints.push(BreakPointType::ExecuteFlat(flat_addr));
                                        }
                                    };
                                
                                    // Push mem breakpoint to list if valid expression
                                    if let Some(addr) = machine.cpu().eval_address(&bp_mem_str) {
                                        let flat_addr = u32::from(addr);
                                        if flat_addr > 0 && flat_addr < 0x100000 {
                                            breakpoints.push(BreakPointType::MemAccessFlat(flat_addr));
                                        }
                                    }
                                
                                    // Push int breakpoint to list 
                                    if let Ok(iv) = u32::from_str_radix(bp_int_str, 10) {
                                        if iv < 256 {
                                            breakpoints.push(BreakPointType::Interrupt(iv as u8));
                                        }
                                    }

                                    machine.set_breakpoints(breakpoints);
                                }
                                GuiEvent::MemoryUpdate => {
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
                                GuiEvent::TokenHover(addr) => {
                                    // Hovered over a token in a TokenListView.
                                    let debug = machine.bus_mut().get_memory_debug(addr);
                                    framework.gui.memory_viewer.set_hover_text(format!("{}", debug));
                                }
                                GuiEvent::FlushLogs => {
                                    // Request to flush trace logs.
                                    machine.flush_trace_logs();
                                }
                                GuiEvent::DelayAdjust => {
                                    let delay_params = framework.gui.delay_adjust.get_params();
    
                                    machine.set_cpu_option(CpuOption::DramRefreshAdjust(delay_params.dram_delay));
                                    machine.set_cpu_option(CpuOption::HaltResumeDelay(delay_params.halt_resume_delay));
                                }
                                GuiEvent::TickDevice(dev, ticks) => {
                                    match dev {
                                        DeviceSelection::Timer(_t) => {
    
                                        }
                                        DeviceSelection::VideoCard => {
                                            if let Some(video_card) = machine.videocard() {
                                                video_card.debug_tick(ticks);
                                            }                                        
                                        }
                                    }
                                }
                                GuiEvent::MachineStateChange(state) => {
    
                                    match state {
                                        MachineState::Off | MachineState::Rebooting => {
                                            // Clear the screen if rebooting or turning off
                                            render_src.fill(0);
                                        }
                                        _ => {}
                                    }
                                    machine.change_state(state);
                                }
                                GuiEvent::TakeScreenshot => {
                                    let mut screenshot_path = PathBuf::new();
                                    screenshot_path.push(config.emulator.basedir.clone());
                                    screenshot_path.push("screenshots");

                                    video.screenshot(
                                        &mut render_src,
                                        video_data.render_w, 
                                        video_data.render_h, 
                                        &screenshot_path
                                    );

                                }
                                GuiEvent::CtrlAltDel => {
                                    machine.ctrl_alt_del();
                                }
                                _ => {}
                            }
                        }
                        else {
                            break;
                        }
                    }

                    // -- Update machine state
                    framework.gui.set_machine_state(machine.get_state());

                    // -- Update list of floppies
                    let name_vec = floppy_manager.get_floppy_names();
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

                            log::debug!("Releasing VHD slot: {}", i);
                            vhd_manager.release_vhd(i as usize);

                            log::debug!("Load new VHD image: {:?} in device: {}", new_vhd_name, i);

                            match vhd_manager.load_vhd_file(i as usize, &new_vhd_name) {
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
                        framework.gui.perf_viewer.update_video_data(video_data);
                        framework.gui.perf_viewer.update_stats(
                            &PerformanceStats {
                                adapter: adapter_name_str.clone(),
                                backend: backend_str.clone(),
                                current_ups: stat_counter.ups,
                                current_fps: stat_counter.fps,
                                emulated_fps: stat_counter.emulated_fps,
                                cycle_target: stat_counter.cycle_target,
                                current_cps: stat_counter.current_cps,
                                current_tps: stat_counter.current_sys_tps,
                                current_ips: stat_counter.current_ips,
                                emulation_time: stat_counter.emulation_time,
                                render_time: stat_counter.render_time,
                                gui_time: Default::default()
                            }
                        )
                    }

                    // -- Update memory viewer window if open
                    if framework.gui.is_window_open(egui::GuiWindow::MemoryViewer) {
                        let mem_dump_addr_str = framework.gui.memory_viewer.get_address();
                        // Show address 0 if expression evail fails
                        let (addr, mem_dump_addr) = match machine.cpu().eval_address(&mem_dump_addr_str) {
                            Some(i) => {
                                let addr: u32 = i.into();
                                // Dump at 16 byte block boundaries
                                (addr, addr & !0x0F)
                            }
                            None => (0,0)
                        };

                        let mem_dump_vec = machine.bus().dump_flat_tokens(mem_dump_addr as usize, addr as usize, 256);
                    
                        //framework.gui.memory_viewer.set_row(mem_dump_addr as usize);
                        framework.gui.memory_viewer.set_memory(mem_dump_vec);
                    }   

                    // -- Update IVR viewer window if open
                    if framework.gui.is_window_open(egui::GuiWindow::IvrViewer) {
                        let vec = machine.bus_mut().dump_ivr_tokens();
                        framework.gui.ivr_viewer.set_content(vec);
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
                        framework.gui.pic_viewer.update_state(&pic_state);
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
                    if framework.gui.is_window_open(egui::GuiWindow::DisassemblyViewer) {
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
    _floppy_manager: FloppyManager
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

    let mut exec_control = ExecutionControl::new();
    exec_control.set_state(ExecutionState::Running);

    loop {
        // This should really return a Result
        machine.run(1000, &mut exec_control);
    }
    
    //std::process::exit(0);
}


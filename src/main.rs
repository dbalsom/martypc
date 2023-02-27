#![deny(clippy::all)]
#![forbid(unsafe_code)]

use std::{
    fs::{File, read},
    time::{Duration, Instant},
    cell::RefCell,
    rc::Rc,
    path::Path,
    ffi::OsString
};

use crate::gui::Framework;

use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::{
    Event, 
    WindowEvent, 
    DeviceEvent, 
    ElementState, 
    StartCause, 
    VirtualKeyCode
};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

#[path = "./devices/ega/mod.rs"]
mod ega;
#[path = "./devices/vga/mod.rs"]
mod vga;

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
mod gui;
mod gui_image;
mod hdc;
mod io;
mod interrupt;
mod machine;
mod memerror;
mod mouse;
mod pic;
mod pit;
mod ppi;
mod rom_manager;
mod serial;
mod sound;
mod util;

mod vhd;
mod vhd_manager;
mod video;
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

use config::{ConfigFileParams, MachineType, VideoType, HardDiskControllerType, ValidatorType, TraceMode};

use machine::{Machine, ExecutionState};
use cpu_808x::Cpu;
use rom_manager::{RomManager, RomError, RomFeature};
use floppy_manager::{FloppyManager, FloppyError};
use vhd_manager::{VHDManager, VHDManagerError};
use vhd::{VirtualHardDisk};
use bytequeue::ByteQueue;
use gui::GuiEvent;
use sound::SoundPlayer;

use io::{IoHandler, IoBusInterface};

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
const CYCLES_PER_FRAME: u32 = (cpu_808x::CPU_MHZ * 1000000.0 / FPS_TARGET) as u32;

// Rendering Stats
struct Counter {
    frame_count: u64,
    current_fps: u32,
    fps: u32,
    last_frame: Instant,
    last_sndbuf: Instant,
    last_second: Instant,
    last_cpu_cycles: u64,
    current_cpu_cps: u64,
    last_pit_ticks: u64,
    current_pit_tps: u64,
    emulation_time: Duration,
    render_time: Duration,
    accumulated_us: u128,
    cycle_target: u32,
}

impl Counter {
    fn new() -> Self {
        Self {
            frame_count: 0,
            current_fps: 0,
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
            cycle_target: CYCLES_PER_FRAME
        }
    }
}
struct MouseData {
    is_captured: bool,
    have_update: bool,
    l_button_was_pressed: bool,
    l_button_is_pressed: bool,
    r_button_was_pressed: bool,
    r_button_is_pressed: bool,
    frame_delta_x: f64,
    frame_delta_y: f64
}
impl MouseData {
    fn new() -> Self {
        Self {
            is_captured: false,
            have_update: false,
            l_button_was_pressed: false,
            l_button_is_pressed: false,
            r_button_was_pressed: false,
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
    aspect_h: u32
}

fn main() {

    env_logger::init();

    let mut features = Vec::new();

    // Read config file
    let config = match config::get_config("./marty.toml"){
        Ok(config) => config,
        Err(e) => {
            match e.downcast_ref::<std::io::Error>() {
                Some(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
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

    // Enumerate serial ports
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
    let video = video::Video::new();

    // Init graphics & GUI 
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let window = {
        let size = LogicalSize::new(WINDOW_WIDTH as f64, WINDOW_HEIGHT as f64);
        WindowBuilder::new()
            .with_title("Marty")
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
        aspect_h: 480
    };

    let (mut pixels, mut framework) = {
        let window_size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        let pixels = 
            Pixels::new(video_data.aspect_w, video_data.aspect_h, surface_texture).unwrap();
        let framework =
            Framework::new(window_size.width, window_size.height, scale_factor, &pixels, exec_control.clone());

        (pixels, framework)
    };

    // Set list of serial ports
    framework.gui.update_serial_ports(serial_ports);

    let mut stat_counter = Counter::new();

    // KB modifiers
    let mut kb_data = KeyboardData::new();
    // Mouse event struct
    let mut mouse_data = MouseData::new();

    // Init sound 
    // The cpal sound library uses generics to initialize depending on the SampleFormat type.
    // On Windows at least a sample type of f32 is typical, but just in case...
    let sample_fmt = SoundPlayer::get_sample_format();
    let mut sp = match sample_fmt {
        cpal::SampleFormat::F32 => SoundPlayer::new::<f32>(),
        cpal::SampleFormat::I16 => SoundPlayer::new::<i16>(),
        cpal::SampleFormat::U16 => SoundPlayer::new::<u16>(),
    };

    // Instantiate the main Machine data struct
    // Machine coordinates all the parts of the emulated computer
    let mut machine = Machine::new(
        &config,
        config.machine.model,
        config.emulator.trace_mode,
        config.machine.video, 
        sp, 
        rom_manager, 
        floppy_manager,
    );

    // Try to load default vhd
    if let Some(vhd_name) = config.machine.drive0 {
        let vhd_os_name: OsString = vhd_name.into();
        match vhd_manager.get_vhd_file(&vhd_os_name) {
            Ok(vhd_file) => {
                match VirtualHardDisk::from_file(vhd_file) {
                    Ok(vhd) => {
                        match machine.hdc().borrow_mut().set_vhd(0 as usize, vhd) {
                            Ok(_) => {
                                log::info!("VHD image {:?} successfully loaded into virtual drive: {}", vhd_os_name, 0);
                            }
                            Err(err) => {
                                log::error!("Error mounting VHD: {}", err);
                            }
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
                pixels.resize_surface(size.width, size.height);
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
                        // Button ID is a raw u32. How confident are we that the mouse buttons for the basic three button
                        // mouse are consistent across platforms?
                        // On Windows it appears the right mouse button is button 3 and the middle mouse button is button 2.

                        // A mouse click could be faster than one frame (pressed & released in 16.6ms), therefore mouse 
                        // clicks are 'sticky', if a button was pressed during the last update period it will be sent as
                        // pressed during virtual mouse update.
                        match (button, state) {
                            (1, ElementState::Pressed) => {
                                mouse_data.l_button_was_pressed = true;
                                mouse_data.l_button_is_pressed = true;
                                mouse_data.have_update = true;
                            },
                            (1, ElementState::Released) => {
                                mouse_data.l_button_is_pressed = false;
                                mouse_data.have_update = true;
                            },
                            (3, ElementState::Pressed) => {
                                mouse_data.r_button_was_pressed = true;
                                mouse_data.r_button_is_pressed = true;
                                mouse_data.have_update = true;
                            },
                            (3, ElementState::Released) => {
                                mouse_data.r_button_is_pressed = false;
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
                                    log::trace!("Control F10 pressed.");
                                    if !mouse_data.is_captured {
                                        match window.set_cursor_grab(true) {
                                            Ok(_) => mouse_data.is_captured = true,
                                            Err(e) => log::error!("Couldn't set cursor grab mode: {:?}", e)
                                        }
                                    }
                                    else {
                                        // Cursor is grabbed, ungrab
                                        match window.set_cursor_grab(false) {
                                            Ok(_) => mouse_data.is_captured = false,
                                            Err(e) => log::error!("Couldn't set cursor grab mode: {:?}", e)
                                        }                                        
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

                    stat_counter.fps = stat_counter.current_fps;
                    stat_counter.current_fps = 0;
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

                    // Get breakpoint from GUI
                    let bp_str = framework.gui.get_breakpoint();
                    let bp_addr = match u32::from_str_radix(bp_str, 16) {
                        Ok(addr) => addr,
                        Err(_) => 0
                    };

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

                    // Send any pending mouse update to machine if mouse is captured
                    if mouse_data.is_captured && mouse_data.have_update {
                        machine.mouse().update(
                            mouse_data.l_button_was_pressed,
                            mouse_data.r_button_was_pressed,
                            mouse_data.frame_delta_x,
                            mouse_data.frame_delta_y
                        );
                        // Reset mouse for next frame
                        mouse_data.reset();
                    }

                    // Emulate a frame worth of instructions
                    let emulation_start = Instant::now();
                    machine.run(stat_counter.cycle_target, &mut exec_control.borrow_mut(), bp_addr);
                    stat_counter.emulation_time = Instant::now() - emulation_start;

                    // Emulation time budget is 16ms - render time in ms - fudge factor
                    let render_time = stat_counter.render_time.as_millis();
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
                    else if emulation_time < emulation_time_allowed_ms {
                        // ignore spurious 0-duration emulation loops
                        if emulation_time > 0 {
                            // Emulation could be faster
                            
                            // Increase speed by half of scaling factor
                            let factor: f64 = (stat_counter.emulation_time.as_millis() as f64) / emulation_time_allowed_ms as f64;

                            let old_target = stat_counter.cycle_target;
                            let new_target = (stat_counter.cycle_target as f64 / factor) as u32;
                            stat_counter.cycle_target += (new_target - old_target) / 2;

                            if stat_counter.cycle_target > CYCLES_PER_FRAME {
                                // Comment to run as fast as possible
                                //stat_counter.cycle_target = CYCLES_PER_FRAME;
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
                    }

                    // Do per-frame updates (Serial port emulation)
                    machine.frame_update();

                    // Check if there was a resolution change
                    let (new_w, new_h) = machine.videocard().borrow().get_display_extents();
                    if new_w >= MIN_RENDER_WIDTH && new_h >= MIN_RENDER_HEIGHT {
                        if new_w != video_data.render_w || new_h != video_data.render_h {
                            // Resize buffers
                            log::info!("Setting internal resolution to ({},{})", new_w, new_h);
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

                            pixels.get_frame().fill(0);
                            pixels.resize_buffer(video_data.aspect_w, video_data.aspect_h);
                        }
                    }

                    // -- Draw video memory --
                    let composite_enabled = framework.gui.get_composite_enabled();
                    let aspect_correct = framework.gui.get_aspect_correct_enabled();

                    let render_start = Instant::now();

                    match aspect_correct {
                        true => {
                            video.draw(&mut render_src, machine.videocard(), machine.bus(), composite_enabled);
                            video::resize_linear(
                                &render_src, 
                                video_data.render_w, 
                                video_data.render_h, 
                                pixels.get_frame(), 
                                video_data.aspect_w, 
                                video_data.aspect_h);                            
                        }
                        false => {
                            video.draw(pixels.get_frame(), machine.videocard(), machine.bus(), composite_enabled);
                        }
                    }
                    stat_counter.render_time = Instant::now() - render_start;

                    // Update egui data

                    // Any errors?
                    if let Some(err) = machine.get_error_str() {
                        framework.gui.show_error(err);
                        framework.gui.show_disassembly_view();
                    }

                    // -- Handle egui "Events"
                    loop {
                        match framework.gui.get_event() {
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
                                        
                                        match machine.fdc().borrow_mut().load_image_from(drive_select, vec) {
                                            Ok(()) => {
                                                log::info!("Floppy image successfully loaded into virtual drive.");
                                            }
                                            Err(err) => {
                                                log::warn!("Floppy image failed to load: {}", err);
                                            }
                                        }
                                    } 
                                    Err(e) => {
                                        log::error!("Failed to load floppy image! {:?}", filename);
                                        // TODO: Some sort of GUI indication of failure
                                        eprintln!("Failed to read floppy image file: {:?}", filename);
                                    }
                                }                                
                            }
                            Some(GuiEvent::EjectFloppy(drive_select)) => {
                                log::info!("Ejecting floppy in drive: {}", drive_select);
                                machine.fdc().borrow_mut().unload_image(drive_select);
                            }
                            Some(GuiEvent::BridgeSerialPort(port_name)) => {

                                log::info!("Bridging serial port: {}", port_name);
                                machine.bridge_serial_port(1, port_name);
                            }
                            Some(GuiEvent::DumpVRAM) => {
                                machine.videocard().borrow().dump_mem();
                            }
                            Some(GuiEvent::DumpCS) => {
                                machine.cpu().dump_cs();
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
                    if framework.gui.is_window_open(gui::GuiWindow::VHDCreator) {
                        framework.gui.update_vhd_formats(machine.hdc().borrow_mut().get_supported_formats());
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
                                            match machine.hdc().borrow_mut().set_vhd(i as usize, vhd) {
                                                Ok(_) => {
                                                    log::info!("VHD image {:?} successfully loaded into virtual drive: {}", new_vhd_name, i);
                                                }
                                                Err(err) => {
                                                    log::error!("Error mounting VHD: {}", err);
                                                }
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
                    if framework.gui.is_window_open(gui::GuiWindow::PerfViewer) {
                        framework.gui.update_video_data(video_data.clone());
                        framework.gui.update_perf_view(
                            stat_counter.fps,
                            stat_counter.emulation_time.as_millis() as u32,
                            stat_counter.render_time.as_millis() as u32
                        )
                    }

                    // -- Update memory viewer window if open
                    if framework.gui.is_window_open(gui::GuiWindow::MemoryViewer) {
                        let mem_dump_addr_str = framework.gui.get_memory_view_address();
                        // Show address 0 if expression evail fails
                        let mem_dump_addr = match machine.cpu().eval_address(mem_dump_addr_str) {
                            Some(i) => i,
                            None => 0
                        };
                        let mem_dump_str = machine.bus().dump_flat(mem_dump_addr as usize, 256);

                        framework.gui.update_memory_view(mem_dump_str);
                    }   

                    // -- Update register viewer window
                    if framework.gui.is_window_open(gui::GuiWindow::CpuStateViewer) {
                        let cpu_state = machine.cpu().get_string_state();
                        framework.gui.update_cpu_state(cpu_state);
                    }

                    // -- Update PIT viewer window
                    if framework.gui.is_window_open(gui::GuiWindow::PitViewer) {
                        let pit_state = machine.pit_state();
                        framework.gui.update_pit_state(pit_state);
                    }

                    // -- Update PIC viewer window
                    if framework.gui.is_window_open(gui::GuiWindow::PicViewer) {
                        let pic_state = machine.pic_state();
                        framework.gui.update_pic_state(pic_state);
                    }

                    // -- Update PPI viewer window
                    if framework.gui.is_window_open(gui::GuiWindow::PpiViewer) {
                        let ppi_state = machine.ppi_state();
                        framework.gui.update_ppi_state(ppi_state);  
                    }

                    // -- Update DMA viewer window
                    if framework.gui.is_window_open(gui::GuiWindow::DmaViewer) {
                        let dma_state = machine.dma_state();
                        framework.gui.update_dma_state(dma_state);
                    }
                    
                    // -- Update VideoCard Viewer (Replace CRTC Viewer)
                    if framework.gui.is_window_open(gui::GuiWindow::VideoCardViewer) {
                        let videocard_state = machine.videocard_state();
                        framework.gui.update_videocard_state(videocard_state);
                    }

                    // -- Update Instruction Trace window
                    if framework.gui.is_window_open(gui::GuiWindow::TraceViewer) {
                        let trace = machine.cpu().dump_instruction_history();
                        framework.gui.update_trace_state(trace);
                    }

                    // -- Update Call Stack window
                    if framework.gui.is_window_open(gui::GuiWindow::CallStack) {
                        let stack = machine.cpu().dump_call_stack();
                        framework.gui.update_call_stack_state(stack);
                    }

                    // -- Update disassembly viewer window
                    if framework.gui.is_window_open(gui::GuiWindow::DiassemblyViewer) {
                        let disassembly_start_addr_str = framework.gui.get_disassembly_view_address();
                        let disassembly_start_addr = match machine.cpu().eval_address(disassembly_start_addr_str) {
                            Some(i) => i,
                            None => 0
                        };

                        let bus = machine.bus_mut();
                        
                        let mut disassembly_string = String::new();
                        let mut disassembly_addr = disassembly_start_addr as usize;
                        for _ in 0..24 {

                            if disassembly_addr < machine::MAX_MEMORY_ADDRESS {

                                bus.seek(disassembly_addr as usize);
                                let decode_str: String = match Cpu::decode(bus) {
                                    Ok(i) => {
                                    
                                        let instr_slice = bus.get_slice_at(disassembly_addr, i.size as usize);
                                        let instr_bytes_str = util::fmt_byte_array(instr_slice);
                                        let decode_str = format!("{:05X} {:012} {}\n", disassembly_addr, instr_bytes_str, i);
                                        disassembly_addr += i.size as usize;

                                        decode_str
                                    }
                                    Err(_) => {
                                        format!("{:05X} INVALID\n", disassembly_addr)
                                    }
                                };
                                disassembly_string.push_str(&decode_str);
                            }
                        }
                        framework.gui.update_dissassembly_view(disassembly_string);
                    }

                    // Prepare egui
                    framework.prepare(&window);

                    // Render everything together
                    let render_result = pixels.render_with(|encoder, render_target, context| {

                        // Render the world texture
                        context.scaling_renderer.render(encoder, render_target);

                        // Render egui
                        #[cfg(not(feature = "pi_validator"))]
                        framework.render(encoder, render_target, context)?;

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

    // Instantiate the main Machine data struct
    // Machine coordinates all the parts of the emulated computer
    let mut machine = Machine::new(
        &config,
        config.machine.model,
        config.emulator.trace_mode,
        config.machine.video, 
        sp, 
        rom_manager, 
        floppy_manager,
    );

    let mut exec_control = machine::ExecutionControl::new();
    exec_control.set_state(ExecutionState::Running);

    loop {
        // This should really return a Result
        machine.run(1000, &mut exec_control, 0);
    }
    
    //std::process::exit(0);
}


#[cfg(feature = "cpu_validator")]
use std::io::{BufWriter, Write};
#[cfg(feature = "cpu_validator")]
use cpu_808x::*;
use crate::cpu_808x::cpu_mnemonic::Mnemonic;

#[cfg(feature = "cpu_validator")]
pub fn main_fuzzer <'a>(
    config: &ConfigFileParams,
    rom_manager: RomManager,
    floppy_manager: FloppyManager
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

    let mut io_bus = IoBusInterface::new();
    let mut pic = Rc::new(RefCell::new(pic::Pic::new()));    

    let mut cpu = Cpu::new(
        CpuType::Cpu8088,
        config.emulator.trace_mode,
        Some(trace_file_option),
        #[cfg(feature = "cpu_validator")]
        config.validator.vtype.unwrap()
    );

    cpu.randomize_seed(0);
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
        // Completed 5000 tests
        */
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

        //cpu.random_inst_from_opcodes(&[0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97]); // XCHG reg, ax
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

        cpu.random_inst_from_opcodes(&[0xE8, 0xE9, 0xEA, 0xEB]); // CALL & JMP

        //cpu.random_inst_from_opcodes(&[0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D]); // SBB 8 & 16 bit
        //cpu.random_inst_from_opcodes(&[0x18, 0x1A, 0x1C]); // SBB 8 bit

        //cpu.random_grp_instruction(0xF6, &[4, 5]); // 8 bit MUL & IMUL
        //cpu.random_grp_instruction(0xF7, &[4, 5]); // 16 bit MUL & IMUL
        
        //cpu.random_inst_from_opcodes(&[0xD4]); // AAM
        //cpu.random_grp_instruction(0xF6, &[6, 7]); // 8 bit DIV & IDIV
        //cpu.random_grp_instruction(0xF7, &[6, 7]); // 16 bit DIV & IDIV

        // Decode this instruction
        let instruction_address = 
            Cpu::calc_linear_address(
                cpu.get_register16(Register16::CS),  
                cpu.get_register16(Register16::IP)
            );

        cpu.bus_mut().seek(instruction_address as usize);
        let (opcode, _cost) = cpu.bus_mut().read_u8(instruction_address as usize).expect("mem err");

        let mut i = match Cpu::decode(cpu.bus_mut()) {
            Ok(i) => i,
            Err(_) => {
                log::error!("Instruction decode error, skipping...");
                continue;
            }                
        };
        
        // Skip N successful instructions
        if test_num < 0 {
            continue;
        }

        let mut rep = false;
        match i.mnemonic {
            Mnemonic::INT | Mnemonic::INT3 | Mnemonic::INTO | Mnemonic::IRET => {
                //continue;
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
        
        // We loop here to handle REP string instructions, which are broken up into 1 effective instruction
        // execution per iteration. The 8088 makes no such distinction.
        loop {
            match cpu.step(&mut io_bus, pic.clone()) {
                Ok(cycles) => {
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

    }
    
    



    //std::process::exit(0);
}
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

use configparser::ini::Ini;

#[path = "./devices/ega/mod.rs"]
mod ega;
#[path = "./devices/vga/mod.rs"]
mod vga;

mod arch;
mod bus;
mod bytebuf;
mod byteinterface;
mod cga;
mod cpu;
mod dma;
mod fdc;
mod floppy_manager;
mod gui;
mod gui_image;
mod hdc;
mod io;
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
mod videocard;
mod input;

use machine::{Machine, MachineType};
use rom_manager::{RomManager, RomError, RomFeature};
use floppy_manager::{FloppyManager, FloppyError};
use vhd_manager::{VHDManager, VHDManagerError};
use vhd::{VirtualHardDisk};
use byteinterface::ByteInterface;
use gui::GuiEvent;
use sound::SoundPlayer;
use videocard::VideoType;

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
const CYCLES_PER_FRAME: u32 = (cpu::CPU_MHZ * 1000000.0 / FPS_TARGET) as u32;

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
    accumulated_us: u128
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
            accumulated_us: 0
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

fn main() -> Result<(), Error> {

    env_logger::init();

    // Read config file
    let mut config = Ini::new();

    // Defaults
    let mut machine_type = MachineType::IBM_XT_5160;
    let mut video_type = VideoType::CGA;

    let mut features = Vec::new();
    let mut cfg_load_vhd_name;

    match std::fs::read_to_string("./marty.cfg") {
        Ok(config_string) => {
            match config.read(config_string) {
                Ok(_) => {

                    let machine_type_s = config.get("machine", "model").unwrap_or("IBM_XT_5160".to_string());
                    machine_type = match machine_type_s.as_str() {
                        "IBM_PC_5150" => MachineType::IBM_PC_5150,
                        "IBM_XT_5160" => MachineType::IBM_XT_5160,
                        _ => {
                            log::warn!("Invalid machine type in config: '{}'", machine_type_s);
                            MachineType::IBM_PC_5150
                        }
                    };

                    let video_type_s = config.get("machine", "video").unwrap_or("CGA".to_string());
                    video_type = match video_type_s.as_str() {
                        "CGA" => VideoType::CGA,
                        "EGA" => {
                            features.push(RomFeature::EGA);
                            VideoType::EGA
                        }
                        "VGA" => {
                            features.push(RomFeature::VGA);
                            VideoType::VGA
                        }                        
                        _ => {
                            log::warn!("Invalid video type in config: '{}'", machine_type_s);
                            VideoType::CGA
                        }
                    };

                    let hdc_type_s = config.get("machine", "hdc").unwrap_or("none".to_string());
                    match hdc_type_s.as_str() {
                        "xebec" => {
                            features.push(RomFeature::XebecHDC)
                        }
                        _ => {
                            log::warn!("Invalid hdc type in config: '{}'", hdc_type_s);
                        }                        
                    }

                    cfg_load_vhd_name = config.get("vhd", "drive0");

                }
                Err(e) => {
                    eprintln!("Error reading configuration file.");
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Couldn't read configuration file.");
            std::process::exit(1);
        }
    };

    // Instantiate the rom manager to load roms for the requested machine type    
    let mut rom_manager = RomManager::new(machine_type, features);

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
            Pixels::new(video_data.aspect_w, video_data.aspect_h, surface_texture)?;
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
    let mut machine = Machine::new(machine_type, video_type, sp, rom_manager, floppy_manager );

    // Try to load default vhd
    if let Some(vhd_name) = cfg_load_vhd_name {
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
                    machine.run(CYCLES_PER_FRAME, &mut exec_control.borrow_mut(), bp_addr);
                    stat_counter.emulation_time = Instant::now() - emulation_start;

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
                                        vhd_manager.scan_dir("./hdd");
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
                        let disassembly_addr_str = framework.gui.get_disassembly_view_address();
                        let disassembly_addr = match machine.cpu().eval_address(disassembly_addr_str) {
                            Some(i) => i,
                            None => 0
                        };

                        let bus = machine.mut_bus();
                        bus.set_cursor(disassembly_addr as usize);
                        let mut disassembly_string = String::new();
                        for _ in 0..24 {

                            let address = bus.tell();
                            if address < machine::MAX_MEMORY_ADDRESS {

                                let decode_str: String = match arch::decode(bus) {
                                    Ok(i) => {
                                    
                                        let instr_slice = bus.get_slice_at(address, i.size as usize);
                                        let instr_bytes_str = util::fmt_byte_array(instr_slice);                                    
                                        format!("{:05X} {:012} {}\n", address, instr_bytes_str, i)
                                    }
                                    Err(_) => {
                                        format!("{:05X} INVALID\n", address)
                                    }
                                };
                                disassembly_string.push_str(&decode_str)
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


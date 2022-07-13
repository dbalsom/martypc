#![deny(clippy::all)]
#![forbid(unsafe_code)]

use crate::gui::Framework;
use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent, StartCause, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

use std::{
    fs::{File, read},
    time::{Duration, Instant},
    cell::RefCell,
    rc::Rc,
    path::Path
};

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
mod hdc;
mod io;
mod machine;
mod memerror;
mod pic;
mod pit;
mod ppi;
mod rom_manager;
mod util;
mod vhd;
mod vhd_manager;
mod video;
mod input;

use machine::{Machine, MachineType, VideoType};
use rom_manager::{RomManager, RomError};
use floppy_manager::{FloppyManager, FloppyError};
use vhd_manager::{VHDManager, VHDManagerError};
use vhd::{VirtualHardDisk};
use byteinterface::ByteInterface;
use gui::GuiEvent;

const EGUI_MENU_BAR: u32 = 25;
const WINDOW_WIDTH: u32 = 1280;
const WINDOW_HEIGHT: u32 = 800 + EGUI_MENU_BAR * 2;
const WIDTH: u32 = 640;
const HEIGHT: u32 = 400;

pub const FPS_TARGET: f64 = 60.0;
const MICROS_PER_FRAME: f64 = 1.0 / FPS_TARGET * 1000000.0;

const CYCLES_PER_FRAME: u32 = (cpu::CPU_MHZ * 1000000.0 / FPS_TARGET) as u32;

// Rendering Stats
struct Counter {
    frame_count: u64,
    current_fps: u32,
    last_period: Instant,
    last_second: Instant,
    last_cpu_cycles: u64,
    current_cpu_cps: u64,
    last_pit_ticks: u64,
    current_pit_tps: u64,
}

fn main() -> Result<(), Error> {

    env_logger::init();

    // Choose machine type (move to cfg?)
    let machine_type = MachineType::IBM_XT_5160;

    // Instantiate the rom manager to load roms for the requested machine type    
    let mut rom_manager = RomManager::new(machine_type);

    if let Err(e) = rom_manager.try_load_from_dir("./rom") {
        match e {
            RomError::DirNotFound => {
                eprintln!("Rom directory not found")
            }
            RomError::RomNotFoundForMachine => {
                eprintln!("No valid rom found for specified machine type")
            }
            _ => {
                eprintln!("Error loading rom file.")
            }
        }
        std::process::exit(1);
    }

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

    // ExecutionControl is shared via RefCell with GUI so that state can be updated by control widget
    let exec_control = Rc::new(RefCell::new(machine::ExecutionControl::new()));

    // Instantiate the main Machine data struct
    // Machine coordinates all the parts of the emulated computer
    let mut machine = Machine::new(machine_type, VideoType::CGA, rom_manager, floppy_manager );
    
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

    let (mut pixels, mut framework) = {
        let window_size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        let pixels = Pixels::new(WIDTH, HEIGHT, surface_texture)?;
        let framework =
            Framework::new(window_size.width, window_size.height, scale_factor, &pixels, exec_control.clone());

        (pixels, framework)
    };
    let mut stat_counter = Counter::new();

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
            Event::WindowEvent{ event, .. } => {

                match event {
                    WindowEvent::KeyboardInput{
                        input: winit::event::KeyboardInput {
                            virtual_keycode:Some(keycode),
                            state,
                            ..
                        },
                        ..
                    } => {
                        if !framework.has_focus() {
                            match state {
                                winit::event::ElementState::Pressed => {
                                    
                                    if let Some(keycode) = input::match_virtual_keycode(keycode) {
                                        log::debug!("Key pressed, keycode: {:?}: xt: {:02X}", keycode, keycode);
                                        machine.key_press(keycode);
                                    };
                                },
                                winit::event::ElementState::Released => {
                                    if let Some(keycode) = input::match_virtual_keycode(keycode) {
                                        log::debug!("Key released, keycode: {:?}: xt: {:02X}", keycode, keycode);
                                        machine.key_release(keycode);
                                    };
                                }
                            }
                        }
                        else {
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

                    stat_counter.current_fps = 0;
                    stat_counter.last_second = Instant::now();
                } 

                // Decide whether to draw a frame
                let elapsed_us = stat_counter.last_period.elapsed().as_micros();

                if elapsed_us > MICROS_PER_FRAME as u128 {

                    stat_counter.last_period = Instant::now();
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

                    machine.run(CYCLES_PER_FRAME, &mut exec_control.borrow_mut(), bp_addr);

                    let composite_enabled = framework.gui.get_composite_enabled();
                    // Draw video memory
                    video.draw(pixels.get_frame(), machine.cga(), machine.bus(), composite_enabled);
                    
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

impl Counter {
    fn new() -> Self {
        Self {

            frame_count: 0,
            current_fps: 0,
            last_second: Instant::now(),
            last_period: Instant::now(),
            last_cpu_cycles: 0,
            current_cpu_cps: 0,
            last_pit_ticks: 0,
            current_pit_tps: 0,
        }
    }

}
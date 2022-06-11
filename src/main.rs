#![deny(clippy::all)]
#![forbid(unsafe_code)]

use crate::gui::Framework;
use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use rand::distributions::DistString;
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent, StartCause, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

use std::{
    fs::{File, read},
    time::{Duration, Instant},
    cell::RefCell,
    rc::Rc
};
use rand::{Rng};


mod arch;
mod bus;
mod byteinterface;
mod cga;
mod cpu;
mod dma;
mod floppy;
mod floppy_manager;
mod gui;
mod io;
mod machine;
mod membuf;
mod memerror;
mod pic;
mod pit;
mod ppi;
mod rom;
mod util;
mod video;
mod input;

use machine::{Machine, MachineType, VideoType};
use rom::{RomManager, RomError};
use floppy_manager::{FloppyManager, FloppyError};
use video::{CGAColor};
use byteinterface::ByteInterface;


const EGUI_MENU_BAR: u32 = 25;
const WINDOW_WIDTH: u32 = 1280;
const WINDOW_HEIGHT: u32 = 800 + EGUI_MENU_BAR * 2;
const WIDTH: u32 = 640;
const HEIGHT: u32 = 400;

pub const FPS_TARGET: f64 = 60.0;
const MICROS_PER_FRAME: f64 = 1.0 / FPS_TARGET * 1000000.0;

const CYCLES_PER_FRAME: u32 = (cpu::CPU_MHZ * 1000000.0 / FPS_TARGET) as u32;

/// Representation of the application state. In this example, a box will bounce around the screen.
struct World {
    frame_count: u64,
    current_fps: u32,
    last_period: Instant,
    last_second: Instant
}

fn main() -> Result<(), Error> {

    let timer_length = Duration::new(1, 0);
    env_logger::init();

    // Choose machine type (move to cfg?)
    let machine_type = MachineType::IBM_PC_5150;

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

    //std::process::exit(0);

    // Init machine
    //let bios_vec = read("./rom/bios.rom").unwrap_or_else(|e| {
    //    eprintln!("Couldn't open BIOS image ./rom/bios.rom: {}", e);
    //    std::process::exit(1);
    //});

    //let basic_vec = read("./rom/basic_v1.rom").unwrap_or_else(|e| {
    //    eprintln!("Couldn't open BASIC image: {}", e);
    //    std::process::exit(1);
    //});

    // ExecutionControl is shared via RefCell with GUI so that state can be updated by control widget
    let exec_control = Rc::new(RefCell::new(machine::ExecutionControl::new()));
    let mut machine = Machine::new(machine_type, VideoType::CGA, rom_manager, floppy_manager );
    
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
    let mut world = World::new();

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
                println!("Initializing...");

                world.last_second = Instant::now();
            }
            //Event::WindowEvent { event, .. } => {
//
            //    match &event {
//
            //        KeyboardInput => {
            //            println!("Event: {:?}", e);
            //        }
            //    }
//
            //    // Update egui inputs
            //    framework.handle_event(&event);
            //}
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
                let elapsed_ms = world.last_second.elapsed().as_millis();
                if elapsed_ms > 1000 {
                    // One second elapsed
                    //println!("fps: {} elapsed ms: {}", world.current_fps, elapsed_ms );
                    world.current_fps = 0;
                    world.last_second = Instant::now();
                } 

                // Decide whether to draw a frame
                let elapsed_us = world.last_period.elapsed().as_micros();

                if elapsed_us > MICROS_PER_FRAME as u128 {

                    world.last_period = Instant::now();
                    world.frame_count += 1;
                    world.current_fps += 1;
                    //println!("frame: {} elapsed: {}", world.current_fps, elapsed_us);

                    // Draw the world
                    world.update();
                    
                    //world.draw(pixels.get_frame());

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

                    // Any errors?
                    if let Some(err) = machine.get_error_str() {
                        framework.gui.show_error(err);
                        framework.gui.show_disassembly_view();
                    }

                    // Draw video memory
                    video.draw(pixels.get_frame(), machine.cga(), machine.bus());
                    
                    // Update egui data

                    // -- Update list of floppies
                    let name_vec = machine.floppy_manager().get_floppy_names();
                    framework.gui.set_floppy_names(name_vec);

                    // -- Do we have a new floppy image to load?
                    if let Some(new_floppy_name) = framework.gui.get_new_floppy_name() {
                        log::debug!("Load new floppy image: {:?}", new_floppy_name);

                        let vec = match machine.floppy_manager().load_floppy_data(&new_floppy_name) {
                            Ok(vec) => {
                                machine.fdc().borrow_mut().load_image_from(0, vec);
                                println!("Loaded okay!");
                            } 
                            Err(e) => {
                                log::error!("Failed to load floppy image! {:?}", new_floppy_name);
                                eprintln!("Failed to read file: {:?}", new_floppy_name);
                            }
                        };
                    }

                    // -- Update memory viewer window
                    {
                        let mem_dump_addr_str = framework.gui.get_memory_view_address();
                        let mem_dump_addr = match machine.cpu().eval_address(mem_dump_addr_str) {
                            Some(i) => i,
                            None => 0
                        };
                        let mem_dump_str = machine.bus().dump_flat(mem_dump_addr as usize, 256);

                        framework.gui.update_memory_view(mem_dump_str);
                    }   
                    // -- Update register viewer window
                    let cpu_state = machine.cpu().get_string_state();
                    framework.gui.update_cpu_state(cpu_state);

                    // -- Update PIT viewer window
                    let pit_state = machine.pit_state();
                    framework.gui.update_pit_state(pit_state);

                    // -- Update PIC viewer window
                    let pic_state = machine.pic_state();
                    framework.gui.update_pic_state(pic_state);

                    // -- Update PPI viewer window
                    let ppi_state = machine.ppi_state();
                    framework.gui.update_ppi_state(ppi_state);

                    // -- Update DMA viewer window
                    let dma_state = machine.dma_state();
                    framework.gui.update_dma_state(dma_state);

                    // -- Update Instruction Trace window
                    let trace = machine.cpu().dump_instruction_history();
                    framework.gui.update_trace_state(trace);

                    // -- Update Call Stack window
                    let stack = machine.cpu().dump_call_stack();
                    framework.gui.update_call_stack_state(stack);

                    // -- Update disassembly viewer window
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

impl World {
    /// Create a new `World` instance that can draw a moving box.
    fn new() -> Self {
        Self {

            frame_count: 0,
            current_fps: 0,
            last_second: Instant::now(),
            last_period: Instant::now()
        }
    }

    /// Update the `World` internal state; bounce the box around the screen.
    fn update(&mut self) {

        
    }

    /// Draw the `World` state to the frame buffer.
    ///
    /// Assumes the default texture format: `wgpu::TextureFormat::Rgba8UnormSrgb`
    fn draw(&self, frame: &mut [u8]) {
        //for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
        //    let x = (i % WIDTH as usize) as i16;
        //    let y = (i / WIDTH as usize) as i16;
        //
        //    let inside_the_box = x >= self.box_x
        //        && x < self.box_x + BOX_SIZE
        //        && y >= self.box_y
        //        && y < self.box_y + BOX_SIZE;
        //
        //    let rgba = if inside_the_box {
        //        [0x5e, 0x48, 0xe8, 0xff]
        //    } else {
        //        [0x48, 0xb2, 0xe8, 0xff]
        //    };
        //
        //    pixel.copy_from_slice(&rgba);
        //}

        for y in 0..25 {
            for x in 0..80 {
                let fg_color: CGAColor = rand::random();
                let bg_color: CGAColor = rand::random();

                let glyph = rand::thread_rng().gen_range(0..256);
                video::draw_glyph2x(glyph as u8, fg_color, bg_color, frame, WIDTH, HEIGHT, x * 8, y * 8);
            }
        }        
    }
}
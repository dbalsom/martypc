#![deny(clippy::all)]
#![forbid(unsafe_code)]

use instant::{Duration, Instant};

use js_sys::{self, Reflect};
use pixels::wgpu::TextureView;
use wasm_bindgen::{closure::Closure, prelude::*, JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{console, window, Blob, FileReader, Headers, ProgressEvent, Request, RequestInit, Response};

use error_iter::ErrorIter as _;
use log::error;
use pixels::{Pixels, SurfaceTexture};
use std::rc::Rc;
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event::{Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

use marty_core::{
    bytequeue::ByteQueue,
    cpu_808x::{Cpu, CpuAddress},
    cpu_common::CpuOption,
    floppy_manager::{FloppyError, FloppyManager},
    input::{self, MouseButton},
    lib::{self, *},
    machine::{self, ExecutionControl, ExecutionState, Machine, MachineState},
    machine_manager::MACHINE_DESCS,
    rom_manager::{RawRomDescriptor, RomManager},
    sound::SoundPlayer,
    syntax_token::SyntaxToken,
    util,
    vhd::{self, VirtualHardDisk},
    vhd_manager::{VHDManager, VHDManagerError},
    videocard::RenderMode,
};

use marty_render::{CompositeParams, ResampleContext, VideoData, VideoRenderer};
//use pixels_stretch_renderer::{StretchingRenderer, SurfaceSize};

const DEFAULT_RENDER_WIDTH: u32 = 768;
const DEFAULT_RENDER_HEIGHT: u32 = 524;

const DEFAULT_ASPECT_WIDTH: u32 = 768;
const DEFAULT_ASPECT_HEIGHT: u32 = 576;

const MIN_RENDER_WIDTH: u32 = 160;
const MIN_RENDER_HEIGHT: u32 = 200;
const RENDER_ASPECT: f32 = 0.75;

pub const FPS_TARGET: f64 = 60.0;
const MICROS_PER_FRAME: f64 = 1.0 / FPS_TARGET * 1000000.0;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window, js_name = sharedState)]
    static SHARED_STATE: JsValue;
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

#[wasm_bindgen(start)]
fn start() {
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));

        log::warn!("Initializing logger...");
        match console_log::init_with_level(log::Level::Warn) {
            Ok(()) => {}
            Err(e) => log::error!("Couldn't initialize logger: {}", e),
        };

        //wasm_bindgen_futures::spawn_local(run());
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
        //pollster::block_on(run());
    }
}

pub async fn fetch_binary_file(url: &str) -> Result<Vec<u8>, JsValue> {
    let client_window = window().expect("no global `window` exists");

    let mut opts = RequestInit::new();
    opts.method("GET");

    let request = Request::new_with_str_and_init(url, &opts)?;
    request
        .headers()
        .set("Content-Type", "application/octet-stream")
        .unwrap();

    let resp_value = JsFuture::from(client_window.fetch_with_request(&request)).await?;
    let resp: Response = resp_value.dyn_into()?;

    let blob = JsFuture::from(resp.blob()?).await?;
    let blob: Blob = blob.dyn_into()?;

    let array_buffer = JsFuture::from(read_blob_as_array_buffer(&blob)).await?;
    let uint8_array = js_sys::Uint8Array::new(&array_buffer);

    let mut vec = vec![0; uint8_array.length() as usize];
    uint8_array.copy_to(&mut vec);

    Ok(vec)
}

fn read_blob_as_array_buffer(blob: &web_sys::Blob) -> js_sys::Promise {
    let file_reader = FileReader::new().unwrap();

    let promise = js_sys::Promise::new(&mut |resolve: js_sys::Function, reject: js_sys::Function| {
        let onload = wasm_bindgen::closure::Closure::once(move |event: web_sys::ProgressEvent| {
            let file_reader: FileReader = event.target().unwrap().dyn_into().unwrap();
            let array_buffer = file_reader.result().unwrap();
            resolve.call1(&JsValue::null(), &array_buffer).unwrap();
        });

        file_reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        file_reader.read_as_array_buffer(blob).unwrap();

        onload.forget();
    });

    promise
}

#[wasm_bindgen]
pub async fn run(cfg: &str) {
    // Emulator stuff
    let mut stat_counter = Counter::new();

    let mut video_data = VideoData {
        render_w: DEFAULT_RENDER_WIDTH,
        render_h: DEFAULT_RENDER_HEIGHT,
        aspect_w: DEFAULT_ASPECT_WIDTH,
        aspect_h: DEFAULT_ASPECT_HEIGHT,
        aspect_correction_enabled: false,
        composite_params: Default::default(),
        last_mode_byte: 0,
    };

    // Create the video renderer
    let mut video;
    let mut render_src = vec![0; (DEFAULT_RENDER_WIDTH * DEFAULT_RENDER_HEIGHT * 4) as usize];
    // Create resampling context
    let mut resample_context = ResampleContext::new();

    let mut exec_control = ExecutionControl::new();
    exec_control.set_state(ExecutionState::Running);

    // Winit stuff
    let event_loop = EventLoop::new();
    let window = {
        let size = LogicalSize::new(DEFAULT_ASPECT_WIDTH as f64, DEFAULT_ASPECT_HEIGHT as f64);

        WindowBuilder::new()
            .with_title("MartyPC WASM Player")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .expect("WindowBuilder error")
    };

    let window = Rc::new(window);

    let mut composite_enabled;
    let mut machine;

    //#[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        use winit::platform::web::WindowExtWebSys;

        // Retrieve current width and height dimensions of browser client window
        let get_window_size = || {
            let client_window = web_sys::window().unwrap();
            LogicalSize::new(
                client_window.inner_width().unwrap().as_f64().unwrap(),
                client_window.inner_height().unwrap().as_f64().unwrap(),
            )
        };

        let window = Rc::clone(&window);

        let client_window = web_sys::window().unwrap();
        let dpr = client_window.device_pixel_ratio();

        // Initialize winit window with current dimensions of browser client
        //window.set_inner_size(get_window_size());
        window.set_inner_size(PhysicalSize::new(768, 576));

        // Attach winit canvas to body element
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| {
                // Here we use query_selector to get the element by class name.
                doc.query_selector("#marty-canvas-container").ok().flatten()
            })
            .and_then(|div| {
                // Append the canvas to the div.
                div.append_child(&web_sys::Element::from(window.canvas())).ok()

                /*
                // Get the canvas element
                let canvas = web_sys::Element::from(window.canvas());

                // Cast the Element to HtmlCanvasElement so we can modify it.
                let canvas = canvas.dyn_into::<web_sys::HtmlCanvasElement>().unwrap();

                // Get the device pixel ratio
                let dpr = client_window.device_pixel_ratio();
                log::warn!("dpr is: {}", dpr);

                // Set the size of the canvas in pixels
                let width = 768.0; // desired width in CSS pixels
                let height = 576.0; // desired height in CSS pixels
                canvas.set_width((width * dpr) as u32);
                canvas.set_height((height * dpr) as u32);

                // Scale the canvas back down to the desired size using CSS
                let style = canvas.style();
                style.set_property("width", &(width.to_string() + "px")).unwrap();
                style.set_property("height", &(height.to_string() + "px")).unwrap();

                // Append the canvas to the div
                div.append_child(&canvas).ok()
                */
            })
            .expect("Couldn't append canvas to the specified div!");

        log::warn!("Got config file name: {}", cfg);

        // Try to load toml config.
        let mut opts = web_sys::RequestInit::new();
        opts.method("GET");

        let request = web_sys::Request::new_with_str_and_init(&format!("./cfg/{}", cfg), &opts)
            .expect("Couldn't create request for configuration file.");
        request
            .headers()
            .set("Content-Type", "text/plain")
            .expect("Couldn't set headers!");

        let resp_value = JsFuture::from(client_window.fetch_with_request(&request))
            .await
            .unwrap();
        let resp: Response = resp_value.into();

        // Get the response as text
        let toml_text = JsFuture::from(resp.text().unwrap()).await.unwrap();

        // Read config file from toml text
        let mut config = match lib::get_config_from_str(&toml_text.as_string().unwrap()) {
            Ok(config) => config,
            Err(e) => {
                match e.downcast_ref::<std::io::Error>() {
                    Some(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        log::error!("Configuration file not found!");
                    }
                    Some(e) => {
                        log::error!("Unknown IO error reading configuration file:\n{}", e);
                    }
                    None => {
                        log::error!(
                            "Failed to parse configuration file. There may be a typo or otherwise invalid toml:\n{}",
                            e
                        );
                    }
                }
                return;
            }
        };

        video = VideoRenderer::new(config.machine.video);

        let rom_override = match config.machine.rom_override {
            Some(ref rom_override) => rom_override,
            None => panic!("No rom file specified!"),
        };

        let floppy_path_str = match config.machine.floppy0 {
            Some(ref floppy) => floppy,
            None => panic!("No floppy image specified!"),
        };

        log::warn!(
            "Read config file. Rom to load: {:?} Floppy to load: {:?}",
            rom_override[0].path,
            floppy_path_str
        );

        // Convert Path to str
        let rom_path_str = &rom_override[0].path.clone().into_os_string().into_string().unwrap();

        // Get the rom file as a vec<u8>
        let rom_vec = fetch_binary_file(rom_path_str).await.unwrap();

        // Get the floppy image as a vec<u8>
        let floppy_vec = fetch_binary_file(floppy_path_str).await.unwrap();

        //log::warn!("rom: {:?}", rom_vec);

        // Look up the machine description given the machine type in the configuration file
        let machine_desc_opt = MACHINE_DESCS.get(&config.machine.model);
        if let Some(machine_desc) = machine_desc_opt {
            log::warn!(
                "Given machine type {:?} got machine description: {:?}",
                config.machine.model,
                machine_desc
            );
        }
        else {
            log::error!(
                "Couldn't get machine description for machine type {:?}. \
                 Check that you have a valid machine type specified in configuration file.",
                config.machine.model
            );
            return;
        }

        // Init sound
        // The cpal sound library uses generics to initialize depending on the SampleFormat type.
        // On Windows at least a sample type of f32 is typical, but just in case...
        let sample_fmt = SoundPlayer::get_sample_format();
        let sp = match sample_fmt {
            cpal::SampleFormat::F32 => SoundPlayer::new::<f32>(),
            cpal::SampleFormat::I16 => SoundPlayer::new::<i16>(),
            cpal::SampleFormat::U16 => SoundPlayer::new::<u16>(),
        };

        // Empty features
        let mut features = Vec::new();

        let mut rom_manager = RomManager::new(config.machine.model, features, config.machine.rom_override.clone());

        rom_manager.add_raw_rom(
            &rom_vec,
            RawRomDescriptor {
                addr:   rom_override[0].address,
                offset: rom_override[0].offset,
                org:    rom_override[0].org,
            },
        );

        // capture option before moving to machine
        composite_enabled = config.machine.composite;

        machine = Machine::new(
            &config,
            config.machine.model,
            *machine_desc_opt.unwrap(),
            config.emulator.trace_mode,
            config.machine.video,
            sp,
            rom_manager,
        );

        if let Some(fdc) = machine.fdc() {
            match fdc.load_image_from(0, floppy_vec) {
                Ok(()) => {
                    log::warn!("Floppy image successfully loaded into virtual drive.");
                }
                Err(err) => {
                    log::error!("Floppy image failed to load: {}", err);
                }
            }
        }

        // Set CPU options
        machine.set_cpu_option(CpuOption::EnableWaitStates(config.cpu.wait_states_enabled));

        /*
        // Listen for resize event on browser client. Adjust winit window dimensions
        // on event trigger
        let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |_e: web_sys::Event| {
            let size = get_window_size();
            window.set_inner_size(size)
        }) as Box<dyn FnMut(_)>);

        client_window
            .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
            .unwrap();

        closure.forget();
        */
    }

    let mut input = WinitInputHelper::new();
    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, window.as_ref());

        Pixels::new_async(DEFAULT_RENDER_WIDTH, DEFAULT_RENDER_HEIGHT, surface_texture)
            .await
            .expect("Pixels error")
    };

    let stretching_renderer = StretchingRenderer::new(
        &pixels,
        video_data.render_w,
        video_data.render_h,
        video_data.aspect_w,
        video_data.aspect_h,
    );

    // Start buffer playback
    machine.play_sound_buffer();

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::ModifiersChanged(modifier_state) => {
                        //kb_data.ctrl_pressed = modifier_state.ctrl();
                    }
                    WindowEvent::KeyboardInput {
                        input:
                            winit::event::KeyboardInput {
                                virtual_keycode: Some(keycode),
                                state,
                                ..
                            },
                        ..
                    } => {
                        match state {
                            winit::event::ElementState::Pressed => {
                                if let Some(keycode) = input::match_virtual_keycode(keycode) {
                                    //log::debug!("Key pressed, keycode: {:?}: xt: {:02X}", keycode, keycode);
                                    machine.key_press(keycode);
                                };
                            }
                            winit::event::ElementState::Released => {
                                if let Some(keycode) = input::match_virtual_keycode(keycode) {
                                    //log::debug!("Key released, keycode: {:?}: xt: {:02X}", keycode, keycode);
                                    machine.key_release(keycode);
                                };
                            }
                        }
                    }
                    _ => {}
                }
            }
            // Draw the current frame
            Event::RedrawRequested(event) => {
                //world.draw(pixels.frame_mut());

                //stat_counter.current_fps += 1;

                if let Err(e) = pixels.render_with(|encoder, render_target, context| {
                    let fill_texture = stretching_renderer.get_texture_view();

                    //context.scaling_renderer.marty_render(encoder, fill_texture);

                    stretching_renderer.render(encoder, render_target);
                    Ok(())
                }) {
                    log::error!("pixels.render_with error: {}", e);
                };

                /*
                if let Err(err) = pixels.marty_render() {
                    log_error("pixels.marty_render", err);
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                */
            }
            _ => {}
        }

        let elapsed_ms = stat_counter.last_second.elapsed().as_millis();
        if elapsed_ms > 1000 {
            log::warn!("FPS: {}", stat_counter.current_fps);
            stat_counter.fps = stat_counter.current_fps;
            stat_counter.current_fps = 0;
            stat_counter.last_second = Instant::now();
        }

        // Don't run the emulator if not in focus.
        let focus = Reflect::get(&SHARED_STATE, &JsValue::from_str("browserFocus")).unwrap();

        if !focus {
            stat_counter.last_frame = Instant::now();
            return;
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

            // Emulate a frame worth of instructions
            // ---------------------------------------------------------------------------

            // Recalculate cycle target based on current CPU speed if it has changed (or uninitialized)
            let mhz = machine.get_cpu_mhz();
            if mhz != stat_counter.cpu_mhz {
                stat_counter.cycles_per_frame = (machine.get_cpu_mhz() * 1000000.0 / FPS_TARGET) as u32;
                stat_counter.cycle_target = stat_counter.cycles_per_frame;
                log::info!(
                    "CPU clock has changed to {}Mhz; new cycle target: {}",
                    mhz,
                    stat_counter.cycle_target
                );
                stat_counter.cpu_mhz = mhz;
            }

            let emulation_start = Instant::now();
            stat_counter.instr_count += machine.run(stat_counter.cycle_target, &mut exec_control);
            stat_counter.emulation_time = Instant::now() - emulation_start;

            // Add instructions to IPS counter
            stat_counter.cycle_count += stat_counter.cycle_target as u64;

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
                    // Now that we marty_render into a fixed frame, we should refactor this
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
                            video_data.aspect_h,
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
            let aspect_correct = false;

            let render_start = Instant::now();

            // Draw video if there is a video card present
            let bus = machine.bus_mut();

            if let Some(video_card) = bus.video() {
                let beam_pos;
                let video_buffer;

                video_buffer = video_card.get_display_buf();
                beam_pos = None;

                // Get the marty_render mode from the device and marty_render appropriately
                match (video_card.get_video_type(), video_card.get_render_mode()) {
                    (VideoType::CGA, RenderMode::Direct) => {
                        // Draw device's front buffer in direct mode (CGA only for now)

                        let extents = video_card.get_display_extents();

                        if video_data.last_mode_byte != extents.mode_byte {
                            // Mode byte has changed, recalculate composite parameters
                            video.cga_direct_mode_update(extents.mode_byte);
                            video_data.last_mode_byte = extents.mode_byte;
                        }

                        match aspect_correct {
                            true => {
                                video.draw_cga_direct(
                                    &mut render_src,
                                    video_data.render_w,
                                    video_data.render_h,
                                    video_buffer,
                                    extents,
                                    composite_enabled,
                                    &video_data.composite_params,
                                    beam_pos,
                                );

                                marty_render::resize_linear_fast(
                                    &mut render_src,
                                    video_data.render_w,
                                    video_data.render_h,
                                    pixels.frame_mut(),
                                    video_data.aspect_w,
                                    video_data.aspect_h,
                                    &mut resample_context,
                                );
                            }
                            false => {
                                video.draw_cga_direct(
                                    pixels.frame_mut(),
                                    video_data.render_w,
                                    video_data.render_h,
                                    video_buffer,
                                    extents,
                                    composite_enabled,
                                    &video_data.composite_params,
                                    beam_pos,
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
                                    &resample_context,
                                );
                            }
                            false => {
                                video.draw(pixels.frame_mut(), video_card, bus, composite_enabled);
                            }
                        }
                    }
                    _ => panic!("Invalid combination of VideoType and RenderMode"),
                }
            }

            window.request_redraw();
        }
    });
}

fn log_error<E: std::error::Error + 'static>(method_name: &str, err: E) {
    error!("{method_name}() failed: {err}");
    for source in err.sources().skip(1) {
        error!("  Caused by: {source}");
    }
}

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

    window_manager

    Code for handling winit windows. MartyPC can optionally create multiple
    windows, a main GUI window and an output window for each output video device.

    These routines simplify window handling and allow returning the a
    reference to the correct window for rendering tasks.

*/

const EGUI_MENU_BAR: u32 = 24;
pub(crate) const WINDOW_MIN_WIDTH: u32 = 640;
pub(crate) const WINDOW_MIN_HEIGHT: u32 = 480;
const DEFAULT_MAIN_WINDOW_WIDTH: u32 = WINDOW_MIN_WIDTH;
const DEFAULT_MAIN_WINDOW_HEIGHT: u32 = WINDOW_MIN_HEIGHT + EGUI_MENU_BAR;

const DEFAULT_RENDER_WINDOW_WIDTH: u32 = WINDOW_MIN_WIDTH;
const DEFAULT_RENDER_WINDOW_HEIGHT: u32 = WINDOW_MIN_HEIGHT;

const STUB_RENDER_WIDTH: u32 = 16;
const STUB_RENDER_HEIGHT: u32 = 16;

use std::{cell::RefCell, collections::HashMap, path::PathBuf, rc::Rc};

use pixels::{
    wgpu::{AdapterInfo, PowerPreference, RequestAdapterOptions},
    Error,
    Pixels,
    PixelsBuilder,
    SurfaceTexture,
};

use winit::{dpi::LogicalSize, event_loop::EventLoop, window::*};

use config_toml_bpaf::ConfigFileParams;
use display_backend_pixels::PixelsBackend;

use marty_core::{machine::ExecutionControl, videocard::VideoType};

use marty_color::MartyColor;
use marty_core::videocard::VideoCardInterface;
use marty_egui::GuiRenderContext;
use marty_pixels_scaler::{DisplayScaler, MartyScaler, ScalerMode};
use videocard_renderer::VideoRenderer;

// Each window is associated with a winit Window, a pixels Pixels instance, a Renderer, and a
// Scaler. As a hack to avoid costly texture uploads for windows without a video card, we make
// the pixels instance for such windows very small, disable backend resizing in the renderer,
// and disable rendering in the scaler.
pub struct MartyWindow {
    //pub(crate) event_loop: EventLoop<()>,
    pub(crate) window:    Window,
    pub(crate) has_gui:   bool,
    pub(crate) has_video: bool,
    pub(crate) renderer:  VideoRenderer,
    pub(crate) gui_ctx:   Option<GuiRenderContext>,
}

/*
impl MartyWindow {
    fn new(window: Window) -> Self {
        Self {
            window,
        }
    }
}*/

pub struct WindowManager {
    // All windows share a common event loop.
    event_loop: Option<EventLoop<()>>,

    // There can be multiple display windows. One for the main egui window, which may or may not
    // be attached to a videocard.
    // Optionally, one for each potential graphics adapter. For the moment I only plan to support
    // two adapters - a primary and secondary adapter. This implies a limit of 3 windows.
    // The window containing egui will always be at index 0.
    displays: Vec<MartyWindow>,

    // Hash maps store indices into the displays vec for lookup by VideoType or id.
    // TODO: Better lookup than VideoType(?) - we might want two adapters of the same type
    //       maybe we can identify video cards by an video id - or we can store a u8 in VideoType
    //       to discriminate instances?
    display_map:   HashMap<VideoType, usize>,
    window_id_map: HashMap<WindowId, usize>,
    card_id_map:   HashMap<usize, usize>,
    primary_idx:   Option<usize>,
    secondary_idx: Option<usize>,
}

impl WindowManager {
    pub fn new() -> WindowManager {
        WindowManager {
            event_loop: Some(EventLoop::new().unwrap()),
            displays: Vec::new(),
            display_map: HashMap::new(),
            window_id_map: HashMap::new(),
            card_id_map: HashMap::new(),
            primary_idx: None,
            secondary_idx: None,
        }
    }
    pub fn create_gui_context(window: &Window, pixels: &Pixels, theme_color: Option<u32>) -> GuiRenderContext {
        let win_size = window.inner_size();

        let ctx = GuiRenderContext::new(
            win_size.width,
            win_size.height,
            window.scale_factor(),
            pixels,
            window,
            theme_color,
        );

        ctx
    }

    pub fn get_primary_gui_context(&mut self) -> Option<&mut GuiRenderContext> {
        if let Some(idx) = self.primary_idx {
            if let Some(ctx) = self.displays[idx].gui_ctx.as_mut() {
                Some(ctx)
            }
            else {
                None
            }
        }
        else {
            None
        }
    }

    fn create_pixels(w: u32, h: u32, window: &Window) -> Result<Pixels, Error> {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);

        // Create the pixels instance for main window.
        let pixels = PixelsBuilder::new(w, h, surface_texture)
            .request_adapter_options(RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .enable_vsync(true)
            .build()?;

        Ok(pixels)
    }

    fn create_renderer(
        w: u32,
        h: u32,
        video_type: VideoType,
        has_gui: bool,
        pixels: &mut Pixels,
        fill_color: u32,
    ) -> Result<VideoRenderer, Error> {
        let fill_color = MartyColor::from(fill_color);

        let marty_scaler = MartyScaler::new(
            ScalerMode::Integer,
            &pixels,
            640,
            480,
            640,
            480,
            640,
            480,
            24, // margin_y == egui menu height
            true,
            fill_color,
        );

        let adapter_info = pixels.adapter().get_info();
        let backend_str = format!("{:?}", adapter_info.backend);
        let adapter_name_str = format!("{}", adapter_info.name);
        log::debug!("wgpu using adapter: {}, backend: {}", adapter_name_str, backend_str);

        // Create the video renderer
        let mut video = VideoRenderer::new(video_type);

        video.set_on_resize_scaler(|pixels, scaler, buf, target, surface| {
            if buf.has_some_size() && surface.has_some_size() {
                log::debug!(
                    "Resizing scaler to texture {}x{}, target: {}x{}, surface: {}x{}...",
                    buf.w,
                    buf.h,
                    target.w,
                    target.h,
                    surface.w,
                    surface.h,
                );
                scaler.resize(&pixels, buf.w, buf.h, target.w, target.h, surface.w, surface.h);
            }
            else {
                log::debug!("Ignoring invalid scaler resize request (window minimized?)");
            }
        });

        video.set_on_scaler_options(|pixels, scaler, opts| {
            scaler.set_options(pixels, opts);
        });

        video.set_on_resize_backend(|pixels, backend| {
            if backend.has_some_size() {
                log::debug!("Resizing pixels buffer...");
                pixels
                    .resize_buffer(backend.w, backend.h)
                    .expect("Failed to resize Pixels buffer.");
            }
            else {
                log::debug!("Ignoring invalid buffer resize request (window minimized?)");
            }
        });

        video.set_on_margin(|scaler, l, r, t, b| {
            scaler.set_margins(l, r, t, b);
        });

        video.set_on_scalemode(|pixels, scaler, m| {
            log::debug!("Setting scaler mode to {:?}", m);
            scaler.set_mode(pixels, m);
        });

        video.set_on_resize_surface(|pixels, surface| {
            if surface.has_some_size() {
                log::debug!("Resizing pixels surface to {}x{}", surface.w, surface.h);
                pixels
                    .resize_surface(surface.w, surface.h)
                    .expect("Failed to resize Pixels surface.");
            }
            else {
                log::debug!("Ignoring invalid surface resize request (window minimized?)");
            }
        });

        /*
        video.set_with_buffer(move|action| {
            if let Ok(mut pixels) = video.get_backend().lock() {
                action(pixels.frame_mut())
            }
        });

         */

        Ok(video)
    }

    //noinspection ALL
    /// Create the system windows. This parses the configuration for the appropriate options.
    pub fn create_windows(
        &mut self,
        config: &ConfigFileParams,
        //exec_control: ExecutionControl,
        cards: Vec<VideoCardInterface>,
        icon: PathBuf,
    ) -> Result<(), Error> {
        // First, let's see if we even have a primary video adapter...
        let have_primary_video = config.machine.primary_video.is_some()
            && matches!(config.machine.primary_video, Some(VideoType::None))
            && cards.len() > 0;

        // Is the primary video output set for the main window?
        let main_has_video = have_primary_video && !config.emulator.primary_video_window;

        // Get the primary video type.
        let primary_video = config.machine.primary_video.unwrap_or_default();

        let main_video = if main_has_video && have_primary_video {
            primary_video
        }
        else {
            VideoType::None
        };

        // Create the main window.
        let window = {
            let size = LogicalSize::new(DEFAULT_MAIN_WINDOW_WIDTH as f64, DEFAULT_MAIN_WINDOW_HEIGHT as f64);

            // TODO: Better error handling here.
            WindowBuilder::new()
                .with_title(format!("MartyPC {}", env!("CARGO_PKG_VERSION")))
                .with_inner_size(size)
                .with_min_inner_size(size)
                .build(&self.event_loop.as_ref().unwrap())
                .unwrap()
        };

        let backend_w;
        let backend_h;

        if main_has_video {
            // Main window is hosting the primary video adapter. Create a pixels instance
            // of normal size.
            backend_w = DEFAULT_MAIN_WINDOW_WIDTH;
            backend_h = DEFAULT_MAIN_WINDOW_HEIGHT;
        }
        else {
            // Main window only has gui. Create a small pixels instance.
            backend_w = STUB_RENDER_WIDTH;
            backend_h = STUB_RENDER_HEIGHT;
        }

        let window_size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);

        // Create the pixels instance for main window.
        let mut pixels = WindowManager::create_pixels(backend_w, backend_h, &window)?;
        let renderer = WindowManager::create_renderer(
            window_size.width,
            window_size.height,
            main_video,
            true,
            &mut pixels,
            config.emulator.scaler_background_color.unwrap_or(0x000000FF),
        )?;

        // Create the gui context for the main window.
        let gui_ctx = WindowManager::create_gui_context(&window, &pixels, config.gui.theme_color);

        let id = window.id();
        self.displays.push(MartyWindow {
            window,
            has_gui: true,
            has_video: main_has_video,
            pixels,
            renderer,
            gui_ctx: Some(gui_ctx),
        });
        self.display_map.insert(main_video, 0);
        self.window_id_map.insert(id, 0);
        self.card_id_map.insert(cards[0].id, 0);

        if main_has_video {
            self.primary_idx = Some(0);
        }

        let have_primary_adapter;

        // Render window for primary video has been specified.
        if !main_has_video && config.emulator.primary_video_window {
            // Render window may have been specified, but we also need a valid primary video type.
            match config.machine.primary_video {
                None | Some(VideoType::None) => {
                    // No primary video adapter!
                    have_primary_adapter = false;
                }
                Some(primary_video) => {
                    let size = LogicalSize::new(DEFAULT_MAIN_WINDOW_WIDTH as f64, DEFAULT_MAIN_WINDOW_HEIGHT as f64);

                    let window = WindowBuilder::new()
                        .with_title(format!(
                            "MartyPC {}, Display: {:?}",
                            env!("CARGO_PKG_VERSION"),
                            primary_video
                        ))
                        .with_inner_size(size)
                        .with_min_inner_size(size)
                        .build(&self.event_loop.as_ref().unwrap())
                        .unwrap();

                    let pixels =
                        WindowManager::create_pixels(DEFAULT_MAIN_WINDOW_WIDTH, DEFAULT_MAIN_WINDOW_HEIGHT, &window)?;

                    let renderer = WindowManager::create_renderer(
                        DEFAULT_MAIN_WINDOW_WIDTH,
                        DEFAULT_MAIN_WINDOW_HEIGHT,
                        primary_video,
                        false,
                        &mut pixels,
                        config.emulator.scaler_background_color.unwrap_or(0x000000FF),
                    )?;

                    let id = window.id();
                    let idx = self.displays.len();
                    self.displays.push(MartyWindow {
                        window,
                        has_gui: false,
                        has_video: true,
                        pixels,
                        renderer,
                        gui_ctx: None,
                    });
                    self.display_map.insert(primary_video, idx);
                    self.window_id_map.insert(id, idx);
                    self.card_id_map.insert(cards[0].id, idx);
                    self.primary_idx = Some(idx);
                }
            }
        }

        // TODO: Handle secondary adapter

        // Set the provided icon to all windows.
        self.set_icon(icon);

        Ok(())
    }

    pub fn set_icon(&mut self, icon_path: PathBuf) {
        if let Ok(image) = image::open(icon_path.clone()) {
            let rgba8 = image.into_rgba8();
            let (width, height) = rgba8.dimensions();
            let icon_raw = rgba8.into_raw();

            self.displays.iter().for_each(|mw| {
                let icon = winit::window::Icon::from_rgba(icon_raw.clone(), width, height).unwrap();
                mw.window.set_window_icon(Some(icon));
            });
        }
        else {
            log::error!("Couldn't load icon: {}", icon_path.display());
        }
    }
    pub fn get_main_window(&mut self) -> Option<&mut MartyWindow> {
        if self.displays.len() > 0 {
            Some(&mut self.displays[0])
        }
        else {
            None
        }
    }

    pub fn get_inner_window_by_id(&self, id: WindowId) -> Option<&Window> {
        self.window_id_map.get(&id).and_then(|idx| {
            //log::warn!("got id, running map():");
            Some(&self.displays[*idx].window)
        })
    }

    pub fn get_event_loop(&mut self) -> Option<&EventLoop<()>> {
        self.event_loop.as_ref()
    }

    pub fn take_event_loop(&mut self) -> EventLoop<()> {
        self.event_loop.take().unwrap()
    }
    pub fn get_render_window(&mut self, video_type: VideoType) -> Option<&mut MartyWindow> {
        self.display_map
            .get(&video_type)
            .and_then(|idx| Some(&mut self.displays[*idx]))
    }
    pub fn get_adapter_info(&mut self) -> AdapterInfo {
        self.displays[0].pixels.adapter().get_info()
    }

    pub fn get_renderer_by_card_id(&mut self, id: usize) -> Option<&mut VideoRenderer> {
        self.card_id_map
            .get(&id)
            .and_then(|idx| Some(&mut self.displays[*idx].renderer))
    }

    pub fn get_renderer_by_window_id(&mut self, id: WindowId) -> Option<&mut VideoRenderer> {
        self.window_id_map
            .get(&id)
            .and_then(|idx| Some(&mut self.displays[*idx].renderer))
    }

    pub fn get_primary_renderer(&mut self) -> Option<&mut VideoRenderer> {
        if let Some(idx) = self.primary_idx {
            self.card_id_map
                .get(&idx)
                .and_then(|idx| Some(&mut self.displays[*idx].renderer))
        }
        else {
            None
        }
    }

    pub fn for_each_renderer<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut VideoRenderer),
    {
        for display in &mut self.displays {
            f(&mut display.renderer);
        }
    }
}

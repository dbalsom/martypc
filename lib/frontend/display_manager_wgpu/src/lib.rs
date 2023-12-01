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

    ---------------------------------------------------------------------------

    display_manager::lib.rs

    Implement MartyPC's DisplayManager for Winit/Pixels/egui frontend.

    This facility handles managing the resources, backend, scaler, winit
    window and egui contexts needed to render the output of a core VideoCard
    to one of several DisplayTargets:

    - The main window background via pixels / marty_pixels_scaler
    - A dedicated window via pixels / marty_pixels_scaler
    - An egui widget via pixels -> texture handle
    - A file (for screenshots)
 */

use config_toml_bpaf::ConfigDimensions;
use std::collections::HashMap;
use std::path::PathBuf;

pub use display_backend_pixels::{
    DisplayBackend,
    DisplayBackendBuilder,
    Pixels,
    PixelsBackend,
    BufferDimensions,
    SurfaceDimensions
};

use anyhow::{anyhow, Context, Error};

use winit::{
    event_loop::EventLoop,
    window::{Window, WindowId},
};
use winit::dpi::LogicalSize;
use winit::event_loop::ControlFlow;
use winit::window::WindowBuilder;

pub use display_manager_trait::{DisplayManager, DisplayTargetType, DisplayManagerGuiOptions};

use marty_egui::{GuiRenderContext};
use videocard_renderer::{AspectCorrectionMode, VideoDimensions, VideoRenderer};
use config_toml_bpaf::{ConfigFileParams, WindowDefinition};
use marty_core::videocard::{VideoCardId, VideoCardInterface};
use marty_pixels_scaler::{DisplayScaler, MartyScaler, ScalerMode, MartyColor};

const EGUI_MENU_BAR: u32 = 24;
pub(crate) const WINDOW_MIN_WIDTH: u32 = 640;
pub(crate) const WINDOW_MIN_HEIGHT: u32 = 480;
const DEFAULT_MAIN_WINDOW_WIDTH: u32 = WINDOW_MIN_WIDTH;
const DEFAULT_MAIN_WINDOW_HEIGHT: u32 = WINDOW_MIN_HEIGHT + EGUI_MENU_BAR;

const DEFAULT_RENDER_WINDOW_WIDTH: u32 = WINDOW_MIN_WIDTH;
const DEFAULT_RENDER_WINDOW_HEIGHT: u32 = WINDOW_MIN_HEIGHT;

const STUB_RENDER_WIDTH: u32 = 16;
const STUB_RENDER_HEIGHT: u32 = 16;

pub struct DisplayTargetDimensions {
    w: u32,
    h: u32
}

impl From<VideoDimensions> for DisplayTargetDimensions {
    fn from(t: VideoDimensions) -> Self {
        DisplayTargetDimensions{ w: t.w, h: t.h }
    }
}
impl From<DisplayTargetDimensions> for BufferDimensions {
    fn from(t: DisplayTargetDimensions) -> Self {
        BufferDimensions { w: t.w, h: t.h, pitch: t.w }
    }
}

impl From<DisplayTargetDimensions> for VideoDimensions {
    fn from(t: DisplayTargetDimensions) -> Self { VideoDimensions { w: t.w, h: t.h } }
}



#[derive (Default)]
pub struct DisplayTargetContext<T> {
    //pub(crate) event_loop: EventLoop<()>,
    pub(crate) ttype: DisplayTargetType,             // The type of display we are targeting
    pub(crate) window: Option<Window>,                // The winit window, if any
    pub(crate) gui_ctx: Option<GuiRenderContext>,     // The egui render context, if any
    pub(crate) card_id: Option<VideoCardId>,             // The video card device id, if any
    pub(crate) renderer: Option<VideoRenderer>,       // The renderer
    pub(crate) backend: Option<T>,                    // The graphics backend instance
    pub(crate) scaler: Option<Box<dyn DisplayScaler>> // The scaler pipeline
}

pub struct WgpuDisplayManagerBuilder {}

pub struct WgpuDisplayManager
{
    // All windows share a common event loop.
    event_loop: Option<EventLoop<()>>,

    // There can be multiple display windows. One for the main egui window, which may or may not
    // be attached to a videocard.
    // Optionally, one for each potential graphics adapter. For the moment I only plan to support
    // two adapters - a primary and secondary adapter. This implies a limit of 3 windows.
    // The window containing egui will always be at index 0.
    targets: Vec<DisplayTargetContext<PixelsBackend>>,

    window_id_map: HashMap<WindowId, usize>,
    card_id_map: HashMap<VideoCardId, Vec<usize>>, // Card id maps to a Vec<usize> as a single card can have multiple targets.
    primary_idx: Option<usize>,
    secondary_idx: Option<usize>,
}

impl Default for WgpuDisplayManager
{
    fn default() -> Self {
        Self {
            event_loop: None,
            targets: Vec::new(),
            window_id_map: HashMap::new(),
            card_id_map: HashMap::new(),
            primary_idx: None,
            secondary_idx: None,
        }
    }
}

impl WgpuDisplayManager {
    pub fn new() -> Self {

        let event_loop = EventLoop::new().expect("Failed to create winit event loop!");

        // We need to poll to drive events so that the emulator keeps running.
        event_loop.set_control_flow(ControlFlow::Poll);

        Self {
            event_loop: Some(event_loop),
            ..Default::default()
        }
    }

    pub fn take_event_loop(&mut self) -> EventLoop<()> {
        self.event_loop.take().unwrap()
    }
}

pub trait DefaultResolver {
    fn resolve_with_defaults(&self) -> Self;
}
impl DefaultResolver for WindowDefinition {
    fn resolve_with_defaults(&self) -> Self {
        WindowDefinition {
            name: self.name.clone(),
            size: self.size.map_or_else(|| Some(ConfigDimensions{ w: 640, h: 480 }), Some),
            card_aperture: self.card_aperture.clone(),
            scaler_aspect_correction: self.scaler_aspect_correction.map_or_else(|| Some(true), Some),
            ..*self
        }
    }
}

impl WgpuDisplayManagerBuilder {
    pub fn build(
        config: &ConfigFileParams,
        cards: Vec<VideoCardInterface>,
        icon_path: PathBuf,
        gui_options: &DisplayManagerGuiOptions,
    ) -> Result<WgpuDisplayManager, Error>
    {
        let mut dm = WgpuDisplayManager::new();

        // Only create windows if the config specifies any!
        if config.emulator.window.len() > 0 {
            // Create the main window.
            Self::create_target_from_window_def(
                &mut dm,
                true,
                &config.emulator.window[0],
                &cards,
                gui_options
            )
                .expect("FATAL: Failed to create a window target");

            // Create the rest of the windows
            for window_def in config.emulator.window.iter().skip(1) {
                Self::create_target_from_window_def(
                    &mut dm,
                    false,
                    &window_def,
                    &cards,
                    gui_options
                )
                    .expect("FATAL: Failed to create a window target");
            }
        }

        Ok(dm)
    }

    pub fn create_target_from_window_def(
        dm: &mut WgpuDisplayManager,
        main_window: bool,
        window_def: &WindowDefinition,
        cards: &Vec<VideoCardInterface>,
        gui_options: &DisplayManagerGuiOptions
    ) -> Result<(), Error> {

        let resolved_def = window_def.resolve_with_defaults();

        let mut card_id_opt = None;

        if let Some(w_card_id) = resolved_def.card_id {
            if w_card_id < cards.len() + 1 {
                card_id_opt = Some(cards[w_card_id].id);
            }
        }

        dm.create_target(
            DisplayTargetType::WindowBackground{ main_window },
            None,
            None,

            card_id_opt,
            window_def.size.unwrap().w, // Guaranteed to be Some after resolve_with_defaults();
            window_def.size.unwrap().h,
            Default::default(),
            gui_options,
        ).expect("Failed to create window target!");

        Ok(())
    }
}

impl DisplayTargetContext<PixelsBackend> {

    /// Set the aspect mode of the target. If the aspect mode is changed, we may need to resize
    /// the backend and scaler.
    pub fn set_aspect_mode(&mut self, mode: AspectCorrectionMode) {

    }

    pub fn set_scale_factor(&mut self, factor: f64) {
        if let Some(gui_ctx) = &mut self.gui_ctx {
            gui_ctx.scale_factor(factor);
        }
    }

    pub fn create_gui_context(
        window: &Window,
        pixels: &Pixels,
        gui_options: &DisplayManagerGuiOptions,
    ) -> GuiRenderContext
    {

        let win_size = window.inner_size();

        let ctx =
            GuiRenderContext::new(
                win_size.width,
                win_size.height,
                window.scale_factor(),
                pixels,
                window,
                gui_options,
            );

        ctx
    }
}

impl DisplayManager<PixelsBackend, GuiRenderContext, WindowId, Window> for WgpuDisplayManager {

    type ImplDisplayTarget = DisplayTargetContext<PixelsBackend>;
    fn create_target(
        &mut self,
        ttype: DisplayTargetType,
        wid: Option<WindowId>,
        window: Option<&Window>,
        card_id: Option<VideoCardId>,
        w: u32,
        h: u32,
        fill_color: Option<MartyColor>,
        gui_options: &DisplayManagerGuiOptions,
    ) -> Result<(), Error>
    {
        // For now, we only support creating new WindowBackground targets.
        match ttype {
            DisplayTargetType::WindowBackground{ main_window} => {
                // Create a new window.

                log::debug!("Creating WindowBackground display target, size: {}x{}", w, h);

                let window = {
                    let size =
                        LogicalSize::new(w as f64, h as f64);

                    // TODO: Better error handling here.
                    WindowBuilder::new()
                        .with_title(format!("MartyPC {}", env!("CARGO_PKG_VERSION")))
                        .with_inner_size(size)
                        .with_min_inner_size(size)
                        .build(&self.event_loop.as_ref().unwrap()).unwrap()
                };

                let wid = window.id();

                // Create the backend.
                let mut pb = PixelsBackend::new(w, h, &window)?;

                // Create the scaler.
                let marty_scaler = MartyScaler::new(
                    ScalerMode::Integer,
                    &pb.get_backend_raw().unwrap(),
                    640,480,
                    640, 480,
                    w, h,
                    24, // margin_y == egui menu height
                    true,
                    fill_color.unwrap_or_default()
                );

                // If we have a video card id, we need to build a VideoRenderer to render the card.
                let renderer = card_id.and_then(|id| {
                    let video = VideoRenderer::new(id.vtype);
                    Some(video)
                });

                // If this is the main window, create a gui context.
                let gui_ctx = if main_window {
                    log::debug!("New display target has main gui.");
                    Some(
                        DisplayTargetContext::create_gui_context(
                            &window,
                            &pb.get_backend_raw().unwrap(),
                            gui_options
                        )
                    )
                }
                else {
                    None
                };

                let dt_idx = self.targets.len();

                self.targets.push(
                    DisplayTargetContext {
                        ttype,
                        window: Some(window),
                        gui_ctx,
                        card_id,
                        renderer,       // The renderer
                        backend: Some(pb),                    // The graphics backend instance
                        scaler: Some(Box::new(marty_scaler)) // The scaler pipeline
                    }
                );

                self.window_id_map.insert(wid, 0);
                if let Some(vid) = card_id {

                    if let Some(card_vec) = self.card_id_map.get_mut(&vid) {
                        // If there's already a vec here, add the card to the vec
                        card_vec.push(0)
                    }
                    else {
                        self.card_id_map.insert(vid, vec![0]);
                    }

                    // The first card added is assumed to be the primary card
                    self.primary_idx.get_or_insert(dt_idx);
                }
            }
            _ => {
                anyhow!("Not implemented.");
            }
        }
        Ok(())
    }

    fn get_window_by_id(&self, wid: WindowId) -> Option<&Window> {
        self.window_id_map.get(&wid).and_then(|idx| {
            //log::warn!("got id, running map():");
            self.targets[*idx].window.as_ref()
        })
    }
    fn set_icon(&mut self, icon_path: PathBuf) {

        if let Ok(image) = image::open(icon_path.clone()) {

            let rgba8 = image.into_rgba8();
            let (width, height) = rgba8.dimensions();
            let icon_raw = rgba8.into_raw();

            self.targets.iter().for_each(|dt| {
                let icon = winit::window::Icon::from_rgba(icon_raw.clone(), width, height).unwrap();
                if let Some(window) = &dt.window {
                    window.set_window_icon(Some(icon));
                }
            });
        }
        else {
            log::error!("Couldn't load icon: {}", icon_path.display());
        }
    }

    fn get_main_window(&self) -> Option<&Window> {
        // Main display should always be index 0.
        self.targets[0].window.as_ref()
    }

    fn get_main_backend(&mut self) -> Option<&PixelsBackend> {
        // Main display should always be index 0.
        self.targets[0].backend.as_ref()
    }
    fn get_main_gui_mut(&mut self) -> Option<&mut GuiRenderContext> {
        self.targets[0].gui_ctx.as_mut()
    }

    fn get_main_backend_mut(&mut self) -> Option<&mut PixelsBackend> {
        // Main display should always be index 0.
        self.targets[0].backend.as_mut()
    }

    fn get_renderer_by_card_id(&mut self, id: VideoCardId) -> Option<&mut VideoRenderer> {
       //self.card_id_map.get(&id).and_then(|idx| {
        //    self.targets[*idx].renderer.as_mut()
        //})
        None
    }

    fn get_primary_renderer(&mut self) -> Option<&mut VideoRenderer> {
        self.primary_idx.and_then(|idx| {
            self.targets[idx].renderer.as_mut()
        })
    }

    /// Reflect a change to a videocard's output resolution.
    /// TODO: A card ID should eventually be able to resolve to multiple DisplayTargets.
    /// Resize the backend and scaler associated with all VideoTargets for this card, if applicable.
    fn on_card_resized(&mut self, id: VideoCardId, w: u32, h: u32) -> Result<(), Error> {
        if let Some(idx_vec) = self.card_id_map.get(&id) {

            for idx in idx_vec {
                let vt = &mut self.targets[*idx];

                let mut aspect_dimensions: Option<BufferDimensions> = None;
                let mut buf_dimensions: Option<BufferDimensions> = None;

                // Resize the VideoRenderer if present.
                if let Some(renderer) = &mut vt.renderer {
                    renderer.resize((w,h).into());

                    buf_dimensions =
                        Some(DisplayTargetDimensions::from(renderer.get_buf_dimensions()).into());
                    aspect_dimensions =
                        Some(DisplayTargetDimensions::from(renderer.get_display_dimensions()).into());
                }

                let src_dimensions =
                    buf_dimensions.unwrap_or(
                        BufferDimensions {
                            w: 16,
                            h: 16,
                            pitch: 16,
                        });
                let target_dimensions =
                    aspect_dimensions.unwrap_or(src_dimensions);

                // Resize the Backend if present.
                if let Some(backend) = &mut vt.backend {

                    let surface_dimensions = backend.surface_dimensions();

                    // Resize the DisplayScaler if present.
                    if let Some(scaler) = &mut vt.scaler {
                        scaler.resize(
                            backend.get_backend_raw().unwrap(),
                            src_dimensions.w,
                            src_dimensions.h,
                            target_dimensions.w,
                            target_dimensions.h,
                            surface_dimensions.w,
                            surface_dimensions.h
                        )
                    }
                }
            }
        }
        Ok(())
    }

    fn on_window_resized(&mut self, wid: WindowId, w: u32, h: u32) -> Result<(), Error> {
        let idx = self.window_id_map.get(&wid).context("Failed to look up window")?;
        let dt = &mut self.targets[*idx];

        if let Some(backend) = &mut dt.backend {
            log::debug!("resizing backend surface");
            backend.resize_surface(SurfaceDimensions{w, h})?;

            if let Some(scaler) = &mut dt.scaler {
                scaler.resize_surface(
                    backend.get_backend_raw().unwrap(),
                    w,
                    h,
                )
            }
        }

        if let Some(gui_ctx) = &mut dt.gui_ctx {
            log::debug!("resizing gui context for window id: {:?}", wid);
            gui_ctx.resize(w, h);
        }
        Ok(())
    }

    fn render_card(&mut self, card_id: VideoCardId) -> Result<(), Error> {

        Ok(())
    }

    fn render_all_cards(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn for_each_renderer<F>(&mut self, mut f: F)
        where
            F: FnMut(&mut VideoRenderer, VideoCardId, &mut [u8])
    {
        for dtc in &mut self.targets {
            if let Some(renderer) = &mut dtc.renderer {
                f(
                    renderer,
                    dtc.card_id.unwrap(),
                    dtc.backend.as_mut().unwrap().buf_mut()
                )
            }
        }
    }

    fn for_each_backend<F>(&mut self, mut f: F)
        where
            F: FnMut(&mut PixelsBackend, &mut dyn DisplayScaler, Option<&mut GuiRenderContext>)
    {
        for dtc in &mut self.targets {
            match dtc.ttype {
                DisplayTargetType::WindowBackground{..} => {
                    // A WindowBackground target will have a PixelsBackend.
                    if let Some(backend) = &mut dtc.backend {
                        if let Some(scaler) = &mut dtc.scaler {
                            f(backend, &mut **scaler, dtc.gui_ctx.as_mut())
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn for_each_target<F>(&mut self, f: F)
        where
            F: FnMut(&mut DisplayTargetContext<PixelsBackend>)
    {
        log::debug!("in for_each_target()!");
    }

    fn for_each_gui<F>(&mut self, mut f: F)
        where
            F: FnMut(&mut GuiRenderContext, &Window)
    {
        // Currently, only the main window can have a gui.

        if self.targets.len() > 0 {
            let dtc = &mut self.targets[0];

            if let Some(gui_ctx) = &mut dtc.gui_ctx {
                if let Some(window) = &mut dtc.window {
                    f(gui_ctx, &window)
                }
            }
        }
    }

    fn for_each_window<F>(&mut self, mut f: F)
        where
            F: FnMut(&Window)
    {
        for dtc in &mut self.targets {
            if let Some(window) = &mut dtc.window {
                f(&window)
            }
        }
    }

    fn with_target_by_wid<F>(&mut self, wid: WindowId, mut f: F)
        where
            F: FnMut(&mut DisplayTargetContext<PixelsBackend>)
    {
        if let Some(idx) = self.window_id_map.get(&wid) {
            f(&mut self.targets[*idx])
        }
    }

    fn with_gui_by_wid<F>(&mut self, wid: WindowId, mut f: F)
        where
            F: FnMut(&mut GuiRenderContext)
    {
        let mut handled = false;
        if let Some(idx) = self.window_id_map.get(&wid) {
            if let Some(gui) = &mut self.targets[*idx].gui_ctx {
                f(gui);
                handled = true;
            }
        }

        if !handled {
            log::warn!("Window event sent to None gui");
        }
    }

}

/*


impl<T,G> DisplayManager<T,G>
where
    T: DisplayBackend<G> + DisplayBackendBuilder
{
    pub fn new() -> Self {
        Default::default()
    }

    pub fn get_window_by_id(&self, wid: WindowId) -> Option<&Window> {
        self.window_id_map.get(&wid).and_then(|idx| {
            //log::warn!("got id, running map():");
            self.displays[*idx].window.as_ref()
        })
    }

    pub fn get_renderer_by_card_id(&mut self, id: usize) -> Option<&mut VideoRenderer> {
        self.card_id_map.get(&id).and_then(|idx| {
            self.displays[*idx].renderer.as_mut()
        })
    }

    /// Reflect a change to a videocard's output resolution.
    /// TODO: A card ID should eventually be able to resolve to multiple DisplayTargets.
    /// Resize the backend and scaler associated with the VideoTarget, if applicable.
    pub fn on_card_resized(&mut self, id: usize, w: u32, h: u32) -> Result<(), Error> {
        if let Some(idx) = self.card_id_map.get(&id) {

            let vt = &mut self.displays[*idx];

            let mut aspect_dimensions: Option<BufferDimensions> = None;
            let mut buf_dimensions: Option<BufferDimensions> = None;

            // Resize the VideoRenderer if present.
            if let Some(renderer) = &mut vt.renderer {
                renderer.resize((w,h).into());

                buf_dimensions =
                    Some(DisplayTargetDimensions::from(renderer.get_buf_dimensions()).into());
                aspect_dimensions =
                    Some(DisplayTargetDimensions::from(renderer.get_display_dimensions()).into());
            }

            let src_dimensions =
                buf_dimensions.unwrap_or(
                    BufferDimensions {
                        w: 16,
                        h: 16,
                        pitch: 16,
                    });
            let target_dimensions =
                aspect_dimensions.unwrap_or(src_dimensions);



            // Resize the Backend if present.
            if let Some(backend) = &mut vt.backend {

                let surface_dimensions = backend.surface_dimensions();

                // Resize the DisplayScaler if present.
                if let Some(scaler) = &mut vt.scaler {
                    scaler.resize(
                        backend.get_backend_raw().unwrap(),
                        src_dimensions.w,
                        src_dimensions.h,
                        target_dimensions.w,
                        target_dimensions.h,
                        surface_dimensions.w,
                        surface_dimensions.h
                    )
                }
            }
        }
        Ok(())
    }

    pub fn on_window_resized(&mut self, wid: WindowId, w: u32, h: u32) -> Result<(), Error> {
        let idx = self.window_id_map.get(&wid).context("Failed to look up window")?;
        if let Some(backend) = &mut self.displays[*idx].backend {
            backend.resize_surface(SurfaceDimensions{w, h})?;
        }
        Ok(())
    }

    pub fn set_icon(&mut self, icon_path: PathBuf) {

        if let Ok(image) = image::open(icon_path.clone()) {

            let rgba8 = image.into_rgba8();
            let (width, height) = rgba8.dimensions();
            let icon_raw = rgba8.into_raw();

            self.displays.iter().for_each(|dt| {
                let icon = winit::window::Icon::from_rgba(icon_raw.clone(), width, height).unwrap();
                if let Some(window) = &dt.window {
                    window.set_window_icon(Some(icon));
                }
            });
        }
        else {
            log::error!("Couldn't load icon: {}", icon_path.display());
        }
    }
}

 */
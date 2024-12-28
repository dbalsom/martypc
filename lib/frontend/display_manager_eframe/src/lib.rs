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

use marty_common::*;

use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use marty_core::{
    device_traits::videocard::{DisplayApertureType, DisplayExtents, VideoCardId},
    file_util,
    machine::Machine,
};

use marty_common::VideoDimensions;

pub use display_backend_eframe::{
    BufferDimensions,
    DisplayBackend,
    DisplayBackendBuilder,
    EFrameBackend,
    SurfaceDimensions,
};

pub use frontend_common::{
    color::MartyColor,
    display_manager::{DisplayManager, DisplayTargetDimensions, DisplayTargetType, DmGuiOptions, DmViewportOptions},
};
use frontend_common::{
    constants::*,
    display_manager::DisplayTargetInfo,
    display_scaler::{PhosphorType, ScalerFilter, ScalerOption, ScalerParams, ScalerPreset},
    types::{display_target_margins::DisplayTargetMargins, window::WindowDefinition},
};
//use marty_egui::context::GuiRenderContext;
use marty_pixels_scaler::{DisplayScaler, MartyScaler, ScalerMode};
use videocard_renderer::{AspectCorrectionMode, AspectRatio, VideoRenderer};

use anyhow::{anyhow, Error};
use display_backend_eframe::EFrameBackendType;
use eframe::{
    egui::{Context, ViewportId},
    wgpu::{CommandEncoder, TextureView},
};
use frontend_common::display_manager::DtHandle;
use marty_egui_eframe::context::GuiRenderContext;
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Icon, Window, WindowButtons, WindowId, WindowLevel},
};

macro_rules! is_valid_handle {
    ($dt:expr, $other:expr) => {
        $dt.idx() < $other.len()
    };
}

macro_rules! is_bad_handle {
    ($dt:expr, $other:expr) => {
        $dt.idx() >= $other.len()
    };
}

macro_rules! resolve_handle {
    ($handle:expr, $other:expr, $closure:expr) => {
        if $handle.idx() < $other.len() {
            $closure($other.get($handle.idx()).unwrap())
        }
        else {
            return Err(anyhow::anyhow!("Handle out of range!"));
        }
    };
}

macro_rules! resolve_handle_mut {
    ($handle:expr, $other:expr, $closure:expr) => {
        if $handle.idx() < $other.len() {
            $closure($other.get_mut($handle.idx()).unwrap())
        }
        else {
            return Err(anyhow::anyhow!("Handle out of range!"));
        }
    };
}

macro_rules! resolve_handle_result {
    ($handle:expr, $other:expr, $closure:expr) => {
        if $handle.idx() < $other.len() {
            return Ok($closure($other.get($handle.idx()).unwrap()));
        }
        else {
            return Err(anyhow::anyhow!("Handle out of range!"));
        }
    };
}

macro_rules! resolve_handle_mut_result {
    ($handle:expr, $other:expr, $closure:expr) => {
        if $handle.idx() < $other.len() {
            return Ok($closure($other.get_mut($handle.idx()).unwrap()));
        }
        else {
            return Err(anyhow::anyhow!("Handle out of range!"));
        }
    };
}

macro_rules! resolve_handle_opt {
    ($handle:expr, $other:expr, $closure:expr) => {
        if $handle.idx() < $other.len() {
            $closure($other.get($handle.idx()).unwrap())
        }
        else {
            return None;
        }
    };
}

const EGUI_MENU_BAR: u32 = 24;

/*
pub(crate) const WINDOW_MIN_WIDTH: u32 = 640;
pub(crate) const WINDOW_MIN_HEIGHT: u32 = 480;
const DEFAULT_MAIN_WINDOW_WIDTH: u32 = WINDOW_MIN_WIDTH;
const DEFAULT_MAIN_WINDOW_HEIGHT: u32 = WINDOW_MIN_HEIGHT + EGUI_MENU_BAR;
const DEFAULT_RENDER_WINDOW_WIDTH: u32 = WINDOW_MIN_WIDTH;
const DEFAULT_RENDER_WINDOW_HEIGHT: u32 = WINDOW_MIN_HEIGHT;
const STUB_RENDER_WIDTH: u32 = 16;
const STUB_RENDER_HEIGHT: u32 = 16;
*/

const RESOLVE_BUFFER: u32 = 0x01;
const RESOLVE_RENDERER: u32 = 0x02;
//const RESOLVE_SCALER: u32 = 0x04;
const RESOLVE_SURFACE: u32 = 0x08;
const RESOLVE_WINDOW: u32 = 0x10;

#[derive(Default)]
pub struct DisplayTargetParams {
    buf_dim: DisplayTargetDimensions,     // The native size of the backend buffer in pixels.
    render_dim: DisplayTargetDimensions,  // The rendered size of the backend buffer in pixels.
    surface_dim: DisplayTargetDimensions, // The surface size of the display target in pixels. This is usually the same as window_dim.
    window_dim: DisplayTargetDimensions,  // The window client area size in pixels.
}

pub struct ResizeTarget {
    pub w: u32,
    pub h: u32,
}

impl DisplayTargetParams {
    /// Given requested display target parameters, return true if they represent a difference that requires the display
    /// target to reconfigure (resolve) one or more of its components, and if so, flags indicating which components need to be resolved.
    fn need_to_resolve(&self, requested: &DisplayTargetParams) -> (bool, u32) {
        let mut resolve_flags = 0;
        if self.buf_dim != requested.buf_dim {
            resolve_flags |= RESOLVE_BUFFER;
        }
        if self.render_dim != requested.render_dim {
            resolve_flags |= RESOLVE_RENDERER;
        }
        if self.surface_dim != requested.surface_dim {
            resolve_flags |= RESOLVE_SURFACE;
        }
        if self.window_dim != requested.window_dim {
            resolve_flags |= RESOLVE_WINDOW;
        }
        (resolve_flags != 0, resolve_flags)
    }
}

#[derive(Default)]
pub struct DisplayTargetContext<T> {
    //pub(crate) event_loop: EventLoop<()>,
    pub name: String,
    pub dt_type: DisplayTargetType, // The type of display we are targeting
    pub initialized: bool,
    pub resolved_params: DisplayTargetParams,
    pub requested_params: Option<DisplayTargetParams>,
    pub viewport: Option<ViewportId>, // The EGUI ViewportId
    pub window_opts: Option<DmViewportOptions>,
    pub(crate) gui_ctx: Option<GuiRenderContext>, // The egui render context, if any
    pub(crate) card_id: Option<VideoCardId>,      // The video card device id, if any
    pub(crate) renderer: Option<VideoRenderer>,   // The renderer
    pub(crate) aspect_ratio: AspectRatio,         // Aspect ratio configured for this display
    pub(crate) backend: Option<T>,                // The graphics backend instance
    pub(crate) scaler:
        Option<Box<dyn DisplayScaler<(), NativeTextureView = TextureView, NativeEncoder = CommandEncoder>>>, // The scaler pipeline
    pub(crate) scaler_params: Option<ScalerParams>,
    pub(crate) card_scale: Option<f32>, // If Some, the card resolution is scaled by this factor
}

pub struct EFrameDisplayManagerBuilder {}

pub struct EFrameDisplayManager {
    // All windows share a common event loop.
    //event_loop: Option<EventLoop<()>>,

    // There can be multiple display windows. One for the main egui window, which may or may not
    // be attached to a videocard.
    // Optionally, one for each potential graphics adapter. For the moment I only plan to support
    // two adapters - a primary and secondary adapter. This implies a limit of 3 windows.
    // The window containing egui will always be at index 0.
    targets: Vec<DisplayTargetContext<EFrameBackend>>,
    viewport_id_map: MartyHashMap<ViewportId, usize>,
    viewport_id_resize_requests: MartyHashMap<ViewportId, ResizeTarget>,
    card_id_map: MartyHashMap<VideoCardId, Vec<usize>>, // Card id maps to a Vec<usize> as a single card can have multiple targets.
    primary_idx: Option<usize>,
    scaler_presets: MartyHashMap<String, ScalerPreset>,
}

impl Default for EFrameDisplayManager {
    fn default() -> Self {
        Self {
            //event_loop: None,
            targets: Vec::new(),
            viewport_id_map: MartyHashMap::default(),
            viewport_id_resize_requests: MartyHashMap::default(),
            card_id_map: MartyHashMap::default(),
            primary_idx: None,
            scaler_presets: MartyHashMap::default(),
        }
    }
}

impl EFrameDisplayManager {
    pub fn new() -> Self {
        Default::default()
    }

    // pub fn take_event_loop(&mut self) -> EventLoop<()> {
    //     self.event_loop.take().unwrap()
    // }
}

pub trait DefaultResolver {
    fn resolve_with_defaults(&self) -> Self;
}
impl DefaultResolver for WindowDefinition {
    fn resolve_with_defaults(&self) -> Self {
        WindowDefinition {
            name: self.name.clone(),
            size: self.size.map_or_else(
                || {
                    Some(VideoDimensions {
                        w: DEFAULT_RESOLUTION_W,
                        h: DEFAULT_RESOLUTION_H,
                    })
                },
                Some,
            ),
            scaler_preset: self.scaler_preset.clone(),
            ..*self
        }
    }
}

/// Display managers should be constructed via a [DisplayManagerBuilder]. This allows display targets
/// to be created as specified by a user-supplied configuration. For [EFrameDisplayManager], we build
/// our display targets using:
///
/// - the user configuration file
/// - a list of video cards from the emulator core
/// - a list of scaler preset definitions
/// - a path to an icon (TODO: support different icons per window?)
/// - a struct of GUI options
impl EFrameDisplayManagerBuilder {
    pub fn build(
        egui_ctx: Context,
        win_configs: &[WindowDefinition],
        cards: Vec<VideoCardId>,
        scaler_presets: &Vec<ScalerPreset>,
        icon_path: Option<PathBuf>,
        icon_buf: Option<&[u8]>,
        gui_options: &DmGuiOptions,
    ) -> Result<EFrameDisplayManager, Error> {
        let icon = {
            if let Some(path) = icon_path {
                if let Ok(image) = image::open(path.clone()) {
                    log::debug!("Using icon from path: {}", path.display());
                    let rgba8 = image.into_rgba8();
                    let (width, height) = rgba8.dimensions();
                    let icon_raw = rgba8.into_raw();

                    let icon = winit::window::Icon::from_rgba(icon_raw.clone(), width, height)?;

                    Some(icon)
                }
                else {
                    log::error!("Couldn't load icon: {}", path.display());
                    log::error!("Couldn't load icon: {}", path.display());
                    None
                }
            }
            else {
                if let Some(buf) = icon_buf {
                    if let Ok(image) = image::load_from_memory(buf) {
                        let rgba8 = image.into_rgba8();
                        let (width, height) = rgba8.dimensions();
                        let icon_raw = rgba8.into_raw();

                        let icon = winit::window::Icon::from_rgba(icon_raw.clone(), width, height)?;

                        Some(icon)
                    }
                    else {
                        log::error!("Couldn't load icon from buffer.");
                        None
                    }
                }
                else {
                    log::warn!("No icon specified.");
                    None
                }
            }
        };

        let mut dm = EFrameDisplayManager::new();

        // Install scaler presets
        for preset in scaler_presets.iter() {
            log::debug!("Installing scaler preset: {}", &preset.name);
            dm.add_scaler_preset(preset.clone());
        }

        // Only create windows if the config specifies any!
        if win_configs.len() > 0 {
            // Create the main window.
            Self::create_target_from_window_def(
                &mut dm,
                egui_ctx.clone(),
                true,
                &win_configs[0],
                &cards,
                gui_options,
                icon.clone(),
            )
            .expect("FATAL: Failed to create a window target");

            // TODO: Reimplement this for egui Viewports

            // // Create the rest of the windows
            // for window_def in win_configs.iter().skip(1) {
            //     if window_def.enabled {
            //         Self::create_target_from_window_def(
            //             &mut dm,
            //             egui_ctx.clone(),
            //             false,
            //             &window_def,
            //             &cards,
            //             gui_options,
            //             icon.clone(),
            //         )
            //         .expect("FATAL: Failed to create a window target");
            //     }
            // }
        }

        Ok(dm)
    }

    pub fn create_target_from_window_def(
        dm: &mut EFrameDisplayManager,
        egui_ctx: Context,
        main_window: bool,
        window_def: &WindowDefinition,
        cards: &Vec<VideoCardId>,
        gui_options: &DmGuiOptions,
        icon: Option<Icon>,
    ) -> Result<(), Error> {
        let resolved_def = window_def.resolve_with_defaults();
        log::debug!("{:?}", window_def);

        let mut card_id_opt = None;
        let mut card_string = String::new();

        if let Some(w_card_id) = resolved_def.card_id {
            if w_card_id < cards.len() {
                card_id_opt = Some(cards[w_card_id]);
                card_string.push_str(&format!("{:?}", cards[w_card_id].vtype))
            }
            card_string.push_str(&format!("({})", w_card_id));
        }

        log::debug!(
            "Creating WindowBackground display target from window definition with card id: {:?}",
            card_id_opt
        );

        // TODO: Implement FROM for this?
        let mut window_opts: DmViewportOptions = Default::default();

        // Honor initial window size, but we may have to resize it later.
        window_opts.size = window_def.size.unwrap_or_default().into();
        window_opts.always_on_top = window_def.always_on_top;

        // If this is the main window, and we have a GUI...
        if main_window && gui_options.enabled {
            // Set the top margin to clear the egui menu bar.
            window_opts.margins = DisplayTargetMargins::from_t(gui_options.menubar_h);
        }

        // Is window resizable?
        if !window_def.resizable {
            window_opts.min_size = Some(window_opts.size);
            window_opts.max_size = Some(window_opts.size);
            window_opts.resizable = false;
        }
        else {
            window_opts.resizable = true;
        }

        // If this is Some, it locks the window resolution to some scale factor of card resolution
        window_opts.card_scale = window_def.card_scale;

        let preset_name = window_def.scaler_preset.clone().unwrap_or("default".to_string());

        // Construct window title.
        let window_title = format!("{}: {}", &window_def.name, card_string).to_string();

        let dt_type = DisplayTargetType::WindowBackground {
            main_window,
            has_gui: main_window,
            has_menu: main_window,
        };

        let dt_type = DisplayTargetType::GuiWidget {
            main_window,
            has_gui: main_window,
            has_menu: main_window,
        };

        dm.create_target(
            window_title,
            dt_type,
            Some(&egui_ctx),
            Some(window_opts),
            card_id_opt,
            preset_name,
            gui_options,
        )
        .expect("Failed to create window target!");

        let last_idx = dm.targets.len() - 1;

        // TODO: figure out how to set icon here
        //dm.targets[last_idx].window.as_mut().unwrap().set_window_icon(icon);

        Ok(())
    }
}

impl DisplayTargetContext<EFrameBackend> {
    /// Set the aspect mode of the target. If the aspect mode is changed, we may need to resize
    /// the backend and scaler.
    pub fn set_aspect_mode(&mut self, _mode: AspectCorrectionMode) {}

    pub fn get_card_id(&mut self) -> Option<VideoCardId> {
        self.card_id
    }

    pub fn set_scale_factor(&mut self, factor: f64) {
        // if let Some(gui_ctx) = &mut self.gui_ctx {
        //     gui_ctx.scale_factor(factor);
        // }
    }

    pub fn set_on_top(&mut self, on_top: bool) {
        if let Some(wopts) = &mut self.window_opts {
            wopts.is_on_top = on_top;
        }
    }

    pub fn is_on_top(&self) -> bool {
        if let Some(wopts) = &self.window_opts {
            return wopts.is_on_top;
        }
        false
    }

    // pub fn create_gui_context(
    //     dt_idx: usize,
    //     window: &Window,
    //     w: u32,
    //     h: u32,
    //     pixels: &Pixels,
    //     gui_options: &DisplayManagerGuiOptions,
    // ) -> GuiRenderContext {
    //     let scale_factor = window.scale_factor();
    //     log::debug!(
    //         "Creating GUI context with size: [{}x{}] (scale factor: {})",
    //         w,
    //         h,
    //         scale_factor
    //     );
    //     GuiRenderContext::new(dt_idx, w, h, scale_factor, pixels, window, gui_options)
    // }

    pub fn apply_scaler_preset(&mut self, preset: &ScalerPreset) {
        // We must have a backend and scaler to continue...
        if !self.backend.is_some() || !self.scaler.is_some() {
            return;
        }
        log::debug!("Applying scaler preset: {}", &preset.name);

        let bilinear = match preset.filter {
            ScalerFilter::Linear => true,
            ScalerFilter::Nearest => false,
        };
        let scaler = self.scaler.as_mut().unwrap();

        scaler.set_mode(
            self.backend.as_mut().unwrap().get_backend_raw().unwrap(),
            preset.mode.unwrap_or(scaler.get_mode()),
        );
        scaler.set_bilinear(bilinear);
        scaler.set_fill_color(MartyColor::from_u24(preset.border_color.unwrap_or(0)));

        self.apply_scaler_params(&ScalerParams::from(preset.clone()));

        // Scaler preset also has certain renderer parameters. Set them now.
        if let Some(renderer) = &mut self.renderer {
            if let Some(aperture) = preset.renderer.display_aperture {
                log::debug!("apply_scaler_preset(): Setting aperture to: {:?}", &aperture);
                renderer.set_aperture(aperture);
            }
            if preset.renderer.aspect_correction {
                renderer.set_aspect_ratio(preset.renderer.aspect_ratio, Some(AspectCorrectionMode::Hardware));
            }
            renderer.set_composite(preset.renderer.composite);
        }
    }

    pub fn apply_scaler_params(&mut self, params: &ScalerParams) {
        // We must have a backend and scaler to continue...
        if !self.backend.is_some() || !self.scaler.is_some() {
            return;
        }

        // Update params on dt
        self.scaler_params = Some(params.clone());

        let mut scaler_update = Vec::new();

        scaler_update.push(ScalerOption::Geometry {
            h_curvature:   params.crt_barrel_distortion,
            v_curvature:   params.crt_barrel_distortion,
            corner_radius: params.crt_corner_radius,
        });

        scaler_update.push(ScalerOption::Adjustment {
            h: 1.0,
            s: 1.0,
            c: 1.0,
            b: 1.0,
            g: params.gamma,
        });

        scaler_update.push(ScalerOption::Filtering(params.filter));

        if let Some(renderer) = &self.renderer {
            let rparams = renderer.get_params();

            let lines = if rparams.line_double {
                rparams.render.h / 2
            }
            else {
                rparams.render.h
            };
            log::debug!(
                "Setting scaler scanlines to {}, doublescan: {}",
                lines,
                rparams.line_double
            );
            scaler_update.push(ScalerOption::Scanlines {
                enabled: Some(params.crt_scanlines),
                lines: Some(lines),
                intensity: Some(0.3),
            });
        }
        else {
            // If there's no renderer, disable scanlines
            scaler_update.push(ScalerOption::Scanlines {
                enabled: Some(false),
                lines: Some(0),
                intensity: Some(0.0),
            });
        }

        match params.crt_phosphor_type {
            PhosphorType::Color => scaler_update.push(ScalerOption::Mono {
                enabled: false,
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            }),
            PhosphorType::White => scaler_update.push(ScalerOption::Mono {
                enabled: true,
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            }),
            PhosphorType::Green => scaler_update.push(ScalerOption::Mono {
                enabled: true,
                r: 0.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            }),
            PhosphorType::Amber => scaler_update.push(ScalerOption::Mono {
                enabled: true,
                r: 1.0,
                g: 0.75,
                b: 0.0,
                a: 1.0,
            }),
        }

        self.scaler
            .as_mut()
            .unwrap()
            .set_options(self.backend.as_mut().unwrap().get_backend_raw().unwrap(), scaler_update);
    }

    pub fn request_params(&mut self, params: DisplayTargetParams) {
        if self.requested_params.is_some() {
            log::warn!("Requesting param change with unresolved param request pending.");
        }

        if self.resolved_params.need_to_resolve(&params).0 {
            log::debug!("Requesting param change for display target {}.", self.name);
            self.requested_params = Some(params);
        }
    }

    /// Resolve a pending parameter request, resizing all display components as required.
    pub fn resolve(&mut self) {
        // Nothing to update!
        if self.requested_params.is_none() {
            return;
        }

        let new_params = self.requested_params.as_ref().unwrap();
        let resolve_flags = self.resolved_params.need_to_resolve(&new_params).1;

        if resolve_flags & RESOLVE_BUFFER != 0 {
            // Resize the backend buffer.
        }

        // Set requested parameters to resolved parameters.
    }
}

impl<'p> DisplayManager<EFrameBackend, GuiRenderContext, ViewportId, ViewportId, Context> for EFrameDisplayManager {
    type NativeTextureView = TextureView;
    type NativeEncoder = CommandEncoder;
    type NativeEventLoop = ActiveEventLoop;
    type ImplScaler = Box<dyn DisplayScaler<(), NativeTextureView = TextureView, NativeEncoder = CommandEncoder>>;
    type ImplDisplayTarget = DisplayTargetContext<EFrameBackend>;

    fn create_target(
        &mut self,
        name: String,
        dt_type: DisplayTargetType,
        native_context: Option<&eframe::egui::Context>,
        window_opts: Option<DmViewportOptions>,
        card_id: Option<VideoCardId>,
        scaler_preset: String,
        gui_options: &DmGuiOptions,
    ) -> Result<DtHandle, Error> {
        // For now, we only support creating new WindowBackground targets.
        match dt_type {
            DisplayTargetType::GuiWidget {
                main_window,
                has_gui: _,
                has_menu,
            }
            | DisplayTargetType::WindowBackground {
                main_window,
                has_gui: _,
                has_menu,
            } if main_window => {
                // Create a display target for the main viewport.
                // In this case, since we are using eframe, the main (root) viewport is already open.

                // Attempt to resolve the specified scaler preset
                let scaler_preset = match self.get_scaler_preset(scaler_preset) {
                    Some(preset) => preset.clone(),
                    None => {
                        return Err(anyhow!("Couldn't load scaler preset!"));
                    }
                };

                // Use the dimensions specified in window options, if supplied, otherwise fall back to default
                let ((tw, th), resizable) = if let Some(ref window_opts) = window_opts {
                    (window_opts.size.into(), window_opts.resizable)
                }
                else {
                    ((800, 600), true)
                };

                let dt_idx = self.targets.len();

                // TODO: Replace this with whatever is the current method
                // let native_ppp = Self::get_native_pixels_per_point(self.event_loop.as_ref().unwrap());
                // let sw = (tw as f32 * native_ppp) as u32;
                // let sh = (th as f32 * native_ppp) as u32;
                let sw = tw;
                let sh = th;

                log::debug!(
                    "Creating WindowBackground display target, main window: {} idx: {} requested size: {}x{} scaled size: {}x{} (factor:) preset: {}",
                    main_window,
                    dt_idx,
                    tw,
                    th,
                    sw,
                    sh,
                    //native_ppp,
                    &scaler_preset.name
                );

                // let window = {
                //     let physical_size = PhysicalSize::new(tw as f64, th as f64);
                //     let logical_size = LogicalSize::new(sw as f64, sh as f64);
                //
                //     let level = match &window_opts {
                //         Some(wopts) if wopts.always_on_top == true => {
                //             log::debug!("Setting window always_on_top.");
                //             WindowLevel::AlwaysOnTop
                //         }
                //         _ => WindowLevel::Normal,
                //     };
                //
                //     let attributes = {
                //         let buttons = match resizable {
                //             true => WindowButtons::all(),
                //             false => WindowButtons::empty(),
                //         };
                //         Window::default_attributes()
                //             .with_title(format!("MartyPC {} [{}]", env!("CARGO_PKG_VERSION"), name))
                //             .with_inner_size(physical_size)
                //             .with_min_inner_size(physical_size)
                //             .with_resizable(resizable)
                //             .with_enabled_buttons(buttons)
                //             .with_window_level(level)
                //     };
                //
                //     event_loop.create_window(attributes)?
                //
                //     //let window = Arc::new(&self.event_loop.create_window(attributes).unwrap());
                // };

                // let wid = window.id();
                // let scale_factor = window.scale_factor();

                let menubar_h = if has_menu {
                    //(EGUI_MENU_BAR as f64 * scale_factor) as u32
                    EGUI_MENU_BAR
                }
                else {
                    0
                };

                // Create the backend.
                let mut pb = EFrameBackend::new(
                    EFrameBackendType::EguiWindow,
                    native_context.unwrap().clone(),
                    BufferDimensions {
                        w: tw,
                        h: th,
                        pitch: tw,
                    },
                    SurfaceDimensions { w: sw, h: sh },
                    None,
                )?;

                // Create the scaler.
                let _scale_mode = match main_window {
                    true => ScalerMode::Integer,
                    false => ScalerMode::Fixed,
                };

                // The texture sizes specified initially aren't important. Since DisplyManager can't
                // query video cards directly, the caller must resize all video cards after calling
                // the Builder.
                // let scaler = MartyScaler::new(
                //     scaler_preset.mode.unwrap_or(ScalerMode::Integer),
                //     &pb.get_backend_raw().unwrap(),
                //     640,
                //     480,
                //     640,
                //     480,
                //     w,
                //     h,
                //     menubar_h, // margin_y == egui menu height
                //     true,
                //     MartyColor::from_u24(scaler_preset.border_color.unwrap_or_default()),
                // );

                // If we have a video card id, we need to build a VideoRenderer to render the card.
                let renderer = if let Some(card_id) = card_id {
                    log::debug!(
                        "New display target {} has renderer. Card type: {:?} Parameters: {:?}",
                        dt_idx,
                        card_id.vtype,
                        &scaler_preset.renderer
                    );
                    let mut video = VideoRenderer::new(card_id.vtype);

                    video.set_config_params(&scaler_preset.renderer);
                    Some(video)
                }
                else {
                    log::warn!("New display target {} has no video card!", dt_idx);
                    None
                };

                // // If window has a gui, create a gui context.
                // let gui_ctx = if main_window {
                //     log::debug!("New display target {} has main gui.", dt_idx);
                //     Some(DisplayTargetContext::create_gui_context(
                //         dt_idx,
                //         &window,
                //         w,
                //         h,
                //         //&pb.get_backend_raw().unwrap(),
                //         gui_options,
                //     ))
                // }
                // else {
                //     log::debug!("Skipping creation of gui context for target {}", dt_idx);
                //     None
                // };

                let card_scale = window_opts.as_ref().and_then(|wo| wo.card_scale);

                let mut dtc = DisplayTargetContext {
                    name,
                    dt_type,
                    initialized: false,
                    resolved_params: DisplayTargetParams {
                        buf_dim: DisplayTargetDimensions::new(tw, th),
                        render_dim: DisplayTargetDimensions::new(tw, th),
                        surface_dim: DisplayTargetDimensions::new(tw, th),
                        window_dim: DisplayTargetDimensions::new(tw, th),
                    },
                    requested_params: None,
                    viewport: None,
                    window_opts,
                    gui_ctx: None,
                    card_id,
                    renderer,
                    aspect_ratio: scaler_preset.renderer.aspect_ratio.unwrap_or_default(),
                    backend: Some(pb), // The graphics backend instance
                    scaler: None,
                    //scaler: Some(Box::new(scaler)), // The scaler pipeline
                    scaler_params: Some(ScalerParams::from(scaler_preset.clone())),
                    card_scale,
                };

                dtc.apply_scaler_preset(&scaler_preset);

                self.targets.push(dtc);

                self.viewport_id_map.insert(eframe::egui::ViewportId::ROOT, dt_idx);

                if let Some(vid) = card_id {
                    if let Some(card_vec) = self.card_id_map.get_mut(&vid) {
                        // If there's already a vec here, add the target index to the vec.
                        card_vec.push(dt_idx)
                    }
                    else {
                        self.card_id_map.insert(vid, vec![dt_idx]);
                    }

                    // The first card added is assumed to be the primary card
                    self.primary_idx.get_or_insert(dt_idx);
                }

                Ok(DtHandle(dt_idx))
            }
            _ => Err(anyhow!("Not implemented.")),
        }
    }

    fn display_info(&self, machine: &Machine) -> Vec<DisplayTargetInfo> {
        let mut info_vec = Vec::new();

        for vt in self.targets.iter() {
            let mut vtype = None;
            if let Some(vid) = vt.card_id {
                vtype = machine.bus().video(&vid).and_then(|card| Some(card.get_video_type()));
            }

            let mut render_time = Duration::from_secs(0);
            let renderer_params = if let Some(renderer) = &vt.renderer {
                render_time = renderer.get_last_render_time();
                Some(renderer.get_config_params().clone())
            }
            else {
                None
            };

            let mut scaler_mode = None;
            if let Some(scaler) = &vt.scaler {
                scaler_mode = Some(scaler.get_mode());
            }

            let mut has_gui = false;
            let mut gui_render_time = Duration::ZERO;
            // if let Some(gui_ctx) = &vt.gui_ctx {
            //     has_gui = true;
            //     gui_render_time = gui_ctx.get_render_time();
            // }

            let mut backend_name = String::new();
            if let Some(backend) = &vt.backend {
                backend_name = backend
                    .get_adapter_info()
                    .map(|info| format!("{:?} ({})", info.backend, info.name))
                    .unwrap_or_default();
            }

            info_vec.push(DisplayTargetInfo {
                backend_name,
                dtype: vt.dt_type,
                vtype,
                vid: vt.card_id,
                name: vt.name.clone(),
                renderer: renderer_params,
                render_time,
                contains_gui: has_gui,
                gui_render_time,
                scaler_mode,
                scaler_params: vt.scaler_params,
            })
        }

        info_vec
    }

    fn viewport_by_id(&self, _vid: ViewportId) -> Option<&ViewportId> {
        None
        // self.viewport_id_map.get(&wid).and_then(|idx| {
        //     //log::warn!("got id, running map():");
        //     self.targets[*idx].window.as_ref()
        // })
    }

    fn viewport(&self, _dt: DtHandle) -> Option<&ViewportId> {
        //self.targets.get(dt.idx()).and_then(|dt| dt.window.as_ref())
        None
    }

    fn main_viewport(&self) -> Option<&ViewportId> {
        // Main display should always be index 0.
        self.targets[0].viewport.as_ref()
    }

    fn main_backend(&mut self) -> Option<&EFrameBackend> {
        // Main display should always be index 0.
        self.targets[0].backend.as_ref()
    }
    fn main_backend_mut(&mut self) -> Option<&mut EFrameBackend> {
        // Main display should always be index 0.
        self.targets[0].backend.as_mut()
    }

    fn main_gui_mut(&mut self) -> Option<&mut GuiRenderContext> {
        self.targets[0].gui_ctx.as_mut()
    }

    fn gui_by_viewport_id(&mut self, vid: ViewportId) -> Option<&mut GuiRenderContext> {
        self.viewport_id_map
            .get(&vid)
            .and_then(|idx| self.targets[*idx].gui_ctx.as_mut())
            .or_else(|| {
                //log::warn!("get_gui_by_window_id(): No gui context for window id: {:?}", wid);
                None
            })
    }

    fn renderer_mut(&mut self, dt: DtHandle) -> Option<&mut VideoRenderer> {
        if dt.idx() < self.targets.len() {
            self.targets[dt.idx()].renderer.as_mut()
        }
        else {
            None
        }
    }

    fn renderer_by_card_id_mut(&mut self, _id: VideoCardId) -> Option<&mut VideoRenderer> {
        //self.card_id_map.get(&id).and_then(|idx| {
        //    self.targets[*idx].renderer.as_mut()
        //})
        None
    }

    fn primary_renderer_mut(&mut self) -> Option<&mut VideoRenderer> {
        self.primary_idx.and_then(|idx| self.targets[idx].renderer.as_mut())
    }

    /// Reflect a potential update to a videocard's output resolution. This can be called once
    /// per frame regardless of whether we anticipate the card resolution actually changed.
    /// This method needs to resize the resolution of the backend, renderer and scaler associated
    /// with all VideoTargets registered for this card.
    /// If the renderer for a display target reports that it would not resize given the updated card
    /// resolution, then we do nothing for that display target.
    /// A renderer and scaler can be updated even if the card resolution has not changed, if aspect
    /// correction was toggled on the renderer since the last update.
    fn on_card_resized(&mut self, vid: &VideoCardId, extents: &DisplayExtents) -> Result<(), Error> {
        if let Some(idx_vec) = self.card_id_map.get(vid) {
            // A single card can be mapped to multiple display targets, so iterate through them.

            // log::debug!("card {:?} has {} display targets", id, idx_vec.len());
            for idx in idx_vec {
                let dtc = &mut self.targets[*idx];

                let mut aspect_dimensions: Option<BufferDimensions> = None;
                let mut buf_dimensions: Option<BufferDimensions> = None;

                let mut resize_dt = false;
                let mut software_aspect = false;

                // Get the VideoRenderer for this display target, and determine whether the renderer
                // (and thus the backend and scaler) should resize.
                if let Some(renderer) = &mut dtc.renderer {
                    // Inform the renderer if the card is to be double-scanned
                    renderer.set_line_double(extents.double_scan);

                    software_aspect = matches!(renderer.get_params().aspect_correction, AspectCorrectionMode::Software);

                    let aperture = renderer.get_params().aperture;
                    let w = extents.apertures[aperture as usize].w;
                    let mut h = extents.apertures[aperture as usize].h;

                    if extents.double_scan {
                        h *= 2;
                    }

                    resize_dt = renderer.would_resize((w, h).into()) || !dtc.initialized;

                    if resize_dt {
                        log::debug!(
                            "on_card_resized(): Card {:?} init:{} new aperture: {}x{} [Doublescan: {}, Aperture: {:?}] Resizing renderer for dt {}...",
                            vid,
                            dtc.initialized,
                            w,
                            h,
                            extents.double_scan,
                            aperture,
                            idx
                        );
                        renderer.resize((w, h).into());
                        dtc.initialized = true;
                    }

                    buf_dimensions = Some(DisplayTargetDimensions::from(renderer.get_buf_dimensions()).into());
                    aspect_dimensions = Some(DisplayTargetDimensions::from(renderer.get_display_dimensions()).into());
                }

                // If no renderer was present we set a minimum placeholder buffer size for backend.
                let src_dimensions = buf_dimensions.unwrap_or(BufferDimensions {
                    w: 16,
                    h: 16,
                    pitch: 16,
                });
                let target_dimensions = aspect_dimensions.unwrap_or(src_dimensions);

                // Resize the Backend and Scaler if the renderer resized.
                if resize_dt {
                    let mut resize_surface = false;

                    let top_margin = dtc.window_opts.as_ref().map_or(0, |opts| opts.margins.t);

                    // Calculate the minimum client area we need (including top margin for gui menu)
                    let mut new_min_surface_size = match dtc.card_scale {
                        Some(card_scale) => {
                            // Card scaling is enabled. Scale the window to the specified factor, even
                            // if that would shrink the window.
                            PhysicalSize::new(
                                (target_dimensions.w as f32 * card_scale) as u32,
                                (target_dimensions.h as f32 * card_scale) as u32 + top_margin,
                            )
                        }
                        _ => PhysicalSize::new(target_dimensions.w, target_dimensions.h + top_margin),
                    };

                    // First we need to see if the viewport needs resizing. If the renderer increased
                    // resolution, we may need to make the viewport bigger to fit. We don't support
                    // scaling downwards.
                    if let Some(viewport) = &mut dtc.viewport {
                        // TODO: fix all this for eframe viewports

                        // First, get the inner size of the window. We may not need to resize it if
                        // its already big enough and we don't have card scaling on.

                        // let win_dim = window.inner_size();
                        //
                        // if dtc.card_scale.is_some() {
                        //     window.set_max_inner_size(Some(new_min_surface_size));
                        //     window.set_min_inner_size(Some(new_min_surface_size));
                        // }
                        // else {
                        //     if win_dim.width < new_min_surface_size.width
                        //         || win_dim.height < new_min_surface_size.height
                        //     {
                        //         // Window is too small in at least one dimension.
                        //         new_min_surface_size = PhysicalSize::new(
                        //             std::cmp::max(win_dim.width, new_min_surface_size.width),
                        //             std::cmp::max(win_dim.height, new_min_surface_size.height),
                        //         );
                        //     }
                        //     else {
                        //         // Window is big enough, retain size
                        //         new_min_surface_size = PhysicalSize::new(win_dim.width, win_dim.height);
                        //     }
                        // }
                        //
                        // log::debug!(
                        //     "on_card_resized(): Resizing window to fit new calculated surface. {}x{} => {}x{} card_scale: {}",
                        //     win_dim.width,
                        //     win_dim.height,
                        //     new_min_surface_size.width,
                        //     new_min_surface_size.height,
                        //     dtc.card_scale.unwrap_or(0.0)
                        // );
                        //
                        // if new_min_surface_size == window.inner_size() {
                        //     // Window is already the correct size.
                        //     log::debug!("on_card_resized(): Window is already the correct size.");
                        //     resize_surface = true;
                        // }
                        // else {
                        //     // Request inner size may not immediately set the new size unless it returns Some.
                        //     // If it returns None then we don't want to resize surfaces now - we'll resize
                        //     // them when we get the window size event. Otherwise we could render a frame at
                        //     // the wrong surface resolution.
                        //     if let Some(resolved_size) = window.request_inner_size(new_min_surface_size) {
                        //         log::debug!("on_card_resized(): Window size resolved immediately.");
                        //         resize_surface = true;
                        //         new_min_surface_size = resolved_size;
                        //     }
                        // }

                        log::debug!("on_card_resized(): resizing viewport currently stubbed.");
                    }

                    if let Some(backend) = &mut dtc.backend {
                        // If software aspect correction is enabled for this renderer, the backend must
                        // be sized for it. Otherwise, the backend should be sized for the native
                        // resolution.
                        if software_aspect {
                            backend
                                .resize_buf(BufferDimensions::from(aspect_dimensions.unwrap()))
                                .expect("FATAL: Failed to resize backend");
                        }
                        else {
                            backend
                                .resize_buf(BufferDimensions::from(buf_dimensions.unwrap()))
                                .expect("FATAL: Failed to resize backend");
                        }

                        // If the window resize resolved immediately, resize the surface and scaler here.
                        // Otherwise, they will resize when we receive the window resize event.
                        if resize_surface {
                            log::debug!(
                                "on_card_resized(): Resizing backend surface to new calculated surface: {}x{}",
                                new_min_surface_size.width,
                                new_min_surface_size.height,
                            );
                            backend
                                .resize_surface(SurfaceDimensions {
                                    w: new_min_surface_size.width,
                                    h: new_min_surface_size.height,
                                })
                                .expect("FATAL: Failed to resize backend surface");

                            let surface_dimensions = backend.surface_dimensions();

                            // Resize the DisplayScaler if present.
                            if let Some(scaler) = &mut dtc.scaler {
                                if resize_dt {
                                    log::debug!(
                                    "on_card_resized(): Resizing scaler to renderer target size: {}x{} surface: {}x{}",
                                    target_dimensions.w,
                                    target_dimensions.h,
                                    surface_dimensions.w,
                                    surface_dimensions.h,
                                );
                                }
                                scaler.resize(
                                    backend.get_backend_raw().unwrap(),
                                    src_dimensions.w,
                                    src_dimensions.h,
                                    target_dimensions.w,
                                    target_dimensions.h,
                                    surface_dimensions.w,
                                    surface_dimensions.h,
                                );
                            }
                        }

                        // Update the scaler's 'Scanlines' ScalerOption.
                        if let Some(scaler) = &mut dtc.scaler {
                            // Update scanline shader param
                            let scanlines = match extents.double_scan {
                                true => src_dimensions.h / 2,
                                false => src_dimensions.h,
                            };

                            scaler.set_option(
                                backend.get_backend_raw().as_mut().unwrap(),
                                ScalerOption::Scanlines {
                                    enabled: None,
                                    lines: Some(scanlines),
                                    intensity: None,
                                },
                                true,
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn on_viewport_resized(&mut self, vid: ViewportId, w: u32, h: u32) -> Result<(), Error> {
        let _idx = match self.viewport_id_map.get(&vid) {
            Some(idx) => *idx,
            None => {
                return Err(anyhow!("No display target for viewport id: {:?}", vid));
            }
        };

        self.viewport_id_resize_requests
            .entry(vid)
            .and_modify(|r| {
                r.w = w;
                r.h = h;
            })
            .or_insert(ResizeTarget { w, h });

        Ok(())
    }

    fn resize_viewports(&mut self) -> Result<(), Error> {
        let wids: Vec<ViewportId> = self.viewport_id_resize_requests.keys().cloned().collect();

        for wid in wids {
            let rt = self.viewport_id_resize_requests.remove(&wid).unwrap();
            use anyhow::Context;
            let idx = self.viewport_id_map.get(&wid).context("Failed to look up window")?;

            let dt = &mut self.targets[*idx];

            if let Some(viewport) = &dt.viewport {
                // TODO: Fix this stuff for eframe viewports

                // let scale_factor = viewport.scale_factor();
                //let resize_string = format!("{}x{} (scale factor: {})", rt.w, rt.h, scale_factor);
                let resize_string = format!("{}x{} (scale factor: {})", rt.w, rt.h, 1.0);
                // if let Some(backend) = &mut dt.backend {
                //     log::debug!(
                //         "resize_windows(): dt{}: resizing backend surface to {}",
                //         *idx,
                //         resize_string
                //     );
                //     backend.resize_surface(SurfaceDimensions { w: rt.w, h: rt.h })?;
                //
                //     // We may receive this event in response to a on_card_resized event that triggered a window size
                //     // change. We should get the current aspect ratio from the renderer.
                //     if let Some(renderer) = &mut dt.renderer {
                //         let buf_dimensions = renderer.get_buf_dimensions();
                //         let aspect_dimensions = renderer.get_display_dimensions();
                //
                //         // Resize the DisplayScaler if present.
                //         if let Some(scaler) = &mut dt.scaler {
                //             log::debug!("resize_windows(): dt{}: resizing scaler to {}", *idx, resize_string);
                //             scaler.resize(
                //                 backend.get_backend_raw().unwrap(),
                //                 buf_dimensions.w,
                //                 buf_dimensions.h,
                //                 aspect_dimensions.w,
                //                 aspect_dimensions.h,
                //                 rt.w,
                //                 rt.h,
                //             );
                //         }
                //     }
                //     else {
                //         // Resize the DisplayScaler if present.
                //         if let Some(scaler) = &mut dt.scaler {
                //             log::debug!("resize_windows(): dt{}: resizing scaler to {}", *idx, resize_string);
                //             scaler.resize_surface(backend.get_backend_raw().unwrap(), rt.w, rt.h)
                //         }
                //     }
                // }

                //eframe doesn't host GUIs

                // if let Some(gui_ctx) = &mut dt.gui_ctx {
                //     log::debug!(
                //         "resize_windows(): dt{}: resizing gui context for window id: {:?} to {}",
                //         *idx,
                //         wid,
                //         resize_string
                //     );
                //     gui_ctx.resize(viewport, rt.w, rt.h);
                // }
            }
        }

        Ok(())
    }

    /// Execute a closure that is passed the VideoCardId for each VideoCard registered in the
    /// DisplayManager.
    fn for_each_card<F>(&mut self, mut f: F)
    where
        F: FnMut(&VideoCardId),
    {
        for vid in &mut self.card_id_map.keys() {
            f(vid)
        }
    }

    fn for_each_renderer<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut VideoRenderer, VideoCardId, &mut [u8]),
    {
        for dtc in &mut self.targets {
            if let Some(renderer) = &mut dtc.renderer {
                f(renderer, dtc.card_id.unwrap(), dtc.backend.as_mut().unwrap().buf_mut())
            }
        }
    }

    fn for_each_backend<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut EFrameBackend, Option<&mut Self::ImplScaler>, Option<&mut GuiRenderContext>),
    {
        for dtc in &mut self.targets {
            //log::debug!("for_each_backend(): dt_type: {:?}", dtc.dt_type);
            match dtc.dt_type {
                DisplayTargetType::WindowBackground { .. } => {
                    // A WindowBackground target will have a Backend and Scaler.
                    if let Some(backend) = &mut dtc.backend {
                        if let Some(scaler) = &mut dtc.scaler {
                            f(backend, Some(&mut *scaler), dtc.gui_ctx.as_mut())
                        }
                    }
                }
                DisplayTargetType::GuiWidget { .. } => {
                    // A GuiWidget target will have a Backend but no Scaler.
                    if let Some(backend) = &mut dtc.backend {
                        f(backend, None, dtc.gui_ctx.as_mut())
                    }
                }
                _ => {}
            }
        }
    }

    fn for_each_target<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut DisplayTargetContext<EFrameBackend>, usize),
    {
        for (i, dtc) in &mut self.targets.iter_mut().enumerate() {
            f(dtc, i)
        }
    }

    fn for_each_gui<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut GuiRenderContext, &ViewportId),
    {
        // Currently, only the main window can have a hosted gui.

        if self.targets.len() > 0 {
            let dtc = &mut self.targets[0];

            if let Some(gui_ctx) = &mut dtc.gui_ctx {
                if let Some(viewport) = &mut dtc.viewport {
                    f(gui_ctx, &viewport)
                }
            }
        }
    }

    fn for_each_viewport<F>(&mut self, mut f: F)
    where
        F: FnMut(&ViewportId, bool) -> Option<bool>,
    {
        for dtc in &mut self.targets {
            if let Some(window) = &mut dtc.viewport {
                let is_on_top = dtc.window_opts.as_ref().map_or(false, |opts| opts.always_on_top);
                dtc.window_opts
                    .as_mut()
                    .map(|opts| opts.is_on_top = f(&window, is_on_top).unwrap_or(opts.is_on_top));
            }
        }
    }

    fn with_renderer<F>(&mut self, dt: DtHandle, mut f: F)
    where
        F: FnMut(&mut VideoRenderer),
    {
        if dt.idx() < self.targets.len() {
            if let Some(renderer) = &mut self.targets[dt.idx()].renderer {
                f(renderer)
            }
        }
    }

    fn with_target_by_vid<F>(&mut self, vid: ViewportId, mut f: F)
    where
        F: FnMut(&mut DisplayTargetContext<EFrameBackend>),
    {
        if let Some(idx) = self.viewport_id_map.get(&vid) {
            f(&mut self.targets[*idx])
        }
    }

    /// Add the specified scaler preset to the Display Manager.
    fn add_scaler_preset(&mut self, preset: ScalerPreset) {
        let hash_key = preset.name.clone();
        if self.scaler_presets.insert(hash_key.clone(), preset).is_some() {
            log::warn!("Scaler preset {} was overwritten", hash_key);
        }
    }

    /// Retrieve the scaler preset by name.
    fn get_scaler_preset(&mut self, name: String) -> Option<&ScalerPreset> {
        self.scaler_presets.get(&name)
    }

    fn apply_scaler_preset(&mut self, dt: DtHandle, name: String) -> Result<(), Error> {
        if is_valid_handle!(dt, self.targets) {
            let preset = self.get_scaler_preset(name).unwrap().clone();
            self.targets[dt.idx()].apply_scaler_preset(&preset);
        }
        else {
            return Err(anyhow!("Display target out of range!"));
        }
        Ok(())
    }

    fn apply_scaler_params(&mut self, dt: DtHandle, params: &ScalerParams) -> Result<(), Error> {
        resolve_handle_mut!(dt, self.targets, |dt: &mut DisplayTargetContext<EFrameBackend>| {
            dt.apply_scaler_params(params);
        });
        Ok(())
    }

    fn get_scaler_params(&self, dt: DtHandle) -> Option<ScalerParams> {
        resolve_handle_opt!(dt, self.targets, |dt: &DisplayTargetContext<EFrameBackend>| {
            dt.scaler_params.clone()
        })
    }

    fn set_display_aperture(
        &mut self,
        dt: DtHandle,
        aperture: DisplayApertureType,
    ) -> Result<Option<VideoCardId>, Error> {
        resolve_handle_mut_result!(dt, self.targets, |dt: &mut DisplayTargetContext<EFrameBackend>| {
            if let Some(renderer) = &mut dt.renderer {
                log::debug!("Setting aperture to: {:?}", &aperture);
                renderer.set_aperture(aperture);
            }
            dt.card_id
        });
    }

    fn set_aspect_correction(&mut self, dt: DtHandle, state: bool) -> Result<(), Error> {
        resolve_handle_mut!(dt, self.targets, |dt: &mut DisplayTargetContext<EFrameBackend>| {
            if let Some(renderer) = &mut dt.renderer {
                let aspect = match state {
                    true => Some(dt.aspect_ratio),
                    false => None,
                };
                log::debug!("Setting aspect ratio to: {:?}", aspect);
                renderer.set_aspect_ratio(aspect, None);
            }
        });
        Ok(())
    }

    fn set_scaler_mode(&mut self, dt: DtHandle, mode: ScalerMode) -> Result<(), Error> {
        if is_bad_handle!(dt, self.targets) {
            return Err(anyhow!("Display target out of range!"));
        }

        let dt = &mut self.targets[dt.idx()];

        if let Some(backend) = &mut dt.backend {
            if let Some(scaler) = &mut dt.scaler {
                scaler.set_mode(&backend.get_backend_raw().unwrap(), mode)
            }
        }
        Ok(())
    }

    fn save_screenshot(&mut self, dt: DtHandle, path: PathBuf) -> Result<(), Error> {
        if is_bad_handle!(dt, self.targets) {
            return Err(anyhow!("Display target out of range!"));
        }

        let filename = file_util::find_unique_filename(&path, "screenshot", "png");

        if let Some(renderer) = &mut self.targets[dt.idx()].renderer {
            renderer.request_screenshot(&filename);
        }
        else {
            return Err(anyhow!("No renderer for display target!"));
        }

        Ok(())
    }
}

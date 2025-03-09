/*
   MartyPC
   https://github.com/dbalsom/martypc

   Copyright 2022-2025 Daniel Balsom

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
*/

//! This module provides the [DisplayManager] trait implementation for the eframe frontend.
//! This required a bit of rework from the previous Pixels-based implementation.
//!
//! A [DisplayManager] handles managing the resources needed to render the output of a core's
//! VideoCard to one or more "Display Targets."
//!
//! A Display Target is an abstraction over some kind of display surface, which could be a native
//! window background or a composited UI element in a GUI.
//!
//! Some of the new design considerations -
//! - An instance of the generic Backend type is no longer created for each display target, but
//!   once for the entire display manager.
//! - A Backend does not hold textures. We can call the backend to create surfaces.
//!   In eframe's case, a surface will never be the final display surface as we are always
//!   rendering to a provided render pass to ultimately be composited by egui.
//! - We do not create windows, we have no control over that. egui 'creates' windows with
//!   immediate-mode drawing calls. This has yet to be implemented.
//!
#[cfg(not(any(feature = "use_wgpu", feature = "use_glow")))]
compile_error!("You must select either the use_wgpu or use_glow features!");

pub mod builder;

use marty_common::*;
use marty_core::{
    device_traits::videocard::{DisplayApertureType, DisplayExtents, VideoCardId},
    file_util,
    machine::Machine,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};

#[cfg(not(feature = "use_wgpu"))]
pub use display_backend_eframe::{
    BufferDimensions,
    DisplayBackend,
    DisplayBackendBuilder,
    DynDisplayTargetSurface,
    EFrameBackend,
    EFrameBackendSurface,
    EFrameScalerType,
    TextureDimensions,
};
#[cfg(feature = "use_wgpu")]
pub use display_backend_eframe_wgpu::{
    BufferDimensions,
    DisplayBackend,
    DisplayBackendBuilder,
    DisplayTargetSurface,
    DynDisplayTargetSurface,
    EFrameBackend,
    EFrameBackendSurface,
    EFrameScalerType,
    TextureDimensions,
};

pub use marty_frontend_common::{
    color::MartyColor,
    display_manager::{
        DisplayManager,
        DisplayTargetDimensions,
        DisplayTargetFlags,
        DisplayTargetType,
        DmGuiOptions,
        DmViewportOptions,
    },
};
use marty_frontend_common::{
    display_manager::{DisplayDimensions, DisplayTargetInfo, DtHandle},
    display_scaler::{PhosphorType, ScalerFilter, ScalerGeometry, ScalerOption, ScalerParams, ScalerPreset},
    types::window::WindowDefinition,
};

// Conditionally use the appropriate scaler per backend
#[cfg(not(feature = "use_wgpu"))]
use marty_scaler_null::{DisplayScaler, MartyScaler, ScalerMode};
#[cfg(feature = "use_wgpu")]
use marty_scaler_wgpu::{MartyScaler, ScalerMode};

use marty_egui_eframe::context::GuiRenderContext;
use marty_videocard_renderer::{AspectCorrectionMode, AspectRatio, VideoRenderer};

use egui::{Context, ViewportId};

#[cfg(feature = "use_wgpu")]
use egui_wgpu::wgpu;

use anyhow::{anyhow, Error};

// use winit::{
//     dpi::{LogicalSize, PhysicalSize},
//     event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
//     window::{Icon, Window, WindowButtons, WindowId, WindowLevel},
// };

// There are a few macros here designed to avoid boilerplate code, with the idea of making
// it easier to copy and paste code from one implementation of DisplayManager to another,
// and in general make the whole thing a bit easier to scan...

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

/// Macro to wrap the type of DisplayTargetContext instance.
macro_rules! dtc {
    () => {
        Arc<RwLock<DisplayTargetContext>>
    };
}

/// Macro to create a new DisplayTargetContext instance.
macro_rules! new_dtc {
    ($expr:expr) => {
        Arc::new(RwLock::new($expr))
    };
}

#[cfg(feature = "use_wgpu")]
macro_rules! resolve_dyn {
    ($expr:expr) => {
        $expr.read().unwrap()
    };
}

#[cfg(not(feature = "use_wgpu"))]
macro_rules! resolve_dyn {
    ($expr:expr) => {
        $expr.read().unwrap()
    };
}

/// Macro to acquire a read lock on a DisplayTargetContext instance.
/// Eventually we can add error handling here.
macro_rules! resolve_dtc {
    ($expr:expr) => {
        $expr.read().unwrap()
    };
}

/// Macro to acquire a write lock on a DisplayTargetContext instance.
/// Eventually we can add error handling here.
macro_rules! resolve_dtc_mut {
    ($expr:expr) => {
        $expr.write().unwrap()
    };
}

#[allow(unused_macros)]
macro_rules! resolve_dtc_ref_mut {
    ($expr:expr) => {
        $expr.write().as_mut().unwrap()
    };
}

// macro_rules! resolve_handle_opt {
//     ($handle:expr, $other:expr, $closure:expr) => {
//         if $handle.idx() < $other.len() {
//             Some($closure(&resolve_dtc!($other.get($handle.idx()).unwrap())))
//         }
//         else {
//             None
//         }
//     };
// }

macro_rules! resolve_handle_mut {
    ($handle:expr, $other:expr, $closure:expr) => {
        if $handle.idx() < $other.len() {
            $closure(&mut resolve_dtc_mut!($other.get_mut($handle.idx()).unwrap()))
        }
        else {
            return Err(anyhow::anyhow!("Handle out of range!"));
        }
    };
}

#[allow(unused_macros)]
macro_rules! resolve_handle_result {
    ($handle:expr, $other:expr, $closure:expr) => {
        if $handle.idx() < $other.len() {
            return Ok($closure(&resolve_dtc!($other.get($handle.idx()).unwrap())));
        }
        else {
            return Err(anyhow::anyhow!("Handle out of range!"));
        }
    };
}

macro_rules! resolve_handle_mut_result {
    ($handle:expr, $other:expr, $closure:expr) => {
        if $handle.idx() < $other.len() {
            return Ok($closure(&mut resolve_dtc_mut!($other.get_mut($handle.idx()).unwrap())));
        }
        else {
            return Err(anyhow::anyhow!("Handle out of range!"));
        }
    };
}

macro_rules! resolve_handle_opt {
    ($handle:expr, $other:expr, $closure:expr) => {
        if $handle.idx() < $other.len() {
            $closure(&resolve_dtc!($other.get($handle.idx()).unwrap()))
        }
        else {
            return None;
        }
    };
}

pub const DEFAULT_RESOLUTION_W: u32 = 640;
pub const DEFAULT_RESOLUTION_H: u32 = 480;

// Unnecessary for the eframe Display Manager as our "screen" is always rendered beneath the
// menu bar with the appropriate dimensions.
//const EGUI_MENU_BAR: u32 = 24;

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

/// Tracks state for a viewport, allowing us to query the viewport size and fullscreen status
/// without a direct viewport reference.
#[derive(Default)]
pub struct ViewportState {
    pub w: u32,
    pub h: u32,
    pub fullscreen: bool,
}

#[derive(Default)]
pub struct DisplayTargetContext {
    //pub(crate) event_loop: EventLoop<()>,
    pub name: String,
    pub dt_type: DisplayTargetType, // The type of display we are targeting
    pub dt_flags: DisplayTargetFlags,
    pub initialized: bool,
    pub resolved_params: DisplayTargetParams,
    pub requested_params: Option<DisplayTargetParams>,
    pub viewport: Option<ViewportId>, // The EGUI ViewportId
    pub viewport_opts: Option<DmViewportOptions>,
    pub viewport_state: ViewportState,
    pub(crate) fill_color: Option<u32>,
    pub(crate) gui_ctx: Option<GuiRenderContext>, // The egui render context, if any
    pub(crate) card_id: Option<VideoCardId>,      // The video card device id, if any
    pub(crate) renderer: Option<VideoRenderer>,   // The renderer
    pub(crate) aspect_ratio: AspectRatio,         // Aspect ratio configured for this display
    pub(crate) surface: Option<DynDisplayTargetSurface>, // The display target surface created by the backend
    prev_scaler_mode: Option<ScalerMode>,         // The previous scaler mode
    pub(crate) scaler: Option<EFrameScalerType>,  // The scaler pipeline
    pub(crate) scaler_params: Option<ScalerParams>,
    pub(crate) card_scale: Option<f32>, // If Some, the card resolution is scaled by this factor
    mouse_grabbed: bool,                // Is the mouse grabbed by this display target?
}

pub struct DisplayTargetCallback {
    pub lock: Arc<RwLock<DisplayTargetContext>>,
}

#[cfg(feature = "use_wgpu")]
impl egui_wgpu::CallbackTrait for DisplayTargetCallback {
    // Required method
    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        _callback_resources: &egui_wgpu::CallbackResources,
    ) {
        // pub struct PaintCallbackInfo {
        //     pub viewport: Rect,
        //     pub clip_rect: Rect,
        //     pub pixels_per_point: f32,
        //     pub screen_size_px: [u32; 2],
        // }

        let dtc = self.lock.write().unwrap();

        //log::debug!("DisplayTargetCallback::paint(): Entered...");
        if dtc.surface.is_none() {
            log::debug!("DisplayTargetCallback::paint(): No surface!.");
            return;
        }
        if dtc.scaler.is_none() {
            log::debug!("DisplayTargetCallback::paint(): No scaler!.");
            return;
        }

        if let (Some(_surface), Some(scaler)) = (&dtc.surface, &dtc.scaler) {
            // log::debug!(
            //     "DisplayTargetCallback::paint(): Rendering with scaler! viewport rect: {:?} clip rect: {:?}",
            //     info.viewport,
            //     info.clip_rect
            // );
            scaler.render_with_renderpass(render_pass);
        }
    }
}

pub struct EFrameDisplayManager {
    // All windows share a common event loop.
    //event_loop: Option<EventLoop<()>>,
    backend: Option<EFrameBackend>,
    // There can be multiple display windows. One for the main egui window, which may or may not
    // be attached to a videocard.
    // Optionally, one for each potential graphics adapter. For the moment I only plan to support
    // two adapters - a primary and secondary adapter. This implies a limit of 3 windows.
    // The window containing egui will always be at index 0.
    targets: Vec<dtc!()>,
    viewport_id_map: MartyHashMap<ViewportId, usize>,
    viewport_id_resize_requests: MartyHashMap<ViewportId, ResizeTarget>,
    card_id_map: MartyHashMap<VideoCardId, Vec<usize>>, // Card id maps to a Vec<usize> as a single card can have multiple targets.
    primary_idx: Option<usize>,
    scaler_presets: MartyHashMap<String, ScalerPreset>,
}

impl Default for EFrameDisplayManager {
    fn default() -> Self {
        Self {
            backend: None,
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

impl DisplayTargetContext {
    pub fn surface(&self) -> Option<&DynDisplayTargetSurface> {
        self.surface.as_ref()
    }

    pub fn destructure_surface<F>(&mut self, f: F)
    where
        F: FnOnce(&mut DynDisplayTargetSurface, &mut Option<EFrameScalerType>, &mut Option<GuiRenderContext>),
    {
        if let Some(surface) = &mut self.surface {
            f(surface, &mut self.scaler, &mut self.gui_ctx);
        }
    }

    pub fn destructure_gui<F>(&mut self, f: F)
    where
        F: FnOnce(&mut GuiRenderContext),
    {
        if let Some(gui_ctx) = &mut self.gui_ctx {
            f(gui_ctx);
        }
    }

    pub fn scaler_geometry(&self) -> Option<ScalerGeometry> {
        if let Some(scaler) = &self.scaler {
            Some(scaler.geometry())
        }
        else {
            None
        }
    }

    /// Set the aspect mode of the target. If the aspect mode is changed, we may need to resize
    /// the backend and scaler.
    pub fn set_aspect_mode(&mut self, _mode: AspectCorrectionMode) {}

    pub fn get_card_id(&mut self) -> Option<VideoCardId> {
        self.card_id
    }

    pub fn set_scale_factor(&mut self, _factor: f64) {
        // if let Some(gui_ctx) = &mut self.gui_ctx {
        //     gui_ctx.scale_factor(factor);
        // }
    }

    pub fn grabbed(&self) -> bool {
        self.mouse_grabbed
    }

    pub fn set_grabbed(&mut self, grabbed: bool) {
        self.mouse_grabbed = grabbed;
    }

    pub fn set_on_top(&mut self, on_top: bool) {
        if let Some(wopts) = &mut self.viewport_opts {
            wopts.is_on_top = on_top;
        }
    }

    pub fn is_on_top(&self) -> bool {
        if let Some(wopts) = &self.viewport_opts {
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

    pub fn apply_scaler_preset(&mut self, backend: &EFrameBackend, preset: &ScalerPreset) {
        // We must have a scaler to continue...
        if !self.scaler.is_some() {
            return;
        }
        log::debug!("Applying scaler preset: {}", &preset.name);

        let bilinear = match preset.filter {
            ScalerFilter::Linear => true,
            ScalerFilter::Nearest => false,
        };
        let scaler = self.scaler.as_mut().unwrap();

        let mut mode = preset.mode.unwrap_or(scaler.mode());

        if self.dt_type == DisplayTargetType::GuiWidget {
            self.prev_scaler_mode = Some(mode);
            mode = ScalerMode::Windowed;
        }

        scaler.set_mode(&*backend.device(), &*backend.queue(), mode);
        scaler.set_bilinear(bilinear);
        scaler.set_fill_color(MartyColor::from_u24(preset.border_color.unwrap_or(0)));

        self.apply_scaler_params(backend, &ScalerParams::from(preset.clone()));

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

    pub fn apply_scaler_params(&mut self, backend: &EFrameBackend, params: &ScalerParams) {
        // We must have a backend and scaler to continue...
        if !self.scaler.is_some() {
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
            .set_options(&*backend.device(), &*backend.queue(), scaler_update);
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

impl EFrameDisplayManager {
    pub fn main_display_target(&self) -> dtc!() {
        self.targets[0].clone()
    }

    pub fn main_display_callback(&self) -> DisplayTargetCallback {
        DisplayTargetCallback {
            lock: self.targets[0].clone(),
        }
    }
}

impl<'p> DisplayManager<EFrameBackend, GuiRenderContext, ViewportId, ViewportId, Context> for EFrameDisplayManager {
    #[cfg(feature = "use_wgpu")]
    type NativeTexture = wgpu::Texture;
    #[cfg(not(feature = "use_wgpu"))]
    type NativeTexture = egui::TextureHandle;

    //#[cfg(feature = "use_wgpu")]
    //type NativeTextureView = wgpu::TextureView;
    //#[cfg(not(feature = "use_wgpu"))]
    //type NativeTextureView = ();

    #[cfg(feature = "use_wgpu")]
    type NativeEncoder = wgpu::CommandEncoder;
    #[cfg(not(feature = "use_wgpu"))]
    type NativeEncoder = ();

    type NativeEventLoop = ();
    type ImplSurface = DynDisplayTargetSurface;
    type ImplScaler = EFrameScalerType;
    type ImplDisplayTarget = DisplayTargetContext;

    fn create_target(
        &mut self,
        name: String,
        dt_type: DisplayTargetType,
        dt_flags: DisplayTargetFlags,
        _native_context: Option<&Context>,
        viewport: Option<ViewportId>,
        viewport_opts: Option<DmViewportOptions>,
        card_id: Option<VideoCardId>,
        scaler_preset: String,
        _gui_options: &DmGuiOptions,
    ) -> Result<DtHandle, Error> {
        // For now, we only support creating new WindowBackground targets.
        #[allow(unreachable_patterns)]
        match dt_type {
            DisplayTargetType::GuiWidget | DisplayTargetType::WindowBackground => {
                // Create a display target for the main viewport.
                // In this case, since we are using eframe, the main (root) viewport is already open.

                // Attempt to resolve the specified scaler preset
                let scaler_preset = match self.scaler_preset(scaler_preset) {
                    Some(preset) => preset.clone(),
                    None => {
                        return Err(anyhow!("Couldn't load scaler preset!"));
                    }
                };

                // Use the dimensions specified in window options, if supplied, otherwise fall back to default
                let ((tw, th), _resizable) = if let Some(ref window_opts) = viewport_opts {
                    (window_opts.size.into(), window_opts.resizable)
                }
                else {
                    ((DEFAULT_RESOLUTION_W, DEFAULT_RESOLUTION_H), true)
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
                    dt_flags.main_window,
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

                // let menubar_h = if dt_flags.has_menu {
                //     //(EGUI_MENU_BAR as f64 * scale_factor) as u32
                //     EGUI_MENU_BAR
                // }
                // else {
                //     0
                // };

                // Create the backend.
                // let mut pb = EFrameBackend::new(
                //     EFrameBackendType::EguiWindow,
                //     native_context.unwrap().clone(),
                //     BufferDimensions {
                //         w: tw,
                //         h: th,
                //         pitch: tw,
                //     },
                //     TextureDimensions { w: sw, h: sh },
                //     None,
                // )?;

                if self.backend.is_none() {
                    return Err(anyhow!("create_target(): No backend!"));
                }

                // Create a new surface for the display target.
                let surface = self.backend.as_mut().unwrap().create_surface(
                    BufferDimensions {
                        w: tw,
                        h: th,
                        pitch: tw,
                    },
                    TextureDimensions { w: sw, h: sh },
                )?;

                // Create the scaler.
                let _scale_mode = match dt_flags.main_window {
                    true => ScalerMode::Integer,
                    false => ScalerMode::Fixed,
                };

                // The texture sizes specified initially aren't important. Since DisplayManager can't
                // query video cards directly, the caller must resize all video cards after calling
                // the Builder.
                #[cfg(feature = "use_wgpu")]
                let scaler = MartyScaler::new(
                    scaler_preset.mode.unwrap_or(ScalerMode::Integer),
                    &*self.backend.as_ref().unwrap().device(),
                    &resolve_dyn!(surface).backing_texture(),
                    resolve_dyn!(surface).backing_texture_format(),
                    DEFAULT_RESOLUTION_W,
                    DEFAULT_RESOLUTION_H,
                    DEFAULT_RESOLUTION_W,
                    DEFAULT_RESOLUTION_W,
                    sw,
                    sh,
                    0, // In the eframe backend, our surface is a panel drawn below the menu
                    true,
                    MartyColor::from_u24(scaler_preset.border_color.unwrap_or_default()),
                );
                #[cfg(not(feature = "use_wgpu"))]
                let scaler = MartyScaler::new();

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

                let card_scale = viewport_opts.as_ref().and_then(|wo| wo.card_scale);

                let viewport_state = ViewportState {
                    w: tw,
                    h: th,
                    fullscreen: false,
                };

                let viewport = viewport.unwrap_or(ViewportId::ROOT);

                let mut dtc = DisplayTargetContext {
                    name,
                    dt_type,
                    dt_flags,
                    initialized: false,
                    resolved_params: DisplayTargetParams {
                        buf_dim: DisplayTargetDimensions::new(tw, th),
                        render_dim: DisplayTargetDimensions::new(tw, th),
                        surface_dim: DisplayTargetDimensions::new(tw, th),
                        window_dim: DisplayTargetDimensions::new(tw, th),
                    },
                    requested_params: None,
                    viewport: Some(viewport),
                    viewport_opts,
                    viewport_state,
                    fill_color: None,
                    gui_ctx: None,
                    card_id,
                    renderer,
                    aspect_ratio: scaler_preset.renderer.aspect_ratio.unwrap_or_default(),
                    //backend: Some(pb), // The graphics backend instance
                    surface: Some(surface),
                    prev_scaler_mode: None,
                    scaler: Some(Box::new(scaler)),
                    scaler_params: Some(ScalerParams::from(scaler_preset.clone())),
                    card_scale,
                    mouse_grabbed: false,
                };

                dtc.apply_scaler_preset(&self.backend.as_ref().unwrap(), &scaler_preset);

                self.targets.push(new_dtc!(dtc));

                self.viewport_id_map.insert(viewport, dt_idx);

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

        for (i, vtc) in self.targets.iter().enumerate() {
            let vtc = resolve_dtc_mut!(vtc);
            let mut vtype = None;
            if let Some(vid) = vtc.card_id {
                vtype = machine.bus().video(&vid).and_then(|card| Some(card.get_video_type()));
            }

            let mut render_time = Duration::from_secs(0);
            let renderer_params = if let Some(renderer) = &vtc.renderer {
                render_time = renderer.get_last_render_time();
                Some(renderer.get_config_params().clone())
            }
            else {
                None
            };

            let mut scaler_mode = None;
            let mut scaler_geometry = None;
            if let Some(scaler) = &vtc.scaler {
                scaler_mode = Some(scaler.mode());
                scaler_geometry = Some(scaler.geometry());
            }

            let has_gui = false;
            let gui_render_time = Duration::ZERO;
            // if let Some(gui_ctx) = &vt.gui_ctx {
            //     has_gui = true;
            //     gui_render_time = gui_ctx.get_render_time();
            // }

            let backend_name = String::new();

            // TODO: A display target doesn't have a backend anymore,
            //       so if we want the adapter name we'll have to either set it,
            //       or get it from the main DisplayManager.

            // #[cfg(feature = "use_wgpu")]
            // if let Some(backend) = &vt.backend {
            //     backend_name = backend
            //         .get_adapter_info()
            //         .map(|info| format!("{:?} ({})", info.backend, info.name))
            //         .unwrap_or_default();
            // }

            info_vec.push(DisplayTargetInfo {
                handle: DtHandle(i),
                backend_name,
                dtype: vtc.dt_type,
                flags: vtc.dt_flags,
                vtype,
                vid: vtc.card_id,
                name: vtc.name.clone(),
                renderer: renderer_params,
                render_time,
                contains_gui: has_gui,
                fill_color: vtc.fill_color,
                gui_render_time,
                scaler_mode,
                scaler_params: vtc.scaler_params,
                scaler_geometry,
            })
        }

        info_vec
    }

    fn display_type(&self, dt: DtHandle) -> Option<DisplayTargetType> {
        resolve_handle_opt!(dt, self.targets, |vtc: &DisplayTargetContext| { Some(vtc.dt_type) })
    }

    fn set_display_type(&mut self, dt: DtHandle, dtype: DisplayTargetType) -> Result<(), Error> {
        resolve_handle_mut!(dt, self.targets, |vtc: &mut DisplayTargetContext| {
            match dtype {
                DisplayTargetType::GuiWidget => {
                    log::debug!("set_display_type(): Setting display target {} to GuiWidget.", dt.idx());

                    vtc.dt_type = DisplayTargetType::GuiWidget;

                    if let Some(scaler) = &mut vtc.scaler {
                        vtc.prev_scaler_mode = Some(scaler.mode());
                        scaler.set_mode(
                            &*self.backend.as_ref().unwrap().device(),
                            &*self.backend.as_ref().unwrap().queue(),
                            ScalerMode::Stretch,
                        );
                    }
                }
                DisplayTargetType::WindowBackground => {
                    log::debug!(
                        "set_display_type(): Setting display target {} to WindowBackground.",
                        dt.idx()
                    );

                    vtc.dt_type = DisplayTargetType::WindowBackground;

                    if let Some(scaler) = &mut vtc.scaler {
                        if let Some(prev_mode) = vtc.prev_scaler_mode {
                            scaler.set_mode(
                                &*self.backend.as_ref().unwrap().device(),
                                &*self.backend.as_ref().unwrap().queue(),
                                prev_mode,
                            );
                        }
                    }
                }
            }
            Ok(())
        })
    }

    fn viewport_by_id(&self, _vid: ViewportId) -> Option<ViewportId> {
        None
        // self.viewport_id_map.get(&wid).and_then(|idx| {
        //     //log::warn!("got id, running map():");
        //     self.targets[*idx].window.as_ref()
        // })
    }

    fn viewport(&self, _dt: DtHandle) -> Option<ViewportId> {
        //self.targets.get(dt.idx()).and_then(|dt| dt.window.as_ref())
        None
    }

    fn main_viewport(&self) -> Option<ViewportId> {
        // Main display should always be index 0.
        resolve_dtc!(self.targets[0]).viewport.clone()
    }

    fn backend(&mut self) -> Option<&EFrameBackend> {
        // Main display should always be index 0.
        self.backend.as_ref()
    }
    fn backend_mut(&mut self) -> Option<&mut EFrameBackend> {
        // Main display should always be index 0.
        self.backend.as_mut()
    }

    fn with_main_gui_mut<F>(&mut self, f: F)
    where
        F: FnOnce(&mut GuiRenderContext),
    {
        resolve_dtc_mut!(self.targets[0]).gui_ctx.as_mut().map(f);
    }

    fn with_gui_by_viewport_id_mut<F>(&mut self, vid: ViewportId, f: F)
    where
        F: FnOnce(&mut GuiRenderContext),
    {
        if let Some(&idx) = self.viewport_id_map.get(&vid) {
            if let Some(dtc) = self.targets.get(idx) {
                if let Some(gui_ctx) = resolve_dtc_mut!(dtc).gui_ctx.as_mut() {
                    f(gui_ctx);
                }
            }
        }
    }

    fn with_renderer_mut<F>(&mut self, dt: DtHandle, f: F)
    where
        F: FnOnce(&mut VideoRenderer),
    {
        self.targets
            .get(dt.idx())
            .and_then(|dtc| resolve_dtc_mut!(dtc).renderer.as_mut().map(f));
    }

    fn with_renderer_by_card_id_mut<F>(&mut self, _id: VideoCardId, _f: F)
    where
        F: FnOnce(&mut VideoRenderer),
    {
        // TODO: Rethink this function. A card can have multiple renderers. Which one would we return?

        // self.card_id_map
        //     .get(&id)
        //     .and_then(|idx| self.targets.get(*idx).and_then(|dtc| dtc.renderer.as_mut().map(f)));
    }

    fn with_primary_renderer_mut<F>(&mut self, f: F)
    where
        F: FnOnce(&mut VideoRenderer),
    {
        self.primary_idx.and_then(|idx| {
            self.targets
                .get_mut(idx)
                .and_then(|dtc| resolve_dtc_mut!(dtc).renderer.as_mut().map(f))
        });
    }

    /// Reflect a potential update to a videocard's output resolution. This can be called once
    /// per frame regardless of whether we anticipate the card resolution actually changed.
    /// This method needs to resize the resolution of the surface, renderer and scaler associated
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
                let dtc = &mut resolve_dtc_mut!(self.targets[*idx]);

                let mut aspect_dimensions: Option<BufferDimensions> = None;
                let mut buf_dimensions: Option<BufferDimensions> = None;

                let mut resize_dt = false;
                let mut software_aspect = false;

                let mut dtc_initialized = dtc.initialized;

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

                    resize_dt = renderer.would_resize((w, h).into()) || !dtc_initialized;

                    if resize_dt {
                        log::debug!(
                            "on_card_resized(): Card {vid:?} init:{} new aperture: {w}x{h} [Doublescan: {}, Aperture: {aperture:?}] Resizing renderer for dt {idx}...",
                            dtc_initialized,
                            extents.double_scan,
                        );
                        renderer.resize((w, h).into());
                        dtc_initialized = true;
                    }

                    buf_dimensions = Some(DisplayTargetDimensions::from(renderer.get_buf_dimensions()).into());
                    aspect_dimensions = Some(DisplayTargetDimensions::from(renderer.get_display_dimensions()).into());
                }

                dtc.initialized = dtc_initialized;

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

                    let top_margin = dtc.viewport_opts.as_ref().map_or(0, |opts| opts.margins.t);

                    // Calculate the minimum client area we need (including top margin for gui menu)
                    let mut new_min_surface_size = match dtc.card_scale {
                        Some(card_scale) => {
                            // Card scaling is enabled. Scale the window to the specified factor, even
                            // if that would shrink the window.
                            DisplayDimensions::new(
                                (target_dimensions.w as f32 * card_scale) as u32,
                                (target_dimensions.h as f32 * card_scale) as u32 + top_margin,
                            )
                        }
                        _ => DisplayDimensions::new(target_dimensions.w, target_dimensions.h + top_margin),
                    };

                    // First we need to see if the viewport needs resizing. If the renderer increased
                    // resolution, we may need to make the viewport bigger to fit. We don't support
                    // scaling downwards.
                    if let Some(_viewport) = &mut dtc.viewport {
                        log::debug!("on_card_resized(): handling viewport");
                        // TODO: fix all this for eframe viewports

                        // First, get the inner size of the window. We may not need to resize it if
                        // its already big enough, and we don't have card scaling on.

                        // let win_dim = window.inner_size();
                        let win_dim = DisplayDimensions::new(dtc.viewport_state.w, dtc.viewport_state.h);

                        if dtc.card_scale.is_some() {
                            // window.set_max_inner_size(Some(new_min_surface_size));
                            // window.set_min_inner_size(Some(new_min_surface_size));
                        }
                        else {
                            if win_dim.w < new_min_surface_size.w || win_dim.h < new_min_surface_size.h {
                                // Window is too small in at least one dimension.
                                new_min_surface_size = DisplayDimensions::new(
                                    std::cmp::max(win_dim.w, new_min_surface_size.w),
                                    std::cmp::max(win_dim.h, new_min_surface_size.h),
                                );
                            }
                            else {
                                // Window is big enough, retain size
                                new_min_surface_size = DisplayDimensions::new(win_dim.w, win_dim.h);
                            }
                        }
                        //
                        log::debug!(
                            "on_card_resized(): Resizing window to fit new calculated surface. {}x{} => {}x{} card_scale: {}",
                            win_dim.w,
                            win_dim.h,
                            new_min_surface_size.w,
                            new_min_surface_size.h,
                            dtc.card_scale.unwrap_or(0.0)
                        );

                        if new_min_surface_size == win_dim {
                            // Window is already the correct size.
                            log::debug!("on_card_resized(): Window is already the correct size.");
                            resize_surface = true;
                        }
                        else {
                            // Request inner size may not immediately set the new size unless it returns Some.
                            // If it returns None then we don't want to resize surfaces now - we'll resize
                            // them when we get the window size event. Otherwise, we could render a frame at
                            // the wrong surface resolution.

                            // if let Some(resolved_size) = window.request_inner_size(new_min_surface_size) {
                            //     log::debug!("on_card_resized(): Window size resolved immediately.");
                            //     resize_surface = true;
                            //     new_min_surface_size = resolved_size;
                            // }
                            resize_surface = true;
                            //new_min_surface_size = resolved_size;
                        }

                        log::debug!("on_card_resized(): resizing viewport currently stubbed.");
                        //resize_surface = true;
                    }

                    // TODO: Fix this stuff for eframe viewports
                    //resize_surface = true;

                    if let (Some(backend), Some(surface)) = (&mut self.backend, &mut dtc.surface) {
                        // If software aspect correction is enabled for this renderer, the backend must
                        // be sized for it. Otherwise, the backend should be sized for the native
                        // resolution.
                        let dims = match software_aspect {
                            true => BufferDimensions::from(aspect_dimensions.unwrap()),
                            false => BufferDimensions::from(buf_dimensions.unwrap()),
                        };
                        backend
                            .resize_backing_texture(surface, dims)
                            .expect("FATAL: Failed to resize backend");

                        // If the window resize resolved immediately, resize the surface and scaler here.
                        // Otherwise, they will resize when we receive the window resize event.
                        if resize_surface {
                            log::debug!(
                                "on_card_resized(): Resizing backend surface to new calculated surface: {}x{}",
                                new_min_surface_size.w,
                                new_min_surface_size.h,
                            );
                            backend
                                .resize_surface_texture(
                                    surface,
                                    TextureDimensions {
                                        w: new_min_surface_size.w,
                                        h: new_min_surface_size.h,
                                    },
                                )
                                .expect("FATAL: Failed to resize backend surface");

                            //let surface_dimensions = surface.read().unwrap().surface_dimensions();

                            dtc.destructure_surface(|surface, scaler, _gui| {
                                let surface = resolve_dyn!(surface);
                                let surface_dimensions = surface.surface_dimensions();

                                // Resize the DisplayScaler if present. This closure is only called if we have a surface, so no need to check.
                                if let Some(scaler) = scaler {
                                    if resize_dt {
                                        log::debug!(
                                            "on_card_resized(): Resizing scaler to renderer target size: {}x{} surface: {}x{}",
                                            target_dimensions.w,
                                            target_dimensions.h,
                                            surface_dimensions.w,
                                            surface_dimensions.h,
                                        );

                                        scaler.resize(
                                            &*backend.device(),
                                            &*backend.queue(),
                                            &surface.backing_texture(),
                                            src_dimensions.w,
                                            src_dimensions.h,
                                            target_dimensions.w,
                                            target_dimensions.h,
                                            surface_dimensions.w,
                                            surface_dimensions.h,
                                        );
                                    }
                                }
                            });
                        }

                        // Update the scaler's 'Scanlines' ScalerOption.
                        if let Some(scaler) = &mut dtc.scaler {
                            // Update scanline shader param
                            let scanlines = match extents.double_scan {
                                true => src_dimensions.h / 2,
                                false => src_dimensions.h,
                            };

                            scaler.set_option(
                                &*backend.device(),
                                &*backend.queue(),
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
        log::debug!(
            "on_viewport_resized(): go resize event for viewport id: {:?} to {}x{}",
            vid,
            w,
            h
        );
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
        let vids: Vec<ViewportId> = self.viewport_id_resize_requests.keys().cloned().collect();

        //log::debug!("resize_viewports(): processing {} resize requests", wids.len());
        for vid in vids {
            let rt = self.viewport_id_resize_requests.remove(&vid).unwrap();
            use anyhow::Context;
            let idx = self.viewport_id_map.get(&vid).context("Failed to look up viewport")?;

            let dtc = &mut resolve_dtc_mut!(self.targets[*idx]);

            log::debug!(
                "resize_viewports(): resizing viewport id: {:?} to {}x{}",
                vid,
                rt.w,
                rt.h
            );
            if let Some(backend) = &mut self.backend {
                if let Some(_viewport) = &dtc.viewport {
                    // TODO: Fix this stuff for eframe viewports

                    // let scale_factor = viewport.scale_factor();
                    //let resize_string = format!("{}x{} (scale factor: {})", rt.w, rt.h, scale_factor);
                    let resize_string = format!("{}x{} (scale factor: {})", rt.w, rt.h, 1.0);

                    log::debug!(
                        "resize_viewports(): dt{}: resizing backend surface to {}",
                        *idx,
                        resize_string
                    );
                    backend.resize_surface_texture(
                        &mut dtc.surface.as_mut().unwrap(),
                        TextureDimensions { w: rt.w, h: rt.h },
                    )?;

                    // We may receive this event in response to an on_card_resized event that triggered a window size
                    // change. We should get the current aspect ratio from the renderer.
                    if let Some(renderer) = &mut dtc.renderer {
                        let buf_dimensions = renderer.get_buf_dimensions();
                        let aspect_dimensions = renderer.get_display_dimensions();

                        // Resize the DisplayScaler if present.
                        dtc.destructure_surface(|surface, scaler, _gui| {
                            if let Some(scaler) = scaler {
                                log::debug!("resize_viewports(): dt{}: resizing scaler to {}", *idx, resize_string);

                                scaler.resize(
                                    &*backend.device(),
                                    &*backend.queue(),
                                    &surface.read().unwrap().backing_texture(),
                                    buf_dimensions.w,
                                    buf_dimensions.h,
                                    aspect_dimensions.w,
                                    aspect_dimensions.h,
                                    rt.w,
                                    rt.h,
                                );
                            }
                        });
                    }
                    else {
                        // Resize the DisplayScaler if present.
                        dtc.destructure_surface(|surface, scaler, _gui| {
                            if let Some(scaler) = scaler {
                                log::debug!("resize_windows(): dt{}: resizing scaler to {}", *idx, resize_string);
                                scaler.resize_surface(
                                    &*backend.device(),
                                    &*backend.queue(),
                                    &surface.read().unwrap().backing_texture(),
                                    rt.w,
                                    rt.h,
                                )
                            }
                        });
                    }

                    // Update the viewport state.
                    dtc.viewport_state.w = rt.w;
                    dtc.viewport_state.h = rt.h;
                }
                else {
                    log::warn!("resize_viewports(): dt{}: no viewport id: {:?}", *idx, vid);
                }

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
            let dtc = &mut resolve_dtc_mut!(dtc);

            let card_id = dtc.card_id.unwrap();
            let surface = dtc.surface.as_ref().unwrap().clone();

            if let Some(renderer) = &mut dtc.renderer {
                let mut surface_lock = surface.write().unwrap();
                let buf_mut = surface_lock.buf_mut();
                f(renderer, card_id, buf_mut)
            }
        }
    }

    fn with_surface_mut<F>(&mut self, dt: DtHandle, f: F) -> Result<(), Error>
    where
        F: FnOnce(&mut EFrameBackend, &mut Self::ImplSurface),
    {
        if let Some(backend) = &mut self.backend {
            resolve_handle_mut!(dt, self.targets, |dtc: &mut DisplayTargetContext| {
                dtc.destructure_surface(|surface, _, _| {
                    f(backend, &mut *surface);
                });
            });
        }
        Ok(())
    }

    #[rustfmt::skip]
    fn for_each_surface<F>(&mut self, dt_type_filter: Option<DisplayTargetType>, mut f: F)
    where
        F: FnMut(
            &mut EFrameBackend,
            &mut Self::ImplSurface,
            Option<&mut Self::ImplScaler>,
            Option<&mut GuiRenderContext>,
        ),
    {
        if let Some(backend) = &mut self.backend {
            for dtc in &mut self.targets {
                let dtc = &mut resolve_dtc_mut!(dtc);

                let dt_type = dtc.dt_type;
                let dt_type_match = dt_type_filter.is_none() || dt_type == dt_type_filter.unwrap();

                if dt_type_match {
                    //log::debug!("for_each_backend(): dt_type: {:?}", dtc.dt_type);
                    match dt_type {
                        DisplayTargetType::WindowBackground { .. } => {
                            // A WindowBackground target will have a Surface and Scaler
                            dtc.destructure_surface(|surface, scaler, gui| {
                                f(backend, surface, scaler.as_mut(), gui.as_mut())
                            });
                        }
                        DisplayTargetType::GuiWidget { .. } => {
                            // TODO: I think we can actually have scalers for GuiWidget targets...
                            // A GuiWidget target will have a Surface but no Scaler.
                            dtc.destructure_surface(|surface, scaler, gui| {
                                f(backend, surface, scaler.as_mut(), gui.as_mut())
                            });
                        }
                    }
                }
            }
        }
    }

    fn for_each_target<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut DisplayTargetContext, usize),
    {
        for (i, dtc) in &mut self.targets.iter_mut().enumerate() {
            f(&mut resolve_dtc_mut!(dtc), i)
        }
    }

    fn for_each_gui<F>(&mut self, _f: F)
    where
        F: FnMut(&mut GuiRenderContext, &ViewportId),
    {
        // Currently, only the main window can have a hosted gui.

        // if self.targets.len() > 0 {
        //     let dtc = &mut resolve_dtc_mut!(self.targets[0]);
        //
        //     if let Some(gui_ctx) = &mut dtc.gui_ctx {
        //         if let Some(viewport) = &mut dtc.viewport {
        //             f(gui_ctx, &viewport)
        //         }
        //     }
        // }
    }

    fn for_each_viewport<F>(&mut self, _f: F)
    where
        F: FnMut(&ViewportId, bool) -> Option<bool>,
    {
        // for dtc in &mut self.targets {
        //     let dtc = &mut resolve_dtc_mut!(dtc);
        //
        //     let (viewport, window_opts) = (dtc.viewport.as_mut(), dtc.window_opts.as_mut());
        //
        //     if let Some(window) = &mut dtc.viewport {
        //         let is_on_top = dtc.window_opts.as_ref().map_or(false, |opts| opts.always_on_top);
        //         dtc.window_opts
        //             .as_mut()
        //             .map(|opts| opts.is_on_top = f(&window, is_on_top).unwrap_or(opts.is_on_top));
        //     }
        // }
    }

    fn with_renderer<F>(&mut self, dt: DtHandle, mut f: F)
    where
        F: FnMut(&mut VideoRenderer),
    {
        if dt.idx() < self.targets.len() {
            if let Some(renderer) = &mut resolve_dtc_mut!(self.targets[dt.idx()]).renderer {
                f(renderer)
            }
        }
    }

    fn with_target_by_vid<F>(&mut self, vid: ViewportId, mut f: F)
    where
        F: FnMut(&mut DisplayTargetContext),
    {
        if let Some(idx) = self.viewport_id_map.get(&vid) {
            f(&mut resolve_dtc_mut!(self.targets[*idx]))
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
    fn scaler_preset(&mut self, name: String) -> Option<&ScalerPreset> {
        self.scaler_presets.get(&name)
    }

    fn apply_scaler_preset(&mut self, dt: DtHandle, name: String) -> Result<(), Error> {
        if is_valid_handle!(dt, self.targets) {
            let preset = self.scaler_preset(name).unwrap().clone();
            resolve_dtc_mut!(self.targets[dt.idx()]).apply_scaler_preset(self.backend.as_ref().unwrap(), &preset);
        }
        else {
            return Err(anyhow!("Display target out of range!"));
        }
        Ok(())
    }

    fn apply_scaler_params(&mut self, dt: DtHandle, params: &ScalerParams) -> Result<(), Error> {
        resolve_handle_mut!(dt, self.targets, |dt: &mut DisplayTargetContext| {
            dt.apply_scaler_params(self.backend.as_ref().unwrap(), params);
        });
        Ok(())
    }

    fn scaler_params(&self, dt: DtHandle) -> Option<ScalerParams> {
        resolve_handle_opt!(dt, self.targets, |dt: &DisplayTargetContext| {
            dt.scaler_params.clone()
        })
    }

    fn set_display_aperture(
        &mut self,
        dt: DtHandle,
        aperture: DisplayApertureType,
    ) -> Result<Option<VideoCardId>, Error> {
        resolve_handle_mut_result!(dt, self.targets, |dt: &mut DisplayTargetContext| {
            if let Some(renderer) = &mut dt.renderer {
                log::debug!("Setting aperture to: {:?}", &aperture);
                renderer.set_aperture(aperture);
            }
            dt.card_id
        })
    }

    fn set_aspect_correction(&mut self, dt: DtHandle, state: bool) -> Result<(), Error> {
        resolve_handle_mut!(dt, self.targets, |dt: &mut DisplayTargetContext| {
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

        let dtc = &mut resolve_dtc_mut!(self.targets[dt.idx()]);

        let mut mode = mode;
        if dtc.dt_type == DisplayTargetType::GuiWidget {
            dtc.prev_scaler_mode = Some(mode);
            mode = ScalerMode::Stretch;
        }
        if let Some(backend) = self.backend.as_mut() {
            if let Some(scaler) = &mut dtc.scaler {
                log::debug!("Setting scaler mode to: {:?}", mode);
                scaler.set_mode(&*backend.device(), &*backend.queue(), mode)
            }
        }
        Ok(())
    }

    fn save_screenshot(&mut self, dt: DtHandle, path: impl AsRef<Path>) -> Result<PathBuf, Error> {
        if is_bad_handle!(dt, self.targets) {
            return Err(anyhow!("Display target out of range!"));
        }

        let filename = file_util::find_unique_filename(path.as_ref(), "screenshot", "png");

        if let Some(renderer) = &mut resolve_dtc_mut!(self.targets[dt.idx()]).renderer {
            renderer.request_screenshot(&filename);
        }
        else {
            return Err(anyhow!("No renderer for display target!"));
        }

        Ok(filename)
    }
}

/*
   MartyPC
   https://github.com/dbalsom/martypc

   Copyright 2022-2026 Daniel Balsom

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
    machine::Machine,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};

#[cfg(not(any(feature = "use_wgpu", feature = "use_glow")))]
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
#[cfg(feature = "use_glow")]
pub use display_backend_eframe_glow::{
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

pub use marty_frontend_common::color::MartyColor;

pub use marty_display_common::display_manager::{
    DisplayManager,
    DisplayTargetDimensions,
    DisplayTargetFlags,
    DisplayTargetType,
    DmGuiOptions,
    DmViewportOptions,
};

use marty_frontend_common::types::window::{BackgroundOrganization, WindowDefinition};

use marty_display_common::{
    display_manager::{DisplayDimensions, DisplayTargetInfo, DtHandle, ViewportInfo, VpHandle},
    display_scaler::{
        PhosphorType,
        ScalerFilter,
        ScalerGeometry,
        ScalerMode,
        ScalerOption,
        ScalerParams,
        ScalerPreset,
    },
};

// Conditionally use the appropriate scaler per backend
#[cfg(feature = "use_glow")]
use marty_scaler_glow::MartyScaler;
#[cfg(not(any(feature = "use_wgpu", feature = "use_glow")))]
use marty_scaler_null::MartyScaler;
#[cfg(feature = "use_wgpu")]
use marty_scaler_wgpu::MartyScaler;

use marty_egui_eframe::context::GuiRenderContext;
use marty_videocard_renderer::{AspectCorrectionMode, AspectRatio, VideoRenderer};

use egui::{Context, ViewportId};

#[cfg(feature = "use_wgpu")]
use egui_wgpu::wgpu;

use anyhow::{anyhow, Error};
use marty_common::types::ui::MouseCaptureMode;
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
#[derive(Clone, Default)]
pub struct ViewportState {
    pub w: u32,
    pub h: u32,
    pub fullscreen: bool,
    pub open: bool,
    pub resize_pending: bool,
}

/// State owned by a configured egui viewport rather than by any display rendered into it.
pub struct EFrameViewportContext {
    pub id: ViewportId,
    pub options: DmViewportOptions,
    pub state: ViewportState,
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

/// Divide a viewport's available background into one rectangle per display target.
pub fn background_target_rects(
    available_rect: egui::Rect,
    target_count: usize,
    organization: BackgroundOrganization,
) -> Vec<egui::Rect> {
    let (columns, rows) = organization.grid_dimensions(target_count);
    if columns == 0 || rows == 0 {
        return Vec::new();
    }

    let cell_width = available_rect.width() / columns as f32;
    let cell_height = available_rect.height() / rows as f32;
    (0..target_count)
        .map(|target_idx| {
            let column = target_idx % columns;
            let row = target_idx / columns;
            let min = egui::pos2(
                available_rect.min.x + column as f32 * cell_width,
                available_rect.min.y + row as f32 * cell_height,
            );
            let max = egui::pos2(min.x + cell_width, min.y + cell_height);
            egui::Rect::from_min_max(min, max)
        })
        .collect()
}

#[cfg(all(target_arch = "wasm32", feature = "use_wgpu"))]
unsafe impl Send for DisplayTargetCallback {}

#[cfg(all(target_arch = "wasm32", feature = "use_wgpu"))]
unsafe impl Sync for DisplayTargetCallback {}

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

#[cfg(feature = "use_glow")]
struct GlowDisplayTargetCallback {
    lock: Arc<RwLock<DisplayTargetContext>>,
}

// SAFETY: On WASM, egui's glow paint callbacks are created and invoked on the browser's main
// thread. The Send + Sync bounds allow the callback to be stored by egui; the WebGL resources
// inside DisplayTargetContext are never transferred to or accessed from a worker.
#[cfg(all(target_arch = "wasm32", feature = "use_glow"))]
unsafe impl Send for GlowDisplayTargetCallback {}

// SAFETY: See the Send implementation above.
#[cfg(all(target_arch = "wasm32", feature = "use_glow"))]
unsafe impl Sync for GlowDisplayTargetCallback {}

#[cfg(feature = "use_glow")]
impl GlowDisplayTargetCallback {
    fn paint(&self, painter: &egui_glow::Painter) {
        if let Ok(mut target) = self.lock.try_write() {
            let surface = target.surface().unwrap();
            let texture = surface.read().unwrap().backing_texture().clone();

            if let Some(scaler) = &mut target.scaler {
                scaler.render_with_context(painter.gl(), texture);
            }
        }
        else {
            log::warn!("Failed to acquire write lock on display target!");
        }
    }
}

pub struct EFrameDisplayManager {
    // All windows share a common event loop.
    //event_loop: Option<EventLoop<()>>,
    backend: Option<EFrameBackend>,
    // Display targets and egui viewports are deliberately independent: a viewport may contain
    // multiple targets, and moving all targets away from a viewport leaves an empty window.
    targets: Vec<dtc!()>,
    viewports: Vec<Arc<RwLock<EFrameViewportContext>>>,
    viewport_id_map: MartyHashMap<ViewportId, usize>,
    viewport_id_resize_requests: MartyHashMap<ViewportId, ResizeTarget>,
    card_id_map: MartyHashMap<VideoCardId, Vec<usize>>, // Card id maps to a Vec<usize> as a single card can have multiple targets.
    primary_idx: Option<usize>,
    scaler_presets: MartyHashMap<String, ScalerPreset>,
    last_screenshot: Option<PathBuf>,
}

impl Default for EFrameDisplayManager {
    fn default() -> Self {
        Self {
            backend: None,
            targets: Vec::new(),
            viewports: Vec::new(),
            viewport_id_map: MartyHashMap::default(),
            viewport_id_resize_requests: MartyHashMap::default(),
            card_id_map: MartyHashMap::default(),
            primary_idx: None,
            scaler_presets: MartyHashMap::default(),
            last_screenshot: None,
        }
    }
}

impl EFrameDisplayManager {
    pub fn new() -> Self {
        Default::default()
    }

    /// Register an egui viewport independently from any display targets assigned to it.
    pub fn create_viewport(&mut self, id: ViewportId, options: DmViewportOptions) -> Result<VpHandle, Error> {
        if self.viewport_id_map.contains_key(&id) {
            return Err(anyhow!("Duplicate viewport ID: {:?}", id));
        }

        let (w, h): (u32, u32) = options.size.into();
        let viewport_idx = self.viewports.len();
        self.viewports.push(Arc::new(RwLock::new(EFrameViewportContext {
            id,
            state: ViewportState {
                w,
                h,
                fullscreen: options.fullscreen,
                open: true,
                resize_pending: false,
            },
            options,
        })));
        self.viewport_id_map.insert(id, viewport_idx);

        Ok(VpHandle(viewport_idx))
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

    pub fn set_grabbed(&mut self, grabbed: bool, capture_mode: MouseCaptureMode) {
        self.mouse_grabbed = grabbed;

        if let MouseCaptureMode::LightPen = capture_mode {
            if let Some(renderer) = &mut self.renderer {
                renderer.set_cursor_state(grabbed)
            }
        }
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

        // Scaler mode belongs to the display target and is changed independently. Applying a
        // visual preset must not reset a mode selected in the window configuration or GUI.
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
        scaler_update.push(ScalerOption::InterlaceSupport(params.interlace_support));

        if let Some(renderer) = &self.renderer {
            let rparams = renderer.params();

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
    pub fn set_scaler_crtc_frame_parity(&mut self, vid: VideoCardId, parity: Option<u8>) {
        let Some(idx_vec) = self.card_id_map.get(&vid).cloned()
        else {
            return;
        };

        let Some(backend) = &self.backend
        else {
            return;
        };

        let enabled = parity.is_some();
        let parity = parity.unwrap_or(0) as u32;

        for idx in idx_vec {
            if let Some(dtc) = self.targets.get_mut(idx) {
                let dtc = &mut resolve_dtc_mut!(dtc);

                if let Some(scaler) = &mut dtc.scaler {
                    scaler.set_option(
                        &*backend.device(),
                        &*backend.queue(),
                        ScalerOption::CrtcFrameParity { enabled, parity },
                        true,
                    );
                }
            }
        }
    }

    pub fn main_display_target(&self) -> dtc!() {
        self.targets[0].clone()
    }

    pub fn display_target(&self, display: DtHandle) -> Option<dtc!()> {
        self.targets.get(display.idx()).cloned()
    }

    /// Return displays assigned to an egui viewport, optionally filtered by display type. Targets
    /// without a renderer are empty configuration slots and are intentionally not returned as
    /// displays.
    pub fn displays_for_viewport(
        &self,
        viewport: ViewportId,
        display_type: Option<DisplayTargetType>,
    ) -> Vec<DtHandle> {
        self.targets
            .iter()
            .enumerate()
            .filter_map(|(idx, target)| {
                target.read().ok().and_then(|target| {
                    (target.renderer.is_some()
                        && target.viewport == Some(viewport)
                        && display_type.is_none_or(|display_type| target.dt_type == display_type))
                    .then_some(DtHandle(idx))
                })
            })
            .collect()
    }

    /// Return the first display assigned to a viewport for call sites that operate on one focused
    /// display, such as mouse capture.
    pub fn display_for_viewport(&self, viewport: ViewportId) -> Option<DtHandle> {
        self.displays_for_viewport(viewport, None).into_iter().next()
    }

    pub fn grabbed_display_for_viewport(&self, viewport: ViewportId) -> Option<DtHandle> {
        self.displays_for_viewport(viewport, None).into_iter().find(|display| {
            self.display_target(*display)
                .and_then(|target| target.read().ok().map(|target| target.grabbed()))
                .unwrap_or(false)
        })
    }

    pub fn grabbed_display(&self) -> Option<(ViewportId, DtHandle)> {
        self.targets.iter().enumerate().find_map(|(idx, target)| {
            target.read().ok().and_then(|target| {
                if target.grabbed() {
                    target.viewport.map(|viewport| (viewport, DtHandle(idx)))
                }
                else {
                    None
                }
            })
        })
    }

    pub fn viewport_fill_color(&self, viewport: ViewportId) -> Option<u32> {
        let viewport_idx = *self.viewport_id_map.get(&viewport)?;
        self.viewports
            .get(viewport_idx)?
            .read()
            .ok()
            .and_then(|viewport| viewport.options.fill_color)
    }

    pub fn viewport_background_organization(&self, viewport: ViewportId) -> BackgroundOrganization {
        self.viewport_id_map
            .get(&viewport)
            .and_then(|viewport_idx| self.viewports.get(*viewport_idx))
            .and_then(|viewport| viewport.read().ok())
            .map(|viewport| viewport.options.background_organization)
            .unwrap_or_default()
    }

    pub fn viewport_can_grab(&self, viewport: ViewportId) -> bool {
        self.viewport_id_map
            .get(&viewport)
            .and_then(|viewport_idx| self.viewports.get(*viewport_idx))
            .and_then(|viewport| viewport.read().ok())
            .is_some_and(|viewport| viewport.options.can_grab)
    }

    fn viewport_size(&self, viewport: ViewportId) -> Option<(u32, u32)> {
        let viewport_idx = *self.viewport_id_map.get(&viewport)?;
        self.viewports.get(viewport_idx)?.read().ok().map(|viewport| {
            let state = &viewport.state;
            (state.w, state.h)
        })
    }

    fn request_viewport_resize(&mut self, viewport: ViewportId) {
        if let Some((w, h)) = self.viewport_size(viewport) {
            self.viewport_id_resize_requests.insert(viewport, ResizeTarget { w, h });
        }
    }

    /// Present every non-root display target as an egui viewport.
    ///
    /// Egui viewports are immediate-mode: this method must be called on every root viewport pass
    /// for as long as the secondary windows should remain open.
    pub fn show_secondary_viewports<F>(&self, ctx: &Context, mut target_ui: F)
    where
        F: FnMut(ViewportId, bool, DtHandle, &mut egui::Ui, &egui::Response, &mut DisplayTargetContext),
    {
        for viewport in &self.viewports {
            let viewport = viewport.clone();

            let Some((viewport_id, viewport_builder, fill_color, background_organization, can_grab)) = (|| {
                let viewport_ref = viewport.read().ok()?;
                if !viewport_ref.state.open {
                    return None;
                }

                let viewport_id = viewport_ref.id;
                if viewport_id == ViewportId::ROOT {
                    return None;
                }
                let viewport_opts = &viewport_ref.options;
                let (width, height): (u32, u32) = viewport_opts.size.into();

                let mut builder = egui::ViewportBuilder::default()
                    .with_title(viewport_opts.title.clone())
                    .with_inner_size([width as f32, height as f32])
                    .with_resizable(viewport_opts.resizable)
                    .with_fullscreen(viewport_opts.fullscreen)
                    .with_close_button(false);

                if let Some(min_size) = viewport_opts.min_size {
                    let (width, height): (u32, u32) = min_size.into();
                    builder = builder.with_min_inner_size([width as f32, height as f32]);
                }
                if let Some(max_size) = viewport_opts.max_size {
                    let (width, height): (u32, u32) = max_size.into();
                    builder = builder.with_max_inner_size([width as f32, height as f32]);
                }
                if viewport_opts.always_on_top {
                    builder = builder.with_always_on_top();
                }

                let fill_color = viewport_opts
                    .fill_color
                    .map_or(egui::Color32::BLACK, |color| MartyColor::from_u24(color).to_color32());

                Some((
                    viewport_id,
                    builder,
                    fill_color,
                    viewport_opts.background_organization,
                    viewport_opts.can_grab,
                ))
            })()
            else {
                continue;
            };

            let viewport_targets: Vec<_> = self
                .displays_for_viewport(viewport_id, Some(DisplayTargetType::WindowBackground))
                .into_iter()
                .filter_map(|display| self.display_target(display).map(|target| (display, target)))
                .collect();
            let viewport_state = viewport.clone();
            let viewport_target_ui = &mut target_ui;
            let viewport_ui = move |ui: &mut egui::Ui, _class: egui::ViewportClass| {
                if ui.input(|input| input.viewport().close_requested()) {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::CancelClose);
                }

                if let Some(size) = ui.input(|input| input.viewport().inner_rect.map(|rect| rect.size())) {
                    let (w, h) = (size.x.max(1.0) as u32, size.y.max(1.0) as u32);
                    if let Ok(mut viewport_ref) = viewport_state.write() {
                        if viewport_ref.state.w != w || viewport_ref.state.h != h {
                            viewport_ref.state.w = w;
                            viewport_ref.state.h = h;
                            viewport_ref.state.resize_pending = true;
                        }
                    }
                }

                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(fill_color))
                    .show_inside(ui, |ui| {
                        if viewport_targets.is_empty() {
                            return;
                        }

                        let target_rects = background_target_rects(
                            ui.available_rect_before_wrap(),
                            viewport_targets.len(),
                            background_organization,
                        );
                        for (rect, (display, target)) in target_rects.into_iter().zip(&viewport_targets) {
                            let grabbed = target.read().ok().is_some_and(|target| target.grabbed());
                            let sense = if can_grab || grabbed {
                                egui::Sense::click()
                            }
                            else {
                                egui::Sense::hover()
                            };
                            let response = ui.allocate_rect(rect, sense);

                            #[cfg(feature = "use_wgpu")]
                            {
                                let callback = DisplayTargetCallback { lock: target.clone() };
                                let paint_callback = egui_wgpu::Callback::new_paint_callback(rect, callback);
                                ui.painter().add(paint_callback);
                            }

                            #[cfg(feature = "use_glow")]
                            {
                                let callback = GlowDisplayTargetCallback { lock: target.clone() };
                                let paint_callback = egui::PaintCallback {
                                    rect,
                                    callback: Arc::new(egui_glow::CallbackFn::new(move |_info, painter| {
                                        callback.paint(painter);
                                    })),
                                };
                                ui.painter().add(paint_callback);
                            }

                            if let Ok(mut target) = target.write() {
                                viewport_target_ui(viewport_id, can_grab, *display, ui, &response, &mut target);
                            }
                        }
                    });
            };

            // Display textures are updated by the root app every frame. Immediate viewports keep
            // native child windows in that same render pass, so changing a target's assignment is
            // visible without waiting for a focus or resize event to wake a deferred viewport.
            //
            // Web backends also require the immediate mode because their Glow resources are bound
            // to the browser's main thread.
            ctx.show_viewport_immediate(viewport_id, viewport_builder, viewport_ui);
        }
    }

    #[cfg(feature = "use_glow")]
    pub fn display_callback(
        &self,
        display: DtHandle,
        _ui: &mut egui::Ui,
        rect: egui::Rect,
    ) -> Option<egui::PaintCallback> {
        let callback = GlowDisplayTargetCallback {
            lock: self.display_target(display)?,
        };

        Some(egui::PaintCallback {
            rect,
            callback: Arc::new(egui_glow::CallbackFn::new(move |_info, painter| {
                callback.paint(painter);
            })),
        })
    }

    #[cfg(feature = "use_glow")]
    pub fn main_display_callback(&self, _ui: &mut egui::Ui, rect: egui::Rect) -> egui::PaintCallback {
        self.display_callback(DtHandle::MAIN, _ui, rect).unwrap()
    }

    #[cfg(feature = "use_wgpu")]
    pub fn display_callback(&self, display: DtHandle) -> Option<DisplayTargetCallback> {
        Some(DisplayTargetCallback {
            lock: self.display_target(display)?,
        })
    }

    #[cfg(feature = "use_wgpu")]
    pub fn main_display_callback(&self) -> DisplayTargetCallback {
        self.display_callback(DtHandle::MAIN).unwrap()
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
        scaler_mode: ScalerMode,
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

                let viewport = viewport.ok_or_else(|| anyhow!("create_target(): No viewport ID specified"))?;
                let viewport_idx = *self
                    .viewport_id_map
                    .get(&viewport)
                    .ok_or_else(|| anyhow!("create_target(): Viewport {:?} has not been created", viewport))?;
                let configured_viewport_opts = self
                    .viewports
                    .get(viewport_idx)
                    .and_then(|viewport| viewport.read().ok())
                    .map(|viewport| viewport.options.clone())
                    .ok_or_else(|| anyhow!("create_target(): Failed to read viewport {:?}", viewport))?;
                let (tw, th): (u32, u32) = configured_viewport_opts.size.into();
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

                // GUI-widget targets temporarily override their configured mode while embedded.
                let active_scaler_mode = if dt_type == DisplayTargetType::GuiWidget {
                    ScalerMode::Windowed
                }
                else {
                    scaler_mode
                };

                // The texture sizes specified initially aren't important. Since DisplayManager can't
                // query video cards directly, the caller must resize all video cards after calling
                // the Builder.
                #[cfg(feature = "use_wgpu")]
                let scaler = MartyScaler::new(
                    active_scaler_mode,
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
                #[cfg(feature = "use_glow")]
                let scaler = MartyScaler::new(
                    &*self.backend.as_ref().unwrap().device(),
                    (DEFAULT_RESOLUTION_W, DEFAULT_RESOLUTION_H),
                    (DEFAULT_RESOLUTION_W, DEFAULT_RESOLUTION_H),
                    (sw, sh),
                    0,
                    active_scaler_mode,
                );
                #[cfg(not(any(feature = "use_wgpu", feature = "use_glow")))]
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
                    fill_color: None,
                    gui_ctx: None,
                    card_id,
                    renderer,
                    aspect_ratio: scaler_preset.renderer.aspect_ratio.unwrap_or_default(),
                    //backend: Some(pb), // The graphics backend instance
                    surface: Some(surface),
                    prev_scaler_mode: if dt_type == DisplayTargetType::GuiWidget {
                        Some(scaler_mode)
                    }
                    else {
                        None
                    },
                    scaler: Some(Box::new(scaler)),
                    scaler_params: Some(ScalerParams::from(scaler_preset.clone())),
                    card_scale,
                    mouse_grabbed: false,
                };

                dtc.apply_scaler_preset(&self.backend.as_ref().unwrap(), &scaler_preset);

                self.targets.push(new_dtc!(dtc));

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
            if vtc.renderer.is_none() {
                continue;
            }

            let mut vtype = None;
            if let Some(vid) = vtc.card_id {
                vtype = machine.bus().video(&vid).and_then(|card| Some(card.video_type()));
            }

            let mut render_time = Duration::from_secs(0);
            let renderer_params = if let Some(renderer) = &vtc.renderer {
                render_time = renderer.get_last_render_time();
                Some(renderer.config_params().clone())
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

            let backend_name = self
                .backend
                .as_ref()
                .map(|backend| backend.backend_name().to_string())
                .unwrap_or_default();
            let adapter_name = self
                .backend
                .as_ref()
                .map(|backend| backend.adapter_name().to_string())
                .unwrap_or_default();

            info_vec.push(DisplayTargetInfo {
                handle: DtHandle(i),
                viewport: vtc
                    .viewport
                    .and_then(|viewport| self.viewport_id_map.get(&viewport).copied())
                    .map(VpHandle),
                backend_name,
                adapter_name,
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

    fn viewport_info(&self) -> Vec<ViewportInfo> {
        self.viewports
            .iter()
            .enumerate()
            .map(|(idx, viewport)| {
                let name = viewport
                    .read()
                    .ok()
                    .map(|viewport| viewport.options.title.clone())
                    .unwrap_or_else(|| format!("Viewport {}", idx));

                ViewportInfo {
                    handle: VpHandle(idx),
                    name,
                }
            })
            .collect()
    }

    fn set_display_viewport(&mut self, dt: DtHandle, viewport: VpHandle) -> Result<(), Error> {
        let destination = self
            .viewports
            .get(viewport.idx())
            .cloned()
            .ok_or_else(|| anyhow!("No viewport for handle: {:?}", viewport))?;
        let target = self
            .targets
            .get(dt.idx())
            .cloned()
            .ok_or_else(|| anyhow!("No display target for handle: {:?}", dt))?;

        let destination_id = destination
            .read()
            .map_err(|_| anyhow!("Viewport lock was poisoned"))?
            .id;

        let source_id = {
            let mut target = target
                .write()
                .map_err(|_| anyhow!("Display target lock was poisoned"))?;
            if target.renderer.is_none() {
                return Err(anyhow!("Display target {:?} has no renderer", dt));
            }

            let source_id = target
                .viewport
                .ok_or_else(|| anyhow!("Display target {:?} has no viewport", dt))?;
            target.viewport = Some(destination_id);
            source_id
        };

        if let Ok(mut destination) = destination.write() {
            destination.state.open = true;
        }

        self.request_viewport_resize(source_id);
        if destination_id != source_id {
            self.request_viewport_resize(destination_id);
        }

        Ok(())
    }

    fn main_viewport(&self) -> Option<ViewportId> {
        self.viewport_id_map
            .contains_key(&ViewportId::ROOT)
            .then_some(ViewportId::ROOT)
    }

    fn viewport_by_id(&self, vid: ViewportId) -> Option<ViewportId> {
        self.viewport_id_map.contains_key(&vid).then_some(vid)
    }

    fn viewport(&self, dt: DtHandle) -> Option<ViewportId> {
        self.targets
            .get(dt.idx())
            .and_then(|target| resolve_dtc!(target).viewport)
    }

    fn display_type(&self, dt: DtHandle) -> Option<DisplayTargetType> {
        resolve_handle_opt!(dt, self.targets, |vtc: &DisplayTargetContext| { Some(vtc.dt_type) })
    }

    fn set_display_type(&mut self, dt: DtHandle, dtype: DisplayTargetType) -> Result<(), Error> {
        let viewport = self.viewport(dt);
        let result = resolve_handle_mut!(dt, self.targets, |vtc: &mut DisplayTargetContext| {
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
        });

        // Send a resize request right away so that scaler doesn't show stale dimensions
        if result.is_ok() {
            if let Some(viewport) = viewport {
                self.request_viewport_resize(viewport);
            }
        }

        result
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
        if let Some(display) = self.display_for_viewport(vid) {
            if let Some(dtc) = self.targets.get(display.idx()) {
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
                // Extract viewport info
                let (viewport_id, is_background) = self.targets[*idx]
                    .read()
                    .ok()
                    .map(|target| (target.viewport, target.dt_type == DisplayTargetType::WindowBackground))
                    .unwrap_or((None, false));
                let background_count = viewport_id
                    .filter(|_| is_background)
                    .map(|viewport| {
                        self.displays_for_viewport(viewport, Some(DisplayTargetType::WindowBackground))
                            .len()
                            .max(1)
                    })
                    .unwrap_or(1);
                let (top_margin, viewport_w, viewport_h) = viewport_id
                    .and_then(|viewport| self.viewport_id_map.get(&viewport).copied())
                    .and_then(|viewport_idx| self.viewports.get(viewport_idx))
                    .and_then(|viewport| viewport.read().ok())
                    .map(|viewport| {
                        let (columns, rows) = viewport
                            .options
                            .background_organization
                            .grid_dimensions(background_count);
                        (
                            viewport.options.margins.t,
                            (viewport.state.w / columns.max(1) as u32).max(1),
                            (viewport.state.h / rows.max(1) as u32).max(1),
                        )
                    })
                    .unwrap_or((0, DEFAULT_RESOLUTION_W, DEFAULT_RESOLUTION_H));

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

                    software_aspect = matches!(renderer.params().aspect_correction, AspectCorrectionMode::Software);

                    let aperture = renderer.params().aperture;
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
                        let win_dim = DisplayDimensions::new(viewport_w, viewport_h);

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
        if !self.viewport_id_map.contains_key(&vid) {
            return Err(anyhow!("No viewport for id: {:?}", vid));
        }

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
        let deferred_resizes: Vec<_> = self
            .viewports
            .iter()
            .filter_map(|viewport| {
                viewport.write().ok().and_then(|mut viewport| {
                    if viewport.state.resize_pending {
                        viewport.state.resize_pending = false;
                        Some((viewport.id, viewport.state.w, viewport.state.h))
                    }
                    else {
                        None
                    }
                })
            })
            .collect();
        for (viewport, w, h) in deferred_resizes {
            self.viewport_id_resize_requests.insert(viewport, ResizeTarget { w, h });
        }

        let vids: Vec<ViewportId> = self.viewport_id_resize_requests.keys().cloned().collect();

        for vid in vids {
            let rt = self.viewport_id_resize_requests.remove(&vid).unwrap();
            use anyhow::Context;
            let viewport_idx = *self.viewport_id_map.get(&vid).context("Failed to look up viewport")?;

            let background_organization = self
                .viewports
                .get(viewport_idx)
                .and_then(|viewport| viewport.write().ok())
                .map(|mut viewport| {
                    viewport.state.w = rt.w;
                    viewport.state.h = rt.h;
                    viewport.options.background_organization
                })
                .unwrap_or_default();

            let target_indices: Vec<usize> = self
                .targets
                .iter()
                .enumerate()
                .filter_map(|(idx, target)| {
                    target.read().ok().and_then(|target| {
                        (target.renderer.is_some()
                            && target.viewport == Some(vid)
                            && target.dt_type == DisplayTargetType::WindowBackground)
                            .then_some(idx)
                    })
                })
                .collect();

            if target_indices.is_empty() {
                continue;
            }

            let (columns, rows) = background_organization.grid_dimensions(target_indices.len());
            let panel_w = (rt.w / columns.max(1) as u32).max(1);
            let panel_h = (rt.h / rows.max(1) as u32).max(1);

            log::debug!(
                "resize_viewports(): resizing viewport id: {:?} to {}x{} across {} display panels in a {}x{} grid",
                vid,
                rt.w,
                rt.h,
                target_indices.len(),
                columns,
                rows
            );
            let Some(backend) = &mut self.backend
            else {
                continue;
            };

            for idx in target_indices {
                let dtc = &mut resolve_dtc_mut!(self.targets[idx]);
                let resize_string = format!("{}x{} (scale factor: {})", panel_w, panel_h, 1.0);

                log::debug!(
                    "resize_viewports(): dt{}: resizing backend surface to {}",
                    idx,
                    resize_string
                );
                backend.resize_surface_texture(
                    dtc.surface.as_mut().context("Display target has no surface")?,
                    TextureDimensions { w: panel_w, h: panel_h },
                )?;

                let dimensions = dtc
                    .renderer
                    .as_mut()
                    .map(|renderer| (renderer.get_buf_dimensions(), renderer.get_display_dimensions()));

                dtc.destructure_surface(|surface, scaler, _gui| {
                    if let (Some(scaler), Some((buf_dimensions, aspect_dimensions))) = (scaler, dimensions) {
                        log::debug!("resize_viewports(): dt{}: resizing scaler to {}", idx, resize_string);
                        scaler.resize(
                            &*backend.device(),
                            &*backend.queue(),
                            &surface.read().unwrap().backing_texture(),
                            buf_dimensions.w,
                            buf_dimensions.h,
                            aspect_dimensions.w,
                            aspect_dimensions.h,
                            panel_w,
                            panel_h,
                        );
                    }
                });
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

            if let (Some(card_id), Some(surface), Some(renderer)) =
                (dtc.card_id, dtc.surface.as_ref().cloned(), dtc.renderer.as_mut())
            {
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
        for display in self.displays_for_viewport(vid, None) {
            f(&mut resolve_dtc_mut!(self.targets[display.idx()]))
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
            let viewport = {
                let mut target = resolve_dtc_mut!(self.targets[dt.idx()]);
                target.apply_scaler_preset(self.backend.as_ref().unwrap(), &preset);
                target.viewport
            };
            if let Some(viewport) = viewport {
                // Presets can change renderer aperture and aspect parameters. Re-run the same
                // viewport/grid layout pass used by a real resize so the scaler receives the
                // current panel dimensions immediately on the next update.
                self.request_viewport_resize(viewport);
            }
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

        let filename = find_unique_filename(path.as_ref(), "screenshot", "png", self.last_screenshot.as_ref());

        if let Some(renderer) = &mut resolve_dtc_mut!(self.targets[dt.idx()]).renderer {
            renderer.request_screenshot(&filename);
        }
        else {
            return Err(anyhow!("No renderer for display target!"));
        }

        Ok(filename)
    }
}

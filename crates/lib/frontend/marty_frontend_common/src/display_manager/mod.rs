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

   frontend_common::display_manager::mod.rs

   Define the DisplayManager trait.

   This trait defines an interface for managing display targets for a given
   graphics backend and windowing system combination.
*/

use crate::display_scaler::{ScalerGeometry, ScalerMode, ScalerParams, ScalerPreset};
use std::{
    fmt::{Display, Formatter},
    path::{Path, PathBuf},
};

pub use crate::{
    types::{display_target_dimensions::DisplayTargetDimensions, display_target_margins::DisplayTargetMargins},
    MartyGuiTheme,
};
use marty_core::{
    device_traits::videocard::{DisplayApertureType, DisplayExtents, VideoCardId, VideoType},
    machine::Machine,
};
use marty_videocard_renderer::{RendererConfigParams, VideoRenderer};

use anyhow::Error;
use web_time::Duration;

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct DtHandle(pub usize);

impl DtHandle {
    pub const MAIN: DtHandle = DtHandle(0);
}

impl Default for DtHandle {
    fn default() -> Self {
        DtHandle(0)
    }
}
impl DtHandle {
    pub fn idx(&self) -> usize {
        self.0
    }
}

impl From<usize> for DtHandle {
    fn from(idx: usize) -> Self {
        DtHandle(idx)
    }
}

impl From<DtHandle> for usize {
    fn from(handle: DtHandle) -> usize {
        handle.0
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DisplayDimensions {
    pub w: u32,
    pub h: u32,
}

impl DisplayDimensions {
    pub fn new(w: u32, h: u32) -> Self {
        DisplayDimensions { w, h }
    }
}

impl From<DisplayDimensions> for (u32, u32) {
    fn from(dim: DisplayDimensions) -> Self {
        (dim.w, dim.h)
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, strum::EnumIter)]
pub enum DisplayTargetType {
    #[default]
    WindowBackground,
    GuiWidget,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct DisplayTargetFlags {
    pub main_window: bool,
    pub has_gui: bool,
    pub has_menu: bool,
}

impl Display for DisplayTargetType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DisplayTargetType::WindowBackground { .. } => write!(f, "Window Background"),
            DisplayTargetType::GuiWidget { .. } => write!(f, "GUI Window"),
        }
    }
}

/// Information about a display target.
/// This can be retrieved from the Display Manager via display_info().
#[derive(Clone)]
pub struct DisplayTargetInfo {
    pub handle: DtHandle,
    pub backend_name: String,
    pub dtype: DisplayTargetType,
    pub flags: DisplayTargetFlags,
    pub vtype: Option<VideoType>,
    pub vid: Option<VideoCardId>,
    pub name: String,
    pub renderer: Option<RendererConfigParams>,
    pub render_time: Duration,
    pub contains_gui: bool,
    pub fill_color: Option<u32>,
    pub gui_render_time: Duration,
    pub scaler_mode: Option<ScalerMode>,
    pub scaler_params: Option<ScalerParams>,
    pub scaler_geometry: Option<ScalerGeometry>,
}

pub struct DmGuiOptions {
    pub enabled: bool,
    pub theme: Option<MartyGuiTheme>,
    pub menu_theme: Option<MartyGuiTheme>,
    pub menubar_h: u32,
    pub zoom: f32,
    pub debug_drawing: bool,
}

/// Options for viewport-based display targets.
/// All dimensions are specified as inner size (sometimes referred to as the client area, for
/// window-based viewports).
pub struct DmViewportOptions {
    pub size: DisplayTargetDimensions,
    pub min_size: Option<DisplayTargetDimensions>,
    pub max_size: Option<DisplayTargetDimensions>,
    pub margins: DisplayTargetMargins,
    pub title: String,
    pub resizable: bool,
    pub always_on_top: bool,
    pub is_on_top: bool,
    pub card_scale: Option<f32>,
    pub fill_color: Option<u32>,
}

impl Default for DmViewportOptions {
    fn default() -> Self {
        Self {
            size: Default::default(),
            min_size: Default::default(),
            max_size: Default::default(),
            margins: Default::default(),
            title: "New Window".to_string(),
            resizable: false,
            always_on_top: false,
            is_on_top: false,
            card_scale: None,
            fill_color: None,
        }
    }
}

/// The [DisplayManager] trait is implemented by a display manager that combines the facilities of
/// a windowing system (Such as winit/eframe), graphics backend (such as Pixels/wgpu), and a GUI
/// (such as egui). It is a difficult task to create an interface trait that can handle the
/// different requirements of each of these systems, so the trait is designed to be as flexible as
/// possible.
///
/// The trait concerns itself with the creation and rendering of `display targets`, which are
/// indexed by integer handles. A display target is not necessarily a unique native window, but can
/// be a texture or GUI widget/internal window. A display target is simply anything that can
/// represent the output of a [VideoRenderer] (which is just a struct now, but will be a trait in
/// the future).
///
/// In some cases, the trait is implemented on top of a pre-initialized backend and gui, such
/// as when we are running under `eframe`. In this case, the trait is implemented on top of egui's
/// Viewport system, and no concept of a native window is present. For this reason, we talk about
/// `viewports` instead of windows. A `Viewport` is a region of the screen that can be rendered to
/// natively, and is the equivalent of a window in a windowing system such as Windows, macOS, or
/// Linux. On the web, there is no concept of a window, so how `viewports` are handled is
/// implementation-dependent.
///
/// Being intended for use in PC emulators, [DisplayManager] handles the concept of a `Video Card`.
/// This is not necessarily a discrete card in an emulated system, but the concept seems to fit
/// and I couldn't think of a better name. A `Video Card` is a logical representation of an emulated
/// display device that can be connected to one *or more* display targets. This is a key detail -
/// we can have multiple viewports displaying the same video card output, with different parameters
/// and potentially different scaling/shaders applied. When creating a context, a [VideoCardId] can
/// be optionally supplied.  If no card id is supplied, the display target will simply not have
/// a video card associated with it - this is fine, a display target can render other things,
/// such as debug displays or GUI widgets.
///
/// [DisplayManager] is extensively generic. The generic parameters are:
/// * B: Graphics `Backend`
/// * G: GUI Context
/// * Vh: Viewport Handle
/// * V: Viewport Context
/// * C: Native Context (such as winit's ActiveEventLoop, or egui's Context)
///
/// If an implementation doesn't require any of these specific types, they can be set to `()`.
///
/// * `Backend`: An implementation of the [DisplayBackend] trait. A graphics backend is a wrapper
///     around a graphics API that provides methods for texture allocation, and maintains a cpu
///     addressable pixel buffer that can be rendered to by a [VideoRenderer].
///
/// * `GUI Context`: A context object that is used to render GUI elements. This is typically used
///     when the display manager is hosting a GUI instead of running on top of one, such as a
///     implementation of a wgpu backend rendering egui. When a Display Manager is hosted on top
///     of a GUI such as eframe, this context is likely minimal or empty.
///
/// * `Event loop abstraction`: An abstraction over the event loop of the windowing system. This
///     is required primarily to support creation of windows under Winit 0.30, which now requires
///     window creation to be done in the event loop. If no event loop is required for window
///     creation this type parameter can be ().
pub trait DisplayManager<B, G, Vh, V, C> {
    /// The native texture handle type for the graphics backend.
    type NativeTexture;
    /// The native texture view type for the graphics backend.
    //type NativeTextureView;
    /// The native encoder type for the graphics backend.
    type NativeEncoder;
    /// The native event loop type
    type NativeEventLoop;
    /// The implementation type of Surface
    type ImplSurface;
    /// The implementation type of Scaler
    type ImplScaler;
    /// The implementation type of DisplayTarget
    type ImplDisplayTarget;

    /// Create a new display target
    /// # Returns:
    /// A `Result` containing either the new [DtHandle] or `Error`.
    fn create_target(
        &mut self,
        name: String,
        dt_type: DisplayTargetType,
        dt_flags: DisplayTargetFlags,
        native_context: Option<&C>,
        viewport: Option<Vh>,
        viewport_opts: Option<DmViewportOptions>,
        card_id: Option<VideoCardId>,
        scaler_preset: String,
        gui_options: &DmGuiOptions,
    ) -> Result<DtHandle, Error>;

    /// Return a vector of [DisplayTargetInfo] representing all displays in the manager. A reference
    /// to a [Machine] must be provided to query video card parameters.
    fn display_info(&self, machine: &Machine) -> Vec<DisplayTargetInfo>;

    /// Return the main `Viewport`.
    /// This viewport should be where the main interface of the emulator is rendered.
    /// (For eframe target, this is the ROOT viewport).
    fn main_viewport(&self) -> Option<V>;

    /// Return the associated `Viewport` given a Viewport ID. This is not always possible,
    /// so the result is an Option.
    fn viewport_by_id(&self, vid: Vh) -> Option<V>;

    /// Return the associated [Viewport] given a [DtHandle].
    fn viewport(&self, dt: DtHandle) -> Option<V>;

    /// Return the [DisplayTargetType] for the specified [DtHandle].
    fn display_type(&self, dt: DtHandle) -> Option<DisplayTargetType>;

    /// Set the [DisplayTargetType] for the specified [DtHandle].
    /// This method can be used to toggle a display target between a GUI widget and a window
    /// background. The corresponding surface and scaler may be updated as needed.
    fn set_display_type(&mut self, dt: DtHandle, new_type: DisplayTargetType) -> Result<(), Error>;

    /// Load and set the specified icon for the main viewport in the DisplayManager.
    /// If the viewport does not support icons, this method should do nothing.
    /// A default implementation is provided that does nothing.
    fn set_icon(&mut self, _icon_path: PathBuf) {}

    /// Load and set the specified icon for the specified viewport in the DisplayManager.
    /// If the viewport does not support icons, this method should do nothing.
    /// A default implementation is provided that does nothing.
    fn set_viewport_icon(&mut self, _vid: Vh, _icon_path: PathBuf) {}

    /// Returns an immutable reference to the [Backend]
    fn backend(&mut self) -> Option<&B>;

    /// Returns a mutable reference to the [Backend]
    fn backend_mut(&mut self) -> Option<&mut B>;

    /// Pass a mutable reference to the main viewport's GUI context to the provided closure.
    fn with_main_gui_mut<F>(&mut self, f: F)
    where
        F: FnOnce(&mut G);

    /// Resolve the GUI context for the specified viewport ID and pass a mutable reference to it to
    /// the provided closure.
    fn with_gui_by_viewport_id_mut<F>(&mut self, vid: Vh, f: F)
    where
        F: FnOnce(&mut G);

    fn with_renderer_mut<F>(&mut self, dt: DtHandle, f: F)
    where
        F: FnOnce(&mut VideoRenderer);

    // TODO: Rethink this function. A card can have multiple renderers. Which one would we return?
    fn with_renderer_by_card_id_mut<F>(&mut self, id: VideoCardId, f: F)
    where
        F: FnOnce(&mut VideoRenderer);

    fn with_primary_renderer_mut<F>(&mut self, f: F)
    where
        F: FnOnce(&mut VideoRenderer);

    /// Reflect a change to a videocard's output resolution, so that associated
    /// resources can be resized as well.
    fn on_card_resized(&mut self, vid: &VideoCardId, extents: &DisplayExtents) -> Result<(), Error>;

    /// Reflect a change in the specified Viewport's dimensions.
    /// Typically called in response to a resize event from a window manager event queue.
    /// The viewport is not actually updated on this call since multiple resize events may be received
    /// per frame. To actually resize the window we must call resize_viewports(), which will apply
    /// the last received resize dimensions for each window.
    fn on_viewport_resized(&mut self, vh: Vh, w: u32, h: u32) -> Result<(), Error>;

    /// Reflect pending viewport resize events, resizing all associated resources as needed.
    fn resize_viewports(&mut self) -> Result<(), Error>;

    /// Execute a closure that is passed the VideoCardId for each VideoCard registered in the
    /// DisplayManager.
    fn for_each_card<F>(&mut self, f: F)
    where
        F: FnMut(&VideoCardId);

    /// Execute a closure that is passed a mutable reference to each VideoRenderer in the manager,
    /// its associated card ID, and a &mut [u8] representing the buffer to which the VideoRenderer
    /// should draw. The buffer is assumed to have been sized correctly by the window manager.
    ///
    /// The card ID can be used to retrieve the internal buffer for the card from the Machine and
    /// call the renderer to create a frame buffer.
    fn for_each_renderer<F>(&mut self, f: F)
    where
        F: FnMut(&mut VideoRenderer, VideoCardId, &mut [u8]);

    /// Execute a closure that is passed a mutable reference to the Surface of the specified display
    /// target.
    fn with_surface_mut<F>(&mut self, dt: DtHandle, f: F) -> Result<(), Error>
    where
        F: FnOnce(&mut B, &mut Self::ImplSurface);

    /// Execute a closure that is passed a mutable reference to the Surface of each Display Target
    /// in the manager.
    /// If dt_type is Some, only surfaces corresponding to a display target of that type will be
    /// passed to the closure.
    fn for_each_surface<F>(&mut self, dt_type_filter: Option<DisplayTargetType>, f: F)
    //where F: FnMut(&mut B, &mut dyn DisplayScaler<B, NativeTextureView=Self::NativeTextureView, NativeEncoder=Self::NativeEncoder>, Option<&mut G>);
    where
        F: FnMut(&mut B, &mut Self::ImplSurface, Option<&mut Self::ImplScaler>, Option<&mut G>);

    /// Execute a closure that is passed a mutable reference to each Display Target in the manager.
    fn for_each_target<F>(&mut self, f: F)
    where
        F: FnMut(&mut Self::ImplDisplayTarget, usize);

    /// Execute a closure that is passed a mutable reference to each Gui context in the manager and
    /// its associated Window.
    fn for_each_gui<F>(&mut self, f: F)
    where
        F: FnMut(&mut G, &V);

    /// Execute a closure that is passed a reference to each Viewport in the manager.
    fn for_each_viewport<F>(&mut self, f: F)
    where
        F: FnMut(&V, bool) -> Option<bool>;

    /// Execute a closure that is passed a reference to the [VideoRenderer] for the specified
    /// display target.
    fn with_renderer<F>(&mut self, dt: DtHandle, f: F)
    where
        F: FnMut(&mut VideoRenderer);

    /// Conditionally execute the provided closure receiving a [DisplayTarget], conditional on
    /// resolution of a [DisplayTarget] for the specified [Viewport] id.
    fn with_target_by_vid<F>(&mut self, vh: Vh, f: F)
    where
        F: FnMut(&mut Self::ImplDisplayTarget);

    /// Conditionally execute the provided closure receiving a reference to the GUI context
    /// and associated Window, conditional on resolution of a DisplayTarget for the specified
    /// [Viewport] ID.
    /// A default implementation is provided that does nothing, if your implementation does not
    /// host GUIs. (Such as when running under eframe).
    fn with_gui_by_vid<F>(&mut self, _vh: Vh, _f: F)
    where
        F: FnMut(&mut G, &V),
    {
    }

    /// Add the new scaler preset definition. It can then later be referenced by name via
    /// get_scaler_preset().
    fn add_scaler_preset(&mut self, preset: ScalerPreset);

    /// Retrieve the scaler preset by name.
    fn scaler_preset(&mut self, name: String) -> Option<&ScalerPreset>;

    /// Apply the named scaler preset to the specified display target.
    fn apply_scaler_preset(&mut self, dt: DtHandle, name: String) -> Result<(), Error>;

    /// Apply the specified scaler parameters to the specified display target.
    fn apply_scaler_params(&mut self, dt: DtHandle, params: &ScalerParams) -> Result<(), Error>;

    /// Get the scaler parameters for the specified display target.
    fn scaler_params(&self, dt: DtHandle) -> Option<ScalerParams>;

    /// Set the desired Display Aperture for the specified display target.
    /// Returns the associated [VideoCardId], as the card will need to be resized when the aperture
    /// is changed.
    fn set_display_aperture(
        &mut self,
        dt: DtHandle,
        aperture: DisplayApertureType,
    ) -> Result<Option<VideoCardId>, Error>;

    /// Enable or disable aspect correction for the specified display target.
    /// The display manager will perform the required resizing of display target resources
    /// and perform buffer clearing.
    fn set_aspect_correction(&mut self, dt: DtHandle, state: bool) -> Result<(), Error>;

    /// Set the ScalerMode for the associated scaler, if present.
    fn set_scaler_mode(&mut self, dt: DtHandle, mode: ScalerMode) -> Result<(), Error>;

    /// Save a screenshot of the specified display target to the specified path.
    /// A unique filename will be generated assuming the path is a directory.
    /// No operational error is returned as screenshot operation may be deferred.
    fn save_screenshot(&mut self, dt: DtHandle, path: impl AsRef<Path>) -> Result<PathBuf, Error>;
}

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

   frontend_common::display_manager::mod.rs

   Define the DisplayManager trait.

   This trait defines an interface for managing display targets for a given
   graphics backend and windowing system combination.
*/
use anyhow::Error;
use marty_core::{cpu_808x::Displacement, machine::Machine};
use std::{
    fmt::{Display, Formatter},
    path::PathBuf,
};

use crate::color::MartyColor;
pub use crate::types::display_target_dimensions::DisplayTargetDimensions;

use crate::display_scaler::ScalerPreset;
use marty_core::videocard::{VideoCardId, VideoType};
use videocard_renderer::VideoRenderer;

#[derive(Copy, Clone)]
pub enum DisplayTargetType {
    WindowBackground { main_window: bool },
    EguiWidget,
}

impl Default for DisplayTargetType {
    fn default() -> Self {
        DisplayTargetType::WindowBackground { main_window: false }
    }
}

impl Display for DisplayTargetType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DisplayTargetType::WindowBackground { .. } => {
                write!(f, "Window")
            }
            DisplayTargetType::EguiWidget => {
                write!(f, "EGUI Widget")
            }
        }
    }
}

#[derive(Clone)]
pub struct DisplayInfo {
    pub dtype: DisplayTargetType,
    pub vtype: Option<VideoType>,
    pub vid:   Option<VideoCardId>,
    pub name:  String,
}

pub struct DisplayManagerGuiOptions {
    pub theme_dark:  bool,
    pub theme_color: Option<u32>,
}

/// Options for windows targets. All dimensions are specified as inner size (client area)
pub struct DisplayManagerWindowOptions {
    pub size: DisplayTargetDimensions,
    pub min_size: Option<DisplayTargetDimensions>,
    pub max_size: Option<DisplayTargetDimensions>,
    pub title: String,
    pub resizable: bool,
    pub always_on_top: bool,
}

impl Default for DisplayManagerWindowOptions {
    fn default() -> Self {
        Self {
            size: Default::default(),
            min_size: Default::default(),
            max_size: Default::default(),
            title: "New Window".to_string(),
            resizable: false,
            always_on_top: false,
        }
    }
}

/// The DisplayManager trait is implemented by a DisplayManager that combines
/// the facilities of a windowing system (Such as Winit), graphics backend
/// (such as Pixels/wgpu), and gui (such as egui)
/// The generic parameters are:
/// B: Graphics Backend
/// G: Gui Context
/// Wi: Window ID
/// W: Window context
pub trait DisplayManager<B, G, Wi, W> {
    type NativeTextureView;
    type NativeEncoder;

    type ImplScaler;
    type ImplDisplayTarget;

    fn create_target(
        &mut self,
        name: String,
        ttype: DisplayTargetType,
        wid: Option<Wi>,
        window: Option<&W>,
        window_opts: Option<DisplayManagerWindowOptions>,
        card_id: Option<VideoCardId>,
        w: u32,
        h: u32,
        fill_color: Option<MartyColor>,
        gui_options: &DisplayManagerGuiOptions,
    ) -> Result<(), Error>;

    /// Return a vector of DisplayInfo structs representing all displays in the manager. A reference
    /// to a Machine must be provided to query video card parameters.
    fn get_display_info(&self, machine: &Machine) -> Vec<DisplayInfo>;

    /// Return the associated Window given a c
    fn get_window_by_id(&self, wid: Wi) -> Option<&W>;

    /// Load and set the specified icon for each window in the DisplayManager.
    fn set_icon(&mut self, icon_path: PathBuf);

    /// Return the main Window. This will be the window where the main gui (if present)
    /// is rendered.
    fn get_main_window(&self) -> Option<&W>;

    /// Returns the associated Backend for the main window.
    fn get_main_backend(&mut self) -> Option<&B>;

    /// Returns the associated Gui render context for the main window.
    fn get_main_gui_mut(&mut self) -> Option<&mut G>;

    /// Returns a mutable reference to the associated Backend for the main window.
    fn get_main_backend_mut(&mut self) -> Option<&mut B>;

    /// Return the associated VideoRenderer given a card id. If the card id is not found,
    /// returns None.
    fn get_renderer_by_card_id(&mut self, id: VideoCardId) -> Option<&mut VideoRenderer>;

    /// Returns the associated VideoRenderer for the primary video card. If no primary card
    /// is present, returns None.
    fn get_primary_renderer(&mut self) -> Option<&mut VideoRenderer>;

    /// Reflect a change to a videocard's output resolution, so that associated
    /// resources can be resized as well.
    fn on_card_resized(&mut self, id: VideoCardId, w: u32, h: u32, doublescan: bool) -> Result<(), Error>;

    /// Reflect a change in the specified window's dimensions, so that associated
    /// resources can be resized as well.
    /// Typically called in response to a resize event from a window manager event queue.
    fn on_window_resized(&mut self, wid: Wi, w: u32, h: u32) -> Result<(), Error>;

    /// Render a VideoCard given the specified card_id. A card may have more than one
    /// RenderTarget, so this call may affect multiple windows / surfaces.
    fn render_card(&mut self, card_id: VideoCardId) -> Result<(), Error>;

    /// Render all cards managed by the DisplayManager. Typically called at the end of
    /// event processing every frame.
    fn render_all_cards(&mut self) -> Result<(), Error>;

    /// Execute a closure that is passed a mutable reference to each VideoRenderer in the manager,
    /// its associated card ID, and a &mut [u8] representing the buffer to which the VideoRenderer
    /// should draw. The buffer is assumed to have been sized correctly by the window manager.
    ///
    /// The card ID can be used to retrieve the internal buffer for the card from the Machine and
    /// call the renderer to create a frame buffer.
    fn for_each_renderer<F>(&mut self, f: F)
    where
        F: FnMut(&mut VideoRenderer, VideoCardId, &mut [u8]);

    /// Execute a closure that is passed a mutable reference to each Backend in the manager.
    fn for_each_backend<F>(&mut self, f: F)
    //where F: FnMut(&mut B, &mut dyn DisplayScaler<B, NativeTextureView=Self::NativeTextureView, NativeEncoder=Self::NativeEncoder>, Option<&mut G>);
    where
        F: FnMut(&mut B, &mut Self::ImplScaler, Option<&mut G>);

    /// Execute a closure that is passed a mutable reference to each RenderTarget in the manager.
    fn for_each_target<F>(&mut self, f: F)
    where
        F: FnMut(&mut Self::ImplDisplayTarget);

    /// Execute a closure that is passed a mutable reference to each Gui context in the manager and
    /// its associated Window.
    fn for_each_gui<F>(&mut self, f: F)
    where
        F: FnMut(&mut G, &W);

    /// Execute a closure that is passed a reference to each Window in the manager.
    fn for_each_window<F>(&mut self, f: F)
    where
        F: FnMut(&W);

    /// Conditionally execute the provided closure receiving a DisplayTarget, condtional on
    /// resolution of a DisplayTarget for the specified Window ID.
    fn with_target_by_wid<F>(&mut self, wid: Wi, f: F)
    where
        F: FnMut(&mut Self::ImplDisplayTarget);

    /// Conditionally execute the provided closure receiving a Gui context, condtional on
    /// resolution of a DisplayTarget for the specified Window ID.
    fn with_gui_by_wid<F>(&mut self, wid: Wi, f: F)
    where
        F: FnMut(&mut G);

    /// Add the new scaler preset definition. It can then later be referenced by name via
    /// get_scaler_preset().
    fn add_scaler_preset(&mut self, preset: ScalerPreset);

    /// Retrieve the scaler preset by name.
    fn get_scaler_preset(&mut self, name: String) -> Option<&ScalerPreset>;
}

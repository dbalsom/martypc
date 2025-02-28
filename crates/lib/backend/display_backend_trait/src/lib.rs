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

    display_backend_common::lib.rs

    Defines a DisplayBackend trait that can be implemented to abstract various
    display backends such as Pixels or (eventually) SDL.

    DisplayBackend has the concept of a 'buffer' which is the raw u8 pixel
    data to be displayed during rendering, and a 'surface' which is the
    destination target for rendering. These may not be the same size, ie, when
    hardware scaling is in effect. In general, the surface should be resized
    when the attached window is resized.

    The render method takes a VideoRenderer trait object, defined by a specific
    frontend. This allows a lot of flexibility in how the image is ultimately
    presented - for example the Pixels VideoRenderer supports multiple scaling
    options and CRT shader effects.

    A DisplayBackend should be able to be instantiated multiple times, to
    support multiple windows/displays.
*/
#![feature(trait_alias)]
#[cfg(not(any(feature = "use_wgpu", feature = "use_egui_backend")))]
compile_error!("Either the 'use_wgpu' or 'use_egui_backend' feature must be enabled.");
#[cfg(all(feature = "use_wgpu", feature = "use_egui_backend"))]
compile_error!("Only one of the 'use_wgpu' or 'use_egui_backend' features can be enabled.");

use std::sync::{Arc, RwLock};
use thiserror::Error;

#[cfg(feature = "use_wgpu")]
pub type DynDisplayTargetSurface = Arc<
    RwLock<
        dyn DisplayTargetSurface<
            NativeTexture = wgpu::Texture,
            NativeDevice = wgpu::Device,
            NativeQueue = wgpu::Queue,
            NativeTextureFormat = wgpu::TextureFormat,
        >,
    >,
>;

#[cfg(feature = "use_egui_backend")]
use egui;
#[cfg(not(feature = "use_wgpu"))]
pub type DynDisplayTargetSurface = Arc<
    RwLock<
        dyn DisplayTargetSurface<
            NativeTexture = egui::TextureHandle,
            NativeDevice = (),
            NativeQueue = (),
            NativeTextureFormat = (),
        >,
    >,
>;

#[derive(Error, Debug)]
pub enum DisplayBackendError {
    #[error("Initialization failed: {0}")]
    InitializationError(String),
    #[error("Validation failed: {0}")]
    ValidationError(String),
    #[error("Render failed: {0}")]
    RenderError(String),
}

/*
impl Display for DisplayBackendError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            DisplayBackendError::InitializationError(ref msg) => write!(f, "Initialization Error: {}", msg),
            DisplayBackendError::ValidationError(ref msg) => write!(f, "Validation Error: {}", msg),
            DisplayBackendError::RenderError(ref msg) => write!(f, "Validation Error: {}", msg),
            // Handle other cases
        }
    }
}

 */

#[derive(Copy, Clone, Debug)]
pub struct BufferDimensions {
    pub w: u32,
    pub h: u32,
    pub pitch: u32,
}

#[derive(Copy, Clone, Debug)]
pub struct TextureDimensions {
    pub w: u32,
    pub h: u32,
}

impl From<BufferDimensions> for TextureDimensions {
    fn from(d: BufferDimensions) -> Self {
        TextureDimensions { w: d.w, h: d.h }
    }
}

impl From<(u32, u32, u32)> for BufferDimensions {
    fn from(t: (u32, u32, u32)) -> Self {
        BufferDimensions {
            w: t.0,
            h: t.1,
            pitch: t.2,
        }
    }
}

impl From<(u32, u32)> for TextureDimensions {
    fn from(t: (u32, u32)) -> Self {
        TextureDimensions { w: t.0, h: t.1 }
    }
}

use anyhow::Error;

#[cfg(not(target_arch = "wasm32"))]
pub trait ThreadSafe: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait ThreadSafe {}

#[cfg(not(target_arch = "wasm32"))]
impl<T> ThreadSafe for T where T: Send + Sync {} // Implement it for all Send + Sync types

#[cfg(target_arch = "wasm32")]
impl<T> ThreadSafe for T where T: Sized {} // Implement it for all types on WASM

/// The [DisplayTargetSurface] trait defines an interface representing a display target surface
/// for emulator rendering. Typically, this comprises three parts:
/// - an RGBA, 32-bit pixel buffer containing the rendered frame data
/// - a dirty flag representing whether the pixel buffer has been modified and should be re-uploaded
/// - a texture object representing the uploaded pixel buffer
/// - a texture object representing a scaled display surface - usually produced by a scaling shader
/// It is this latter texture that is ultimately presented to the display.
pub trait DisplayTargetSurface: Send + Sync {
    type NativeDevice;
    type NativeQueue;
    type NativeTexture;
    type NativeTextureFormat;
    /// Retrieve the pixel buffer dimensions.
    fn buf_dimensions(&self) -> BufferDimensions;
    /// Retrieve the backing texture dimensions.
    fn backing_dimensions(&self) -> TextureDimensions;
    /// Resize the backing texture and RGBA pixel buffer.
    fn resize_backing(&mut self, device: Arc<Self::NativeDevice>, new_dim: BufferDimensions) -> Result<(), Error>;
    /// Update the backing texture from the RGBA pixel buffer.
    fn update_backing(&mut self, device: Arc<Self::NativeDevice>, queue: Arc<Self::NativeQueue>) -> Result<(), Error>;
    /// Retrieve the display surface dimensions.
    fn surface_dimensions(&self) -> TextureDimensions;
    /// Resize the display surface texture.
    fn resize_surface(
        &mut self,
        device: Arc<Self::NativeDevice>,
        queue: Arc<Self::NativeQueue>,
        new_dim: TextureDimensions,
    ) -> Result<(), Error>;
    /// Retrieve an immutable reference to the RGBA pixel buffer data.
    fn buf(&self) -> &[u8];
    /// Retrieve a mutable reference to the RGBA pixel buffer data.
    fn buf_mut(&mut self) -> &mut [u8];
    /// Retrieve the pixel buffer backing texture.
    fn backing_texture(&self) -> Arc<Self::NativeTexture>;
    /// Retrieve the pixel buffer backing texture format.
    fn backing_texture_format(&self) -> Self::NativeTextureFormat;
    /// Retrieve the display surface texture.
    fn surface_texture(&self) -> Arc<Self::NativeTexture>;
    /// Retrieve the dirty flag.
    fn dirty(&self) -> bool;
    /// Set the dirty flag.
    fn set_dirty(&mut self, dirty: bool);
}

/// The [DisplayBackend] trait is an attempt to create a generic interface for various graphical
/// backends. It was originally designed to support `wgpu`, and its design may not be suitable for
/// all backends yet.  I would like to be able to have an `SDL3` backend as well; we'll see if this
/// trait can be expanded to support that.
pub trait DisplayBackend<'p, 'win, G> {
    /// The native type for a device instance. Ideally, this should be Clone.
    /// For wgpu, this is [wgpu::Device].
    type NativeDevice;
    /// The native type for the device queue. Ideally, this should be Clone.
    /// For wgpu, this is [wgpu::Queue].
    type NativeQueue;
    /// The native type for a Texture.
    /// For wgpu, this is [wgpu::Texture].
    type NativeTexture;
    /// The native type for a TextureFormat.
    type NativeTextureFormat;
    /// Originally I wrote a DisplayBackend that was a simple wrapper around the Pixels crate, and
    /// this returned a reference to the Pixels instance. I'm not sure if this is necessary.
    type NativeBackend;
    /// A type alias for a structure containing adapter information. It may not always be available.
    /// Therefore, the trait interface returns this as an Option. For greatest flexibility I should
    /// probably implement an AdapterInfo trait and return a trait object.
    type NativeBackendAdapterInfo;
    /// The native type for a scaler. For wgpu backends, this defines a shader that is used to
    /// scale and apply effects to the pixel buffer before rendering to a display target surface.
    type NativeScaler;

    /// Return a structure containing information about the backend adapter, or None if no
    /// adapter information is available (web targets, for example).
    fn adapter_info(&self) -> Option<Self::NativeBackendAdapterInfo>;
    /// Return the native device object for the backend.
    fn device(&self) -> Arc<Self::NativeDevice>;
    /// Return the native queue object for the backend.
    fn queue(&self) -> Arc<Self::NativeQueue>;
    /// Create a new display target surface.
    fn create_surface(
        &self,
        buffer_size: BufferDimensions,
        surface_size: TextureDimensions,
    ) -> Result<DynDisplayTargetSurface, Error>;
    /// Resize the cpu pixel buffer and backing texture to the specified dimensions, or return an error.
    fn resize_backing_texture(
        &mut self,
        surface: &mut DynDisplayTargetSurface,
        new_dim: BufferDimensions,
    ) -> Result<(), Error>;
    /// Resize the display surface to the specified dimensions, or return an error.
    fn resize_surface_texture(
        &mut self,
        surface: &mut DynDisplayTargetSurface,
        new_dim: TextureDimensions,
    ) -> Result<(), Error>;

    fn get_backend_raw(&mut self) -> Option<&mut Self::NativeBackend>;

    /// Render the pixel buffer. If a scaler is provided, it will be used to scale the pixel buffer
    /// to the display surface. If a GUI renderer is provided, it can be used to either
    /// overlay a GUI on top of the display surface, or render the display surface with the GUI,
    /// depending on the backend implementation.
    fn render(
        &mut self,
        surface: &mut DynDisplayTargetSurface,
        scaler: Option<&mut Self::NativeScaler>,
        gui_renderer: Option<&mut G>,
    ) -> Result<(), Error>;

    // Present the rendered frame to the display.
    // This method should be called every host frame to display the rendered frame.
    // It may not need to be specifically implemented by all backends, so a default implementation is provided.
    // fn present(&mut self) -> Result<(), Error> {
    //     Ok(())
    // }
}

pub trait DisplayBackendBuilder {
    fn build(buffer_size: BufferDimensions, surface_size: TextureDimensions) -> Self
    where
        Self: Sized;
}

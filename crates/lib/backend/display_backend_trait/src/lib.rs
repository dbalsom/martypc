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

use thiserror::Error;

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
pub struct SurfaceDimensions {
    pub w: u32,
    pub h: u32,
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

impl From<(u32, u32)> for SurfaceDimensions {
    fn from(t: (u32, u32)) -> Self {
        SurfaceDimensions { w: t.0, h: t.1 }
    }
}

use anyhow::Error;

pub trait DisplayBackend<'p, 'win, G> {
    type NativeBackend;
    type NativeBackendAdapterInfo;
    type NativeScaler;

    fn get_adapter_info(&self) -> Option<Self::NativeBackendAdapterInfo>;
    fn resize_buf(&mut self, new: BufferDimensions) -> Result<(), Error>;
    fn resize_surface(&mut self, new: SurfaceDimensions) -> Result<(), Error>;
    fn buf_dimensions(&self) -> BufferDimensions;
    fn surface_dimensions(&self) -> SurfaceDimensions;
    fn buf(&self) -> &[u8];
    fn buf_mut(&mut self) -> &mut [u8];
    fn get_backend_raw(&mut self) -> Option<&mut Self::NativeBackend>;
    fn render(&mut self, scaler: Option<&mut Self::NativeScaler>, gui_renderer: Option<&mut G>) -> Result<(), Error>;

    /// Present the rendered frame to the display.
    /// This method should be called every host frame to display the rendered frame.
    /// It may not need to be specifically implemented by all backends, so a default implementation is provided.
    fn present(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

pub trait DisplayBackendBuilder {
    fn build(buffer_size: BufferDimensions, surface_size: SurfaceDimensions) -> Self
    where
        Self: Sized;
}

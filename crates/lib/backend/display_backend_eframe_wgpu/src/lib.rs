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

    display_backend_pixels::lib.rs

    Implements DisplayBackend for the Pixels backend
*/

mod display_window;
mod gui;
pub mod surface;
mod util;

pub use display_backend_trait::{
    BufferDimensions,
    DisplayBackend,
    DisplayBackendBuilder,
    DisplayTargetSurface,
    DynDisplayTargetSurface,
    TextureDimensions,
};
pub use surface::EFrameBackendSurface;

use std::sync::{Arc, RwLock};

use marty_scaler_null::DisplayScaler;

use anyhow::{anyhow, Error};
use egui;
use egui_wgpu::wgpu;

pub struct EFrameBackend {
    ctx: egui::Context,
    adapter_info: Option<wgpu::AdapterInfo>, // Adapter information
    device: Arc<wgpu::Device>,               // Wgpu device. Cloneable handle to the GPU device instance.
    queue: Arc<wgpu::Queue>,                 // Wgpu queue. Cloneable handle to the GPU rendering queue instance.
    texture_format: wgpu::TextureFormat,
}

impl EFrameBackend {
    pub fn new(
        ctx: egui::Context,
        _buffer_dim: BufferDimensions,
        _surface_dim: TextureDimensions,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        texture_format: wgpu::TextureFormat,
    ) -> Result<EFrameBackend, Error> {
        Ok(EFrameBackend {
            ctx,
            adapter_info: None,
            device,
            queue,
            texture_format,
        })
    }
}

impl DisplayBackendBuilder for EFrameBackend {
    fn build(_buffer_size: BufferDimensions, _surface_size: TextureDimensions) -> Self
    where
        Self: Sized,
    {
        todo!()
    }
}

pub type EFrameScalerType = Box<
    dyn DisplayScaler<
        wgpu::Device,
        wgpu::Queue,
        wgpu::Texture,
        NativeTextureView = wgpu::TextureView,
        NativeEncoder = wgpu::CommandEncoder,
        NativeRenderPass = wgpu::RenderPass<'static>,
    >,
>;

impl EFrameBackend {}

impl DisplayBackend<'_, '_, ()> for EFrameBackend {
    type NativeDevice = wgpu::Device;
    type NativeQueue = wgpu::Queue;
    type NativeTexture = wgpu::Texture;
    type NativeTextureFormat = wgpu::TextureFormat;
    type NativeBackend = ();
    type NativeBackendAdapterInfo = wgpu::AdapterInfo;
    type NativeScaler = EFrameScalerType;

    fn adapter_info(&self) -> Option<Self::NativeBackendAdapterInfo> {
        Some(self.adapter_info.clone()?)
    }

    fn device(&self) -> Arc<Self::NativeDevice> {
        self.device.clone()
    }

    fn queue(&self) -> Arc<Self::NativeQueue> {
        self.queue.clone()
    }

    /// Create a new display target surface as a [DynDisplayTargetSurface].
    /// A display target surface comprises:
    /// - A backing virtual pixel buffer (Vec<u8>).
    /// - A wgpu texture corresponding to the pixel buffer.
    /// - A wgpu texture corresponding to the display surface, upon which the pixel buffer texture
    ///   is rendered with a scaler / shader.
    ///
    /// The 'display surface' may not be the actual display surface; it may be an intermediate
    /// texture. It is the display surface from the perspective of a display target.
    fn create_surface(
        &self,
        buffer_size: BufferDimensions,
        surface_size: TextureDimensions,
    ) -> Result<DynDisplayTargetSurface, Error> {
        let (backing_texture, buf_size) = util::create_texture(
            self.device.clone(),
            TextureDimensions {
                w: buffer_size.w,
                h: buffer_size.h,
            },
            self.texture_format,
        )?;

        // Create the backing vector for the pixel buffer.
        let cpu_buffer = vec![0u8; buf_size];

        let (surface_texture, _) = util::create_texture(self.device.clone(), surface_size, self.texture_format)?;

        Ok(Arc::new(RwLock::new(EFrameBackendSurface {
            pixel_dimensions: buffer_size,
            pixels: cpu_buffer,
            backing: Arc::new(backing_texture),
            surface: Arc::new(surface_texture),
            texture_format: self.texture_format,
            texture_format_size: util::texture_format_size(self.texture_format),
            dirty: false,
        })))
    }

    fn resize_backing_texture(
        &mut self,
        surface: &mut DynDisplayTargetSurface,
        new_dim: BufferDimensions,
    ) -> Result<(), Error> {
        surface.write().unwrap().resize_backing(self.device.clone(), new_dim)?;
        Ok(())
    }

    fn resize_surface_texture(
        &mut self,
        surface: &mut DynDisplayTargetSurface,
        new_dim: TextureDimensions,
    ) -> Result<(), Error> {
        surface
            .write()
            .unwrap()
            .resize_surface(self.device.clone(), self.queue.clone(), new_dim)?;
        Ok(())
    }

    fn get_backend_raw(&mut self) -> Option<&mut Self::NativeBackend> {
        None
    }

    fn render(
        &mut self,
        surface: &mut DynDisplayTargetSurface,
        _scaler: Option<&mut Self::NativeScaler>,
        _gui: Option<&mut ()>,
    ) -> Result<(), Error> {
        // Update backing texture here if dirty.
        Ok(())
    }
}

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

use crate::display_window::DisplayWindow;
pub use display_backend_trait::{
    BufferDimensions,
    DisplayBackend,
    DisplayBackendBuilder,
    SurfaceDimensions,
    //DisplayBackendError
};

use marty_scaler_null::DisplayScaler;

use anyhow::{anyhow, Error};
use egui;

#[cfg(feature = "use_wgpu")]
use egui_wgpu::wgpu;

#[derive(Debug)]
pub enum EFrameBackendType {
    RenderPass,
    EguiWindow,
}

pub struct EFrameBackend {
    be_type: EFrameBackendType,
    ctx: egui::Context,
    cpu_buffer: Vec<u8>, // Virtual pixel buffer
    buffer_dim: BufferDimensions,
    surface_dim: SurfaceDimensions,
    #[cfg(feature = "use_wgpu")]
    adapter_info: Option<wgpu::AdapterInfo>, // Adapter information
    buffer_handle: Option<egui::TextureHandle>,  // Egui texture handle
    surface_handle: Option<egui::TextureHandle>, // Egui texture handle
    win: DisplayWindow,
}

impl EFrameBackend {
    pub fn new(
        be_type: EFrameBackendType,
        ctx: egui::Context,
        buffer_dim: BufferDimensions,
        surface_dim: SurfaceDimensions,
        //wgpu_render_state: &eframe::RenderState,
        #[cfg(feature = "use_wgpu")] adapter_info: Option<wgpu::AdapterInfo>,
        #[cfg(not(feature = "use_wgpu"))] _adapter_info: Option<()>,
    ) -> Result<EFrameBackend, Error> {
        //let adapter_info = wgpu_render_state.adapter_info.clone();

        let cpu_buffer = vec![0; buffer_dim.w as usize * buffer_dim.h as usize * 4];

        let buffer_image = egui::ColorImage {
            size:   [buffer_dim.w as usize, buffer_dim.h as usize],
            pixels: cpu_buffer
                .chunks_exact(4)
                .map(|rgba| egui::Color32::from_rgba_premultiplied(rgba[0], rgba[1], rgba[2], rgba[3]))
                .collect(),
        };
        let buffer_handle = ctx.load_texture("marty_buffer_texture", buffer_image, egui::TextureOptions::default());

        Ok(EFrameBackend {
            be_type,
            ctx,
            cpu_buffer,
            buffer_dim,
            surface_dim,
            #[cfg(feature = "use_wgpu")]
            adapter_info,
            buffer_handle: Some(buffer_handle),
            surface_handle: None,
            win: Default::default(),
        })
    }
}

impl DisplayBackendBuilder for EFrameBackend {
    fn build(_buffer_size: BufferDimensions, _surface_size: SurfaceDimensions) -> Self
    where
        Self: Sized,
    {
        todo!()
    }
}

impl DisplayBackend<'_, '_, ()> for EFrameBackend {
    type NativeBackend = ();
    #[cfg(feature = "use_wgpu")]
    type NativeBackendAdapterInfo = wgpu::AdapterInfo;
    #[cfg(not(feature = "use_wgpu"))]
    type NativeBackendAdapterInfo = ();

    #[cfg(feature = "use_wgpu")]
    type NativeScaler =
        Box<dyn DisplayScaler<(), NativeTextureView = wgpu::TextureView, NativeEncoder = wgpu::CommandEncoder>>;
    #[cfg(not(feature = "use_wgpu"))]
    type NativeScaler = Box<dyn DisplayScaler<(), NativeTextureView = (), NativeEncoder = ()>>;

    #[cfg(feature = "use_wgpu")]
    fn get_adapter_info(&self) -> Option<Self::NativeBackendAdapterInfo> {
        Some(self.adapter_info.clone()?)
    }
    #[cfg(not(feature = "use_wgpu"))]
    fn get_adapter_info(&self) -> Option<Self::NativeBackendAdapterInfo> {
        None
    }

    fn resize_buf(&mut self, new: BufferDimensions) -> Result<(), Error> {
        self.cpu_buffer.resize((new.w * new.h * 4) as usize, 0);
        self.buffer_dim = (new.w, new.h, new.w).into();
        Ok(())
    }

    fn resize_surface(&mut self, new: SurfaceDimensions) -> Result<(), Error> {
        //self.pixels.resize_surface(new.w, new.h)?;
        self.surface_dim = (new.w, new.h).into();
        Ok(())
    }

    fn buf_dimensions(&self) -> BufferDimensions {
        self.buffer_dim
    }
    fn surface_dimensions(&self) -> SurfaceDimensions {
        self.surface_dim
    }

    fn buf(&self) -> &[u8] {
        &self.cpu_buffer
    }
    fn buf_mut(&mut self) -> &mut [u8] {
        &mut self.cpu_buffer
    }

    fn get_backend_raw(&mut self) -> Option<&mut Self::NativeBackend> {
        None
    }

    fn render(
        &mut self,
        _scaler: Option<&mut Box<(dyn DisplayScaler<(), NativeEncoder = (), NativeTextureView = ()> + 'static)>>,
        _gui: Option<&mut ()>,
    ) -> Result<(), Error> {
        //log::trace!("Rendering eframe backend: {:?}", self.be_type);

        // Update texture handle
        if let Some(texture_handle) = &mut self.buffer_handle {
            texture_handle.set(
                egui::ColorImage {
                    size:   [self.buffer_dim.w as usize, self.buffer_dim.h as usize],
                    pixels: self
                        .cpu_buffer
                        .chunks_exact(4)
                        .map(|rgba| egui::Color32::from_rgba_premultiplied(rgba[0], rgba[1], rgba[2], rgba[3]))
                        .collect(),
                },
                egui::TextureOptions::default(),
            );
            Ok(())
        }
        else {
            Err(anyhow!("No buffer handle"))
        }
    }

    fn present(&mut self) -> Result<(), Error> {
        match self.be_type {
            EFrameBackendType::EguiWindow => {
                self.win
                    .show(&mut self.ctx, "Display", self.buffer_handle.as_ref().unwrap());
            }
            EFrameBackendType::RenderPass => {
                todo!();
            }
        }
        Ok(())
    }
}

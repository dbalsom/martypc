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

pub use display_backend_trait::{
    BufferDimensions,
    DisplayBackend,
    DisplayBackendBuilder,
    SurfaceDimensions,
    //DisplayBackendError
};

use winit::window::Window;

//use marty_egui::context::GuiRenderContext;
use marty_pixels_scaler::DisplayScaler;

use anyhow::{anyhow, Error};
use eframe::{
    egui,
    egui::TextureOptions,
    wgpu::{CommandEncoder, TextureView},
};

pub struct EFrameBackend {
    cpu_buffer:   Vec<u8>, // Virtual pixel buffer
    buffer_dim:   BufferDimensions,
    surface_dim:  SurfaceDimensions,
    adapter_info: Option<eframe::wgpu::AdapterInfo>, // Adapter information

    buffer_handle:  Option<eframe::egui::TextureHandle>, // Egui texture handle
    surface_handle: Option<eframe::egui::TextureHandle>, // Egui texture handle
}

impl EFrameBackend {
    pub fn new(
        buffer_dim: BufferDimensions,
        surface_dim: SurfaceDimensions,
        //wgpu_render_state: &eframe::RenderState,
        adapter_info: Option<eframe::wgpu::AdapterInfo>,
    ) -> Result<EFrameBackend, Error> {
        //let adapter_info = wgpu_render_state.adapter_info.clone();

        let cpu_buffer = vec![0; buffer_dim.w as usize * buffer_dim.h as usize * 4];

        Ok(EFrameBackend {
            cpu_buffer,
            buffer_dim,
            surface_dim,
            adapter_info,
            buffer_handle: None,
            surface_handle: None,
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

impl DisplayBackend<'_, '_, eframe::egui::Context> for EFrameBackend {
    type NativeBackend = ();
    type NativeBackendAdapterInfo = eframe::wgpu::AdapterInfo;
    type NativeScaler = Box<
        dyn DisplayScaler<
            (),
            NativeTextureView = eframe::wgpu::TextureView,
            NativeEncoder = eframe::wgpu::CommandEncoder,
        >,
    >;

    fn get_adapter_info(&self) -> Option<Self::NativeBackendAdapterInfo> {
        Some(self.adapter_info.clone()?)
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
        scaler: Option<
            &mut Box<
                (dyn DisplayScaler<(), NativeTextureView = TextureView, NativeEncoder = CommandEncoder> + 'static),
            >,
        >,
        gui: Option<&mut eframe::egui::Context>,
    ) -> Result<(), Error> {
        let egui_ctx = match gui {
            Some(ctx) => ctx,
            None => return Err(anyhow!("No GUI context provided")),
        };

        // Update or create texture handle
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
                TextureOptions::default(),
            );
            Ok(())
        }
        else {
            self.buffer_handle = Some(
                egui_ctx.load_texture(
                    "display_texture",
                    egui::ColorImage {
                        size:   [self.buffer_dim.w as usize, self.buffer_dim.h as usize],
                        pixels: self
                            .cpu_buffer
                            .chunks_exact(4)
                            .map(|rgba| egui::Color32::from_rgba_premultiplied(rgba[0], rgba[1], rgba[2], rgba[3]))
                            .collect(),
                    },
                    Default::default(),
                ),
            );
            Ok(())
        }
    }
}

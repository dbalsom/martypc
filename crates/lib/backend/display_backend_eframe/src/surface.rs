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

use std::sync::Arc;

use display_backend_trait::{BufferDimensions, DisplayTargetSurface, TextureDimensions};

use anyhow::Error;

pub struct EFrameBackendSurface {
    pub ctx: egui::Context,
    pub pixels: Vec<u8>, // Virtual pixel buffer
    pub buffer: egui::TextureHandle,
    pub buffer_dim: BufferDimensions,
    pub surface_dim: TextureDimensions,
    pub dirty: bool,
}

impl DisplayTargetSurface for EFrameBackendSurface {
    type NativeDevice = ();
    type NativeQueue = ();
    type NativeTexture = egui::TextureHandle;
    type NativeTextureFormat = ();

    fn buf_dimensions(&self) -> BufferDimensions {
        self.buffer_dim
    }

    fn backing_dimensions(&self) -> TextureDimensions {
        TextureDimensions {
            w: self.buffer.size()[0] as u32,
            h: self.buffer.size()[1] as u32,
        }
    }

    fn resize_backing(&mut self, _device: Arc<Self::NativeDevice>, new_dim: BufferDimensions) -> Result<(), Error> {
        self.pixels.resize(new_dim.w as usize * new_dim.h as usize * 4, 0);
        let buffer_image = egui::ColorImage {
            size:   [new_dim.w as usize, new_dim.h as usize],
            pixels: self
                .pixels
                .chunks_exact(4)
                .map(|rgba| egui::Color32::from_rgba_premultiplied(rgba[0], rgba[1], rgba[2], rgba[3]))
                .collect(),
        };

        self.buffer_dim = new_dim;
        self.buffer = self
            .ctx
            .load_texture("marty_buffer_texture", buffer_image, egui::TextureOptions::default());

        Ok(())
    }

    fn update_backing(&mut self, device: Arc<Self::NativeDevice>, queue: Arc<Self::NativeQueue>) -> Result<(), Error> {
        let texture_manager = self.ctx.tex_manager();
        let buffer_image = egui::ColorImage {
            size:   [self.buffer_dim.w as usize, self.buffer_dim.h as usize],
            pixels: self
                .pixels
                .chunks_exact(4)
                .map(|rgba| egui::Color32::from_rgba_premultiplied(rgba[0], rgba[1], rgba[2], rgba[3]))
                .collect(),
        };

        let image_delta = egui::epaint::ImageDelta {
            image: buffer_image.into(),
            options: Default::default(),
            pos: None,
        };
        texture_manager.write().set(self.buffer.id(), image_delta);
        Ok(())
    }

    fn surface_dimensions(&self) -> TextureDimensions {
        self.surface_dim
    }

    fn resize_surface(
        &mut self,
        _device: Arc<Self::NativeDevice>,
        _queue: Arc<Self::NativeQueue>,
        new_dim: TextureDimensions,
    ) -> Result<(), Error> {
        self.surface_dim = new_dim;
        Ok(())
    }

    fn buf(&self) -> &[u8] {
        &self.pixels
    }

    fn buf_mut(&mut self) -> &mut [u8] {
        &mut self.pixels
    }

    fn backing_texture(&self) -> Arc<Self::NativeTexture> {
        Arc::new(self.buffer.clone())
    }

    fn backing_texture_format(&self) -> Self::NativeTextureFormat {
        ()
    }

    fn surface_texture(&self) -> Arc<Self::NativeTexture> {
        todo!()
    }

    fn dirty(&self) -> bool {
        self.dirty
    }

    fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }
}

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
use egui_glow::{
    glow,
    glow::{HasContext, PixelUnpackData},
};

pub struct EFrameBackendSurface {
    pub ctx: egui::Context,
    pub pixels: Vec<u8>, // Virtual pixel buffer
    pub buffer_texture: Arc<glow::Texture>,
    pub buffer_object: glow::Framebuffer,
    pub buffer_dim: BufferDimensions,
    pub surface_dim: TextureDimensions,
    pub dirty: bool,
}

impl DisplayTargetSurface for EFrameBackendSurface {
    type NativeDevice = glow::Context;
    type NativeQueue = ();
    type NativeTexture = glow::Texture;
    type NativeTextureFormat = ();

    fn buf_dimensions(&self) -> BufferDimensions {
        self.buffer_dim
    }

    fn backing_dimensions(&self) -> TextureDimensions {
        TextureDimensions {
            w: self.buffer_dim.w,
            h: self.buffer_dim.h,
        }
    }

    fn resize_backing(&mut self, gl: Arc<Self::NativeDevice>, new_dim: BufferDimensions) -> Result<(), Error> {
        self.pixels.resize(new_dim.w as usize * new_dim.h as usize * 4, 0);
        self.buffer_dim = new_dim;

        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(*self.buffer_texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                new_dim.w as i32,
                new_dim.h as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                PixelUnpackData::Slice(Some(&self.pixels)),
            );
        }

        // let buffer_image = egui::ColorImage {
        //     size:   [new_dim.w as usize, new_dim.h as usize],
        //     pixels: self
        //         .pixels
        //         .chunks_exact(4)
        //         .map(|rgba| egui::Color32::from_rgba_premultiplied(rgba[0], rgba[1], rgba[2], rgba[3]))
        //         .collect(),
        // };
        //
        //
        // self.buffer = self
        //     .ctx
        //     .load_texture("marty_buffer_texture", buffer_image, egui::TextureOptions::default());

        Ok(())
    }

    fn update_backing(&mut self, gl: Arc<Self::NativeDevice>, _q: Arc<Self::NativeQueue>) -> Result<(), Error> {
        // if !self.dirty {
        //     return Ok(()); // nothing to do
        // }

        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(*self.buffer_texture));

            gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                0,
                0,
                self.buffer_dim.w as i32,
                self.buffer_dim.h as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(&self.pixels)),
            );

            gl.bind_texture(glow::TEXTURE_2D, None);
        }

        self.dirty = false;
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
        //Arc::new(self.buffer.clone())
        self.buffer_texture.clone()
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

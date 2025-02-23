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
*/
use crate::util;
use anyhow::Error;
use display_backend_trait::{BufferDimensions, DisplayTargetSurface, TextureDimensions};
use egui_wgpu::wgpu;
use std::sync::Arc;

pub struct EFrameBackendSurface {
    pub pixel_dimensions: BufferDimensions,
    pub pixels: Vec<u8>,
    pub backing: Arc<wgpu::Texture>,
    pub surface: Arc<wgpu::Texture>,
    pub texture_format: wgpu::TextureFormat,
    pub texture_format_size: f32,
    pub dirty: bool,
}

impl DisplayTargetSurface for EFrameBackendSurface {
    type NativeDevice = wgpu::Device;
    type NativeQueue = wgpu::Queue;
    type NativeTexture = wgpu::Texture;
    type NativeTextureFormat = wgpu::TextureFormat;

    #[inline(always)]
    fn buf_dimensions(&self) -> BufferDimensions {
        self.pixel_dimensions
    }

    #[inline(always)]
    fn backing_dimensions(&self) -> TextureDimensions {
        TextureDimensions {
            w: self.backing.width(),
            h: self.backing.height(),
        }
    }

    fn resize_backing(&mut self, device: Arc<Self::NativeDevice>, new_dim: BufferDimensions) -> Result<(), Error> {
        // There's no actual way to resize the texture, so we just create a new one.
        let (new_texture, new_size) = util::create_texture(device, new_dim.into(), self.texture_format)?;
        self.backing = Arc::new(new_texture);

        // Resize the pixel buffer.
        self.pixels.resize(new_size, 0);
        self.pixel_dimensions = new_dim;

        Ok(())
    }

    fn update_backing(&mut self, _device: Arc<Self::NativeDevice>, queue: Arc<Self::NativeQueue>) -> Result<(), Error> {
        self.dirty = true;
        if self.dirty {
            self.dirty = false;

            let bytes_per_row = (self.pixel_dimensions.w as f32 * self.texture_format_size) as u32;
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture:   &self.backing,
                    mip_level: 0,
                    origin:    wgpu::Origin3d { x: 0, y: 0, z: 0 },
                    aspect:    wgpu::TextureAspect::All,
                },
                &self.pixels,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(self.pixel_dimensions.h),
                },
                wgpu::Extent3d {
                    width: self.pixel_dimensions.w,
                    height: self.pixel_dimensions.h,
                    depth_or_array_layers: 1,
                },
            );
        }

        Ok(())
    }

    #[inline(always)]
    fn surface_dimensions(&self) -> TextureDimensions {
        TextureDimensions {
            w: self.surface.width(),
            h: self.surface.height(),
        }
    }

    fn resize_surface(
        &mut self,
        device: Arc<Self::NativeDevice>,
        _queue: Arc<Self::NativeQueue>,
        new_dim: TextureDimensions,
    ) -> Result<(), Error> {
        // There's no actual way to resize the texture, so we just create a new one.
        let (new_texture, _new_size) = util::create_texture(device, new_dim.into(), self.texture_format)?;
        self.surface = Arc::new(new_texture);
        Ok(())
    }

    #[inline(always)]
    fn buf(&self) -> &[u8] {
        &self.pixels
    }

    #[inline(always)]
    fn buf_mut(&mut self) -> &mut [u8] {
        &mut self.pixels
    }

    #[inline(always)]
    fn backing_texture(&self) -> Arc<Self::NativeTexture> {
        // Texture in wgpu 0.24 is Clone, we could avoid using an Arc.
        self.backing.clone()
    }

    #[inline]
    fn backing_texture_format(&self) -> Self::NativeTextureFormat {
        self.backing_texture().format()
    }

    #[inline(always)]
    fn surface_texture(&self) -> Arc<Self::NativeTexture> {
        self.surface.clone()
    }

    #[inline(always)]
    fn dirty(&self) -> bool {
        self.dirty
    }

    #[inline(always)]
    fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }
}

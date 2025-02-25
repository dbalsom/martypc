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
use anyhow::Error;
use display_backend_trait::{BufferDimensions, DisplayTargetSurface, TextureDimensions};
use std::sync::Arc;

pub struct EFrameBackendSurface {
    pub cpu_buffer: Vec<u8>, // Virtual pixel buffer
    pub buffer: egui::TextureHandle,
    pub buffer_dim: BufferDimensions,
    pub surface_dim: TextureDimensions,
}

impl DisplayTargetSurface for EFrameBackendSurface {
    type NativeDevice = ();
    type NativeQueue = ();
    type NativeTexture = egui::TextureHandle;
    type NativeTextureFormat = ();

    fn buf_dimensions(&self) -> BufferDimensions {
        todo!()
    }

    fn backing_dimensions(&self) -> TextureDimensions {
        todo!()
    }

    fn resize_backing(&mut self, device: Arc<Self::NativeDevice>, new_dim: BufferDimensions) -> Result<(), Error> {
        todo!()
    }

    fn update_backing(&mut self, device: Arc<Self::NativeDevice>, queue: Arc<Self::NativeQueue>) -> Result<(), Error> {
        todo!()
    }

    fn surface_dimensions(&self) -> TextureDimensions {
        todo!()
    }

    fn resize_surface(
        &mut self,
        device: Arc<Self::NativeDevice>,
        queue: Arc<Self::NativeQueue>,
        new_dim: TextureDimensions,
    ) -> Result<(), Error> {
        todo!()
    }

    fn buf(&self) -> &[u8] {
        todo!()
    }

    fn buf_mut(&mut self) -> &mut [u8] {
        todo!()
    }

    fn backing_texture(&self) -> Arc<Self::NativeTexture> {
        todo!()
    }

    fn backing_texture_format(&self) -> Self::NativeTextureFormat {
        todo!()
    }

    fn surface_texture(&self) -> Arc<Self::NativeTexture> {
        todo!()
    }

    fn dirty(&self) -> bool {
        todo!()
    }

    fn set_dirty(&mut self, dirty: bool) {
        todo!()
    }
}

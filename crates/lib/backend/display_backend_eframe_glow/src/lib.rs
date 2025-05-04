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

    display_backend_pixels::lib.rs

    Implements DisplayBackend for the Pixels backend
*/

#[cfg(not(feature = "use_egui_backend"))]
compile_error!("'use_egui_backend' feature is required!");

mod display_window;
mod surface;

use std::sync::{Arc, RwLock};

pub use display_backend_trait::{
    BufferDimensions,
    DisplayBackend,
    DisplayBackendBuilder,
    DynDisplayTargetSurface,
    TextureDimensions,
    //DisplayBackendError
};
pub use surface::EFrameBackendSurface;

use marty_display_common::display_scaler::DisplayScaler;

use anyhow::{anyhow, bail, Error};
use display_backend_trait::DisplayTargetSurface;
use egui;
use egui_glow::{
    glow,
    glow::{HasContext, PixelUnpackData},
};

pub struct EFrameBackend {
    ctx: egui::Context,
    gl:  Arc<glow::Context>,
}

impl EFrameBackend {
    pub fn new(ctx: egui::Context, gl: Arc<glow::Context>) -> Result<EFrameBackend, Error> {
        Ok(EFrameBackend { ctx, gl })
    }
}

pub type EFrameScalerType = Box<
    dyn DisplayScaler<
        glow::Context,
        (),
        glow::Texture,
        NativeContext = glow::Context,
        NativeTexture = glow::Texture,
        NativeTextureView = (),
        NativeEncoder = (),
        NativeRenderPass = (),
    >,
>;

impl DisplayBackend<'_, '_, ()> for EFrameBackend {
    type NativeDevice = glow::Context;
    type NativeQueue = ();
    type NativeTexture = glow::Texture;
    type NativeTextureFormat = ();
    type NativeBackend = ();
    type NativeBackendAdapterInfo = ();

    type NativeScaler = EFrameScalerType;

    fn adapter_info(&self) -> Option<Self::NativeBackendAdapterInfo> {
        None
    }

    fn device(&self) -> Arc<Self::NativeDevice> {
        self.gl.clone()
    }

    fn queue(&self) -> Arc<Self::NativeQueue> {
        Arc::new(())
    }

    fn create_surface(
        &self,
        buffer_dim: BufferDimensions,
        surface_dim: TextureDimensions,
    ) -> Result<DynDisplayTargetSurface, Error> {
        let pixels = vec![0; buffer_dim.w as usize * buffer_dim.h as usize * 4];
        let gl = &self.gl;
        unsafe {
            let tex = gl.create_texture().unwrap();

            gl.bind_texture(glow::TEXTURE_2D, Some(tex));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                buffer_dim.w as i32,
                buffer_dim.h as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                PixelUnpackData::Slice(Some(&pixels)),
            );
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);

            let fbo = gl.create_framebuffer().map_err(|_| anyhow!("Failed to create FBO"))?;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(tex),
                0,
            );

            if gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
                bail!("Framebuffer is not complete");
            }

            gl.bind_framebuffer(glow::FRAMEBUFFER, None);

            Ok(Arc::new(RwLock::new(EFrameBackendSurface {
                ctx: self.ctx.clone(),
                pixels,
                buffer_texture: Arc::new(tex),
                buffer_object: fbo,
                buffer_dim,
                surface_dim,
                dirty: false,
            })))
        }
    }

    fn resize_backing_texture(
        &mut self,
        surface: &mut DynDisplayTargetSurface,
        new_dim: BufferDimensions,
    ) -> Result<(), Error> {
        surface.write().unwrap().resize_backing(self.gl.clone(), new_dim)?;
        Ok(())
    }

    fn resize_surface_texture(
        &mut self,
        surface: &mut DynDisplayTargetSurface,
        new_dim: TextureDimensions,
    ) -> Result<(), Error> {
        //self.pixels.resize_surface(new.w, new.h)?;
        surface
            .write()
            .unwrap()
            .resize_surface(self.gl.clone(), Arc::new(()), new_dim)?;
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

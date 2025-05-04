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

    --------------------------------------------------------------------------
*/

// Reexport trait items
use marty_display_common::display_scaler::{DisplayScaler, ScalerGeometry, ScalerMode, ScalerOption};

pub use marty_frontend_common::color::MartyColor;

#[cfg(feature = "use_egui_backend")]
use egui::TextureHandle;

/// A logical texture size for a window surface.
#[derive(Debug, Default)]
pub struct SurfaceSize {
    pub width:  u32,
    pub height: u32,
}

/// The default renderer that scales your frame to the screen size.
#[derive(Default)]
pub struct MartyScaler {
    mode: ScalerMode,
    bilinear: bool,

    pub texture_size: SurfaceSize,
    pub target_size:  SurfaceSize,
    pub screen_size:  SurfaceSize,
}

impl MartyScaler {
    pub fn new() -> Self {
        Self {
            mode: ScalerMode::Fixed,
            bilinear: true,
            ..Default::default()
        }
    }
}

#[cfg(feature = "use_egui_backend")]
type Texture = TextureHandle;
#[cfg(not(feature = "use_egui_backend"))]
type Texture = ();

impl DisplayScaler<(), (), Texture> for MartyScaler {
    type NativeContext = ();
    type NativeRenderPass = ();
    type NativeTexture = ();
    type NativeTextureView = ();
    type NativeEncoder = ();

    fn render(&self, _encoder: &mut (), _render_target: &()) {}
    fn render_with_renderpass(&self, _render_pass: &mut Self::NativeRenderPass) {}

    fn resize(
        &mut self,
        _device: &(),
        _queue: &(),
        _texture: &Texture,
        texture_width: u32,
        texture_height: u32,
        target_width: u32,
        target_height: u32,
        screen_width: u32,
        screen_height: u32,
    ) {
        self.texture_size = SurfaceSize {
            width:  texture_width,
            height: texture_height,
        };
        self.target_size = SurfaceSize {
            width:  target_width,
            height: target_height,
        };
        self.screen_size = SurfaceSize {
            width:  screen_width,
            height: screen_height,
        };
    }

    fn resize_surface(&mut self, _device: &(), _queue: &(), _texture: &Texture, screen_width: u32, screen_height: u32) {
        self.screen_size = SurfaceSize {
            width:  screen_width,
            height: screen_height,
        };
    }

    fn mode(&self) -> ScalerMode {
        self.mode
    }

    fn set_mode(&mut self, _device: &(), _queue: &(), _new_mode: ScalerMode) {}

    fn geometry(&self) -> ScalerGeometry {
        ScalerGeometry {
            texture_w: self.texture_size.width,
            texture_h: self.texture_size.height,
            target_w:  self.target_size.width,
            target_h:  self.target_size.height,
            surface_w: self.target_size.width,
            surface_h: self.target_size.height,
        }
    }

    fn set_margins(&mut self, _l: u32, _r: u32, _t: u32, _b: u32) {}

    fn set_bilinear(&mut self, bilinear: bool) {
        self.bilinear = bilinear
    }

    fn set_fill_color(&mut self, _fill: MartyColor) {}

    /// Apply a ScalerOption. Update of uniform buffers is controlled by the 'update' boolean. If
    /// it is true we will perform an immediate uniform update; if false it will be delayed and
    /// set_option() will return true to indicate that the caller should perform an update.
    fn set_option(&mut self, _device: &(), _queue: &(), _opt: ScalerOption, _update: bool) -> bool {
        false
    }

    /// Iterate though a vector of ScalerOptions and apply them all. We can defer uniform update
    /// until all options have been processed.
    fn set_options(&mut self, _device: &(), _queue: &(), _opts: Vec<ScalerOption>) {}
}

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

    frontend_common::display_scaler::lib.rs

    Definition of the DisplayScaler trait

*/
use marty_frontend_common::color::MartyColor;
use marty_videocard_renderer::RendererConfigParams;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Deserialize)]
pub enum ScalerMode {
    Null,
    Fixed,
    Integer,
    Fit,
    Stretch,
    Windowed,
}

// This array is intended to represent modes to be displayed to the user. Since Null is an
// internal mode, we don't include it.
pub const SCALER_MODES: [ScalerMode; 4] = [
    ScalerMode::Fixed,
    ScalerMode::Integer,
    ScalerMode::Fit,
    ScalerMode::Stretch,
];

#[derive(Copy, Clone, Debug, Eq, PartialEq, Deserialize)]

pub enum ScalerFilter {
    Nearest,
    Linear,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ScanlineMode {
    Square,
    Sin,
}

pub enum ScalerEffect {
    None,
    Crt {
        h_curvature: f32,
        v_curvature: f32,
        corner_radius: f32,
        option: ScanlineMode,
    },
}

pub enum ScalerOption {
    Mode(ScalerMode),
    Adjustment { h: f32, s: f32, b: f32, c: f32, g: f32 },
    Margins { l: u32, r: u32, t: u32, b: u32 },
    Filtering(ScalerFilter),
    FillColor { r: u8, g: u8, b: u8, a: u8 },
    Mono { enabled: bool, r: f32, g: f32, b: f32, a: f32 },
    Geometry { h_curvature: f32, v_curvature: f32, corner_radius: f32 },
    Scanlines { enabled: Option<bool>, lines: Option<u32>, intensity: Option<f32> },
    Effect(ScalerEffect),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Deserialize)]
pub enum PhosphorType {
    Color,
    White,
    Green,
    Amber,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ScalerPreset {
    pub name: String,
    pub mode: Option<ScalerMode>,
    pub border_color: Option<u32>,
    // Fields below should be identical to ScalerParams
    pub filter: ScalerFilter,
    pub crt_effect: bool,
    pub crt_barrel_distortion: f32,
    pub crt_corner_radius: f32,
    pub crt_scanlines: bool,
    pub crt_phosphor_type: PhosphorType,
    pub gamma: f32,
    // Options for associated renderer
    pub renderer: RendererConfigParams,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct ScalerGeometry {
    pub texture_w: u32,
    pub texture_h: u32,
    pub target_w:  u32,
    pub target_h:  u32,
    pub surface_w: u32,
    pub surface_h: u32,
}

#[derive(Copy, Clone, Debug)]
pub struct ScalerParams {
    pub filter: ScalerFilter,
    pub crt_effect: bool,
    pub crt_barrel_distortion: f32,
    pub crt_corner_radius: f32,
    pub crt_scanlines: bool,
    pub crt_phosphor_type: PhosphorType,
    pub gamma: f32,
}

impl From<ScalerPreset> for ScalerParams {
    fn from(value: ScalerPreset) -> Self {
        Self {
            filter: value.filter,
            crt_effect: value.crt_effect,
            crt_barrel_distortion: value.crt_barrel_distortion,
            crt_scanlines: value.crt_scanlines,
            crt_phosphor_type: value.crt_phosphor_type,
            crt_corner_radius: value.crt_corner_radius,
            gamma: value.gamma,
        }
    }
}

impl Default for ScalerParams {
    fn default() -> Self {
        Self {
            filter: ScalerFilter::Linear,
            crt_effect: true,
            crt_barrel_distortion: 0.0,
            crt_corner_radius: 0.0,
            crt_scanlines: false,
            crt_phosphor_type: PhosphorType::Color,
            gamma: 1.0,
        }
    }
}

impl Default for ScalerMode {
    fn default() -> Self {
        ScalerMode::Integer
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub trait ThreadSafe: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait ThreadSafe {}

#[cfg(not(target_arch = "wasm32"))]
impl<T> ThreadSafe for T where T: Send + Sync {} // Implement it for all Send + Sync types

#[cfg(target_arch = "wasm32")]
impl<T> ThreadSafe for T where T: Sized {} // Implement it for all types on WASM

pub trait DisplayScaler<D, Q, T>: Send + Sync {
    type NativeContext;
    type NativeRenderPass;

    type NativeTexture;
    type NativeTextureView;
    type NativeEncoder;

    //fn texture_view(&self) -> &Self::NativeTextureView;
    fn render(&self, encoder: &mut Self::NativeEncoder, render_target: &Self::NativeTextureView);

    fn render_with_context(&self, _context: &Self::NativeContext, _texture: Arc<Self::NativeTexture>) {
        // Default implementation does nothing
    }
    fn render_with_renderpass(&self, render_pass: &mut Self::NativeRenderPass);
    fn resize(
        &mut self,
        device: &D,
        queue: &Q,
        texture: &T,
        texture_width: u32,  // Actual width, in pixels, of source texture
        texture_height: u32, // Actual height, in pixels, of source texture
        target_width: u32,   // Width, in pixels, of destination texture (stretch to fit)
        target_height: u32,  // Height, in pixels, of destination texture (stretch to fit)
        screen_width: u32,   // Width, in pixels, of destination surface
        screen_height: u32,  // Height, in pixels, of destination surface
    );
    fn resize_surface(
        &mut self,
        device: &D,
        queue: &Q,
        texture: &T,
        screen_width: u32,  // Width, in pixels, of destination surface
        screen_height: u32, // Height, in pixels, of destination surface
    );

    fn mode(&self) -> ScalerMode;
    fn set_mode(&mut self, device: &D, queue: &Q, new_mode: ScalerMode);

    fn geometry(&self) -> ScalerGeometry;
    fn set_margins(&mut self, l: u32, r: u32, t: u32, b: u32);
    fn set_bilinear(&mut self, bilinear: bool);
    fn set_fill_color(&mut self, fill: MartyColor);
    fn set_option(&mut self, device: &D, queue: &Q, opt: ScalerOption, update: bool) -> bool;
    fn set_options(&mut self, device: &D, queue: &Q, opts: Vec<ScalerOption>);
}

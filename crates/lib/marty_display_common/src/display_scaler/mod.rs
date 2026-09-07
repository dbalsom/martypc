/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

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

//! # Display Scaler
//!
//! Common display scaler functionality and main DisplayScaler trait definition.

use marty_frontend_common::color::MartyColor;
pub use marty_frontend_common::types::window::ScalerMode;
use marty_videocard_renderer::RendererConfigParams;
use serde::Deserialize;
use std::sync::Arc;

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
    Adjustment {
        h: f32,
        s: f32,
        b: f32,
        c: f32,
        g: f32,
    },
    Margins {
        l: u32,
        r: u32,
        t: u32,
        b: u32,
    },
    Filtering(ScalerFilter),
    FillColor {
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    },
    Mono {
        enabled: bool,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
    },
    Geometry {
        h_curvature:   f32,
        v_curvature:   f32,
        corner_radius: f32,
    },
    // `lines` is the number of desired CRT scanline bands. For doubled render textures this is
    // typically half the backing/render texture height.
    Scanlines {
        enabled: Option<bool>,
        lines: Option<u32>,
        intensity: Option<f32>,
    },
    CrtcFrameParity {
        enabled: bool,
        parity:  u32,
    },
    InterlaceSupport(bool),
    /// Normalized CRT power-off animation progress. Zero is normal display output; one is black.
    PowerOff {
        progress: f32,
    },
    Effect(ScalerEffect),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Deserialize)]
pub enum PhosphorType {
    Color,
    White,
    Green,
    Amber,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize)]
pub struct ScalerPreset {
    pub name: String,
    /// Legacy default-mode setting. New configurations define mode per window; this field is
    /// consulted only on the preset named `default` when a window omits `scaler_mode`.
    pub mode: Option<ScalerMode>,
    pub border_color: Option<u32>,
    // Fields below should be identical to ScalerParams
    pub filter: ScalerFilter,
    pub crt_effect: bool,
    pub crt_barrel_distortion: f32,
    pub crt_corner_radius: f32,
    pub crt_scanlines: bool,
    #[serde(default = "default_true")]
    pub interlace_support: bool,
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

/// Map the scaler UV at a physical surface position to the source UV sampled by the CRT shader.
///
/// This intentionally mirrors `apply_crt_curvature()` in the WGPU and Glow scaler shaders.
pub fn apply_crt_curvature(uv: (f32, f32), h_curvature: f32, v_curvature: f32) -> Option<(f32, f32)> {
    let mapped_x = uv.0 * 2.0 - 1.0;
    let mapped_y = uv.1 * 2.0 - 1.0;
    let radius_squared = mapped_x * mapped_x + mapped_y * mapped_y;
    let curvature = (h_curvature + v_curvature) * 0.1;
    let distortion = 1.0 - radius_squared * curvature;

    if !distortion.is_finite() || distortion.abs() <= f32::EPSILON {
        return None;
    }

    let texture_x = (mapped_x / distortion) * 0.5 + 0.5;
    let texture_y = (mapped_y / distortion) * 0.5 + 0.5;
    const EDGE_EPSILON: f32 = 0.000_01;

    (texture_x.is_finite()
        && texture_y.is_finite()
        && texture_x >= -EDGE_EPSILON
        && texture_x <= 1.0 + EDGE_EPSILON
        && texture_y >= -EDGE_EPSILON
        && texture_y <= 1.0 + EDGE_EPSILON)
        .then_some((texture_x.clamp(0.0, 1.0), texture_y.clamp(0.0, 1.0)))
}

#[derive(Copy, Clone, Debug)]
pub struct ScalerParams {
    pub filter: ScalerFilter,
    pub crt_effect: bool,
    pub crt_barrel_distortion: f32,
    pub crt_corner_radius: f32,
    pub crt_scanlines: bool,
    pub interlace_support: bool,
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
            interlace_support: value.interlace_support,
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
            interlace_support: true,
            crt_phosphor_type: PhosphorType::Color,
            gamma: 1.0,
        }
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

pub trait DisplayScaler<D, Q, T>: ThreadSafe {
    type NativeContext;
    type NativeRenderPass;

    type NativeTexture;
    type NativeTextureView;
    type NativeEncoder;

    //fn texture_view(&self) -> &Self::NativeTextureView;
    fn render(&self, encoder: &mut Self::NativeEncoder, render_target: &Self::NativeTextureView);

    /// Render to the supplied target after clearing it to an explicit color.
    ///
    /// Backends that do not support encoder-based rendering may retain the default implementation.
    fn render_with_clear_color(
        &self,
        encoder: &mut Self::NativeEncoder,
        render_target: &Self::NativeTextureView,
        _clear_color: MartyColor,
    ) {
        self.render(encoder, render_target);
    }

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
    fn surface_to_texture(&self, surface_x: f32, surface_y: f32) -> Option<(f32, f32)>;
    fn set_margins(&mut self, l: u32, r: u32, t: u32, b: u32);
    fn set_bilinear(&mut self, bilinear: bool);
    fn set_fill_color(&mut self, fill: MartyColor);
    fn set_option(&mut self, device: &D, queue: &Q, opt: ScalerOption, update: bool) -> bool;
    fn set_options(&mut self, device: &D, queue: &Q, opts: Vec<ScalerOption>);
}

#[cfg(test)]
mod tests {
    use super::apply_crt_curvature;

    fn assert_point_close(actual: (f32, f32), expected: (f32, f32)) {
        assert!(
            (actual.0 - expected.0).abs() < 0.000_01,
            "x: {actual:?} != {expected:?}"
        );
        assert!(
            (actual.1 - expected.1).abs() < 0.000_01,
            "y: {actual:?} != {expected:?}"
        );
    }

    #[test]
    fn zero_crt_curvature_is_identity() {
        for point in [(0.0, 0.0), (0.25, 0.75), (0.5, 0.5), (1.0, 1.0)] {
            assert_point_close(apply_crt_curvature(point, 0.0, 0.0).unwrap(), point);
        }
    }

    #[test]
    fn crt_curvature_leaves_center_fixed_and_matches_shader_equation() {
        assert_point_close(apply_crt_curvature((0.5, 0.5), 0.2, 0.2).unwrap(), (0.5, 0.5));
        assert_point_close(apply_crt_curvature((0.75, 0.5), 0.2, 0.2).unwrap(), (0.752_525_27, 0.5));
    }

    #[test]
    fn crt_curvature_is_symmetric() {
        let left = apply_crt_curvature((0.25, 0.35), 0.15, 0.15).unwrap();
        let right = apply_crt_curvature((0.75, 0.65), 0.15, 0.15).unwrap();
        assert_point_close((left.0 + right.0, left.1 + right.1), (1.0, 1.0));
    }

    #[test]
    fn crt_curvature_rejects_shader_coordinates_outside_texture() {
        assert!(apply_crt_curvature((0.0, 0.0), 1.0, 1.0).is_none());
        assert!(apply_crt_curvature((1.0, 1.0), 1.0, 1.0).is_none());
    }
}

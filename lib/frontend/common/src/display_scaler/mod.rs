/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

    frontend_libs::display_scaler::lib.rs

    Definition of the DisplayScaler trait

*/

use serde::Deserialize;
pub use wgpu::Color;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Deserialize)]
pub enum ScalerMode {
    Null,
    None,
    Integer,
    Fit,
    Stretch,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]

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
    Scanlines { enabled: bool, lines: u32, intensity: f32 },
    Effect(ScalerEffect),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PhosphorType {
    Color,
    White,
    Green,
    Amber,
}

#[derive(Copy, Clone, Debug)]
pub struct ScalerParams {
    pub filter: ScalerFilter,
    pub crt_effect: bool,
    pub crt_hcurvature: f32,
    pub crt_vcurvature: f32,
    pub crt_cornerradius: f32,
    pub crt_scanlines: bool,
    pub crt_phosphor_type: PhosphorType,
    pub gamma: f32,
}

impl Default for ScalerParams {
    fn default() -> Self {
        Self {
            filter: ScalerFilter::Linear,
            crt_effect: false,
            crt_hcurvature: 0.0,
            crt_vcurvature: 0.0,
            crt_cornerradius: 0.0,
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

pub trait HasWgpu<Q> {
    fn queue(&self) -> &Q;
}

pub trait DisplayScaler<B>: Send + Sync {
    type NativeTextureView;
    type NativeEncoder;

    fn get_texture_view(&self) -> &Self::NativeTextureView;
    fn render(&self, encoder: &mut Self::NativeEncoder, render_target: &Self::NativeTextureView);
    fn resize(
        &mut self,
        pixels: &B,
        texture_width: u32,  // Actual width, in pixels, of source texture
        texture_height: u32, // Actual height, in pixels, of source texture
        target_width: u32,   // Width, in pixels, of destination texture (stretch to fit)
        target_height: u32,  // Height, in pixels, of destination texture (stretch to fit)
        screen_width: u32,   // Width, in pixels, of destination surface
        screen_height: u32,  // Height, in pixels, of destination surface
    );
    fn resize_surface(
        &mut self,
        pixels: &B,
        screen_width: u32,  // Width, in pixels, of destination surface
        screen_height: u32, // Height, in pixels, of destination surface
    );

    fn set_mode(&mut self, pixels: &B, new_mode: ScalerMode);
    fn set_margins(&mut self, l: u32, r: u32, t: u32, b: u32);
    fn set_bilinear(&mut self, bilinear: bool);
    fn set_fill_color(&mut self, fill: wgpu::Color);
    fn set_option(&mut self, pixels: &B, opt: ScalerOption, update: bool) -> bool;
    fn set_options(&mut self, pixels: &B, opts: Vec<ScalerOption>);
}

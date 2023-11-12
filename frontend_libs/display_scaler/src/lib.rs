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

pub use wgpu::Color;

#[derive (Copy, Clone, Debug, Eq, PartialEq)]
pub enum ScalerMode {
    None,
    Integer,
    Fit,
    Stretch
}

pub enum ScalerFilter {
    Nearest,
    Linear
}

pub enum ScanlineMode {
    Square,
    Sin
}

pub enum ScalerEffect {
    None,
    Crt{h_curvature: f32, v_curvature: f32, corner_radius: f32, option: ScanlineMode},
}
pub enum ScalerOption {
    Mode(ScalerMode),
    Margins{l: u32, r: u32, t: u32, b: u32},
    Filtering(ScalerFilter),
    FillColor{r: u8, g: u8, b: u8, a: u8},
    Effect(ScalerEffect),
}

impl Default for ScalerMode {
    fn default() -> Self {
        ScalerMode::Integer
    }
}

pub trait DisplayScaler: Send + Sync {
    fn get_texture_view(&self) -> &wgpu::TextureView;
    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        bilinear: bool
    );
    fn resize(
        &mut self,
        pixels: &pixels::Pixels,
        texture_width: u32,
        texture_height: u32,
        screen_width: u32,
        screen_height: u32,
    );
    fn set_mode(&mut self, pixels: &pixels::Pixels, new_mode: ScalerMode);
    fn set_margins(&mut self, l: u32, r: u32, t: u32, b: u32);
    fn set_bilinear(&mut self, bilinear: bool);
    fn set_fill_color(&mut self, fill: wgpu::Color);
    fn set_option(&mut self, pixels: &pixels::Pixels, opt: ScalerOption);
    fn set_options(&mut self, pixels: &pixels::Pixels, opts: Vec<ScalerOption>);
}
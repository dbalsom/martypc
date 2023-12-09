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

    frontend_common::color::lib.rs

    Defines tne MartyColor utility type. By controlling the feature flags to
    this library crate, you can enable conversion of colors to and from various
    implementation-defined types, like egui and wgpu Colors.

*/

/// Define a universal color type that can be converted to and from implementation-defined types
/// and other common color formats.
#[derive(Debug)]
pub struct MartyColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for MartyColor {
    fn default() -> Self {
        MartyColor {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        }
    }
}
/// Convert a MartyColor to an array of f32. This method is used for sending colors to a shader
/// via uniform buffers.
impl From<MartyColor> for [f32; 4] {
    fn from(color: MartyColor) -> Self {
        [color.r, color.g, color.b, color.a]
    }
}

/// Convert a u32 value to a MartyColor.
/// Implementing From<u32> also provides Into<u32>.
impl From<u32> for MartyColor {
    fn from(rgba: u32) -> Self {
        // Should we implement 32 bit or 24 bit conversion?
        let r = ((rgba >> 24) & 0xff) as f32 / 255.0;
        let g = ((rgba >> 16) & 0xff) as f32 / 255.0;
        let b = ((rgba >> 8) & 0xff) as f32 / 255.0;
        let a = (rgba & 0xff) as f32 / 255.0;

        /*
        let r = ((rgba >> 16) & 0xff) as f32 / 255.0;
        let g = ((rgba >> 8) & 0xff) as f32  / 255.0;
        let b = (rgba & 0xff) as f32 / 255.0;
        let a = 1.0;
         */
        MartyColor { r, g, b, a }
    }
}

impl MartyColor {
    pub fn from_u24(rgb: u32) -> Self {
        let r = ((rgb >> 16) & 0xff) as f32 / 255.0;
        let g = ((rgb >> 8) & 0xff) as f32 / 255.0;
        let b = (rgb & 0xff) as f32 / 255.0;
        let a = 1.0;
        MartyColor { r, g, b, a }
    }
}

#[cfg(feature = "use_wgpu")]
/// Convert a wgpu::Color to MartyColor.
/// Implementing From<wgpu::Color> also provides Into<wgpu::Color>.
impl From<wgpu::Color> for MartyColor {
    fn from(color: wgpu::Color) -> MartyColor {
        MartyColor {
            r: color.r as f32,
            g: color.g as f32,
            b: color.b as f32,
            a: color.a as f32,
        }
    }
}

#[cfg(feature = "use_wgpu")]
/// Color conversions for wgpu::Color.
impl MartyColor {
    pub fn to_wgpu_color(&self) -> wgpu::Color {
        wgpu::Color {
            r: self.r as f64,
            g: self.g as f64,
            b: self.b as f64,
            a: self.a as f64,
        }
    }

    /// Convert a color (assumed to be in linear RGBA) to sRGB.
    pub fn to_wgpu_color_srgb(&self) -> wgpu::Color {
        fn convert_component(comp: f64) -> f64 {
            if comp <= 0.0031308 {
                12.92 * comp
            }
            else {
                1.055 * comp.powf(1.0 / 2.4) - 0.055
            }
        }

        wgpu::Color {
            r: convert_component(self.r as f64),
            g: convert_component(self.g as f64),
            b: convert_component(self.b as f64),
            a: self.a as f64,
        }
    }

    /// Convert a color (assumed to be in sRGB) to linear RGB
    pub fn to_wgpu_color_linear(&self) -> wgpu::Color {
        fn convert_component(comp: f64) -> f64 {
            if comp <= 0.04045 {
                comp / 12.92
            }
            else {
                ((comp + 0.055) / 1.055).powf(2.4)
            }
        }

        wgpu::Color {
            r: convert_component(self.r as f64),
            g: convert_component(self.g as f64),
            b: convert_component(self.b as f64),
            a: self.a as f64,
        }
    }
}

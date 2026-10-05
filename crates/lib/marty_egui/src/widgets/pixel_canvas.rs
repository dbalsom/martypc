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

    --------------------------------------------------------------------------

    marty_egui::widgets::pixel_canvas.rs

    A resizable texture widget backed by a pixel buffer.

*/
use crate::glyphs::FontInfo;
use egui::{Color32, ColorImage, Context, ImageData, Rect, ScrollArea, TextureHandle, TextureOptions};
use std::path::Path;

use anyhow::Error;
use marty_frontend_common::color::cga::CGAColor;
use std::sync::Arc;
use strum_macros::EnumIter;

pub const GRAYSCALE_RAMP: [Color32; 256] = {
    let mut palette = [Color32::BLACK; 256];
    let mut i = 0;
    while i < 256 {
        let shade = i as u8;
        palette[i] = Color32::from_rgb(shade, shade, shade);
        i += 1;
    }
    palette
};

pub const GRAYSCALE_RAMP_REVERSED: [Color32; 256] = {
    let mut palette = [Color32::BLACK; 256];
    let mut i = 0;
    while i < 256 {
        palette[i] = GRAYSCALE_RAMP[255 - i];
        i += 1;
    }
    palette
};

pub const DEFAULT_VGA_PALETTE: [Color32; 256] = {
    // Default VGA DAC palette, stored as RGB triplets with 6 bits per component.
    const RGB6: [u8; 256 * 3] = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x2a, 0x00, 0x2a, 0x00, 0x00, 0x2a, 0x2a, 0x2a, 0x00, 0x00, 0x2a, 0x00, 0x2a,
        0x2a, 0x15, 0x00, 0x2a, 0x2a, 0x2a, 0x15, 0x15, 0x15, 0x15, 0x15, 0x3f, 0x15, 0x3f, 0x15, 0x15, 0x3f, 0x3f,
        0x3f, 0x15, 0x15, 0x3f, 0x15, 0x3f, 0x3f, 0x3f, 0x15, 0x3f, 0x3f, 0x3f, 0x00, 0x00, 0x00, 0x05, 0x05, 0x05,
        0x08, 0x08, 0x08, 0x0b, 0x0b, 0x0b, 0x0e, 0x0e, 0x0e, 0x11, 0x11, 0x11, 0x14, 0x14, 0x14, 0x18, 0x18, 0x18,
        0x1c, 0x1c, 0x1c, 0x20, 0x20, 0x20, 0x24, 0x24, 0x24, 0x28, 0x28, 0x28, 0x2d, 0x2d, 0x2d, 0x32, 0x32, 0x32,
        0x38, 0x38, 0x38, 0x3f, 0x3f, 0x3f, 0x00, 0x00, 0x3f, 0x10, 0x00, 0x3f, 0x1f, 0x00, 0x3f, 0x2f, 0x00, 0x3f,
        0x3f, 0x00, 0x3f, 0x3f, 0x00, 0x2f, 0x3f, 0x00, 0x1f, 0x3f, 0x00, 0x10, 0x3f, 0x00, 0x00, 0x3f, 0x10, 0x00,
        0x3f, 0x1f, 0x00, 0x3f, 0x2f, 0x00, 0x3f, 0x3f, 0x00, 0x2f, 0x3f, 0x00, 0x1f, 0x3f, 0x00, 0x10, 0x3f, 0x00,
        0x00, 0x3f, 0x00, 0x00, 0x3f, 0x10, 0x00, 0x3f, 0x1f, 0x00, 0x3f, 0x2f, 0x00, 0x3f, 0x3f, 0x00, 0x2f, 0x3f,
        0x00, 0x1f, 0x3f, 0x00, 0x10, 0x3f, 0x1f, 0x1f, 0x3f, 0x27, 0x1f, 0x3f, 0x2f, 0x1f, 0x3f, 0x37, 0x1f, 0x3f,
        0x3f, 0x1f, 0x3f, 0x3f, 0x1f, 0x37, 0x3f, 0x1f, 0x2f, 0x3f, 0x1f, 0x27, 0x3f, 0x1f, 0x1f, 0x3f, 0x27, 0x1f,
        0x3f, 0x2f, 0x1f, 0x3f, 0x37, 0x1f, 0x3f, 0x3f, 0x1f, 0x37, 0x3f, 0x1f, 0x2f, 0x3f, 0x1f, 0x27, 0x3f, 0x1f,
        0x1f, 0x3f, 0x1f, 0x1f, 0x3f, 0x27, 0x1f, 0x3f, 0x2f, 0x1f, 0x3f, 0x37, 0x1f, 0x3f, 0x3f, 0x1f, 0x37, 0x3f,
        0x1f, 0x2f, 0x3f, 0x1f, 0x27, 0x3f, 0x2d, 0x2d, 0x3f, 0x31, 0x2d, 0x3f, 0x36, 0x2d, 0x3f, 0x3a, 0x2d, 0x3f,
        0x3f, 0x2d, 0x3f, 0x3f, 0x2d, 0x3a, 0x3f, 0x2d, 0x36, 0x3f, 0x2d, 0x31, 0x3f, 0x2d, 0x2d, 0x3f, 0x31, 0x2d,
        0x3f, 0x36, 0x2d, 0x3f, 0x3a, 0x2d, 0x3f, 0x3f, 0x2d, 0x3a, 0x3f, 0x2d, 0x36, 0x3f, 0x2d, 0x31, 0x3f, 0x2d,
        0x2d, 0x3f, 0x2d, 0x2d, 0x3f, 0x31, 0x2d, 0x3f, 0x36, 0x2d, 0x3f, 0x3a, 0x2d, 0x3f, 0x3f, 0x2d, 0x3a, 0x3f,
        0x2d, 0x36, 0x3f, 0x2d, 0x31, 0x3f, 0x00, 0x00, 0x1c, 0x07, 0x00, 0x1c, 0x0e, 0x00, 0x1c, 0x15, 0x00, 0x1c,
        0x1c, 0x00, 0x1c, 0x1c, 0x00, 0x15, 0x1c, 0x00, 0x0e, 0x1c, 0x00, 0x07, 0x1c, 0x00, 0x00, 0x1c, 0x07, 0x00,
        0x1c, 0x0e, 0x00, 0x1c, 0x15, 0x00, 0x1c, 0x1c, 0x00, 0x15, 0x1c, 0x00, 0x0e, 0x1c, 0x00, 0x07, 0x1c, 0x00,
        0x00, 0x1c, 0x00, 0x00, 0x1c, 0x07, 0x00, 0x1c, 0x0e, 0x00, 0x1c, 0x15, 0x00, 0x1c, 0x1c, 0x00, 0x15, 0x1c,
        0x00, 0x0e, 0x1c, 0x00, 0x07, 0x1c, 0x0e, 0x0e, 0x1c, 0x11, 0x0e, 0x1c, 0x15, 0x0e, 0x1c, 0x18, 0x0e, 0x1c,
        0x1c, 0x0e, 0x1c, 0x1c, 0x0e, 0x18, 0x1c, 0x0e, 0x15, 0x1c, 0x0e, 0x11, 0x1c, 0x0e, 0x0e, 0x1c, 0x11, 0x0e,
        0x1c, 0x15, 0x0e, 0x1c, 0x18, 0x0e, 0x1c, 0x1c, 0x0e, 0x18, 0x1c, 0x0e, 0x15, 0x1c, 0x0e, 0x11, 0x1c, 0x0e,
        0x0e, 0x1c, 0x0e, 0x0e, 0x1c, 0x11, 0x0e, 0x1c, 0x15, 0x0e, 0x1c, 0x18, 0x0e, 0x1c, 0x1c, 0x0e, 0x18, 0x1c,
        0x0e, 0x15, 0x1c, 0x0e, 0x11, 0x1c, 0x14, 0x14, 0x1c, 0x16, 0x14, 0x1c, 0x18, 0x14, 0x1c, 0x1a, 0x14, 0x1c,
        0x1c, 0x14, 0x1c, 0x1c, 0x14, 0x1a, 0x1c, 0x14, 0x18, 0x1c, 0x14, 0x16, 0x1c, 0x14, 0x14, 0x1c, 0x16, 0x14,
        0x1c, 0x18, 0x14, 0x1c, 0x1a, 0x14, 0x1c, 0x1c, 0x14, 0x1a, 0x1c, 0x14, 0x18, 0x1c, 0x14, 0x16, 0x1c, 0x14,
        0x14, 0x1c, 0x14, 0x14, 0x1c, 0x16, 0x14, 0x1c, 0x18, 0x14, 0x1c, 0x1a, 0x14, 0x1c, 0x1c, 0x14, 0x1a, 0x1c,
        0x14, 0x18, 0x1c, 0x14, 0x16, 0x1c, 0x00, 0x00, 0x10, 0x04, 0x00, 0x10, 0x08, 0x00, 0x10, 0x0c, 0x00, 0x10,
        0x10, 0x00, 0x10, 0x10, 0x00, 0x0c, 0x10, 0x00, 0x08, 0x10, 0x00, 0x04, 0x10, 0x00, 0x00, 0x10, 0x04, 0x00,
        0x10, 0x08, 0x00, 0x10, 0x0c, 0x00, 0x10, 0x10, 0x00, 0x0c, 0x10, 0x00, 0x08, 0x10, 0x00, 0x04, 0x10, 0x00,
        0x00, 0x10, 0x00, 0x00, 0x10, 0x04, 0x00, 0x10, 0x08, 0x00, 0x10, 0x0c, 0x00, 0x10, 0x10, 0x00, 0x0c, 0x10,
        0x00, 0x08, 0x10, 0x00, 0x04, 0x10, 0x08, 0x08, 0x10, 0x0a, 0x08, 0x10, 0x0c, 0x08, 0x10, 0x0e, 0x08, 0x10,
        0x10, 0x08, 0x10, 0x10, 0x08, 0x0e, 0x10, 0x08, 0x0c, 0x10, 0x08, 0x0a, 0x10, 0x08, 0x08, 0x10, 0x0a, 0x08,
        0x10, 0x0c, 0x08, 0x10, 0x0e, 0x08, 0x10, 0x10, 0x08, 0x0e, 0x10, 0x08, 0x0c, 0x10, 0x08, 0x0a, 0x10, 0x08,
        0x08, 0x10, 0x08, 0x08, 0x10, 0x0a, 0x08, 0x10, 0x0c, 0x08, 0x10, 0x0e, 0x08, 0x10, 0x10, 0x08, 0x0e, 0x10,
        0x08, 0x0c, 0x10, 0x08, 0x0a, 0x10, 0x0b, 0x0b, 0x10, 0x0c, 0x0b, 0x10, 0x0d, 0x0b, 0x10, 0x0f, 0x0b, 0x10,
        0x10, 0x0b, 0x10, 0x10, 0x0b, 0x0f, 0x10, 0x0b, 0x0d, 0x10, 0x0b, 0x0c, 0x10, 0x0b, 0x0b, 0x10, 0x0c, 0x0b,
        0x10, 0x0d, 0x0b, 0x10, 0x0f, 0x0b, 0x10, 0x10, 0x0b, 0x0f, 0x10, 0x0b, 0x0d, 0x10, 0x0b, 0x0c, 0x10, 0x0b,
        0x0b, 0x10, 0x0b, 0x0b, 0x10, 0x0c, 0x0b, 0x10, 0x0d, 0x0b, 0x10, 0x0f, 0x0b, 0x10, 0x10, 0x0b, 0x0f, 0x10,
        0x0b, 0x0d, 0x10, 0x0b, 0x0c, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let mut palette = [Color32::BLACK; 256];
    let mut i = 0;
    while i < 256 {
        // Match the VGA device's DAC conversion, mapping 0..63 to 0..255.
        let r = (RGB6[i * 3] as u16 * 255 / 63) as u8;
        let g = (RGB6[i * 3 + 1] as u16 * 255 / 63) as u8;
        let b = (RGB6[i * 3 + 2] as u16 * 255 / 63) as u8;
        palette[i] = Color32::from_rgb(r, g, b);
        i += 1;
    }
    palette
};

pub const DEFAULT_WIDTH: u32 = 128;
pub const DEFAULT_HEIGHT: u32 = 128;

//pub const PALETTE_1BPP: [Color32; 2] = [Color32::from_rgb(0, 0, 0), Color32::from_rgb(255, 255, 255)];
pub const PALETTE_4BPP: [Color32; 16] = [
    Color32::from_rgb(0x00u8, 0x00u8, 0x00u8),
    Color32::from_rgb(0x00u8, 0x00u8, 0xAAu8),
    Color32::from_rgb(0x00u8, 0xAAu8, 0x00u8),
    Color32::from_rgb(0x00u8, 0xAAu8, 0xAAu8),
    Color32::from_rgb(0xAAu8, 0x00u8, 0x00u8),
    Color32::from_rgb(0xAAu8, 0x00u8, 0xAAu8),
    Color32::from_rgb(0xAAu8, 0x55u8, 0x00u8),
    Color32::from_rgb(0xAAu8, 0xAAu8, 0xAAu8),
    Color32::from_rgb(0x55u8, 0x55u8, 0x55u8),
    Color32::from_rgb(0x55u8, 0x55u8, 0xFFu8),
    Color32::from_rgb(0x55u8, 0xFFu8, 0x55u8),
    Color32::from_rgb(0x55u8, 0xFFu8, 0xFFu8),
    Color32::from_rgb(0xFFu8, 0x55u8, 0x55u8),
    Color32::from_rgb(0xFFu8, 0x55u8, 0xFFu8),
    Color32::from_rgb(0xFFu8, 0xFFu8, 0x55u8),
    Color32::from_rgb(0xFFu8, 0xFFu8, 0xFFu8),
];

#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub enum PixelCanvasDepth {
    Text,
    #[default]
    OneBpp,
    TwoBpp,
    FourBpp,
    EightBpp,
    Rgb,
    Rgba,
}

impl PixelCanvasDepth {
    pub fn bits(&self) -> usize {
        match self {
            PixelCanvasDepth::Text => 16,
            PixelCanvasDepth::OneBpp => 1,
            PixelCanvasDepth::TwoBpp => 2,
            PixelCanvasDepth::FourBpp => 4,
            PixelCanvasDepth::EightBpp => 8,
            PixelCanvasDepth::Rgb => 24,
            PixelCanvasDepth::Rgba => 32,
        }
    }
}

#[derive(EnumIter, Copy, Clone, PartialEq, Eq, Default, Debug)]
pub enum CgaPalette {
    Palette0Low,
    Palette0High,
    Palette1Low,
    #[default]
    Palette1High,
    Palette2Low,
    Palette2High,
}

impl std::fmt::Display for CgaPalette {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Palette0Low => "CGA 0: Green / Red / Brown (low)",
            Self::Palette0High => "CGA 0: Green / Red / Yellow (high)",
            Self::Palette1Low => "CGA 1: Cyan / Magenta / White (low)",
            Self::Palette1High => "CGA 1: Cyan / Magenta / White (high)",
            Self::Palette2Low => "CGA 2: Cyan / Red / White (low)",
            Self::Palette2High => "CGA 2: Cyan / Red / White (high)",
        })
    }
}

impl CgaPalette {
    pub fn colors(self) -> [Color32; 4] {
        // Palette 2 is the alternate RGB palette used by CGA mode 5.
        // Keep color zero black in both intensity variants.
        let indices = match self {
            Self::Palette0Low => [0, 2, 4, 6],
            Self::Palette0High => [0, 10, 12, 14],
            Self::Palette1Low => [0, 3, 5, 7],
            Self::Palette1High => [0, 11, 13, 15],
            Self::Palette2Low => [0, 3, 4, 7],
            Self::Palette2High => [0, 11, 12, 15],
        };
        indices.map(|index| PALETTE_4BPP[index])
    }
}

#[derive(EnumIter, Copy, Clone, PartialEq, Eq, Default, Debug)]
pub enum VgaPalette {
    #[default]
    Device,
    DefaultVga,
    Grayscale,
    GrayscaleReversed,
}

impl std::fmt::Display for VgaPalette {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Device => "Device Palette",
            Self::DefaultVga => "Default VGA",
            Self::Grayscale => "Grayscale (black to white)",
            Self::GrayscaleReversed => "Grayscale (white to black)",
        })
    }
}

struct TextLayout {
    glyph_w: usize,
    glyph_h: usize,
    cols:    usize,
    rows:    usize,
}

pub struct PixelCanvas {
    data_buf: Vec<u8>,
    backing_buf: Vec<Color32>,
    view_dimensions: (u32, u32),
    zoom: f32,
    bpp: PixelCanvasDepth,
    device_palette: Vec<Color32>,
    cga_palette: CgaPalette,
    vga_palette: VgaPalette,
    old_texture: Option<TextureHandle>,
    texture: Option<TextureHandle>,
    image_data: ImageData,
    texture_opts: TextureOptions,
    default_uv: Rect,
    ctx: Context,
    data_unpacked: bool,
}

impl Default for PixelCanvas {
    fn default() -> Self {
        Self::new((DEFAULT_WIDTH, DEFAULT_HEIGHT), Context::default())
    }
}

impl PixelCanvas {
    pub fn new(dims: (u32, u32), ctx: Context) -> Self {
        let pixel_count = Self::pixel_count(dims);
        Self {
            data_buf: vec![0; Self::calc_slice_size(dims, PixelCanvasDepth::OneBpp, None)],
            backing_buf: vec![Color32::BLACK; pixel_count],
            view_dimensions: dims,
            zoom: 1.0,
            bpp: PixelCanvasDepth::OneBpp,
            device_palette: vec![Color32::BLACK, Color32::WHITE],
            cga_palette: CgaPalette::default(),
            vga_palette: VgaPalette::default(),
            old_texture: None,
            texture: None,
            image_data: Self::create_default_imagedata(dims),
            texture_opts: TextureOptions {
                magnification: egui::TextureFilter::Nearest,
                minification: egui::TextureFilter::Nearest,
                mipmap_mode: Some(egui::TextureFilter::Nearest),
                wrap_mode: egui::TextureWrapMode::ClampToEdge,
            },
            default_uv: Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            ctx,
            data_unpacked: false,
        }
    }

    pub fn create_default_colorimage(dims: (u32, u32)) -> ColorImage {
        Self::pixel_count(dims);
        ColorImage::filled([dims.0 as usize, dims.1 as usize], Color32::BLACK)
    }

    pub fn create_default_imagedata(dims: (u32, u32)) -> ImageData {
        ImageData::Color(Arc::new(PixelCanvas::create_default_colorimage(dims)))
    }

    pub fn update_imagedata(&mut self) {
        if !self.data_unpacked {
            log::warn!("PixelCanvas::update_imagedata(): Data not unpacked.");
        }
        let color_image = ColorImage {
            size: [self.view_dimensions.0 as usize, self.view_dimensions.1 as usize],
            source_size: egui::vec2(self.view_dimensions.0 as f32, self.view_dimensions.1 as f32),
            pixels: self.backing_buf.clone(),
        };
        self.image_data = ImageData::Color(Arc::new(color_image));
    }

    // Sanity-check pixel count given dimensions
    fn pixel_count(dims: (u32, u32)) -> usize {
        let count = (dims.0 as usize)
            .checked_mul(dims.1 as usize)
            .expect("PixelCanvas dimensions overflow usize");
        assert!(
            count <= isize::MAX as usize / size_of::<Color32>(),
            "PixelCanvas backing buffer exceeds the maximum allocation size"
        );
        count
    }

    fn text_layout(dims: (u32, u32), font: Option<&FontInfo>) -> Option<TextLayout> {
        let font = font?;
        let glyph_h = font.h.min(font.max_scanline);
        // The font stores one byte per glyph scanline, with the leftmost pixel in bit 7.
        if !(1..=8).contains(&font.w) || glyph_h == 0 {
            return None;
        }
        Some(TextLayout {
            glyph_w: font.w as usize,
            glyph_h: glyph_h as usize,
            cols:    (dims.0 / font.w) as usize,
            rows:    (dims.1 / glyph_h) as usize,
        })
    }

    pub fn calc_slice_size(dims: (u32, u32), bpp: PixelCanvasDepth, font: Option<&FontInfo>) -> usize {
        let pixels = Self::pixel_count(dims);
        match bpp {
            PixelCanvasDepth::Text => Self::text_layout(dims, font).map_or(0, |layout| {
                layout
                    .cols
                    .checked_mul(layout.rows)
                    .and_then(|cells| cells.checked_mul(2))
                    .expect("PixelCanvas text data size overflow")
            }),
            PixelCanvasDepth::OneBpp => pixels.div_ceil(8),
            PixelCanvasDepth::TwoBpp => pixels.div_ceil(4),
            PixelCanvasDepth::FourBpp => pixels.div_ceil(2),
            PixelCanvasDepth::EightBpp => pixels,
            PixelCanvasDepth::Rgb => pixels.checked_mul(3).expect("PixelCanvas RGB data size overflow"),
            PixelCanvasDepth::Rgba => pixels.checked_mul(4).expect("PixelCanvas RGBA data size overflow"),
        }
    }

    pub fn create_texture(&mut self) -> TextureHandle {
        self.ctx
            .load_texture("pixel_canvas".to_string(), self.image_data.clone(), self.texture_opts)
    }

    pub fn get_width(&self) -> f32 {
        self.view_dimensions.0 as f32 * self.zoom
    }

    pub fn draw(&mut self, ui: &mut egui::Ui) {
        if self.texture.is_none() {
            log::debug!("PixelCanvas::draw(): Creating initial texture...");
            self.texture = Some(self.create_texture());
        }

        let texture_opt_ref = match self.data_unpacked {
            true => self.texture.as_ref(),
            false => self.old_texture.as_ref(),
        };

        if let Some(texture) = texture_opt_ref {
            ui.vertical(|ui| {
                // Draw background rect
                let scroll_area = ScrollArea::vertical().auto_shrink([false; 2]);
                let img_w = self.view_dimensions.0 as f32 * self.zoom;
                let img_h = self.view_dimensions.1 as f32 * self.zoom;
                ui.shrink_width_to_current();
                ui.set_width(img_w);
                ui.set_height(img_h);

                scroll_area.show_viewport(ui, |ui, viewport| {
                    let start_x = viewport.min.x + ui.min_rect().left();
                    let start_y = viewport.min.y + ui.min_rect().top();

                    //log::debug!("Viewport is: {:?} StartX: {} StartY: {}", viewport, start_x, start_y);

                    ui.painter().image(
                        texture.id(),
                        Rect::from_min_max(
                            egui::pos2(start_x, start_y),
                            egui::pos2(start_x + img_w, start_y + img_h),
                        ),
                        self.default_uv,
                        Color32::WHITE,
                    );
                });
            });
        }
        else {
            log::debug!("No texture to draw.");
        }
    }

    pub fn get_required_data_size(&self, font: Option<&FontInfo>) -> usize {
        PixelCanvas::calc_slice_size(self.view_dimensions, self.bpp, font)
    }

    pub fn update_data(&mut self, data: &[u8], font: Option<&FontInfo>) {
        let slice_size = PixelCanvas::calc_slice_size(self.view_dimensions, self.bpp, font);
        self.data_buf.clear();
        self.data_buf.extend_from_slice(&data[..slice_size.min(data.len())]);
        self.data_buf.resize(slice_size, 0);

        self.unpack_pixels(font);
        if self.data_unpacked {
            self.update_texture();
        }
    }

    pub fn update_device_palette(&mut self, palette: Vec<Color32>) {
        if matches!(palette.len(), 2 | 4 | 16 | 256) {
            self.device_palette = palette;
            // Only 8bpp uses the device palette. A depth change needs fresh data first.
            if self.bpp == PixelCanvasDepth::EightBpp && self.vga_palette == VgaPalette::Device && self.data_unpacked {
                self.unpack_pixels(None);
                self.update_texture();
            }
        }
    }

    // pub fn has_texture(&self) -> bool {
    //     self.texture.is_some()
    // }

    pub fn update_texture(&mut self) {
        self.update_imagedata();
        if let Some(texture) = &mut self.texture {
            texture.set(self.image_data.clone(), self.texture_opts);
        }
    }

    pub fn set_bpp(&mut self, bpp: PixelCanvasDepth) {
        self.bpp = bpp;
        self.data_unpacked = false;
    }

    pub fn cga_palette(&self) -> CgaPalette {
        self.cga_palette
    }

    pub fn set_cga_palette(&mut self, palette: CgaPalette) {
        if self.cga_palette == palette {
            return;
        }
        self.cga_palette = palette;
        // Recolor a loaded 2bpp image immediately, including when emulation is paused.
        // After a depth change or resize, wait for data in the new format.
        if self.bpp == PixelCanvasDepth::TwoBpp && self.data_unpacked {
            self.unpack_pixels(None);
            self.update_texture();
        }
    }

    pub fn vga_palette(&self) -> VgaPalette {
        self.vga_palette
    }

    pub fn set_vga_palette(&mut self, palette: VgaPalette) {
        if self.vga_palette == palette {
            return;
        }
        self.vga_palette = palette;
        if self.bpp == PixelCanvasDepth::EightBpp && self.data_unpacked {
            self.unpack_pixels(None);
            self.update_texture();
        }
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom;
    }

    pub fn resize(&mut self, dims: (u32, u32), font: Option<&FontInfo>) {
        let pixel_count = Self::pixel_count(dims);
        let slice_size = Self::calc_slice_size(dims, self.bpp, font);
        let data_buf = vec![0; slice_size];
        let backing_buf = vec![Color32::BLACK; pixel_count];
        let image_data = Self::create_default_imagedata(dims);

        self.view_dimensions = dims;
        self.data_buf = data_buf;
        self.backing_buf = backing_buf;
        self.image_data = image_data;

        self.old_texture = self.texture.take();
        self.texture = Some(self.create_texture());
        self.data_unpacked = false;
    }

    pub fn save_buffer(&mut self, path: &Path) -> Result<(), Error> {
        let byte_slice: &[u8] = bytemuck::cast_slice(&self.backing_buf);
        image::save_buffer(
            path,
            byte_slice,
            self.view_dimensions.0,
            self.view_dimensions.1,
            image::ColorType::Rgba8,
        )?;
        Ok(())
    }

    fn unpack_pixels(&mut self, font: Option<&FontInfo>) {
        self.data_unpacked = false;
        let data = &self.data_buf;
        let read_byte = |index: usize| data.get(index).copied().unwrap_or(0);
        match self.bpp {
            PixelCanvasDepth::Text => {
                let Some(font) = font
                else {
                    return;
                };
                let Some(layout) = Self::text_layout(self.view_dimensions, Some(font))
                else {
                    return;
                };
                let span = self.view_dimensions.0 as usize;
                // Dimensions need not be multiples of the glyph size. Clear the unused edges.
                self.backing_buf.fill(Color32::BLACK);
                for row in 0..layout.rows {
                    for col in 0..layout.cols {
                        let glyph_idx = (row * layout.cols + col) * 2;
                        let char = read_byte(glyph_idx);
                        let attr = read_byte(glyph_idx + 1);
                        let (fg_color, bg_color) = CGAColor::decode_attr(attr);
                        for y in 0..layout.glyph_h {
                            let glyph = y
                                .checked_mul(256)
                                .and_then(|offset| offset.checked_add(char as usize))
                                .and_then(|offset| font.font_data.get(offset))
                                .copied()
                                .unwrap_or(0);
                            for x in 0..layout.glyph_w {
                                let bit = 1 << (7 - x);
                                let color = if glyph & bit != 0 { fg_color } else { bg_color };
                                let idx = (row * layout.glyph_h + y) * span + col * layout.glyph_w + x;
                                let rgba = color.to_rgba();
                                self.backing_buf[idx] = Color32::from_rgb(rgba[0], rgba[1], rgba[2]);
                            }
                        }
                    }
                }
            }
            PixelCanvasDepth::OneBpp => {
                for (i, pixel) in self.backing_buf.iter_mut().enumerate() {
                    let byte = read_byte(i / 8);
                    let shift = i % 8;
                    let bit = 1 << (7 - shift);
                    *pixel = if byte & bit != 0 {
                        Color32::WHITE
                    }
                    else {
                        Color32::BLACK
                    };
                }
            }
            PixelCanvasDepth::TwoBpp => {
                let palette = self.cga_palette.colors();
                for (i, pixel) in self.backing_buf.iter_mut().enumerate() {
                    let byte = read_byte(i / 4);
                    let shift = (i % 4) * 2;
                    let color = (byte >> (6 - shift)) & 0x03;
                    *pixel = palette[color as usize];
                }
            }
            PixelCanvasDepth::FourBpp => {
                for (i, pixel) in self.backing_buf.iter_mut().enumerate() {
                    let byte = read_byte(i / 2);
                    let shift = (i % 2) * 4;
                    let color = (byte >> (4 - shift)) & 0x0F;
                    *pixel = PALETTE_4BPP[color as usize];
                }
            }
            PixelCanvasDepth::EightBpp => {
                let pal = match self.vga_palette {
                    VgaPalette::Device if self.device_palette.len() == 256 => &self.device_palette[..],
                    VgaPalette::DefaultVga => &DEFAULT_VGA_PALETTE,
                    VgaPalette::GrayscaleReversed => &GRAYSCALE_RAMP_REVERSED,
                    _ => &GRAYSCALE_RAMP,
                };

                for (i, pixel) in self.backing_buf.iter_mut().enumerate() {
                    *pixel = pal[read_byte(i) as usize];
                }
            }
            PixelCanvasDepth::Rgb => {
                for (i, pixel) in self.backing_buf.iter_mut().enumerate() {
                    let idx = i * 3;
                    *pixel = Color32::from_rgb(read_byte(idx), read_byte(idx + 1), read_byte(idx + 2));
                }
            }
            PixelCanvasDepth::Rgba => {
                for (i, pixel) in self.backing_buf.iter_mut().enumerate() {
                    let idx = i * 4;
                    *pixel = Color32::from_rgba_premultiplied(
                        read_byte(idx),
                        read_byte(idx + 1),
                        read_byte(idx + 2),
                        read_byte(idx + 3),
                    );
                }
            }
        }
        self.data_unpacked = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    fn assert_image_matches_buffer(canvas: &PixelCanvas) {
        let ImageData::Color(image) = &canvas.image_data;
        assert_eq!(
            image.size,
            [canvas.view_dimensions.0 as usize, canvas.view_dimensions.1 as usize]
        );
        assert_eq!(image.pixels.len(), image.size[0] * image.size[1]);
        assert_eq!(image.pixels, canvas.backing_buf);
    }

    #[test]
    fn buffer_sizes_round_to_whole_bytes() {
        for (depth, pixels_per_byte) in [
            (PixelCanvasDepth::OneBpp, 8),
            (PixelCanvasDepth::TwoBpp, 4),
            (PixelCanvasDepth::FourBpp, 2),
        ] {
            for (pixels, bytes) in [(0, 0), (1, 1), (pixels_per_byte, 1), (pixels_per_byte + 1, 2)] {
                assert_eq!(PixelCanvas::calc_slice_size((pixels, 1), depth, None), bytes);
            }
        }
        for (depth, bytes) in [
            (PixelCanvasDepth::OneBpp, 2),
            (PixelCanvasDepth::TwoBpp, 3),
            (PixelCanvasDepth::FourBpp, 5),
            (PixelCanvasDepth::EightBpp, 9),
            (PixelCanvasDepth::Rgb, 27),
            (PixelCanvasDepth::Rgba, 36),
        ] {
            assert_eq!(PixelCanvas::calc_slice_size((3, 3), depth, None), bytes);
        }
    }

    #[test]
    fn bitmap_decoding_pads_and_truncates() {
        let white = Color32::WHITE;
        let black = Color32::BLACK;
        let palette_2bpp = CgaPalette::default().colors();
        // Each case crosses a source-byte or pixel boundary. Short RGB(A) data ends mid-pixel.
        let cases = [
            (
                PixelCanvasDepth::OneBpp,
                vec![0b10100000, 0b10000000],
                vec![white, black, white, black, black, black, black, black, white],
                1,
                vec![white, black, white, black, black, black, black, black, black],
            ),
            (
                PixelCanvasDepth::TwoBpp,
                vec![0b00011011, 0b10000000],
                vec![
                    palette_2bpp[0],
                    palette_2bpp[1],
                    palette_2bpp[2],
                    palette_2bpp[3],
                    palette_2bpp[2],
                ],
                1,
                vec![
                    palette_2bpp[0],
                    palette_2bpp[1],
                    palette_2bpp[2],
                    palette_2bpp[3],
                    palette_2bpp[0],
                ],
            ),
            (
                PixelCanvasDepth::FourBpp,
                vec![0x1F, 0xA0],
                vec![PALETTE_4BPP[1], PALETTE_4BPP[15], PALETTE_4BPP[10]],
                1,
                vec![PALETTE_4BPP[1], PALETTE_4BPP[15], PALETTE_4BPP[0]],
            ),
            (
                PixelCanvasDepth::EightBpp,
                vec![17, 128, 255],
                vec![Color32::from_gray(17), Color32::from_gray(128), white],
                1,
                vec![Color32::from_gray(17), black, black],
            ),
            (
                PixelCanvasDepth::Rgb,
                vec![11, 22, 33, 44, 55, 66],
                vec![Color32::from_rgb(11, 22, 33), Color32::from_rgb(44, 55, 66)],
                4,
                vec![Color32::from_rgb(11, 22, 33), Color32::from_rgb(44, 0, 0)],
            ),
            (
                PixelCanvasDepth::Rgba,
                vec![11, 22, 33, 77, 44, 55, 66, 99],
                vec![
                    Color32::from_rgba_premultiplied(11, 22, 33, 77),
                    Color32::from_rgba_premultiplied(44, 55, 66, 99),
                ],
                7,
                vec![
                    Color32::from_rgba_premultiplied(11, 22, 33, 77),
                    Color32::from_rgba_premultiplied(44, 55, 66, 0),
                ],
            ),
        ];

        for (depth, data, expected, short_len, short_expected) in cases {
            let mut canvas = PixelCanvas::new((expected.len() as u32, 1), Context::default());
            canvas.set_bpp(depth);
            let empty_color = if depth == PixelCanvasDepth::Rgba {
                Color32::TRANSPARENT
            }
            else {
                black
            };
            let empty_expected = vec![empty_color; expected.len()];
            let mut oversized = data.clone();
            oversized.extend_from_slice(&[255, 255]);
            for (input, pixels) in [
                (data.as_slice(), &expected),
                (oversized.as_slice(), &expected),
                (&data[..short_len], &short_expected),
                (&[][..], &empty_expected),
            ] {
                canvas.update_data(input, None);
                assert_eq!(&canvas.backing_buf, pixels, "{depth:?}, input length {}", input.len());
                assert_eq!(canvas.data_buf.len(), data.len());
                assert!(canvas.data_unpacked);
                assert_image_matches_buffer(&canvas);
            }

            // Exercise a short buffer without update_data's padding.
            canvas.data_buf = data[..short_len].to_vec();
            canvas.unpack_pixels(None);
            assert_eq!(canvas.backing_buf, short_expected, "unbuffered {depth:?}");
        }
    }

    #[test]
    fn zero_area_needs_no_data() {
        let font = FontInfo::default();
        for dims in [(0, 9), (9, 0)] {
            for depth in [PixelCanvasDepth::OneBpp, PixelCanvasDepth::Text] {
                let mut canvas = PixelCanvas::new(dims, Context::default());
                canvas.set_bpp(depth);
                assert_eq!(canvas.get_required_data_size(Some(&font)), 0);
                canvas.update_data(&[255; 8], Some(&font));
                assert!(canvas.data_buf.is_empty());
                assert!(canvas.backing_buf.is_empty());
                assert_image_matches_buffer(&canvas);
            }
        }
    }

    #[test]
    fn construction_and_resize_keep_buffers_consistent() {
        let mut default_canvas = PixelCanvas::default();
        assert_eq!(default_canvas.data_buf.len(), 2048);
        assert_image_matches_buffer(&default_canvas);
        default_canvas.update_data(&[255], None);
        assert_image_matches_buffer(&default_canvas);

        let mut canvas = PixelCanvas::new((3, 3), Context::default());
        assert_eq!(canvas.data_buf.len(), 2);
        assert_image_matches_buffer(&canvas);
        canvas.update_data(&[255, 255], None);
        canvas.texture = Some(canvas.create_texture());
        assert_eq!(canvas.texture.as_ref().unwrap().size(), [3, 3]);
        assert_eq!(canvas.backing_buf, vec![Color32::WHITE; 9]);
        assert_image_matches_buffer(&canvas);

        canvas.resize((5, 3), None);
        assert_eq!(canvas.data_buf.len(), 2);
        assert_eq!(canvas.backing_buf, vec![Color32::BLACK; 15]);
        assert_eq!(canvas.texture.as_ref().unwrap().size(), [5, 3]);
        assert_eq!(canvas.old_texture.as_ref().unwrap().size(), [3, 3]);
        assert!(!canvas.data_unpacked);
        assert_image_matches_buffer(&canvas);
        canvas.update_data(&[255, 255], None);
        assert_eq!(canvas.backing_buf, vec![Color32::WHITE; 15]);
        assert_image_matches_buffer(&canvas);
    }

    #[test]
    fn text_layout_respects_glyph_dimensions() {
        let white = Color32::WHITE;
        let black = Color32::BLACK;
        let expected = [
            [white, black, white, white, white, white, black],
            [black, white, black, black, black, black, black],
            [white, black, white, white, white, white, black],
            [black, white, black, black, black, black, black],
            [black, black, black, black, black, black, black],
        ]
        .concat();
        for (h, max_scanline) in [(4, 2), (2, 4)] {
            let mut font = FontInfo {
                w: 3,
                h,
                max_scanline,
                font_data: vec![0; 512],
            };
            font.font_data[1] = 0xA0;
            font.font_data[257] = 0x40;
            font.font_data[2] = 0xE0;

            let mut canvas = PixelCanvas::new((7, 5), Context::default());
            // Fill the canvas first so uncleared partial cells would be visible.
            canvas.update_data(&[255; 5], None);
            canvas.set_bpp(PixelCanvasDepth::Text);
            assert_eq!(canvas.get_required_data_size(Some(&font)), 8);
            canvas.update_data(&[1, 0x0F, 2, 0x0F, 1, 0x0F, 2, 0x0F], Some(&font));
            assert_eq!(canvas.backing_buf, expected);
            assert_image_matches_buffer(&canvas);
        }

        // A canvas too narrow or too short for a glyph must clear the previous frame.
        let font = FontInfo::default();
        for dims in [(7, 8), (8, 7)] {
            let mut canvas = PixelCanvas::new(dims, Context::default());
            canvas.update_data(&[255; 8], None);
            canvas.set_bpp(PixelCanvasDepth::Text);
            assert_eq!(canvas.get_required_data_size(Some(&font)), 0);
            canvas.update_data(&[255; 8], Some(&font));
            assert!(canvas.backing_buf.iter().all(|pixel| *pixel == Color32::BLACK));
            assert!(canvas.data_unpacked);
            assert_image_matches_buffer(&canvas);
        }
    }

    #[test]
    fn text_input_pads_and_truncates() {
        let font = FontInfo {
            w: 8,
            h: 1,
            max_scanline: 1,
            font_data: vec![255; 256],
        };
        let mut canvas = PixelCanvas::new((16, 1), Context::default());
        canvas.set_bpp(PixelCanvasDepth::Text);
        let red = Color32::from_rgb(0xAA, 0, 0);
        for (data, first, second) in [
            (vec![0, 0x0F, 42, 0x04], Color32::WHITE, red),
            (vec![0, 0x0F, 42, 0x04, 99, 99], Color32::WHITE, red),
            (vec![0, 0x0F, 42], Color32::WHITE, Color32::BLACK),
            (vec![0, 0x0F], Color32::WHITE, Color32::BLACK),
            (vec![], Color32::BLACK, Color32::BLACK),
        ] {
            canvas.update_data(&data, Some(&font));
            assert_eq!(canvas.data_buf.len(), 4);
            assert_eq!(&canvas.backing_buf[..8], &[first; 8]);
            assert_eq!(&canvas.backing_buf[8..], &[second; 8]);
            assert_image_matches_buffer(&canvas);
        }

        // Check a missing attribute without update_data's padding.
        canvas.data_buf = vec![0, 0x0F, 42];
        canvas.unpack_pixels(Some(&font));
        assert_eq!(&canvas.backing_buf[..8], &[Color32::WHITE; 8]);
        assert_eq!(&canvas.backing_buf[8..], &[Color32::BLACK; 8]);
    }

    #[test]
    fn missing_glyph_data_uses_background() {
        let mut font = FontInfo {
            w: 8,
            h: 2,
            max_scanline: 2,
            font_data: vec![0; 66],
        };
        font.font_data[65] = 0x80;
        let mut canvas = PixelCanvas::new((8, 2), Context::default());
        canvas.set_bpp(PixelCanvasDepth::Text);
        let blue = Color32::from_rgb(0, 0, 0xAA);
        canvas.update_data(&[65, 0x1F], Some(&font));
        assert_eq!(canvas.backing_buf[0], Color32::WHITE);
        assert_eq!(&canvas.backing_buf[1..], &[blue; 15]);
        canvas.update_data(&[255, 0x1F], Some(&font));
        assert_eq!(canvas.backing_buf, vec![blue; 16]);
        font.font_data.clear();
        canvas.update_data(&[0, 0x1F], Some(&font));
        assert_eq!(canvas.backing_buf, vec![blue; 16]);
    }

    #[test]
    fn invalid_fonts_defer_rendering() {
        let mut canvas = PixelCanvas::new((8, 8), Context::default());
        canvas.update_data(&[255; 8], None);
        canvas.set_bpp(PixelCanvasDepth::Text);
        let invalid_fonts = [
            None,
            Some(FontInfo {
                w: 0,
                ..FontInfo::default()
            }),
            Some(FontInfo {
                w: 9,
                ..FontInfo::default()
            }),
            Some(FontInfo {
                h: 0,
                ..FontInfo::default()
            }),
            Some(FontInfo {
                max_scanline: 0,
                ..FontInfo::default()
            }),
        ];
        for font in &invalid_fonts {
            assert_eq!(canvas.get_required_data_size(font.as_ref()), 0);
            canvas.update_data(&[0, 0], font.as_ref());
            assert!(!canvas.data_unpacked);
            assert_eq!(canvas.backing_buf, vec![Color32::WHITE; 64]);
            assert_image_matches_buffer(&canvas);
        }

        canvas.update_texture();
        assert!(!canvas.data_unpacked);
        assert_image_matches_buffer(&canvas);
    }

    #[test]
    fn cga_palettes_recolor_loaded_pixels() {
        let mut canvas = PixelCanvas::new((8, 1), Context::default());
        canvas.set_bpp(PixelCanvasDepth::TwoBpp);
        let data = [0b00011011, 0b11100100];
        canvas.update_data(&data, None);
        canvas.texture = Some(canvas.create_texture());
        assert_eq!(canvas.cga_palette(), CgaPalette::Palette1High);
        assert_eq!(canvas.backing_buf[1], Color32::from_rgb(0x55, 0xFF, 0xFF));
        assert_eq!(canvas.backing_buf[2], Color32::from_rgb(0xFF, 0x55, 0xFF));
        assert_eq!(canvas.backing_buf[3], Color32::WHITE);

        // Independent RGB expectations cover brown correction, intensity, and the mode 5 palette.
        for (palette, rgb) in [
            (
                CgaPalette::Palette0Low,
                [[0x00, 0xAA, 0x00], [0xAA, 0x00, 0x00], [0xAA, 0x55, 0x00]],
            ),
            (
                CgaPalette::Palette0High,
                [[0x55, 0xFF, 0x55], [0xFF, 0x55, 0x55], [0xFF, 0xFF, 0x55]],
            ),
            (
                CgaPalette::Palette1Low,
                [[0x00, 0xAA, 0xAA], [0xAA, 0x00, 0xAA], [0xAA, 0xAA, 0xAA]],
            ),
            (
                CgaPalette::Palette1High,
                [[0x55, 0xFF, 0xFF], [0xFF, 0x55, 0xFF], [0xFF, 0xFF, 0xFF]],
            ),
            (
                CgaPalette::Palette2Low,
                [[0x00, 0xAA, 0xAA], [0xAA, 0x00, 0x00], [0xAA, 0xAA, 0xAA]],
            ),
            (
                CgaPalette::Palette2High,
                [[0x55, 0xFF, 0xFF], [0xFF, 0x55, 0x55], [0xFF, 0xFF, 0xFF]],
            ),
        ] {
            let [one, two, three] = rgb.map(|[r, g, b]| Color32::from_rgb(r, g, b));
            let expected = [Color32::BLACK, one, two, three, three, two, one, Color32::BLACK];
            canvas.set_cga_palette(palette);
            assert_eq!(canvas.backing_buf, expected, "{palette:?}");
            assert_eq!(canvas.data_buf, data);
            assert!(canvas.data_unpacked);
            assert_image_matches_buffer(&canvas);

            // Device updates must not replace the manually selected CGA palette.
            canvas.update_device_palette(vec![Color32::GREEN; 4]);
            assert_eq!(canvas.backing_buf, expected);
            canvas.update_data(&data, None);
            assert_eq!(canvas.backing_buf, expected);
        }
    }

    #[test]
    fn cga_palette_changes_wait_for_fresh_data() {
        let mut canvas = PixelCanvas::new((4, 1), Context::default());
        canvas.update_data(&[255], None);
        canvas.set_bpp(PixelCanvasDepth::TwoBpp);
        canvas.set_cga_palette(CgaPalette::Palette0Low);
        assert!(!canvas.data_unpacked);
        assert_eq!(canvas.backing_buf, [Color32::WHITE; 4]);
        canvas.update_data(&[0b00011011], None);
        let expected = [
            Color32::BLACK,
            Color32::from_rgb(0, 0xAA, 0),
            Color32::from_rgb(0xAA, 0, 0),
            Color32::from_rgb(0xAA, 0x55, 0),
        ];
        assert_eq!(canvas.backing_buf, expected);

        // The selected palette survives switching to another depth and back.
        canvas.set_bpp(PixelCanvasDepth::FourBpp);
        canvas.update_data(&[0x12, 0x34], None);
        canvas.set_bpp(PixelCanvasDepth::TwoBpp);
        canvas.update_data(&[0b00011011], None);
        assert_eq!(canvas.backing_buf, expected);

        canvas.resize((4, 2), None);
        canvas.set_cga_palette(CgaPalette::Palette2High);
        assert!(!canvas.data_unpacked);
        assert_eq!(canvas.backing_buf, [Color32::BLACK; 8]);
        assert_image_matches_buffer(&canvas);
        canvas.update_data(&[0b01010101; 2], None);
        assert_eq!(canvas.backing_buf, [Color32::from_rgb(0x55, 0xFF, 0xFF); 8]);
        assert_image_matches_buffer(&canvas);
    }

    #[test]
    fn eight_bpp_palette_refresh_and_fallback() {
        let mut canvas = PixelCanvas::new((2, 1), Context::default());
        canvas.set_bpp(PixelCanvasDepth::EightBpp);
        canvas.update_data(&[0, 255], None);
        assert_eq!(canvas.backing_buf, [Color32::BLACK, Color32::WHITE]);

        let mut palette = vec![Color32::BLUE; 256];
        palette[255] = Color32::RED;
        canvas.update_device_palette(palette.clone());
        assert_eq!(canvas.backing_buf, [Color32::BLUE, Color32::RED]);
        canvas.set_cga_palette(CgaPalette::Palette0Low);
        assert_eq!(canvas.backing_buf, [Color32::BLUE, Color32::RED]);
        assert_image_matches_buffer(&canvas);
        canvas.update_device_palette(vec![]);
        assert_eq!(canvas.backing_buf, [Color32::BLUE, Color32::RED]);
        canvas.set_vga_palette(VgaPalette::Grayscale);
        canvas.update_device_palette(palette);
        assert_eq!(canvas.backing_buf, [Color32::BLACK, Color32::WHITE]);
        canvas.set_vga_palette(VgaPalette::Device);
        canvas.update_device_palette(vec![Color32::GREEN; 16]);
        assert_eq!(canvas.backing_buf, [Color32::BLACK, Color32::WHITE]);
    }

    #[test]
    fn default_vga_palette_decodes_dac_colors() {
        let mut canvas = PixelCanvas::new((256, 1), Context::default());
        canvas.set_bpp(PixelCanvasDepth::EightBpp);
        canvas.update_device_palette(vec![Color32::GREEN; 256]);
        let data: Vec<u8> = (0..=255).collect();
        canvas.update_data(&data, None);
        canvas.texture = Some(canvas.create_texture());
        canvas.set_vga_palette(VgaPalette::DefaultVga);

        assert_eq!(&canvas.backing_buf[..16], &PALETTE_4BPP);
        let grays = [0, 20, 32, 44, 56, 68, 80, 97, 113, 129, 145, 161, 182, 202, 226, 255];
        assert_eq!(&canvas.backing_buf[16..32], &grays.map(Color32::from_gray));
        for (index, [r, g, b]) in [
            (32, [0, 0, 255]),
            (33, [64, 0, 255]),
            (34, [125, 0, 255]),
            (35, [190, 0, 255]),
            (40, [255, 0, 0]),
            (48, [0, 255, 0]),
            (56, [125, 125, 255]),
            (80, [182, 182, 255]),
            (104, [0, 0, 113]),
            (128, [56, 56, 113]),
            (152, [80, 80, 113]),
            (176, [0, 0, 64]),
            (200, [32, 32, 64]),
            (224, [44, 44, 64]),
        ] {
            assert_eq!(canvas.backing_buf[index], Color32::from_rgb(r, g, b), "index {index}");
        }
        assert_eq!(&canvas.backing_buf[248..], &[Color32::BLACK; 8]);
        assert_image_matches_buffer(&canvas);

        let expected = canvas.backing_buf.clone();
        canvas.update_device_palette(vec![Color32::BLUE; 256]);
        assert_eq!(canvas.backing_buf, expected);
        canvas.update_data(&data, None);
        assert_eq!(canvas.backing_buf, expected);
        canvas.set_vga_palette(VgaPalette::Device);
        assert_eq!(canvas.backing_buf, vec![Color32::BLUE; 256]);
        assert_image_matches_buffer(&canvas);
    }

    #[test]
    fn vga_grayscale_palettes_recolor_all_indices() {
        let mut canvas = PixelCanvas::new((256, 1), Context::default());
        canvas.set_bpp(PixelCanvasDepth::EightBpp);
        canvas.update_device_palette(vec![Color32::RED; 256]);
        let data: Vec<u8> = (0..=255).collect();
        canvas.update_data(&data, None);
        canvas.texture = Some(canvas.create_texture());
        assert_eq!(canvas.vga_palette(), VgaPalette::Device);
        assert_eq!(canvas.backing_buf, vec![Color32::RED; 256]);

        for palette in [VgaPalette::Grayscale, VgaPalette::GrayscaleReversed] {
            canvas.set_vga_palette(palette);
            let expected: Vec<Color32> = (0..=255)
                .map(|index| {
                    let shade = if palette == VgaPalette::Grayscale {
                        index
                    }
                    else {
                        255 - index
                    };
                    Color32::from_gray(shade)
                })
                .collect();
            assert_eq!(canvas.backing_buf, expected);
            assert_eq!(canvas.data_buf, data);
            assert_image_matches_buffer(&canvas);

            // New device colors are retained without overriding the selected grayscale ramp.
            canvas.update_device_palette(vec![Color32::GREEN; 256]);
            assert_eq!(canvas.backing_buf, expected);
            canvas.update_data(&data, None);
            assert_eq!(canvas.backing_buf, expected);
        }

        canvas.set_vga_palette(VgaPalette::Device);
        assert_eq!(canvas.backing_buf, vec![Color32::GREEN; 256]);
        assert_image_matches_buffer(&canvas);
    }

    #[test]
    fn vga_palette_changes_wait_for_fresh_data() {
        let mut canvas = PixelCanvas::new((2, 1), Context::default());
        canvas.update_data(&[255], None);
        canvas.set_bpp(PixelCanvasDepth::EightBpp);
        canvas.set_vga_palette(VgaPalette::GrayscaleReversed);
        assert!(!canvas.data_unpacked);
        assert_eq!(canvas.backing_buf, [Color32::WHITE; 2]);
        canvas.update_data(&[0, 255], None);
        assert_eq!(canvas.backing_buf, [Color32::WHITE, Color32::BLACK]);

        // A depth change and resize retain the selected ramp.
        canvas.set_bpp(PixelCanvasDepth::TwoBpp);
        canvas.update_data(&[0b00011011], None);
        canvas.set_bpp(PixelCanvasDepth::EightBpp);
        canvas.resize((4, 1), None);
        assert_eq!(canvas.vga_palette(), VgaPalette::GrayscaleReversed);
        canvas.set_vga_palette(VgaPalette::Grayscale);
        assert!(!canvas.data_unpacked);
        assert_eq!(canvas.backing_buf, [Color32::BLACK; 4]);
        assert_image_matches_buffer(&canvas);
        canvas.update_data(&[0, 85, 170, 255], None);
        assert_eq!(
            canvas.backing_buf,
            [
                Color32::BLACK,
                Color32::from_gray(85),
                Color32::from_gray(170),
                Color32::WHITE
            ]
        );
        assert_image_matches_buffer(&canvas);
    }

    #[test]
    fn palette_updates_preserve_text() {
        let font = FontInfo {
            w: 8,
            h: 1,
            max_scanline: 1,
            font_data: vec![255; 256],
        };
        let mut canvas = PixelCanvas::new((8, 1), Context::default());
        canvas.set_bpp(PixelCanvasDepth::Text);
        canvas.update_data(&[0, 0x0F], Some(&font));
        canvas.update_device_palette(vec![Color32::BLUE; 256]);
        canvas.set_cga_palette(CgaPalette::Palette2Low);
        canvas.set_vga_palette(VgaPalette::GrayscaleReversed);
        assert!(canvas.data_unpacked);
        assert_eq!(canvas.backing_buf, vec![Color32::WHITE; 8]);
        assert_image_matches_buffer(&canvas);
    }

    #[test]
    fn dimension_overflow_preserves_state() {
        let dims = (u32::MAX, u32::MAX);
        assert!(catch_unwind(|| PixelCanvas::new(dims, Context::default())).is_err());

        let mut canvas = PixelCanvas::new((8, 1), Context::default());
        canvas.update_data(&[255], None);
        assert!(catch_unwind(AssertUnwindSafe(|| canvas.resize(dims, None))).is_err());
        assert_eq!(canvas.view_dimensions, (8, 1));
        assert_eq!(canvas.data_buf, [255]);
        assert_eq!(canvas.backing_buf, vec![Color32::WHITE; 8]);
        assert!(canvas.data_unpacked);
        assert_image_matches_buffer(&canvas);
    }

    #[test]
    fn size_calculation_avoids_u32_overflow() {
        assert_eq!(
            PixelCanvas::calc_slice_size((65536, 65536), PixelCanvasDepth::EightBpp, None),
            4_294_967_296
        );
        assert_eq!(
            PixelCanvas::calc_slice_size((65536, 65536), PixelCanvasDepth::Rgba, None),
            17_179_869_184
        );
    }
}

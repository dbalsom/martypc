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

    render::consts.rs

*/

pub const DEFAULT_RENDER_WIDTH: u32 = 640;
pub const DEFAULT_RENDER_HEIGHT: u32 = 400;

pub const ATTR_BLUE_FG: u8      = 0b0000_0001;
pub const ATTR_GREEN_FG: u8     = 0b0000_0010;
pub const ATTR_RED_FG: u8       = 0b0000_0100;
pub const ATTR_BRIGHT_FG: u8    = 0b0000_1000;
pub const ATTR_BLUE_BG: u8      = 0b0001_0000;
pub const ATTR_GREEN_BG: u8     = 0b0010_0000;
pub const ATTR_RED_BG: u8       = 0b0100_0000;
pub const ATTR_BRIGHT_BG: u8    = 0b1000_0000;

// Font is encoded as a bit pattern with a span of 256 bits per row
//static CGA_FONT: &'static [u8; 2048] = include_bytes!("cga_font.bin");

pub const CGA_FIELD_OFFSET: u32 = 8192;

pub const FONT_SPAN: u32 = 32;
//const FONT_W: u32 = 8;
//const FONT_H: u32 = 8;

pub const CGA_HIRES_GFX_W: u32 = 640;
pub const CGA_HIRES_GFX_H: u32 = 200;
pub const CGA_GFX_W: u32 = 320;
pub const CGA_GFX_H: u32 = 200;

pub const EGA_LORES_GFX_W: u32 = 320;
pub const EGA_LORES_GFX_H: u32 = 200;
pub const EGA_HIRES_GFX_W: u32 = 640;
pub const EGA_HIRES_GFX_H: u32 = 350;

pub const VGA_LORES_GFX_W: u32 = 320;
pub const VGA_LORES_GFX_H: u32 = 200;
pub const VGA_HIRES_GFX_W: u32 = 640;
pub const VGA_HIRES_GFX_H: u32 = 480;

pub const XOR_COLOR: u8 = 0x80;

// This color-index to RGBA table supports two conversion palettes,
// the "standard" palette given by most online references, and the 
// alternate, more monitor-accurate "VileR palette"
// See https://int10h.org/blog/2022/06/ibm-5153-color-true-cga-palette/ 
// for details.
pub const CGA_RGBA_COLORS: &[[[u8; 4]; 16]; 2] = &[
    [
        [0x10, 0x10, 0x10, 0xFF], // 0 - Black  (Slightly brighter for debugging)
        [0x00, 0x00, 0xAA, 0xFF], // 1 - Blue
        [0x00, 0xAA, 0x00, 0xFF], // 2 - Green
        [0x00, 0xAA, 0xAA, 0xFF], // 3 - Cyan
        [0xAA, 0x00, 0x00, 0xFF], // 4 - Red
        [0xAA, 0x00, 0xAA, 0xFF], // 5 - Magenta
        [0xAA, 0x55, 0x00, 0xFF], // 6 - Brown
        [0xAA, 0xAA, 0xAA, 0xFF], // 7 - Light Gray
        [0x55, 0x55, 0x55, 0xFF], // 8 - Dark Gray
        [0x55, 0x55, 0xFF, 0xFF], // 9 - Light Blue
        [0x55, 0xFF, 0x55, 0xFF], // 10 - Light Green
        [0x55, 0xFF, 0xFF, 0xFF], // 11 - Light Cyan
        [0xFF, 0x55, 0x55, 0xFF], // 12 - Light Red
        [0xFF, 0x55, 0xFF, 0xFF], // 13 - Light Magenta
        [0xFF, 0xFF, 0x55, 0xFF], // 14 - Yellow
        [0xFF, 0xFF, 0xFF, 0xFF], // 15 - White
    ],
    // VileR's palette
    [
        [0x00, 0x00, 0x00, 0xFF], // 0 - Black
        [0x00, 0x00, 0xC4, 0xFF], // 1 - Blue
        [0x00, 0xC4, 0x00, 0xFF], // 2 - Green
        [0x00, 0xC4, 0xC4, 0xFF], // 3 - Cyan
        [0xC4, 0x00, 0x00, 0xFF], // 4 - Red
        [0xC4, 0x00, 0xC4, 0xFF], // 5 - Magenta
        [0xC4, 0x7E, 0x00, 0xFF], // 6 - Brown
        [0xC4, 0xC4, 0xC4, 0xFF], // 7 - Light Gray
        [0x4E, 0x4E, 0x4E, 0xFF], // 8 - Dark Gray
        [0x4E, 0x4E, 0xDC, 0xFF], // 9 - Light Blue
        [0x4E, 0xDC, 0x4E, 0xFF], // 10 - Light Green
        [0x4E, 0xF3, 0xF3, 0xFF], // 11 - Light Cyan
        [0xDC, 0x4E, 0x4E, 0xFF], // 12 - Light Red
        [0xF3, 0x4E, 0xF3, 0xFF], // 13 - Light Magenta
        [0xF3, 0xF3, 0x4E, 0xFF], // 14 - Yellow
        [0xFF, 0xFF, 0xFF, 0xFF], // 15 - White
    ],
];

// Little-endian
pub const CGA_RGBA_COLORS_U32: &[[u32; 16]; 2] = &[
    [
        u32::from_le_bytes(CGA_RGBA_COLORS[0][0]),
        u32::from_le_bytes(CGA_RGBA_COLORS[0][1]),
        u32::from_le_bytes(CGA_RGBA_COLORS[0][2]),
        u32::from_le_bytes(CGA_RGBA_COLORS[0][3]),
        u32::from_le_bytes(CGA_RGBA_COLORS[0][4]),
        u32::from_le_bytes(CGA_RGBA_COLORS[0][5]),
        u32::from_le_bytes(CGA_RGBA_COLORS[0][6]),
        u32::from_le_bytes(CGA_RGBA_COLORS[0][7]),
        u32::from_le_bytes(CGA_RGBA_COLORS[0][8]),
        u32::from_le_bytes(CGA_RGBA_COLORS[0][9]),
        u32::from_le_bytes(CGA_RGBA_COLORS[0][10]),
        u32::from_le_bytes(CGA_RGBA_COLORS[0][11]),
        u32::from_le_bytes(CGA_RGBA_COLORS[0][12]),
        u32::from_le_bytes(CGA_RGBA_COLORS[0][13]),
        u32::from_le_bytes(CGA_RGBA_COLORS[0][14]),
        u32::from_le_bytes(CGA_RGBA_COLORS[0][15]),
    ],
    // VileR's palette
    [
        u32::from_le_bytes(CGA_RGBA_COLORS[1][0]),
        u32::from_le_bytes(CGA_RGBA_COLORS[1][1]),
        u32::from_le_bytes(CGA_RGBA_COLORS[1][2]),
        u32::from_le_bytes(CGA_RGBA_COLORS[1][3]),
        u32::from_le_bytes(CGA_RGBA_COLORS[1][4]),
        u32::from_le_bytes(CGA_RGBA_COLORS[1][5]),
        u32::from_le_bytes(CGA_RGBA_COLORS[1][6]),
        u32::from_le_bytes(CGA_RGBA_COLORS[1][7]),
        u32::from_le_bytes(CGA_RGBA_COLORS[1][8]),
        u32::from_le_bytes(CGA_RGBA_COLORS[1][9]),
        u32::from_le_bytes(CGA_RGBA_COLORS[1][10]),
        u32::from_le_bytes(CGA_RGBA_COLORS[1][11]),
        u32::from_le_bytes(CGA_RGBA_COLORS[1][12]),
        u32::from_le_bytes(CGA_RGBA_COLORS[1][13]),
        u32::from_le_bytes(CGA_RGBA_COLORS[1][14]),
        u32::from_le_bytes(CGA_RGBA_COLORS[1][15]),
    ],
];
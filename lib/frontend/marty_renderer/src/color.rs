
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

    marty_render::color.rs

*/

use super::*;

// Return a RGBA slice given a CGA color Enum
pub fn color_enum_to_rgba(color: &CGAColor) -> &'static [u8; 4] {
    
    match color {
        CGAColor::Black         => &[0x10u8, 0x10u8, 0x10u8, 0xFFu8], // Make slightly visible for debugging
        CGAColor::Blue          => &[0x00u8, 0x00u8, 0xAAu8, 0xFFu8],
        CGAColor::Green         => &[0x00u8, 0xAAu8, 0x00u8, 0xFFu8],
        CGAColor::Cyan          => &[0x00u8, 0xAAu8, 0xAAu8, 0xFFu8],
        CGAColor::Red           => &[0xAAu8, 0x00u8, 0x00u8, 0xFFu8],
        CGAColor::Magenta       => &[0xAAu8, 0x00u8, 0xAAu8, 0xFFu8],
        CGAColor::Brown         => &[0xAAu8, 0x55u8, 0x00u8, 0xFFu8],
        CGAColor::White         => &[0xAAu8, 0xAAu8, 0xAAu8, 0xFFu8],
        CGAColor::BlackBright   => &[0x55u8, 0x55u8, 0x55u8, 0xFFu8],
        CGAColor::BlueBright    => &[0x55u8, 0x55u8, 0xFFu8, 0xFFu8],
        CGAColor::GreenBright   => &[0x55u8, 0xFFu8, 0x55u8, 0xFFu8],
        CGAColor::CyanBright    => &[0x55u8, 0xFFu8, 0xFFu8, 0xFFu8],
        CGAColor::RedBright     => &[0xFFu8, 0x55u8, 0x55u8, 0xFFu8],
        CGAColor::MagentaBright => &[0xFFu8, 0x55u8, 0xFFu8, 0xFFu8],
        CGAColor::Yellow        => &[0xFFu8, 0xFFu8, 0x55u8, 0xFFu8],
        CGAColor::WhiteBright   => &[0xFFu8, 0xFFu8, 0xFFu8, 0xFFu8],
    }
}

pub fn get_ega_gfx_color16(bits: u8) -> &'static [u8; 4] {

    #[allow(clippy::unusual_byte_groupings)]
    match bits & 0b010_111 {
        0b000_000 => &[0x10, 0x10, 0x10, 0xFF], // Make slightly brighter for debugging
        0b000_001 => &[0x00, 0x00, 0xAA, 0xFF],
        0b000_010 => &[0x00, 0xAA, 0x00, 0xFF],
        0b000_011 => &[0x00, 0xAA, 0xAA, 0xFF],
        0b000_100 => &[0xAA, 0x00, 0x00, 0xFF],
        0b000_101 => &[0xAA, 0x00, 0xAA, 0xFF],
        0b000_110 => &[0xAA, 0x55, 0x00, 0xFF], // Brown instead of dark yellow
        0b000_111 => &[0xAA, 0xAA, 0xAA, 0xFF],
        0b010_000 => &[0x55, 0x55, 0x55, 0xFF],
        0b010_001 => &[0x55, 0x55, 0xFF, 0xFF],
        0b010_010 => &[0x55, 0xFF, 0x55, 0xFF],
        0b010_011 => &[0x55, 0xFF, 0xFF, 0xFF],
        0b010_100 => &[0xFF, 0x55, 0x55, 0xFF],
        0b010_101 => &[0xFF, 0x55, 0xFF, 0xFF],
        0b010_110 => &[0xFF, 0xFF, 0x55, 0xFF],
        0b010_111 => &[0xFF, 0xFF, 0xFF, 0xFF],
        _ => &[0x00, 0x00, 0x00, 0xFF], // Default black
    }
}

pub fn get_ega_gfx_color64(bits: u8) -> &'static [u8; 4] {

    #[allow(clippy::unusual_byte_groupings)]
    match bits {
        0b000_000 => &[0x10, 0x10, 0x10, 0xFF], // Make slightly brighter for debugging
        0b000_001 => &[0x00, 0x00, 0xAA, 0xFF],
        0b000_010 => &[0x00, 0xAA, 0x00, 0xFF],
        0b000_011 => &[0x00, 0xAA, 0xAA, 0xFF],
        0b000_100 => &[0xAA, 0x00, 0x00, 0xFF],
        0b000_101 => &[0xAA, 0x00, 0xAA, 0xFF],
        0b000_110 => &[0xAA, 0xAA, 0x00, 0xFF], 
        0b000_111 => &[0xAA, 0xAA, 0xAA, 0xFF],
        0b001_000 => &[0x00, 0x00, 0x55, 0xFF],
        0b001_001 => &[0x00, 0x00, 0xFF, 0xFF],
        0b001_010 => &[0x00, 0xAA, 0x55, 0xFF],
        0b001_011 => &[0x00, 0xAA, 0xFF, 0xFF],
        0b001_100 => &[0xAA, 0x00, 0x55, 0xFF],
        0b001_101 => &[0xAA, 0x00, 0xFF, 0xFF],
        0b001_110 => &[0xAA, 0xAA, 0x55, 0xFF],
        0b001_111 => &[0xAA, 0xAA, 0xFF, 0xFF],
        0b010_000 => &[0x00, 0x55, 0x00, 0xFF],
        0b010_001 => &[0x00, 0x55, 0xAA, 0xFF],
        0b010_010 => &[0x00, 0xFF, 0x00, 0xFF],
        0b010_011 => &[0x00, 0xFF, 0xAA, 0xFF],
        0b010_100 => &[0xAA, 0x55, 0x00, 0xFF],
        0b010_101 => &[0xAA, 0x55, 0xAA, 0xFF],
        0b010_110 => &[0xAA, 0xFF, 0x00, 0xFF],
        0b010_111 => &[0xAA, 0xFF, 0xAA, 0xFF],
        0b011_000 => &[0x00, 0x55, 0x55, 0xFF],
        0b011_001 => &[0x00, 0x55, 0xFF, 0xFF],
        0b011_010 => &[0x00, 0xFF, 0x55, 0xFF],
        0b011_011 => &[0x00, 0xFF, 0xFF, 0xFF],
        0b011_100 => &[0xAA, 0x55, 0x55, 0xFF],
        0b011_101 => &[0xAA, 0x55, 0xFF, 0xFF],
        0b011_110 => &[0xAA, 0xFF, 0x55, 0xFF],
        0b011_111 => &[0xAA, 0xFF, 0xFF, 0xFF],
        0b100_000 => &[0x55, 0x00, 0x00, 0xFF],
        0b100_001 => &[0x55, 0x00, 0xAA, 0xFF],
        0b100_010 => &[0x55, 0xAA, 0x00, 0xFF],
        0b100_011 => &[0x55, 0xAA, 0xAA, 0xFF],
        0b100_100 => &[0xFF, 0x00, 0x00, 0xFF],
        0b100_101 => &[0xFF, 0x00, 0xAA, 0xFF],
        0b100_110 => &[0xFF, 0xAA, 0x00, 0xFF],
        0b100_111 => &[0xFF, 0xAA, 0xAA, 0xFF],
        0b101_000 => &[0x55, 0x00, 0x55, 0xFF],
        0b101_001 => &[0x55, 0x00, 0xFF, 0xFF],
        0b101_010 => &[0x55, 0xAA, 0x55, 0xFF],
        0b101_011 => &[0x55, 0xAA, 0xFF, 0xFF],
        0b101_100 => &[0xFF, 0x00, 0x55, 0xFF],
        0b101_101 => &[0xFF, 0x00, 0xFF, 0xFF],
        0b101_110 => &[0xFF, 0xAA, 0x55, 0xFF],
        0b101_111 => &[0xFF, 0xAA, 0xFF, 0xFF],
        0b110_000 => &[0x55, 0x55, 0x00, 0xFF],
        0b110_001 => &[0x55, 0x55, 0xAA, 0xFF],
        0b110_010 => &[0x55, 0xFF, 0x00, 0xFF],
        0b110_011 => &[0x55, 0xFF, 0xAA, 0xFF],
        0b110_100 => &[0xFF, 0x55, 0x00, 0xFF],
        0b110_101 => &[0xFF, 0x55, 0xAA, 0xFF],
        0b110_110 => &[0xFF, 0xFF, 0x00, 0xFF],
        0b110_111 => &[0xFF, 0xFF, 0xAA, 0xFF],
        0b111_000 => &[0x55, 0x55, 0x55, 0xFF],
        0b111_001 => &[0x55, 0x55, 0xFF, 0xFF],
        0b111_010 => &[0x55, 0xFF, 0x55, 0xFF],
        0b111_011 => &[0x55, 0xFF, 0xFF, 0xFF],
        0b111_100 => &[0xFF, 0x55, 0x55, 0xFF],
        0b111_101 => &[0xFF, 0x55, 0xFF, 0xFF],
        0b111_110 => &[0xFF, 0xFF, 0x55, 0xFF],
        0b111_111 => &[0xFF, 0xFF, 0xFF, 0xFF],
        _ => &[0x10, 0x10, 0x10, 0xFF], // Default black
    }
}

/// Attempt a simple 4-pixel lookup to composite artifact color. 
/// This is legacy code - you cannot accurately convert a composite image this way
pub fn get_cga_composite_color( bits: u8, palette: &CGAPalette ) -> &'static [u8; 4] {

    match (bits, palette) {

        (0b0000, CGAPalette::Monochrome(_)) => &[0x00, 0x00, 0x00, 0xFF], // Black
        (0b0001, CGAPalette::Monochrome(_)) => &[0x00, 0x68, 0x0C, 0xFF], // Forest Green
        (0b0010, CGAPalette::Monochrome(_)) => &[0x21, 0x2B, 0xBD, 0xFF], // Dark Blue
        (0b0011, CGAPalette::Monochrome(_)) => &[0x0D, 0x9E, 0xD5, 0xFF], // Cyan
        (0b0100, CGAPalette::Monochrome(_)) => &[0x85, 0x09, 0x6C, 0xFF], // Maroon
        (0b0101, CGAPalette::Monochrome(_)) => &[0x75, 0x73, 0x76, 0xFF], // Grey
        (0b0110, CGAPalette::Monochrome(_)) => &[0xAF, 0x36, 0xFF, 0xFF], // Magenta
        (0b0111, CGAPalette::Monochrome(_)) => &[0x9B, 0xA9, 0xFF, 0xFF], // Lilac
        (0b1000, CGAPalette::Monochrome(_)) => &[0x51, 0x47, 0x00, 0xFF], // Brown
        (0b1001, CGAPalette::Monochrome(_)) => &[0x42, 0xBD, 0x00, 0xFF], // Bright Green
        (0b1010, CGAPalette::Monochrome(_)) => &[0x51, 0x53, 0x51, 0xFF], // Darker Grey  0x70 0x74 0x70 actual values but this looks better in KQI
        (0b1011, CGAPalette::Monochrome(_)) => &[0x5D, 0xF4, 0x7A, 0xFF], // Lime Green
        (0b1100, CGAPalette::Monochrome(_)) => &[0xE5, 0x54, 0x1D, 0xFF], // Red-Orange
        (0b1101, CGAPalette::Monochrome(_)) => &[0xD7, 0xCB, 0x19, 0xFF], // Yellow
        (0b1110, CGAPalette::Monochrome(_)) => &[0xFF, 0x81, 0xF2, 0xFF], // Pink
        (0b1111, CGAPalette::Monochrome(_)) => &[0xFD, 0xFF, 0xFC, 0xFF], // White

        (0b0000, CGAPalette::MagentaCyanWhite(_)) => &[0x00, 0x00, 0x00, 0xFF], // Black
        (0b0001, CGAPalette::MagentaCyanWhite(_)) => &[0x00, 0x9A, 0xFF, 0xFF], // Blue #1
        (0b0010, CGAPalette::MagentaCyanWhite(_)) => &[0x00, 0x42, 0xFF, 0xFF], // Dark Blue
        (0b0011, CGAPalette::MagentaCyanWhite(_)) => &[0x00, 0x90, 0xFF, 0xFF], // Blue #2
        (0b0100, CGAPalette::MagentaCyanWhite(_)) => &[0xAA, 0x4C, 0x00, 0xFF], // Brown
        (0b0101, CGAPalette::MagentaCyanWhite(_)) => &[0x84, 0xFA, 0xD2, 0xFF], // Lime Green
        (0b0110, CGAPalette::MagentaCyanWhite(_)) => &[0xB9, 0xA2, 0xAD, 0xFF], // Gray
        (0b0111, CGAPalette::MagentaCyanWhite(_)) => &[0x96, 0xF0, 0xFF, 0xFF], // Pale Blue
        (0b1000, CGAPalette::MagentaCyanWhite(_)) => &[0xCD, 0x1F, 0x00, 0xFF], // Dark red
        (0b1001, CGAPalette::MagentaCyanWhite(_)) => &[0xA7, 0xCD, 0xFF, 0xFF], // Lilac #1
        (0b1010, CGAPalette::MagentaCyanWhite(_)) => &[0xDC, 0x75, 0xFF, 0xFF], // Magenta
        (0b1011, CGAPalette::MagentaCyanWhite(_)) => &[0xB9, 0xC3, 0xFF, 0xFF], // Lilac #2
        (0b1100, CGAPalette::MagentaCyanWhite(_)) => &[0xFF, 0x5C, 0x00, 0xFF], // Orange-Red
        (0b1101, CGAPalette::MagentaCyanWhite(_)) => &[0xED, 0xFF, 0xCC, 0xFF], // Pale yellow
        (0b1110, CGAPalette::MagentaCyanWhite(_)) => &[0xFF, 0xB2, 0xA6, 0xFF], // Peach
        (0b1111, CGAPalette::MagentaCyanWhite(_)) => &[0xFF, 0xFF, 0xFF, 0xFF], // White
        _ => &[0x00, 0x00, 0x00, 0xFF], // Default black
    }
}

pub fn get_cga_gfx_color(bits: u8, palette: &CGAPalette, intensity: bool) -> &'static [u8; 4] {
    match (bits, palette, intensity) {
        // Monochrome
        (0b00, CGAPalette::Monochrome(_), false) => &[0x00u8, 0x00u8, 0x00u8, 0x00u8], // Black
        (0b01, CGAPalette::Monochrome(fg), false) => color_enum_to_rgba(fg), // Foreground color
        // Palette 0 - Low Intensity
        (0b00, CGAPalette::RedGreenYellow(bg), false) => color_enum_to_rgba(bg), // Background color
        (0b01, CGAPalette::RedGreenYellow(_), false) => &[0x00u8, 0xAAu8, 0x00u8, 0xFFu8], // Green
        (0b10, CGAPalette::RedGreenYellow(_), false) => &[0xAAu8, 0x00u8, 0x00u8, 0xFFu8], // Red
        (0b11, CGAPalette::RedGreenYellow(_), false) => &[0xAAu8, 0x55u8, 0x00u8, 0xFFu8], // Brown
        // Palette 0 - High Intensity
        (0b00, CGAPalette::RedGreenYellow(bg), true) => color_enum_to_rgba(bg), // Background color
        (0b01, CGAPalette::RedGreenYellow(_), true) => &[0x55u8, 0xFFu8, 0x55u8, 0xFFu8], // GreenBright
        (0b10, CGAPalette::RedGreenYellow(_), true) => &[0xFFu8, 0x55u8, 0x55u8, 0xFFu8], // RedBright
        (0b11, CGAPalette::RedGreenYellow(_), true) => &[0xFFu8, 0xFFu8, 0x55u8, 0xFFu8], // Yellow
        // Palette 1 - Low Intensity
        (0b00, CGAPalette::MagentaCyanWhite(bg), false) => color_enum_to_rgba(bg), // Background color
        (0b01, CGAPalette::MagentaCyanWhite(_), false) => &[0x00u8, 0xAAu8, 0xAAu8, 0xFFu8], // Cyan
        (0b10, CGAPalette::MagentaCyanWhite(_), false) => &[0xAAu8, 0x00u8, 0xAAu8, 0xFFu8], // Magenta
        (0b11, CGAPalette::MagentaCyanWhite(_), false) => &[0xAAu8, 0xAAu8, 0xAAu8, 0xFFu8], // Gray
        // Palette 1 - High Intensity
        (0b00, CGAPalette::MagentaCyanWhite(bg), true) => color_enum_to_rgba(bg), // Background color
        (0b01, CGAPalette::MagentaCyanWhite(_), true) => &[0x55u8, 0xFFu8, 0xFFu8, 0xFFu8], // CyanBright
        (0b10, CGAPalette::MagentaCyanWhite(_), true) => &[0xFFu8, 0x55u8, 0xFFu8, 0xFFu8], // MagentaBright
        (0b11, CGAPalette::MagentaCyanWhite(_), true) => &[0xFFu8, 0xFFu8, 0xFFu8, 0xFFu8], // WhiteBright
        // Palette 2 - Low Intensity
        (0b00, CGAPalette::RedCyanWhite(bg), false) => color_enum_to_rgba(bg), // Background color
        (0b01, CGAPalette::RedCyanWhite(_), false) => &[0x00u8, 0xAAu8, 0xAAu8, 0xFFu8], // Cyan
        (0b10, CGAPalette::RedCyanWhite(_), false) => &[0xAAu8, 0x00u8, 0x00u8, 0xFFu8], // Red
        (0b11, CGAPalette::RedCyanWhite(_), false) => &[0xAAu8, 0xAAu8, 0xAAu8, 0xFFu8], // Gray
        // Palette 2 - High Intensity
        (0b00, CGAPalette::RedCyanWhite(bg), true) => color_enum_to_rgba(bg), // Background color
        (0b01, CGAPalette::RedCyanWhite(_), true) => &[0x55u8, 0xFFu8, 0xFFu8, 0xFFu8], // CyanBright
        (0b10, CGAPalette::RedCyanWhite(_), true) => &[0xFFu8, 0x55u8, 0x55u8, 0xFFu8], // RedBright
        (0b11, CGAPalette::RedCyanWhite(_), true) => &[0xFFu8, 0xFFu8, 0xFFu8, 0xFFu8], // WhiteBright
        _=> &[0x00u8, 0x00u8, 0x00u8, 0xFFu8] // Default Black
    }
}

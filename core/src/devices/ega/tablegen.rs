/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2024 Daniel Balsom

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

    devices::ega::tablegen.rs

    Const table generation for various lookups used by the CGA for fast
    character drawing.

*/

/// LUT to extend an 8-bit bitfield into a packed 64-bit value
pub const BIT_EXTEND_TABLE64: [u64; 256] = {
    let mut table = [0u64; 256];
    let mut i = 0;
    while i < 256 {
        let mut bit = 0;
        let mut j = 0u64;
        while bit < 8 {
            let segment = if (i >> bit) & 0x01 != 0 { 0xFFu64 } else { 0x00u64 };
            j |= segment << (bit * 8);
            bit += 1;
        }
        table[i] = j;
        i += 1;
    }
    table
};

/// LUT to extend an 8-bit bitfield into a packed 64-bit value, reversing the bit order
pub const BIT_EXTEND_REVERSE_TABLE64: [u64; 256] = {
    let mut table = [0u64; 256];
    let mut i = 0;
    while i < 256 {
        let mut bit = 0;
        let mut j = 0u64;
        while bit < 8 {
            let segment = if (i >> (7 - bit)) & 0x01 != 0 { 0xFFu64 } else { 0x00u64 };
            j |= segment << (bit * 8);
            bit += 1;
        }
        table[i] = j;
        i += 1;
    }
    table
};

pub const BYTE_EXTEND_TABLE64: [u64; 256] = {
    let mut table: [u64; 256] = [0; 256];
    let mut i: usize = 0;

    while i < 256 {
        let mut bit: usize = 0;
        let mut k: u64 = 0;

        while bit < 8 {
            k |= (i as u64) << ((7 - bit) * 8);
            bit += 1;
        }

        table[i] = k;
        i += 1;
    }
    table
};

pub const BYTE_EXTEND_TABLE: [[u8; 8]; 256] = {
    let mut table: [[u8; 8]; 256] = [[0; 8]; 256];
    let mut i: u32 = 0;

    while i < 256 {
        let mut j: u8 = 0;
        while j < 8 {
            table[i as usize][j as usize] = ((i as u8) >> (7 - j)) & 0x01;
            j += 1;
        }
        i += 1;
    }
    table
};

/// Constant initializer to pack all possible 6-bit values into 64, 64 bit words
/// representing 8 packed pixels each.
pub const EGA_COLORS_U64: [u64; 64] = {
    let mut packed = [0u64; 64];
    let mut i = 0;

    while i < 64 {
        packed[i] = (i as u64) * 0x0101010101010101;
        i += 1;
    }

    packed
};

/// Constant initializer to unpack all possible 8 bit patterns
pub const EGA_8BIT_TABLE: [u64; 256] = {
    let mut table: [u64; 256] = [0; 256];

    let mut glyph: usize = 0;
    let mut glyph_u64: u64;
    let mut bit: u8;
    loop {
        bit = 0;
        glyph_u64 = 0;
        loop {
            let bit_val = glyph & (0x01 << (7 - bit)) != 0;

            glyph_u64 |= (if bit_val { 0xFF } else { 0x00 }) << (bit * 8);

            if bit < 7 {
                bit += 1;
            }
            else {
                break;
            }
        }

        table[glyph] = glyph_u64;

        if glyph < 255 {
            glyph += 1;
        }
        else {
            break;
        }
    }

    table
};

/// Constant initializer to unpack all possible 8 bit patterns
/// in all 16 possible colors into 64 bit values for fast drawing.
pub const EGA_HIRES_GFX_TABLE: [[u64; 256]; 16] = {
    let mut table: [[u64; 256]; 16] = [[0; 256]; 16];
    let mut glyph;
    let mut color: usize = 0;

    loop {
        glyph = 0;
        loop {
            table[color][glyph] = EGA_8BIT_TABLE[glyph] & EGA_COLORS_U64[color];

            if glyph < 255 {
                glyph += 1;
            }
            else {
                break;
            }
        }

        if color < 15 {
            color += 1;
        }
        else {
            break;
        }
    }

    table
};

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

    devices::mda::tablegen.rs

    Const table generation for various lookups used by the MDA for fast
    character drawing.

*/

/// Constant initializer to unpack all possible 8 bit patterns into 64-bit values for fast writing.
pub const HGC_8BIT_TABLE: [u64; 256] = {
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

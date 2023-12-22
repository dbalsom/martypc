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

    ega::planes.rs

    Functions for handling writes to bit planes.
    Planes can be read directly - planes must be written to through the
    functions defined here to properly de-planarize data for fast rasterization.
*/

use super::*;

impl EGACard {
    pub fn deplane(&mut self, offset: usize) {
        for i in 0..8 {
            let mask = 0x80 >> i;
            let bit0 = (self.planes[0].buf[offset] & mask) >> (7 - i) << 0;
            let bit1 = ((self.planes[1].buf[offset] & mask) >> (7 - i)) << 1;
            let bit2 = ((self.planes[2].buf[offset] & mask) >> (7 - i)) << 2;
            let bit3 = ((self.planes[3].buf[offset] & mask) >> (7 - i)) << 3;
            let fourbpp = bit0 | bit1 | bit2 | bit3;
            self.chain_buf[offset * 8 + i] = fourbpp as u8;
        }
    }

    #[inline]
    pub fn plane_set(&mut self, p: usize, offset: usize, data: u8) {
        self.planes[p].buf[offset] = data;
        self.deplane(offset);
    }

    #[inline]
    pub fn plane_and(&mut self, p: usize, offset: usize, data: u8) {
        self.planes[p].buf[offset] &= data;
        self.deplane(offset);
    }

    #[inline]
    pub fn plane_or(&mut self, p: usize, offset: usize, data: u8) {
        self.planes[p].buf[offset] |= data;
        self.deplane(offset);
    }
}

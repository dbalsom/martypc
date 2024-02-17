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

    ---------------------------------------------------------------------------

    ega::vram.rs

    Implement the IBM EGA card's video RAM.

    A fully equipped EGA has four planes of 64k each, for a total of 256k.

    This module supplies an interface for reading and writing to the video RAM.
    Writes to video RAM are linearized, to assist with rasterization routines,
    and emulation of the EGA's plane chaining capability in CGA emulation mode.

*/

use crate::devices::ega::{DisplayPlane, EGA_GFX_PLANE_SIZE, EGA_MAX_CLOCK16};

/// When in CGA compatibility mode, the Graphics Controller outputs odd and even bits
/// to different planes so that the Attribute Controller can process the 2bpp data as
/// a 4bpp pixel. This array simulates that behavior - instead of emulating the process
/// in the graphics controller, we precalculate it on write.
pub const CGA_SHIFT: [u8; 4] = [0, 11, 13, 15];

pub struct Vram {
    // Display Planes
    planes: Box<[[u8; EGA_GFX_PLANE_SIZE]; 4]>,
    linear_buf: Box<[u8; EGA_GFX_PLANE_SIZE * 8]>,
    linear_cga_buf: Box<[u8; EGA_GFX_PLANE_SIZE * 4]>,
}

impl Vram {
    pub fn new() -> Self {
        Self {
            planes: vec![
                [0; EGA_GFX_PLANE_SIZE],
                [0; EGA_GFX_PLANE_SIZE],
                [0; EGA_GFX_PLANE_SIZE],
                [0; EGA_GFX_PLANE_SIZE],
            ]
            .into_boxed_slice()
            .try_into()
            .unwrap(),
            linear_buf: vec![0; EGA_GFX_PLANE_SIZE * 8].into_boxed_slice().try_into().unwrap(),
            linear_cga_buf: vec![0; EGA_GFX_PLANE_SIZE * 4].into_boxed_slice().try_into().unwrap(),
        }
    }

    #[inline]
    pub fn read_glyph(&self, offset: usize) -> u8 {
        self.planes[2][offset & 0xFFFF]
    }

    #[inline]
    pub fn peek_u8(&self, plane: usize, offset: usize) -> u8 {
        self.planes[plane][offset & 0xFFFF]
    }

    #[inline]
    pub fn read_u8(&self, plane: usize, offset: usize) -> u8 {
        self.planes[plane][offset & 0xFFFF]
    }

    #[inline]
    pub fn write_u8(&mut self, plane: usize, offset: usize, data: u8) {
        self.planes[plane][offset] = data;
        self.deplane(offset);
    }

    #[inline]
    pub fn read_linear(&self, offset: usize) -> u8 {
        self.linear_buf[offset]
    }

    /// Return a slice of 8 pixels from the linear buffer. This represents serialization of one byte from the
    /// four display planes.
    #[inline]
    pub fn serialize_linear(&self, offset: usize) -> &[u8] {
        let offset = offset << 3;
        &self.linear_buf[offset..offset + 8]
    }

    pub fn plane_len(&self) -> usize {
        self.planes[0].len()
    }

    pub fn plane_slice(&self, plane: usize) -> &[u8] {
        &self.planes[plane]
    }

    pub fn deplane(&mut self, offset: usize) {
        for i in 0..8 {
            let mask = 0x80 >> i;
            let bit0 = (self.planes[0][offset] & mask) >> (7 - i) << 0;
            let bit1 = ((self.planes[1][offset] & mask) >> (7 - i)) << 1;
            let bit2 = ((self.planes[2][offset] & mask) >> (7 - i)) << 2;
            let bit3 = ((self.planes[3][offset] & mask) >> (7 - i)) << 3;
            let fourbpp = bit0 | bit1 | bit2 | bit3;
            self.linear_buf[offset * 8 + i] = fourbpp as u8;
        }
    }

    #[inline]
    pub fn plane_set(&mut self, p: usize, offset: usize, data: u8) {
        self.planes[p][offset] = data;
        self.deplane(offset);
    }

    #[inline]
    pub fn plane_and(&mut self, p: usize, offset: usize, data: u8) {
        self.planes[p][offset] &= data;
        self.deplane(offset);
    }

    #[inline]
    pub fn plane_or(&mut self, p: usize, offset: usize, data: u8) {
        self.planes[p][offset] |= data;
        self.deplane(offset);
    }
}

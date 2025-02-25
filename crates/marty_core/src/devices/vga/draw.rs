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

    ---------------------------------------------------------------------------

    ega::draw.rs

    Indexed-color drawing routines for EGA.

*/

use super::*;

impl VGACard {
    /// Draw a character in hires mode (8 pixels) using a single solid color.
    /// Since all pixels are the same we can draw 64 bits at a time.
    #[inline]
    pub fn draw_solid_hchar_4bpp(&mut self, color: u8) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        frame_u64[self.rba >> 3] = EGA_COLORS_U64[(color & 0x0F) as usize];
    }

    #[inline]
    pub fn draw_from_ac(&mut self, attr_char: u64) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        frame_u64[self.rba >> 3] = attr_char;
    }

    #[inline]
    pub fn draw_from_ac_halfclock(&mut self, attr_char0: u64, attr_char1: u64) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        frame_u64[self.rba >> 3] = attr_char0;
        frame_u64[(self.rba >> 3) + 1] = attr_char1;
    }

    #[inline]
    pub fn draw_solid_hchar_6bpp(&mut self, color: u8) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        frame_u64[self.rba >> 3] = EGA_COLORS_6BPP_U64[(color & 0x3F) as usize];
    }

    #[inline]
    pub fn draw_debug_hchar_at(&mut self, addr: usize, color: u8) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        frame_u64[addr >> 3] = EGA_COLORS_U64[(color & 0x0F) as usize];
    }

    /// Draw a character clock worth of overscan. Overscan only applies to low resolution graphics modes
    /// and the palette is not applied.
    #[inline]
    pub fn draw_overscan_lchar(&mut self) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        let attr_color = EGA_COLORS_U64[self.ac.overscan_color.six as usize];
        frame_u64[self.rba >> 3] = attr_color;
        frame_u64[(self.rba >> 3) + 1] = attr_color;
    }

    /// Draw a character in low res mode (16 pixels) using a single solid color.
    /// Since all pixels are the same we can draw 64 bits at a time.
    #[inline]
    pub fn draw_solid_lchar(&mut self, color: u8) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        let attr_color = EGA_COLORS_U64[(color & 0x0F) as usize];
        frame_u64[self.rba >> 3] = attr_color;
        frame_u64[(self.rba >> 3) + 1] = attr_color;
    }

    #[inline]
    pub fn draw_solid_lchar_6bpp(&mut self, color: u8) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        let attr_color = EGA_COLORS_6BPP_U64[(color & 0x0F) as usize];
        frame_u64[self.rba >> 3] = attr_color;
        frame_u64[(self.rba >> 3) + 1] = attr_color;
    }

    pub fn draw_gfx_mode_hchar_4bpp(&mut self) {
        //let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        //let deplaned_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.chain_buf);
        //frame_u64[self.rba >> 3] = deplaned_u64[(self.vma & 0xFFFFF) >> 3];

        for i in 0..8 {
            let buf_i = i - self.pel_pan_latch as usize;
            self.buf[self.back_buf][self.rba + buf_i] = self.sequencer.read_linear((self.vma * 8 & 0x7FFFF) + i);
        }
    }

    pub fn get_gfx_mode_lchar_6pp(&mut self) -> u64 {
        let mut span64 = 0;
        for i in 0..8 {
            let pixel = self.ac.palette_registers
                [(self.sequencer.read_linear((self.vma * 8 & 0x7FFFF) + i) & 0x0F) as usize]
                .four_to_six;
            span64 |= (pixel as u64) << ((7 - i) * 8);
        }
        span64
    }

    pub fn get_gfx_mode_hchar_6pp(&mut self) -> u64 {
        let mut span64 = 0;
        for i in 0..8 {
            let pixel = self.ac.palette_registers
                [(self.sequencer.read_linear((self.vma * 8 & 0x7FFFF) + i) & 0x0F) as usize]
                .six;
            span64 |= (pixel as u64) << ((7 - i) * 8);
        }
        span64
    }

    pub fn draw_gfx_mode_hchar_6bpp(&mut self) {
        //let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        //let deplaned_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.chain_buf);
        //frame_u64[self.rba >> 3] = deplaned_u64[(self.vma & 0xFFFFF) >> 3];

        for i in 0..8 {
            let buf_i = i - self.pel_pan_latch as usize;

            let attr_color = self.ac.palette_registers
                [(self.sequencer.read_linear((self.vma * 8 & 0x7FFFF) + i) & 0x0F) as usize]
                .six;
            self.buf[self.back_buf][self.rba + buf_i] = attr_color;
        }
    }

    pub fn draw_gfx_mode_lchar_4bpp(&mut self) {
        //let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        //let deplaned_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.chain_buf);
        //frame_u64[self.rba >> 3] = deplaned_u64[(self.vma & 0xFFFFF) >> 3];

        let ser = self.gc.serialize(&self.sequencer, self.vma);
        for i in 0..8 {
            let buf_i = (i * 2) - (self.pel_pan_latch * 2) as usize;
            let attr_color = self.ac.palette_registers[(ser[i] & 0x3F) as usize].four_to_six;
            self.buf[self.back_buf][self.rba + buf_i] = attr_color;
            self.buf[self.back_buf][self.rba + buf_i + 1] = attr_color;
        }
    }

    pub fn draw_gfx_mode_lchar_6bpp(&mut self) {
        //let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        //let deplaned_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.chain_buf);
        //frame_u64[self.rba >> 3] = deplaned_u64[(self.vma & 0xFFFFF) >> 3];

        for i in 0..8 {
            let buf_i = (i * 2) - (self.pel_pan_latch * 2) as usize;

            let attr_color = self.ac.palette_registers
                [(self.sequencer.read_linear((self.vma * 8 & 0x7FFFF) + i) & 0x3F) as usize]
                .six;
            self.buf[self.back_buf][self.rba + buf_i] = attr_color;
            self.buf[self.back_buf][self.rba + buf_i + 1] = attr_color;
        }
    }

    pub fn draw_from_ac8(&mut self, pixels: &[u8]) {
        for (i, nibble_pair) in pixels.chunks_exact(2).enumerate() {
            let byte = (nibble_pair[0] << 4) | nibble_pair[1];
            self.buf[self.back_buf][self.rba + (i * 2)] = byte;
            self.buf[self.back_buf][self.rba + (i * 2) + 1] = byte;
        }
    }

    pub fn draw_gc_debug(&mut self) {
        for (i, nibble_pair) in self.gc_debug.chunks_exact(2).enumerate() {
            let byte = (nibble_pair[0] << 4) | nibble_pair[1];
            self.buf[self.back_buf][self.rba + (i * 2)] = byte;
            self.buf[self.back_buf][self.rba + (i * 2) + 1] = byte;
        }
    }
}

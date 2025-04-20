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

macro_rules! buf_mut {
    ($self:expr) => {
        &mut $self.buf[$self.back_buf][$self.rba..]
    };
}

macro_rules! palette_lookup4 {
    ($self:expr, $color:expr) => {
        $self.ac.color_registers_u32[$self.ac.palette_registers[($color & 0x0F) as usize].four_to_six as usize]
    };
}

macro_rules! palette_lookup6 {
    ($self:expr, $color:expr) => {
        $self.ac.color_registers_u32[$self.ac.palette_registers[($color & 0x0F) as usize].six as usize]
    };
}

impl VGACard {
    /// Draw a character (8 pixels) in a hires 4bpp mode using a single solid color.
    /// (32-bit)
    #[inline]
    pub fn draw_solid_hchar_4bpp(&mut self, pixel: u8) {
        let buf_mut = buf_mut!(self);
        let color_u32 = palette_lookup4!(self, pixel);

        for i in 0..8 {
            buf_mut[i] = color_u32;
        }
    }

    #[inline]
    pub fn draw_solid_rgba(&mut self, color_u32: u32) {
        let buf_mut = buf_mut!(self);
        for i in 0..8 {
            buf_mut[i] = color_u32;
        }
    }

    #[inline]
    pub fn draw_tint_blue(&mut self) {
        let buf_mut = buf_mut!(self);
        for i in 0..8 {
            let mut color = buf_mut[i];
            color |= 0xFFFF0000; // Set the blue channel to 0xFF
            buf_mut[i] = color;
        }
    }

    #[inline]
    pub fn draw_tint_green(&mut self) {
        let buf_mut = buf_mut!(self);
        for i in 0..8 {
            let mut color = buf_mut[i];
            color |= 0xFF00FF00; // Set the green channel to 0xFF
            buf_mut[i] = color;
        }
    }

    #[inline]
    pub fn draw_tint_red(&mut self) {
        let buf_mut = buf_mut!(self);
        for i in 0..8 {
            let mut color = buf_mut[i];
            color |= 0xFF0000FF; // Set the red channel to 0xFF
            buf_mut[i] = color;
        }
    }

    /*    #[inline]
        pub fn draw_from_ac(&mut self, attr_char: u64) {
            let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
            frame_u64[self.rba >> 3] = attr_char;
        }
    */
    /// Draw a character from the attribute controller packed into a u64 of 8 pixels
    /// (32-bit)
    #[inline]
    pub fn draw_from_ac(&mut self, attr_char: u64) {
        let buf_mut = buf_mut!(self);

        for i in (0..=7).rev() {
            let pixel = (attr_char >> (i * 8)) as u8;
            let color_u32 = self.ac.color_registers_u32[pixel as usize];
            //let color_u32 = palette_lookup6!(self, pixel);
            buf_mut[i] = color_u32;
        }
    }

    /// Draw a character from the attribute controller packed into a u64 of 8 pixels
    /// Nine-column mode.
    /// (32-bit)
    #[inline]
    pub fn draw_from_ac_9col(&mut self, attr_char: (u64, u8)) {
        let buf_mut = buf_mut!(self);

        for i in (0..=7).rev() {
            let pixel = (attr_char.0 >> (i * 8)) as u8;
            buf_mut[i] = palette_lookup6!(self, pixel);
        }
        buf_mut[8] = palette_lookup6!(self, attr_char.1);
    }

    /// Draw a character from the attribute controller packed into a u64 of 8 pixels
    /// Double each pixel due to a halved pixel clock.
    /// (32-bit)
    #[inline]
    pub fn draw_from_ac_halfclock(&mut self, attr_char0: u64, attr_char1: u64) {
        let buf_mut = buf_mut!(self);

        for i in (0..=7).rev() {
            let pixel = (attr_char0 >> (i * 8)) as u8;
            let color_u32 = palette_lookup6!(self, pixel);
            buf_mut[i] = color_u32;
        }
        for i in (0..=7).rev() {
            let pixel = (attr_char1 >> (i * 8)) as u8;
            let color_u32 = palette_lookup6!(self, pixel);
            buf_mut[8 + i] = color_u32;
        }
    }

    /// Draw a character (8 pixels) in a hires 4bpp mode using a single solid color.
    /// Use the color's 6bpp palette entry.
    /// (32-bit)
    #[inline]
    pub fn draw_solid_hchar_6bpp(&mut self, pixel: u8) {
        let buf_mut = buf_mut!(self);
        let color_u32 = palette_lookup6!(self, pixel);

        for i in 0..8 {
            buf_mut[i] = color_u32;
        }
    }

    /// Draw 8 pixels of a solid color at a specified framebuffer address.
    /// (32-bit)
    #[inline]
    pub fn draw_debug_hchar_at(&mut self, addr: usize, color: u8) {
        let buf_mut = &mut self.buf[self.back_buf][addr..];

        for i in (0..=7).rev() {
            buf_mut[i] = EGA_PALETTE[(color & FOUR_BITS) as usize];
        }
    }

    /// Draw a character clock worth of overscan. Overscan only applies to low resolution graphics
    /// modes and the palette is not applied.
    /// (32-bit)
    #[inline]
    pub fn draw_overscan_lchar(&mut self) {
        let buf_mut = buf_mut!(self);
        let attr_color = EGA_PALETTE[self.ac.overscan_color.six as usize];
        for i in 0..8 {
            buf_mut[i] = attr_color;
        }
    }

    /// Draw a character at half pixel clock (16 pixels) using a single solid 4bpp color.
    /// (32-bit)
    #[inline]
    pub fn draw_solid_lchar(&mut self, color: u8) {
        let buf_mut = buf_mut!(self);
        let attr_color = EGA_PALETTE[(color & FOUR_BITS) as usize];
        for i in 0..16 {
            buf_mut[i] = attr_color;
        }
    }

    /// Draw a character at half pixel clock (16 pixels) using a single solid 6bpp color.
    /// (32-bit)
    #[inline]
    pub fn draw_solid_lchar_6bpp(&mut self, color: u8) {
        let buf_mut = buf_mut!(self);
        let attr_color = EGA_PALETTE[(color & SIX_BITS) as usize];

        for i in 0..16 {
            buf_mut[i] = attr_color;
        }
    }

    /* Old routine for reference.
    pub fn draw_gfx_mode_hchar_4bpp(&mut self) {
        //let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        //let deplaned_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.chain_buf);
        //frame_u64[self.rba >> 3] = deplaned_u64[(self.vma & 0xFFFFF) >> 3];

        for i in 0..8 {
            let buf_i = i - self.pel_pan_latch as usize;
            self.buf[self.back_buf][self.rba + buf_i] = self.sequencer.read_linear(((self.vma * 8) & 0x7FFFF) + i);
        }
    }
    */

    /// Draw a character at full pixel clock (8 pixels) in graphics mode.
    /// (32-bit)
    pub fn draw_gfx_mode_hchar_4bpp(&mut self) {
        let buf_mut = buf_mut!(self);
        for i in 0..8 {
            // TODO: won't this underflow?
            let buf_i = i - self.pel_pan_latch as usize;
            let pixel = self.sequencer.read_linear(((self.vma * 8) & 0x7FFFF) + i);
            let color = palette_lookup6!(self, pixel);
            buf_mut[buf_i] = color;
        }
    }

    pub fn get_gfx_mode_lchar_6pp(&mut self) -> u64 {
        let mut span64 = 0;
        for i in 0..8 {
            let pixel = self.ac.palette_registers
                [(self.sequencer.read_linear(((self.vma * 8) & 0x7FFFF) + i) & 0x0F) as usize]
                .four_to_six;
            span64 |= (pixel as u64) << ((7 - i) * 8);
        }
        span64
    }

    pub fn get_gfx_mode_hchar_6pp(&mut self) -> u64 {
        let mut span64 = 0;
        for i in 0..8 {
            let pixel = self.ac.palette_registers
                [(self.sequencer.read_linear(((self.vma * 8) & 0x7FFFF) + i) & 0x0F) as usize]
                .six;
            span64 |= (pixel as u64) << ((7 - i) * 8);
        }
        span64
    }

    /// Rasterize a character clock of graphics at the full pixel clock (8 pixels).
    /// (32-bit)
    pub fn draw_gfx_mode_hchar_6bpp(&mut self) {
        //let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        //let deplaned_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.chain_buf);
        //frame_u64[self.rba >> 3] = deplaned_u64[(self.vma & 0xFFFFF) >> 3];
        let buf_mut = buf_mut!(self);

        for i in 0..8 {
            let buf_i = i - self.pel_pan_latch as usize;

            let attr_color = self.ac.palette_registers
                [(self.sequencer.read_linear(((self.vma * 8) & 0x7FFFF) + i) & 0x0F) as usize]
                .six;
            let color32 = self.ac.color_registers_u32[attr_color as usize];
            buf_mut[buf_i] = color32;
        }
    }

    /// Rasterize a character clock of graphics at half pixel clock (16 pixels).
    /// Use the color indexes' 4bpp palette entry.
    /// (32-bit)
    pub fn draw_gfx_mode_lchar_4bpp(&mut self) {
        //let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        //let deplaned_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.chain_buf);
        //frame_u64[self.rba >> 3] = deplaned_u64[(self.vma & 0xFFFFF) >> 3];
        let buf_mut = buf_mut!(self);
        let ser = self.gc.serialize(&self.sequencer, self.vma);
        for i in 0..8 {
            let buf_i = (i * 2) - (self.pel_pan_latch * 2) as usize;
            let attr_color = self.ac.palette_registers[(ser[i] & 0x3F) as usize].four_to_six;
            let color32 = self.ac.color_registers_u32[attr_color as usize];
            buf_mut[buf_i] = color32;
            buf_mut[buf_i + 1] = color32;
        }
    }

    /// Rasterize a character clock of graphics at half pixel clock (16 pixels).
    /// Use the color indexes' 6bpp palette entry.
    /// (32-bit)
    pub fn draw_gfx_mode_lchar_6bpp(&mut self) {
        //let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        //let deplaned_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.chain_buf);
        //frame_u64[self.rba >> 3] = deplaned_u64[(self.vma & 0xFFFFF) >> 3];
        let buf_mut = buf_mut!(self);
        let ser = self.gc.serialize(&self.sequencer, self.vma);
        for i in 0..8 {
            let buf_i = (i * 2) - (self.pel_pan_latch * 2) as usize;
            let attr_color = self.ac.palette_registers[(ser[i] & 0x3F) as usize].six;
            let color32 = self.ac.color_registers_u32[attr_color as usize];
            buf_mut[buf_i] = color32;
            buf_mut[buf_i + 1] = color32;
        }
    }

    /// Rasterize a character clock of graphics in 8-bit mode at the full pixel clock (8 pixels).
    pub fn draw_from_ac8(&mut self, pixels: &[u8]) {
        log::debug!("draw_from_ac8: {:02X?}", pixels);
        let buf_mut = buf_mut!(self);
        for (i, nibble_pair) in pixels.chunks_exact(2).enumerate() {
            let byte = (nibble_pair[0] << 4) | nibble_pair[1];
            let color32 = self.ac.color_registers_u32[byte as usize];
            buf_mut[i * 2] = color32;
            buf_mut[(i * 2) + 1] = color32;
        }
    }

    pub fn draw_gc_debug(&mut self) {
        let buf_mut = buf_mut!(self);
        for (i, nibble_pair) in self.gc_debug.chunks_exact(2).enumerate() {
            let byte = (nibble_pair[0] << 4) | nibble_pair[1];
            let color32 = self.ac.color_registers_u32[byte as usize];
            buf_mut[i * 2] = color32;
            buf_mut[(i * 2) + 1] = color32;
        }
    }
}

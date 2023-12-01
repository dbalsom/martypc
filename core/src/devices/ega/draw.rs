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

    ega::draw.rs

    Indexed-color drawing routines for EGA.

*/

use crate::devices::ega::*;

impl EGACard {
    /// Draw a character in hires mode (8 pixels) using a single solid color.
    /// Since all pixels are the same we can draw 64 bits at a time.
    #[inline]
    pub fn draw_solid_hchar_4bpp(&mut self, color: u8) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        frame_u64[self.rba >> 3] = EGA_COLORS_U64[(color & 0x0F) as usize];
    }

    #[inline]
    pub fn draw_solid_hchar_6bpp(&mut self, color: u8) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);

        let attr_color = EGA_COLORS_6BPP_U64[(color & 0x3F) as usize];
        frame_u64[self.rba >> 3] = attr_color;
    }

    pub fn draw_debug_hchar_at(&mut self, addr: usize, color: u8) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        frame_u64[addr >> 3] = EGA_COLORS_U64[(color & 0x0F) as usize];
    }

    /// Draw a character in lowres mode (16 pixels) using a single solid color.
    /// Since all pixels are the same we can draw 64 bits at a time.
    #[inline]
    pub fn draw_solid_lchar(&mut self, color: u8) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        frame_u64[self.rba >> 3] = EGA_COLORS_U64[(color & 0x0F) as usize];
        frame_u64[(self.rba >> 3) + 1] = EGA_COLORS_U64[(color & 0x0F) as usize];
    }

    #[inline]
    pub fn draw_solid_lchar_6bpp(&mut self, color: u8) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);

        let attr_color = EGA_COLORS_6BPP_U64[self.attribute_palette_registers[(color & 0x3F) as usize].six as usize];
        frame_u64[self.rba >> 3] = attr_color;
        frame_u64[(self.rba >> 3) + 1] = attr_color;
    }

    /// Draw an entire character row in high resolution text mode (8 pixels)
    pub fn draw_text_mode_hchar14(&mut self) {
        // Do cursor if visible, enabled and defined
        if     self.vma == self.crtc_cursor_address as usize
            && self.cursor_status
            && self.blink_state
            && self.cursor_data[(self.vlc & 0x3F) as usize]
        {
            self.draw_solid_hchar_6bpp(self.cur_fg);
        }
        else if self.mode_enable {
            let glyph_row: u64;
            // Get the u64 glyph row to draw for the current fg and bg colors and character row (vlc)
            glyph_row = self.get_hchar_glyph14_row(self.cur_char as usize, self.vlc as usize);

            let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
            frame_u64[self.rba >> 3] = glyph_row;
        }
        else {
            // When mode bit is disabled in text mode, the CGA acts like VRAM is all 0.
            self.draw_solid_hchar_6bpp(EgaDefaultColor4Bpp::Brown as u8);
        }
    }

    /// Draw an entire character row in low resolution text mode (16 pixels)
    pub fn draw_text_mode_lchar14(&mut self) {
        // Do cursor if visible, enabled and defined
        if     self.vma == self.crtc_cursor_address as usize
            && self.cursor_status
            && self.blink_state
            && self.cursor_data[(self.vlc & 0x3F) as usize]
        {
            self.draw_solid_lchar_6bpp(self.cur_fg);
        }
        else if self.mode_enable {
            // Get the two u64 glyph row components to draw for the current fg and bg colors and character row (vlc)
            let (glyph_row0, glyph_row1) =
                self.get_lchar_glyph14_rows(self.cur_char as usize, self.vlc as usize);

            let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
            frame_u64[self.rba >> 3] = glyph_row0;
            frame_u64[(self.rba >> 3) + 1] = glyph_row1;
        } else {
            // When mode bit is disabled in text mode, the CGA acts like VRAM is all 0.
            self.draw_solid_lchar(0);
        }
    }

    pub fn draw_gfx_mode_hchar_4bpp(&mut self) {
        //let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        //let deplaned_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.chain_buf);
        //frame_u64[self.rba >> 3] = deplaned_u64[(self.vma & 0xFFFFF) >> 3];

        for i in 0..8 {
            let buf_i = i - self.pel_pan_latch as usize;
            self.buf[self.back_buf][self.rba + buf_i] =
                self.chain_buf[(self.vma * 8 & 0x7FFFF) + i];
        }
    }

    pub fn draw_gfx_mode_hchar_6bpp(&mut self) {
        //let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        //let deplaned_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.chain_buf);
        //frame_u64[self.rba >> 3] = deplaned_u64[(self.vma & 0xFFFFF) >> 3];

        for i in 0..8 {
            let buf_i = i - self.pel_pan_latch as usize;

            let attr_color = self.attribute_palette_registers[(self.chain_buf[(self.vma * 8 & 0x7FFFF) + i] & 0x0F) as usize].six;
            self.buf[self.back_buf][self.rba + buf_i] = attr_color;
        }
    }

    pub fn draw_gfx_mode_lchar_4bpp(&mut self) {
        //let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        //let deplaned_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.chain_buf);
        //frame_u64[self.rba >> 3] = deplaned_u64[(self.vma & 0xFFFFF) >> 3];

        for i in 0..8 {
            let buf_i = (i * 2) - (self.pel_pan_latch * 2) as usize;
            let attr_color = self.attribute_palette_registers[(self.chain_buf[(self.vma * 8 & 0x7FFFF) + i] & 0x3F) as usize].four_to_six;
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

            let attr_color = self.attribute_palette_registers[(self.chain_buf[(self.vma * 8 & 0x7FFFF) + i] & 0x3F) as usize].six;
            self.buf[self.back_buf][self.rba + buf_i] = attr_color;
            self.buf[self.back_buf][self.rba + buf_i + 1] = attr_color;
        }
    }


}
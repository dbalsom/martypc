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

    devices::tga::draw.rs

    Indexed framebuffer drawing routines.

*/

use super::{tga::tablegen::CGA_HIRES_GFX_TABLE, *};

impl TGACard {
    pub fn draw_overscan_pixel(&mut self) {
        self.buf[self.back_buf][self.rba] = self.cc_overscan_color;

        if self.clock_divisor == 2 {
            // If we are in a 320 column mode, duplicate the last pixel drawn
            self.buf[self.back_buf][self.rba + 1] = self.cc_overscan_color;
        }
    }

    pub fn draw_pixel(&mut self, color: u8) {
        self.buf[self.back_buf][self.rba] = color & 0x0F;

        if self.clock_divisor == 2 {
            // If we are in a 320 column mode, duplicate the last pixel drawn
            self.buf[self.back_buf][self.rba + 1] = color & 0x0F;
        }
    }

    /*
    #[inline]
    pub fn draw_solid_char(&mut self, color: u8) {

        let draw_span = (8 * self.clock_divisor) as usize;

        for i in 0..draw_span {
            self.buf[self.back_buf][self.rba + i] = color;
        }
    }
    */

    /// Draw a character (8 or 16 pixels) using a single solid color.
    /// Since all pixels are the same we can draw 64 bits at a time.
    #[inline]
    pub fn draw_solid_char(&mut self, color: u8) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);

        frame_u64[self.rba >> 3] = CGA_COLORS_U64[(color & 0x0F) as usize];
        if self.clock_divisor == 2 {
            frame_u64[(self.rba >> 3) + 1] = CGA_COLORS_U64[(color & 0x0F) as usize];
        }
    }

    /// Draw a character in hires mode (8 pixels) using a single solid color.
    /// Since all pixels are the same we can draw 64 bits at a time.
    #[inline]
    pub fn draw_solid_hchar(&mut self, color: u8) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        frame_u64[self.rba >> 3] = CGA_COLORS_U64[(color & 0x0F) as usize];
    }

    /// Draw a character in lowres mode (16 pixels) using a single solid color.
    /// Since all pixels are the same we can draw 64 bits at a time.
    #[inline]
    pub fn draw_solid_lchar(&mut self, color: u8) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        frame_u64[self.rba >> 3] = CGA_COLORS_U64[(color & 0x0F) as usize];
        frame_u64[(self.rba >> 3) + 1] = CGA_COLORS_U64[(color & 0x0F) as usize];
    }

    /// Draw a single character glyph column pixel in text mode, doubling the pixel if
    /// in 40 column mode.
    pub fn draw_text_mode_pixel(&mut self) {
        let mut new_pixel = match TGACard::get_glyph_bit(self.cur_char, self.char_col, self.vlc_c9) {
            true => {
                if self.cur_blink {
                    if self.blink_state {
                        self.cur_fg
                    }
                    else {
                        self.cur_bg
                    }
                }
                else {
                    self.cur_fg
                }
            }
            false => self.cur_bg,
        };

        // Do cursor
        if (self.vma == self.crtc_cursor_address) && self.cursor_status && self.blink_state {
            // This cell has the cursor address, cursor is enabled and not blinking
            if self.cursor_data[(self.vlc_c9 & 0x1F) as usize] {
                new_pixel = self.cur_fg;
            }
        }

        if !self.mode_enable {
            new_pixel = 0;
        }

        self.buf[self.back_buf][self.rba] = new_pixel;

        if self.clock_divisor == 2 {
            // If we are in a 320 column mode, duplicate the last pixel drawn
            self.buf[self.back_buf][self.rba + 1] = new_pixel;
        }
    }

    /// Draw an entire character row in high resolution text mode (8 pixels)
    pub fn draw_text_mode_hchar(&mut self) {
        // Do cursor if visible, enabled and defined
        if self.vma == self.crtc_cursor_address
            && self.cursor_status
            && self.blink_state
            && self.cursor_data[(self.vlc_c9 & 0x1F) as usize]
        {
            self.draw_solid_hchar(self.cur_fg);
        }
        else if self.mode_enable {
            let glyph_row: u64;
            // Get the u64 glyph row to draw for the current fg and bg colors and character row (vlc)
            glyph_row = self.get_hchar_glyph_row(self.cur_char as usize, self.vlc_c9 as usize);

            let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
            frame_u64[self.rba >> 3] = glyph_row;
        }
        else {
            // When mode bit is disabled in text mode, the CGA acts like VRAM is all 0.
            self.draw_solid_hchar(0);
        }
    }

    /// Draw an entire character row in low resolution text mode (16 pixels)
    pub fn draw_text_mode_lchar(&mut self) {
        //let draw_span = (8 * self.clock_divisor) as usize;

        // Do cursor if visible, enabled and defined
        if self.vma == self.crtc_cursor_address
            && self.cursor_status
            && self.blink_state
            && self.cursor_data[(self.vlc_c9 & 0x1F) as usize]
        {
            self.draw_solid_lchar(self.cur_fg);
        }
        else if self.mode_enable {
            // Get the two u64 glyph row components to draw for the current fg and bg colors and character row (vlc)
            let (glyph_row0, glyph_row1) = self.get_lchar_glyph_rows(self.cur_char as usize, self.vlc_c9 as usize);

            let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
            frame_u64[self.rba >> 3] = glyph_row0;
            frame_u64[(self.rba >> 3) + 1] = glyph_row1;
        }
        else {
            // When mode bit is disabled in text mode, the CGA acts like VRAM is all 0.
            self.draw_solid_lchar(0);
        }
    }

    /// Draw a pixel in low resolution graphics mode (320x200)
    /// In this mode, pixels are doubled
    pub fn draw_lowres_gfx_mode_pixel(&mut self) {
        let mut new_pixel = self.get_lowres_pixel_color(self.vlc_c9, self.char_col);

        if self.rba >= CGA_MAX_CLOCK - 2 {
            return;
        }

        if !self.mode_enable {
            new_pixel = self.cc_altcolor;
        }

        self.buf[self.back_buf][self.rba] = new_pixel;
        self.buf[self.back_buf][self.rba + 1] = new_pixel;
    }

    /// Draw 16 pixels in low res graphics mode (320x200)
    /// This routine uses precalculated lookups and masks to generate two u64
    /// values to write to the index frame buffer directly.
    pub fn draw_lowres_gfx_mode_char(&mut self) {
        if self.mode_enable {
            let lchar_dat = self.get_lowres_gfx_lchar(self.vlc_c9);
            let color0 = lchar_dat.0 .0;
            let color1 = lchar_dat.1 .0;
            let mask0 = lchar_dat.0 .1;
            let mask1 = lchar_dat.1 .1;

            let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);

            frame_u64[self.rba >> 3] = color0 | (mask0 & CGA_COLORS_U64[self.cc_altcolor as usize]);
            frame_u64[(self.rba >> 3) + 1] = color1 | (mask1 & CGA_COLORS_U64[self.cc_altcolor as usize]);
        }
        else {
            self.draw_solid_char(self.cc_altcolor);
        }
    }

    /// Draw 16 pixels in low res 4bpp graphics mode (320x200x16)
    pub fn draw_lowres_gfx_mode_4bpp_char(&mut self) {
        if self.mode_enable {
            let base_addr = self.get_gfx_addr(self.vlc_c9);
            let pair0 = self.mem[base_addr] as usize;
            let pair1 = self.mem[base_addr + 1] as usize;
            let pair2 = self.mem[base_addr + 2] as usize;
            let pair3 = self.mem[base_addr + 3] as usize;
            self.buf[self.back_buf][self.rba] = self.palette_registers[pair0 & 0x0F];
            self.buf[self.back_buf][self.rba + 1] = self.palette_registers[pair0 & 0x0F];
            self.buf[self.back_buf][self.rba + 2] = self.palette_registers[pair0 >> 4];
            self.buf[self.back_buf][self.rba + 3] = self.palette_registers[pair0 >> 4];
            self.buf[self.back_buf][self.rba + 4] = self.palette_registers[pair1 & 0x0F];
            self.buf[self.back_buf][self.rba + 5] = self.palette_registers[pair1 & 0x0F];
            self.buf[self.back_buf][self.rba + 6] = self.palette_registers[pair1 >> 4];
            self.buf[self.back_buf][self.rba + 7] = self.palette_registers[pair1 >> 4];
            self.buf[self.back_buf][self.rba + 8] = self.palette_registers[pair2 & 0x0F];
            self.buf[self.back_buf][self.rba + 9] = self.palette_registers[pair2 & 0x0F];
            self.buf[self.back_buf][self.rba + 10] = self.palette_registers[pair2 >> 4];
            self.buf[self.back_buf][self.rba + 11] = self.palette_registers[pair2 >> 4];
            self.buf[self.back_buf][self.rba + 12] = self.palette_registers[pair3 & 0x0F];
            self.buf[self.back_buf][self.rba + 13] = self.palette_registers[pair3 & 0x0F];
            self.buf[self.back_buf][self.rba + 14] = self.palette_registers[pair3 >> 4];
            self.buf[self.back_buf][self.rba + 15] = self.palette_registers[pair3 >> 4];
        }
        else {
            self.draw_solid_char(self.cc_altcolor);
        }
    }

    /// Draw pixels in high resolution graphics mode. (640x200)
    /// In this mode, two pixels are drawn at the same time.
    pub fn draw_hires_gfx_mode_pixel(&mut self) {
        let base_addr = self.get_gfx_addr(self.vlc_c9);

        let word = (self.mem[base_addr] as u16) << 8 | self.mem[base_addr + 1] as u16;

        let bit1 = (word >> CGA_LCHAR_CLOCK - (self.char_col * 2 + 1)) & 0x01;
        let bit2 = (word >> CGA_LCHAR_CLOCK - (self.char_col * 2 + 2)) & 0x01;

        if self.mode_enable {
            if bit1 == 0 {
                self.buf[self.back_buf][self.rba] = 0;
            }
            else {
                self.buf[self.back_buf][self.rba] = self.cc_altcolor;
            }

            if bit2 == 0 {
                self.buf[self.back_buf][self.rba + 1] = 0;
            }
            else {
                self.buf[self.back_buf][self.rba + 1] = self.cc_altcolor;
            }
        }
        else {
            self.buf[self.back_buf][self.rba] = 0;
            self.buf[self.back_buf][self.rba + 1] = 0;
        }
    }

    /// Draw a single character column in high resolution graphics mode (640x200)
    pub fn draw_hires_gfx_mode_char(&mut self) {
        let base_addr = self.get_gfx_addr(self.vlc_c9);
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);

        if self.mode_enable {
            let byte0 = self.mem[base_addr];
            let byte1 = self.mem[base_addr + 1];

            frame_u64[self.rba >> 3] = CGA_HIRES_GFX_TABLE[self.cc_altcolor as usize][byte0 as usize];
            frame_u64[(self.rba >> 3) + 1] = CGA_HIRES_GFX_TABLE[self.cc_altcolor as usize][byte1 as usize];
        }
        else {
            frame_u64[self.rba >> 3] = 0;
            frame_u64[(self.rba >> 3) + 1] = 0;
        }
    }
}

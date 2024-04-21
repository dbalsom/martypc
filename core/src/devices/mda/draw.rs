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

    devices::mda::draw.rs

    Indexed framebuffer drawing routines.

*/

use super::*;
use crate::devices::mda::tablegen::HGC_8BIT_TABLE;

impl MDACard {
    pub fn draw_overscan_pixel(&mut self) {
        self.buf[self.back_buf][self.rba] = 0;
    }

    pub fn draw_pixel(&mut self, color: u8) {
        self.buf[self.back_buf][self.rba] = color;
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

    /// Draw a character in hires mode (8 pixels) using a single solid color.
    /// Since all pixels are the same we can draw 64 bits at a time.
    #[inline]
    pub fn draw_solid_hchar(&mut self, color: u8) {
        for i in 0..MDA_CHAR_CLOCK as usize {
            self.buf[self.back_buf][self.rba + i] = color;
        }
    }

    /// Draw a character in hires mode (8 pixels) using a single solid color.
    /// Since all pixels are the same we can draw 64 bits at a time.
    #[inline]
    pub fn draw_solid_gchar(&mut self, color: u8) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        let color_u64 = CGA_COLORS_U64[color as usize];
        frame_u64[self.rba >> 3] = color_u64;
        frame_u64[(self.rba >> 3) + 1] = color_u64;
    }

    #[inline]
    pub fn draw_blank_gchar(&mut self, color: u8) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        frame_u64[self.rba >> 3] = 0;
        frame_u64[(self.rba >> 3) + 1] = 0;
    }

    /// Draw a single character glyph column pixel in text mode, doubling the pixel if
    /// in 40 column mode.
    pub fn draw_text_mode_pixel(&mut self) {
        let mut new_pixel = match MDACard::get_glyph_bit(self.cur_char, self.char_col, self.crtc.vlc()) {
            true => {
                if self.cur_blink {
                    if self.text_blink_state {
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
        if self.cursor_blink_state && self.crtc.cursor() {
            new_pixel = self.cur_fg;
        }

        if !self.mode_enable {
            new_pixel = 0;
        }

        self.buf[self.back_buf][self.rba] = new_pixel;
    }

    pub fn draw_text_mode_hchar_slow(&mut self) {
        // The MDA font is only 8 pixels wide, despite the 9 dot character clock. Certain glyphs
        // have the last column repeated.

        let glyph_on_color = match self.cur_blink {
            true if self.text_blink_state => self.cur_fg,
            true => self.cur_bg,
            false => self.cur_fg,
        };

        let glyph_row = self.crtc.vlc();

        let mut last_pixel = self.cur_fg;
        let mut do_ul = false;
        if self.mode.display_enable() {
            for hdot in 0..(MDA_CHAR_CLOCK - 1) {
                let mut new_pixel = match MDACard::get_glyph_bit(self.cur_char, hdot, glyph_row) {
                    true => {
                        self.last_bit |= true;
                        glyph_on_color
                    }
                    false => self.cur_bg,
                };

                // Do cursor
                if self.crtc.cursor() {
                    new_pixel = self.cur_fg;
                    self.last_bit |= true;
                }

                // Do underline
                if self.cur_ul && glyph_row == 12 {
                    new_pixel = self.cur_fg;
                    self.last_bit |= true;
                    do_ul = true;
                }

                self.buf[self.back_buf][self.rba + hdot as usize] = new_pixel;
                last_pixel = new_pixel;
            }

            if do_ul || self.cur_char & MDA_REPEAT_COL_MASK == MDA_REPEAT_COL_VAL {
                // Underlines and characters 0xC0-0xDF have the last column repeated.
                self.buf[self.back_buf][self.rba + (MDA_CHAR_CLOCK as usize) - 1] = last_pixel;
                self.last_bit |= last_pixel != 0;
            }
            else {
                self.buf[self.back_buf][self.rba + (MDA_CHAR_CLOCK as usize) - 1] = self.cur_bg;
            }
        }
        else {
            // When display is disabled, the MDA acts like VRAM is all 0.
            for hdot in 0..MDA_CHAR_CLOCK {
                self.buf[self.back_buf][self.rba + hdot as usize] = 0;
            }
        }
    }

    /// Draw a single character column in high resolution graphics mode (640x200)
    pub fn draw_hires_gfx_mode_char(&mut self) {
        let base_addr = self.get_gfx_addr(self.crtc.vlc());
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);

        if self.mode_enable {
            let byte0 = self.mem[base_addr];
            let byte1 = self.mem[base_addr + 1];

            frame_u64[self.rba >> 3] = HGC_8BIT_TABLE[byte0 as usize];
            frame_u64[(self.rba >> 3) + 1] = HGC_8BIT_TABLE[byte1 as usize];
        }
        else {
            frame_u64[self.rba >> 3] = 0;
            frame_u64[(self.rba >> 3) + 1] = 0;
        }
    }
}

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

    --------------------------------------------------------------------------

    devices::mda::draw.rs

    Indexed framebuffer drawing routines.

*/

use crate::devices::mda::*;

impl MDACard {
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
        let frame_addr = self.rba >> 3;
        // If we are 64-bit aligned, draw 64 bits at a time.
        /*
        if self.rba % 8 == 0 {
            let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
            frame_u64[frame_addr] = CGA_COLORS_U64[(color & 0x0F) as usize];
            // Draw 9th col.
            self.buf[self.back_buf][self.rba + 9] = color;
        }
        else {

         */
        for i in 0..MDA_CHAR_CLOCK as usize {
            self.buf[self.back_buf][self.rba + i] = color;
        }
        //}
    }

    /// Draw a single character glyph column pixel in text mode, doubling the pixel if
    /// in 40 column mode.
    pub fn draw_text_mode_pixel(&mut self) {
        let mut new_pixel = match MDACard::get_glyph_bit(self.cur_char, self.char_col, self.vlc_c9) {
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

    pub fn draw_text_mode_hchar_slow(&mut self) {
        // The MDA font is only 8 pixels wide, despite the 9 dot character clock. Certain glyphs
        // have the last column repeated.
        for hdot in 0..(MDA_CHAR_CLOCK - 1) {
            let mut new_pixel = match MDACard::get_glyph_bit(self.cur_char, hdot, self.vlc_c9) {
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

            if !self.mode.display_enable() {
                new_pixel = 0;
            }

            self.buf[self.back_buf][self.rba] = new_pixel;
            self.rba += 1;
        }
        // TODO: Properly handle 9th column here.
        self.buf[self.back_buf][self.rba] = 0;
        self.rba += 1;
    }

    /*
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

     */

    /*
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
     */
}

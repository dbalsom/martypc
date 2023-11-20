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

    ega::crtc_regs.rs

    Implement the EGA CRTC logic

*/

use crate::devices::ega::*;

pub const EGA_VBLANK_MASK: u16 = 0x001F;
pub const EGA_HBLANK_MASK: u8 = 0x001F;
pub const EGA_HSYNC_MASK: u8 = 0x001F;
pub const EGA_HSLC_MASK: u16 = 0x01FF;

impl EGACard {
    /// Update the CRTC logic for next character.
    pub fn tick_crtc_char(&mut self) {

        // Update horizontal character counter
        self.hcc = self.hcc.wrapping_add(1);

        /*
        if self.hcc == 0 {
            self.crtc_hborder = false;
            if self.hslc == 0 {
                // We are at the first character of a CRTC frame. Update start address.
                self.vma = self.crtc_frame_address;
            }
        }     

        if self.hcc == 0 && self.hslc == 0 {
            // We are at the first character of a CRTC frame. Update start address.
            self.vma = self.crtc_frame_address;
        }
        */

        // Advance video memory address offset and grab the next character + attr
        self.vma += 1;
        self.set_char_addr();

        // Glyph column reset to 0 for next char
        self.char_col = 0;

        // Process horizontal sync period
        if self.crtc_hblank {
            // End horizontal blank when we reach R3
            if (self.hcc & EGA_HBLANK_MASK) == self.crtc_end_horizontal_blank.end_horizontal_blank() {
                self.crtc_hblank = false;
            }
        }

        // Process horizontal sync period
        if self.monitor_hsync {
            // Increment horizontal sync counter (wrapping)
            self.hsc = self.hsc.wrapping_add(1);

            // Implement a fixed hsync width from the monitor's perspective - 
            // A wider programmed hsync width than these values shifts the displayed image to the right.
            let hsync_target = if self.clock_divisor == 1 { 
                std::cmp::min(5, 5)
            }
            else {
                std::cmp::min(2, 2)
            };

            // Do a horizontal sync
            if self.hsc == hsync_target {
                // Update the video mode, if an update is pending.
                // It is important not to change graphics mode while we are catching up during an IO instruction.

                /* TODO: implement deferred mode change for EGA?
                if !self.catching_up && self.mode_pending {
                    self.update_mode();
                    self.mode_pending = false;
                }*/

                // END OF LOGICAL SCANLINE
                if self.crtc_vblank {
                
                    //if self.vsc_c3h == CRTC_VBLANK_HEIGHT || self.beam_y == CGA_MONITOR_VSYNC_POS {
                    if (self.hslc & EGA_VBLANK_MASK) == self.crtc_end_vertical_blank {

                        self.in_last_vblank_line = true;
                        // We are leaving vblank period. Generate a frame.                      
                        self.do_vsync();
                        self.monitor_hsync = false;
                        return
                    }                        
                }

                // Restrict HSLC to 9-bit range.
                self.hslc = (self.hslc + 1) & EGA_HSLC_MASK;
                self.scanline += 1;
                
                // Reset beam to left of screen if we haven't already
                if self.raster_x > 0 {
                    self.raster_y += 1;
                }
                self.raster_x = 0;
                self.char_col = 0;

                let new_rba = self.extents.row_stride * self.raster_y as usize;
                self.rba = new_rba;

                // CRTC may still be in hsync at this point (if the programmed CRTC hsync width is larger
                // than our fixed hsync value)
                self.monitor_hsync = false;
            }                     
        }

        if self.crtc_hsync {
            // End horizontal sync when we reach R3
            if (self.hcc & EGA_HSYNC_MASK) == self.crtc_end_horizontal_retrace.end_horizontal_retrace() {
                self.hsync_ct += 1;
                // Only end CRTC hsync. Monitor can still be in hsync until fixed target elapses.
                self.crtc_hsync = false;
                self.hsc = 0;
            }   
        }

        if self.hcc == self.crtc_horizontal_display_end + 1 {
            // C0 == R1. At right edge of the active display area. Entering right overscan.

            if self.vlc == self.crtc_maximum_scanline {
                // At end of character height for current character row.
                // Save VMA in VMA'
                //log::debug!("Updating vma_t: {:04X}", self.vma_t);
                self.vma_t = self.vma;
            }

            // Save right overscan start position to calculate width of right overscan later
            //self.overscan_right_start = self.raster_x;
            self.in_display_area = false;
            self.crtc_den = false;
            self.crtc_hborder = true;
        }

        if self.hcc == self.crtc_start_horizontal_blank + 1 {
            // Leaving right overscan and entering horizontal blank
            self.crtc_hblank = true;
            self.crtc_den = false;
        }

        if self.hcc == self.crtc_start_horizontal_retrace + 1 {
            // Entering horizontal retrace
            self.crtc_hblank = true;
            // Both monitor and CRTC will enter hsync at the same time. Monitor may leave hsync first.
            self.crtc_hsync = true;
            self.monitor_hsync = true;
            self.crtc_den = false;
            self.hsc = 0;
        }

        if self.hcc == self.crtc_horizontal_total && self.in_last_vblank_line {
            // We are one char away from the beginning of the new frame.
            // Draw one char of border
            self.crtc_hborder = true;
        }

        // Total + 2 on EGA.
        if self.hcc == self.crtc_horizontal_total + 2 {
            // Leaving left overscan, finished scanning row. Entering active display area with
            // new logical scanline.

            /*
            if self.crtc_vblank {
                // If we are in vblank, advance Vertical Sync Counter
                self.vsc_c3h += 1;
            }
            */

            if self.in_last_vblank_line {
                self.in_last_vblank_line = false;
                self.crtc_vblank = false;
            }

            // Reset Horizontal Character Counter and increment character row counter
            self.hcc = 0;
            self.crtc_hborder = false;
            self.vlc += 1;
            // Return video memory address to starting position for next character row
            self.vma = self.vma_sl;
            
            // Reset the current character glyph to start of row
            self.set_char_addr();

            if !self.crtc_vblank {
                // Start the new row
                if self.hslc < self.crtc_vertical_display_end + 1 {
                    self.in_display_area = true;
                    self.crtc_den = true;
                }
            }
            
            if self.vlc > self.crtc_maximum_scanline  {
                // C9 == R9 We finished drawing this row of characters 

                self.vlc = 0;
                // Advance Vertical Character Counter
                self.vcc = self.vcc.wrapping_add(1);

                // Set vma to starting position for next character row
                //self.vma = (self.vcc_c4 as usize) * (self.crtc_horizontal_displayed as usize) + self.crtc_frame_address;
                //self.vma = self.vma_t;
                self.vma_sl = self.vma_sl + self.crtc_offset as usize * 2;
                self.vma = self.vma_sl;
                
                // Load next char + attr
                self.set_char_addr();
            }

            if self.hslc == self.crtc_vertical_retrace_start {
                // We've reached vertical retrace start. We set the crtc_vblank flag to start comparing hslc against
                // vertical_retrace_end register.

                //trace_regs!(self);
                //trace!(self, "Entering vsync");
                self.crtc_vblank = true;
                self.in_display_area = false;
                self.crtc_den = false;
            }

            if self.hslc == self.crtc_vertical_display_end + 1 {
                // We are leaving the bottom of the active display area, entering the lower overscan area.
                self.extents.visible_h = self.scanline;
                self.in_display_area = false;
                self.crtc_den = false;
                self.crtc_vborder = true;
            }

            if self.hslc == self.crtc_vertical_total {
                // We have reached vertical total, we are at the end of the top overscan and entering the active
                // display area.
                self.in_vta = false;
                self.vtac_c5 = 0;
                self.hslc = 0;

                self.hcc = 0;
                self.vcc = 0;
                self.vlc = 0;
                self.char_col = 0;                            
                self.crtc_frame_address = self.crtc_start_address as usize;
                self.vma = self.crtc_start_address as usize;
                self.vma_sl = self.vma;
                self.vma_t = self.vma;
                self.in_display_area = true;
                self.crtc_den = true;
                self.crtc_vborder = false;
                self.crtc_vblank = false;

                // Toggle blink state. This is toggled every 8 frames by default.
                if (self.frame % EGA_CURSOR_BLINK_RATE as u64) == 0 {
                    self.blink_state = !self.blink_state;
                }

                // Load first char + attr
                self.set_char_addr();     
            }
        }   
    }

    /// Set the character attributes for the current character.
    /// This applies to text mode only, but is computed in all modes at appropriate times.
    fn set_char_addr(&mut self) {

        // Address from CRTC is masked by 0x1FFF by the CGA card (bit 13 ignored) and doubled.
        let addr = (self.vma << 1) & 0xFFFF;
        self.cur_char = self.planes[0].buf[addr];
        self.cur_attr = self.planes[0].buf[addr + 1];

        self.cur_fg = self.cur_attr & 0x0F;
        
        // If blinking is enabled, the bg attribute is only 3 bits and only low-intensity colors 
        // are available. 
        // If blinking is disabled, all 16 colors are available as background attributes.
        if self.mode_blinking {
            self.cur_bg = (self.cur_attr >> 4) & 0x07;
            self.cur_blink = self.cur_attr & 0x80 != 0;
        }
        else {
            self.cur_bg = self.cur_attr >> 4;
            self.cur_blink = false;
        }
    }    
}
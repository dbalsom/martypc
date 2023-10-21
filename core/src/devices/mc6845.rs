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

    devices::mc6845.rs

    Implementation of the Motorola MC6845 CRT controller.
    Used internally by the MDA and CGA video cards.

*/

use crate::tracelogger::TraceLogger;

const CURSOR_LINE_MASK: u8      = 0b0000_1111;
const CURSOR_ATTR_MASK: u8      = 0b0011_0000;

const REGISTER_MAX: usize = 17;
const REGISTER_UNREADABLE_VALUE: u8 = 0xFF;

#[derive (Copy, Clone, Debug)]
pub enum CrtcRegister {
    HorizontalTotal,
    HorizontalDisplayed,
    HorizontalSyncPosition,
    SyncWidth,
    VerticalTotal,
    VerticalTotalAdjust,
    VerticalDisplayed,
    VerticalSync,
    InterlaceMode,
    MaximumScanlineAddress,
    CursorStartLine,
    CursorEndLine,
    StartAddressH,
    StartAddressL,
    CursorAddressH,
    CursorAddressL,
    LightPenPositionH,
    LightPenPositionL,
}

use crate::mc6845::CrtcRegister::*;

macro_rules! trace {
    ($self:ident, $($t:tt)*) => {{
        $self.trace_logger.print(&format!($($t)*));
        $self.trace_logger.print("\n".to_string());
    }};
}

macro_rules! trace_regs {
    ($self:ident) => {
        $self.trace_logger.print(
            &format!("")
            /*
            &format!(
                "[SL:{:03} HCC:{:03} VCC:{:03} VT:{:03} VS:{:03}] ", 
                $self.scanline,
                $self.hcc_c0,
                $self.vcc_c4,
                $self.crtc_vertical_total,
                $self.crtc_vertical_sync_pos
            )
            */
        );
    };
}


pub struct Crtc6845 {

    reg: [u8; 18],                  // Externally-accessable CRTC register file
    reg_select: CrtcRegister,       // Selected CRTC register

    start_address: u16,             // Calculated value from R12 & R13
    cursor_address: u16,            // Calculated value from R14 & R15
    lightpen_position: u16,         // Calculated value from R16 & R17

    cursor_status: bool,
    cursor_start_line: u8,
    cursor_slow_blink: bool,
    cursor_blink_rate: f64,

    display_enable: bool,           // True if we are in counting in the display area, false otherwise

    hcc_c0: u8,                     // Horizontal character counter (x pos of character)
    vlc_c9: u8,                     // Vertical line counter - counts during vsync period
    vcc_c4: u8,                     // Vertical character counter (y pos of character)
    vsc_c3h: u8,
    hsc_c3l: u8,
    vtac_c5: u8,
    vma: u16,                       // VMA register - Video memory address
    vma_t: u16,                     // VMA' register - Video memory address temporary    

    trace_logger: TraceLogger,    
}

impl Crtc6845 {

    fn new(trace_logger: TraceLogger) -> Self {
        Self {
            reg: [0; 18],
            reg_select: HorizontalTotal,
        
            start_address: 0,
            cursor_address: 0,
            lightpen_position: 0,
        
            cursor_status: false,
            cursor_start_line: 0,
            cursor_slow_blink: false,
            cursor_blink_rate: 0.0,

            display_enable: false,

            hcc_c0: 0,
            vlc_c9: 0,
            vcc_c4: 0,
            vsc_c3h: 0,
            hsc_c3l: 0,
            vtac_c5: 0,
            vma: 0,
            vma_t: 0,

            trace_logger
        }
    }
    pub fn select_register(&mut self, idx: usize) {
        if idx > REGISTER_MAX {
            return
        }

        let reg_select = match idx {
            0  => HorizontalTotal,
            1  => HorizontalDisplayed,
            2  => HorizontalSyncPosition,
            3  => SyncWidth,
            4  => VerticalTotal,
            5  => VerticalTotalAdjust,
            6  => VerticalDisplayed,
            7  => VerticalSync,
            8  => InterlaceMode,
            9  => MaximumScanlineAddress,
            10 => CursorStartLine,
            11 => CursorEndLine,
            12 => StartAddressH,
            13 => StartAddressL,
            14 => CursorAddressH,
            15 => CursorAddressL,
            16 => LightPenPositionH,
            _  => LightPenPositionL,
        };
    }

    pub fn write_register(&mut self, byte: u8) {

        match self.reg_select {
            CrtcRegister::HorizontalTotal => {
                // (R0) 8 bit write only
                self.reg[0] = byte;
            },
            CrtcRegister::HorizontalDisplayed => {
                // (R1) 8 bit write only
                self.reg[1] = byte;
            }
            CrtcRegister::HorizontalSyncPosition => {
                // (R2) 8 bit write only
                self.reg[2] = byte;
            },
            CrtcRegister::SyncWidth => {
                // (R3) 8 bit write only
                self.reg[3] = byte;
            },
            CrtcRegister::VerticalTotal => {
                // (R4) 7 bit write only
                self.reg[4] = byte & 0x7F;

                trace_regs!(self);
                trace!(
                    self,
                    "CRTC Register Write (04h): VerticalTotal updated: {}",
                    self.reg[4]
                )
            },
            CrtcRegister::VerticalTotalAdjust => {
                // (R5) 5 bit write only
                self.reg[5] = byte & 0x1F;
            }
            CrtcRegister::VerticalDisplayed => {
                // (R6) 7 bit write only
                self.reg[6] = byte & 0x7F;
            },
            CrtcRegister::VerticalSync => {
                // (R7) 7 bit write only
                self.reg[7] = byte & 0x7F;

                trace_regs!(self);
                trace!(
                    self,
                    "CRTC Register Write (07h): VerticalSync updated: {}",
                    self.reg[7]
                )
            },
            CrtcRegister::InterlaceMode => {
                // (R8) 2 bit write only
                self.reg[8] = byte & 0x03;
            },            
            CrtcRegister::MaximumScanlineAddress => {
                // (R9) 5 bit write only
                self.reg[9] = byte & 0x1F;
            }            
            CrtcRegister::CursorStartLine => {
                // (R10) 7 bit bitfield. Write only.
                self.reg[10] = byte & 0x7F;

                self.cursor_start_line = byte & CURSOR_LINE_MASK;
                match byte & CURSOR_ATTR_MASK >> 4 {
                    0b00 | 0b10 => {
                        self.cursor_status = true;
                        self.cursor_slow_blink = false;
                    }
                    0b01 => {
                        self.cursor_status = false;
                        self.cursor_slow_blink = false;
                    }
                    _ => {
                        self.cursor_status = true;
                        self.cursor_slow_blink = true;
                    }
                }
            }
            CrtcRegister::CursorEndLine => {
                // (R11) 5 bit write only
                self.reg[11] = byte & 0x1F;
            }
            CrtcRegister::StartAddressH => {
                // (R12) 6 bit write only
                self.reg[12] = byte & 0x3F;
                trace_regs!(self);
                trace!(
                    self,
                    "CRTC Register Write (0Ch): StartAddressH updated: {:02X}",
                    byte
                );
                self.update_start_address();
            }
            CrtcRegister::StartAddressL => {
                // (R13) 8 bit write only
                self.reg[13] = byte;
                trace_regs!(self);
                trace!(
                    self,
                    "CRTC Register Write (0Dh): StartAddressL updated: {:02X}",
                    byte
                );                
                self.update_start_address();
            }
            CrtcRegister::CursorAddressH => {
                // (R14) 6 bit read/write
                self.reg[14] = byte & 0x3F;
                self.update_cursor_address();
            }
            CrtcRegister::CursorAddressL => {
                // (R15) 8 bit read/write
                self.reg[15] = byte;
                self.update_cursor_address();
            }
            CrtcRegister::LightPenPositionH => {
                // (R16) 6 bit read only
            }
            CrtcRegister::LightPenPositionL => {
                // (R17) 8 bit read only
            }                          
        }
        
    }

    pub fn read_register(&self) -> u8 {

        match self.reg_select {
            CursorAddressH | CursorAddressL | LightPenPositionH | LightPenPositionL => {
                self.reg[self.reg_select as usize]
            }
            _ => REGISTER_UNREADABLE_VALUE
        }
    }

    pub fn read_address(&self) -> u16 {
        self.vma
    }

    fn update_start_address(&mut self) {
        self.start_address = (self.reg[12] as u16) << 8 | self.reg[13] as u16
    }  

    fn update_cursor_address(&mut self) {
        self.cursor_address = (self.reg[14] as u16) << 8 | self.reg[15] as u16
    }

    /// Tick the CRTC to the next character.
    fn tick(&mut self) {

        // Update C0
        self.hcc_c0 = self.hcc_c0.wrapping_add(1);
        if self.hcc_c0 == 0 {
            // C0 has wrapped?
            self.hborder = false;
            if self.vcc_c4 == 0 {
                // We are at the first character of a CRTC frame. Update start address.
                self.vma = self.crtc_frame_address;
            }
        }     

        // Advance video memory address offset and grab the next character + attr
        self.vma += 1;
        self.set_char_addr();

        // Glyph column reset to 0 for next char
        self.char_col = 0;

        // Process horizontal blanking period
        if self.in_crtc_hblank {

            // Increment horizontal sync counter (wrapping)

            /*
            if ((self.hsc_c3l + 1) & 0x0F) != self.hsc_c3l.wrapping_add(1) {
                log::warn!("hsc0: {} hsc1: {}", ((self.hsc_c3l + 1) & 0x0F), self.hsc_c3l.wrapping_add(1));
            }
            */

            //self.hsc_c3l = (self.hsc_c3l + 1) & 0x0F;
            self.hsc_c3l = self.hsc_c3l.wrapping_add(1);

            // Implement a fixed hsync width from the monitor's perspective - 
            // A wider programmed hsync width than these values shifts the displayed image to the right.
            let hsync_target = if self.clock_divisor == 1 { 
                std::cmp::min(10, self.crtc_sync_width)
            }
            else {
                5
            };

            // Do a horizontal sync
            if self.hsc_c3l == hsync_target {
                // Update the video mode, if an update is pending.
                // It is important not to change graphics mode while we are catching up during an IO instruction.
                if !self.catching_up && self.mode_pending {
                    self.update_mode();
                    self.mode_pending = false;
                }

                // END OF LOGICAL SCANLINE
                if self.in_crtc_vblank {
                    // If we are in vblank, advance Vertical Sync Counter
                    self.vsc_c3h += 1;
                
                    //if self.vsc_c3h == CRTC_VBLANK_HEIGHT || self.beam_y == CGA_MONITOR_VSYNC_POS {
                    if self.vsc_c3h == CRTC_VBLANK_HEIGHT {

                        self.in_last_vblank_line = true;
                        // We are leaving vblank period. Generate a frame.

                        // Previously, we generated frames upon reaching vertical total. This was convenient as 
                        // the display area would be at the top of the render buffer and both overscan periods
                        // beneath it.
                        // However, CRTC tricks like 8088mph rewrite vertical total; this causes multiple 
                        // 'screens' per frame in between vsyncs. To enable these tricks to work, we must render 
                        // like a monitor would.                        

                        self.vsc_c3h = 0;
                        self.do_vsync();
                        return
                    }                        
                }

                self.scanline += 1;
                
                // Reset beam to left of screen if we haven't already
                if self.beam_x > 0 {
                    self.beam_y += 1;
                }
                self.beam_x = 0;
                self.char_col = 0;

                let new_rba = (CGA_XRES_MAX * self.beam_y) as usize;
                self.rba = new_rba;
            }

            // End horizontal blank when we reach R3
            if self.hsc_c3l == self.crtc_sync_width {
                self.in_crtc_hblank = false;
                self.hsc_c3l = 0;
            }            
        }

        if self.hcc_c0 == self.crtc_horizontal_displayed {
            // C0 == R1. Entering right overscan.

            if self.vlc_c9 == self.crtc_maximum_scanline_address {
                // Save VMA in VMA'
                //log::debug!("Updating vma_t: {:04X}", self.vma_t);
                self.vma_t = self.vma;
            }

            // Save right overscan start position to calculate width of right overscan later
            self.overscan_right_start = self.beam_x;
            self.in_display_area = false;
            self.hborder = true;
        }

        if self.hcc_c0 == self.crtc_horizontal_sync_pos {
            // We entered horizontal blank

            // Save width of right overscan
            if self.beam_x > self.overscan_right_start {
                self.extents[self.front_buf].overscan_r = self.beam_x - self.overscan_right_start;
            }
            self.in_crtc_hblank = true;
            self.hsc_c3l = 0;
        }

        if self.hcc_c0 == self.crtc_horizontal_total && self.in_last_vblank_line {
            // We are one char away from the beginning of the new frame.
            // Draw one char of border
            self.hborder = true;
        }

        if self.hcc_c0 == self.crtc_horizontal_total + 1 {
            // Leaving left overscan, finished scanning row

            if self.in_last_vblank_line {
                self.in_last_vblank_line = false;
                self.in_crtc_vblank = false;
            }

            // Reset Horizontal Character Counter and increment character row counter
            self.hcc_c0 = 0;
            self.hborder = false;
            self.vlc_c9 += 1;
            self.extents[self.front_buf].overscan_l = self.beam_x;
            // Return video memory address to starting position for next character row
            self.vma = self.vma_t;
            
            // Reset the current character glyph to start of row
            self.set_char_addr();

            if !self.in_crtc_vblank {
                // Start the new row
                if self.vcc_c4 < self.crtc_vertical_displayed {
                    self.in_display_area = true;
                }
            }
            
            if self.vlc_c9 > self.crtc_maximum_scanline_address  {
                // C9 == R9 We finished drawing this row of characters 

                self.vlc_c9 = 0;
                // Advance Vertical Character Counter
                self.vcc_c4 = self.vcc_c4.wrapping_add(1);

                // Set vma to starting position for next character row
                //self.vma = (self.vcc_c4 as usize) * (self.crtc_horizontal_displayed as usize) + self.crtc_frame_address;
                self.vma = self.vma_t;
                
                // Load next char + attr
                self.set_char_addr();

                if self.vcc_c4 == self.crtc_vertical_sync_pos {
                    // We've reached vertical sync
                    trace_regs!(self);
                    trace!(self, "Entering vsync");
                    self.in_crtc_vblank = true;
                    self.in_display_area = false;
                }
            }

            if self.vcc_c4 == self.crtc_vertical_displayed {
                // Enter lower overscan area.
                // This represents reaching the lowest visible scanline, so save the scanline in extents.
                self.extents[self.front_buf].visible_h = self.scanline;
                self.in_display_area = false;
                self.vborder = true;
            }
            
            /*
            if self.vcc_c4 >= (self.crtc_vertical_total + 1)  {

                // We are at vertical total, start incrementing vertical total adjust counter.
                self.vtac_c5 += 1;

                if self.vtac_c5 > self.crtc_vertical_total_adjust {
                    // We have reached vertical total adjust. We are at the end of the top overscan.
                    self.hcc_c0 = 0;
                    self.vcc_c4 = 0;
                    self.vtac_c5 = 0;
                    self.vlc_c9 = 0;
                    self.char_col = 0;                            
                    self.crtc_frame_address = self.crtc_start_address;
                    self.vma = self.crtc_start_address;
                    self.vma_t = self.vma;
                    self.in_display_area = true;
                    self.vborder = false;
                    self.in_crtc_vblank = false;
                    
                    // Load first char + attr
                    self.set_char_addr();     
                }
            }
            */

            if self.vcc_c4 == self.crtc_vertical_total + 1 {
                // We are at vertical total, start incrementing vertical total adjust counter.
                self.in_vta = true;
            }

            if self.in_vta {
                // We are in vertical total adjust.
                self.vtac_c5 += 1;
                
                if self.vtac_c5 > self.crtc_vertical_total_adjust {
                    // We have reached vertical total adjust. We are at the end of the top overscan.
                    self.in_vta = false;
                    self.vtac_c5 = 0;

                    self.hcc_c0 = 0;
                    self.vcc_c4 = 0;
                    self.vlc_c9 = 0;
                    self.char_col = 0;                            
                    self.crtc_frame_address = self.crtc_start_address;
                    self.vma = self.crtc_start_address;
                    self.vma_t = self.vma;
                    self.in_display_area = true;
                    self.vborder = false;
                    self.in_crtc_vblank = false;
                    
                    // Load first char + attr
                    self.set_char_addr();     
                }
            }
        }   
    }
}
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

    devices::mc6845.rs

    Implementation of the Motorola MC6845 CRT controller.
    Used internally by the MDA and CGA video cards.

*/

use crate::{device_traits::videocard::VideoCardStateEntry, tracelogger::TraceLogger};

const CURSOR_LINE_MASK: u8 = 0b0000_1111;
const CURSOR_ATTR_MASK: u8 = 0b0011_0000;

const BLINK_FAST_RATE: u8 = 8;
const BLINK_SLOW_RATE: u8 = 16;

const CRTC_VBLANK_HEIGHT: u8 = 16;
const CRTC_ROW_MAX: usize = 32;

const REGISTER_MAX: usize = 17;
const REGISTER_UNREADABLE_VALUE: u8 = 0xFF;

#[derive(Copy, Clone, Debug)]
pub enum CursorStatus {
    Solid,
    Hidden,
    Blink,
    SlowBlink,
}

#[derive(Copy, Clone, Debug)]
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

use crate::devices::mc6845::CrtcRegister::*;

macro_rules! trace {
    ($self:ident, $($t:tt)*) => {{
        $self.trace_logger.print(&format!($($t)*));
        $self.trace_logger.print("\n".to_string());
    }};
}

macro_rules! trace_regs {
    ($self:ident) => {
        $self.trace_logger.print(
            &format!(""), /*
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

// Helper macro for pushing video card state entries.
macro_rules! push_reg_str {
    ($vec: expr, $reg: expr, $decorator: expr, $val: expr ) => {
        $vec.push((
            format!("{} {:?}", $decorator, $reg),
            VideoCardStateEntry::String(format!("{}", $val)),
        ))
    };
}

pub type HBlankCallback = dyn FnMut() -> u8;

#[derive(Copy, Clone, Default, Debug)]
pub struct CrtcStatus {
    pub hblank: bool,
    pub vblank: bool,
    pub den: bool,
    pub hborder: bool,
    pub vborder: bool,
    pub cursor: bool,
    pub hsync: bool,
    pub vsync: bool,
}

pub struct Crtc6845 {
    pub reg:    [u8; 18],     // Externally-accessible CRTC register file
    reg_select: CrtcRegister, // Selected CRTC register

    start_address: u16,       // Calculated value from R12 & R13
    start_address_latch: u16, // start address, latched per frame
    lightpen_position: u16,   // Calculated value from R16 & R17

    cursor_data: [bool; CRTC_ROW_MAX],
    cursor_address: u16, // Calculated value from R14 & R15
    cursor_enabled: bool,
    cursor_start_line: u8,
    cursor_end_line: u8,
    blink_state: bool,
    cursor_blink_ct: u8,
    cursor_blink_rate: Option<u8>,

    display_enable: bool, // True if we are in counting in the display area, false otherwise

    hcc_c0: u8,   // Horizontal character counter (x pos of character)
    char_col: u8, // Character column counter (x pos of bit in glyph)
    vlc_c9: u8,   // Vertical line counter - counts during vsync period
    vcc_c4: u8,   // Vertical character counter (y pos of character)
    vsc_c3h: u8,
    hsc_c3l: u8,
    vtac_c5: u8,
    in_vta: bool,
    vma: u16,   // VMA register - Video memory address
    vma_t: u16, // VMA' register - Video memory address temporary

    hsync_target: u8,
    status: CrtcStatus,
    in_last_vblank_line: bool,

    trace_logger: TraceLogger,
}

impl Crtc6845 {
    pub(crate) fn new(trace_logger: TraceLogger) -> Self {
        Self {
            reg: [0; 18],
            reg_select: HorizontalTotal,

            start_address: 0,
            start_address_latch: 0,
            lightpen_position: 0,

            cursor_data: [false; CRTC_ROW_MAX],
            cursor_address: 0,
            cursor_enabled: false,
            cursor_start_line: 0,
            cursor_end_line: 0,
            blink_state: false,
            cursor_blink_ct: 0,
            cursor_blink_rate: Some(BLINK_FAST_RATE),

            display_enable: false,

            hcc_c0: 0,
            char_col: 0,
            vlc_c9: 0,
            vcc_c4: 0,
            vsc_c3h: 0,
            hsc_c3l: 0,
            vtac_c5: 0,
            in_vta: false,
            vma: 0,
            vma_t: 0,

            hsync_target: 0,

            status: Default::default(),
            in_last_vblank_line: false,

            trace_logger,
        }
    }

    pub fn port_write(&mut self, port: u16, data: u8) {
        match port & 0x01 {
            0 => {
                // CRTC register select
                self.select_register(data as usize);
            }
            1 => {
                // CRTC register write
                self.write_register(data);
            }
            _ => {}
        }
    }

    pub fn port_read(&mut self, port: u16) -> u8 {
        match port & 0x01 {
            0 => {
                // CRTC address register is not readable
                0xFF
            }
            1 => {
                // CRTC data register is partially readable (depends on register selected)
                self.read_register()
            }
            _ => 0xFF,
        }
    }

    fn select_register(&mut self, idx: usize) {
        if idx > REGISTER_MAX {
            return;
        }

        self.reg_select = match idx {
            0 => HorizontalTotal,
            1 => HorizontalDisplayed,
            2 => HorizontalSyncPosition,
            3 => SyncWidth,
            4 => VerticalTotal,
            5 => VerticalTotalAdjust,
            6 => VerticalDisplayed,
            7 => VerticalSync,
            8 => InterlaceMode,
            9 => MaximumScanlineAddress,
            10 => CursorStartLine,
            11 => CursorEndLine,
            12 => StartAddressH,
            13 => StartAddressL,
            14 => CursorAddressH,
            15 => CursorAddressL,
            16 => LightPenPositionH,
            _ => LightPenPositionL,
        };
        log::debug!("CRTC register selected: {:?}", self.reg_select);
    }

    fn write_register(&mut self, byte: u8) {
        log::warn!("crtc write register: {:02X}", byte);
        match self.reg_select {
            CrtcRegister::HorizontalTotal => {
                // (R0) 8 bit write only
                self.reg[0] = byte;
            }
            CrtcRegister::HorizontalDisplayed => {
                // (R1) 8 bit write only
                self.reg[1] = byte;
            }
            CrtcRegister::HorizontalSyncPosition => {
                // (R2) 8 bit write only
                self.reg[2] = byte;
            }
            CrtcRegister::SyncWidth => {
                // (R3) 8 bit write only
                self.reg[3] = byte;
            }
            CrtcRegister::VerticalTotal => {
                // (R4) 7 bit write only
                self.reg[4] = byte & 0x7F;

                trace_regs!(self);
                trace!(
                    self,
                    "CRTC Register Write (04h): VerticalTotal updated: {}",
                    self.reg[4]
                )
            }
            CrtcRegister::VerticalTotalAdjust => {
                // (R5) 5 bit write only
                self.reg[5] = byte & 0x1F;
            }
            CrtcRegister::VerticalDisplayed => {
                // (R6) 7 bit write only
                self.reg[6] = byte & 0x7F;
            }
            CrtcRegister::VerticalSync => {
                // (R7) 7 bit write only
                self.reg[7] = byte & 0x7F;

                trace_regs!(self);
                trace!(self, "CRTC Register Write (07h): VerticalSync updated: {}", self.reg[7])
            }
            CrtcRegister::InterlaceMode => {
                // (R8) 2 bit write only
                self.reg[8] = byte & 0x03;
            }
            CrtcRegister::MaximumScanlineAddress => {
                // (R9) 5 bit write only
                self.reg[9] = byte & 0x1F;
            }
            CrtcRegister::CursorStartLine => {
                // (R10) 7 bit bitfield. Write only.
                self.reg[10] = byte & 0x7F;

                self.cursor_start_line = byte & CURSOR_LINE_MASK;
                match byte & CURSOR_ATTR_MASK >> 4 {
                    0b00 => {
                        self.cursor_enabled = true;
                        self.cursor_blink_rate = None;
                    }

                    0b01 => {
                        self.cursor_enabled = false;
                        // We can disable cursor blink on the CRTC chip, but some cards like the IBM CGA
                        // will stubbornly continue blinking.
                        self.cursor_blink_rate = None;
                    }
                    0b10 => {
                        self.cursor_enabled = true;
                        self.cursor_blink_rate = Some(BLINK_FAST_RATE);
                    }
                    _ => {
                        self.cursor_enabled = true;
                        self.cursor_blink_rate = Some(BLINK_SLOW_RATE);
                    }
                }
                self.update_cursor_data();
            }
            CrtcRegister::CursorEndLine => {
                // (R11) 5 bit write only
                self.reg[11] = byte & 0x1F;
                self.update_cursor_data();
            }
            CrtcRegister::StartAddressH => {
                // (R12) 6 bit write only
                self.reg[12] = byte & 0x3F;
                trace_regs!(self);
                trace!(self, "CRTC Register Write (0Ch): StartAddressH updated: {:02X}", byte);
                self.update_start_address();
            }
            CrtcRegister::StartAddressL => {
                // (R13) 8 bit write only
                self.reg[13] = byte;
                trace_regs!(self);
                trace!(self, "CRTC Register Write (0Dh): StartAddressL updated: {:02X}", byte);
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
            _ => REGISTER_UNREADABLE_VALUE,
        }
    }

    #[inline]
    pub fn start_address(&self) -> u16 {
        self.start_address_latch
    }

    #[inline]
    pub fn address(&self) -> u16 {
        self.vma
    }

    #[inline]
    pub fn vlc(&self) -> u8 {
        self.vlc_c9
    }

    pub fn status(&self) -> &CrtcStatus {
        &self.status
    }

    fn update_start_address(&mut self) {
        self.start_address = (self.reg[12] as u16) << 8 | self.reg[13] as u16
    }

    fn update_cursor_address(&mut self) {
        self.cursor_address = (self.reg[14] as u16) << 8 | self.reg[15] as u16
    }

    /// Update the cursor data array based on the values of cursor_start_line and cursor_end_line.
    fn update_cursor_data(&mut self) {
        // Reset cursor data to 0.
        self.cursor_data.fill(false);

        if self.reg[10] <= self.reg[11] {
            // Normal cursor definition. Cursor runs from R10 CursorStartLine to R11 CursorEndLine
            for i in self.reg[10]..=self.reg[11] {
                self.cursor_data[i as usize] = true;
            }
            self.cursor_start_line = self.reg[10];
            self.cursor_end_line = self.reg[11];
        }
        else {
            // "Split" cursor.
            for i in 0..=self.reg[11] {
                // First part of cursor is 0->R11 CursorEndLine
                self.cursor_data[i as usize] = true;
            }

            for i in (self.reg[10] as usize)..CRTC_ROW_MAX {
                // Second part of cursor is R10 CursorStartLine->max
                self.cursor_data[i] = true;
                self.cursor_start_line = self.reg[10];
                self.cursor_end_line = CRTC_ROW_MAX as u8 - 1;
            }
        }
    }

    #[inline]
    pub fn cursor_address(&self) -> u16 {
        self.cursor_address
    }

    pub fn cursor_extents(&self) -> (u8, u8) {
        (self.cursor_start_line, self.cursor_end_line)
    }

    /// Return the immediate cursor status as a tuple. Refle
    #[inline]
    pub fn cursor(&self) -> bool {
        if let Some(_) = self.cursor_blink_rate {
            self.cursor_enabled
                && self.blink_state
                && (self.vma == self.cursor_address)
                && self.cursor_data[(self.vlc_c9 & 0x1F) as usize]
        }
        else {
            self.cursor_enabled
        }
    }

    #[inline]
    pub fn cursor_status(&self) -> bool {
        self.cursor_enabled
    }

    #[inline]
    pub fn hblank(&self) -> bool {
        self.status.hblank
    }

    #[inline]
    pub fn vblank(&self) -> bool {
        self.status.vblank
    }

    #[inline]
    pub fn den(&self) -> bool {
        self.status.den
    }

    #[inline]
    pub fn border(&self) -> bool {
        self.status.hborder | self.status.vborder
    }

    /// Tick the CRTC to the next character.
    pub fn tick(&mut self, hblank_callback: &mut HBlankCallback) -> (&CrtcStatus, u16) {
        // Reset hsync and vsync status. These are transient status flags.
        self.status.hsync = false;
        self.status.vsync = false;

        // Update C0
        self.hcc_c0 = self.hcc_c0.wrapping_add(1);
        if self.hcc_c0 == 0 {
            // C0 has wrapped?
            self.status.hborder = false;
            if self.vcc_c4 == 0 {
                // We are at the first character of a CRTC frame. Update start address.
                self.vma = self.start_address_latch;
            }
        }

        // Advance video memory address offset and grab the next character + attr
        self.vma += 1;
        //self.set_char_addr();

        // Glyph column reset to 0 for next char
        self.char_col = 0;

        // Process horizontal blanking period
        if self.status.hblank {
            // Increment horizontal sync counter (wrapping)
            self.hsc_c3l = self.hsc_c3l.wrapping_add(1);

            // This is our one concession from a "pure" CRTC implementation. We allow the card implementation to drive
            // the effective hsync width based on the current dot and character clocks.
            self.hsync_target = std::cmp::min(hblank_callback(), self.reg[3]);

            if self.hsc_c3l == self.hsync_target {
                // C3L == our "effective" hsync width. Do a horizontal sync.

                // Update the video mode, if an update is pending.
                // // It is important not to change graphics mode while we are catching up during an IO instruction.
                // if !self.catching_up && self.mode_pending {
                //     self.update_mode();
                //     self.mode_pending = false;
                // }

                // END OF LOGICAL SCANLINE
                if self.status.vblank {
                    // If we are in vblank, advance Vertical Sync Counter
                    self.vsc_c3h += 1;
                    if self.vsc_c3h == CRTC_VBLANK_HEIGHT {
                        // We are leaving vblank period. Call the vsync callback to generate a frame.
                        self.in_last_vblank_line = true;
                        self.vsc_c3h = 0;
                        self.status.vsync = true;
                        return (&self.status, self.vma);
                    }
                }

                // We are leaving horizontal blanking period.
                self.char_col = 0;
                self.status.hsync = true;

                // self.scanline += 1;
                // // Reset beam to left of screen if we haven't already
                // if self.beam_x > 0 {
                //     self.beam_y += 1;
                // }
                // self.beam_x = 0;
                // let new_rba = (CGA_XRES_MAX * self.beam_y) as usize;
                // self.rba = new_rba;
            }

            // End horizontal blank when we reach R3 (SyncWidth)
            if self.hsc_c3l == self.reg[3] {
                self.status.hblank = false;
                self.hsc_c3l = 0;
            }
        }

        if self.hcc_c0 == self.reg[1] {
            // C0 == R1 (HorizontalDisplayed): Entering right overscan.
            if self.vlc_c9 == self.reg[9] {
                // C9 == R9 (MaximumScanlineAddress): We are at the last character row
                // Save VMA in VMA'
                self.vma_t = self.vma;
            }
            self.status.den = false;
            self.status.hborder = true;
        }

        if self.hcc_c0 == self.reg[2] {
            // C0 == R2 (HorizontalSyncPos) We entered horizontal blank.
            // Retrieve our hsync target from the hblank callback. This allows a card implementation to drive the
            // effective hsync width based on the current dot and character clocks.
            self.hsync_target = hblank_callback();
            self.status.hblank = true;
            self.hsc_c3l = 0;
        }

        if self.hcc_c0 == self.reg[0] && self.in_last_vblank_line {
            // C0 == R0 (HorizontalTotal): We are one char away from the beginning of the new frame.
            // Draw one char of border
            self.status.hborder = true;
        }

        if self.hcc_c0 == self.reg[0] + 1 {
            // C0 == R0 (HorizontalTotal): Leaving left overscan, finished scanning row
            if self.in_last_vblank_line {
                self.in_last_vblank_line = false;
                self.status.vblank = false;
            }

            // Reset Horizontal Character Counter and increment character row counter
            self.hcc_c0 = 0;
            self.status.hborder = false;
            self.vlc_c9 += 1;
            // Return video memory address to starting position for next character row
            self.vma = self.vma_t;

            // Reset the current character glyph to start of row
            //self.set_char_addr();

            if !self.status.vblank {
                // Start the new row
                if self.vcc_c4 < self.reg[6] {
                    // C4 < R6 (VerticalDisplayed): We are in the display area
                    self.status.den = true;
                }
            }

            if self.vlc_c9 > self.reg[9] {
                // C9 == R9 (MaxScanlineAddress): We finished drawing this row of characters
                self.vlc_c9 = 0;
                // Advance Vertical Character Counter
                self.vcc_c4 = self.vcc_c4.wrapping_add(1);
                // Set vma to starting position for next character row
                self.vma = self.vma_t;

                // Load next char + attr
                //self.set_char_addr();

                if self.vcc_c4 == self.reg[7] {
                    // C4 == R7 (VerticalSyncPos): We've reached vertical sync
                    trace_regs!(self);
                    trace!(self, "Entering vsync");
                    self.status.vblank = true;
                    self.status.den = false;

                    if let Some(rate) = self.cursor_blink_rate {
                        self.cursor_blink_ct = self.cursor_blink_ct.wrapping_add(1);
                        if self.cursor_blink_ct == rate {
                            self.cursor_blink_ct = 0;
                            self.blink_state = !self.blink_state;
                        }
                    }
                }
            }

            if self.vcc_c4 == self.reg[6] {
                // C4 == R6 (VerticalDisplayed): Entering lower overscan area.
                self.status.den = false;
                self.status.vborder = true;
            }

            if self.vcc_c4 == self.reg[4] + 1 {
                // C4 == R4 (VerticalTotal): Reached VerticalTotal, start incrementing vertical total adjust counter.
                self.in_vta = true;
            }

            if self.in_vta {
                // We are in vertical total adjust. Increment vtac counter.
                self.vtac_c5 += 1;

                if self.vtac_c5 > self.reg[5] {
                    // C5 == R5 (VerticalTotalAdjust): We are at the end of the top overscan.
                    self.in_vta = false;
                    self.vtac_c5 = 0;
                    self.hcc_c0 = 0;
                    self.vcc_c4 = 0;
                    self.vlc_c9 = 0;
                    self.char_col = 0;
                    self.start_address_latch = self.start_address;
                    self.vma = self.start_address;
                    self.vma_t = self.vma;
                    self.status.den = true;
                    self.status.vborder = false;
                    self.status.vblank = false;

                    // Load first char + attr
                    //self.set_char_addr();
                }
            }
        }

        self.status.cursor = self.cursor();

        (&self.status, self.vma)
    }

    #[rustfmt::skip]
    pub fn get_reg_state(&self) -> Vec<(String, VideoCardStateEntry)> {
        let mut crtc_vec = Vec::new();

        push_reg_str!(crtc_vec, HorizontalTotal, "[R0]", self.reg[0]);
        push_reg_str!(crtc_vec, HorizontalDisplayed, "[R1]", self.reg[1]);
        push_reg_str!(crtc_vec, HorizontalSyncPosition, "[R2]", self.reg[2]);
        push_reg_str!(crtc_vec, SyncWidth, "[R3]", self.reg[3]);
        push_reg_str!(crtc_vec, VerticalTotal, "[R4]", self.reg[4]);
        push_reg_str!(crtc_vec, VerticalTotalAdjust, "[R5]", self.reg[5]);
        push_reg_str!(crtc_vec, VerticalDisplayed, "[R6]", self.reg[6]);
        push_reg_str!(crtc_vec, VerticalSync, "[R7]", self.reg[7]);
        push_reg_str!(crtc_vec, InterlaceMode, "[R8]", self.reg[8]);
        push_reg_str!(crtc_vec, MaximumScanlineAddress, "[R9]", self.reg[9]);
        push_reg_str!(crtc_vec, CursorStartLine, "[R10]", self.reg[10]);
        push_reg_str!(crtc_vec, CursorEndLine, "[R11]", self.reg[11]);
        push_reg_str!(crtc_vec, StartAddressH, "[R12]", self.reg[12]);
        push_reg_str!(crtc_vec, StartAddressL, "[R13]", self.reg[13]);
        crtc_vec.push(("Start Address".to_string(), VideoCardStateEntry::String(format!("{:04X}", self.start_address))));
        push_reg_str!(crtc_vec, CursorAddressH, "[R14]", self.reg[14]);
        push_reg_str!(crtc_vec, CursorAddressL, "[R15]", self.reg[15]);

        crtc_vec
    }
}

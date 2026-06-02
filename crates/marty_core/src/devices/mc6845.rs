/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

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

use std::ops::{Index, IndexMut};

use crate::{device_traits::videocard::VideoCardStateEntry, tracelogger::TraceLogger};
use strum::EnumCount;

const CURSOR_LINE_MASK: u8 = 0b0000_1111;
const CURSOR_ATTR_MASK: u8 = 0b0011_0000;

const BLINK_FAST_RATE: u8 = 8;
const BLINK_SLOW_RATE: u8 = 16;

const CRTC_VBLANK_HEIGHT: u8 = 16;
const CRTC_ROW_MAX: usize = 32;
const HORIZONTAL_SYNC_WIDTH_MASK: u8 = 0x0F;

const REGISTER_UNREADABLE_VALUE: u8 = 0xFF;

#[derive(Copy, Clone, Debug)]
pub enum CursorStatus {
    Solid,
    Hidden,
    Blink,
    SlowBlink,
}

#[derive(Copy, Clone, Debug, PartialEq, strum_macros::EnumCount)]
#[repr(usize)]
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

impl TryFrom<usize> for CrtcRegister {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(HorizontalTotal),
            1 => Ok(HorizontalDisplayed),
            2 => Ok(HorizontalSyncPosition),
            3 => Ok(SyncWidth),
            4 => Ok(VerticalTotal),
            5 => Ok(VerticalTotalAdjust),
            6 => Ok(VerticalDisplayed),
            7 => Ok(VerticalSync),
            8 => Ok(InterlaceMode),
            9 => Ok(MaximumScanlineAddress),
            10 => Ok(CursorStartLine),
            11 => Ok(CursorEndLine),
            12 => Ok(StartAddressH),
            13 => Ok(StartAddressL),
            14 => Ok(CursorAddressH),
            15 => Ok(CursorAddressL),
            16 => Ok(LightPenPositionH),
            17 => Ok(LightPenPositionL),
            _ => Err(()),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct CrtcRegisterFile([u8; CrtcRegister::COUNT]);

impl Default for CrtcRegisterFile {
    fn default() -> Self {
        Self([0; CrtcRegister::COUNT])
    }
}

impl Index<CrtcRegister> for CrtcRegisterFile {
    type Output = u8;

    fn index(&self, reg: CrtcRegister) -> &Self::Output {
        &self.0[reg as usize]
    }
}

impl IndexMut<CrtcRegister> for CrtcRegisterFile {
    fn index_mut(&mut self, reg: CrtcRegister) -> &mut Self::Output {
        &mut self.0[reg as usize]
    }
}

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

#[derive(Copy, Clone, Default, Debug)]
pub struct CrtcStatus {
    pub den: bool, // Display Enable. True if we are in counting in the display area, false otherwise
    pub hborder: bool,
    pub vborder: bool,
    pub cursor: bool,
    pub hsync: bool,
    pub vsync: bool,
}

pub struct Crtc6845 {
    pub reg:    CrtcRegisterFile, // Externally-accessible CRTC register file
    reg_select: CrtcRegister,     // Selected CRTC register

    start_address: u16,       // Calculated value from R12 & R13
    start_address_latch: u16, // start address, latched per frame
    lightpen_position: u16,   // Calculated value from R16 & R17

    cursor_address: u16, // Calculated value from R14 & R15
    cursor_enabled: bool,
    cursor_start_line: u8,
    cursor_end_line: u8,
    cursor_active: bool, // Whether cursor is between cursor_start_line and curse_end_line
    blink_state: bool,
    cursor_blink_ct: u8,
    cursor_blink_rate: Option<u8>,

    hcc_c0: u8,   // Horizontal character counter (x pos of character)
    char_col: u8, // Character column counter (x pos of bit in glyph)
    vlc_c9: u8,   // Vertical line counter - counts during vsync period
    vcc_c4: u8,   // Vertical character counter (y pos of character)
    last_row: bool,
    last_line: bool,
    vsc_c3h: u8,
    hsc_c3l: u8,
    vtac_c5: u8,
    in_vta: bool,
    vma: u16,   // VMA register - Video memory address
    vma_t: u16, // VMA' register - Video memory address temporary

    status: CrtcStatus,
    in_hblank: bool,
    in_vblank: bool,
    in_display_rows: bool,
    in_last_vblank_line: bool,

    trace_logger: TraceLogger,
}

impl Crtc6845 {
    pub fn new(trace_logger: TraceLogger) -> Self {
        Self {
            reg: Default::default(),
            reg_select: HorizontalTotal,

            start_address: 0,
            start_address_latch: 0,
            lightpen_position: 0,

            cursor_address: 0,
            cursor_enabled: false,
            cursor_start_line: 0,
            cursor_end_line: 0,
            cursor_active: false,
            blink_state: false,
            cursor_blink_ct: 0,
            cursor_blink_rate: Some(BLINK_FAST_RATE),

            hcc_c0: 0,
            char_col: 0,
            vlc_c9: 0,
            vcc_c4: 0,
            last_row: false,
            last_line: false,
            vsc_c3h: 0,
            hsc_c3l: 0,
            vtac_c5: 0,
            in_vta: false,
            vma: 0,
            vma_t: 0,

            status: Default::default(),
            in_hblank: false,
            in_vblank: false,
            in_display_rows: false,
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

    pub fn select_register(&mut self, idx: usize) {
        let Ok(reg) = CrtcRegister::try_from(idx)
        else {
            return;
        };

        self.reg_select = reg;
        //log::trace!("CRTC register selected: {:?}", self.reg_select);
    }

    pub fn write_register_direct(&mut self, reg: CrtcRegister, byte: u8) {
        self.reg_select = reg;
        self.write_register(byte);
    }

    pub fn write_register(&mut self, byte: u8) {
        //log::trace!("crtc write register: {:02X}", byte);
        match self.reg_select {
            HorizontalTotal => {
                // (R0) 8 bit write only
                self.reg[HorizontalTotal] = byte;
            }
            HorizontalDisplayed => {
                // (R1) 8 bit write only
                self.reg[HorizontalDisplayed] = byte;
            }
            HorizontalSyncPosition => {
                // (R2) 8 bit write only
                self.reg[HorizontalSyncPosition] = byte;
            }
            SyncWidth => {
                // (R3) 8 bit write only
                self.reg[SyncWidth] = byte;
            }
            VerticalTotal => {
                // (R4) 7 bit write only
                self.reg[VerticalTotal] = byte & 0x7F;

                trace_regs!(self);
                trace!(
                    self,
                    "CRTC Register Write (04h): VerticalTotal updated: {}",
                    self.reg[VerticalTotal]
                )
            }
            VerticalTotalAdjust => {
                // (R5) 5 bit write only
                self.reg[VerticalTotalAdjust] = byte & 0x1F;
            }
            VerticalDisplayed => {
                // (R6) 7 bit write only
                self.reg[VerticalDisplayed] = byte & 0x7F;
            }
            VerticalSync => {
                // (R7) 7 bit write only
                self.reg[VerticalSync] = byte & 0x7F;

                trace_regs!(self);
                trace!(
                    self,
                    "CRTC Register Write (07h): VerticalSync updated: {}",
                    self.reg[VerticalSync]
                )
            }
            InterlaceMode => {
                // (R8) 2 bit write only
                self.reg[InterlaceMode] = byte & 0x03;
            }
            MaximumScanlineAddress => {
                // (R9) 5 bit write only
                self.reg[MaximumScanlineAddress] = byte & 0x1F;
            }
            CursorStartLine => {
                // (R10) 7 bit bitfield. Write only.
                self.reg[CursorStartLine] = byte & 0x7F;

                self.cursor_start_line = byte & CURSOR_LINE_MASK;
                match (byte & CURSOR_ATTR_MASK) >> 5 {
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
            }
            CursorEndLine => {
                // (R11) 5 bit write only
                self.reg[CursorEndLine] = byte & 0x1F;
            }
            StartAddressH => {
                // (R12) 6 bit write only
                self.reg[StartAddressH] = byte & 0x3F;
                trace_regs!(self);
                trace!(self, "CRTC Register Write (0Ch): StartAddressH updated: {:02X}", byte);
                self.update_start_address();
            }
            StartAddressL => {
                // (R13) 8 bit write only
                self.reg[StartAddressL] = byte;
                trace_regs!(self);
                trace!(self, "CRTC Register Write (0Dh): StartAddressL updated: {:02X}", byte);
                self.update_start_address();
            }
            CursorAddressH => {
                // (R14) 6 bit read/write
                self.reg[CursorAddressH] = byte & 0x3F;
                self.update_cursor_address();
            }
            CursorAddressL => {
                // (R15) 8 bit read/write
                self.reg[CursorAddressL] = byte;
                self.update_cursor_address();
            }
            LightPenPositionH => {
                // (R16) 6 bit read only
            }
            LightPenPositionL => {
                // (R17) 8 bit read only
            }
        }
    }

    pub fn read_register(&self) -> u8 {
        match self.reg_select {
            CursorAddressH | CursorAddressL | LightPenPositionH | LightPenPositionL => self.reg[self.reg_select],
            _ => REGISTER_UNREADABLE_VALUE,
        }
    }

    #[inline]
    fn horizontal_sync_width(&self) -> u8 {
        self.reg[SyncWidth] & HORIZONTAL_SYNC_WIDTH_MASK
    }

    #[inline]
    pub fn start_address_latch(&self) -> u16 {
        self.start_address_latch
    }

    #[inline]
    pub fn start_address(&self) -> u16 {
        self.start_address
    }

    #[inline]
    pub fn address(&self) -> u16 {
        self.vma
    }

    #[inline]
    pub fn vlc(&self) -> u8 {
        self.vlc_c9
    }

    #[inline]
    pub fn hcc(&self) -> u8 {
        self.hcc_c0
    }

    #[inline]
    pub fn vcc(&self) -> u8 {
        self.vcc_c4
    }

    #[inline]
    pub fn hsc(&self) -> u8 {
        self.hsc_c3l
    }

    #[inline]
    pub fn vsc(&self) -> u8 {
        self.vsc_c3h
    }

    #[inline]
    pub fn vtac(&self) -> u8 {
        self.vtac_c5
    }

    #[inline]
    pub fn last_row(&self) -> bool {
        self.last_row
    }

    #[inline]
    pub fn last_line(&self) -> bool {
        self.last_line
    }

    #[inline]
    pub fn in_vta(&self) -> bool {
        self.in_vta
    }

    pub fn status(&self) -> &CrtcStatus {
        &self.status
    }

    #[inline]
    pub fn maximum_scanline(&self) -> u8 {
        self.reg[MaximumScanlineAddress]
    }

    pub fn latch_lightpen(&mut self) {
        self.lightpen_position = self.vma;
        self.reg[LightPenPositionH] = ((self.lightpen_position >> 8) & 0x3F) as u8;
        self.reg[LightPenPositionL] = (self.lightpen_position & 0xFF) as u8;
    }

    fn update_start_address(&mut self) {
        self.start_address = (self.reg[StartAddressH] as u16) << 8 | self.reg[StartAddressL] as u16
    }

    fn update_cursor_address(&mut self) {
        self.cursor_address = (self.reg[CursorAddressH] as u16) << 8 | self.reg[CursorAddressL] as u16
    }

    // Update the cursor data array based on the values of R9, R10 and R11.
    // fn update_cursor_data(&mut self) {
    //     // Reset cursor data to 0.
    //     self.cursor_data.fill(false);
    //     self.cursor_start_line = self.reg[CursorStartLine] & CURSOR_LINE_MASK;
    //
    //     if self.cursor_start_line > self.reg[MaximumScanlineAddress] {
    //         // R10 CursorStartLine must be hit during row scan-out to start drawing a cursor.
    //         // Therefore if R10 is > R9, the cursor will never be drawn.
    //         return;
    //     }
    //
    //     if self.cursor_start_line <= self.reg[CursorEndLine] {
    //         // Normal cursor definition. Cursor runs from R10 CursorStartLine to R11 CursorEndLine
    //         for i in self.cursor_start_line..=self.reg[CursorEndLine] {
    //             self.cursor_data[i as usize] = true;
    //         }
    //         self.cursor_end_line = self.reg[CursorEndLine];
    //     }
    //     else {
    //         // "Split" cursor.
    //         for i in 0..=self.reg[CursorEndLine] {
    //             // First part of cursor is 0->R11 CursorEndLine
    //             self.cursor_data[i as usize] = true;
    //         }
    //
    //         for i in (self.cursor_start_line as usize)..CRTC_ROW_MAX {
    //             // Second part of cursor is R10 CursorStartLine->max
    //             self.cursor_data[i] = true;
    //             self.cursor_end_line = CRTC_ROW_MAX as u8 - 1;
    //         }
    //     }
    // }

    #[inline]
    pub fn cursor_address(&self) -> u16 {
        self.cursor_address
    }

    pub fn cursor_extents(&self) -> (u8, u8) {
        (self.cursor_start_line, self.cursor_end_line)
    }

    /// Return the immediate cursor status for the current character clock
    #[inline]
    pub fn cursor(&self) -> bool {
        let mut cursor = self.cursor_enabled && self.cursor_active && (self.vma == self.cursor_address);

        if self.cursor_blink_rate.is_some() {
            cursor &= self.blink_state;
        }
        cursor
    }

    #[inline]
    pub fn cursor_status(&self) -> bool {
        self.cursor_enabled
    }

    #[inline]
    pub fn hsync(&self) -> bool {
        self.status.hsync
    }

    #[inline]
    pub fn vsync(&self) -> bool {
        self.status.vsync
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
    pub fn tick(&mut self) -> (&CrtcStatus, u16) {
        if self.hcc_c0 == 0 && self.vcc_c4 == 0 {
            // We are at the first character of a CRTC frame. Update start address.
            self.status.den = true;
            self.in_display_rows = true;
            self.vma = self.start_address_latch;
        }

        if self.hcc_c0 < 2 {
            // START-OF-LINE processing.
            // Turn cursor on.
            if self.vlc_c9 == self.reg[CursorStartLine] {
                self.cursor_active = true;
            }

            // The vertical-total decision is sampled early in the scanline and
            // consumed when that scanline completes. This preserves behavior when
            // software rewrites vertical timing registers during the line.
            if self.vcc_c4 == self.reg[VerticalTotal] {
                self.last_row = true;
                self.last_line = self.vlc_c9 == self.reg[MaximumScanlineAddress];
                self.vtac_c5 = 0;
            }
            else {
                self.last_line = false;
            }
        }

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
        if self.in_hblank {
            // The 6845 uses a 4-bit horizontal sync width counter. Loading zero naturally
            // produces a 16-character sync pulse after the first decrement wraps to 0x0F.
            // This produces an hysnc width long enough to allow color-burst generation.
            // This trick is used by the PC demo "8088 MPH".
            self.hsc_c3l = self.hsc_c3l.wrapping_sub(1) & HORIZONTAL_SYNC_WIDTH_MASK;

            if self.hsc_c3l == 0 {
                // C3L expired. End the horizontal sync pulse.
                self.status.hsync = false;

                // Update the video mode, if an update is pending.
                // // It is important not to change graphics mode while we are catching up during an IO instruction.
                // if !self.catching_up && self.mode_pending {
                //     self.update_mode();
                //     self.mode_pending = false;
                // }

                // END OF LOGICAL SCANLINE
                if self.in_vblank {
                    // If we are in the vertical sync interval, advance Vertical Sync Counter.
                    self.vsc_c3h += 1;
                    if self.vsc_c3h == CRTC_VBLANK_HEIGHT {
                        // We are leaving the fixed 6845 vertical sync period.
                        self.in_last_vblank_line = true;
                        self.vsc_c3h = 0;
                        self.status.vsync = false;
                        //return (&self.status, self.vma);
                    }
                }

                // We are leaving horizontal blanking period.
                self.char_col = 0;
                self.in_hblank = false;
            }
        }

        if self.hcc_c0 == self.reg[HorizontalDisplayed] {
            // C0 == R1 (HorizontalDisplayed): Entering right overscan.
            if self.vlc_c9 == self.reg[MaximumScanlineAddress] {
                // C9 == R9 (MaximumScanlineAddress): We are at the last character row
                // Save VMA in VMA'
                self.vma_t = self.vma;
            }
            self.status.den = false;
            self.status.hborder = true;
        }

        if self.hcc_c0 == self.reg[HorizontalSyncPosition] {
            // C0 == R2 (HorizontalSyncPos) We entered horizontal sync.
            self.in_hblank = true;
            self.status.hsync = true;
            self.hsc_c3l = self.horizontal_sync_width();
        }

        if self.hcc_c0 == self.reg[HorizontalTotal] && self.in_last_vblank_line {
            // C0 == R0 (HorizontalTotal): We are one char away from the beginning of the new frame.
            // Draw one char of border
            self.status.hborder = true;
        }

        if self.hcc_c0 == self.reg[HorizontalTotal] + 1 {
            // END-OF-LINE processing
            if self.vlc_c9 == self.reg[CursorEndLine] {
                self.cursor_active = false;
            }

            // C0 == R0 (HorizontalTotal): Leaving left overscan, finished scanning row
            if self.in_last_vblank_line {
                self.in_last_vblank_line = false;
                self.in_vblank = false;
                self.status.vsync = false;
            }

            // Reset Horizontal Character Counter and increment character row counter
            self.hcc_c0 = 0;
            self.status.hborder = false;
            self.vlc_c9 += 1;
            // Return video memory address to starting position for next character row
            self.vma = self.vma_t;

            // Reset the current character glyph to start of row
            //self.set_char_addr();

            if !self.in_vblank && self.in_display_rows {
                // Start the new row
                self.status.den = true;
            }

            if self.vlc_c9 > self.reg[MaximumScanlineAddress] {
                // C9 == R9 (MaxScanlineAddress): We finished drawing this row of characters
                self.vlc_c9 = 0;
                // Increment Vertical Character Counter
                self.vcc_c4 = self.vcc_c4.wrapping_add(1);
                // Set vma to starting position for next character row
                self.vma = self.vma_t;

                // Load next char + attr
                //self.set_char_addr();

                if self.vcc_c4 == self.reg[VerticalSync] {
                    // C4 == R7 (VerticalSyncPos): We've reached vertical sync
                    trace_regs!(self);
                    trace!(self, "Entering vsync");
                    self.in_vblank = true;
                    self.status.vsync = true;
                    self.status.den = false;

                    if let Some(rate) = self.cursor_blink_rate {
                        self.cursor_blink_ct = self.cursor_blink_ct.wrapping_add(1);
                        if self.cursor_blink_ct == rate {
                            self.cursor_blink_ct = 0;
                            self.blink_state = !self.blink_state;
                        }
                    }
                }

                if self.last_line {
                    // The last scanline of R4 has completed; start vertical total adjust.
                    self.in_vta = true;
                    self.last_row = false;
                    self.last_line = false;
                }
            }

            if self.vcc_c4 == self.reg[VerticalDisplayed] {
                // C4 == R6 (VerticalDisplayed): Entering lower overscan area.
                self.in_display_rows = false;
                self.status.den = false;
                self.status.vborder = true;
            }

            if self.in_vta {
                // We are in vertical total adjust. Increment vtac counter.
                self.vtac_c5 += 1;

                if self.vtac_c5 > self.reg[VerticalTotalAdjust] {
                    // C5 == R5 (VerticalTotalAdjust): We are at the end of the top overscan.
                    self.in_vta = false;
                    self.vtac_c5 = 0;
                    self.vcc_c4 = 0;
                    self.vlc_c9 = 0;
                    self.char_col = 0;
                    self.start_address_latch = self.start_address;
                    self.vma = self.start_address;
                    self.vma_t = self.vma;
                    self.status.den = true;
                    self.in_display_rows = true;
                    self.status.vborder = false;
                    self.in_vblank = false;
                    self.status.vsync = false;

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

        push_reg_str!(crtc_vec, HorizontalTotal, "[R0]", self.reg[HorizontalTotal]);
        push_reg_str!(crtc_vec, HorizontalDisplayed, "[R1]", self.reg[HorizontalDisplayed]);
        push_reg_str!(crtc_vec, HorizontalSyncPosition, "[R2]", self.reg[HorizontalSyncPosition]);
        push_reg_str!(crtc_vec, SyncWidth, "[R3]", self.reg[SyncWidth]);
        push_reg_str!(crtc_vec, VerticalTotal, "[R4]", self.reg[VerticalTotal]);
        push_reg_str!(crtc_vec, VerticalTotalAdjust, "[R5]", self.reg[VerticalTotalAdjust]);
        push_reg_str!(crtc_vec, VerticalDisplayed, "[R6]", self.reg[VerticalDisplayed]);
        push_reg_str!(crtc_vec, VerticalSync, "[R7]", self.reg[VerticalSync]);
        push_reg_str!(crtc_vec, InterlaceMode, "[R8]", self.reg[InterlaceMode]);
        push_reg_str!(crtc_vec, MaximumScanlineAddress, "[R9]", self.reg[MaximumScanlineAddress]);
        push_reg_str!(crtc_vec, CursorStartLine, "[R10]", self.reg[CursorStartLine]);
        push_reg_str!(crtc_vec, CursorEndLine, "[R11]", self.reg[CursorEndLine]);
        push_reg_str!(crtc_vec, StartAddressH, "[R12]", self.reg[StartAddressH]);
        push_reg_str!(crtc_vec, StartAddressL, "[R13]", self.reg[StartAddressL]);
        crtc_vec.push(("Start Address".to_string(), VideoCardStateEntry::String(format!("{:04X}", self.start_address))));
        push_reg_str!(crtc_vec, CursorAddressH, "[R14]", self.reg[CursorAddressH]);
        push_reg_str!(crtc_vec, CursorAddressL, "[R15]", self.reg[CursorAddressL]);

        crtc_vec
    }

    #[rustfmt::skip]
    pub fn get_counter_state(&self) -> Vec<(String, VideoCardStateEntry)> {
        let mut counter_vec = Vec::new();

        counter_vec.push(("hcc_c0:".to_string(), VideoCardStateEntry::String(format!("{}", self.hcc_c0))));
        counter_vec.push(("vcc_c4:".to_string(), VideoCardStateEntry::String(format!("{}", self.vcc_c4))));
        counter_vec.push(("last_row:".to_string(), VideoCardStateEntry::String(format!("{}", self.last_row))));
        counter_vec.push(("last_line:".to_string(), VideoCardStateEntry::String(format!("{}", self.last_line))));
        counter_vec.push(("vtac_c5:".to_string(), VideoCardStateEntry::String(format!("{}", self.vtac_c5))));

        counter_vec
    }

    pub fn debug_string(&self) -> String {
        format!("hcc_c0: {} vcc_c4: {}", self.hcc_c0, self.vcc_c4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degenerate_2_by_2_den_test() {
        let trace_logger = TraceLogger::None;
        let mut crtc = Crtc6845::new(trace_logger);

        crtc.write_register_direct(HorizontalTotal, 0x01);
        crtc.write_register_direct(HorizontalDisplayed, 0x01);
        crtc.write_register_direct(VerticalTotal, 0x01);
        crtc.write_register_direct(VerticalDisplayed, 0x01);

        for clock in 0..16 {
            let (status, _) = crtc.tick();
            println!(
                "Clock: {} den:{} {}",
                clock,
                if status.den { "1" } else { "0" },
                crtc.debug_string()
            );
        }
    }

    fn measure_hsync_width(sync_width: u8) -> usize {
        let trace_logger = TraceLogger::None;
        let mut crtc = Crtc6845::new(trace_logger);

        crtc.write_register_direct(HorizontalTotal, 100);
        crtc.write_register_direct(HorizontalDisplayed, 1);
        crtc.write_register_direct(HorizontalSyncPosition, 2);
        crtc.write_register_direct(SyncWidth, sync_width);

        let mut saw_hsync = false;
        let mut width = 0;
        for _ in 0..64 {
            let (status, _) = crtc.tick();
            if status.hsync {
                saw_hsync = true;
                width += 1;
            }
            else if saw_hsync {
                break;
            }
        }

        width
    }

    #[test]
    fn horizontal_sync_width_uses_low_nibble() {
        assert_eq!(measure_hsync_width(0x0A), 10);
        assert_eq!(measure_hsync_width(0x1A), 10);
    }

    #[test]
    fn horizontal_sync_width_zero_wraps_to_sixteen() {
        assert_eq!(measure_hsync_width(0x00), 16);
    }

    #[test]
    fn vertical_total_last_line_is_latched_early() {
        let trace_logger = TraceLogger::None;
        let mut crtc = Crtc6845::new(trace_logger);

        crtc.write_register_direct(HorizontalTotal, 3);
        crtc.write_register_direct(HorizontalDisplayed, 1);
        crtc.write_register_direct(HorizontalSyncPosition, 2);
        crtc.write_register_direct(SyncWidth, 1);
        crtc.write_register_direct(VerticalTotal, 0);
        crtc.write_register_direct(VerticalTotalAdjust, 4);
        crtc.write_register_direct(MaximumScanlineAddress, 0);

        crtc.tick();
        assert!(crtc.last_line);

        crtc.write_register_direct(VerticalTotal, 1);
        assert!(crtc.last_line);

        while crtc.vlc_c9 == 0 && !crtc.in_vta {
            crtc.tick();
        }

        assert!(crtc.in_vta);
    }

    #[test]
    fn port_write_register_select() {
        let trace_logger = TraceLogger::None;
        let mut crtc = Crtc6845::new(trace_logger);
        crtc.port_write(0, 4);
        assert_eq!(crtc.reg_select, CrtcRegister::VerticalTotal);
    }

    #[test]
    fn port_write_register_write() {
        let trace_logger = TraceLogger::None;
        let mut crtc = Crtc6845::new(trace_logger);
        crtc.port_write(0, 4);
        crtc.port_write(1, 0x7F);
        assert_eq!(crtc.reg[VerticalTotal], 0x7F);
    }

    #[test]
    fn port_read_address_register() {
        let trace_logger = TraceLogger::None;
        let mut crtc = Crtc6845::new(trace_logger);
        assert_eq!(crtc.port_read(0), 0xFF);
    }

    #[test]
    fn port_read_data_register() {
        let trace_logger = TraceLogger::None;
        let mut crtc = Crtc6845::new(trace_logger);
        crtc.port_write(0, 14);
        crtc.reg[CursorAddressH] = 0x3F;
        assert_eq!(crtc.port_read(1), 0x3F);
    }

    #[test]
    fn select_register_out_of_bounds() {
        let trace_logger = TraceLogger::None;
        let mut crtc = Crtc6845::new(trace_logger);
        crtc.select_register(18);
        assert_eq!(crtc.reg_select, CrtcRegister::HorizontalTotal);
    }

    #[test]
    fn write_register_horizontal_total() {
        let trace_logger = TraceLogger::None;
        let mut crtc = Crtc6845::new(trace_logger);
        crtc.select_register(0);
        crtc.write_register(0xFF);
        assert_eq!(crtc.reg[HorizontalTotal], 0xFF);
    }

    #[test]
    fn write_register_vertical_total() {
        let trace_logger = TraceLogger::None;
        let mut crtc = Crtc6845::new(trace_logger);
        crtc.select_register(4);
        crtc.write_register(0xFF);
        assert_eq!(crtc.reg[VerticalTotal], 0x7F);
    }

    #[test]
    fn read_register_unreadable() {
        let trace_logger = TraceLogger::None;
        let crtc = Crtc6845::new(trace_logger);
        assert_eq!(crtc.read_register(), 0xFF);
    }

    #[test]
    fn update_start_address() {
        let trace_logger = TraceLogger::None;
        let mut crtc = Crtc6845::new(trace_logger);
        crtc.reg[StartAddressH] = 0x12;
        crtc.reg[StartAddressL] = 0x34;
        crtc.update_start_address();
        assert_eq!(crtc.start_address, 0x1234);
    }

    #[test]
    fn update_cursor_address() {
        let trace_logger = TraceLogger::None;
        let mut crtc = Crtc6845::new(trace_logger);
        crtc.reg[CursorAddressH] = 0x12;
        crtc.reg[CursorAddressL] = 0x34;
        crtc.update_cursor_address();
        assert_eq!(crtc.cursor_address, 0x1234);
    }
}

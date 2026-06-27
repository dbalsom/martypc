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
*/

//! Implementation of the Motorola MC6845 CRT controller.
//! Used internally by the MDA, CGA, and TGA adapters.
//! EGA & VGA have custom LSI CRTC's more suited for graphics modes.
//!
//! This implementation is accurate enough to run demanding PC demos such as "8088 MPH" and
//! "Area 5150". A great deal of troubleshooting was done against logic analyzer captures
//! of the 6845's outputs while these demos were running.
//!
//! More recently, direct die analysis of the MC6845 has been performed, thanks to excellent
//! photography by InfosecDJ:
//! https://siliconprawn.org/map/motorola/mc6845p-jr5/infosecdj_mz_nikpa20x/
//!
//! Major credits go to Longshot of LOGON SYSTEM for the essential CRTC COMPENDIUM:
//! https://logonsystem.eu/html/downloadlogon.htm without which this would not have been possible.
//!
//! Thanks to reenigne and VileR for essential clarifications and corrections.

use std::ops::{Index, IndexMut};

use crate::{device_traits::videocard::VideoCardStateEntry, tracelogger::TraceLogger};
use strum::EnumCount;

const CURSOR_LINE_MASK: u8 = 0b0001_1111;
const CURSOR_ATTR_MASK: u8 = 0b0110_0000;

const BLINK_FAST_RATE: u16 = 8;
const BLINK_SLOW_RATE: u16 = 16;

const CRTC_VBLANK_HEIGHT: u8 = 16;
const CRTC_ROW_MAX: usize = 32;
const HORIZONTAL_SYNC_WIDTH_MASK: u8 = 0x0F;

const REGISTER_UNREADABLE_VALUE: u8 = 0xFF;

#[derive(Copy, Clone, Debug, Default)]
pub enum InterlacedParity {
    #[default]
    Even,
    Odd,
}

impl InterlacedParity {
    pub fn next(&self) -> InterlacedParity {
        match self {
            InterlacedParity::Even => InterlacedParity::Odd,
            InterlacedParity::Odd => InterlacedParity::Even,
        }
    }
    #[inline]
    pub fn is_even(&self) -> bool {
        matches!(self, InterlacedParity::Even)
    }
    #[inline]
    pub fn is_odd(&self) -> bool {
        matches!(self, InterlacedParity::Odd)
    }
    #[inline]
    pub fn bit(&self) -> u8 {
        match self {
            InterlacedParity::Even => 0,
            InterlacedParity::Odd => 1,
        }
    }
}

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
    HorizontalTotalR0,        // R0
    HorizontalDisplayedR1,    // R1
    HorizontalSyncPositionR2, // R2
    SyncWidthR3,              // R3
    VerticalTotalR4,          // R4
    VerticalTotalAdjustR5,    // R5
    VerticalDisplayedR6,      // R6
    VerticalSyncR7,           // R7
    InterlaceModeR8,          // R8
    MaximumScanlineAddressR9, // R9
    CursorStartLine,          // R10
    CursorEndLine,            // R11
    StartAddressH,            // R12
    StartAddressL,            // R13
    CursorAddressH,           // R14
    CursorAddressL,           // R15
    LightPenPositionH,        // R16
    LightPenPositionL,        // R17
    InvalidRegister,
}

use crate::devices::mc6845::CrtcRegister::*;

// Implement From<usize> for CrtcRegister to allow indexing into CrtcRegisterFile via enum.
impl From<usize> for CrtcRegister {
    fn from(value: usize) -> Self {
        match value {
            0 => HorizontalTotalR0,
            1 => HorizontalDisplayedR1,
            2 => HorizontalSyncPositionR2,
            3 => SyncWidthR3,
            4 => VerticalTotalR4,
            5 => VerticalTotalAdjustR5,
            6 => VerticalDisplayedR6,
            7 => VerticalSyncR7,
            8 => InterlaceModeR8,
            9 => MaximumScanlineAddressR9,
            10 => CursorStartLine,
            11 => CursorEndLine,
            12 => StartAddressH,
            13 => StartAddressL,
            14 => CursorAddressH,
            15 => CursorAddressL,
            16 => LightPenPositionH,
            17 => LightPenPositionL,
            _ => InvalidRegister,
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
pub enum CrtcMode {
    #[default]
    NormalRows,
    VerticalTotalAdjust,
    InterlacedHalfLine,
}

#[derive(Copy, Clone, Default, Debug)]
pub enum CrtcInterlacedMode {
    #[default]
    NormalVideo,
    InterlacedSync,
    InterlacedSyncAndVideo,
}

impl From<u8> for CrtcInterlacedMode {
    fn from(val: u8) -> Self {
        match val {
            0b01 => CrtcInterlacedMode::InterlacedSync,
            0b11 => CrtcInterlacedMode::InterlacedSyncAndVideo,
            _ => CrtcInterlacedMode::NormalVideo,
        }
    }
}

impl CrtcInterlacedMode {
    #[inline]
    pub fn is_interlaced_sync(&self) -> bool {
        matches!(
            self,
            CrtcInterlacedMode::InterlacedSyncAndVideo | CrtcInterlacedMode::InterlacedSync
        )
    }
    #[inline]
    pub fn is_interlaced_video(&self) -> bool {
        matches!(self, CrtcInterlacedMode::InterlacedSyncAndVideo)
    }
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

    ticks:  usize, // Number of CRTC ticks.
    frames: usize, // Number of CRTC frames.

    mode: CrtcMode,

    start_address: u16,       // Immediate value calculated from R12 & R13
    start_address_latch: u16, // Start address, latched per frame
    lightpen_position: u16,   // Calculated value from R16 & R17

    cursor_address: u16, // Immediate value calculated from R14 & R15
    cursor_enabled: bool,
    cursor_start_line: u8, // Latch for R10 sans extra state bits.
    cursor_active: bool,   // Whether cursor is between cursor_start_line and curse_end_line
    blink_state: bool,     // State of cursor blink (true => displayed), (false => not displayed)
    cursor_blink_rate: Option<u16>,

    // Internal counters and state.
    // Counter values are hybrid names between "classical" emulator names and the names used in
    // the CRTC Compendium.
    hcc_c0:  u8, // Horizontal character counter (8 bits) (x pos of character)
    vlc_c9:  u8, // Vertical line counter (5 bits) - counts scanlines within a character row.
    vlc_c9i: u8, // Interlaced vertical line counter (4 bits) - counts scanlines within a character row when interlaced video is enabled.
    vcc_c4:  u8, // Vertical character counter (7 bits) (y pos of character)
    vsc_c3h: u8, // Vertical scan counter (4 bits) - counts during vsync period
    hsc_c3l: u8, // Horizontal sync counter (4 bits) - counts during hsync period
    vtac_c5: u8, // Vertical total adjust counter (5 bits) - counts during vertical total adjust period.

    last_row: bool,
    previous_last_line: bool, // Was the previous line the last line in a frame?
    last_line: bool,          // Is the current line the last line in a frame?
    last_line_mgmt: bool,     // "Last Line Management" state bit. Controls whether last line flag can be set.

    vma:   u16, // VMA register - Video memory address
    vma_t: u16, // VMA' register - Video memory address temporary

    // Interlaced mode stuff.
    interlaced_mode: CrtcInterlacedMode,
    scanline_parity: InterlacedParity,
    frame_parity:    InterlacedParity,

    status: CrtcStatus,
    in_hsync: bool,
    in_vsync: bool,
    vertical_de: bool,
    horizontal_de: bool,
    in_display_rows: bool,
    in_last_vblank_line: bool,

    trace_logger: TraceLogger,
}

impl Default for Crtc6845 {
    fn default() -> Self {
        Self {
            reg: Default::default(),
            reg_select: HorizontalTotalR0,

            ticks:  0,
            frames: 0,

            mode: CrtcMode::NormalRows,

            start_address: 0,
            start_address_latch: 0,
            lightpen_position: 0,

            cursor_address: 0,
            cursor_enabled: false,
            cursor_start_line: 0,
            cursor_active: false,
            blink_state: false,
            cursor_blink_rate: Some(BLINK_FAST_RATE),

            hcc_c0: 0,
            vlc_c9: 0,
            vlc_c9i: 0,
            vcc_c4: 0,
            last_row: false,
            previous_last_line: false,
            last_line: false,
            last_line_mgmt: false,

            vsc_c3h: 0,
            hsc_c3l: 0,
            vtac_c5: 0,

            vma:   0,
            vma_t: 0,

            interlaced_mode: CrtcInterlacedMode::default(),
            scanline_parity: InterlacedParity::default(),
            frame_parity:    InterlacedParity::default(),

            status: Default::default(),
            in_hsync: false,
            in_vsync: false,
            vertical_de: false,
            horizontal_de: false,
            in_display_rows: false,
            in_last_vblank_line: false,

            trace_logger: Default::default(),
        }
    }
}

impl Crtc6845 {
    pub fn new(trace_logger: TraceLogger) -> Self {
        Self {
            trace_logger,
            ..Default::default()
        }
    }

    pub fn reset(&mut self) {
        let trace_logger = std::mem::replace(&mut self.trace_logger, TraceLogger::None);
        *self = Self {
            trace_logger,
            ..Default::default()
        }
    }

    // Convenience wrapper to read from the CRTC's address or data register based on A0.
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

    // Convenience wrapper to write to the CRTC's address or data register based on A0.
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

    // Select a CRTC register.
    // The 6845 has an 5-bit address register will hold any value 0-13. This feeds into a address
    // decoding PLA with 18 outputs, one for each valid register.
    // So invalid registers can indeed be specified and the address register will accept them, but
    // nothing will happen when writing to a register out of range as no output from the PLA will
    // be activated.
    pub fn select_register(&mut self, idx: usize) {
        self.reg_select = CrtcRegister::from(idx);
        // Very noisy in certain games and demo effects. Enable with caution.
        //log::trace!("CRTC register selected: {:?}", self.reg_select);
    }

    // Helper function to both select a register and write a value to it.
    // Mostly used by tests.
    pub fn write_register_direct(&mut self, reg: CrtcRegister, byte: u8) {
        self.reg_select = reg;
        self.write_register(byte);
    }

    // Write the specified data to the currently selected CRTC register.
    pub fn write_register(&mut self, byte: u8) {
        //log::trace!("crtc write register: {:02X}", byte);
        match self.reg_select {
            HorizontalTotalR0 => {
                // (R0) 8 bit write only
                self.reg[HorizontalTotalR0] = byte;
            }
            HorizontalDisplayedR1 => {
                // (R1) 8 bit write only
                self.reg[HorizontalDisplayedR1] = byte;
            }
            HorizontalSyncPositionR2 => {
                // (R2) 8 bit write only
                self.reg[HorizontalSyncPositionR2] = byte;
            }
            SyncWidthR3 => {
                // (R3) 8 bit write only
                self.reg[SyncWidthR3] = byte;
            }
            VerticalTotalR4 => {
                // (R4) 7 bit write only
                self.reg[VerticalTotalR4] = byte & 0x7F;

                trace_regs!(self);
                trace!(
                    self,
                    "CRTC Register Write (04h): VerticalTotal updated: {}",
                    self.reg[VerticalTotalR4]
                )
            }
            VerticalTotalAdjustR5 => {
                // (R5) 5 bit write only
                self.reg[VerticalTotalAdjustR5] = byte & 0x1F;
            }
            VerticalDisplayedR6 => {
                // (R6) 7 bit write only
                self.reg[VerticalDisplayedR6] = byte & 0x7F;
            }
            VerticalSyncR7 => {
                // (R7) 7 bit write only
                self.reg[VerticalSyncR7] = byte & 0x7F;

                trace_regs!(self);
                trace!(
                    self,
                    "CRTC Register Write (07h): VerticalSync updated: {}",
                    self.reg[VerticalSyncR7]
                )
            }
            InterlaceModeR8 => {
                // (R8) 2 bit write only
                self.reg[InterlaceModeR8] = byte & 0x03;

                let last_interlaced_mode = self.interlaced_mode;
                self.interlaced_mode = CrtcInterlacedMode::from(self.reg[InterlaceModeR8]);

                if last_interlaced_mode.is_interlaced_sync() && !self.interlaced_mode.is_interlaced_sync() {
                    // We have turned off interlaced sync. Reset frame parity.
                    self.frame_parity = InterlacedParity::Even;
                }
            }
            MaximumScanlineAddressR9 => {
                // (R9) 5 bit write only
                self.reg[MaximumScanlineAddressR9] = byte & 0x1F;
            }
            CursorStartLine => {
                // (R10) 7 bit bitfield. Write only.
                self.reg[CursorStartLine] = byte & 0x7F;

                self.cursor_start_line = byte & CURSOR_LINE_MASK;
                match self.reg[CursorStartLine] >> 5 {
                    0b00 => {
                        // 6845 documentation specifies that this value will disable cursor blink.
                        // We can disable cursor blink on the CRTC chip, but some cards like the
                        // IBM CGA will stubbornly continue blinking as they add their own blink
                        // logic. Some systems like the Amstrad CPC do not use the CRTC cursor at
                        // all.
                        self.cursor_enabled = true;
                        self.cursor_blink_rate = None;
                    }
                    0b01 => {
                        self.cursor_enabled = false;
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
            InvalidRegister => {
                // Nothing happens due to lack of PLA activation when invalid register is selected.
            }
        }
    }

    // Attempt to read the currently selected register.
    // On the 6845 only the Cursor Address and Light Pen Position registers are readable.
    // On some systems that do not use the 6845's hardware cursor, you can stash values in the
    // cursor address registers.
    pub fn read_register(&self) -> u8 {
        match self.reg_select {
            CursorAddressH | CursorAddressL | LightPenPositionH | LightPenPositionL => self.reg[self.reg_select],
            _ => REGISTER_UNREADABLE_VALUE,
        }
    }

    #[inline]
    fn horizontal_sync_width(&self) -> u8 {
        self.reg[SyncWidthR3] & HORIZONTAL_SYNC_WIDTH_MASK
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
    pub fn ra(&self) -> u8 {
        if self.interlaced_mode.is_interlaced_video() {
            (self.vlc_c9i << 1) | self.frame_parity.bit()
        }
        else {
            self.vlc_c9
        }
    }

    #[inline]
    pub fn ma(&self) -> u16 {
        self.vma
    }

    #[inline]
    pub fn vlc(&self) -> u8 {
        match self.interlaced_mode {
            CrtcInterlacedMode::InterlacedSyncAndVideo => self.vlc_c9i,
            _ => self.vlc_c9,
        }
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
        matches!(self.mode, CrtcMode::VerticalTotalAdjust)
    }

    #[inline]
    pub fn blink_bits(&self) -> u8 {
        (self.reg[CursorStartLine] & CURSOR_ATTR_MASK) >> 5
    }

    #[inline]
    pub fn status(&self) -> &CrtcStatus {
        &self.status
    }

    #[inline]
    pub fn maximum_scanline(&self) -> u8 {
        self.reg[MaximumScanlineAddressR9]
    }

    #[inline]
    pub fn frame_parity(&self) -> InterlacedParity {
        self.frame_parity
    }

    #[inline]
    pub fn frame_parity_bit(&self) -> u8 {
        self.frame_parity.bit()
    }

    #[inline]
    pub fn interlaced_sync_enabled(&self) -> bool {
        self.interlaced_mode.is_interlaced_sync()
    }

    // Emulate the cursor strobe by taking the current VMA and splitting it into the two light
    // pen registers, R16 & R17.
    pub fn latch_lightpen(&mut self) {
        self.lightpen_position = self.vma;
        self.reg[LightPenPositionH] = ((self.lightpen_position >> 8) & 0x3F) as u8;
        self.reg[LightPenPositionL] = (self.lightpen_position & 0xFF) as u8;
    }

    // Combine R12 & R13 into a single 16-bit value.
    fn update_start_address(&mut self) {
        self.start_address = (self.reg[StartAddressH] as u16) << 8 | self.reg[StartAddressL] as u16
    }

    // Combine R14 & R15 into a single 16-bit value.
    fn update_cursor_address(&mut self) {
        self.cursor_address = (self.reg[CursorAddressH] as u16) << 8 | self.reg[CursorAddressL] as u16
    }

    #[inline]
    pub fn cursor_address(&self) -> u16 {
        self.cursor_address
    }

    pub fn cursor_extents(&self) -> (u8, u8) {
        (self.cursor_start_line, self.reg[CursorEndLine])
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
    pub fn cursor_enabled(&self) -> bool {
        self.cursor_enabled
    }

    #[inline]
    pub fn hsync(&self) -> bool {
        self.in_hsync
    }

    #[inline]
    pub fn vsync(&self) -> bool {
        self.in_vsync
    }

    #[inline]
    pub fn den(&self) -> bool {
        self.horizontal_de && self.vertical_de
    }

    #[inline]
    pub fn border(&self) -> bool {
        !self.horizontal_de | !self.vertical_de
    }

    #[inline]
    fn tick_vlc(&mut self, tick_i: bool) {
        // Increment 5-bit progressive VLC
        self.vlc_c9 = (self.vlc_c9 + 1) & 0x1F;

        if tick_i {
            // Increment 4-bit interlaced VLC
            self.vlc_c9i = (self.vlc_c9i + 1) & 0x0F;
        }

        if (self.vlc_c9 == self.cursor_start_line) && !self.in_vta() {
            self.cursor_active = true;
        }
    }

    fn frame_management(&mut self) {}

    /// Tick the CRTC to the next character.
    pub fn tick(&mut self) -> (&CrtcStatus, u16) {
        // Evaluate coincidence circuits.
        let _c5_r5 = self.vtac_c5 == self.reg[VerticalTotalAdjustR5];
        let _c3l_r3 = self.hsc_c3l == self.reg[SyncWidthR3];
        // C4 comparisons
        let _c4_r7 = self.vcc_c4 == self.reg[VerticalSyncR7];
        let _c4_r6 = self.vcc_c4 == self.reg[VerticalDisplayedR6];
        let c4_r4 = self.vcc_c4 == self.reg[VerticalTotalR4];

        // C0 comparisons
        let c0_r0 = self.hcc_c0 == self.reg[HorizontalTotalR0];
        let c0_r0_half = self.hcc_c0 == (self.reg[HorizontalTotalR0] >> 1);
        let _c0_r2 = self.hcc_c0 == self.reg[HorizontalSyncPositionR2];
        let _c0_r1 = self.hcc_c0 == self.reg[HorizontalDisplayedR1];
        // C9 comparisons
        let c9i_r9 = self.vlc_c9i == (self.reg[MaximumScanlineAddressR9] >> 1);
        let c9_r9_half = self.vlc_c9 == (self.reg[MaximumScanlineAddressR9] >> 1);
        let c9_r9 = self.vlc_c9 == self.reg[MaximumScanlineAddressR9];
        let c9_ivm_split = self.interlaced_mode.is_interlaced_video() && c9_r9_half;

        let _c9_r11 = self.vlc_c9 == self.reg[CursorEndLine];
        let _c9_r10 = self.vlc_c9 == self.cursor_start_line; // Coincidence circuit is 5 bit.

        let _vma_cursor = self.vma == self.cursor_address;

        if self.hcc_c0 == 0 {
            // START-OF-LINE processing.
            // Various logic is evalauated at the start of a line when C0 == 0.

            // Turn cursor on if this line matches CursorStartLine.
            if (self.vlc_c9 == self.cursor_start_line) && !self.in_vta() {
                self.cursor_active = true;
            }

            if self.vcc_c4 == 0 {
                // We are at the first character of the first character row
                if self.vlc_c9 == 0 {
                    // We are at the first scanline of the first character of the first character
                    // row.
                    // START-OF-FRAME processing
                    self.vma = self.start_address_latch;
                }
            }

            // Evaluate the 'last_line' status.
            // See CRTC Compendium, 12.4.1

            // [Coincidence circuit]: C4 == R4
            if self.vcc_c4 == self.reg[VerticalTotalR4] {
                self.last_row = true;
                // 'last_line' is true if (C4 == R4) && (C9 == R9),
                // except:
                //  - the previous line was a 'last line'
                //  - if a HSYNC takes place on position C0==0 (v1.9 p94)
                self.last_line = c9_r9 && !self.previous_last_line && !self.in_hsync;
                // 'last_line_mgmt' is false if (C4 == 0 && (C9 == 0)
                self.last_line_mgmt = !(self.vcc_c4 == 0 && self.vlc_c9 == 0);
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
            self.horizontal_de = true;
            if self.vcc_c4 == 0 {
                // START-OF-FRAME processing.
                // We are at the first character of a CRTC frame. Update start address.
                self.vma = self.start_address_latch;
            }
        }

        // Advance video memory address
        self.vma += 1;

        // Process horizontal blanking period
        if self.in_hsync {
            // The MC6845 uses a 4-bit horizontal sync width down-counter. Loading zero naturally
            // produces a 16-character sync pulse after the first decrement wraps to 0x0F.
            // Conveniently, this produces an hsync width long enough to allow color-burst
            // generation on the CGA in 80-column mode. This trick is used by the PC demo "8088 MPH"
            // This only works on a Motorola 6845, not on Hitachi 6485s.
            self.hsc_c3l = self.hsc_c3l.wrapping_sub(1) & HORIZONTAL_SYNC_WIDTH_MASK;

            if self.hsc_c3l == 0 {
                // C3L expired. End the horizontal sync pulse.

                // CRTC Compendium, 12.4.1
                // "During a HSYNC, a test is performed on position C0=R2+R3-1, in order to
                // determine if line N is a last line for line N+1 (at C0=0)."
                if c4_r4 && c9_r9 {
                    self.previous_last_line = true;
                }
                else {
                    self.previous_last_line = false;
                    // "Furthermore, if C4<>R4 or C9<>R9 on this last position of the HSYNC, this
                    // updates the "Last Line Management" state by setting it to true"
                    self.last_line_mgmt = true;
                }

                // Update the video mode, if an update is pending.
                // // It is important not to change graphics mode while we are catching up during an IO instruction.
                // if !self.catching_up && self.mode_pending {
                //     self.update_mode();
                //     self.mode_pending = false;
                // }

                if self.in_vsync {
                    // If we are in the vertical sync interval, advance Vertical Sync Counter.
                    self.vsc_c3h += 1;
                    if self.vsc_c3h == CRTC_VBLANK_HEIGHT {
                        // We are leaving the fixed 6845 vertical sync period.
                        self.in_last_vblank_line = true;
                        self.vsc_c3h = 0;
                        self.in_vsync = false;
                    }
                }

                // We are leaving horizontal blanking period.
                self.in_hsync = false;
            }
        }

        // [Coincidence circuit]: C0 == R1.
        if self.hcc_c0 == self.reg[HorizontalDisplayedR1] {
            // C0 == R1 (HorizontalDisplayed): Entering right overscan.

            if c9_r9 || c9_ivm_split {
                // C9 == R9 (MaximumScanlineAddress): We are at the last character row
                // Save VMA in VMA'
                self.vma_t = self.vma;
            }
            self.horizontal_de = false;
        }

        // [Coincidence circuit]: C0 == R2
        if self.hcc_c0 == self.reg[HorizontalSyncPositionR2] {
            // C0 == R2 (HorizontalSyncPos) We entered horizontal sync.
            self.in_hsync = true;
            self.hsc_c3l = self.horizontal_sync_width();
        }

        // if self.hcc_c0 == self.reg[HorizontalTotal] && self.in_last_vblank_line {
        //     // C0 == R0 (HorizontalTotal): We are one char away from the beginning of the new frame.
        //     // Draw one char of border
        //     self.status.hborder = true;
        // }

        // [Coincidence circuit]: C0 == R0
        if c0_r0 {
            // END-OF-LINE processing
            if (self.vlc_c9 == self.reg[CursorEndLine]) && !self.in_vta() {
                self.cursor_active = false;
            }

            // C0 == R0 (HorizontalTotal): Leaving left overscan, finished scanning row
            if self.in_last_vblank_line {
                self.in_last_vblank_line = false;
                self.in_vsync = false;
            }

            // Reset Horizontal Character Counter and increment character row counter
            self.hcc_c0 = 0;
            self.horizontal_de = true;

            // Return video memory address to starting position for next character row
            self.vma = self.vma_t;

            // Reset the current character glyph to start of row
            //self.set_char_addr();

            // [Coincidence circuit]: C9 == R9
            if c9_r9 {
                // C9 == R9 (MaxScanlineAddress): We finished drawing this row of characters
                self.vlc_c9 = 0;
                self.vlc_c9i = 0;
                // Increment Vertical Character Counter
                self.vcc_c4 = self.vcc_c4.wrapping_add(1);
                // Set vma to starting position for next character row
                self.vma = self.vma_t;

                if self.vcc_c4 == self.reg[VerticalSyncR7] {
                    // C4 == R7 (VerticalSyncPos): We've reached vertical sync
                    trace_regs!(self);
                    trace!(self, "Entering vsync");
                    self.in_vsync = true;

                    // Update cursor blink (I'm not actually sure when this is done, but at vsync
                    // is as good as any...)
                    // The cursor has no counter, it is based on a divided clock which we model here.
                    if let Some(rate) = self.cursor_blink_rate {
                        //log::trace!("Cursor has blink rate. Frames: {0}, rate: {rate}", self.frames);

                        if self.frames.is_multiple_of(rate as usize) {
                            //log::trace!("Cursor blinking");
                            self.blink_state = !self.blink_state;
                        }
                    }
                }

                if self.last_line {
                    self.process_last_line();
                }
            }
            else if c9_ivm_split {
                self.vlc_c9i = 0;
                self.tick_vlc(false);
            }
            else {
                self.tick_vlc(true);
            }

            // [Coincidence circuit]: C4 == R6
            if self.vcc_c4 == self.reg[VerticalDisplayedR6] {
                // C4 == R6 (VerticalDisplayed): Entering lower overscan area.
                self.in_display_rows = false;
                self.vertical_de = false;
            }

            if self.in_vta() {
                // We are in vertical total adjust. Increment vtac counter.
                self.vtac_c5 += 1;

                if self.vtac_c5 > self.reg[VerticalTotalAdjustR5] {
                    // C5 == R5 (VerticalTotalAdjust): We are at the end of the top overscan.
                    // START-OF-FRAME processing.
                    self.process_start_of_frame();
                }
            }
        }
        else if matches!(self.mode, CrtcMode::InterlacedHalfLine) && c0_r0_half {
            // At half-scanline vsync trigger in interlaced mode.
            self.process_last_line();
        }

        self.update_status();
        self.ticks += 1;

        (&self.status, self.vma)
    }

    #[inline]
    pub fn update_status(&mut self) {
        self.status.cursor = self.cursor();
        self.status.den = self.den();
        self.status.vsync = self.in_vsync;
        self.status.hsync = self.in_hsync;
        self.status.hborder = !self.horizontal_de;
        self.status.vborder = !self.vertical_de;
    }

    #[inline]
    pub fn transition_mode(&mut self, new_mode: CrtcMode) {
        // We can handle state transitions here, but currently nothing needs to be done.
        match new_mode {
            CrtcMode::NormalRows => {}
            _ => {
                self.in_display_rows = false;
            }
        }
        self.mode = new_mode;
    }

    #[inline]
    pub fn process_last_line(&mut self) {
        match self.mode {
            CrtcMode::NormalRows => {
                // The last scanline of R4 has completed.
                if self.reg[VerticalTotalAdjustR5] != 0 {
                    // If R5 is non-zero we will enter the vertical total adjust mode.
                    self.transition_mode(CrtcMode::VerticalTotalAdjust);
                    self.last_row = false;
                    self.last_line = false;
                }
                else if self.has_half_scanline() {
                    self.transition_mode(CrtcMode::InterlacedHalfLine);
                }
                else {
                    self.process_start_of_frame();
                }
            }
            CrtcMode::InterlacedHalfLine => {
                self.process_start_of_frame();
            }
            _ => {}
        }
    }

    #[inline]
    pub fn has_half_scanline(&self) -> bool {
        // Even frames have half scanlines.
        self.interlaced_mode.is_interlaced_sync() && matches!(self.frame_parity, InterlacedParity::Even)
    }

    pub fn process_start_of_frame(&mut self) {
        self.frames = self.frames.wrapping_add(1);

        if self.interlaced_mode.is_interlaced_sync() {
            self.frame_parity = self.frame_parity.next();
        }

        self.transition_mode(CrtcMode::NormalRows);
        self.last_row = false;
        self.last_line = false;
        self.vtac_c5 = 0;
        self.vcc_c4 = 0;
        self.vlc_c9 = 0;
        self.vlc_c9i = 0;
        self.start_address_latch = self.start_address;
        self.vma = self.start_address;
        self.vma_t = self.vma;

        self.in_display_rows = true;
        self.horizontal_de = true;
        self.vertical_de = true;
        self.in_vsync = false;
    }

    #[rustfmt::skip]
    pub fn get_reg_state(&self) -> Vec<(String, VideoCardStateEntry)> {
        let mut crtc_vec = Vec::new();

        push_reg_str!(crtc_vec, HorizontalTotalR0, "[R0]", self.reg[HorizontalTotalR0]);
        push_reg_str!(crtc_vec, HorizontalDisplayedR1, "[R1]", self.reg[HorizontalDisplayedR1]);
        push_reg_str!(crtc_vec, HorizontalSyncPositionR2, "[R2]", self.reg[HorizontalSyncPositionR2]);
        push_reg_str!(crtc_vec, SyncWidthR3, "[R3]", self.reg[SyncWidthR3]);
        push_reg_str!(crtc_vec, VerticalTotalR4, "[R4]", self.reg[VerticalTotalR4]);
        push_reg_str!(crtc_vec, VerticalTotalAdjustR5, "[R5]", self.reg[VerticalTotalAdjustR5]);
        push_reg_str!(crtc_vec, VerticalDisplayedR6, "[R6]", self.reg[VerticalDisplayedR6]);
        push_reg_str!(crtc_vec, VerticalSyncR7, "[R7]", self.reg[VerticalSyncR7]);
        push_reg_str!(crtc_vec, InterlaceModeR8, "[R8]", self.reg[InterlaceModeR8]);
        push_reg_str!(crtc_vec, MaximumScanlineAddressR9, "[R9]", self.reg[MaximumScanlineAddressR9]);
        push_reg_str!(crtc_vec, CursorStartLine, "[R10]", self.reg[CursorStartLine]);
        push_reg_str!(crtc_vec, CursorEndLine, "[R11]", self.reg[CursorEndLine]);
        push_reg_str!(crtc_vec, StartAddressH, "[R12]", self.reg[StartAddressH]);
        push_reg_str!(crtc_vec, StartAddressL, "[R13]", self.reg[StartAddressL]);
        crtc_vec.push(("Start Address".to_string(), VideoCardStateEntry::String(format!("{:04X}", self.start_address))));
        push_reg_str!(crtc_vec, CursorAddressH, "[R14]", self.reg[CursorAddressH]);
        push_reg_str!(crtc_vec, CursorAddressL, "[R15]", self.reg[CursorAddressL]);
        push_reg_str!(crtc_vec, LightPenPositionH, "[R16]", self.reg[LightPenPositionH]);
        push_reg_str!(crtc_vec, LightPenPositionL, "[R17]", self.reg[LightPenPositionL]);
        crtc_vec
    }

    #[rustfmt::skip]
    pub fn get_counter_state(&self) -> Vec<(String, VideoCardStateEntry)> {
        let mut counter_vec = Vec::new();

        counter_vec.push(("hcc_c0:".to_string(), VideoCardStateEntry::String(format!("{}", self.hcc_c0))));
        counter_vec.push(("vcc_c4:".to_string(), VideoCardStateEntry::String(format!("{}", self.vcc_c4))));
        counter_vec.push(("vlc_c9:".to_string(), VideoCardStateEntry::String(format!("{}", self.vlc_c9))));
        counter_vec.push(("vlc_c9i:".to_string(), VideoCardStateEntry::String(format!("{}", self.vlc_c9i))));
        counter_vec.push(("last_row:".to_string(), VideoCardStateEntry::String(format!("{}", self.last_row))));
        counter_vec.push(("last_line:".to_string(), VideoCardStateEntry::String(format!("{}", self.last_line))));
        counter_vec.push(("vtac_c5:".to_string(), VideoCardStateEntry::String(format!("{}", self.vtac_c5))));
        counter_vec.push(("cursor_active".to_string(), VideoCardStateEntry::String(format!("{}", self.cursor_active))));
        counter_vec.push(("frame_parity:".to_string(), VideoCardStateEntry::String(format!("{:?}", self.frame_parity))));


        counter_vec.push(("den:".to_string(), VideoCardStateEntry::String(format!("{:?}", self.den()))));
        counter_vec.push(("hs:".to_string(), VideoCardStateEntry::String(format!("{:?}", self.in_hsync))));
        counter_vec.push(("cs:".to_string(), VideoCardStateEntry::String(format!("{:?}", self.in_vsync))));
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

        crtc.write_register_direct(HorizontalTotalR0, 0x01);
        crtc.write_register_direct(HorizontalDisplayedR1, 0x01);
        crtc.write_register_direct(VerticalTotalR4, 0x01);
        crtc.write_register_direct(VerticalDisplayedR6, 0x01);

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

        crtc.write_register_direct(HorizontalTotalR0, 100);
        crtc.write_register_direct(HorizontalDisplayedR1, 1);
        crtc.write_register_direct(HorizontalSyncPositionR2, 2);
        crtc.write_register_direct(SyncWidthR3, sync_width);

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

        crtc.write_register_direct(HorizontalTotalR0, 3);
        crtc.write_register_direct(HorizontalDisplayedR1, 1);
        crtc.write_register_direct(HorizontalSyncPositionR2, 2);
        crtc.write_register_direct(SyncWidthR3, 1);
        crtc.write_register_direct(VerticalTotalR4, 0);
        crtc.write_register_direct(VerticalTotalAdjustR5, 4);
        crtc.write_register_direct(MaximumScanlineAddressR9, 0);

        crtc.tick();
        assert!(crtc.last_line);

        crtc.write_register_direct(VerticalTotalR4, 1);
        assert!(crtc.last_line);

        while crtc.vlc_c9 == 0 && !crtc.in_vta() {
            crtc.tick();
        }

        assert!(crtc.in_vta());
    }

    #[test]
    fn port_write_register_select() {
        let trace_logger = TraceLogger::None;
        let mut crtc = Crtc6845::new(trace_logger);
        crtc.port_write(0, 4);
        assert_eq!(crtc.reg_select, CrtcRegister::VerticalTotalR4);
    }

    #[test]
    fn port_write_register_write() {
        let trace_logger = TraceLogger::None;
        let mut crtc = Crtc6845::new(trace_logger);
        crtc.port_write(0, 4);
        crtc.port_write(1, 0x7F);
        assert_eq!(crtc.reg[VerticalTotalR4], 0x7F);
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
        assert_eq!(crtc.reg_select, CrtcRegister::InvalidRegister);
    }

    #[test]
    fn write_register_horizontal_total() {
        let trace_logger = TraceLogger::None;
        let mut crtc = Crtc6845::new(trace_logger);
        crtc.select_register(0);
        crtc.write_register(0xFF);
        assert_eq!(crtc.reg[HorizontalTotalR0], 0xFF);
    }

    #[test]
    fn write_register_vertical_total() {
        let trace_logger = TraceLogger::None;
        let mut crtc = Crtc6845::new(trace_logger);
        crtc.select_register(4);
        crtc.write_register(0xFF);
        assert_eq!(crtc.reg[VerticalTotalR4], 0x7F);
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

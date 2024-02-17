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

    ---------------------------------------------------------------------------

    ega::crtc.rs

    Implement the EGA CRTC logic

*/

use super::*;

pub const EGA_VBLANK_MASK: u16 = 0x001F;
pub const EGA_HBLANK_MASK: u8 = 0x001F;
pub const EGA_HSYNC_MASK: u8 = 0x001F;
pub const EGA_HSLC_MASK: u16 = 0x01FF;

const CURSOR_LINE_MASK: u8 = 0b0001_1111;
const AC_LATENCY: u8 = 1;

// Helper macro for pushing video card state entries.
macro_rules! push_reg_str {
    ($vec: expr, $reg: expr, $decorator: expr, $val: expr ) => {
        $vec.push((
            format!("{} {:?}", $decorator, $reg),
            VideoCardStateEntry::String(format!("{}", $val)),
        ))
    };
}

#[derive(Debug)]
pub enum CRTCRegister {
    HorizontalTotal,
    HorizontalDisplayEnd,
    StartHorizontalBlank,
    EndHorizontalBlank,
    StartHorizontalRetrace,
    EndHorizontalRetrace,
    VerticalTotal,
    Overflow,
    PresetRowScan,
    MaximumScanLine,
    CursorStartLine,
    CursorEndLine,
    StartAddressH,
    StartAddressL,
    CursorAddressH,
    CursorAddressL,
    VerticalRetraceStart, // These replace the CGA lightpen registers on EGA
    VerticalRetraceEnd,   //
    VerticalDisplayEnd,
    Offset,
    UnderlineLocation,
    StartVerticalBlank,
    EndVerticalBlank,
    ModeControl,
    LineCompare,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct CCursorEnd {
    pub cursor_end: B5,
    pub cursor_skew: B2,
    #[skip]
    unused: B1,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct CEndHorizontalBlank {
    pub end_horizontal_blank: B5,
    pub display_enable_skew: B2,
    #[skip]
    unused: B1,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct CEndHorizontalRetrace {
    pub end_horizontal_retrace: B5,
    pub horizontal_retrace_delay: B2,
    pub start_odd: B1,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct CVerticalRetraceEnd {
    pub vertical_retrace_end: B4,
    pub cvi: B1,
    pub dvi: B1,
    #[skip]
    unused: B2,
}

#[derive(Debug, BitfieldSpecifier)]
pub enum WordOrByteMode {
    Word,
    Byte,
}

#[derive(Debug, BitfieldSpecifier)]
pub enum CompatibilityMode {
    Cga,
    Ega,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct CModeControl {
    pub compatibility_mode: CompatibilityMode,
    pub select_row_scan_counter: B1,
    pub horizontal_retrace_select: B1,
    pub count_by_two: B1,
    pub output_control: B1,
    pub address_wrap: B1,
    pub word_or_byte_mode: WordOrByteMode,
    pub hardware_reset: B1,
}

#[derive(Copy, Clone, Default, Debug)]
pub struct CrtcStatus {
    pub begin_hsync: bool,
    pub begin_vsync: bool,
    pub hsync: bool,
    pub vsync: bool,
    pub hblank: bool,
    pub vblank: bool,
    pub hborder: bool,
    pub vborder: bool,
    pub den: bool,
    pub den_skew: bool,
    pub cursor: bool,
    pub cref: bool,
}

pub struct EgaCrtc {
    // CRTC registers
    register_select_byte: u8,
    register_selected:    CRTCRegister,

    crtc_horizontal_total: u8,                          // R(0) Horizontal Total
    crtc_horizontal_display_end: u8,                    // R(1) Horizontal Display End
    crtc_start_horizontal_blank: u8,                    // R(2) Start Horizontal Blank
    crtc_end_horizontal_blank: CEndHorizontalBlank,     // R(3) Bits 0-4 - End Horizontal Blank
    crtc_end_horizontal_blank_norm: u8,                 // End Horizontal Blank value normalized to column number
    crtc_display_enable_skew: u8,                       // Calculated from R(3) Bits 5-6
    crtc_start_horizontal_retrace: u8,                  // R(4) Start Horizontal Retrace
    crtc_end_horizontal_retrace: CEndHorizontalRetrace, // R(5) End Horizontal Retrace
    crtc_end_horizontal_retrace_norm: u8,               // End Horizontal Retrace value normalized to column number
    crtc_retrace_width: u8,
    crtc_vertical_total: u16,    // R(6) Vertical Total (9-bit value)
    crtc_overflow: u8,           // R(7) Overflow
    crtc_preset_row_scan: u8,    // R(8) Preset Row Scan
    crtc_maximum_scanline: u8,   // R(9) Max Scanline
    crtc_cursor_start: u8,       // R(A) Cursor Location (9-bit value)
    crtc_cursor_enabled: bool,   // Calculated from R(A) bit 5
    crtc_cursor_end: CCursorEnd, // R(B)
    crtc_cursor_skew: u8,        // Calculated from R(B) bits 5-6
    crtc_start_address_ho: u8,   // R(C)
    crtc_start_address_lo: u8,   // R(D)
    crtc_start_address: u16,     // Calculated from C&D
    start_address_latch: u16,
    crtc_cursor_address_lo: u8, // R(E)
    crtc_cursor_address_ho: u8, // R(F)
    crtc_cursor_address: u16,
    crtc_vertical_retrace_start: u16, // R(10) Vertical Retrace Start (9-bit value)
    crtc_vertical_retrace_end: CVerticalRetraceEnd, // R(11) Vertical Retrace End (5-bit value)
    crtc_vertical_retrace_end_norm: u16, // Vertical Retrace Start value normalized to scanline number
    crtc_vertical_display_end: u16,   // R(12) Vertical Display Enable End (9-bit value)
    crtc_offset: u8,                  // R(13)
    crtc_underline_location: u8,      // R(14)
    crtc_start_vertical_blank: u16,   // R(15) Start Vertical Blank (9-bit value)
    crtc_end_vertical_blank: u16,     // R(16)
    crtc_mode_control: CModeControl,  // R(17)
    crtc_line_compare: u16,           // R(18) Line Compare (9-bit value)

    // CRTC internal counters
    hcc: u8,  // Horizontal character counter (x pos of character)
    vlc: u8,  // Vertical line counter - row of character being drawn
    vcc: u8,  // Vertical character counter (y pos of character)
    slc: u16, // Scanline counter - increments after reaching vertical total
    hsc: u8,  // Horizontal sync counter - counts during hsync period
    vtac_c5: u8,
    in_vta: bool,
    in_hrd: bool,
    hrdc: u8,
    effective_vta: u8,
    vma: u16,             // VMA register - Video memory address
    vma_sl: u16,          // VMA of start of scanline
    vma_t: u16,           // VMA' register - Video memory address temporary
    vmws: usize,          // Video memory word size
    den_skew_front: bool, // Display enable skew control for front porch
    den_skew_back: bool,  // Display enable skew control for back porch
    dsc: u8,              // Display enable skew counter

    pub status: CrtcStatus,
    blink_state: bool,
    monitor_hsync: bool,
    in_last_vblank_line: bool,
    cursor_data: [bool; EGA_CURSOR_MAX],
    frame: u64,
}

impl Default for EgaCrtc {
    fn default() -> Self {
        Self {
            // CRTC registers
            register_selected:    CRTCRegister::HorizontalTotal,
            register_select_byte: 0,

            crtc_horizontal_total: DEFAULT_HORIZONTAL_TOTAL,
            crtc_horizontal_display_end: DEFAULT_HORIZONTAL_DISPLAYED,
            crtc_start_horizontal_blank: DEFAULT_HORIZONTAL_SYNC_POS,
            crtc_end_horizontal_blank: CEndHorizontalBlank::new(),
            crtc_end_horizontal_blank_norm: 0,
            crtc_display_enable_skew: 0,
            crtc_start_horizontal_retrace: 0,
            crtc_end_horizontal_retrace: CEndHorizontalRetrace::new(),
            crtc_end_horizontal_retrace_norm: 0,
            crtc_retrace_width: 0,
            crtc_vertical_total: DEFAULT_VERTICAL_TOTAL,
            crtc_overflow: DEFAULT_OVERFLOW,
            crtc_preset_row_scan: DEFAULT_PRESET_ROW_SCAN,
            crtc_maximum_scanline: DEFAULT_MAX_SCANLINE,
            crtc_cursor_start: DEFAULT_CURSOR_START_LINE,
            crtc_cursor_enabled: true,
            crtc_cursor_end: CCursorEnd::from_bytes([DEFAULT_CURSOR_END_LINE]),
            crtc_cursor_skew: 0,
            crtc_start_address: 0,
            crtc_start_address_ho: 0,
            crtc_start_address_lo: 0,
            start_address_latch: 0,
            crtc_cursor_address_lo: 0,
            crtc_cursor_address_ho: 0,
            crtc_cursor_address: 0,
            crtc_vertical_retrace_start: 0,
            crtc_vertical_retrace_end: CVerticalRetraceEnd::new(),
            crtc_vertical_retrace_end_norm: 0,
            crtc_vertical_display_end: 0,
            crtc_offset: 0,
            crtc_underline_location: 0,
            crtc_start_vertical_blank: 0,
            crtc_end_vertical_blank: 0,
            crtc_mode_control: CModeControl::new(),
            crtc_line_compare: 0,

            hcc: 0,
            vlc: 0,
            vcc: 0,
            slc: 0,
            hsc: 0,
            vtac_c5: 0,
            in_vta: false,
            in_hrd: false,
            hrdc: 0,
            effective_vta: 0,
            vma: 0,
            vma_t: 0,
            vma_sl: 0,
            vmws: 1,
            den_skew_front: false,
            den_skew_back: false,
            dsc: 0,

            status: CrtcStatus::default(),
            blink_state: false,
            monitor_hsync: false,
            in_last_vblank_line: false,
            cursor_data: [false; EGA_CURSOR_MAX],
            frame: 0,
        }
    }
}

impl EgaCrtc {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_crtc_register_address(&mut self, byte: u8) {
        //log::trace!("CGA: CRTC register {:02X} selected", byte);
        self.register_select_byte = byte & 0x1F;

        self.register_selected = match self.register_select_byte {
            0x00 => CRTCRegister::HorizontalTotal,
            0x01 => CRTCRegister::HorizontalDisplayEnd,
            0x02 => CRTCRegister::StartHorizontalBlank,
            0x03 => CRTCRegister::EndHorizontalBlank,
            0x04 => CRTCRegister::StartHorizontalRetrace,
            0x05 => CRTCRegister::EndHorizontalRetrace,
            0x06 => CRTCRegister::VerticalTotal,
            0x07 => CRTCRegister::Overflow,
            0x08 => CRTCRegister::PresetRowScan,
            0x09 => CRTCRegister::MaximumScanLine,
            0x0A => CRTCRegister::CursorStartLine,
            0x0B => CRTCRegister::CursorEndLine,
            0x0C => CRTCRegister::StartAddressH,
            0x0D => CRTCRegister::StartAddressL,
            0x0E => CRTCRegister::CursorAddressH,
            0x0F => CRTCRegister::CursorAddressL,
            0x10 => CRTCRegister::VerticalRetraceStart,
            0x11 => CRTCRegister::VerticalRetraceEnd,
            0x12 => CRTCRegister::VerticalDisplayEnd,
            0x13 => CRTCRegister::Offset,
            0x14 => CRTCRegister::UnderlineLocation,
            0x15 => CRTCRegister::StartVerticalBlank,
            0x16 => CRTCRegister::EndVerticalBlank,
            0x17 => CRTCRegister::ModeControl,
            0x18 => CRTCRegister::LineCompare,
            _ => {
                log::debug!("Select to invalid CRTC register: {:02X}", byte);
                self.register_select_byte = 0;
                CRTCRegister::HorizontalTotal
            }
        }
    }

    /// Write to one of the CRT Controller registers.
    /// Returns a boolean representing whether the card should recalculate mode parameters after this write.
    pub fn write_crtc_register_data(&mut self, byte: u8) -> bool {
        //log::debug!("CGA: Write to CRTC register: {:?}: {:02}", self.crtc_register_selected, byte );
        match self.register_selected {
            CRTCRegister::HorizontalTotal => {
                // (R0) 8 bit write only
                self.crtc_horizontal_total = byte;
            }
            CRTCRegister::HorizontalDisplayEnd => {
                // (R1) 8 bit write only
                self.crtc_horizontal_display_end = byte;
            }
            CRTCRegister::StartHorizontalBlank => {
                // (R2) 8 bit write only
                self.crtc_start_horizontal_blank = byte;
                self.normalize_end_horizontal_blank();
            }
            CRTCRegister::EndHorizontalBlank => {
                // (R3) 8 bit write only
                // Bits 0-4: End Horizontal Blank
                // Bits 5-6: Display Enable Skew
                self.crtc_end_horizontal_blank = CEndHorizontalBlank::from_bytes([byte]);
                self.normalize_end_horizontal_blank();
            }
            CRTCRegister::StartHorizontalRetrace => {
                // (R4)
                self.crtc_start_horizontal_retrace = byte;
                self.normalize_end_horizontal_retrace();
            }
            CRTCRegister::EndHorizontalRetrace => {
                // (R5)
                self.crtc_end_horizontal_retrace = CEndHorizontalRetrace::from_bytes([byte]);

                if self.crtc_end_horizontal_retrace.start_odd() != 0 {
                    log::warn!("som == 1!");
                }
                self.normalize_end_horizontal_retrace();
            }
            CRTCRegister::VerticalTotal => {
                // (R6) 9-bit - Vertical Total
                // Bit 8 in overflow register. Set only lower 8 bits here.
                self.crtc_vertical_total &= 0xFF00;
                self.crtc_vertical_total |= byte as u16;
            }
            CRTCRegister::Overflow => {
                // (R7) 6 bit write only
                // Bit 0: Vertical Total Bit 8
                // Bit 1: Vertical Display Enable Bit 8
                // Bit 2: Vertical Retrace Start Bit 8
                // Bit 3: Start Vertical Blank Bit 8
                // Bit 4: Line Compare Bit 8
                // Bit 5: Cursor Location Bit 8 (??)
                // Bits 6-7: Unused
                self.crtc_overflow = byte;
                self.set_crtc_overflow_bits(byte);
            }
            CRTCRegister::PresetRowScan => {
                // (R8)
                self.crtc_preset_row_scan = byte;
                //log::debug!("Preset row scan changed!");
            }
            CRTCRegister::MaximumScanLine => {
                // (R9)
                self.crtc_maximum_scanline = byte
            }
            CRTCRegister::CursorStartLine => {
                // R(A)
                // Bits 0-4: Cursor Start Line
                // Bit 5: Cursor Enable (This field only valid in VGA)
                // I suppose the only way to disable the cursor on IBM EGA is to position it off
                // the screen.
                //self.crtc_cursor_enabled = byte >> 5 & 0x01 != 0;

                self.crtc_cursor_start = byte & CURSOR_LINE_MASK;
                self.update_cursor_data();
            }
            CRTCRegister::CursorEndLine => {
                // R(B)
                // Bits 0-4: Cursor Start Line
                // Bits 5-6: Cursor Skew
                self.crtc_cursor_end = CCursorEnd::from_bytes([byte]);
                self.update_cursor_data();
            }
            CRTCRegister::StartAddressH => {
                // (RC) - 8 bits. High byte of Cursor Address register.
                // Calculate full address on write.
                self.crtc_start_address_ho = byte;
                self.crtc_start_address &= 0x00FF;
                self.crtc_start_address |= (byte as u16) << 8;
            }
            CRTCRegister::StartAddressL => {
                // (RD) - 8 bits. Low byte of Cursor Address register.
                // Calculate full address on write.
                self.crtc_start_address_lo = byte;
                self.crtc_start_address &= 0xFF00;
                self.crtc_start_address |= byte as u16;
                //log::debug!("CGA: Start address set to: {:04X}", self.crtc_start_address);
            }
            CRTCRegister::CursorAddressH => {
                // (RE) - 8 bits.  High byte of Cursor Address register
                self.crtc_cursor_address_ho = byte;
                self.crtc_cursor_address &= 0x00FF;
                self.crtc_cursor_address |= (byte as u16) << 8;
            }
            CRTCRegister::CursorAddressL => {
                // (RF) - 8 bits. Low byte of Cursor Address register.
                self.crtc_cursor_address_lo = byte;
                self.crtc_cursor_address &= 0xFF00;
                self.crtc_cursor_address |= byte as u16;
            }
            CRTCRegister::VerticalRetraceStart => {
                // (R10) 9 bits - Vertical Retrace Start
                // Bit 8 in overflow register. Set only lower 8 bits here.
                self.crtc_vertical_retrace_start &= 0xFF00;
                self.crtc_vertical_retrace_start |= byte as u16;
                self.normalize_end_vertical_retrace();
            }
            CRTCRegister::VerticalRetraceEnd => {
                // (R11) Vertical Retrace End
                // Bit 7: Protect bit
                // Bit 6: Bandwidth bit (ignored)
                self.crtc_vertical_retrace_end = CVerticalRetraceEnd::from_bytes([byte]);
                self.normalize_end_vertical_retrace();
            }
            CRTCRegister::VerticalDisplayEnd => {
                // (R12) 9 bits - Vertical Display End
                // Bit 8 in overflow register. Set only lower 8 bits here.
                self.crtc_vertical_display_end &= 0xFF00;
                self.crtc_vertical_display_end |= byte as u16;
            }
            CRTCRegister::Offset => {
                // (R13) 8 bits -
                self.crtc_offset = byte;
            }
            CRTCRegister::UnderlineLocation => {
                // (R14) 5 bits - Scanline at which underlining occurs
                self.crtc_underline_location = byte & 0x1F;
            }
            CRTCRegister::StartVerticalBlank => {
                // R(15) - Start Vertical Blank
                // bit 8 in overflow register. Set only lower 8 bits here.
                self.crtc_start_vertical_blank &= 0xFF00;
                self.crtc_start_vertical_blank |= byte as u16;
            }
            CRTCRegister::EndVerticalBlank => {
                // R(16) - Bits 0-3: End Vertical Blank
                self.crtc_end_vertical_blank = (byte & 0x0F) as u16;
            }
            CRTCRegister::ModeControl => {
                // (R17) Mode Control Register
                self.write_crtc_mode_control_register(byte);
            }
            CRTCRegister::LineCompare => {
                // (R18) Line Compare Register
                // Bit 8 in overflow register. Set only lower 8 bits here.
                self.crtc_line_compare &= 0xFF00;
                self.crtc_line_compare |= byte as u16;
            }
        }
        true
    }

    /// Update the miscellaneous registers that share bits with the Overflow register.
    ///
    /// This register is Write-only on the EGA, so it must have been a real pain to maintain
    /// Bit 0: Vertical Total Bit 8
    /// Bit 1: Vertical Display Enable End Bit 8
    /// Bit 2: Vertical Retrace Start Bit 8
    /// Bit 3: Start Vertical Blank Bit 8
    /// Bit 4: Line Compare Bit 8
    /// Bit 5: Cursor Location Bit 8 (?? See note below)
    /// Bits 6-7: Unused
    fn set_crtc_overflow_bits(&mut self, byte: u8) {
        // Bit 0: Vertical Total
        self.crtc_vertical_total &= 0x00FF;
        self.crtc_vertical_total |= (byte as u16 & 0x01) << 8;
        // Bit 1: Vertical Display Enable End
        self.crtc_vertical_display_end &= 0x00FF;
        self.crtc_vertical_display_end |= (byte as u16 >> 1 & 0x01) << 8;
        // Bit 2: Vertical Retrace Start
        self.crtc_vertical_retrace_start &= 0x00FF;
        self.crtc_vertical_retrace_start |= (byte as u16 >> 2 & 0x01) << 8;
        // Bit 3: Start Vertical Blank
        self.crtc_start_vertical_blank &= 0x00FF;
        self.crtc_start_vertical_blank |= (byte as u16 >> 3 & 0x01) << 8;
        // Bit 4: Line Compare
        self.crtc_line_compare &= 0x00FF;
        self.crtc_line_compare |= (byte as u16 >> 4 & 0x01) << 8;

        // In IBM's documentation, bit 5 is specified to be "Bit 8" of register 0x0A,
        // but they call it the "Cursor Location" register in the same paragraph, which is
        // contradictory. 0x0A is the Cursor Start Line register, not Location.
        // 0x0A is also only 5 bits, so it doesn't need a Bit 8 from Overflow.

        // The Programmers Guide to the EGA/VGA Cards also refers to the Cursor Location
        // register. But the Cursor Location is already a split 16 bit register with H/L
        // bytes.  So where does that 8th bit go exactly? Mysterious.
    }

    /// Calculate the normalized End Horizontal Blank value
    ///
    /// The value stored in the End Horizontal Blank field of the End Horizontal Blank
    /// register is actually the 5 low order bits to compare against the current column
    /// counter to determine when the horizontal blank period is over. We convert this
    /// into the actual column number.
    fn normalize_end_horizontal_blank(&mut self) {
        let ehb = self.crtc_end_horizontal_blank.end_horizontal_blank();

        let mut proposed_ehb = self.crtc_start_horizontal_blank & 0xE0 | ehb;
        if proposed_ehb <= self.crtc_start_horizontal_blank {
            proposed_ehb = (self.crtc_start_horizontal_blank + 0x20) & 0xE0 | ehb;
        }

        if proposed_ehb > self.crtc_horizontal_total {
            // Wrap at HT
            proposed_ehb = ehb
        }
        self.crtc_end_horizontal_blank_norm = proposed_ehb;
    }

    /// Calculate the normalized End Horizontal Retrace value
    ///
    /// The value stored in the End Horizontal Retrace field of the End Horizontal Retrace
    /// register is actually the 5 low order bits to compare against the current column
    /// counter to determine when the horizontal retrace period is over. We convert this
    /// into the actual column number.
    fn normalize_end_horizontal_retrace(&mut self) {
        let ehr = self.crtc_end_horizontal_retrace.end_horizontal_retrace();

        let mut proposed_ehr = self.crtc_start_horizontal_retrace & 0xE0 | ehr;
        if proposed_ehr <= self.crtc_start_horizontal_retrace {
            proposed_ehr = (self.crtc_start_horizontal_retrace + 0x20) & 0xE0 | ehr;
        }

        if proposed_ehr > self.crtc_horizontal_total {
            // Wrap at HT
            proposed_ehr = ehr;
            self.crtc_retrace_width = self.crtc_horizontal_total - self.crtc_start_horizontal_retrace + ehr;
        }
        else {
            self.crtc_retrace_width = proposed_ehr - self.crtc_start_horizontal_retrace;
        }
        self.crtc_end_horizontal_retrace_norm = proposed_ehr;
    }

    /// Calculate the normalized Vertical Retrace End value
    ///
    /// The value stored in the Vertical Retrace End field of the Vertical Retrace End
    /// register is actually the 5 low order bits to compare against the current scanline
    /// counter to determine when the vertical retrace period is over. We convert this
    /// into the actual scanline number.
    fn normalize_end_vertical_retrace(&mut self) {
        let evr = self.crtc_vertical_retrace_end.vertical_retrace_end() as u16;

        let mut proposed_evr = self.crtc_vertical_retrace_start & 0xFFE0 | evr;
        if proposed_evr <= self.crtc_vertical_retrace_start {
            proposed_evr = (self.crtc_vertical_retrace_start + 0x20) & 0xFFE0 | evr;
        }

        if proposed_evr > self.crtc_vertical_total {
            // Wrap at VT
            proposed_evr = evr
        }
        self.crtc_vertical_retrace_end_norm = proposed_evr;
    }

    pub fn read_crtc_register(&mut self) -> u8 {
        match self.register_selected {
            CRTCRegister::CursorStartLine => self.crtc_cursor_start,
            CRTCRegister::CursorEndLine => self.crtc_cursor_end.into_bytes()[0],
            CRTCRegister::CursorAddressH => {
                //log::debug!("CGA: Read from CRTC register: {:?}: {:02}", self.crtc_register_selected, self.crtc_cursor_address_ho );
                self.crtc_cursor_address_ho
            }
            CRTCRegister::CursorAddressL => {
                //log::debug!("CGA: Read from CRTC register: {:?}: {:02}", self.crtc_register_selected, self.crtc_cursor_address_lo );
                self.crtc_cursor_address_lo
            }
            _ => {
                log::debug!("Read from unsupported CRTC register: {:?}", self.register_selected);
                0
            }
        }
    }

    /// Handle a write to the CRTC Mode Control Register (R17)
    fn write_crtc_mode_control_register(&mut self, byte: u8) {
        self.crtc_mode_control = CModeControl::from_bytes([byte]);
    }

    /// Update the cursor data array based when either cursor_start or cursor_end have changed.
    fn update_cursor_data(&mut self) {
        // Reset cursor data to 0.
        self.cursor_data.fill(false);

        // Start line must be reached when iterating through character rows to draw a cursor at all.
        // Therefore, if start_line > maximum_scanline, the cursor is disabled.
        if self.crtc_cursor_start > self.crtc_maximum_scanline {
            return;
        }

        // If start == end, a single line of cursor is drawn.
        if self.crtc_cursor_start == self.crtc_cursor_end.cursor_end() {
            self.cursor_data[self.crtc_cursor_start as usize] = true;
            return;
        }

        if self.crtc_cursor_start <= self.crtc_cursor_end.cursor_end() {
            // Normal cursor definition. Cursor runs from start_line to end_line - 1
            // EGA differs from CGA in this regard as the CGA's cursor runs from start_line to end_line.
            for i in self.crtc_cursor_start..=self.crtc_cursor_end.cursor_end().saturating_sub(1) {
                self.cursor_data[i as usize] = true;
            }
        }
        else {
            // The EGA will draw a single scanline instead of a split cursor if (end % 16) == start
            // https://www.pcjs.org/blog/2018/03/20/
            if (self.crtc_cursor_end.cursor_end() & 0x0F) == self.crtc_cursor_start {
                self.cursor_data[self.crtc_cursor_start as usize] = true;
                return;
            }

            // Split cursor.
            for i in 0..self.crtc_cursor_end.cursor_end().saturating_sub(1) {
                // First part of cursor is 0->end_line
                self.cursor_data[i as usize] = true;
            }

            for i in (self.crtc_cursor_start as usize)..EGA_CURSOR_MAX {
                // Second part of cursor is start_line->max
                self.cursor_data[i] = true;
            }
        }
    }

    /// Update the CRTC logic for next character.
    pub fn tick(&mut self, clock_divisor: u32) -> u16 {
        // Reset hsync and vsync edge-triggered flags
        self.status.begin_hsync = false;
        self.status.begin_vsync = false;

        // Advance video memory address offset and grab the next character + attr
        self.vma += 1;

        // Update horizontal character counter
        self.hcc = self.hcc.wrapping_add(1);

        // Process horizontal sync period
        if self.status.hblank {
            // End horizontal blank when we reach R3
            if (self.hcc & EGA_HBLANK_MASK) == self.crtc_end_horizontal_blank.end_horizontal_blank() {
                self.status.hblank = false;
                self.status.hborder = true;
            }
        }

        // Process horizontal sync period
        if self.monitor_hsync {
            // Increment horizontal sync counter (wrapping)
            self.hsc = self.hsc.wrapping_add(1);

            // Implement a fixed hsync width from the monitor's perspective -
            // A wider programmed hsync width than these values shifts the displayed image to the right.
            let hsync_target = if clock_divisor == 1 { std::cmp::min(6, 6) } else { 3 };

            // Do a horizontal sync
            if self.hsc == hsync_target {
                // Update the video mode, if an update is pending.
                // It is important not to change graphics mode while we are catching up during an IO instruction.

                /* TODO: implement deferred mode change for EGA?
                if !self.catching_up && self.mode_pending {
                    self.update_mode();
                    self.mode_pending = false;
                }*/

                //log::debug!("doing monitor hsync");
                self.do_hsync();

                // CRTC may still be in hsync at this point (if the programmed CRTC hsync width is larger
                // than our fixed hsync value)
                self.monitor_hsync = false;
            }
        }

        if self.status.hsync {
            // End horizontal sync when we reach R3
            if ((self.hcc - 1) & EGA_HSYNC_MASK) == self.crtc_end_horizontal_retrace.end_horizontal_retrace() {
                // Enter horizontal retrace delay time
                //log::debug!("entering hrd @ {}", self.hcc);
                self.in_hrd = true;
                self.hrdc = 0;
            }
        }

        if self.in_hrd {
            if self.hrdc == self.crtc_end_horizontal_retrace.horizontal_retrace_delay() {
                // If the monitor is still in hsync, we can end it now - the monitor hsync
                // only enforces a maximum hsync width, not a minimum.
                // If the monitor is not in hsync, hsync has already occurred, so don't perform one.
                if self.monitor_hsync {
                    self.monitor_hsync = false;
                    self.do_hsync();
                }

                //log::debug!("leaving hrd @ {}", self.hcc);
                self.status.hsync = false;
                self.in_hrd = false;
                self.hrdc = 0;
                self.hsc = 0;
            }
            else {
                self.hrdc = self.hrdc.wrapping_add(1);
            }
        }

        if self.hcc == self.crtc_horizontal_display_end + 1 {
            // Leaving active display area, entering right overscan
            self.den_skew_back = true;
            self.status.den = false;
        }

        if self.hcc == self.crtc_start_horizontal_blank + 1 {
            // Leaving right overscan and entering horizontal blank
            self.status.hborder = false;
            self.status.hblank = true;
            self.status.den = false;
        }

        if self.hcc == self.crtc_start_horizontal_blank + 2 {
            self.status.cref = false; // CRTC stops generating addresses
        }

        if self.hcc == self.crtc_start_horizontal_retrace + self.crtc_end_horizontal_retrace.horizontal_retrace_delay()
        {
            // Entering horizontal retrace. Retrace can start before hblank!

            // Both monitor and CRTC will enter hsync at the same time. Monitor may leave hsync first.
            self.status.hsync = true;
            self.monitor_hsync = true;
            self.status.den = false;
            // Delay toggle of display enable by Display Enable Skew value.
            //self.den_skew = self.crtc_end_horizontal_blank.display_enable_skew();
            self.hsc = 0;
        }

        if self.hcc == self.crtc_horizontal_total && self.in_last_vblank_line {
            // We are one char away from the beginning of the new frame.
            // Draw one char of border
            //self.status.hborder = true;
        }

        if self.hcc == self.crtc_horizontal_total + 1 {
            // Start generating addresses
        }

        // Actual HorizontalTotal is register value + 2 on EGA.
        if self.hcc == self.crtc_horizontal_total + 2 {
            // Leaving left overscan, finished scanning row. Entering active display area with
            // new logical scanline.

            /*
            if self.crtc_vblank {
                // If we are in vblank, advance Vertical Sync Counter
                self.vsc_c3h += 1;
            }
            */
            self.status.cref = true;

            if self.in_last_vblank_line {
                self.in_last_vblank_line = false;
                self.status.vblank = false;
            }

            // Reset Horizontal Character Counter and increment character row counter
            self.hcc = 0;
            //self.status.hborder = false;
            self.vlc += 1;
            // Return video memory address to starting position for next character row
            self.vma = self.vma_sl;

            // Reset the current character glyph to start of row

            if !self.status.vblank {
                // Start the new row
                if self.slc < self.crtc_vertical_display_end + 1 {
                    self.den_skew_front = true;
                }
            }

            if self.vlc > self.crtc_maximum_scanline {
                // C9 == R9 We finished drawing this row of characters

                self.vlc = 0;
                // Advance Vertical Character Counter
                self.vcc = self.vcc.wrapping_add(1);

                // Set vma to starting position for next character row
                // TODO: Offset is multiplied by 2 in byte mode, by 4 in word mode

                self.vma_sl = self.vma_sl + self.crtc_offset as u16 * 2;
                self.vma = self.vma_sl;

                // Load next char + attr
            }

            if self.slc == self.crtc_line_compare {
                // The line compare register is used to reset the effective start address to 0.
                // This is used to implement split screen effects - the top of the screen is drawn from some start
                // address offset, and then the split-screen window is drawn from address 0 after line compare.
                self.vma_sl = 0;
                self.vma = 0;
            }

            if self.slc == self.crtc_vertical_retrace_start {
                // We've reached vertical retrace start. We set the crtc_vblank flag to start comparing hslc against
                // vertical_retrace_end register.

                //trace_regs!(self);
                //trace!(self, "Entering vsync");
                self.status.vblank = true;
                self.status.den = false;
            }

            if self.slc == self.crtc_vertical_display_end + 1 {
                // We are leaving the bottom of the active display area, entering the lower overscan area.
                self.status.vborder = true;
                self.status.den = false;
                self.den_skew_back = true;
                self.status.den_skew = true;

                // Latch CRTC start address at VSYNC (https://www.vogons.org/viewtopic.php?t=57320)
                self.start_address_latch = self.crtc_start_address;
            }

            if self.slc == self.crtc_vertical_total {
                // We have reached vertical total, we are at the end of the top overscan and entering the active
                // display area.
                self.in_vta = false;
                self.vtac_c5 = 0;
                self.slc = 0;

                self.hcc = 0;
                self.vcc = 0;
                self.vlc = 0;

                self.frame += 1;
                // Toggle blink state. This is toggled every 8 frames by default.
                if (self.frame % EGA_CURSOR_BLINK_RATE as u64) == 0 {
                    self.blink_state = !self.blink_state;
                }

                // The SOM (Start Odd/Even Memory Address) register is used to select the starting address for each
                // scanline. If set to 0, even memory addresses are used. If set to 1, odd memory addresses are used.
                // TODO: I actually have no idea how this is implemented

                /*
                let start_addr_som = if self.crtc_end_horizontal_retrace.start_odd() == 0 {
                    // Start on even address (0)
                    if self.crtc_start_address & 1 != 0 {
                        self.crtc_start_address + 1
                    }
                    else {
                        self.crtc_start_address
                    }
                }
                else {
                    // Start on odd address (1)
                    if self.crtc_start_address & 1 == 0 {
                        self.crtc_start_address + 1
                    }
                    else {
                        self.crtc_start_address
                    }
                };

                self.start_address_latch = start_addr_som;
                self.vma = start_addr_som;
                */

                self.vma = self.start_address_latch;
                self.vma_sl = self.vma;

                // Delay toggle of display enable by Display Enable Skew value.
                self.den_skew_front = true;
                //self.status.den_skew = true;
                self.status.vborder = false;
                self.status.vblank = false;
            }
        }

        // Handle DEN skew.  Ideally we would not have a separate status variable for this, but it's
        // a little easier to handle this way. DEN skew is 1 in almost all modes on the EGA as far as I can tell
        if self.den_skew_front {
            if self.dsc == self.crtc_end_horizontal_blank.display_enable_skew() + AC_LATENCY {
                self.den_skew_front = false;
                self.status.vborder = false;
                self.status.hborder = false;
                self.status.den = true;
                self.dsc = 0;
            }
            else {
                self.dsc = self.dsc.wrapping_add(1);
            }
        }

        if self.den_skew_back {
            if self.dsc == self.crtc_end_horizontal_blank.display_enable_skew() + AC_LATENCY {
                self.den_skew_back = false;
                self.status.den_skew = false;
                self.status.hborder = true;
                self.dsc = 0;
            }
            else {
                self.status.den_skew = true;
                self.dsc = self.dsc.wrapping_add(1);
            }
        }

        // Update cursor status
        self.status.cursor = (self.vma == (self.crtc_cursor_address + 1 + self.crtc_cursor_end.cursor_skew() as u16))
            && self.blink_state
            && self.cursor_data[(self.vlc & 0x3F) as usize];

        let mut output_addr = self.vma;

        if let WordOrByteMode::Word = self.crtc_mode_control.word_or_byte_mode() {
            // Word mode selected
            let bit = match self.crtc_mode_control.address_wrap() {
                0 => (self.vma & (1 << 13)) >> 13,
                _ => (self.vma & (1 << 15)) >> 15,
            };
            output_addr = (output_addr << 1) | bit;
        }

        if let CompatibilityMode::Cga = self.crtc_mode_control.compatibility_mode() {
            // Compatibility mode selected. Set A13 to VLC bit 0.
            output_addr = (output_addr & !(1 << 13)) | ((self.vlc as u16 & 0x01) << 13)
        };

        output_addr
    }

    fn do_hsync(&mut self) {
        // Reset hsync delay
        self.in_hrd = false;
        self.hrdc = 0;

        if self.status.vblank {
            //if self.vsc_c3h == CRTC_VBLANK_HEIGHT || self.beam_y == CGA_MONITOR_VSYNC_POS {
            if (self.slc & EGA_VBLANK_MASK) == self.crtc_end_vertical_blank {
                self.in_last_vblank_line = true;
                // We are leaving vblank period. Generate a frame.
                self.status.begin_vsync = true;
                self.monitor_hsync = false;
                return;
            }
        }

        // Restrict HSLC to 9-bit range.
        self.slc = (self.slc + 1) & EGA_HSLC_MASK;
        self.status.begin_hsync = true;
    }

    #[inline]
    pub fn vlc(&self) -> u8 {
        self.vlc
    }

    #[inline]
    pub fn scanline(&self) -> u16 {
        self.slc
    }

    #[inline]
    pub fn maximum_scanline(&self) -> u8 {
        self.crtc_maximum_scanline
    }

    #[inline]
    pub fn in_skew(&self) -> bool {
        self.den_skew_front | (self.den_skew_back && self.dsc < self.crtc_end_horizontal_blank.display_enable_skew())
    }

    #[inline]
    pub fn in_blanking(&self) -> bool {
        self.status.hblank | self.status.vblank
    }

    #[inline]
    pub fn start_address(&self) -> u16 {
        self.crtc_start_address
    }

    pub fn get_cursor_span(&self) -> (u8, u8) {
        (self.crtc_cursor_start, self.crtc_cursor_end.cursor_end())
    }

    pub fn horizontal_display_end(&self) -> u8 {
        self.crtc_horizontal_display_end
    }

    #[rustfmt::skip]
    pub fn get_state(&self) -> Vec<(String, VideoCardStateEntry)> {
        let mut crtc_vec = Vec::new();
        push_reg_str!(crtc_vec, CRTCRegister::HorizontalTotal, "[R00]", self.crtc_horizontal_total);
        push_reg_str!(crtc_vec, CRTCRegister::HorizontalDisplayEnd, "[R01]", self.crtc_horizontal_display_end);
        push_reg_str!(crtc_vec, CRTCRegister::StartHorizontalBlank, "[R02]", self.crtc_start_horizontal_blank);
        push_reg_str!(crtc_vec, CRTCRegister::EndHorizontalBlank, "[R03]", self.crtc_end_horizontal_blank.end_horizontal_blank());
        push_reg_str!(crtc_vec, CRTCRegister::EndHorizontalBlank, "[R03:des]", self.crtc_end_horizontal_blank.display_enable_skew());
        push_reg_str!(crtc_vec, CRTCRegister::EndHorizontalBlank, "[R03:norm]", self.crtc_end_horizontal_blank_norm);
        push_reg_str!(crtc_vec, CRTCRegister::StartHorizontalRetrace, "[R04]", self.crtc_start_horizontal_retrace);
        push_reg_str!(crtc_vec, CRTCRegister::EndHorizontalRetrace, "[R05]", self.crtc_end_horizontal_retrace.end_horizontal_retrace());
        push_reg_str!(crtc_vec, CRTCRegister::EndHorizontalRetrace, "[R05:hrd]", self.crtc_end_horizontal_retrace.horizontal_retrace_delay());
        push_reg_str!(crtc_vec, CRTCRegister::EndHorizontalRetrace, "[R05:som]", self.crtc_end_horizontal_retrace.start_odd());
        push_reg_str!(crtc_vec, CRTCRegister::EndHorizontalRetrace, "[R05:norm]", self.crtc_end_horizontal_retrace_norm);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalTotal, "[R06]", self.crtc_vertical_total);
        push_reg_str!(crtc_vec, CRTCRegister::Overflow, "[R07]", self.crtc_overflow);
        push_reg_str!(crtc_vec, CRTCRegister::PresetRowScan, "[R08]", self.crtc_preset_row_scan);
        push_reg_str!(crtc_vec, CRTCRegister::MaximumScanLine, "[R09]", self.crtc_maximum_scanline);
        push_reg_str!(crtc_vec, CRTCRegister::CursorStartLine, "[R0A]", self.crtc_cursor_start);
        push_reg_str!(crtc_vec, CRTCRegister::CursorEndLine, "[R0B]", self.crtc_cursor_end.into_bytes()[0]);
        push_reg_str!(crtc_vec, CRTCRegister::StartAddressH, "[R0C]", self.crtc_start_address_ho);
        push_reg_str!(crtc_vec, CRTCRegister::StartAddressL, "[R0D]", self.crtc_start_address_lo);
        push_reg_str!(crtc_vec, CRTCRegister::CursorAddressH, "[R0E]", self.crtc_cursor_address_ho);
        push_reg_str!(crtc_vec, CRTCRegister::CursorAddressL, "[R0F]", self.crtc_cursor_address_lo);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalRetraceStart, "[R10]", self.crtc_vertical_retrace_start);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalRetraceEnd, "[R11]", self.crtc_vertical_retrace_end.vertical_retrace_end());
        push_reg_str!(crtc_vec, CRTCRegister::VerticalRetraceEnd, "[R11:norm]", self.crtc_vertical_retrace_end_norm);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalDisplayEnd, "[R12]", self.crtc_vertical_display_end);
        push_reg_str!(crtc_vec, CRTCRegister::Offset, "[R13]", self.crtc_offset);
        push_reg_str!(crtc_vec, CRTCRegister::UnderlineLocation, "[R14]", self.crtc_underline_location);
        push_reg_str!(crtc_vec, CRTCRegister::StartVerticalBlank, "[R15]", self.crtc_start_vertical_blank);
        push_reg_str!(crtc_vec, CRTCRegister::EndVerticalBlank, "[R16]", self.crtc_end_vertical_blank);
        //push_reg_str!(crtc_vec, CRTCRegister::ModeControl, "[R17]", self.crtc_mode_control.into_bytes()[0]);
        crtc_vec.push(("[R17] ModeControl".to_string(), VideoCardStateEntry::String(format!("{:08b}",self.crtc_mode_control.into_bytes()[0]))));
        push_reg_str!(crtc_vec, CRTCRegister::LineCompare, "[R18]", self.crtc_line_compare);

        crtc_vec
    }

    #[rustfmt::skip]
    pub fn get_counter_state(&self) ->  Vec<(String, VideoCardStateEntry)> {
        let mut internal_vec = Vec::new();
        internal_vec.push(("hcc:".to_string(), VideoCardStateEntry::String(format!("{}", self.hcc))));
        internal_vec.push(("vlc:".to_string(), VideoCardStateEntry::String(format!("{}", self.vlc))));
        internal_vec.push(("vcc:".to_string(), VideoCardStateEntry::String(format!("{}", self.vcc))));
        internal_vec.push(("hslc:".to_string(), VideoCardStateEntry::String(format!("{}", self.slc))));
        internal_vec.push(("hsc:".to_string(), VideoCardStateEntry::String(format!("{}", self.hsc))));
        internal_vec.push(("vma':".to_string(), VideoCardStateEntry::String(format!("{:04X}", self.vma_t))));
        internal_vec.push(("vmws:".to_string(), VideoCardStateEntry::String(format!("{}", self.vmws))));
        internal_vec.push(("den:".to_string(), VideoCardStateEntry::String(format!("{:?}", self.status.den))));
        internal_vec.push(("den_skew:".to_string(), VideoCardStateEntry::String(format!("{:?}", self.status.den_skew))));
        internal_vec.push(("ds_front:".to_string(), VideoCardStateEntry::String(format!("{:?}", self.den_skew_front))));
        internal_vec.push(("ds_back:".to_string(), VideoCardStateEntry::String(format!("{:?}", self.den_skew_back))));
        internal_vec.push(("dsc:".to_string(), VideoCardStateEntry::String(format!("{}", self.dsc))));
        internal_vec.push(("hblank:".to_string(), VideoCardStateEntry::String(format!("{}", self.status.hblank))));
        internal_vec.push(("vblank:".to_string(), VideoCardStateEntry::String(format!("{}", self.status.vblank))));
        internal_vec.push(("hborder:".to_string(), VideoCardStateEntry::String(format!("{}", self.status.hborder))));
        internal_vec.push(("vborder:".to_string(), VideoCardStateEntry::String(format!("{}", self.status.vborder))));
        internal_vec
    }
}

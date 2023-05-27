/*
    vga_crtc_regs.rs

    Implement the CRTC Registers of the IBM VGA Card

*/

use modular_bitfield::prelude::*;

use crate::devices::vga::*;

const CURSOR_LINE_MASK: u8      = 0b0001_1111;

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

#[derive(Debug, BitfieldSpecifier)]
pub enum DRAMBandwidth {
    ThreeCycles,
    FiveCycles
}

#[derive(Debug, BitfieldSpecifier)]
pub enum CompatibilityMode {
    SubstituteA0,
    Normal
}

#[derive(Debug, BitfieldSpecifier)]
pub enum SelectRowScanCounter {
    SubstituteA14,
    Sequential
}

#[derive(Debug, BitfieldSpecifier)]
pub enum HorizontalRetraceSelect {
    ClockOnce,
    ClockDividedByTwo
}

#[derive(Debug, BitfieldSpecifier)]
pub enum AddressWrap {
    AddressBit13,
    AddressBit15
}

#[derive(Debug, BitfieldSpecifier)]
pub enum WordByteMode {
    WordMode,
    ByteMode
}

#[derive(Debug, BitfieldSpecifier)]
pub enum HardwareReset {
    ResetHold,
    Enable
}

#[bitfield]
#[derive (Copy, Clone)]
pub struct CEndHorizontalBlank {
    pub end_horizontal_blank: B5,
    pub display_enable_skew: B2,
    pub compatible_read: bool
}

#[bitfield]
#[derive (Copy, Clone)]
pub struct CEndHorizontalRetrace {
    pub end_horizontal_retrace: B5,
    pub horizontal_retrace_delay: B2,
    pub ehb_bit_5: B1
}

#[bitfield]
#[derive (Copy, Clone)]
pub struct CPresetRowScan {
    pub preset_row_scan: B5,
    pub byte_panning: B2,
    #[skip]
    unused: B1
}

#[bitfield]
#[derive (Copy, Clone)]
pub struct CMaximumScanline {
    pub maximum_scanline: B5,
    pub vbs_bit_9: B1,
    pub lc_bit_9 : B1,
    pub two_to_four: bool
}

#[bitfield]
#[derive (Copy, Clone)]
pub struct CCursorStart {
    pub cursor_start: B5,
    pub cursor_enable: bool,
    #[skip]
    unused: B2
}

#[bitfield]
#[derive (Copy, Clone)]
pub struct CCursorEnd {
    pub cursor_end: B5,
    pub cursor_skew: B2,
    #[skip]
    unused: B1
}

#[bitfield]
#[derive (Copy, Clone)]
pub struct CVerticalRetraceEnd {
    pub vertical_retrace_end: B4,
    pub cvi: B1, // Unused on VGA
    pub dvi: B1, // Unused on VGA
    pub bw: DRAMBandwidth,
    pub protect_regs: bool,
}

#[bitfield]
#[derive (Copy, Clone)]
pub struct CUnderlineLocation {
    pub underline_location: B5,
    pub count_by_four: bool,
    pub double_word_mode: bool,
    #[skip]
    unused: B1
}

#[bitfield]
#[derive (Copy, Clone)]
pub struct CModeControl {
    pub compatibility_mode: CompatibilityMode,
    pub select_row_scan_counter: SelectRowScanCounter,
    pub horizontal_retrace_select: HorizontalRetraceSelect,
    pub count_by_two: bool,
    #[skip]
    unused: B1, // Output Control on EGA, not used on VGA
    pub address_wrap: AddressWrap,
    pub word_byte_mode: WordByteMode,
    pub hardware_reset: HardwareReset,
}

impl VGACard {
    pub fn write_crtc_register_address(&mut self, byte: u8 ) {

        //log::trace!("CGA: CRTC register {:02X} selected", byte);
        self.crtc_register_select_byte = byte & 0x1F;

        self.crtc_register_selected = match self.crtc_register_select_byte {
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
                trace!(self, "Select to invalid CRTC register: {:02X}", byte);
                log::debug!("Select to invalid CRTC register: {:02X}", byte);
                self.crtc_register_select_byte = 0;
                CRTCRegister::HorizontalTotal
            } 
        }
    }

    /// Write a value to the currently selected CRTC Register
    /// 
    /// Bit 7 of the Vertical Retrace End register protects registers 0-7 from 
    /// inadvertent writes from programs expecting an EGA card.
    pub fn write_crtc_register_data(&mut self, byte: u8 ) {

        //log::debug!("CGA: Write to CRTC register: {:?}: {:02}", self.crtc_register_selected, byte );
        match self.crtc_register_selected {
            CRTCRegister::HorizontalTotal => {
                // (R0) 8 bit write only
                if !self.protect_crtc_registers {
                    self.crtc_horizontal_total = byte;
                }
            },
            CRTCRegister::HorizontalDisplayEnd => {
                // (R1) 8 bit write only
                if !self.protect_crtc_registers {
                    self.crtc_horizontal_display_end = byte;
                }
            }
            CRTCRegister::StartHorizontalBlank => {
                // (R2) 8 bit write only
                if !self.protect_crtc_registers {
                    self.crtc_start_horizontal_blank = byte;
                }
            },
            CRTCRegister::EndHorizontalBlank => {
                // (R3) 8 bit write only
                // Bits 0-4: End Horizontal Blank
                // Bits 5-6: Display Enable Skew
                if !self.protect_crtc_registers {
                    // Bits 0-4 from EndHorizontalBlank, bit 5 from EndHorizontalRetrace bit 7
                    self.crtc_end_horizontal_blank = CEndHorizontalBlank::from_bytes([byte]);
                    self.normalize_end_horizontal_blank()
                }
            },
            CRTCRegister::StartHorizontalRetrace => {
                // (R4) 
                if !self.protect_crtc_registers {
                    self.crtc_start_horizontal_retrace = byte;
                }
            },
            CRTCRegister::EndHorizontalRetrace => {
                // (R5) 
                if !self.protect_crtc_registers {
                    self.crtc_end_horizontal_retrace = CEndHorizontalRetrace::from_bytes([byte]);
                    self.normalize_end_horizontal_retrace();
                    // Set Bit 5 of End Horizontal Blank register.
                    self.normalize_end_horizontal_blank();
                    
                }
            }
            CRTCRegister::VerticalTotal => {
                // (R6) 9-bit - Vertical Total
                // Bit 8 in register. Set only lower 8 bits here.
                if !self.protect_crtc_registers {
                    self.crtc_vertical_total &= 0xFF00;
                    self.crtc_vertical_total |= byte as u16; 
                }
            },
            CRTCRegister::Overflow => {
                // (R7) 6 bit write only
                // Bit 0: Vertical Total Bit 8
                // Bit 1: Vertical Display Enable Bit 8
                // Bit 2: Vertical Retrace Start Bit 8
                // Bit 3: Start Vertical Blank Bit 8
                // Bit 4: Line Compare Bit 8
                // Bit 5: Cursor Location Bit 8 (??)
                // Bits 6-7: Unused
                if !self.protect_crtc_registers {
                    self.crtc_overflow = byte;
                    self.set_crtc_overflow_bits(byte);
                }
                else {
                    // Ferraro: "The exception is the line compare field (LC) in the Overflow Register." 
                    self.crtc_overflow = (self.crtc_overflow & !0x10) | (byte & 0x10);
                    self.set_crtc_overflow_bits(byte);
                }
            },
            CRTCRegister::PresetRowScan => {
                // (R8)
                self.crtc_preset_row_scan = CPresetRowScan::from_bytes([byte]);
            },            
            CRTCRegister::MaximumScanLine => {
                // (R9) 
                self.crtc_maximum_scanline = CMaximumScanline::from_bytes([byte]);
                // Contains bit #9 for both Start Vertical Blanking and Line Compare registers.
                self.crtc_start_vertical_blank &= 0x01FF;
                self.crtc_start_vertical_blank |= (self.crtc_maximum_scanline.vbs_bit_9() as u16) << 9;

                self.crtc_line_compare &= 0x01FF;
                self.crtc_line_compare |= (self.crtc_maximum_scanline.lc_bit_9() as u16) << 9;                
            }            
            CRTCRegister::CursorStartLine => {
                // R(A)
                // Bits 0-4: Cursor Start Line
                // Bit 5: Cursor Enable (This field only valid in VGA)
                self.crtc_cursor_start = CCursorStart::from_bytes([byte]);
            }
            CRTCRegister::CursorEndLine => {
                // R(B)
                // Bits 0-4: Cursor Start Line     
                // Bits 5-6: Cursor Skew        
                self.crtc_cursor_end = CCursorEnd::from_bytes([byte]);
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
            }            
            CRTCRegister::CursorAddressH => {
                // (RE) - 8 bits.  High byte of Cursor Address register
                self.crtc_cursor_address_ho = byte
            }
            CRTCRegister::CursorAddressL => {
                // (RF) - 8 bits. Low byte of Cursor Address register.
                self.crtc_cursor_address_lo = byte
            }
            CRTCRegister::VerticalRetraceStart => {
                // (R10) - 9 bits - Vertical Retrace Start
                // Bit 8 in overflow register. Set only lower 8 bits here.

                // Access controlled by CR bit in End Horizontal Blank reg. 
                if self.crtc_end_horizontal_blank.compatible_read() {
                    self.crtc_vertical_retrace_start &= 0xFF00;
                    self.crtc_vertical_retrace_start |= byte as u16;
                    self.normalize_end_vertical_retrace();
                }
            }
            CRTCRegister::VerticalRetraceEnd => {
                // (R11) Vertical Retrace End
                // Bit 6: Bandwidth bit (VGA)
                // Bit 7: Protect registers 0-7

                // Access controlled by CR bit in End Horizontal Blank reg. 
                if self.crtc_end_horizontal_blank.compatible_read() {                
                    self.crtc_vertical_retrace_end = CVerticalRetraceEnd::from_bytes([byte]);
                    self.protect_crtc_registers = self.crtc_vertical_retrace_end.protect_regs();
                    self.normalize_end_vertical_retrace();
                }
            }
            CRTCRegister::VerticalDisplayEnd => {
                // (R12) 9 bits - Vertical Display End
                // Bit 8 in overflow register. Set only lower 8 bits here.
                self.crtc_vertical_display_end &= 0xFF00;
                self.crtc_vertical_display_end |= byte as u16;
            },
            CRTCRegister::Offset => {
                // (R13) 8 bits - 
                self.crtc_offset = byte;
            }
            CRTCRegister::UnderlineLocation => {
                // (R14) Bits 0-4 - Scanline at which underlining occurs
                //       Bit 5 - Count by Four
                //       Bit 6 - Double Word Mode
                self.crtc_underline_location = CUnderlineLocation::from_bytes([byte]);
            }
            CRTCRegister::StartVerticalBlank => {
                // R(15) - Start Vertical Blank
                // bit 8 in overflow register. Set only lower 8 bits here.
                self.crtc_start_vertical_blank &= 0xFF00;
                self.crtc_start_vertical_blank |= byte as u16;
                self.normalize_end_vertical_blank();
            }
            CRTCRegister::EndVerticalBlank => {
                // R(16) - Bits 0-6: End Vertical Blank (Expanded on VGA)
                self.crtc_end_vertical_blank = byte & 0x7F;
                self.normalize_end_vertical_blank();
            }
            CRTCRegister::ModeControl => {
                // (R17) Mode Control Register
                self.crtc_mode_control = CModeControl::from_bytes([byte]);
            }
            CRTCRegister::LineCompare => {
                // (R18) Line Compare Register
                // Bit 8 in overflow register. Set only lower 8 bits here.
                self.crtc_line_compare &= 0xFF00;
                self.crtc_line_compare |= byte as u16;
            }
        }
        self.recalculate_mode();
        self.recalculate_timings();
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
    pub fn set_crtc_overflow_bits(&mut self, byte: u8) {

        // Bit 0: Vertical Total Bit 8
        self.crtc_vertical_total &= 0x00FF;
        self.crtc_vertical_total |= (byte as u16 & 0x01) << 8;
        // Bit 1: Vertical Display Enable End Bit 8
        self.crtc_vertical_display_end &= 0x00FF;
        self.crtc_vertical_display_end |= (byte as u16 >> 1 & 0x01) << 8;
        // Bit 2: Vertical Retrace Start Bit 8
        self.crtc_vertical_retrace_start &= 0x00FF;
        self.crtc_vertical_retrace_start |= (byte as u16 >> 2 & 0x01) << 8;
        // Bit 3: Start Vertical Blank Bit 8
        self.crtc_start_vertical_blank &= 0x00FF;
        self.crtc_start_vertical_blank |= (byte as u16 >> 3 & 0x01) << 8;
        // Bit 4: Line Compare
        self.crtc_line_compare &= 0x00FF;
        self.crtc_line_compare |= (byte as u16 >> 4 & 0x01) << 8;
        // VGA Specific bits:
        // Bit 5: Vertical Total Bit 9 (VGA)
        self.crtc_vertical_total &= 0x01FF;
        self.crtc_vertical_total |= (byte as u16 >> 5 & 0x01) << 9;
        // Bit 6: Vertical Display Enable End Bit 9 (VGA)
        self.crtc_vertical_display_end &= 0x01FF;
        self.crtc_vertical_display_end |= (byte as u16 >> 6 & 0x01) << 9;
        // Bit 7: Vertical Retrace Start Bit 9 (VGA)
        self.crtc_vertical_retrace_start &= 0x01FF;
        self.crtc_vertical_retrace_start |= (byte as u16 >> 7 & 0x01) << 8;

    }

    /// Calculate the normalized End Horizontal Blank value
    /// 
    /// The value stored in the End Horizontal Blank field of the End Horizontal Blank
    /// register is actually the 6 low order bits to compare against the current column
    /// counter to determine when the horizontal blanking period is over. We convert this 
    /// into the actual column number. 
    fn normalize_end_horizontal_blank(&mut self) {

        let bit_5 = self.crtc_end_horizontal_retrace.ehb_bit_5();
        let ehb = self.crtc_end_horizontal_blank.end_horizontal_blank() | (bit_5 << 5);

        let mut proposed_ehb = self.crtc_start_horizontal_blank & 0xC0 | ehb;
        if proposed_ehb <= self.crtc_start_horizontal_blank {
            proposed_ehb = (self.crtc_start_horizontal_blank + 0x40) & 0xC0 | ehb;
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

        self.crtc_vertical_retrace_end_norm = proposed_evr;
    }    

    /// Calculate the normalized Vertical Blank End value
    /// 
    /// The value stored in the Vertical Blank End register is actually the lower 7 
    /// bits to compare against the current scanline counter to determine when the
    /// vertical blank period is over. We convert this into the actual scanline number. 
    fn normalize_end_vertical_blank(&mut self) {

        let evb = self.crtc_end_vertical_blank as u16;

        let mut proposed_evb = self.crtc_start_vertical_blank as u16 & 0xFF80 | evb;
        if proposed_evb <= self.crtc_start_vertical_blank {
            proposed_evb = (self.crtc_start_vertical_blank + 0x80) & 0xFF80 | evb;
        }

        self.crtc_end_vertical_blank_norm = proposed_evb;
    }    


    /// Handle a read from the selected CRTC register.
    /// 
    /// Unlike the EGA, most of the VGA CRTC registers are readable.
    pub fn read_crtc_register(&mut self ) -> u8 {
        match self.crtc_register_selected {
            CRTCRegister::HorizontalTotal => self.crtc_horizontal_total,
            CRTCRegister::HorizontalDisplayEnd => self.crtc_horizontal_display_end,
            CRTCRegister::StartHorizontalBlank => self.crtc_start_horizontal_blank,
            CRTCRegister::EndHorizontalBlank => self.crtc_end_horizontal_blank.into_bytes()[0],
            CRTCRegister::StartHorizontalRetrace => self.crtc_start_horizontal_retrace,
            CRTCRegister::EndHorizontalRetrace => self.crtc_end_horizontal_retrace.into_bytes()[0],
            CRTCRegister::VerticalTotal => (self.crtc_vertical_total & 0xFF) as u8,
            CRTCRegister::Overflow => self.crtc_overflow,
            CRTCRegister::PresetRowScan => self.crtc_preset_row_scan.into_bytes()[0],
            CRTCRegister::MaximumScanLine => self.crtc_maximum_scanline.into_bytes()[0],
            CRTCRegister::CursorStartLine => self.crtc_cursor_start.into_bytes()[0],
            CRTCRegister::CursorEndLine => self.crtc_cursor_end.into_bytes()[0],
            CRTCRegister::StartAddressH => self.crtc_start_address_ho,
            CRTCRegister::StartAddressL => self.crtc_start_address_lo,
            CRTCRegister::CursorAddressH => self.crtc_cursor_address_ho,
            CRTCRegister::CursorAddressL => self.crtc_cursor_address_lo,
            CRTCRegister::VerticalRetraceStart => (self.crtc_vertical_retrace_start & 0xFF) as u8, 
            CRTCRegister::VerticalRetraceEnd => self.crtc_vertical_retrace_end.into_bytes()[0],
            CRTCRegister::VerticalDisplayEnd => (self.crtc_vertical_display_end & 0xFF) as u8,
            CRTCRegister::Offset => self.crtc_offset,
            CRTCRegister::UnderlineLocation => self.crtc_underline_location.into_bytes()[0],
            CRTCRegister::StartVerticalBlank => (self.crtc_start_vertical_blank & 0xFF) as u8,
            CRTCRegister::EndVerticalBlank => self.crtc_end_vertical_blank,
            CRTCRegister::ModeControl => self.crtc_mode_control.into_bytes()[0],
            CRTCRegister::LineCompare => (self.crtc_line_compare & 0xFF) as u8,                   
        }
    }

}
/*
    ega_crtc_regs.rs

    Implement the CRTC Registers of the IBM EGA Card

*/

use modular_bitfield::prelude::*;
use crate::ega::EGACard;

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

#[bitfield]
#[derive (Copy, Clone)]
pub struct CEndHorizontalBlank {
    pub end_horizontal_blank: B5,
    pub display_enable_skew: B2,
    #[skip]
    unused: B1
}

#[bitfield]
#[derive (Copy, Clone)]
pub struct CEndHorizontalRetrace {
    pub end_horizontal_retrace: B5,
    pub horizontal_retrace_delay: B2,
    #[skip]
    unused: B1
}

#[bitfield]
#[derive (Copy, Clone)]
pub struct CVerticalRetraceEnd {
    pub vertical_retrace_end: B4,
    pub cvi: B1,
    pub dvi: B1,
    #[skip] 
    unused: B2
}

impl EGACard {
    
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
                log::debug!("Select to invalid CRTC register: {:02X}", byte);
                self.crtc_register_select_byte = 0;
                CRTCRegister::HorizontalTotal
            } 
        }
    }

    pub fn write_crtc_register_data(&mut self, byte: u8 ) {

        //log::debug!("CGA: Write to CRTC register: {:?}: {:02}", self.crtc_register_selected, byte );
        match self.crtc_register_selected {
            CRTCRegister::HorizontalTotal => {
                // (R0) 8 bit write only
                self.crtc_horizontal_total = byte;
            },
            CRTCRegister::HorizontalDisplayEnd => {
                // (R1) 8 bit write only
                self.crtc_horizontal_display_end = byte;
            }
            CRTCRegister::StartHorizontalBlank => {
                // (R2) 8 bit write only
                self.crtc_start_horizontal_blank = byte;
            },
            CRTCRegister::EndHorizontalBlank => {
                // (R3) 8 bit write only
                // Bits 0-4: End Horizontal Blank
                // Bits 5-6: Display Enable Skew
                self.crtc_end_horizontal_blank = CEndHorizontalBlank::from_bytes([byte]);
            },
            CRTCRegister::StartHorizontalRetrace => {
                // (R4) 
                self.crtc_start_horizontal_retrace = byte;
            },
            CRTCRegister::EndHorizontalRetrace => {
                // (R5) 
                self.crtc_end_horizontal_retrace = CEndHorizontalRetrace::from_bytes([byte]);
                //self.normalize_end_horizontal_retrace();
            }
            CRTCRegister::VerticalTotal => {
                // (R6) 9-bit - Vertical Total
                // Bit 8 in overflow register. Set only lower 8 bits here.
                self.crtc_vertical_total &= 0xFF00;
                self.crtc_vertical_total |= byte as u16; 
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
                self.crtc_overflow = byte;
                self.set_crtc_overflow_bits(byte);
            },
            CRTCRegister::PresetRowScan => {
                // (R8)
                self.crtc_preset_row_scan = byte;
            },            
            CRTCRegister::MaximumScanLine => {
                // (R9) 
                self.crtc_maximum_scanline = byte
            }            
            CRTCRegister::CursorStartLine => {
                // R(A)
                // Bits 0-4: Cursor Start Line
                // Bit 5: Cursor Enable (This field only valid in VGA)
                self.crtc_cursor_start = byte & CURSOR_LINE_MASK;
                //self.crtc_cursor_enabled = byte >> 5 & 0x01 != 0;
            }
            CRTCRegister::CursorEndLine => {
                // R(B)
                // Bits 0-4: Cursor Start Line     
                // Bits 5-6: Cursor Skew        
                self.crtc_cursor_end = byte & CURSOR_LINE_MASK;
                self.crtc_cursor_skew = byte >> 5 & 0x03;
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
                // (R10) 9 bits - Vertical Retrace Start
                // Bit 8 in overflow register. Set only lower 8 bits here.
                self.crtc_vertical_retrace_start &= 0xFF00;
                self.crtc_vertical_retrace_start |= byte as u16;
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
            },
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
                // R(16) - Bits 0-4: End Vertical Blank
                self.crtc_end_vertical_blank = byte & 0x1F;
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
            _ => {
                log::debug!("Write to unsupported CRTC register {:?}: {:02X}", self.crtc_register_selected, byte);
            }
        }
        self.recalculate_mode();
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
    /// The value stored in the Rertical Retrace End field of the Vertical Retrace End
    /// register is actually the 5 low order bits to compare against the current scanline
    /// counter to determine when the vertical retrace period is over. We convert this 
    /// into the actual scanline number. 
    fn normalize_end_vertical_retrace(&mut self) {

        let evr = self.crtc_vertical_retrace_end.vertical_retrace_end() as u16;

        let mut proposed_evr = self.crtc_vertical_retrace_start & 0xE0 | evr;
        if proposed_evr <= self.crtc_vertical_retrace_start {
            proposed_evr = (self.crtc_vertical_retrace_start + 0x20) & 0xE0 | evr;
        }

        self.crtc_vertical_retrace_end_norm = proposed_evr;
    }     

    pub fn read_crtc_register(&mut self ) -> u8 {
        match self.crtc_register_selected {
            CRTCRegister::CursorStartLine => self.crtc_cursor_start,
            CRTCRegister::CursorEndLine => self.crtc_cursor_end,
            CRTCRegister::CursorAddressH => {
                //log::debug!("CGA: Read from CRTC register: {:?}: {:02}", self.crtc_register_selected, self.crtc_cursor_address_ho );
                self.crtc_cursor_address_ho 
            },
            CRTCRegister::CursorAddressL => {
                //log::debug!("CGA: Read from CRTC register: {:?}: {:02}", self.crtc_register_selected, self.crtc_cursor_address_lo );
                self.crtc_cursor_address_lo
            }
            _ => {
                log::debug!("Read from unsupported CRTC register: {:?}", self.crtc_register_selected);
                0
            }
        }
    }

    /// Handle a write to the CRTC Mode Control Register (R17)
    fn write_crtc_mode_control_register(&mut self, byte: u8 ) {
        self.crtc_mode_control = byte;
    }

}
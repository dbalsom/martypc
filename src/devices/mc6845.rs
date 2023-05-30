/*
    MartyPC Emulator 
    (C)2023 Daniel Balsom
    https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

    --------------------------------------------------------------------------

    mc6845.rs

    Implementation of the Motorola MC6845 CRT controller.
    Used internally by the MDA and CGA video cards.

*/

use crate::tracelogger::TraceLogger;

const CURSOR_LINE_MASK: u8      = 0b0000_1111;
const CURSOR_ATTR_MASK: u8      = 0b0011_0000;

const REGISTER_MAX: usize = 17;
const REGISTER_UNREADABLE_VALUE: u8 = 0x00;

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

  
}
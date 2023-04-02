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

    cga.rs

    Implementation of the IBM CGA card, built around the Motorola MC6845 
    display controller.



*/


#![allow(dead_code)]
use std::collections::HashMap;

use crate::config::VideoType;
use crate::bus::{BusInterface, IoDevice, MemoryMappedDevice};
use crate::videocard::*;

static DUMMY_PLANE: [u8; 1] = [0];
static DUMMY_PIXEL: [u8; 4] = [0, 0, 0, 0];

pub const CGA_MEM_ADDRESS: usize = 0xB8000;
// CGA memory is repeated twice due to incomplete address decoding.
pub const CGA_MEM_APERTURE: usize = 0x8000;
pub const CGA_MEM_SIZE: usize = 0x4000; // 16384 bytes
pub const CGA_MEM_MASK: usize = !0x4000; // Applying this mask will implement memory mirror.

// Sensible defaults for CRTC registers. A real CRTC is probably
// uninitialized
const DEFAULT_CURSOR_START_LINE: u8 = 6;
const DEFAULT_CURSOR_END_LINE: u8 = 7;
const DEFAULT_HORIZONTAL_TOTAL: u8 = 113;
const DEFAULT_HORIZONTAL_DISPLAYED: u8 = 80;
const DEFAULT_HORIZONTAL_SYNC_POS: u8 = 90;
const DEFAULT_HORIZONTAL_SYNC_WIDTH: u8 = 10;
const DEFAULT_VERTICAL_TOTAL: u8 = 31;
const DEFAULT_VERTICAL_TOTAL_ADJUST: u8 = 6;
const DEFAULT_VERTICAL_DISPLAYED: u8 = 25;
const DEFAULT_VERTICAL_SYNC_POS: u8 = 28;

// CGA is clocked at 14.318180Mhz, which is the main clock of the entire PC system.
// The original CGA card did not have its own crystal.
const CGA_CLOCK: f64 = 14.318180;
const US_PER_CLOCK: f64 = 1.0 / CGA_CLOCK;

/*  
    We can calculate the maximum theoretical size of a CGA display by working from the 
    14.31818Mhz CGA clock. We are limited to 262 scanlines per NTSC (525/2)
    This gives us 262 maximum scanlines. 
    The CGA gets programmed with a Horizontal Character total of 113(+1)=114 characters
    in standard 80 column text mode. This is total - not displayed characters. 
    So a single scan line is 114 * 8 or 912 clocks wide. 
    912 clocks * 262 scanlines = 238,944 clocks per frame.
    14,318,180Hz / 238,944 clocks equals a 59.92Hz refresh rate.
    So our final numbers are 912x262 @ 59.92Hz. This is a much higher resolution than
    the expected maximum of 640x200, but it includes overscan and retrace periods.
    With a default horizontal sync width of 10(*8), and a fixed (on the Motorola at least)
    vsync 'width' of 16, this brings us down to a visible area of 832x246.
    This produces vertical ovescan borders of 26 pixels and horizontal borders of 96 pixels
    The Area5150 demo manages to squeeze out a 768 pixel horizontal resolution mode from
    the CGA. This is accomplished with a HorizontalDisplayed value of 96. (96 * 8 = 768)
    I am assuming this is the highest value we will actually ever encounter and anything
    wider might not sync to a real monitor.
*/

// Calculate the maximum possible area of buf field (including refresh period)
const CGA_XRES_MAX: u32 = (CRTC_R0_HORIZONTAL_MAX + 1) * CRTC_CHAR_CLOCK as u32;
const CGA_YRES_MAX: u32 = CRTC_SCANLINE_MAX;
const CGA_MAX_CLOCK: usize = (CGA_XRES_MAX * CGA_YRES_MAX) as usize; // Should be 238944

// For derivision of CGA timings, see https://www.vogons.org/viewtopic.php?t=47052
// We run the CGA card independent of the CPU frequency.
// Timings in 4.77Mhz CPU cycles are provided for reference.
const FRAME_TIME_US: f64 = 16_688.15452339;
const FRAME_VBLANK_US: f64 = 14_732.45903422;
//const FRAME_CPU_TIME: u32 = 79_648;
//const FRAME_VBLANK_START: u32 = 70_314;

const SCANLINE_TIME_US: f64 = 63.69524627;
const SCANLINE_HBLANK_US: f64 = 52.38095911;
//const SCANLINE_CPU_TIME: u32 = 304;
//const SCANLINE_HBLANK_START: u32 = 250;

const CGA_HBLANK: f64 = 0.1785714;

const CGA_DEFAULT_CURSOR_BLINK_RATE: f64 = 0.0625;
const CGA_CURSOR_BLINK_RATE_US: f64 = FRAME_TIME_US * CGA_DEFAULT_CURSOR_BLINK_RATE;

const CGA_DEFAULT_CURSOR_FRAME_CYCLE: u32 = 8;

pub const CRTC_REGISTER_SELECT: u16         = 0x3D4;
pub const CRTC_REGISTER: u16                = 0x3D5;

pub const CGA_MODE_CONTROL_REGISTER: u16    = 0x3D8;
pub const CGA_COLOR_CONTROL_REGISTER: u16   = 0x3D9;
pub const CGA_STATUS_REGISTER: u16          = 0x3DA;
pub const CGA_LIGHTPEN_REGISTER: u16        = 0x3DB;

const MODE_MATCH_MASK: u8       = 0b0001_1111;
const MODE_HIRES_TEXT: u8       = 0b0000_0001;
const MODE_GRAPHICS: u8         = 0b0000_0010;
const MODE_BW: u8               = 0b0000_0100;
const MODE_ENABLE: u8           = 0b0000_1000;
const MODE_HIRES_GRAPHICS: u8   = 0b0001_0000;
const MODE_BLINKING: u8         = 0b0010_0000;

const CURSOR_LINE_MASK: u8      = 0b0000_1111;
const CURSOR_ATTR_MASK: u8      = 0b0011_0000;

// Color control register bits.
// Alt color = Overscan in Text mode, BG color in 320x200 graphics, FG color in 640x200 graphics
const CC_ALT_COLOR_MASK: u8     = 0b0000_0111;
const CC_ALT_INTENSITY: u8      = 0b0000_1000;
const CC_BRIGHT_BIT: u8         = 0b0001_0000; // Controls whether palette is high intensity
const CC_PALETTE_BIT: u8        = 0b0010_0000; // Controls primary palette between magenta/cyan and red/green

const STATUS_DISPLAY_ENABLE: u8         = 0b0000_0001;
const STATUS_LIGHTPEN_TRIGGER_SET: u8   = 0b0000_0010;
const STATUS_LIGHTPEN_SWITCH_STATUS: u8 = 0b0000_0100;
const STATUS_VERTICAL_RETRACE: u8       = 0b0000_1000;

/// Include the standard 8x8 CGA font.
/// TODO: Support alternate font with thinner glyphs? It was normally not accessable except 
/// by soldering a jumper
static CGA_FONT: &'static [u8] = include_bytes!("../assets/cga_8by8.bin");
const CGA_FONT_SPAN: usize = 256; // Font bitmap is 2048 bits wide (256 * 8 characters)

const CRTC_CHAR_CLOCK: u8 = 8;
const CRTC_FONT_HEIGHT: u8 = 8;
const CRTC_VBLANK_HEIGHT: u8 = 16;

const CRTC_R0_HORIZONTAL_MAX: u32 = 113;
const CRTC_SCANLINE_MAX: u32 = 262;



pub enum Resolution {
    Res640by200,
    Res320by200
}

pub enum BitDepth {
    Depth1,
    Depth2,
    Depth4,
}
pub struct CGACard {

    mode_byte: u8,
    display_mode: DisplayMode,
    mode_enable: bool,
    mode_graphics: bool,
    mode_bw: bool,
    mode_hires_gfx: bool,
    mode_hires_txt: bool,
    mode_blinking: bool,
    scanline_us: f64,
    scanline_cycles: u32,
    frame_us: f64,
    frame_cycles: u32,
    cursor_frames: u32,
    in_hblank: bool,
    in_vblank: bool,
    frame_count: u64,

    cursor_status: bool,
    cursor_slowblink: bool,
    cursor_blink_rate: f64,

    crtc_register_select_byte: u8,
    crtc_register_selected: CRTCRegister,

    crtc_horizontal_total: u8,
    crtc_horizontal_displayed: u8,
    crtc_horizontal_sync_pos: u8,
    crtc_sync_width: u8,
    crtc_vertical_total: u8,
    crtc_vertical_total_adjust: u8,
    crtc_vertical_displayed: u8,
    crtc_vertical_sync_pos: u8,
    crtc_interlace_mode: u8,
    crtc_maximum_scanline_address: u8,
    crtc_cursor_start_line: u8,
    crtc_cursor_end_line: u8,
    crtc_start_address: usize,
    crtc_start_address_ho: u8,
    crtc_start_address_lo: u8,
    crtc_cursor_address_lo: u8,
    crtc_cursor_address_ho: u8,
    crtc_cursor_address: usize,

    cc_register: u8,

    // CRTC counters
    beam_x: u32,
    beam_y: u32,
    rows_drawn: u32,

    overscan_left: u32,
    overscan_right: u32,
    in_display_area: bool,
    cur_char: u8,                   // Current character being drawn
    cur_attr: u8,                   // Current attribute byte being drawn
    cur_fg: u8,                     // Current glyph fg color
    cur_bg: u8,                     // Current glyph bg color
    cur_blink: bool,                // Current glyph blink attribute
    char_col: u8,                   // Column of character glyph being drawn
    char_row: u8,                   // Row of character glyph being drawn
    hcc_c0: u8,                     // Horizontal character counter (x pos of character)
    vlc_c9: u8,                     // Vertical line counter - counts during vsync period
    vcc_c4: u8,                     // Vertical character counter (y pos of character)
    vsc_c3h: u8,
    hsc_c3l: u8,
    vtac_c5: u8,
    vma: usize,
    rba: usize,    
    blink_state: bool,              // Used to control blinking of cursor and text with blink attribute
    blink_accum_us: f64,            // Microsecond accumulator for blink state flipflop
    accumulated_us: f64,

    mem: Vec<u8>,

    back_buf: usize,
    front_buf: usize,
    extents: [DisplayExtents; 2],
    buf: Vec<Vec<u8>>,

    debug_color: u8
}

#[derive(Debug)]
pub enum CRTCRegister {
    HorizontalTotal,
    HorizontalDisplayed,
    HorizontalSyncPosition,
    SyncWidth,
    VerticalTotal,
    VerticalTotalAdjust,
    VerticalDisplayed,
    VerticalSync,
    InterlaceMode,
    MaximumScanLineAddress,
    CursorStartLine,
    CursorEndLine,
    StartAddressH,
    StartAddressL,
    CursorAddressH,
    CursorAddressL,
    LightPenPositionH,
    LightPenPositionL
}

impl IoDevice for CGACard {
    fn read_u8(&mut self, port: u16) -> u8 {
        match port {
            CGA_MODE_CONTROL_REGISTER => {
                log::error!("CGA: Read from Mode control register!");
                0
            }            
            CGA_STATUS_REGISTER => {
                self.handle_status_register_read()
            }
            CRTC_REGISTER => {
                self.handle_crtc_register_read()
            }
            _ => {
                0
            }
        }
    }

    fn write_u8(&mut self, port: u16, data: u8, _bus: Option<&mut BusInterface>) {
        match port {
            CGA_MODE_CONTROL_REGISTER => {
                self.handle_mode_register(data);
            }
            CRTC_REGISTER_SELECT => {
                self.handle_crtc_register_select(data);
            }
            CRTC_REGISTER => {
                self.handle_crtc_register_write(data);
            }
            CGA_COLOR_CONTROL_REGISTER => {
                self.handle_cc_register_write(data);
            }
            _ => {}
        }
    }

    fn port_list(&self) -> Vec<u16> {
        vec![
            CRTC_REGISTER_SELECT,
            CRTC_REGISTER,
            CGA_MODE_CONTROL_REGISTER,
            CGA_COLOR_CONTROL_REGISTER,
            CGA_LIGHTPEN_REGISTER,
            CGA_STATUS_REGISTER,
        ]
    }

}

// CGA implementation of Default for DisplayExtents.
// Each videocard implementation should implement sensible defaults.
// In CGA's case we know the maximum field size and thus row_stride.
impl Default for DisplayExtents {
    fn default() -> Self {
        Self {
            field_w: CGA_XRES_MAX,
            field_h: CGA_YRES_MAX,
            visible_w: 0,
            visible_h: 0,
            overscan_l: 0,
            overscan_r: 0,
            overscan_t: 0,
            overscan_b: 0,
            row_stride: CGA_XRES_MAX as usize
        }
    }
}

impl CGACard {

    pub fn new() -> Self {
        Self {
            mode_byte: 0,
            display_mode: DisplayMode::Mode3TextCo80,
            mode_enable: true,
            mode_graphics: false,
            mode_bw: false,
            mode_hires_gfx: false,
            mode_hires_txt: true,
            mode_blinking: true,
            frame_us: 0.0,
            frame_cycles: 0,
            cursor_frames: 0,
            scanline_us: 0.0,
            scanline_cycles: 0,
            in_hblank: false,
            in_vblank: false,
            frame_count: 0,

            cursor_status: false,
            cursor_slowblink: false,
            cursor_blink_rate: CGA_DEFAULT_CURSOR_BLINK_RATE,

            crtc_register_selected: CRTCRegister::HorizontalTotal,
            crtc_register_select_byte: 0,

            crtc_horizontal_total: DEFAULT_HORIZONTAL_TOTAL,
            crtc_horizontal_displayed: DEFAULT_HORIZONTAL_DISPLAYED,
            crtc_horizontal_sync_pos: DEFAULT_HORIZONTAL_SYNC_POS,
            crtc_sync_width: DEFAULT_HORIZONTAL_SYNC_WIDTH,
            crtc_vertical_total: DEFAULT_VERTICAL_TOTAL,
            crtc_vertical_total_adjust: DEFAULT_VERTICAL_TOTAL_ADJUST,
            crtc_vertical_displayed: DEFAULT_VERTICAL_DISPLAYED,
            crtc_vertical_sync_pos: DEFAULT_VERTICAL_SYNC_POS,
            crtc_interlace_mode: 0,
            crtc_maximum_scanline_address: 7,
            crtc_cursor_start_line: DEFAULT_CURSOR_START_LINE,
            crtc_cursor_end_line: DEFAULT_CURSOR_END_LINE,
            crtc_start_address: 0,
            crtc_start_address_ho: 0,
            crtc_start_address_lo: 0,
            crtc_cursor_address_lo: 0,
            crtc_cursor_address_ho: 0,
            crtc_cursor_address: 0,

            cc_register: CC_PALETTE_BIT | CC_BRIGHT_BIT,

            beam_x: 0,
            beam_y: 0,
            rows_drawn: 0,

            overscan_left: 0,
            overscan_right: 0,
            in_display_area: false,
            cur_char: 0,
            cur_attr: 0,
            cur_fg: 0,
            cur_bg: 0,
            cur_blink: false,
            char_col: 0,
            char_row: 0,
            hcc_c0: 0,
            vlc_c9: 0,
            vcc_c4: 0,
            vsc_c3h: 0,
            hsc_c3l: 0,
            vtac_c5: 0,
            vma: 0,
            rba: 0,
            blink_state: false,
            blink_accum_us: 0.0,

            accumulated_us: 0.0,

            mem: vec![0; CGA_MEM_SIZE],

            back_buf: 1,
            front_buf: 0,
            extents: [Default::default(); 2],
            buf: vec![vec![0; (CGA_XRES_MAX * CGA_YRES_MAX) as usize]; 2],

            debug_color: 0,
        }
    }

    fn get_cursor_span(&self) -> (u8, u8) {
        (self.crtc_cursor_start_line, self.crtc_cursor_end_line)
    }

    fn get_cursor_address(&self) -> usize {
        self.crtc_cursor_address
    }

    fn update_cursor_address(&mut self) {
        self.crtc_cursor_address = (self.crtc_cursor_address_ho as usize) << 8 | self.crtc_cursor_address_lo as usize
    }

    fn get_cursor_status(&self) -> bool {
        self.cursor_status
    }

    fn handle_crtc_register_select(&mut self, byte: u8 ) {

        //log::trace!("CGA: CRTC register {:02X} selected", byte);
        self.crtc_register_select_byte = byte;
        self.crtc_register_selected = match byte {
            0x00 => CRTCRegister::HorizontalTotal,
            0x01 => CRTCRegister::HorizontalDisplayed,
            0x02 => CRTCRegister::HorizontalSyncPosition,
            0x03 => CRTCRegister::SyncWidth,
            0x04 => CRTCRegister::VerticalTotal,
            0x05 => CRTCRegister::VerticalTotalAdjust,
            0x06 => CRTCRegister::VerticalDisplayed,
            0x07 => CRTCRegister::VerticalSync,
            0x08 => CRTCRegister::InterlaceMode,
            0x09 => CRTCRegister::MaximumScanLineAddress,
            0x0A => CRTCRegister::CursorStartLine,
            0x0B => CRTCRegister::CursorEndLine,
            0x0C => CRTCRegister::StartAddressH,
            0x0D => CRTCRegister::StartAddressL,
            0x0E => CRTCRegister::CursorAddressH,
            0x0F => CRTCRegister::CursorAddressL,
            0x10 => CRTCRegister::LightPenPositionH,
            0x11 => CRTCRegister::LightPenPositionL,
            _ => {
                log::debug!("CGA: Select to invalid CRTC register");
                self.crtc_register_select_byte = 0;
                CRTCRegister::HorizontalTotal
            } 
        }
    }

    fn handle_crtc_register_write(&mut self, byte: u8 ) {

        //log::debug!("CGA: Write to CRTC register: {:?}: {:02}", self.crtc_register_selected, byte );
        match self.crtc_register_selected {
            CRTCRegister::HorizontalTotal => {
                // (R0) 8 bit write only
                self.crtc_horizontal_total = byte;
            },
            CRTCRegister::HorizontalDisplayed => {
                // (R1) 8 bit write only
                self.crtc_horizontal_displayed = byte;
            }
            CRTCRegister::HorizontalSyncPosition => {
                // (R2) 8 bit write only
                self.crtc_horizontal_sync_pos = byte;
            },
            CRTCRegister::SyncWidth => {
                // (R3) 8 bit write only
                self.crtc_sync_width = byte;
            },
            CRTCRegister::VerticalTotal => {
                // (R4) 7 bit write only
                self.crtc_vertical_total = byte & 0x7F;
            },
            CRTCRegister::VerticalTotalAdjust => {
                // (R5) 5 bit write only
                self.crtc_vertical_total_adjust = byte & 0x1F;
            }
            CRTCRegister::VerticalDisplayed => {
                // (R6) 7 bit write only
                self.crtc_vertical_displayed = byte; 
            },
            CRTCRegister::VerticalSync => {
                // (R7) 7 bit write only
                self.crtc_vertical_sync_pos = byte & 0x7F;
            },
            CRTCRegister::InterlaceMode => {
                self.crtc_interlace_mode = byte;
            },            
            CRTCRegister::MaximumScanLineAddress => {
                self.crtc_maximum_scanline_address = byte
            }            
            CRTCRegister::CursorStartLine => {
                self.crtc_cursor_start_line = byte & CURSOR_LINE_MASK;
                match byte & CURSOR_ATTR_MASK >> 4 {
                    0b00 | 0b10 => {
                        self.cursor_status = true;
                        self.cursor_slowblink = false;
                    }
                    0b01 => {
                        self.cursor_status = false;
                        self.cursor_slowblink = false;
                    }
                    _ => {
                        self.cursor_status = true;
                        self.cursor_slowblink = true;
                    }
                }
            }
            CRTCRegister::CursorEndLine => {
                self.crtc_cursor_end_line = byte & CURSOR_LINE_MASK;
            }
            CRTCRegister::CursorAddressH => {
                self.crtc_cursor_address_ho = byte;
                self.update_cursor_address();
            }
            CRTCRegister::CursorAddressL => {
                self.crtc_cursor_address_lo = byte;
                self.update_cursor_address();
            }
            CRTCRegister::StartAddressH => {
                self.crtc_start_address_ho = byte;
            }
            CRTCRegister::StartAddressL => {
                self.crtc_start_address_lo = byte;
            }
            _ => {
                log::debug!("CGA: Write to unsupported CRTC register {:?}: {:02X}", self.crtc_register_selected, byte);
            }
        }
    }
    
    fn handle_crtc_register_read(&mut self ) -> u8 {
        match self.crtc_register_selected {
            CRTCRegister::CursorStartLine => self.crtc_cursor_start_line,
            CRTCRegister::CursorEndLine => self.crtc_cursor_end_line,
            CRTCRegister::CursorAddressH => {
                //log::debug!("CGA: Read from CRTC register: {:?}: {:02}", self.crtc_register_selected, self.crtc_cursor_address_ho );
                self.crtc_cursor_address_ho 
            },
            CRTCRegister::CursorAddressL => {
                //log::debug!("CGA: Read from CRTC register: {:?}: {:02}", self.crtc_register_selected, self.crtc_cursor_address_lo );
                self.crtc_cursor_address_lo
            }
            _ => {
                log::debug!("CGA: Read from unsupported CRTC register: {:?}", self.crtc_register_selected);
                0
            }
        }
    }

    fn handle_mode_register(&mut self, mode_byte: u8) {

        self.mode_hires_txt = mode_byte & MODE_HIRES_TEXT != 0;
        self.mode_graphics = mode_byte & MODE_GRAPHICS != 0;
        self.mode_bw = mode_byte & MODE_BW != 0;
        self.mode_enable = mode_byte & MODE_ENABLE != 0;
        self.mode_hires_gfx = mode_byte & MODE_HIRES_GRAPHICS != 0;
        self.mode_blinking = mode_byte & MODE_BLINKING != 0;
        self.mode_byte = mode_byte;
        
        if mode_byte & MODE_ENABLE == 0 {
            self.display_mode = DisplayMode::Disabled;
        }
        else {
            self.display_mode = match mode_byte & 0x1F {
                0b0_1100 => DisplayMode::Mode0TextBw40,
                0b0_1000 => DisplayMode::Mode1TextCo40,
                0b0_1101 => DisplayMode::Mode2TextBw80,
                0b0_1001 => DisplayMode::Mode3TextCo80,
                0b0_1010 => DisplayMode::Mode4LowResGraphics,
                0b0_1110 => DisplayMode::Mode5LowResAltPalette,
                0b1_1110 => DisplayMode::Mode6HiResGraphics,
                0b1_1010 => DisplayMode::Mode7LowResComposite,
                _ => {
                    log::error!("CGA: Invalid buf mode selected: {:02X}", mode_byte & 0x0F);
                    DisplayMode::Mode3TextCo80
                }
            };
        }

        log::debug!("CGA: Mode Selected ({:?}:{:02X}) Enabled: {}", 
            self.display_mode,
            mode_byte, 
            self.mode_enable );
    }

    fn handle_status_register_read(&mut self) -> u8 {
        // Bit 1 of the status register is set when the CGA can be safely written to without snow.
        // It is tied to the 'Display Enable' line from the CGA card, inverted.
        // Thus it will be 1 when the CGA card is not currently scanning, IE during both horizontal
        // and vertical refresh.

        // https://www.vogons.org/viewtopic.php?t=47052
        
        if self.in_hblank {
            STATUS_DISPLAY_ENABLE
        }
        else if self.in_vblank {
            STATUS_VERTICAL_RETRACE | STATUS_DISPLAY_ENABLE
        }
        else {
            0
        }
    }

    fn handle_cc_register_write(&mut self, data: u8) {
        //log::trace!("Write to color control register: {:02X}", data);
        self.cc_register = data;
    }

    /// Swaps the front and back buffers by exchanging indices.
    fn swap(&mut self) {

        //std::mem::swap(&mut self.back_buf, &mut self.front_buf);
        
        if self.back_buf == 0 {
            self.front_buf = 0;
            self.back_buf = 1;
        }
        else {
            self.front_buf = 1;
            self.back_buf = 0;
        }
        
        self.buf[self.back_buf].fill(0x00);
    }    

    /// Return the bit value at (col,row) of the given font glyph
    fn get_glyph_bit(glyph: u8, col: u8, row: u8) -> bool {

        debug_assert!(col < CRTC_CHAR_CLOCK);
        debug_assert!(row < CRTC_CHAR_CLOCK);

        // Calculate byte offset 
        let glyph_offset: usize = (row as usize * CGA_FONT_SPAN) + glyph as usize;
        CGA_FONT[glyph_offset] & (0x01 << (7 - col)) != 0
    }

    /// Set the character attributes for the current character.
    fn set_char_addr(&mut self, addr: usize) {

        self.cur_char = self.mem[addr];
        self.cur_attr = self.mem[addr + 1];

        self.cur_fg = self.cur_attr & 0x0F;
        // If blinking is enabled, the bg attribute is only 3 bytes and only low-intensity colors 
        // are available. 
        // If blinking is disabled, all 16 colors are available as background attributes.

        if self.mode_blinking {
            self.cur_bg = (self.cur_attr >> 4) & 0x07;
        }
        else {
            self.cur_bg = self.cur_attr >> 4;
        }
        
        //(self.cur_fg, self.cur_bg) = ATTRIBUTE_TABLE[self.cur_attr as usize];
    }

}

macro_rules! push_reg_str {
    ($vec: expr, $reg: expr, $decorator: expr, $val: expr ) => {
        $vec.push((format!("{:?} {}", $reg, $decorator), VideoCardStateEntry::String(format!("{}", $val))))
    };
}

/*
macro_rules! push_reg_str_bin8 {
    ($vec: expr, $reg: expr, $decorator: expr, $val: expr ) => {
        $vec.push((format!("{:?} {}", $reg, $decorator), VideoCardStateEntry::String(format!("{:08b}", $val))))
    };
}
*/

macro_rules! push_reg_str_enum {
    ($vec: expr, $reg: expr, $decorator: expr, $val: expr ) => {
        $vec.push((format!("{:?} {}", $reg, $decorator), VideoCardStateEntry::String(format!("{:?}", $val))))
    };
}   

impl VideoCard for CGACard {

    fn get_video_type(&self) -> VideoType {
        VideoType::CGA
    }

    fn get_render_mode(&self) -> RenderMode {
        RenderMode::Direct
    }

    fn get_display_mode(&self) -> DisplayMode {
        self.display_mode
    }

    fn get_display_size(&self) -> (u32, u32) {

        // CGA supports a single fixed 8x8 font. The size of the displayed window 
        // is always HorizontalDisplayed * (VerticalDisplayed * (MaximumScanlineAddress + 1))
        // (Excepting fancy CRTC tricks that delay vsync)
        let mut width = self.crtc_horizontal_displayed as u32 * CRTC_CHAR_CLOCK as u32;
        let height = self.crtc_vertical_displayed as u32 * (self.crtc_maximum_scanline_address as u32 + 1);

        if self.mode_hires_gfx {
            width = width * 2;
        }
        (width, height)
    }

    fn get_display_extents(&self) -> &DisplayExtents {
        &self.extents[self.back_buf]
    }

    fn get_scanline_double(&self) -> bool {
        true
    }

    fn get_display_buf(&self) -> &[u8] {
        &self.buf[self.front_buf][..]
    }
    
    fn get_refresh_rate(&self) -> u32 {
        60
    }

    fn is_40_columns(&self) -> bool {

        match self.display_mode {
            DisplayMode::Mode0TextBw40 => true,
            DisplayMode::Mode1TextCo40 => true,
            DisplayMode::Mode2TextBw80 => false,
            DisplayMode::Mode3TextCo80 => false,
            DisplayMode::Mode4LowResGraphics => true,
            DisplayMode::Mode5LowResAltPalette => true,
            DisplayMode::Mode6HiResGraphics => false,
            DisplayMode::Mode7LowResComposite => false,
            _=> false
        }
    }

    fn is_graphics_mode(&self) -> bool {
        self.mode_graphics
    }

    /// Return the 16-bit value computed from the CRTC's pair of Page Address registers.
    fn get_start_address(&self) -> u16 {
        return (self.crtc_start_address_ho as u16) << 8 | self.crtc_start_address_lo as u16;
    }

    fn get_cursor_info(&self) -> CursorInfo {
        let addr = self.get_cursor_address();

        match self.display_mode {
            DisplayMode::Mode0TextBw40 | DisplayMode::Mode1TextCo40 => {
                CursorInfo{
                    addr,
                    pos_x: (addr % 40) as u32,
                    pos_y: (addr / 40) as u32,
                    line_start: self.crtc_cursor_start_line,
                    line_end: self.crtc_cursor_end_line,
                    visible: self.get_cursor_status()
                }
            }
            DisplayMode::Mode2TextBw80 | DisplayMode::Mode3TextCo80 => {
                CursorInfo{
                    addr,
                    pos_x: (addr % 80) as u32,
                    pos_y: (addr / 80) as u32,
                    line_start: self.crtc_cursor_start_line,
                    line_end: self.crtc_cursor_end_line,
                    visible: self.get_cursor_status()
                }
            }
            _=> {
                // Not a valid text mode
                CursorInfo{
                    addr: 0,
                    pos_x: 0,
                    pos_y: 0,
                    line_start: 0,
                    line_end: 0,
                    visible: false
                }
            }
        }
    }
    
    fn get_clock_divisor(&self) -> u32 {
        1
    }

    fn get_current_font(&self) -> FontInfo {
        FontInfo {
            w: CRTC_CHAR_CLOCK as u32,
            h: CRTC_FONT_HEIGHT as u32,
            font_data: CGA_FONT
        }
    }

    fn get_character_height(&self) -> u8 {
        self.crtc_maximum_scanline_address + 1
    }    

    /// Return the current palette number, intensity attribute bit, and alt color
    fn get_cga_palette(&self) -> (CGAPalette, bool) {

        let intensity = self.cc_register & CC_BRIGHT_BIT != 0;
        
        // Get background color
        let alt_color = match self.cc_register & 0x0F {
            0b0000 => CGAColor::Black,
            0b0001 => CGAColor::Blue,
            0b0010 => CGAColor::Green,
            0b0011 => CGAColor::Cyan,
            0b0100 => CGAColor::Red,
            0b0101 => CGAColor::Magenta,
            0b0110 => CGAColor::Brown,
            0b0111 => CGAColor::White,
            0b1000 => CGAColor::BlackBright,
            0b1001 => CGAColor::BlueBright,
            0b1010 => CGAColor::GreenBright,
            0b1011 => CGAColor::CyanBright,
            0b1100 => CGAColor::RedBright,
            0b1101 => CGAColor::MagentaBright,
            0b1110 => CGAColor::Yellow,
            _ => CGAColor::WhiteBright
        };

        // Are we in high res mode?
        if self.mode_hires_gfx {
            return (CGAPalette::Monochrome(alt_color), true); 
        }

        let mut palette = match self.cc_register & CC_PALETTE_BIT != 0 {
            true => CGAPalette::MagentaCyanWhite(alt_color),
            false => CGAPalette::RedGreenYellow(alt_color)
        };
        
        // Check for 'hidden' palette - Black & White mode bit in lowres graphics selects Red/Cyan palette
        if self.mode_bw && self.mode_graphics && !self.mode_hires_gfx { 
            palette = CGAPalette::RedCyanWhite(alt_color);
        }
    
        (palette, intensity)
    }    

    fn get_videocard_string_state(&self) -> HashMap<String, Vec<(String, VideoCardStateEntry)>> {

        let mut map = HashMap::new();
        
        let mut general_vec = Vec::new();

        push_reg_str_enum!(general_vec, "Adapter Type:", "", self.get_video_type());
        push_reg_str_enum!(general_vec, "Display Mode:", "", self.get_display_mode());
        push_reg_str_enum!(general_vec, "Frame Count:", "", self.frame_count);
        map.insert("General".to_string(), general_vec);

        let mut crtc_vec = Vec::new();

        push_reg_str!(crtc_vec, CRTCRegister::HorizontalTotal, "", self.crtc_horizontal_total);
        push_reg_str!(crtc_vec, CRTCRegister::HorizontalDisplayed, "", self.crtc_horizontal_displayed);
        push_reg_str!(crtc_vec, CRTCRegister::HorizontalSyncPosition, "", self.crtc_horizontal_sync_pos);
        push_reg_str!(crtc_vec, CRTCRegister::SyncWidth, "", self.crtc_sync_width);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalTotal, "", self.crtc_vertical_total);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalTotalAdjust, "", self.crtc_vertical_total_adjust);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalDisplayed, "", self.crtc_vertical_displayed);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalSync, "", self.crtc_vertical_sync_pos);
        push_reg_str!(crtc_vec, CRTCRegister::InterlaceMode, "", self.crtc_interlace_mode);
        push_reg_str!(crtc_vec, CRTCRegister::MaximumScanLineAddress, "", self.crtc_maximum_scanline_address);
        push_reg_str!(crtc_vec, CRTCRegister::CursorStartLine, "", self.crtc_cursor_start_line);
        push_reg_str!(crtc_vec, CRTCRegister::CursorEndLine, "", self.crtc_cursor_end_line);
        push_reg_str!(crtc_vec, CRTCRegister::StartAddressH, "", self.crtc_start_address_ho);
        push_reg_str!(crtc_vec, CRTCRegister::StartAddressL, "", self.crtc_start_address_lo);
        push_reg_str!(crtc_vec, CRTCRegister::CursorAddressH, "", self.crtc_cursor_address_ho);
        push_reg_str!(crtc_vec, CRTCRegister::CursorAddressL, "", self.crtc_cursor_address_lo);
        map.insert("CRTC".to_string(), crtc_vec);

        map       
    }

    fn run(&mut self, us: f64) {

        self.accumulated_us += us;

        // Handle blinking. 
        self.blink_accum_us += us;
        if self.blink_accum_us > CGA_CURSOR_BLINK_RATE_US {
            self.blink_state = !self.blink_state;
        }

        // Tick the CRTC. Since the CGA is much faster clocked than the CPU this will 
        // probably happen several times per CPU instruction.
        while self.accumulated_us > US_PER_CLOCK {
            
            if self.in_display_area {
                // Draw current pixel
                if self.rba < CGA_MAX_CLOCK {
                    self.buf[self.back_buf][self.rba] = 
                        match CGACard::get_glyph_bit(self.cur_char, self.char_col, self.char_row) {
                            true => {
                                if self.cur_blink {
                                    if self.blink_state { self.cur_fg } else { self.cur_bg }
                                } 
                                else {
                                    self.cur_fg
                                }
                            },
                            false => self.cur_bg
                        };

                        // Draw cursor
                        if self.rba == self.get_cursor_address() {

                        }

                    //self.buf[self.back_buf][self.rba] = (self.rows_drawn & 0x0F) as u8;
                }
            }
            else if self.in_hblank {
                // Draw overscan area blue
                if self.rba < CGA_MAX_CLOCK {
                    self.buf[self.back_buf][self.rba] = 1;
                }
            }
            else if self.in_vblank {
                // Draw vblank area magenta
                if self.rba < CGA_MAX_CLOCK {
                    self.buf[self.back_buf][self.rba] = 5;
                }
            }
            else {
                // Draw overscan green
                if self.rba < CGA_MAX_CLOCK {
                    self.buf[self.back_buf][self.rba] = 2;
                }
            }

            // Update position to next pixel
            self.beam_x += 1;
            self.char_col += 1;
            self.rba += 1;

            // Done with the current character      
            if self.char_col == CRTC_CHAR_CLOCK {
                
                // Update horizontal character counter
                self.hcc_c0 = self.hcc_c0.wrapping_add(1);

                self.debug_color = (self.debug_color + 1) & 0x0F;

                // Advance video memory address offset and grab the next character + attr
                self.vma += 2;
                self.set_char_addr((self.crtc_start_address + self.vma) & 0x3FFF);

                // Glyph colun reset to 0 for next char
                self.char_col = 0;

                //if self.hcc_c0 == self.overscan_left as u8 {
                //    // We entered buf area
                //    self.in_display_area = true
                //}
                //if self.hcc_c0 == self.overscan_left as u8 + self.crtc_horizontal_displayed {
                //    // We left buf area
                //    self.in_display_area = false
                //}

                if self.hcc_c0 == self.crtc_horizontal_displayed {
                    // Enter right overscan
                    self.in_display_area = false;
                }
                if self.hcc_c0 == self.crtc_horizontal_sync_pos {
                    // We entered horizontal blank
                    self.in_hblank = true;
                }
                else if self.hcc_c0 == self.crtc_horizontal_sync_pos + (self.crtc_sync_width & 0x0F) { 
                    // We've left horizontal blank, enter left overscan
                    self.in_hblank = false;

                    // Reset beam to left of screen
                    self.beam_x = 0;
                    self.char_col = 0;
                }                 
                else if self.hcc_c0 == self.crtc_horizontal_total + 1 {
                    // Finished scanning row

                    // Reset Horizontal Character Counter and increment character row counter
                    self.hcc_c0 = 0;
                    self.char_row += 1;

                    // Return video memory address to starting position for next character row
                    self.vma = self.vcc_c4 as usize * (self.crtc_horizontal_displayed * 2) as usize;
                    
                    // Reset the current character glyph to start of row
                    self.set_char_addr((self.crtc_start_address + self.vma) & 0x3FFF);

                    if self.in_vblank {
                        // If we are in vblank, advance Vertical Sync Counter
                        self.vsc_c3h += 1;
                        
                        if self.vsc_c3h == CRTC_VBLANK_HEIGHT {
                            // We are leaving vblank period
                            self.in_vblank = false;
                            self.vsc_c3h = 0;
                        }
                    }
                    else {
                        // Start the new row
                        if self.vcc_c4 < self.crtc_vertical_displayed {
                            self.rows_drawn += 1;
                            self.in_display_area = true;
                        }
                    }
                    
                    if self.char_row > self.crtc_maximum_scanline_address  {
                        // We finished drawing this row of characters 

                        self.char_row = 0;
                        // Advance Vertical Character Counter
                        self.vcc_c4 = self.vcc_c4.wrapping_add(1);

                        // Set vma to starting position for next character row
                        self.vma = self.vcc_c4 as usize * (self.crtc_horizontal_displayed * 2) as usize;
                        // Load next char + attr
                        self.set_char_addr((self.crtc_start_address + self.vma) & 0x3FFF);

                        if self.vcc_c4 == self.crtc_vertical_sync_pos {
                            // We've reached vertical sync
                            self.in_vblank = true;
                            self.in_display_area = false;
                        }
                    }

                    if self.vcc_c4 == self.crtc_vertical_displayed {
                        // Enter lower overscan area
                        self.in_display_area = false;
                    }
                    
                    if self.vcc_c4 == self.crtc_vertical_total + 1 {

                        // Completed a frame.
                        self.frame_count += 1;

                        // Set beam to top left of screen.
                        self.hcc_c0 = 0;
                        self.vcc_c4 = 0;
                        self.beam_x = 0;
                        self.beam_y = 0;
                        self.char_row = 0;
                        self.char_col = 0;
                        self.vma = 0;
                        self.rba = 0;
                        self.in_display_area = true;

                        // Swap the display buffers
                        self.swap();

                        // Write out preliminary DisplayExtents data for new front buffer based on current crtc values
                        self.extents[self.front_buf].visible_w = self.crtc_horizontal_displayed as u32 * CRTC_CHAR_CLOCK as u32;
                        self.extents[self.front_buf].visible_h = self.rows_drawn;
                        //log::debug!("new extents: {}, {}", self.extents[self.front_buf].visible_w, self.extents[self.front_buf].visible_h);

                        self.rows_drawn = 0;

                        // Load first char + attr
                        self.set_char_addr(self.crtc_start_address & 0x3FFF);
                    }
                }
            }

            self.accumulated_us -= US_PER_CLOCK;
        }

        /*
         old impl
        self.frame_cycles += cpu_cycles;
        self.scanline_cycles += cpu_cycles;
        if self.frame_cycles > FRAME_CPU_TIME {
            self.frame_cycles -= FRAME_CPU_TIME;
            self.cursor_frames += 1;
            // Blink the cursor
            let cursor_cycle = CGA_DEFAULT_CURSOR_FRAME_CYCLE * (self.cursor_slowblink as u32 + 1);
            if self.cursor_frames > cursor_cycle {
                self.cursor_frames -= cursor_cycle;
                self.cursor_status = !self.cursor_status;
            }
        }
        if self.scanline_cycles > SCANLINE_CPU_TIME {
            self.scanline_cycles -= SCANLINE_CPU_TIME;
        }
        // Are we in HBLANK interval?
        self.in_hblank = self.scanline_cycles > SCANLINE_HBLANK_START;
        // Are we in VBLANK interval?
        self.in_vblank = self.frame_cycles > FRAME_VBLANK_START;
        */
    }

    fn reset(&mut self) {
        log::debug!("Resetting")
    }

    fn get_pixel(&self, _x: u32, _y:u32) -> &[u8] {
        &DUMMY_PIXEL
    }

    fn get_pixel_raw(&self, _x: u32, _y:u32) -> u8 {
        0
    }

    fn get_plane_slice(&self, _plane: usize) -> &[u8] {
        &DUMMY_PLANE
    }

    fn get_frame_count(&self) -> u64 {
        self.frame_count
    }

    fn dump_mem(&self) {
        let filename = format!("./dumps/cga.bin");
        
        match std::fs::write(filename.clone(), &self.mem) {
            Ok(_) => {
                log::debug!("Wrote memory dump: {}", filename)
            }
            Err(e) => {
                log::error!("Failed to write memory dump '{}': {}", filename, e)
            }
        }
    }

}

/// Unlike the EGA or VGA the CGA doesn't do any operations on video memory on read/write,
/// but we handle the mirroring of VRAM this way, and for consistency with other devices
impl MemoryMappedDevice for CGACard {

    fn read_u8(&mut self, address: usize) -> u8 {

        let a_offset = (address & CGA_MEM_MASK) - CGA_MEM_ADDRESS;
        if a_offset < CGA_MEM_SIZE {
            self.mem[a_offset]
        }
        else {
            // Read out of range, shouldn't happen...
            0xFF
        }
    }

    fn write_u8(&mut self, address: usize, byte: u8) {
        let a_offset = (address & CGA_MEM_MASK) - CGA_MEM_ADDRESS;
        if a_offset < CGA_MEM_SIZE {
            self.mem[a_offset] = byte
        }
    }

    fn read_u16(&mut self, address: usize) -> u16 {

        let lo_byte = MemoryMappedDevice::read_u8(self, address);
        let ho_byte = MemoryMappedDevice::read_u8(self, address + 1);

        log::warn!("Unsupported 16 bit read from VRAM");
        return (ho_byte as u16) << 8 | lo_byte as u16
    }    

    fn write_u16(&mut self, _address: usize, _data: u16) {
        //trace!(self, "16 byte write to VRAM, {:04X} -> {:05X} ", data, address);
        log::warn!("Unsupported 16 bit write to VRAM");
    }
}
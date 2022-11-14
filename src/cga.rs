#![allow(dead_code)]

use std::collections::HashMap;

use log;
use crate::io::{IoBusInterface, IoDevice};
use crate::videocard::{
    VideoCard,
    VideoType,
    DisplayMode,
    CursorInfo,
    FontInfo,
    CGAColor,
    CGAPalette
};

static DUMMY_PLANE: [u8; 1] = [0];
static DUMMY_PIXEL: [u8; 4] = [0, 0, 0, 0];

pub const CGA_MEM_ADDRESS: usize = 0xB8000;
pub const CGA_MEM_SIZE: usize = 16384;

const CGA_DEFAULT_CURSOR_BLINK_RATE: f64 = 0.0625;
const CGA_DEFAULT_CURSOR_FRAME_CYCLE: u32 = 8;

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

const FRAME_CPU_TIME: u32 = 79648;
const FRAME_VBLANK_START: u32 = 70314;
const SCANLINE_CPU_TIME: u32 = 304;
const SCANLINE_HBLANK_START: u32 = 250;

const CGA_HBLANK: f64 = 0.1785714;

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
// Controls whether palette is high intensity
const CC_BRIGHT_BIT: u8         = 0b0001_0000;
// Controls primary palette between magenta/cyan and red/green
const CC_PALETTE_BIT: u8        = 0b0010_0000;

const STATUS_DISPLAY_ENABLE: u8 = 0b0000_0001;
const STATUS_LIGHTPEN_TRIGGER_SET: u8 = 0b0000_0010;
const STATUS_LIGHTPEN_SWITCH_STATUS: u8 = 0b0000_0100;
const STATUS_VERTICAL_RETRACE: u8 = 0b0000_1000;

static CGA_FONT: &'static [u8; 2048] = include_bytes!("../assets/cga_8by8.bin");
const CGA_FONT_W: u32 = 8;
const CGA_FONT_H: u32 = 8;



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
    scanline_cycles: u32,
    frame_cycles: u32,
    cursor_frames: u32,
    in_hblank: bool,
    in_vblank: bool,
    
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
    crtc_start_address: u16,
    crtc_start_address_ho: u8,
    crtc_start_address_lo: u8,
    crtc_cursor_address_lo: u8,
    crtc_cursor_address_ho: u8,

    cc_register: u8
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
    fn write_u8(&mut self, port: u16, data: u8) {
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
            frame_cycles: 0,
            cursor_frames: 0,
            scanline_cycles: 0,
            in_hblank: false,
            in_vblank: false,

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

            cc_register: CC_PALETTE_BIT | CC_BRIGHT_BIT
        }
    }

    fn get_cursor_span(&self) -> (u8, u8) {
        (self.crtc_cursor_start_line, self.crtc_cursor_end_line)
    }

    fn get_cursor_address(&self) -> u32 {
        (self.crtc_cursor_address_ho as u32) << 8 | self.crtc_cursor_address_lo as u32
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
                self.crtc_cursor_address_ho = byte
            }
            CRTCRegister::CursorAddressL => {
                self.crtc_cursor_address_lo = byte
            }
            CRTCRegister::StartAddressH => {
                self.crtc_start_address_ho = byte
            }
            CRTCRegister::StartAddressL => {
                self.crtc_start_address_lo = byte
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
                    log::error!("CGA: Invalid display mode selected: {:02X}", mode_byte & 0x0F);
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
        log::trace!("Write to color control register: {:02X}", data);
        self.cc_register = data;
    }


}

impl VideoCard for CGACard {

    fn get_video_type(&self) -> VideoType {
        VideoType::CGA
    }

    fn get_display_mode(&self) -> DisplayMode {
        self.display_mode
    }

    fn get_display_extents(&self) -> (u32, u32) {

        // CGA supports a single fixed 8x8 font. The size of the displayed window 
        // is always HorizontalDisplayed * (VerticalDisplayed * (MaximumScanlineAddress + 1))
        // (Excepting fancy CRTC tricks that delay vsync)
        let mut width = self.crtc_horizontal_displayed as u32 * CGA_FONT_W as u32;
        let height = self.crtc_vertical_displayed as u32 * (self.crtc_maximum_scanline_address as u32 + 1);

        if self.mode_hires_gfx {
            width = width * 2;
        }
        (width, height)
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
                    pos_x: addr % 40,
                    pos_y: addr / 40,
                    line_start: self.crtc_cursor_start_line,
                    line_end: self.crtc_cursor_end_line,
                    visible: self.get_cursor_status()
                }
            }
            DisplayMode::Mode2TextBw80 | DisplayMode::Mode3TextCo80 => {
                CursorInfo{
                    addr,
                    pos_x: addr % 80,
                    pos_y: addr / 80,
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
            w: CGA_FONT_W,
            h: CGA_FONT_H,
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

    fn get_videocard_string_state(&self) -> HashMap<String, Vec<(String,String)>> {

        let mut map = HashMap::new();

        let mut general_vec = Vec::new();
        general_vec.push((format!("Adapter Type:"), format!("{:?}", self.get_video_type())));
        general_vec.push((format!("Display Mode:"), format!("{:?}", self.get_display_mode())));
        map.insert("General".to_string(), general_vec);

        let mut crtc_vec = Vec::new();
        crtc_vec.push((format!("{:?}", CRTCRegister::HorizontalTotal), format!("{}", self.crtc_horizontal_total)));
        crtc_vec.push((format!("{:?}", CRTCRegister::HorizontalDisplayed), format!("{}", self.crtc_horizontal_displayed)));
        crtc_vec.push((format!("{:?}", CRTCRegister::HorizontalSyncPosition), format!("{}", self.crtc_horizontal_sync_pos)));
        crtc_vec.push((format!("{:?}", CRTCRegister::SyncWidth), format!("{}", self.crtc_sync_width)));
        crtc_vec.push((format!("{:?}", CRTCRegister::VerticalTotal), format!("{}", self.crtc_vertical_total)));
        crtc_vec.push((format!("{:?}", CRTCRegister::VerticalTotalAdjust), format!("{}", self.crtc_vertical_total_adjust)));
        crtc_vec.push((format!("{:?}", CRTCRegister::VerticalDisplayed), format!("{}", self.crtc_vertical_displayed)));
        crtc_vec.push((format!("{:?}", CRTCRegister::VerticalSync), format!("{}", self.crtc_vertical_sync_pos)));
        crtc_vec.push((format!("{:?}", CRTCRegister::InterlaceMode), format!("{}", self.crtc_interlace_mode)));
        crtc_vec.push((format!("{:?}", CRTCRegister::MaximumScanLineAddress), format!("{}", self.crtc_maximum_scanline_address)));
        crtc_vec.push((format!("{:?}", CRTCRegister::CursorStartLine), format!("{}", self.crtc_cursor_start_line)));
        crtc_vec.push((format!("{:?}", CRTCRegister::CursorEndLine), format!("{}", self.crtc_cursor_end_line)));
        crtc_vec.push((format!("{:?}", CRTCRegister::StartAddressH), format!("{}", self.crtc_start_address_ho)));
        crtc_vec.push((format!("{:?}", CRTCRegister::StartAddressL), format!("{}", self.crtc_start_address_lo)));
        crtc_vec.push((format!("{:?}", CRTCRegister::CursorAddressH), format!("{}", self.crtc_cursor_address_ho)));
        crtc_vec.push((format!("{:?}", CRTCRegister::CursorAddressL), format!("{}", self.crtc_cursor_address_lo)));
        map.insert("CRTC".to_string(), crtc_vec);
        map       
    }


    fn run(&mut self, cpu_cycles: u32) {

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
    }    

    fn reset(&mut self) {
        log::debug!("Resetting")
    }

    fn get_pixel(&self, x: u32, y:u32) -> &[u8] {
        &DUMMY_PIXEL
    }

    fn get_pixel_raw(&self, x: u32, y:u32) -> u8 {
        0
    }

    fn get_plane_slice(&self, plane: usize) -> &[u8] {
        &DUMMY_PLANE
    }

    fn dump_mem(&self) {
        
        log::warn!("memory dump for CGA unimplemented")
    }

}

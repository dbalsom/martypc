#![allow(dead_code)]
use log;
use crate::io::{IoBusInterface, IoDevice};

pub const CGA_MEM_ADDRESS: usize = 0xB8000;
pub const CGA_MEM_SIZE: usize = 16384;

pub const CGA_DEFAULT_CURSOR_START_LINE: u8 = 6;
pub const CGA_DEFAULT_CURSOR_END_LINE: u8 = 7;
pub const CGA_DEFAULT_CURSOR_BLINK_RATE: f64 = 0.0625;

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

// Color control register bits.
// Alt color = Overscan in Text mode, BG color in 320x200 graphics, FG color in 640x200 graphics
const CC_ALT_COLOR_MASK: u8     = 0b0000_0111;
// Controls whether palette is high intensity
const CC_BRIGHT_BIT: u8         = 0b0000_1000;
// Controls primary palette between magenta/cyan and red/green
const CC_PALETTE_BIT: u8        = 0b0001_0000;

const STATUS_DISPLAY_ENABLE: u8 = 0b0000_0001;
const STATUS_LIGHTPEN_TRIGGER_SET: u8 = 0b0000_0010;
const STATUS_LIGHTPEN_SWITCH_STATUS: u8 = 0b0000_0100;
const STATUS_VERTICAL_RETRACE: u8 = 0b0000_1000;

pub enum CGAPalette {
    MagentaCyanWhite,
    RedGreenYellow,
    RedCyanWhite // "Hidden" CGA palette
}
pub enum Resolution {
    Res640by200,
    Res320by200
}

pub enum BitDepth {
    Depth1,
    Depth2,
    Depth4,
}

#[derive(Copy, Clone, Debug)]
pub enum DisplayMode {
    Disabled,
    Mode0TextBw40,
    Mode1TextCo40,
    Mode2TextBw80,
    Mode3TextCo80,
    Mode4LowResGraphics,
    Mode5LowResAltPalette,
    Mode6HiResGraphics,
    Mode7LowResComposite,
    Mode8LowResTweaked
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
    in_hblank: bool,
    in_vblank: bool,
    
    crtc_cursor_status: bool,
    crtc_cursor_blink_rate: f64,
    crtc_register_select_byte: u8,
    crtc_register_selected: CRTCRegister,
    crtc_cursor_start_line: u8,
    crtc_cursor_end_line: u8,
    crtc_cursor_address_lo: u8,
    crtc_cursor_address_ho: u8,

    cc_register: u8
}

#[derive(Debug)]
pub enum CRTCRegister {
    TotalHorizontalCharacter,
    DisplayHorizontalCharacter,
    HorizontalSyncSignal,
    HorizontalSyncDuration,
    TotalVerticalCharacter,
    AdjustVerticalCharacter,
    DisplayVerticalCharacter,
    VerticalSyncSignal,
    InterlaceMode,
    NumberOfScanLinesPerScreenLine,
    CursorStartLine,
    CursorEndLine,
    PageAddressLOByte,
    PageAddressHOByte,
    CursorAddressHOByte,
    CursorAddressLOByte,
    LightPenPositionHOByte,
    LightPenPositionLOByte
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

    fn read_u16(&mut self, _port: u16) -> u16 {
        log::error!("Invalid 16-bit read from CGA");
        0   
    }
    fn write_u16(&mut self, _port: u16, _data: u16) {
        log::error!("Invalid 16-bit write to CGA");
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
            scanline_cycles: 0,
            in_hblank: false,
            in_vblank: false,

            crtc_cursor_status: false,
            crtc_cursor_blink_rate: CGA_DEFAULT_CURSOR_BLINK_RATE,
            crtc_register_selected: CRTCRegister::TotalHorizontalCharacter,
            crtc_register_select_byte: 0,

            crtc_cursor_start_line: CGA_DEFAULT_CURSOR_START_LINE,
            crtc_cursor_end_line: CGA_DEFAULT_CURSOR_END_LINE,
            crtc_cursor_address_lo: 0,
            crtc_cursor_address_ho: 0,

            cc_register: CC_PALETTE_BIT | CC_BRIGHT_BIT
        }
    }

    pub fn get_cursor_span(&self) -> (u8, u8) {
        (self.crtc_cursor_start_line, self.crtc_cursor_end_line)
    }

    pub fn get_cursor_address(&self) -> u32 {
        (self.crtc_cursor_address_ho as u32) << 8 | self.crtc_cursor_address_lo as u32
    }

    pub fn get_cursor_status(&self) -> bool {
        self.crtc_cursor_status
    }

    pub fn get_display_mode(&self) -> DisplayMode {
        self.display_mode
    }

    /// Return the current palette number, intensity attribute bit, and alt color
    pub fn get_palette(&self) -> (CGAPalette, bool, u8) {

        let alt_color = self.cc_register & 0x03;
        let intensity = self.cc_register & CC_BRIGHT_BIT != 0;
        
        let mut palette = match self.cc_register & CC_PALETTE_BIT != 0 {
            true => CGAPalette::MagentaCyanWhite,
            false => CGAPalette::RedGreenYellow
        };
        
        // Check for 'hidden' palette - Black & White mode bit in lowres graphics selects Red/Cyan palette
        if self.mode_bw && self.mode_graphics && !self.mode_hires_gfx { 
            palette = CGAPalette::RedCyanWhite;
        }
    
        (palette, intensity, alt_color)
    }

    pub fn is_graphics_mode(&self) -> bool {
        self.mode_graphics
    }

    pub fn is_40_columns(&self) -> bool {

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

    pub fn handle_crtc_register_select(&mut self, byte: u8 ) {

        //log::trace!("CGA: CRTC register {:02X} selected", byte);
        self.crtc_register_select_byte = byte;
        self.crtc_register_selected = match byte {
            0x00 => CRTCRegister::TotalHorizontalCharacter,
            0x01 => CRTCRegister::DisplayHorizontalCharacter,
            0x02 => CRTCRegister::HorizontalSyncSignal,
            0x03 => CRTCRegister::HorizontalSyncDuration,
            0x04 => CRTCRegister::TotalVerticalCharacter,
            0x05 => CRTCRegister::AdjustVerticalCharacter,
            0x06 => CRTCRegister::DisplayVerticalCharacter,
            0x07 => CRTCRegister::VerticalSyncSignal,
            0x08 => CRTCRegister::InterlaceMode,
            0x09 => CRTCRegister::NumberOfScanLinesPerScreenLine,
            0x0A => CRTCRegister::CursorStartLine,
            0x0B => CRTCRegister::CursorEndLine,
            0x0C => CRTCRegister::PageAddressLOByte,
            0x0D => CRTCRegister::PageAddressHOByte,
            0x0E => CRTCRegister::CursorAddressHOByte,
            0x0F => CRTCRegister::CursorAddressLOByte,
            0x10 => CRTCRegister::LightPenPositionHOByte,
            0x11 => CRTCRegister::LightPenPositionLOByte,
            _ => {
                log::debug!("CGA: Select to invalid CRTC register");
                self.crtc_register_select_byte = 0;
                CRTCRegister::TotalHorizontalCharacter
            } 
        }
    }

    pub fn handle_crtc_register_write(&mut self, byte: u8 ) {

        match self.crtc_register_selected {
            CRTCRegister::CursorStartLine => self.crtc_cursor_start_line = byte,
            CRTCRegister::CursorEndLine => self.crtc_cursor_end_line = byte,
            CRTCRegister::CursorAddressHOByte => {
                //log::debug!("CGA: Write to CRTC register: {:?}: {:02}", self.crtc_register_selected, byte );
                self.crtc_cursor_address_ho = byte
            }
            CRTCRegister::CursorAddressLOByte => {
                //log::debug!("CGA: Write to CRTC register: {:?}: {:02}", self.crtc_register_selected, byte );
                self.crtc_cursor_address_lo = byte
            }
            _ => {
                log::debug!("CGA: Write to unsupported CRTC register: {:?}", self.crtc_register_selected);
            }
        }
    }
    
    pub fn handle_crtc_register_read(&mut self ) -> u8 {
        match self.crtc_register_selected {
            CRTCRegister::CursorStartLine => self.crtc_cursor_start_line,
            CRTCRegister::CursorEndLine => self.crtc_cursor_end_line,
            CRTCRegister::CursorAddressHOByte => {
                //log::debug!("CGA: Read from CRTC register: {:?}: {:02}", self.crtc_register_selected, self.crtc_cursor_address_ho );
                self.crtc_cursor_address_ho 
            },
            CRTCRegister::CursorAddressLOByte => {
                //log::debug!("CGA: Read from CRTC register: {:?}: {:02}", self.crtc_register_selected, self.crtc_cursor_address_lo );
                self.crtc_cursor_address_lo
            }
            _ => {
                log::debug!("CGA: Read from unsupported CRTC register: {:?}", self.crtc_register_selected);
                0
            }
        }
    }

    pub fn handle_mode_register(&mut self, mode_byte: u8) {

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

    pub fn handle_status_register_read(&mut self) -> u8 {
        // Bit 1 of the status register is set when the CGA can be safely written to without snow.
        // It is tied to the 'Display Enable' line from the CGA card, inverted.
        // Thus it will be 1 when the CGA card is not currently scanning, IE during both horizontal
        // and vertical refresh.
        // For now, we just always return 1.
        // https://www.vogons.org/viewtopic.php?t=47052
        
        if self.in_hblank {
            STATUS_DISPLAY_ENABLE
        }
        else if self.in_vblank {
            STATUS_VERTICAL_RETRACE
        }
        else {
            0
        }
    }

    pub fn handle_cc_register_write(&mut self, data: u8) {

    }

    pub fn run(&mut self, io_bus: &mut IoBusInterface, cpu_cycles: u32) {

        self.frame_cycles += cpu_cycles;
        self.scanline_cycles += cpu_cycles;

        if self.frame_cycles > FRAME_CPU_TIME {
            self.frame_cycles -= FRAME_CPU_TIME;
        }
        if self.scanline_cycles > SCANLINE_CPU_TIME {
            self.scanline_cycles -= SCANLINE_CPU_TIME;
        }

        // Are we in HBLANK interval?
        self.in_hblank = self.scanline_cycles > SCANLINE_HBLANK_START;
        // Are we in VBLANK interval?
        self.in_vblank = self.frame_cycles > FRAME_VBLANK_START;

        // Blink cursor 
        let blink_period = (FRAME_CPU_TIME as f64 * self.crtc_cursor_blink_rate) as u32;
        // Blink cycle on/off per blink_period
        self.crtc_cursor_status = blink_period & 0x01 != 0;
    }
}
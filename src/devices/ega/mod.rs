/*
    ega.rs

    Implement the IBM Enhanced Graphics Adapter

    Resources:
    "IBM Enhanced Graphics Adapter", IBM (C) 
        NOTE: This reference incorrectly describes the Display Enable bit. It 
              is reversed. 
    "Programmer's Guide to the EGA, VGA and Super VGA Cards", Richard F Ferraro 
    "EGA/VGA, A Programmer's Reference Guide 2nd Edition", Bradley Dyck Kliewer
    "Hardware Level VGA and SVGA Video Programming Information Page", 
        http://www.osdever.net/FreeVGA/home.htm

*/
use std::collections::HashMap;
use modular_bitfield::prelude::*;

//#![allow(dead_code)]
use log;
use crate::io::IoDevice;
use crate::bus::MemoryMappedDevice;

use crate::videocard::{
    VideoCard,
    VideoType,
    DisplayMode,
    CursorInfo,
    FontInfo,
    CGAColor,
    CGAPalette
};

mod ega_attribute_regs;
mod ega_crtc_regs;
mod ega_graphics_regs;
mod ega_sequencer_regs;

use ega_attribute_regs::*;
use ega_crtc_regs::*;
use ega_graphics_regs::*;
use ega_sequencer_regs::*;

static DUMMY_PIXEL: [u8; 4] = [0, 0, 0, 0];

pub const CGA_ADDRESS: usize = 0xB8000;
pub const EGA_GFX_ADDRESS: usize = 0xA0000;

// pub const CGA_MEM_SIZE: usize = 16384;
pub const EGA_TEXT_PLANE_SIZE: usize = 16384;
pub const EGA_GFX_PLANE_SIZE: usize = 65536;

// For an EGA card connected to an EGA monitor
// See http://www.minuszerodegrees.net/ibm_ega/ibm_ega_switch_settings.htm
// This is inverted (Checkit will report 0110)
// This is the only value that gives high-resolution text 640x350
pub const EGA_DIP_SWITCH: u8 = 0b0000_1001;

const CGA_DEFAULT_CURSOR_BLINK_RATE: f64 = 0.0625;
const CGA_DEFAULT_CURSOR_FRAME_CYCLE: u32 = 8;

const DEFAULT_CURSOR_START_LINE: u8 = 6;
const DEFAULT_CURSOR_END_LINE: u8 = 7;
const DEFAULT_HORIZONTAL_TOTAL: u8 = 113;
const DEFAULT_HORIZONTAL_DISPLAYED: u8 = 80;
const DEFAULT_HORIZONTAL_SYNC_POS: u8 = 90;
const DEFAULT_HORIZONTAL_SYNC_WIDTH: u8 = 10;

const DEFAULT_VERTICAL_TOTAL: u16 = 31;

const DEFAULT_VERTICAL_TOTAL_ADJUST: u8 = 6;
const DEFAULT_VERTICAL_DISPLAYED: u8 = 25;
const DEFAULT_VERTICAL_SYNC_POS: u8 = 28;

const DEFAULT_OVERFLOW: u8 = 0;
const DEFAULT_PRESET_ROW_SCAN: u8 = 0;
const DEFAULT_MAX_SCANLINE: u8 = 13;

const CGA_FRAME_CPU_TIME: u32 = 79648;
const CGA_VBLANK_START: u32 = 70314;
const CGA_SCANLINE_CPU_TIME: u32 = 304;
const CGA_HBLANK_START: u32 = 250;

const EGA_FRAME_CPU_TIME: u32 = 70150;
const EGA_VBLANK_START: u32 = 61928;
const EGA_SCANLINE_CPU_TIME: u32 = 267;
const EGA_HBLANK_START: u32 = 220;



const CGA_HBLANK: f64 = 0.1785714;

// Negative offset to use for CRTC, Feature Control and and ISR1 when in Monochrome
// compatibility mode (as controlled by bit 0 in the Miscellaneous Output Register)
const MDA_COMPAT_IO_ADJUST: u16 = 0x20;

/* The attribute address register is multiplexed with the attribute data register
   at the same address. An internal flip-flop controls whether the port reads in
   a register address or data value. 
   The flip-flop should be initialized to a known value before any operation. 
   The flip-flop can be cleared by reading from Input Status Register 1 (0x3DA)
*/
pub const ATTRIBUTE_REGISTER: u16           = 0x3C0;
/* Incomplete address decoding for the Attribute Register means it can also be
   accessed at 0x3C1. The EGA BIOS requires emulating this behavior.
   See: https://www.vogons.org/viewtopic.php?f=9&t=82050&start=60 
*/
pub const ATTRIBUTE_REGISTER_ALT: u16       = 0x3C1;
//ub const ATTRIBUTE_ADDRESS_REGISTER: u16   = 0x3C0; 
//pub const ATTRIBUTE_DATA_REGISTER: u16      = 0x3C0;


pub const MISC_OUTPUT_REGISTER: u16         = 0x3C2;    // Write-only to 3C2
pub const INPUT_STATUS_REGISTER_0: u16      = 0x3C2;    // Read-only from 3C2
pub const INPUT_STATUS_REGISTER_1: u16      = 0x3DA;
pub const INPUT_STATUS_REGISTER_1_MDA: u16  = 0x3BA;    // Used in MDA compatibility mode

pub const SEQUENCER_ADDRESS_REGISTER: u16   = 0x3C4;    
pub const SEQUENCER_DATA_REGISTER: u16      = 0x3C5;

pub const CRTC_REGISTER_ADDRESS: u16        = 0x3D4;
pub const CRTC_REGISTER: u16                = 0x3D5;
pub const CRTC_REGISTER_ADDRESS_MDA: u16    = 0x3B4;    // Used in MDA compatibility mode
pub const CRTC_REGISTER_MDA: u16            = 0x3B5;    // Used in MDA compatibility mode

//pub const CGA_MODE_CONTROL_REGISTER: u16  = 0x3D8;     // This register does not exist on the EGA
//pub const CGA_COLOR_CONTROL_REGISTER: u16 = 0x3D9;     // This register does not exist on the EGA

//pub const CGA_LIGHTPEN_REGISTER: u16      = 0x3DB;

pub const EGA_GRAPHICS_1_POSITION: u16      = 0x3CC;
pub const EGA_GRAPHICS_2_POSITION: u16      = 0x3CA;
pub const EGA_GRAPHICS_ADDRESS: u16         = 0x3CE;    
pub const EGA_GRAPHICS_DATA: u16            = 0x3CF;                                                    
             


/* cga things
const MODE_MATCH_MASK: u8       = 0b0001_1111;
const MODE_HIRES_TEXT: u8       = 0b0000_0001;
const MODE_GRAPHICS: u8         = 0b0000_0010;
const MODE_BW: u8               = 0b0000_0100;
const MODE_ENABLE: u8           = 0b0000_1000;
const MODE_HIRES_GRAPHICS: u8   = 0b0001_0000;
const MODE_BLINKING: u8         = 0b0010_0000;

const CURSOR_ATTR_MASK: u8      = 0b0011_0000;

const STATUS_DISPLAY_ENABLE: u8 = 0b0000_0001;
const STATUS_LIGHTPEN_TRIGGER_SET: u8 = 0b0000_0010;
const STATUS_LIGHTPEN_SWITCH_STATUS: u8 = 0b0000_0100;
const STATUS_VERTICAL_RETRACE: u8 = 0b0000_1000;
*/

// Color control register bits.
// Alt color = Overscan in Text mode, BG color in 320x200 graphics, FG color in 640x200 graphics
const CC_ALT_COLOR_MASK: u8     = 0b0000_0111;
const CC_ALT_INTENSITY: u8      = 0b0000_1000;
// Controls whether palette is high intensity
const CC_BRIGHT_BIT: u8         = 0b0001_0000;
// Controls primary palette between magenta/cyan and red/green
const CC_PALETTE_BIT: u8        = 0b0010_0000;

pub struct VideoTimings {
    cpu_frame: u32,
    vblank_start: u32,
    cpu_scanline: u32,
    hblank_start: u32
}


pub struct EGAFont {
    w: u32,
    h: u32,
    span: usize,
    data: &'static [u8]
}

static EGA_FONTS: [EGAFont; 2] = [
    EGAFont {
        w: 8,
        h: 8,
        span: 256,
        data: include_bytes!("..\\..\\..\\assets\\ega_8by8.bin"),
        
    },
    EGAFont {
        w: 8,
        h: 14,
        span: 256,
        data: include_bytes!("..\\..\\..\\assets\\ega_8by14.bin"),
    }
];

#[derive (Clone)]
pub struct DisplayPlane {
    latch: u8,
    buf: Box<[u8]>
}

impl DisplayPlane {
    fn new() -> Self {
        Self {
            latch: 0,
            buf: Box::new([0; EGA_GFX_PLANE_SIZE])
        }
    }
}

pub struct EGACard {

    timings: [VideoTimings; 2],

    io_adjust: u16,
    mode_byte: u8,
    display_mode: DisplayMode,
    mode_enable: bool,
    mode_graphics: bool,
    mode_bw: bool,
    mode_line_gfx: bool,
    mode_hires_gfx: bool,
    mode_hires_txt: bool,
    mode_blinking: bool,
    scanline: u32,
    scanline_cycles: f32,
    frame_cycles: f32,
    cursor_frames: u32,
    in_hblank: bool,
    in_vblank: bool,
    
    cursor_status: bool,
    cursor_slowblink: bool,
    cursor_blink_rate: f64,

    cc_register: u8,

    crtc_register_select_byte: u8,
    crtc_register_selected: CRTCRegister,

    crtc_horizontal_total: u8,              // R(0) Horizontal Total
    crtc_horizontal_display_end: u8,        // R(1) Horizontal Display End
    crtc_start_horizontal_blank: u8,        // R(2) Start Horizontal Blank
    crtc_end_horizontal_blank: CEndHorizontalBlank, // R(3) Bits 0-4 - End Horizontal Blank
    crtc_end_horizontal_blank_norm: u8,     // End Horizontal Blank value normalized to column number
    crtc_display_enable_skew: u8,           // Calculated from R(3) Bits 5-6 
    crtc_start_horizontal_retrace: u8,      // R(4) Start Horizontal Retrace
    crtc_end_horizontal_retrace: CEndHorizontalRetrace,  // R(5) End Horizontal Retrace
    crtc_end_horizontal_retrace_norm: u8,   // End Horizontal Retrace value normalized to column number
    crtc_vertical_total: u16,               // R(6) Vertical Total (9-bit value)
    crtc_overflow: u8,                      // R(7) Overflow
    crtc_preset_row_scan: u8,               // R(8) Preset Row Scan
    crtc_maximum_scanline: u8,              // R(9) Max Scanline
    crtc_cursor_start: u8,                  // R(A) Cursor Location (9-bit value)
    crtc_cursor_enabled: bool,              // Calculated from R(A) bit 5
    crtc_cursor_end: u8,                    // R(B)
    crtc_cursor_skew: u8,                   // Calculated from R(B) bits 5-6
    crtc_start_address_ho: u8,              // R(C)
    crtc_start_address_lo: u8,              // R(D)
    crtc_start_address: u16,                // Calculated from C&D
    crtc_cursor_address_lo: u8,             // R(E)
    crtc_cursor_address_ho: u8,             // R(F)
    crtc_vertical_retrace_start: u16,       // R(10) Vertical Retrace Start (9-bit value)
    crtc_vertical_retrace_end: CVerticalRetraceEnd, // R(11) Vertical Retrace End (5-bit value)
    crtc_vertical_retrace_end_norm: u16,    // Vertial Retrace Start value normalized to scanline number
    crtc_vertical_display_end: u16,         // R(12) Vertical Display Enable End (9-bit value)
    crtc_offset: u8,                        // R(13)
    crtc_underline_location: u8,            // R(14)
    crtc_start_vertical_blank: u16,         // R(15) Start Vertical Blank (9-bit value)
    crtc_end_vertical_blank: u8,            // R(16)
    crtc_mode_control: u8,                  // R(17)
    crtc_line_compare: u16,                 // R(18) Line Compare (9-bit value)

    sequencer_address_byte: u8,
    sequencer_register_selected: SequencerRegister,
    sequencer_reset: u8,                            // S(0) Reset (WO)
    sequencer_clocking_mode: SClockingModeRegister, // S(1) Clocking Mode (WO)
    sequencer_map_mask: u8,                         // S(2) Map Mask (wO)
    sequencer_character_map_select: u8,             // S(3) Character Map Select (WO)
    sequencer_memory_mode: u8,                      // S(4) Memory Mode (wO)

    graphics_register_select_byte: u8,
    graphics_register_selected: GraphicsRegister,
    graphics_set_reset: u8,
    graphics_enable_set_reset: u8,
    graphics_color_compare: u8,
    graphics_data_rotate: GDataRotateRegister,
    graphics_data_rotate_function: RotateFunction,    
    graphics_read_map_select: u8,
    graphics_mode: GModeRegister,
    graphics_micellaneous: GMiscellaneousRegister,
    graphics_color_dont_care: u8,
    graphics_bitmask: u8,

    attribute_register_flipflop: AttributeRegisterFlipFlop,
    attribute_register_select_byte: u8,
    attribute_register_selected: AttributeRegister,
    attribute_palette_registers: [u8; 16],
    attribute_palette_index: usize,
    attribute_mode_control: AModeControl,
    attribute_overscan_color: AOverscanColor,
    attribute_color_plane_enable: AColorPlaneEnable,
    attribute_pel_panning: u8,

    current_font: usize,

    misc_output_register: EMiscellaneousOutputRegister,

    // Display Planes
    planes: [DisplayPlane; 4],
    pixel_buf: [u8; 8],
    pipeline_buf: [u8; 4],
    write_buf: [u8; 4]
}


#[bitfield]
#[derive (Copy, Clone) ]
struct EMiscellaneousOutputRegister {
    #[bits = 1]
    io_address_select: IoAddressSelect,
    enable_ram: bool,
    #[bits = 2]
    clock_select: ClockSelect,
    disable_internal_drivers: bool,
    #[bits = 1]
    oddeven_page_select: PageSelect,
    #[bits = 1]
    horizontal_retrace_polarity: RetracePolarity,
    #[bits = 1]
    vertical_retrace_polarity: RetracePolarity
}

/// IO Address Select field of External Miscellaneous Register:
/// Bit 0
#[derive (Debug, BitfieldSpecifier)]
pub enum IoAddressSelect {
    CompatMonochrome,
    CompatCGA
}

/// Clock Select field of External Miscellaneous Register:
/// Bits 2-3
#[derive (Debug, BitfieldSpecifier)]
pub enum ClockSelect {
    Clock14,
    Clock16,
    ExternalClock,
    Unused
}

/// Odd/Even Page Select field of External Miscellaneous Register:
#[derive (Debug, BitfieldSpecifier)]
pub enum PageSelect {
    LowPage,
    HighPage
}

#[derive (Debug, BitfieldSpecifier)]
pub enum RetracePolarity {
    Positive,
    Negative
}

impl IoDevice for EGACard {
    fn read_u8(&mut self, port: u16) -> u8 {
        match port {
            INPUT_STATUS_REGISTER_0 => {
                self.read_input_status_register_0()
            }
            INPUT_STATUS_REGISTER_1 => {
                // Don't answer this port if we are in MDA compatibility mode
                match self.misc_output_register.io_address_select() {
                    IoAddressSelect::CompatMonochrome => 0xFF, 
                    IoAddressSelect::CompatCGA => self.read_input_status_register_1()
                }
            }            
            INPUT_STATUS_REGISTER_1_MDA => {
                // Don't respond on this port if we are in CGA compatibility mode
                match self.misc_output_register.io_address_select() {
                    IoAddressSelect::CompatMonochrome => self.read_input_status_register_1(), 
                    IoAddressSelect::CompatCGA => 0xFF
                }                
            }       
            //MODE_CONTROL_REGISTER => {
            //    log::error!("Read from write-only mode control register");
            //    0
            //}            
            CRTC_REGISTER => {
                // Don't answer this port if we are in MDA compatibility mode
                match self.misc_output_register.io_address_select() {
                    IoAddressSelect::CompatMonochrome => 0xFF, 
                    IoAddressSelect::CompatCGA => self.read_input_status_register_1()
                }
            }
            CRTC_REGISTER_MDA => {
                // Don't respond on this port if we are in CGA compatibility mode
                match self.misc_output_register.io_address_select() {
                    IoAddressSelect::CompatMonochrome => self.read_crtc_register(), 
                    IoAddressSelect::CompatCGA => 0xFF
                }                 
            }
            _ => {
                0xFF // Open bus
            }
        }
    }
    fn write_u8(&mut self, port: u16, data: u8) {
        match port {
            MISC_OUTPUT_REGISTER => {
                self.write_external_misc_output_register(data);
            }
            //MODE_CONTROL_REGISTER => {
            //    self.handle_mode_register(data);
            //}
            CRTC_REGISTER_ADDRESS => {
                self.write_crtc_register_address(data);
            }
            CRTC_REGISTER => {
                self.write_crtc_register_data(data);
            }
            EGA_GRAPHICS_1_POSITION => {
                self.write_graphics_position(1, data)
            }
            EGA_GRAPHICS_2_POSITION => {
                self.write_graphics_position(2, data)
            }            
            EGA_GRAPHICS_ADDRESS => {
                self.write_graphics_address(data)
            }
            EGA_GRAPHICS_DATA => {
                self.write_graphics_data(data);
            }
            SEQUENCER_ADDRESS_REGISTER => {
                self.write_sequencer_address(data)
            }
            SEQUENCER_DATA_REGISTER => {
                self.write_sequencer_data(data)
            }
            ATTRIBUTE_REGISTER | ATTRIBUTE_REGISTER_ALT => {
                self.write_attribute_register(data)
            }
            //COLOR_CONTROL_REGISTER => {
            //    self.handle_cc_register_write(data);
            //}
            _ => {}
        }
    }

}

impl EGACard {

    pub fn new() -> Self {
        Self {

            timings: [
                VideoTimings {
                    cpu_frame: CGA_FRAME_CPU_TIME,
                    vblank_start: CGA_VBLANK_START,
                    cpu_scanline: CGA_SCANLINE_CPU_TIME,
                    hblank_start: CGA_HBLANK_START
                },
                VideoTimings {
                    cpu_frame: CGA_FRAME_CPU_TIME,
                    vblank_start: EGA_VBLANK_START,
                    cpu_scanline: EGA_SCANLINE_CPU_TIME,
                    hblank_start: EGA_HBLANK_START,
                }
            ],

            io_adjust: 0,
            mode_byte: 0,
            display_mode: DisplayMode::Mode3TextCo80,
            mode_enable: true,
            mode_graphics: false,
            mode_bw: false,
            mode_line_gfx: false,
            mode_hires_gfx: false,
            mode_hires_txt: true,
            mode_blinking: true,
            frame_cycles: 0.0,
            cursor_frames: 0,
            scanline: 0,
            scanline_cycles: 0.0,
            in_hblank: false,
            in_vblank: false,

            cursor_status: false,
            cursor_slowblink: false,
            cursor_blink_rate: CGA_DEFAULT_CURSOR_BLINK_RATE,

            cc_register: CC_PALETTE_BIT | CC_BRIGHT_BIT,

            crtc_register_selected: CRTCRegister::HorizontalTotal,
            crtc_register_select_byte: 0,

            crtc_horizontal_total: DEFAULT_HORIZONTAL_TOTAL,
            crtc_horizontal_display_end: DEFAULT_HORIZONTAL_DISPLAYED,
            crtc_start_horizontal_blank: DEFAULT_HORIZONTAL_SYNC_POS,
            crtc_end_horizontal_blank: CEndHorizontalBlank::new(),
            crtc_end_horizontal_blank_norm: 0,
            crtc_display_enable_skew: 0,
            crtc_start_horizontal_retrace: 0,
            crtc_end_horizontal_retrace: CEndHorizontalRetrace::new(),
            crtc_end_horizontal_retrace_norm: 0,
            crtc_vertical_total: DEFAULT_VERTICAL_TOTAL,
            crtc_overflow: DEFAULT_OVERFLOW,
            crtc_preset_row_scan: DEFAULT_PRESET_ROW_SCAN,
            crtc_maximum_scanline: DEFAULT_MAX_SCANLINE,
            crtc_cursor_start: DEFAULT_CURSOR_START_LINE,
            crtc_cursor_enabled: false,
            crtc_cursor_end: DEFAULT_CURSOR_END_LINE,
            crtc_cursor_skew: 0,
            crtc_start_address: 0,
            crtc_start_address_ho: 0,
            crtc_start_address_lo: 0,
            crtc_cursor_address_lo: 0,
            crtc_cursor_address_ho: 0,
            crtc_vertical_retrace_start: 0,
            crtc_vertical_retrace_end: CVerticalRetraceEnd::new(),
            crtc_vertical_retrace_end_norm: 0,
            crtc_vertical_display_end: 0,
            crtc_offset: 0,
            crtc_underline_location: 0,
            crtc_start_vertical_blank: 0,
            crtc_end_vertical_blank: 0,
            crtc_mode_control: 0,
            crtc_line_compare: 0,
        
            sequencer_address_byte: 0,
            sequencer_register_selected: SequencerRegister::Reset,
            sequencer_reset: 0,
            sequencer_clocking_mode: SClockingModeRegister::new(),
            sequencer_map_mask: 0,
            sequencer_character_map_select: 0,
            sequencer_memory_mode: 0,     
            
            graphics_register_select_byte: 0,
            graphics_register_selected: GraphicsRegister::SetReset,
            graphics_set_reset: 0,
            graphics_enable_set_reset: 0,
            graphics_color_compare: 0,
            graphics_data_rotate: GDataRotateRegister::new(),
            graphics_data_rotate_function: RotateFunction::Unmodified,
            graphics_read_map_select: 0,
            graphics_mode: GModeRegister::new(),
            graphics_micellaneous: GMiscellaneousRegister::new(),
            graphics_color_dont_care: 0,
            graphics_bitmask: 0,

            attribute_register_flipflop: AttributeRegisterFlipFlop::Address,
            attribute_register_select_byte: 0,
            attribute_register_selected: AttributeRegister::Palette0,
            attribute_palette_registers: [0; 16],
            attribute_palette_index: 0,
            attribute_mode_control: AModeControl::new(),
            attribute_overscan_color: AOverscanColor::new(),
            attribute_color_plane_enable: AColorPlaneEnable::new(),
            attribute_pel_panning: 0,

            current_font: 0,
            misc_output_register: EMiscellaneousOutputRegister::new(),

            planes: [
                DisplayPlane::new(),
                DisplayPlane::new(),
                DisplayPlane::new(),
                DisplayPlane::new()
            ],

            pixel_buf: [0; 8],
            pipeline_buf: [0; 4],
            write_buf: [0; 4],
        }
    }

    fn reset_private(&mut self) {
        self.mode_byte = 0;
        self.display_mode= DisplayMode::Mode3TextCo80;
        self.mode_enable = true;
        self.mode_graphics = false;
        self.mode_bw = false;
        self.mode_line_gfx = false;
        self.mode_hires_gfx = false;
        self.mode_hires_txt = true;
        self.mode_blinking = true;
        self.frame_cycles = 0.0;
        self.cursor_frames = 0;
        self.scanline = 0;
        self.scanline_cycles = 0.0;
        self.in_hblank = false;
        self.in_vblank = false;

        self.cursor_status = false;
        self.cursor_slowblink = false;
        self.cursor_blink_rate = CGA_DEFAULT_CURSOR_BLINK_RATE;

        //self.cc_register: CC_PALETTE_BIT | CC_BRIGHT_BIT,

        self.crtc_register_selected = CRTCRegister::HorizontalTotal;
        self.crtc_register_select_byte = 0;

        self.crtc_horizontal_total = DEFAULT_HORIZONTAL_TOTAL;
        self.crtc_horizontal_display_end = DEFAULT_HORIZONTAL_DISPLAYED;
        self.crtc_start_horizontal_blank = DEFAULT_HORIZONTAL_SYNC_POS;
        self.crtc_end_horizontal_blank = CEndHorizontalBlank::new()
            .with_end_horizontal_blank(DEFAULT_HORIZONTAL_SYNC_WIDTH);
        self.crtc_display_enable_skew = 0;
        self.crtc_start_horizontal_retrace = 0;
        self.crtc_end_horizontal_retrace = CEndHorizontalRetrace::new();
        self.crtc_vertical_total = DEFAULT_VERTICAL_TOTAL;
        self.crtc_overflow = DEFAULT_OVERFLOW;
        self.crtc_preset_row_scan = DEFAULT_PRESET_ROW_SCAN;
        self.crtc_maximum_scanline = DEFAULT_MAX_SCANLINE;
        self.crtc_cursor_start = DEFAULT_CURSOR_START_LINE;
        self.crtc_cursor_enabled = false;
        self.crtc_cursor_end = DEFAULT_CURSOR_END_LINE;
        
    }

    fn get_cursor_span(&self) -> (u8, u8) {
        (self.crtc_cursor_start, self.crtc_cursor_end)
    }

    fn get_cursor_address(&self) -> u32 {
        (self.crtc_cursor_address_ho as u32) << 8 | self.crtc_cursor_address_lo as u32
    }

    fn get_cursor_status(&self) -> bool {
        self.cursor_status
    }

    /// Handle a write to the External Miscellaneous Output Register, 0x3C2
    fn write_external_misc_output_register(&mut self, byte: u8) {

        self.misc_output_register = EMiscellaneousOutputRegister::from_bytes([byte]);

        log::trace!("Write to Misc Output Register: {:02X} Address Select: {:?} Clock Select: {:?}, Odd/Even Page bit: {:?}", 
            byte,
            self.misc_output_register.io_address_select(),
            self.misc_output_register.clock_select(),
            self.misc_output_register.oddeven_page_select()
        )
    }

    /// Handle a read from the Input Status Register Zero, 0x3C2
    /// 
    /// The Switch Sense bit 4 has the state of the DIP switches on the card 
    /// depending on the Clock Select set in the Misc Output Register.
    fn read_input_status_register_0(&mut self) -> u8 {

        let mut byte = 0;

        // These shifts match the EGA BIOS sense switch behavior
        // Switches should be Open, Closed, Closed, Open for EGA Card & Monitor
        let switch_status = match self.misc_output_register.clock_select() {
            ClockSelect::Unused => {
                EGA_DIP_SWITCH >> 3 & 0x01
            }
            ClockSelect::ExternalClock => {
                EGA_DIP_SWITCH >> 2 & 0x01
            }
            ClockSelect::Clock16 => {
                EGA_DIP_SWITCH >> 1 & 0x01
            }
            ClockSelect::Clock14 => {
                EGA_DIP_SWITCH & 0x01
            }
        };

        // Set switch sense bit
        byte |= switch_status << 4;

        // Set CRT interrupt bit. Bit is 0 when retrace is occurring.
        byte |= match self.in_vblank {
            true => 0,
            false => 0x80
        };

        log::trace!("Read from Input Status Register 0: {:08b}", byte);
        byte
    }

    /// Handle a read from the Input Status Register One, 0x3DA
    /// 
    /// Reading from this register also resets the Attribute Controller flip-flip
    fn read_input_status_register_1(&mut self) -> u8 {

        // Reset Address Register flip-flop
        // false == Address
        self.attribute_register_flipflop = AttributeRegisterFlipFlop::Address;

        let mut byte = 0;

        // Display Enable NOT bit is set to 1 if display is in vsync or hsync period
        // Note: IBM's documentation on this bit is wrong. 
        if self.in_hblank || self.in_vblank {
            byte |= 0x01;
        }
        if self.in_vblank {
            byte |= 0x08;
        }

        // The EGA can feed two lines off the Attribute Controller's color outputs back 
        // into the Input Status Register 1 bits 4 & 5. Which lines to feed back are 
        // controlled by bits 4 & 5 of the Color Plane Enable Register Video Status 
        // Mux Field.
        // The EGA BIOS performs a diagnostic that senses these line transitions after
        // drawing a line of high-intensity white characters to the screen. 
        // Currently, we just fake this whole affair by setting the bits to be on during 
        // the first FONT_HEIGHT scanlines.

        if self.scanline < EGA_FONTS[self.current_font].h {
            byte |= 0x30;
        }
        
        byte
    }

    /// Calculate the current display mode based on the various register parameters of the EGA
    /// 
    /// The EGA doesn't have a convenient mode register like the CGA to determine display mode.
    /// Instead several fields are used: 
    /// Sequencer Clocking Mode Register Dot Clock field: Determines 320 low res modes 0,1,4,5
    /// Sequencer Memory Mode Register: Alpha bit: Determines alphanumeric mode
    /// Attribute Controller Mode Control: Graphics/Alpha bit. Also determines alphanumeric mode
    /// Attribute Controller Mode Control: Display Type bit. Determines Color or Monochrome
    /// 
    fn recalculate_mode(&mut self) {

        if self.crtc_maximum_scanline > 7 {
            // Use 8x14 font
            self.current_font = 1;
        }
        else {
            self.current_font = 0;
        }

        match self.attribute_mode_control.mode() {

            AttributeMode::Text => {
                self.display_mode = match (
                    self.crtc_horizontal_display_end, 
                    self.attribute_mode_control.display_type()) {

                    (00..=39, AttributeDisplayType::Monochrome) => DisplayMode::Mode0TextBw40,
                    (00..=39, AttributeDisplayType::Color) => DisplayMode::Mode1TextCo40,
                    (79, AttributeDisplayType::Monochrome) => DisplayMode::Mode2TextBw80,
                    (79, AttributeDisplayType::Color) => DisplayMode::Mode3TextCo80,
                    _ => {
                        log::warn!("Nonstandard text mode set.");
                        DisplayMode::Mode3TextCo80
                    }
                }
            }
            AttributeMode::Graphics => {
                //self.display_mode = match 
                self.display_mode = match (
                    self.crtc_horizontal_display_end, 
                    self.attribute_mode_control.display_type()) {
        
                    (00..=39, AttributeDisplayType::Color) => DisplayMode::ModeDEGALowResGraphics,
                    (79, AttributeDisplayType::Color) => DisplayMode::Mode10EGAHiResGraphics,
                    _ => {
                        log::warn!("Unsupported graphics mode.");
                        DisplayMode::Mode3TextCo80
                    }
                }
            }
        }

        //if self.crt
    }

    fn plane_bounds_check(&self, address: usize) -> Option<usize> {

        match self.graphics_micellaneous.memory_map() {
            MemoryMap::A0000_128k => {
                if address >= EGA_GFX_ADDRESS && address < EGA_GFX_ADDRESS + 128_000 {
                    return Some(address - EGA_GFX_ADDRESS);
                }
                else {
                    return None;
                }
            }
            MemoryMap::A0000_64K => {
                if address >= EGA_GFX_ADDRESS && address < EGA_GFX_ADDRESS + 64_000 {
                    return Some(address - EGA_GFX_ADDRESS);
                }
                else {
                    return None;
                }
            }
            MemoryMap::B8000_32K => {
                if address >= CGA_ADDRESS && address < CGA_ADDRESS + 32_000 {
                    return Some(address - CGA_ADDRESS)
                }
                else {
                    return None;
                }
            }
            _=> return None
        }
    }


    /// Return the 4bpp pixel value from the graphics planes at the specified position
    fn get_pixel(&self, byte: usize, bit: u8) -> u8 {

        let mut bits = 0;

        bits |= self.planes[0].buf[byte] >> (7 - bit) & 0x01;
        bits |= (self.planes[1].buf[byte] >> (7 - bit) & 0x01) << 1;
        bits |= (self.planes[2].buf[byte] >> (7 - bit) & 0x01) << 2;
        bits |= (self.planes[3].buf[byte] >> (7 - bit) & 0x01) << 3;
        bits
    }

    /// Fill a slice of 8 elements with the 4bpp pixel values at the specified memory
    /// address.
    fn get_pixels(&mut self, byte: usize) {
        for p in 0..8 {
            self.pixel_buf[p] |= self.planes[0].buf[byte] >> (7 - p) & 0x01;
            self.pixel_buf[p] |= (self.planes[1].buf[byte] >> (7 - p) & 0x01) << 1;
            self.pixel_buf[p] |= (self.planes[2].buf[byte] >> (7 - p) & 0x01) << 2;
            self.pixel_buf[p] |= (self.planes[3].buf[byte] >> (7 - p) & 0x01) << 3;
        }
    }

    /// Compare the pixels in pixel_buf with the Color Compare and Color Don't Care registers.
    fn pixel_op_compare(&self) -> u8 {

        let mut comparison = 0;
        
        for i in 0..8 {
            let mut plane_comp = 0;

            plane_comp |= match self.planes[0].latch & (0x01 << i) != 0 {
                true => 0x01,
                false => 0x00
            };
            plane_comp |= match self.planes[1].latch & (0x01 << i) != 0 {
                true => 0x02,
                false => 0x00
            };
            plane_comp |= match self.planes[2].latch & (0x01 << i) != 0 {
                true => 0x04,
                false => 0x00
            };
            plane_comp |= match self.planes[3].latch & (0x01 << i) != 0 {
                true => 0x08,
                false => 0x00
            };                       
            
            let masked_cmp = self.graphics_color_compare & self.graphics_color_dont_care;

            if (plane_comp & self.graphics_color_dont_care) == masked_cmp {
                comparison |= 0x01 << i
            }
        }
        comparison
    }

    fn rotate_right_u8(mut byte: u8, mut count: u8) -> u8 {
        let mut carry;
        while count > 0 {
            carry = byte & 0x01 != 0;
            byte >>= 1;
            if carry {
                byte |= 0x80;
            }
            count -= 1;
        }
        byte
    }   

    /* 
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
                    log::error!("Invalid display mode selected: {:02X}", mode_byte & 0x0F);
                    DisplayMode::Mode3TextCo80
                }
            };
        }

        log::debug!("Mode Selected ({:?}:{:02X}) Enabled: {}", 
            self.display_mode,
            mode_byte, 
            self.mode_enable );
    }
    */

    /*
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
    */

    /*
    fn handle_cc_register_write(&mut self, data: u8) {
        log::trace!("Write to color control register: {:02X}", data);
        self.cc_register = data;
    }
    */


}

impl VideoCard for EGACard {

    fn get_video_type(&self) -> VideoType {
        VideoType::EGA
    }

    fn get_display_mode(&self) -> DisplayMode {
        self.display_mode
    }

    fn get_display_extents(&self) -> (u32, u32) {

        // EGA supports multiple fonts.

        let font_w = EGA_FONTS[self.current_font].w;
        let _font_h = EGA_FONTS[self.current_font].h;

        // Clock divisor effectively doubles the CRTC register values
        let clock_divisor = match self.sequencer_clocking_mode.dot_clock() {
            DotClock::Native => 1,
            DotClock::HalfClock => 2
        };

        //let width = (self.crtc_horizontal_display_end as u32 + 1) * clock_divisor * font_w as u32;
        let width = (self.crtc_horizontal_display_end as u32 + 1) * font_w as u32;
        let height = self.crtc_vertical_display_end as u32 + 1;
        (width, height)
    }

    fn get_clock_divisor(&self) -> u32 {
        match self.sequencer_clocking_mode.dot_clock() {
            DotClock::Native => 1,
            DotClock::HalfClock => 2
        }
    }

    fn is_40_columns(&self) -> bool {

        match self.display_mode {
            DisplayMode::Mode0TextBw40 => true,
            DisplayMode::Mode1TextCo40 => true,
            DisplayMode::Mode4LowResGraphics => true,
            DisplayMode::Mode5LowResAltPalette => true,
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
                    line_start: self.crtc_cursor_start,
                    line_end: self.crtc_cursor_end,
                    visible: self.get_cursor_status()
                }
            }
            DisplayMode::Mode2TextBw80 | DisplayMode::Mode3TextCo80 => {
                CursorInfo{
                    addr,
                    pos_x: addr % 80,
                    pos_y: addr / 80,
                    line_start: self.crtc_cursor_start,
                    line_end: self.crtc_cursor_end,
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

    fn get_current_font(&self) -> FontInfo {

        let w = EGA_FONTS[self.current_font].w;
        let h = EGA_FONTS[self.current_font].h;
        let data = EGA_FONTS[self.current_font].data;

        FontInfo {
            w,
            h,
            font_data: data
        }
    }

    fn get_character_height(&self) -> u8 {
        self.crtc_maximum_scanline + 1
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

    /// Returns a string representation of all the CRTC Registers.
    fn get_videocard_string_state(&self) -> HashMap<String, Vec<(String,String)>> {

        let mut map = HashMap::new();

        let mut general_vec = Vec::new();
        general_vec.push((format!("Adapter Type:"), format!("{:?}", self.get_video_type())));
        general_vec.push((format!("Display Mode:"), format!("{:?}", self.get_display_mode())));
        map.insert("General".to_string(), general_vec);

        let mut crtc_vec = Vec::new();
        crtc_vec.push((format!("{:?}", CRTCRegister::HorizontalTotal), format!("{}", self.crtc_horizontal_total)));
        crtc_vec.push((format!("{:?}", CRTCRegister::HorizontalDisplayEnd), format!("{}", self.crtc_horizontal_display_end)));
        crtc_vec.push((format!("{:?}", CRTCRegister::StartHorizontalBlank), format!("{}", self.crtc_start_horizontal_blank)));
        crtc_vec.push((format!("{:?}", CRTCRegister::EndHorizontalBlank), 
            format!("{}", self.crtc_end_horizontal_blank.end_horizontal_blank())));
        crtc_vec.push((format!("{:?} [des]", CRTCRegister::EndHorizontalBlank), 
            format!("{}", self.crtc_end_horizontal_blank.display_enable_skew())));            
        crtc_vec.push((format!("{:?}", CRTCRegister::StartHorizontalRetrace), format!("{}", self.crtc_start_horizontal_retrace)));
        crtc_vec.push((format!("{:?}", CRTCRegister::EndHorizontalRetrace),
            format!("{}", self.crtc_end_horizontal_retrace.end_horizontal_retrace())));
        crtc_vec.push((format!("{:?}", CRTCRegister::VerticalTotal), format!("{}", self.crtc_vertical_total)));
        crtc_vec.push((format!("{:?}", CRTCRegister::Overflow), format!("{}", self.crtc_overflow)));
        crtc_vec.push((format!("{:?}", CRTCRegister::PresetRowScan), format!("{}", self.crtc_preset_row_scan)));
        crtc_vec.push((format!("{:?}", CRTCRegister::MaximumScanLine), format!("{}", self.crtc_maximum_scanline)));
        crtc_vec.push((format!("{:?}", CRTCRegister::CursorStartLine), format!("{}", self.crtc_cursor_start)));
        crtc_vec.push((format!("{:?}", CRTCRegister::CursorEndLine), format!("{}", self.crtc_cursor_end)));
        crtc_vec.push((format!("{:?}", CRTCRegister::StartAddressH), format!("{}", self.crtc_start_address_ho)));
        crtc_vec.push((format!("{:?}", CRTCRegister::StartAddressL), format!("{}", self.crtc_start_address_lo)));
        crtc_vec.push((format!("{:?}", CRTCRegister::CursorAddressH), format!("{}", self.crtc_cursor_address_ho)));
        crtc_vec.push((format!("{:?}", CRTCRegister::CursorAddressL), format!("{}", self.crtc_cursor_address_lo)));
        crtc_vec.push((format!("{:?}", CRTCRegister::VerticalRetraceStart), format!("{}", self.crtc_vertical_retrace_start)));

        crtc_vec.push((format!("{:?}", CRTCRegister::VerticalRetraceEnd), 
            format!("{}", self.crtc_vertical_retrace_end.vertical_retrace_end())));
        crtc_vec.push((format!("{:?} [norm]", CRTCRegister::VerticalRetraceEnd), 
            format!("{}", self.crtc_vertical_retrace_end_norm)));
        
        crtc_vec.push((format!("{:?}", CRTCRegister::VerticalDisplayEnd), format!("{}", self.crtc_vertical_display_end)));
        crtc_vec.push((format!("{:?}", CRTCRegister::Offset), format!("{}", self.crtc_offset)));
        crtc_vec.push((format!("{:?}", CRTCRegister::UnderlineLocation), format!("{}", self.crtc_underline_location)));
        crtc_vec.push((format!("{:?}", CRTCRegister::StartVerticalBlank), format!("{}", self.crtc_start_vertical_blank)));
        crtc_vec.push((format!("{:?}", CRTCRegister::EndVerticalBlank), format!("{}", self.crtc_end_vertical_blank)));
        crtc_vec.push((format!("{:?}", CRTCRegister::ModeControl), format!("{}", self.crtc_mode_control)));
        crtc_vec.push((format!("{:?}", CRTCRegister::LineCompare), format!("{}", self.crtc_line_compare)));
        map.insert("CRTC".to_string(), crtc_vec);

        let mut external_vec = Vec::new();
        external_vec.push((format!("Misc Output"), format!("{:08b}", self.misc_output_register.into_bytes()[0])));
        external_vec.push((format!("Misc Output [ios]"), format!("{:?}", self.misc_output_register.io_address_select())));
        external_vec.push((format!("Misc Output [er]"), format!("{:?}", self.misc_output_register.enable_ram())));
        external_vec.push((format!("Misc Output [cs]"), format!("{:?}", self.misc_output_register.clock_select())));
        external_vec.push((format!("Misc Output [div]"), format!("{:?}", self.misc_output_register.disable_internal_drivers())));
        external_vec.push((format!("Misc Output [pb]"), format!("{:?}", self.misc_output_register.oddeven_page_select())));
        external_vec.push((format!("Misc Output [hrp]"), format!("{:?}", self.misc_output_register.horizontal_retrace_polarity())));
        external_vec.push((format!("Misc Output [vrp]"), format!("{:?}", self.misc_output_register.vertical_retrace_polarity())));
        map.insert("External".to_string(), external_vec);

        let mut sequencer_vec = Vec::new();
        sequencer_vec.push((format!("{:?}", SequencerRegister::Reset), format!("{:02b}", self.sequencer_reset)));
        sequencer_vec.push((format!("{:?}", SequencerRegister::ClockingMode), 
            format!("{:08b}", self.sequencer_clocking_mode.into_bytes()[0])));           
        sequencer_vec.push((format!("{:?} [cc]", SequencerRegister::ClockingMode), 
            format!("{:?}", self.sequencer_clocking_mode.character_clock())));
        sequencer_vec.push((format!("{:?} [bw]", SequencerRegister::ClockingMode), 
            format!("{}", self.sequencer_clocking_mode.bandwidth())));
        sequencer_vec.push((format!("{:?} [sl]", SequencerRegister::ClockingMode), 
            format!("{}", self.sequencer_clocking_mode.shift_load())));
        sequencer_vec.push((format!("{:?} [dc]", SequencerRegister::ClockingMode), 
            format!("{:?}", self.sequencer_clocking_mode.dot_clock())));

        sequencer_vec.push((format!("{:?}", SequencerRegister::MapMask), format!("{:04b}", self.sequencer_map_mask)));
        sequencer_vec.push((format!("{:?}", SequencerRegister::CharacterMapSelect), format!("{}", self.sequencer_character_map_select)));
        sequencer_vec.push((format!("{:?}", SequencerRegister::MemoryMode), format!("{}", self.sequencer_memory_mode)));
        map.insert("Sequencer".to_string(), sequencer_vec);

        let mut graphics_vec = Vec::new();
        graphics_vec.push((format!("{:?}", GraphicsRegister::SetReset), format!("{:04b}", self.graphics_set_reset)));
        graphics_vec.push((format!("{:?}", GraphicsRegister::EnableSetReset), format!("{:04b}", self.graphics_enable_set_reset)));
        graphics_vec.push((format!("{:?}", GraphicsRegister::ColorCompare), format!("{:04b}", self.graphics_color_compare)));
        graphics_vec.push((format!("{:?} [fn]", GraphicsRegister::DataRotate), 
            format!("{:?}", self.graphics_data_rotate.function())));
        graphics_vec.push((format!("{:?} [ct]", GraphicsRegister::DataRotate), 
            format!("{:?}", self.graphics_data_rotate.count())));              
        graphics_vec.push((format!("{:?}", GraphicsRegister::ReadMapSelect), format!("{:03b}", self.graphics_read_map_select)));

        graphics_vec.push((format!("{:?} [sr]", GraphicsRegister::Mode), 
            format!("{:?}", self.graphics_mode.shift_mode())));
        graphics_vec.push((format!("{:?} [o/e]", GraphicsRegister::Mode), 
            format!("{:?}", self.graphics_mode.odd_even())));
        graphics_vec.push((format!("{:?} [rm]", GraphicsRegister::Mode), 
            format!("{:?}",self.graphics_mode.read_mode())));
        graphics_vec.push((format!("{:?} [tc]", GraphicsRegister::Mode), 
            format!("{:?}", self.graphics_mode.test_condition())));
        graphics_vec.push((format!("{:?} [wm]", GraphicsRegister::Mode), 
            format!("{:?}", self.graphics_mode.write_mode())));

        graphics_vec.push((format!("{:?} [gm]", GraphicsRegister::Miscellaneous), 
            format!("{:?}", self.graphics_micellaneous.graphics_mode())));
        graphics_vec.push((format!("{:?} [com]", GraphicsRegister::Miscellaneous), 
            format!("{:?}", self.graphics_micellaneous.chain_odd_maps())));
        graphics_vec.push((format!("{:?} [mm]", GraphicsRegister::Miscellaneous), 
            format!("{:?}", self.graphics_micellaneous.memory_map())));            

        graphics_vec.push((format!("{:?}", GraphicsRegister::ColorDontCare), format!("{:04b}", self.graphics_color_dont_care)));
        graphics_vec.push((format!("{:?}", GraphicsRegister::BitMask), format!("{:08b}", self.graphics_bitmask)));
        map.insert("Graphics".to_string(), graphics_vec);

        let mut attribute_pal_vec = Vec::new();
        for i in 0..16 {
            attribute_pal_vec.push((format!("Palette register {}", i), 

                format!("{:06b}", self.attribute_palette_registers[i])
                /* 
                format!("{:01b}{:01b}{:01b},{:01b}{:01b}{:01b}", 
                    self.attribute_palette_registers[i].secondary_red(),
                    self.attribute_palette_registers[i].secondary_green(),
                    self.attribute_palette_registers[i].secondary_blue(),
                    self.attribute_palette_registers[i].red(),
                    self.attribute_palette_registers[i].green(),
                    self.attribute_palette_registers[i].blue(),
                )));
                */
            ));
        }
        map.insert("AttributePalette".to_string(), attribute_pal_vec);

        let mut attribute_vec = Vec::new();
        attribute_vec.push((format!("{:?} mode:", AttributeRegister::ModeControl), 
            format!("{:?}", self.attribute_mode_control.mode())));
        attribute_vec.push((format!("{:?} disp:", AttributeRegister::ModeControl), 
            format!("{:?}", self.attribute_mode_control.display_type())));
        attribute_vec.push((format!("{:?} elgc:", AttributeRegister::ModeControl), 
            format!("{:?}", self.attribute_mode_control.enable_line_character_codes())));
        attribute_vec.push((format!("{:?} attr:", AttributeRegister::ModeControl), 
            format!("{:?}", self.attribute_mode_control.enable_blink_or_intensity())));            

        attribute_vec.push((format!("{:?}", AttributeRegister::OverscanColor), 
            format!("{:06b}", self.attribute_overscan_color.into_bytes()[0])));
            
        attribute_vec.push((format!("{:?} en:", AttributeRegister::ColorPlaneEnable), 
            format!("{:04b}", self.attribute_color_plane_enable.enable_plane())));           
        attribute_vec.push((format!("{:?} mux:", AttributeRegister::ColorPlaneEnable), 
            format!("{:02b}", self.attribute_color_plane_enable.video_status_mux())));                
        attribute_vec.push((format!("{:?}", AttributeRegister::HorizontalPelPanning), 
            format!("{}", self.attribute_pel_panning)));     
        //attribute_overscan_color: AOverscanColor::new(),
        //attribute_color_plane_enable: AColorPlaneEnable::new(),
        map.insert("Attribute".to_string(), attribute_vec);

        map
    }

    fn run(&mut self, cpu_cycles: u32) {

        self.frame_cycles += cpu_cycles as f32;
        self.scanline_cycles += cpu_cycles as f32;

        // Select the appropriate timings based on the current clocking mode
        let ti = match self.misc_output_register.clock_select() {
            ClockSelect::Clock14 => 0,
            ClockSelect::Clock16 => 1,
            _ => 0
        };

        if self.frame_cycles > self.timings[ti].cpu_frame as f32 {
            self.frame_cycles -= self.timings[ti].cpu_frame as f32;
            self.cursor_frames += 1;
            // Blink the cursor
            let cursor_cycle = CGA_DEFAULT_CURSOR_FRAME_CYCLE * (self.cursor_slowblink as u32 + 1);
            if self.cursor_frames > cursor_cycle {
                self.cursor_frames -= cursor_cycle;
                self.cursor_status = !self.cursor_status;
            }
        }

        // CyclesPerFrame / VerticalTotal = CyclesPerScanline
        let cpu_scanline = self.timings[ti].cpu_frame as f32 / (self.crtc_vertical_total + 1 ) as f32;
        
        while self.scanline_cycles > cpu_scanline {
            self.scanline_cycles -= cpu_scanline ;
            if !self.in_vblank {
                self.scanline += 1;
            }
        }

        let hblank_start;
        let vblank_start;

        // Are we in HBLANK interval?
        if self.crtc_start_horizontal_retrace > 0 && self.crtc_horizontal_total > 0 {
            hblank_start = ((self.crtc_start_horizontal_retrace as f32 * 8.0) / (self.crtc_horizontal_total as f32 * 8.0) * cpu_scanline as f32) as u32;
            self.in_hblank = self.scanline_cycles > hblank_start as f32;
        }
        // Are we in VBLANK interval?
        if self.crtc_start_vertical_blank > 0 && self.crtc_vertical_total > 0 {
            vblank_start = ((self.crtc_start_vertical_blank as f32 / (self.crtc_vertical_total + 1) as f32) * self.timings[ti].cpu_frame as f32) as u32;
            self.in_vblank = self.frame_cycles > vblank_start as f32;

            if self.in_vblank {
                self.scanline = 0;
            }
        }
        //self.in_hblank = self.scanline_cycles > self.timings[ti].hblank_start;
        // Are we in VBLANK interval?
        //self.in_vblank = self.frame_cycles > self.timings[ti].vblank_start;
    }    

    fn reset(&mut self) {
        self.reset_private();
    }

    fn get_pixel(&self, x: u32, y: u32 ) -> &[u8] {
        &DUMMY_PIXEL
    }

    fn get_pixel_raw(&self, x: u32, y:u32) -> u8 {
        
        let mut byte = 0;

        let x_byte_offset = (x + self.attribute_pel_panning as u32) / 8;
        let x_bit_offset = (x + self.attribute_pel_panning as u32) % 8;


        // Get the current width of screen + offset
        // let span = (self.crtc_horizontal_display_end + 1 + 64) as u32;
        let span = self.crtc_offset as u32 * 2;

        let y_offset = y * span;

        // The line compare register resets the CRTC Start Address and line counter to 0 at the 
        // specified scanline. 
        // If we are above the value in Line Compare calculate the read offset as normal.
        let read_offset;
        if y >= self.crtc_line_compare as u32 {
            read_offset = (((y - self.crtc_line_compare as u32) * span) + x_byte_offset) as usize;
        }
        else {
            read_offset = (y_offset + x_byte_offset + self.crtc_start_address as u32 ) as usize;
        }
        
        if read_offset < self.planes[0].buf.len() {

            for i in 0..4 {
            
                let read_byte = self.planes[i].buf[read_offset];
                let read_bit = match read_byte & (0x01 << (7-x_bit_offset)) != 0 {
                    true => 1,
                    false => 0
                };
    
                //byte |= read_bit << (3 - i);
                byte |= read_bit << i;
            }
            // return self.attribute_palette_registers[byte & 0x0F].into_bytes()[0];
            return self.attribute_palette_registers[byte & 0x0F];
        }
        0
    }

    fn get_plane_slice(&self, plane: usize) -> &[u8] {

        &self.planes[plane].buf
    }

    fn dump_mem(&self) {
        
        for i in 0..4 {

            let filename = format!("./dumps/ega_plane{}.bin", i);
            
            match std::fs::write(filename.clone(), &self.planes[i].buf) {
                Ok(_) => {
                    log::debug!("Wrote memory dump: {}", filename)
                }
                Err(e) => {
                    log::error!("Failed to write memory dump '{}': {}", filename, e)
                }
            }
        }
    }

}

impl MemoryMappedDevice for EGACard {

    fn read_u8(&mut self, address: usize) -> u8 {

        // RAM Enable disables memory mapped IO
        if !self.misc_output_register.enable_ram() {
            return 0;
        }

        // Validate address is within current memory map and get the offset
        let offset = match self.plane_bounds_check(address) {
            Some(offset) => offset,
            None => {
                return 0;
            }
        };

        // Load all the latches regardless of selected plane
        for i in 0..4 {
            self.planes[i].latch = self.planes[i].buf[offset];
        }

        // Reads are controlled by the Read Mode bit in the Mode register of the Graphics Controller.
        match self.graphics_mode.read_mode() {
            ReadMode::ReadSelectedPlane => {
                // In Read Mode 0, the processor reads data from the memory plane selected 
                // by the read map select register.
                let plane = (self.graphics_read_map_select & 0x03) as usize;
                let byte = self.planes[plane].buf[offset];
                return byte;
            }
            ReadMode::ReadComparedPlanes => {
                // In Read Mode 1, the processor reads the result of a comparison with the value in the 
                // Color Compare register, from the set of enabled planes in the Color Dont Care register
                self.get_pixels(offset);
                let comparison = self.pixel_op_compare();
                return comparison;
            }
        }
    }

    fn read_u16(&mut self, address: usize) -> u16 {

        let lo_byte = MemoryMappedDevice::read_u8(self, address);
        let ho_byte = MemoryMappedDevice::read_u8(self, address + 1);


        //log::warn!("Unsupported 16 bit read from VRAM");
        return (ho_byte as u16) << 8 | lo_byte as u16
    }

    fn write_u8(&mut self, address: usize, byte: u8) {

        // RAM Enable disables memory mapped IO
        if !self.misc_output_register.enable_ram() {
            return
        }

        // Validate address is within current memory map and get the offset
        let offset = match self.plane_bounds_check(address) {
            Some(offset) => offset,
            None => {
                return
            }
        };        

        match self.graphics_mode.write_mode() {
            WriteMode::Mode0 => {

                // Write mode 0 performs a pipeline of operations:
                // First, data is rotated as specified by the Rotate Count field of the Data Rotate Register.
                let data_rot = EGACard::rotate_right_u8(byte, self.graphics_data_rotate.count());

                // Second, data is is either passed through to the next stage or replaced by a value determined
                // by the Set/Reset register. The bits in the Enable Set/Reset register controls whether this occurs.
                for i in 0..4 {
                    if self.graphics_enable_set_reset & (0x01 << i) != 0 {
                        // If the Set/Reset Enable bit is set, use expansion of corresponding Set/Reset register bit
                        self.pipeline_buf[i] = match self.graphics_set_reset & (0x01 << i) != 0 {
                            true  => 0xFF,
                            false => 0x00
                        }                        
                    }
                    else {
                        // Set/Reset Enable bit not set, use data from rotate step
                        self.pipeline_buf[i] = data_rot
                    }
                }

                // Third, the operation specified by the Logical Operation field of the Data Rotate register
                // is perfomed on the data for each plane and the latch read register.
                // A 1 bit in the Graphics Bit Mask register will use the bit result of the Logical Operation.
                // A 0 bit in the Graphics Bit Mask register will use the bit unchanged from the Read Latch register.
                for i in 0..4 {
                    self.pipeline_buf[i] = match self.graphics_data_rotate.function() {
                        RotateFunction::Unmodified => {
                            // Clear masked bits from pipeline, set them with mask bits from latch
                            (self.pipeline_buf[i] & self.graphics_bitmask) | (!self.graphics_bitmask & self.planes[i].latch)
                        }
                        RotateFunction::And => {
                            (self.pipeline_buf[i] | !self.graphics_bitmask) & self.planes[i].latch
                        }
                        RotateFunction::Or => {
                            (self.pipeline_buf[i] & self.graphics_bitmask) | self.planes[i].latch
                        }
                        RotateFunction::Xor => {
                            (self.pipeline_buf[i] & self.graphics_bitmask) ^ self.planes[i].latch
                        }
                    }
                }
                // Fourth, the value of the Bit Mask register is used: A set bit in the Mask register will pass
                // the bit from the data pipeline, a 0 bit will pass a bit from the read latch register.
                //for i in 0..4 {
//
                //    self.write_buf[i] = 0;
                //    
                //    for k in 0..8 {
                //        if self.graphics_bitmask & (0x01 << k) != 0 {
                //            // If a bit is set in the mask register, pass the bit from the previous stage
                //            self.write_buf[i] |= self.pipeline_buf[i] & (0x01 << k);
                //        }
                //        else {
                //            // Otherwise, pass the corresponding bit from the read latch register
                //            self.write_buf[i] |= self.planes[i].latch & (0x01 << k);
                //        }
                //    }
                //}

                // Finally, write data to the planes enabled in the Memory Plane Write Enable field of
                // the Sequencer Map Mask register.
                for i in 0..4 {
                    if self.sequencer_map_mask & (0x01 << i) != 0 {
                        self.planes[i].buf[offset] = self.pipeline_buf[i];
                    }
                }
            }
            WriteMode::Mode1 => {
                // Write the contents of the platches to their corresponding planes. This assumes that the latches
                // were loaded propery via a previous read operation.

                for i in 0..4 {
                    // Only write to planes enabled in the Sequencer Map Mask.
                    if self.sequencer_map_mask & (0x01 << i) != 0 {
                        self.planes[i].buf[offset] = self.planes[i].latch;
                    }
                }
            }
            WriteMode::Mode2 => {

                for i in 0..4 {
                    // Only write to planes enabled in the Sequencer Map Mask.
                    if self.sequencer_map_mask & (0x01 << i) != 0 {

                        // Extend the bit for this plane to 8 bits.
                        let bit_span: u8 = match byte & (0x01 << i) != 0 {
                            true => 0xFF,
                            false => 0x00,
                        };

                        // Clear bits not masked
                        self.planes[i].buf[offset] &= !self.graphics_bitmask;

                        // Mask off bits not to set
                        let set_bits = bit_span & self.graphics_bitmask;

                        self.planes[i].buf[offset] |= set_bits;
                    }
                }

                //log::warn!("Unimplemented write mode 2")
            }
            WriteMode::Invalid => {
                log::warn!("Invalid write mode!");
                return
            }
        }

    }

    fn write_u16(&mut self, address: usize, data: u16) {
        log::warn!("Unsupported 16 bit write to VRAM");
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_compare() {
        let mut ega = EGACard::new();

        ega.pixel_buf[0] = 0b1100;
        ega.pixel_buf[1] = 0b0101;
        ega.pixel_buf[2] = 0b1010;
        ega.pixel_buf[3] = 0b1111;
        ega.pixel_buf[4] = 0b0001;
        ega.pixel_buf[5] = 0b1010;
        ega.pixel_buf[6] = 0b1010;
        ega.pixel_buf[7] = 0b0010;

        ega.graphics_color_compare = 0b1010;
        ega.graphics_color_dont_care = 0b0000;

        let result = ega.pixel_op_compare();
        log::debug!("result: {:08b} expected: {:08b}", result, 0b00100110);
        assert_eq!(result, 0b00100110);

        ega.graphics_color_dont_care = 0b0111;
        let result = ega.pixel_op_compare();
        assert_eq!(result, 0b10110110);

        ega.graphics_color_dont_care = 0b0011;
        let result = ega.pixel_op_compare();
        assert_eq!(result, 0b00100110);        

        ega.graphics_color_dont_care = 0b1011;
        let result = ega.pixel_op_compare();
        assert_eq!(result, 0b00101111);

        ega.graphics_color_dont_care = 0b1000;
        let result = ega.pixel_op_compare();
        assert_eq!(result, 0b00100111);        


    }
}
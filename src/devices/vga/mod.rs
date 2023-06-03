/*
    Marty PC Emulator 
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

    ---------------------------------------------------------------------------

    vga::mod.rs

    Implement the IBM PS/2 (VGA) Graphics Adapter

    Resources:

    "Programmer's Guide to the EGA, VGA and Super VGA Cards", Richard F Ferraro 
    "EGA/VGA, A Programmer's Reference Guide 2nd Edition", Bradley Dyck Kliewer
    "Hardware Level VGA and SVGA Video Programming Information Page", 
        http://www.osdever.net/FreeVGA/home.htm

*/

#![allow(dead_code)]

use std::{
    collections::HashMap,
    path::{Path}
};

use modular_bitfield::prelude::*;

use crate::config::VideoType;
use crate::bus::{BusInterface, IoDevice, MemoryMappedDevice, DeviceRunTimeUnit};
use crate::tracelogger::TraceLogger;

use crate::videocard::*;

mod attribute_regs;
mod crtc_regs;
mod graphics_regs;
mod sequencer_regs;
mod color_regs;

use attribute_regs::*;
use crtc_regs::*;
use graphics_regs::*;
use sequencer_regs::*;
#[allow(unused_imports)]
use color_regs::*;

pub const VGA_CLOCK_1: f64 = 25.175;
pub const VGA_CLOCK_2: f64 = 28.322;
pub const US_PER_CLOCK_1: f64 = 1.0 / VGA_CLOCK_1;
pub const US_PER_CLOCK_2: f64 = 1.0 / VGA_CLOCK_2;

pub const CGA_ADDRESS: usize = 0xB8000;
pub const VGA_GFX_ADDRESS: usize = 0xA0000;

pub const MEM_SIZE_128K: usize = 131072;
pub const MEM_SIZE_64K: usize = 65536;
pub const MEM_SIZE_32K: usize = 32768;

// pub const CGA_MEM_SIZE: usize = 16384;
pub const VGA_TEXT_PLANE_SIZE: usize = 16384;
pub const VGA_GFX_PLANE_SIZE: usize = 65536;

// For an EGA card connected to an EGA monitor
// See http://www.minuszerodegrees.net/ibm_ega/ibm_ega_switch_settings.htm
// This is inverted (Checkit will report 0110)
// This is the only value that gives high-resolution text 640x350
pub const VGA_DIP_SWITCH: u8 = 0b0000_1001;

const CGA_DEFAULT_CURSOR_BLINK_RATE: f64 = 0.0625;
const CGA_DEFAULT_CURSOR_FRAME_CYCLE: u32 = 8;

const DEFAULT_CURSOR_START_LINE: u8 = 6;
const DEFAULT_CURSOR_END_LINE: u8 = 7;
const DEFAULT_HORIZONTAL_TOTAL: u8 = 113;
const DEFAULT_HORIZONTAL_DISPLAYED: u8 = 80;
const DEFAULT_HORIZONTAL_SYNC_POS: u8 = 90;
const DEFAULT_HORIZONTAL_SYNC_WIDTH: u8 = 10;

const DEFAULT_HORIZONTAL_BLANK: u8 = 0;

const DEFAULT_VERTICAL_TOTAL: u16 = 31;

const DEFAULT_VERTICAL_TOTAL_ADJUST: u8 = 6;
const DEFAULT_VERTICAL_DISPLAYED: u8 = 25;
const DEFAULT_VERTICAL_SYNC_POS: u8 = 28;

const DEFAULT_OVERFLOW: u8 = 0;
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

/* 
    EGA/VGA Ports

    Register Name      Write (EGA/VGA)  Read (EGA)  Read (VGA)
    -=---------------- ---------------  ----------  ----------
    Misc. Output                   3C2                     3CC
    Feature Control            3DA/3BA                     3CA
    Input Status #0                            3C2         3C2
    Input Status #1            3DA/3BA                 3DA/3BA
*/

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
pub const MISC_OUTPUT_REGISTER_READ: u16    = 0x3CC;    // Read address differs for EGA back compat.
pub const MISC_OUTPUT_REGISTER_WRITE: u16   = 0x3C2;    // Write-only to 3C2
pub const INPUT_STATUS_REGISTER_0: u16      = 0x3C2;    // Read-only from 3C2
pub const INPUT_STATUS_REGISTER_1: u16      = 0x3DA;    
pub const INPUT_STATUS_REGISTER_1_MDA: u16  = 0x3BA;    // Used in MDA compatibility mode

// Sequencer Registerrs
pub const SEQUENCER_ADDRESS_REGISTER: u16   = 0x3C4;    
pub const SEQUENCER_DATA_REGISTER: u16      = 0x3C5;

pub const CRTC_REGISTER_ADDRESS: u16        = 0x3D4;
pub const CRTC_REGISTER: u16                = 0x3D5;
pub const CRTC_REGISTER_ADDRESS_MDA: u16    = 0x3B4;    // Used in MDA compatibility mode
pub const CRTC_REGISTER_MDA: u16            = 0x3B5;    // Used in MDA compatibility mode

// Graphics Registers
//pub const GRAPHICS_1_POSITION: u16      = 0x3CC;    // EGA only
//pub const GRAPHICS_2_POSITION: u16      = 0x3CA;    // EGA only
pub const GRAPHICS_ADDRESS: u16             = 0x3CE;    
pub const GRAPHICS_DATA: u16                = 0x3CF;                                                    
             
// Color Registers
pub const PEL_ADDRESS_READ_MODE: u16        = 0x3C7;
pub const PEL_ADDRESS_WRITE_MODE: u16       = 0x3C8;
pub const PEL_DATA: u16                     = 0x3C9;
pub const PEL_MASK: u16                     = 0x3C6;
pub const DAC_STATE_REGISTER: u16           = 0x3C7;


/* cga things

//pub const CGA_MODE_CONTROL_REGISTER: u16  = 0x3D8;     // This register does not exist on the EGA
//pub const CGA_COLOR_CONTROL_REGISTER: u16 = 0x3D9;     // This register does not exist on the EGA
//pub const CGA_LIGHTPEN_REGISTER: u16      = 0x3DB;

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

/* This lookup table holds the values for the first color register that should
   trigger the switch sense bit to be set. I am not sure of the actual triggering
   condition, but we model what the BIOS expects
*/
static SWITCH_SENSE_LUT: [[u8; 3]; 6] = 
    [[0x14, 0x14, 0x14],
    [0x04, 0x12, 0x04],
    [0x00, 0x00, 0x00],
    [0x04, 0x12, 0x04],
    [0x12, 0x12, 0x12],
    [0x04, 0x04, 0x04]];

pub struct VideoTimings {
    cpu_frame: u32,
    vblank_start: u32,
    cpu_scanline: u32,
    hblank_start: u32
}

#[derive (Default)]
pub struct VideoMicroTimings {
    character_clock: u32,
    frame_end: u32,
    scanline_end: u32,
    hblank_start: u32,
    hblank_end: u32,
    vblank_start: u32,
    vblank_end: u32
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
        data: include_bytes!("../../../assets/ega_8by8.bin"),
        
    },
    EGAFont {
        w: 8,
        h: 14,
        span: 256,
        data: include_bytes!("../../../assets/ega_8by14.bin"),
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
            buf: Box::new([0; VGA_GFX_PLANE_SIZE])
        }
    }
}

macro_rules! trace {
    ($self:ident, $($t:tt)*) => {{
        $self.trace_logger.print(&format!($($t)*));
    }};
}

pub(crate) use trace;

pub struct VGACard {

    timings: [VideoTimings; 2],
    u_timings: VideoMicroTimings,
    extents: DisplayExtents,
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
    scanline_cycles: u32,
    frame_cycles: u32,
    vga_cycle_accumulator: f64,
    cursor_frames: u32,
    in_hblank: bool,
    in_vblank: bool,
    
    cursor_status: bool,
    cursor_slowblink: bool,
    cursor_blink_rate: f64,

    cc_register: u8,

    crtc_register_select_byte: u8,
    crtc_register_selected: CRTCRegister,
    protect_crtc_registers: bool,           // When set, registers 0-7 are read-only

    crtc_horizontal_total: u8,              // R(0) Horizontal Total
    crtc_horizontal_display_end: u8,        // R(1) Horizontal Display End
    crtc_start_horizontal_blank: u8,        // R(2) Start Horizontal Blank
    crtc_end_horizontal_blank: CEndHorizontalBlank,         // R(3) Bits 0-4 - End Horizontal Blank
    crtc_end_horizontal_blank_norm: u8,     // End Horizontal Blank value normalized to column number

    crtc_start_horizontal_retrace: u8,      // R(4) Start Horizontal Retrace
    crtc_end_horizontal_retrace: CEndHorizontalRetrace,     // R(5) End Horizontal Retrace
    crtc_end_horizontal_retrace_norm: u8,   // End Horizontal Retrace value normalized to column number
    crtc_vertical_total: u16,               // R(6) Vertical Total (9-bit value)
    crtc_overflow: u8,                      // R(7) Overflow
    crtc_preset_row_scan: CPresetRowScan,   // R(8) Preset Row Scan
    crtc_maximum_scanline: CMaximumScanline,// R(9) Max Scanline
    crtc_cursor_start: CCursorStart,        // R(A)
    crtc_cursor_end: CCursorEnd,            // R(B)
    crtc_start_address_ho: u8,              // R(C)
    crtc_start_address_lo: u8,              // R(D)
    crtc_start_address: u16,                // Calculated from C&D
    crtc_cursor_address_lo: u8,             // R(E)
    crtc_cursor_address_ho: u8,             // R(F)
    crtc_vertical_retrace_start: u16,       // R(10) Vertical Retrace Start (9-bit value)
    crtc_vertical_retrace_end: CVerticalRetraceEnd, // R(11) 
    crtc_vertical_retrace_end_norm: u16,    // Vertical Retrace End value normalized to scanline number
    crtc_vertical_display_end: u16,         // R(12) Vertical Display Enable End (9-bit value)
    crtc_offset: u8,                        // R(13)
    crtc_underline_location: CUnderlineLocation, // R(14)
    crtc_start_vertical_blank: u16,         // R(15) Start Vertical Blank (9-bit value)
    crtc_end_vertical_blank: u8,            // R(16)
    crtc_end_vertical_blank_norm: u16,      // End Vertical Blank value normalized to scanline number
    crtc_mode_control: CModeControl,        // R(17)
    crtc_line_compare: u16,                 // R(18) Line Compare (9-bit value)

    sequencer_address_byte: u8,
    sequencer_register_selected: SequencerRegister,
    sequencer_reset: u8,                            // S(0) Reset (WO)
    sequencer_clocking_mode: SClockingModeRegister, // S(1) Clocking Mode (WO)
    sequencer_map_mask: u8,                         // S(2) Map Mask (wO)
    sequencer_character_map_select: SCharacterMapSelect, // S(3) Character Map Select (WO)
    sequencer_character_map_a: u8,                  // Calculated from extension bit in VGA
    sequencer_character_map_b: u8,                  // Calculated from extension bit in VGA
    sequencer_memory_mode: SMemoryMode,             // S(4) Memory Mode (wO)

    graphics_register_address: u8,
    graphics_register_selected: GraphicsRegister,
    graphics_set_reset: u8,
    graphics_enable_set_reset: u8,
    graphics_color_compare: u8,
    graphics_data_rotate: GDataRotateRegister,   
    graphics_read_map_select: u8,
    graphics_mode: GModeRegister,
    graphics_micellaneous: GMiscellaneousRegister,
    graphics_color_dont_care: u8,
    graphics_bitmask: u8,

    attribute_flipflop: AttributeRegisterFlipFlop,
    attribute_address: AttributeAddress,
    attribute_selected: AttributeRegister,
    attribute_palette_registers: [u8; 16],
    attribute_palette_index: usize,
    attribute_mode_control: AModeControl,
    attribute_overscan_color: AOverscanColor,
    attribute_color_plane_enable: AColorPlaneEnable,
    attribute_pel_panning: u8,
    attribute_color_select: AColorSelect,       // New on VGA

    color_pel_write_address: u8,
    color_pel_write_address_color: u8,
    color_pel_read_address: u8,
    color_pel_read_address_color: u8,
    color_dac_state: u8,
    color_pel_mask: u8,

    color_registers: [[u8; 3]; 256],
    color_registers_rgba: [[u8; 4]; 256],

    current_font: usize,

    misc_output_register: EMiscellaneousOutputRegister,

    latch_addr: u32,

    // Display Planes
    planes: [DisplayPlane; 4],
    pixel_buf: [u8; 8],
    pipeline_buf: [u8; 4],
    write_buf: [u8; 4],

    trace_logger: TraceLogger,
}

#[bitfield]
#[derive (Copy, Clone)]
struct EMiscellaneousOutputRegister {
    #[bits = 1]
    io_address_select: IoAddressSelect,
    enable_ram: bool,
    #[bits = 2]
    clock_select: ClockSelect,
    disable_internal_drivers: bool, // This field unused in VGA
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
    Clock25,
    Clock28,
    Reserved1,
    Reserved2
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

/// Implement Device IO for the VGA Card.
/// 
/// Unlike the EGA, most of the registers on the VGA are readable.
impl IoDevice for VGACard {
    fn read_u8(&mut self, port: u16, _delta: DeviceRunTimeUnit) -> u8 {
        match port {
            MISC_OUTPUT_REGISTER_READ => {
                self.misc_output_register.into_bytes()[0]
            }
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
            GRAPHICS_ADDRESS => {
                self.graphics_register_address
            }
            GRAPHICS_DATA => {
                self.read_graphics_data()
            }            
            SEQUENCER_ADDRESS_REGISTER => {
                self.sequencer_address_byte
            }
            SEQUENCER_DATA_REGISTER => {
                self.read_sequencer_data()
            }
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
            PEL_ADDRESS_WRITE_MODE => {
                self.color_pel_write_address
            }
            PEL_DATA => {
                self.read_pel_data()
            }
            DAC_STATE_REGISTER => {
                // Read only register
                self.color_dac_state
            }
            PEL_MASK => {
                self.color_pel_mask
            }
            _ => {
                0xFF
            }
        }
    }

    fn write_u8(&mut self, port: u16, data: u8, _bus: Option<&mut BusInterface>, _delta: DeviceRunTimeUnit) {
        match port {
            MISC_OUTPUT_REGISTER_WRITE => {
                self.write_external_misc_output_register(data);
            }
            //MODE_CONTROL_REGISTER => {
            //    self.handle_mode_register(data);
            //}
            CRTC_REGISTER_ADDRESS => {
                // Don't listen on this port if we are in MDA compatibility mode
                
                //match self.misc_output_register.io_address_select() { 
                //    IoAddressSelect::CompatMonochrome => {},                  
                //    IoAddressSelect::CompatCGA => self.write_crtc_register_address(data)
                //}
                self.write_crtc_register_address(data)
            }
            CRTC_REGISTER => {
                // Don't listen on this port if we are in MDA compatibility mode
                //match self.misc_output_register.io_address_select() {     
                //    IoAddressSelect::CompatMonochrome => {},           
                //    IoAddressSelect::CompatCGA => self.write_crtc_register_data(data)
                //}

                self.write_crtc_register_data(data)
            }
            //GRAPHICS_1_POSITION => {
            //    self.write_graphics_position(1, data)
            //}
            //GRAPHICS_2_POSITION => {
            //    self.write_graphics_position(2, data)
            //}            
            GRAPHICS_ADDRESS => {
                self.write_graphics_address(data)
            }
            GRAPHICS_DATA => {
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
            PEL_ADDRESS_WRITE_MODE => {
                self.color_pel_write_address = data
            }      
            PEL_ADDRESS_READ_MODE => {
                self.color_pel_read_address = data
            }   
            PEL_DATA => {
                self.write_pel_data(data)
            }
            PEL_MASK => {
                self.color_pel_mask = data
            }                              
            //COLOR_CONTROL_REGISTER => {
            //    self.handle_cc_register_write(data);
            //}
            _ => {}
        }
    }

    fn port_list(&self) -> Vec<u16> {
        vec![
            ATTRIBUTE_REGISTER,
            ATTRIBUTE_REGISTER_ALT,
            MISC_OUTPUT_REGISTER_READ,
            MISC_OUTPUT_REGISTER_WRITE,
            INPUT_STATUS_REGISTER_0,
            INPUT_STATUS_REGISTER_1,
            INPUT_STATUS_REGISTER_1_MDA,
            SEQUENCER_ADDRESS_REGISTER,
            SEQUENCER_DATA_REGISTER,
            CRTC_REGISTER_ADDRESS,
            CRTC_REGISTER,
            CRTC_REGISTER_ADDRESS_MDA,
            CRTC_REGISTER_MDA,
            GRAPHICS_ADDRESS,
            GRAPHICS_DATA,                                          
            PEL_ADDRESS_READ_MODE,
            PEL_ADDRESS_WRITE_MODE,
            PEL_DATA,
            PEL_MASK,
            DAC_STATE_REGISTER,
        ]
    }

}

impl VGACard {

    pub fn new(trace_logger: TraceLogger) -> Self {
        Self {

            timings: [
                VideoTimings {
                    cpu_frame: CGA_FRAME_CPU_TIME,
                    vblank_start: CGA_VBLANK_START,
                    cpu_scanline: CGA_SCANLINE_CPU_TIME,
                    hblank_start: CGA_HBLANK_START
                },
                VideoTimings {
                    cpu_frame: EGA_FRAME_CPU_TIME,
                    vblank_start: EGA_VBLANK_START,
                    cpu_scanline: EGA_SCANLINE_CPU_TIME,
                    hblank_start: EGA_HBLANK_START,
                }
            ],
            u_timings: Default::default(),
            extents: Default::default(),
            mode_byte: 0,
            display_mode: DisplayMode::Mode3TextCo80,
            mode_enable: true,
            mode_graphics: false,
            mode_bw: false,
            mode_line_gfx: false,
            mode_hires_gfx: false,
            mode_hires_txt: true,
            mode_blinking: true,
            frame_cycles: 0,
            vga_cycle_accumulator: 0.0,
            cursor_frames: 0,
            scanline: 0,
            scanline_cycles: 0,
            in_hblank: false,
            in_vblank: false,

            cursor_status: false,
            cursor_slowblink: false,
            cursor_blink_rate: CGA_DEFAULT_CURSOR_BLINK_RATE,

            cc_register: CC_PALETTE_BIT | CC_BRIGHT_BIT,

            crtc_register_selected: CRTCRegister::HorizontalTotal,
            crtc_register_select_byte: 0,
            protect_crtc_registers: false,

            crtc_horizontal_total: DEFAULT_HORIZONTAL_TOTAL,
            crtc_horizontal_display_end: DEFAULT_HORIZONTAL_DISPLAYED,
            crtc_start_horizontal_blank: DEFAULT_HORIZONTAL_SYNC_POS,
            crtc_end_horizontal_blank: CEndHorizontalBlank::new(),
            crtc_end_horizontal_blank_norm: 0,

            crtc_start_horizontal_retrace: 0,
            crtc_end_horizontal_retrace: CEndHorizontalRetrace::new(),
            crtc_end_horizontal_retrace_norm: 0,
            crtc_vertical_total: DEFAULT_VERTICAL_TOTAL,
            crtc_overflow: 0,
            crtc_preset_row_scan: CPresetRowScan::new(),
            crtc_maximum_scanline: CMaximumScanline::new()
                .with_maximum_scanline(DEFAULT_MAX_SCANLINE),
            crtc_cursor_start: CCursorStart::new()
                .with_cursor_start(DEFAULT_CURSOR_START_LINE),
            crtc_cursor_end: CCursorEnd::new()
                .with_cursor_end(DEFAULT_CURSOR_END_LINE),
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
            crtc_underline_location: CUnderlineLocation::new(),
            crtc_start_vertical_blank: 0,
            crtc_end_vertical_blank: 0,
            crtc_end_vertical_blank_norm: 0,
            crtc_mode_control: CModeControl::new(),
            crtc_line_compare: 0,
        
            sequencer_address_byte: 0,
            sequencer_register_selected: SequencerRegister::Reset,
            sequencer_reset: 0,
            sequencer_clocking_mode: SClockingModeRegister::new(),
            sequencer_map_mask: 0,
            sequencer_character_map_select: SCharacterMapSelect::new(),
            sequencer_character_map_a: 0,
            sequencer_character_map_b: 0,
            sequencer_memory_mode: SMemoryMode::new(),     
            
            graphics_register_address: 0,
            graphics_register_selected: GraphicsRegister::SetReset,
            graphics_set_reset: 0,
            graphics_enable_set_reset: 0,
            graphics_color_compare: 0,
            graphics_data_rotate: GDataRotateRegister::new(),
            graphics_read_map_select: 0,
            graphics_mode: GModeRegister::new(),
            graphics_micellaneous: GMiscellaneousRegister::new(),
            graphics_color_dont_care: 0,
            graphics_bitmask: 0,

            attribute_flipflop: AttributeRegisterFlipFlop::Address,
            attribute_address: AttributeAddress::new(),
            attribute_selected: AttributeRegister::Palette0,
            attribute_palette_registers: [0; 16],
            attribute_palette_index: 0,
            attribute_mode_control: AModeControl::new(),
            attribute_overscan_color: AOverscanColor::new(),
            attribute_color_plane_enable: AColorPlaneEnable::new(),
            attribute_pel_panning: 0,
            attribute_color_select: AColorSelect::new(),

            color_pel_write_address: 0,
            color_pel_write_address_color: 0,
            color_pel_read_address: 0,
            color_pel_read_address_color: 0,
            color_dac_state: 0,
            color_pel_mask: 0,
            color_registers: [[0; 3]; 256],
            color_registers_rgba: [[0; 4]; 256],

            current_font: 0,
            misc_output_register: EMiscellaneousOutputRegister::new(),
            latch_addr: 0,

            planes: [
                DisplayPlane::new(),
                DisplayPlane::new(),
                DisplayPlane::new(),
                DisplayPlane::new()
            ],

            pixel_buf: [0; 8],
            pipeline_buf: [0; 4],
            write_buf: [0; 4],

            trace_logger
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
        self.frame_cycles = 0;
        self.cursor_frames = 0;
        self.scanline = 0;
        self.scanline_cycles = 0;
        self.in_hblank = false;
        self.in_vblank = false;

        self.cursor_status = false;
        self.cursor_slowblink = false;
        self.cursor_blink_rate = CGA_DEFAULT_CURSOR_BLINK_RATE;

        //self.cc_register: CC_PALETTE_BIT | CC_BRIGHT_BIT,

        self.crtc_register_selected = CRTCRegister::HorizontalTotal;
        self.crtc_register_select_byte = 0;
        self.protect_crtc_registers = false;

        self.crtc_horizontal_total = DEFAULT_HORIZONTAL_TOTAL;
        self.crtc_horizontal_display_end = DEFAULT_HORIZONTAL_DISPLAYED;
        self.crtc_start_horizontal_blank = DEFAULT_HORIZONTAL_SYNC_POS;
        self.crtc_end_horizontal_blank = CEndHorizontalBlank::new()
            .with_end_horizontal_blank(DEFAULT_HORIZONTAL_BLANK);

        self.crtc_start_horizontal_retrace = 0;
        self.crtc_end_horizontal_retrace = CEndHorizontalRetrace::new();
        self.crtc_vertical_total = DEFAULT_VERTICAL_TOTAL;
        self.crtc_overflow = DEFAULT_OVERFLOW;
        self.crtc_preset_row_scan = CPresetRowScan::new();
        self.crtc_maximum_scanline = CMaximumScanline::new()
            .with_maximum_scanline(DEFAULT_MAX_SCANLINE);

        self.crtc_cursor_start = CCursorStart::new()
            .with_cursor_start(DEFAULT_CURSOR_START_LINE);
        self.crtc_cursor_end = CCursorEnd::new()
            .with_cursor_end(DEFAULT_CURSOR_END_LINE);
        
    }

    fn get_cursor_span(&self) -> (u8, u8) {
        (self.crtc_cursor_start.cursor_start(), self.crtc_cursor_end.cursor_end())
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

        /*
            The switch sense bit on the VGA seems to relate to what Color Register 0 
            is programmed for. There's a table in the BIOS with some values that get 
            written to Color Register 0 along with the status of the Switch Sense bit,
            and the BIOS will error if they don't match. I couldn't figure out the 
            pattern here so I represent the palette contents that set the bit in 
            SWITCH_SENSE_LUT.  If Color Regsiter 0 matches an entry in the LUT, set
            the Switch Sense bit.
        */
        for i in 0..SWITCH_SENSE_LUT.len() {
            if SWITCH_SENSE_LUT[i][0] == self.color_registers[0][0] 
                && SWITCH_SENSE_LUT[i][1] == self.color_registers[0][1] 
                && SWITCH_SENSE_LUT[i][2] == self.color_registers[0][2] {

                    log::trace!("Setting switch status bit from Color Register 0 contents.");
                    byte |= 0x10;
                    break;
                }
        }

        // Set CRT interrupt bit. Bit is 0 when retrace is occurring.
        // TODO: Is this EGA specific?
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
        self.attribute_flipflop = AttributeRegisterFlipFlop::Address;

        let mut byte = 0;

        // Display Enable NOT bit is set to 1 if display is in vsync or hsync period
        // Note: IBM's documentation on this bit is wrong. 
        if self.in_hblank || self.in_vblank {
            byte |= 0x01;
        }
        if self.in_vblank {
            byte |= 0x08;
        }

        /*
            Ferraro lists the two 'DIA' bits as EGA-specific, but it is clear that the IBM
            VGA BIOS checks them as well after writing a line of 0xFFFF to the screen, 
            waiting for them to turn on and then back off again.
        
            The EGA can feed two lines off the Attribute Controller's color outputs back 
            into the Input Status Register 1 bits 4 & 5. Which lines to feed back are 
            controlled by bits 4 & 5 of the Color Plane Enable Register Video Status 
            Mux Field. 
            
            It's not clear if the VGA DIA bits behave in exactly the same manner, but we 
            can fake them the same way. Set them on if the scanline counter is 0, with an 
            added condition that the Screen Off bit is also set, which it is during POST.
        */

        if self.sequencer_clocking_mode.screen_off() && self.scanline == 0 {
            byte |= 0x30;
        }

        /*
        log::trace!("isr1 read: {:04b} scanline: {} slc: {} [{}:{}]", 
            byte, 
            self.scanline, 
            self.scanline_cycles, 
            self.u_timings.hblank_start, 
            self.u_timings.hblank_end);
        */
        byte
    }

    /// Calculate the current display mode based on the various register parameters of the VGA
    /// 
    /// The VGA doesn't have a convenient mode register like the CGA to determine display mode.
    /// Instead several fields are used: 
    /// Sequencer Clocking Mode Register Dot Clock field: Determines 320 low res modes 0,1,4,5
    /// Sequencer Memory Mode Register: Alpha bit: Determines alphanumeric mode
    /// Attribute Controller Mode Control: Graphics/Alpha bit. Also determines alphanumeric mode
    /// Attribute Controller Mode Control: Display Type bit. Determines Color or Monochrome
    /// 
    fn recalculate_mode(&mut self) {

        if self.crtc_maximum_scanline.maximum_scanline() > 7 {
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
                    (79, AttributeDisplayType::Color) => {

                        if self.sequencer_memory_mode.chain4_enable() {
                            // Chain4 => mode13h
                            DisplayMode::Mode13VGALowRes256
                        }
                        else {
                            match self.crtc_vertical_display_end {

                                349 => DisplayMode::Mode10EGAHiResGraphics,
                                479 => DisplayMode::Mode12VGAHiResGraphics,
                                _ => DisplayMode::Mode12VGAHiResGraphics
                            }
                        }

                    } 
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
                if address >= VGA_GFX_ADDRESS && address < VGA_GFX_ADDRESS + MEM_SIZE_128K {
                    return Some(address - VGA_GFX_ADDRESS);
                }
                else {
                    return None;
                }
            }
            MemoryMap::A0000_64K => {
                if address >= VGA_GFX_ADDRESS && address < VGA_GFX_ADDRESS + MEM_SIZE_64K {
                    return Some(address - VGA_GFX_ADDRESS);
                }
                else {
                    return None;
                }
            }
            MemoryMap::B8000_32K => {
                if address >= CGA_ADDRESS && address < CGA_ADDRESS + MEM_SIZE_32K {
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

        /*
            There is conflicting documentation on the meaning of a set bit in the Color Don't
            care register. Some sources state a 1 bit means ignore the plane in the comparison,
            others specify that 0 means ignore the plane. 

            I suppose the VGA BIOS would be authoritative - it writes 0xAA to 64k of video
            RAM and then reads it back in Read Mode 1 with Color Don't Care set to 0x0F.  It
            expects to get 0xAA back, but this only works if bit 1 in Color Don't Care means 
            that the plane counts in the comparision.
        */


        // Bits normally set to 1 in the Color Don't Care register mean to ignore the bit in 
        // comparison. By OR'ing the Color Don't Care with the Color Comparison & Src pixel
        // we can effectively force them to match and thus 'don't care'

        //let color_compare = self.graphics_color_compare | self.graphics_color_dont_care;
        //
        //let mut bit;
        //for p in 0..8 {
        //    if color_compare == self.pixel_buf[p] | self.graphics_color_dont_care {
        //        bit = 1;
        //    }
        //    else {
        //        bit = 0;
        //    }
        //    comparison |= bit << (7-p);
        //}
        //comparison

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

    pub fn recalculate_timings(&mut self) {

        let char_clock = match self.sequencer_clocking_mode.character_clock() {
            CharacterClock::EightDots => 8u32,
            CharacterClock::NineDots => 9u32
        };

        self.u_timings.character_clock = char_clock;

        self.u_timings.scanline_end = (self.crtc_horizontal_total as u32 + 5) * char_clock;
        
        //self.u_timings.hblank_start = self.crtc_start_horizontal_blank as u32 * char_clock;
        //self.u_timings.hblank_end = self.crtc_end_horizontal_blank_norm as u32 * char_clock; 
        self.u_timings.hblank_start = (self.crtc_horizontal_display_end + 1) as u32 * char_clock;
        self.u_timings.hblank_end = self.u_timings.scanline_end;

        self.u_timings.vblank_start = self.crtc_start_vertical_blank as u32;
        self.u_timings.vblank_end = self.crtc_end_vertical_blank_norm as u32;
        
    }

    fn tick(&mut self) {

        self.frame_cycles += 1;
        self.scanline_cycles += 1;

        if self.scanline_cycles >= self.u_timings.hblank_start && self.scanline_cycles <= self.u_timings.hblank_end {
            self.in_hblank = true;
        }
        else {
            self.in_hblank = false;
        }

        if self.scanline_cycles >= self.u_timings.scanline_end {
            self.scanline_cycles = 0;
            
            if self.scanline == (self.crtc_vertical_total + 2) as u32 {
                //log::trace!("last scanline hit: {}", self.scanline);
                self.scanline = 0;
                self.frame_cycles = 0;
            }
            else {
                self.scanline += 1;
            }
        }

        if self.crtc_vertical_display_end > 0 {
            if self.scanline > self.crtc_vertical_display_end as u32 {
                if !self.in_vblank {
                    // Transitioning to vblank
                    //log::trace!("vblank at {} cycles", self.frame_cycles);
                }
                self.in_vblank = true;
            }
            else {
                self.in_vblank = false;
            }
        }

        //if self.scanline >= self.crtc_start_vertical_blank as u32 && self.scanline < self.crtc_end_vertical_blank_norm as u32 {
        //    self.in_vblank = true;
        //}
        //else {
        //    self.in_vblank = false;
        //}

        //if self.scanline >= self.crtc_vertical_retrace_start as u32 && self.scanline < self.crtc_vertical_retrace_end_norm as u32 {
        //    self.in_vblank = true;
        //}
        //else {
        //    self.in_vblank = false;
        //}
        
    }

}

macro_rules! push_reg_str {
    ($vec: expr, $reg: expr, $decorator: expr, $val: expr ) => {
        $vec.push((format!("{:?} {}", $reg, $decorator), VideoCardStateEntry::String(format!("{}", $val))))
    };
}

macro_rules! push_reg_str_bin8 {
    ($vec: expr, $reg: expr, $decorator: expr, $val: expr ) => {
        $vec.push((format!("{:?} {}", $reg, $decorator), VideoCardStateEntry::String(format!("{:08b}", $val))))
    };
}

macro_rules! push_reg_str_enum {
    ($vec: expr, $reg: expr, $decorator: expr, $val: expr ) => {
        $vec.push((format!("{:?} {}", $reg, $decorator), VideoCardStateEntry::String(format!("{:?}", $val))))
    };
}    

impl VideoCard for VGACard {

    fn get_video_type(&self) -> VideoType {
        VideoType::VGA
    }

    fn get_render_mode(&self) -> RenderMode {
        RenderMode::Indirect
    }

    fn get_display_mode(&self) -> DisplayMode {
        self.display_mode
    }

    fn get_display_size(&self) -> (u32, u32) {

        // VGA supports multiple fonts.

        let font_w = EGA_FONTS[self.current_font].w;
        let _font_h = EGA_FONTS[self.current_font].h;

        // Clock divisor effectively doubles the CRTC register values
        let _clock_divisor = match self.sequencer_clocking_mode.dot_clock() {
            DotClock::Native => 1,
            DotClock::HalfClock => 2
        };

        //let width = (self.crtc_horizontal_display_end as u32 + 1) * clock_divisor * font_w as u32;
        let width = (self.crtc_horizontal_display_end as u32 + 1) * font_w as u32;
        let height = self.crtc_vertical_display_end as u32 + 1;
        (width, height)
    }

    /// Unimplemented for indirect rendering.
    fn get_display_extents(&self) -> &DisplayExtents {
        &self.extents
    }

    /// Unimplemented for indirect rendering.
    fn get_display_aperture(&self) -> (u32, u32) {
        (0, 0)
    }

    /// Unimplemented for indirect rendering.
    fn get_beam_pos(&self) -> Option<(u32, u32)> {
        None
    }

    fn debug_tick(&mut self, _ticks: u32) {
        self.tick();
    }

    fn get_overscan_color(&self) -> u8 {
        0
    }    
    
    /// Get the current scanline being rendered.
    fn get_scanline(&self) -> u32 {
        0
    }

    /// Return whether to double scanlines produced by this adapter.
    /// For VGA, this is false.
    fn get_scanline_double(&self) -> bool {
        false
    }

    /// Unimplemented for indirect rendering.
    fn get_display_buf(&self) -> &[u8] {
        &[0]
    }

    /// Unimplemented for indirect rendering.
    fn get_back_buf(&self) -> &[u8] {
        &[0]
    }    
    
    /// Return the current refresh rate.
    /// TODO: Handle VGA 70Hz modes.
    fn get_refresh_rate(&self) -> u32 {
        60
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
                    addr: addr as usize,
                    pos_x: addr % 40,
                    pos_y: addr / 40,
                    line_start: self.crtc_cursor_start.cursor_start(),
                    line_end: self.crtc_cursor_end.cursor_end(),
                    visible: self.get_cursor_status()
                }
            }
            DisplayMode::Mode2TextBw80 | DisplayMode::Mode3TextCo80 => {
                CursorInfo{
                    addr: addr as usize,
                    pos_x: addr % 80,
                    pos_y: addr / 80,
                    line_start: self.crtc_cursor_start.cursor_start(),
                    line_end: self.crtc_cursor_end.cursor_end(),
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
        //self.crtc_maximum_scanline.maximum_scanline() + 1

        14
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
    fn get_videocard_string_state(&self) -> HashMap<String, Vec<(String, VideoCardStateEntry)>> {

        let mut map: HashMap<String, Vec<(String, VideoCardStateEntry)>> = HashMap::new();

        let mut general_vec = Vec::new();
        push_reg_str_enum!(general_vec, "Adapter Type:", "", self.get_video_type());
        push_reg_str_enum!(general_vec, "Display Mode:", "", self.get_display_mode());

        //general_vec.push((format!("Adapter Type:"), format!("{:?}", self.get_video_type())));
        //general_vec.push((format!("Display Mode:"), format!("{:?}", self.get_display_mode())));
        map.insert("General".to_string(), general_vec);

        let mut crtc_vec = Vec::new();

        push_reg_str!(crtc_vec, CRTCRegister::HorizontalTotal, "", self.crtc_horizontal_total);
        push_reg_str!(crtc_vec, CRTCRegister::HorizontalDisplayEnd, "", self.crtc_horizontal_display_end);
        push_reg_str!(crtc_vec, CRTCRegister::StartHorizontalBlank, "", self.crtc_start_horizontal_blank);
        push_reg_str!(crtc_vec, CRTCRegister::EndHorizontalBlank, "", self.crtc_end_horizontal_blank.end_horizontal_blank());
        push_reg_str!(crtc_vec, CRTCRegister::EndHorizontalBlank, "[norm]", self.crtc_end_horizontal_blank_norm);
        push_reg_str!(crtc_vec, CRTCRegister::EndHorizontalBlank, "[des]", self.crtc_end_horizontal_blank.display_enable_skew());
        push_reg_str!(crtc_vec, CRTCRegister::StartHorizontalRetrace, "", self.crtc_start_horizontal_retrace);
        push_reg_str!(crtc_vec, CRTCRegister::EndHorizontalRetrace, "", self.crtc_end_horizontal_retrace.end_horizontal_retrace());
        push_reg_str!(crtc_vec, CRTCRegister::EndHorizontalRetrace, "[norm]", self.crtc_end_horizontal_retrace_norm);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalTotal, "", self.crtc_vertical_total);
        push_reg_str_bin8!(crtc_vec, CRTCRegister::Overflow, "", self.crtc_overflow);

        push_reg_str!(crtc_vec, CRTCRegister::PresetRowScan, "", self.crtc_preset_row_scan.preset_row_scan());
        push_reg_str!(crtc_vec, CRTCRegister::MaximumScanLine, "", self.crtc_maximum_scanline.maximum_scanline());
        push_reg_str!(crtc_vec, CRTCRegister::MaximumScanLine, "[2T4]", self.crtc_maximum_scanline.two_to_four());
        
        push_reg_str!(crtc_vec, CRTCRegister::CursorStartLine, "", self.crtc_cursor_start.cursor_start());
        push_reg_str!(crtc_vec, CRTCRegister::CursorEndLine, "", self.crtc_cursor_end.cursor_end());
        push_reg_str!(crtc_vec, CRTCRegister::StartAddressH, "", self.crtc_start_address_ho);
        push_reg_str!(crtc_vec, CRTCRegister::StartAddressL, "", self.crtc_start_address_lo);
        push_reg_str!(crtc_vec, CRTCRegister::CursorAddressH, "", self.crtc_cursor_address_ho);
        push_reg_str!(crtc_vec, CRTCRegister::CursorAddressL, "", self.crtc_cursor_address_lo);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalRetraceStart, "", self.crtc_vertical_retrace_start);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalRetraceEnd, "", self.crtc_vertical_retrace_end.vertical_retrace_end());
        push_reg_str!(crtc_vec, CRTCRegister::VerticalRetraceEnd, "[norm]", self.crtc_vertical_retrace_end_norm);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalDisplayEnd, "", self.crtc_vertical_display_end);
        push_reg_str!(crtc_vec, CRTCRegister::Offset, "", self.crtc_offset);
        push_reg_str!(crtc_vec, CRTCRegister::UnderlineLocation, "[ul]", self.crtc_underline_location.underline_location());
        push_reg_str!(crtc_vec, CRTCRegister::UnderlineLocation, "[cb4]", self.crtc_underline_location.count_by_four());
        push_reg_str!(crtc_vec, CRTCRegister::UnderlineLocation, "[dw]", self.crtc_underline_location.double_word_mode());
        push_reg_str!(crtc_vec, CRTCRegister::StartVerticalBlank, "",  self.crtc_start_vertical_blank);

        push_reg_str!(crtc_vec, CRTCRegister::EndVerticalBlank, "", self.crtc_end_vertical_blank);
        push_reg_str!(crtc_vec, CRTCRegister::EndVerticalBlank, "[norm]", self.crtc_end_vertical_blank_norm);
        push_reg_str_enum!(crtc_vec, CRTCRegister::ModeControl, "[cms]", self.crtc_mode_control.compatibility_mode());
        push_reg_str_enum!(crtc_vec, CRTCRegister::ModeControl, "[srs]", self.crtc_mode_control.select_row_scan_counter());
        push_reg_str_enum!(crtc_vec, CRTCRegister::ModeControl, "[hrs]", self.crtc_mode_control.horizontal_retrace_select());
        push_reg_str!(crtc_vec, CRTCRegister::ModeControl, "[cbr]", self.crtc_mode_control.count_by_two());
        push_reg_str!(crtc_vec, CRTCRegister::LineCompare, "", self.crtc_line_compare);

        map.insert("CRTC".to_string(), crtc_vec);

        /*

        crtc_vec.push((format!("{:?}", CRTCRegister::EndVerticalBlank), 
            VideoCardStateEntry::String(format!("{}", self.crtc_end_vertical_blank))));
        crtc_vec.push((format!("{:?} [norm]", CRTCRegister::EndVerticalBlank), 
            VideoCardStateEntry::String(format!("{}", self.crtc_end_vertical_blank_norm))));

        crtc_vec.push((format!("{:?} [cms]", CRTCRegister::ModeControl), 
            VideoCardStateEntry::String(format!("{:?}", self.crtc_mode_control.compatibility_mode()))));
        crtc_vec.push((format!("{:?} [srs]", CRTCRegister::ModeControl), 
            VideoCardStateEntry::String(format!("{:?}", self.crtc_mode_control.select_row_scan_counter()))));
        crtc_vec.push((format!("{:?} [hrs]", CRTCRegister::ModeControl), 
            VideoCardStateEntry::String(format!("{:?}", self.crtc_mode_control.horizontal_retrace_select()))));
        crtc_vec.push((format!("{:?} [cbt]", CRTCRegister::ModeControl), 
            VideoCardStateEntry::String(format!("{:?}", self.crtc_mode_control.count_by_two()))));          

        crtc_vec.push((format!("{:?}", CRTCRegister::LineCompare), 
            VideoCardStateEntry::String(format!("{}", self.crtc_line_compare))));
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
        sequencer_vec.push((format!("{:?} [s4]", SequencerRegister::ClockingMode), 
            format!("{:?}", self.sequencer_clocking_mode.shift_four())));
        sequencer_vec.push((format!("{:?} [so]", SequencerRegister::ClockingMode), 
            format!("{:?}", self.sequencer_clocking_mode.screen_off())));

        sequencer_vec.push((format!("{:?}", SequencerRegister::MapMask), format!("{:04b}", self.sequencer_map_mask)));
        sequencer_vec.push((format!("{:?} [a]", SequencerRegister::CharacterMapSelect), format!("{}", self.sequencer_character_map_a)));
        sequencer_vec.push((format!("{:?} [b]", SequencerRegister::CharacterMapSelect), format!("{}", self.sequencer_character_map_b)));
        sequencer_vec.push((format!("{:?} [em]", SequencerRegister::MemoryMode), format!("{}", self.sequencer_memory_mode.extended_memory())));
        sequencer_vec.push((format!("{:?} [o/e]", SequencerRegister::MemoryMode), format!("{}", self.sequencer_memory_mode.odd_even_enable())));
        sequencer_vec.push((format!("{:?} [c4]", SequencerRegister::MemoryMode), format!("{}", self.sequencer_memory_mode.chain4_enable())));
        
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
        attribute_vec.push((format!("{:?} [45]", AttributeRegister::ColorSelect), 
            format!("{}", self.attribute_color_select.c45())));
        attribute_vec.push((format!("{:?} [67]", AttributeRegister::ColorSelect), 
            format!("{}", self.attribute_color_select.c67())));            

        //attribute_overscan_color: AOverscanColor::new(),
        //attribute_color_plane_enable: AColorPlaneEnable::new(),
        map.insert("Attribute".to_string(), attribute_vec);
        */

        let mut attribute_pal_vec = Vec::new();
        for i in 0..16 {
            attribute_pal_vec.push((
                format!("Palette register {}", i), 
                VideoCardStateEntry::Color(
                    format!("{:06b}", self.attribute_palette_registers[i]),
                    self.color_registers_rgba[i][0],
                    self.color_registers_rgba[i][1],
                    self.color_registers_rgba[i][2],
                )
            ));
        }
        map.insert("AttributePalette".to_string(), attribute_pal_vec);

        let mut dac_pal_vec = Vec::new();
        for i in 0..256 {
            dac_pal_vec.push((
                format!("{}", i), 
                VideoCardStateEntry::Color(
                    format!("#{:02x}{:02x}{:02x}",                    
                        self.color_registers_rgba[i][0],
                        self.color_registers_rgba[i][1],
                        self.color_registers_rgba[i][2],
                    ),
                    self.color_registers_rgba[i][0],
                    self.color_registers_rgba[i][1],
                    self.color_registers_rgba[i][2],
                )
            ));
        }
        map.insert("DACPalette".to_string(), dac_pal_vec);

        map
    }

    fn run(&mut self, time: DeviceRunTimeUnit) {

        let elapsed_us = if let DeviceRunTimeUnit::Microseconds(us) = time {
            us
        }
        else {
            panic!("VGA requires us time unit");
        };

        //let vga_cycles = match self.misc_output_register.clock_select() {
        //    ClockSelect::Clock25 => elapsed_us / US_PER_CLOCK_1,
        //    ClockSelect::Clock28 => elapsed_us / US_PER_CLOCK_2,
        //    _ => elapsed_us / US_PER_CLOCK_1
        //};

        let vga_cycles = match (self.misc_output_register.clock_select(), self.sequencer_clocking_mode.dot_clock()) {
            (ClockSelect::Clock25, DotClock::Native) => elapsed_us / US_PER_CLOCK_1,
            (ClockSelect::Clock25, DotClock::HalfClock) => elapsed_us / (US_PER_CLOCK_2 * 2.0), // hack for BIOS
            (ClockSelect::Clock28, DotClock::Native) => elapsed_us / US_PER_CLOCK_2,
            (ClockSelect::Clock28, DotClock::HalfClock) => elapsed_us / (US_PER_CLOCK_2 * 2.0),
            _ => elapsed_us / US_PER_CLOCK_1
        };

        self.vga_cycle_accumulator += vga_cycles;

        while self.vga_cycle_accumulator > 1.0 {
            self.tick();
            self.vga_cycle_accumulator -= 1.0;
        }
    }

    /*
    fn run(&mut self, cpu_cycles: u32) {

        let elapsed_us = cpu_cycles as f32 * (1.0 / 4.77272666);

        //let vga_cycles = match self.misc_output_register.clock_select() {
        //    ClockSelect::Clock25 => elapsed_us / US_PER_CLOCK_1,
        //    ClockSelect::Clock28 => elapsed_us / US_PER_CLOCK_2,
        //    _ => elapsed_us / US_PER_CLOCK_1
        //};

        let vga_cycles = match (self.misc_output_register.clock_select(), self.sequencer_clocking_mode.dot_clock()) {
            (ClockSelect::Clock25, DotClock::Native) => elapsed_us / US_PER_CLOCK_1,
            (ClockSelect::Clock25, DotClock::HalfClock) => elapsed_us / (US_PER_CLOCK_2 * 2.0), // hack for BIOS
            (ClockSelect::Clock28, DotClock::Native) => elapsed_us / US_PER_CLOCK_2,
            (ClockSelect::Clock28, DotClock::HalfClock) => elapsed_us / (US_PER_CLOCK_2 * 2.0),
            _ => elapsed_us / US_PER_CLOCK_1
        };

        self.vga_cycle_accumulator += vga_cycles;

        while self.vga_cycle_accumulator > 1.0 {
            self.tick();
            self.vga_cycle_accumulator -= 1.0;
        }

    } 
    */   

    fn reset(&mut self) {
        self.reset_private();
    }

    fn get_pixel(&self, x: u32, y:u32) -> &[u8] {

        let pixel_byte = self.get_pixel_raw(x, y);

        return &self.color_registers_rgba[pixel_byte as usize];
    }

    fn get_pixel_raw(&self, x: u32, y :u32) -> u8 {
        
        let mut byte = 0;

        if self.sequencer_memory_mode.chain4_enable() {
            // Chain4 mode
            let x_byte_offset = x + self.attribute_pel_panning as u32;

            let span = self.crtc_offset as u32 * 2;
            let y_offset = y * span;
            

            let byte_select = (x_byte_offset + self.crtc_start_address as u32) >> 2 as usize;
            let plane_select = ((x_byte_offset + self.crtc_start_address as u32) & 0x03) as usize;
            
            let read_offset = (y_offset + byte_select) as usize;
            // LO 2 bits selects plane
            

            let byte = self.planes[plane_select].buf[read_offset];
            return byte;
        }
        else {

            let x_byte_offset = (x + self.attribute_pel_panning as u32) / 8;
            let x_bit_offset = (x + self.attribute_pel_panning as u32) % 8;

            // Get the current width of screen + offset
            //let span = (self.crtc_horizontal_display_end + 1 + 64) as u32;
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
                
                    byte |= read_bit << i;
                }
                // return self.attribute_palette_registers[byte & 0x0F].into_bytes()[0];


                if x == 0 && y == 0 {
                    // break me
        
                    //log::trace!("pixel (0,0): byte: {:01X}, palette: {:04X}", byte, self.attribute_palette_registers[byte & 0x0F]);
                }

                return self.attribute_palette_registers[byte & 0x0F];
            }
        }
        0
    }

    fn get_plane_slice(&self, plane: usize) -> &[u8] {

        &self.planes[plane].buf
    }

    fn dump_mem(&self, path: &Path) {
        
        for i in 0..4 {

            let mut filename = path.to_path_buf();
            filename.push(format!("vga_plane{}.bin", i));
            
            match std::fs::write(filename.clone(), &self.planes[i].buf) {
                Ok(_) => {
                    log::debug!("Wrote memory dump: {}", &filename.display())
                }
                Err(e) => {
                    log::error!("Failed to write memory dump '{}': {}", &filename.display(), e)
                }
            }
        }
    }

    fn get_frame_count(&self) -> u64 {
        0
    }

    fn write_trace_log(&mut self, msg: String) {
        self.trace_logger.print(msg);
    }

    fn trace_flush(&mut self) {
        self.trace_logger.flush();
    }

}

impl MemoryMappedDevice for VGACard {

    fn get_read_wait(&mut self, _address: usize, _cycles: u32) -> u32 {
        0
    }

    fn get_write_wait(&mut self, _address: usize, _cycles: u32) -> u32 {
        0
    }

    fn read_u8(&mut self, address: usize, _cycles: u32) -> (u8, u32) {

        // RAM Enable disables memory mapped IO
        if !self.misc_output_register.enable_ram() {
            return (0, 0);
        }

        // Validate address is within current memory map and get the offset into VRAM
        let offset = match self.plane_bounds_check(address) {
            Some(offset) => offset,
            None => {
                trace!(self, "Read out of range: Failed to set latches for address: {:05X}", address);
                log::warn!("Failed to set latches for address: {:05X}", address);

                for i in 0..4 {
                    self.planes[i].latch = 0xFF;
                }
                return (0xFF, 0);
            }
        };

        // Load all the latches regardless of selected plane or read mode
        self.latch_addr = address as u32;
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

                trace!(self, "READ ({:?}) [{:05X}]: BYTE:{:02X} L:[{:02X},{:02X},{:02X},{:02X}]",
                    self.graphics_mode.read_mode() as u8,
                    address,
                    //plane,
                    byte,
                    self.planes[0].latch,
                    self.planes[1].latch,
                    self.planes[2].latch,
                    self.planes[3].latch
                );
                return (byte, 0);
            }
            ReadMode::ReadComparedPlanes => {
                // In Read Mode 1, the processor reads the result of a comparison with the value in the 
                // Color Compare register, from the set of enabled planes in the Color Dont Care register
                self.get_pixels(offset);
                let comparison = self.pixel_op_compare();

                trace!(self, "READ {:?} [{:05X}]: BYTE:{:02X} L:[{:02X},{:02X},{:02X},{:02X}]",
                    self.graphics_mode.read_mode() as u8,
                    address,
                    comparison,
                    self.planes[0].latch,
                    self.planes[1].latch,
                    self.planes[2].latch,
                    self.planes[3].latch
                );                
                return (comparison, 0);
            }
        }
        (0, 0)
    }

    fn read_u16(&mut self, address: usize, _cycles: u32) -> (u16, u32) {

        let (lo_byte, wait1) = MemoryMappedDevice::read_u8(self, address, 0);
        let (ho_byte, wait2) = MemoryMappedDevice::read_u8(self, address + 1, 0);

        log::warn!("Unsupported 16 bit read from VRAM");
        ((ho_byte as u16) << 8 | lo_byte as u16, wait1 + wait2)
    }

    fn write_u8(&mut self, address: usize, byte: u8, _cycles: u32) -> u32 {

        // RAM Enable disables memory mapped IO
        if !self.misc_output_register.enable_ram() {
            return 0;
        }

        // Validate address is within current memory map and get the offset
        let mut offset = match self.plane_bounds_check(address) {
            Some(offset) => offset,
            None => {
                return 0;
            }
        };

        let mut c4_plane_select = 0;
        // In chain4 mode, the first two bits of the memory address select the plane to read/write
        if self.sequencer_memory_mode.chain4_enable() {
            c4_plane_select = offset & 0x03;
            offset >>= 2;
        }
        
        match self.graphics_mode.write_mode() {
            WriteMode::Mode0 => {

                // Write mode 0 performs a pipeline of operations:
                // First, data is rotated as specified by the Rotate Count field of the Data Rotate Register.
                let data_rot = VGACard::rotate_right_u8(byte, self.graphics_data_rotate.count());



                for i in 0..4 {
                    // Second, data is is either passed through to the next stage or replaced by a value determined
                    // by the Set/Reset register. The bits in the Enable Set/Reset register controls whether this occurs.
                    if (self.graphics_enable_set_reset & (0x01 << i)) != 0 {
                        // If the Set/Reset Enable bit is set, use expansion of corresponding Set/Reset register bit
                        self.pipeline_buf[i] = match (self.graphics_set_reset & (0x01 << i)) != 0 {
                            true  => 0xFF,
                            false => 0x00
                        }                        
                    }
                    else {
                        // Set/Reset Enable bit not set, use data from rotate step
                        self.pipeline_buf[i] = data_rot
                    }

                    // Third, the operation specified by the Logical Operation field of the Data Rotate register
                    // is perfomed on the data for each plane and the latch read register.
                    // Only the bits set in the Bit Mask register will be affected by the Logical Operation. 

                    let rotate_function = self.graphics_data_rotate.function();
                    
                    self.pipeline_buf[i] = match rotate_function {
                        RotateFunction::Unmodified => {
                            (self.pipeline_buf[i] & self.graphics_bitmask) | (self.planes[i].latch & !self.graphics_bitmask)
                        }
                        RotateFunction::And => {
                            ((self.pipeline_buf[i] & self.planes[i].latch) & self.graphics_bitmask) | (self.planes[i].latch & !self.graphics_bitmask)
                        }
                        RotateFunction::Or => {
                            ((self.pipeline_buf[i] | self.planes[i].latch) & self.graphics_bitmask) | (self.planes[i].latch & !self.graphics_bitmask)
                        }
                        RotateFunction::Xor => {
                            ((self.pipeline_buf[i] ^ self.planes[i].latch) & self.graphics_bitmask) | (self.planes[i].latch & !self.graphics_bitmask)
                        }
                    };

              
                }

                trace!(self, "WRITE(0) [{:05X}]: BYTE:{:02x} L:[{:02x},{:02x},{:02x},{:02x}] W:[{:02x},{:02x},{:02x},{:02x}] ROT:{:01X} ROP:{:01X} MM:{:01X} LA:[{:05X}]", 
                    address,
                    byte,                    
                    self.planes[0].latch,
                    self.planes[1].latch,
                    self.planes[2].latch,
                    self.planes[3].latch,
                    self.pipeline_buf[0],
                    self.pipeline_buf[1],
                    self.pipeline_buf[2],
                    self.pipeline_buf[3],                    
                    self.graphics_data_rotate.count(),
                    self.graphics_data_rotate.function() as u8,
                    self.sequencer_map_mask,
                    self.latch_addr
                );

                /*
                trace!(self, "wm0: [{:05X}] func: {:?} esr: {:01X} gsr: {:01X} mask: {:02X} writing: {:02x},{:02x},{:02x},{:02x} map_mask: {:02X}",
                    address,
                    self.graphics_data_rotate.function(),
                    self.graphics_enable_set_reset,
                    self.graphics_set_reset,
                    self.graphics_bitmask,
                    self.pipeline_buf[0],
                    self.pipeline_buf[1],
                    self.pipeline_buf[2],
                    self.pipeline_buf[3],
                    self.sequencer_map_mask
                );
                */

                if self.sequencer_memory_mode.chain4_enable() {
                    // (Chain4 mode...)
                    // Write the data to the plane enabled by address lines 0-1.
                    self.planes[c4_plane_select].buf[offset] = self.pipeline_buf[c4_plane_select];
                }
                else {
                    // (Not Chain4 mode...)
                    // Finally, write data to the planes enabled in the Memory Plane Write Enable field of
                    // the Sequencer Map Mask register.
                    for i in 0..4 {
                        if self.sequencer_map_mask & (0x01 << i) != 0 {
                            self.planes[i].buf[offset] = self.pipeline_buf[i];
                        }
                    }
                }                
            }
            WriteMode::Mode1 => {
                // Write the contents of the latches to their corresponding planes. This assumes that the latches
                // were loaded propery via a previous read operation.

                trace!(self, "WRITE(1) [{:05X}]: BYTE:XX L:[{:02x},{:02x},{:02x},{:02x}] MM:{:01X} LA:[{:05X}]", 
                    address,
                    self.planes[0].latch,
                    self.planes[1].latch,
                    self.planes[2].latch,
                    self.planes[3].latch,
                    self.sequencer_map_mask,
                    self.latch_addr
                );

                for i in 0..4 {
                    // Only write to planes enabled in the Sequencer Map Mask.
                    if (self.sequencer_map_mask & (0x01 << i)) != 0 {
                        self.planes[i].buf[offset] = self.planes[i].latch;
                    }
                }
            }
            WriteMode::Mode2 => {

                

                for i in 0..4 {
                    // Only write to planes enabled in the Sequencer Map Mask.
                    if self.sequencer_map_mask & (0x01 << i) != 0 {

                        // Extend the bit for this plane to 8 bits.
                        let bit_span: u8 = match (byte & (0x01 << i)) != 0 {
                            true => 0xFF,
                            false => 0x00,
                        };

                        //self.planes[i].buf[offset] = (self.planes[i].buf[offset] & !self.graphics_bitmask) | (bit_span & self.graphics_bitmask);
                        self.pipeline_buf[i] = bit_span;

                        self.pipeline_buf[i] = match self.graphics_data_rotate.function() {
                            RotateFunction::Unmodified => {
                                (self.pipeline_buf[i] & self.graphics_bitmask) | (self.planes[i].latch & !self.graphics_bitmask)
                            }
                            RotateFunction::And => {
                                ((self.pipeline_buf[i] & self.planes[i].latch) & self.graphics_bitmask) | (self.planes[i].latch & !self.graphics_bitmask)
                            }
                            RotateFunction::Or => {
                                ((self.pipeline_buf[i] | self.planes[i].latch) & self.graphics_bitmask) | (self.planes[i].latch & !self.graphics_bitmask)
                            }
                            RotateFunction::Xor => {
                                ((self.pipeline_buf[i] ^ self.planes[i].latch) & self.graphics_bitmask) | (self.planes[i].latch & !self.graphics_bitmask)
                            }
                        };

                        self.planes[i].buf[offset] = self.pipeline_buf[i];
                    }
                }

                trace!(self, "WRITE(2) [{:05X}] BYTE: {:02X} L:[{:02x},{:02x},{:02x},{:02x}] W:[{:02x},{:02x},{:02x},{:02x}] ROP:{:01X} BM:{:02X} MM:{:01X} LA:[{:05X}]",
                    address,
                    byte,
                    self.planes[0].latch,
                    self.planes[1].latch,
                    self.planes[2].latch,
                    self.planes[3].latch,
                    self.pipeline_buf[0],
                    self.pipeline_buf[1],
                    self.pipeline_buf[2],
                    self.pipeline_buf[3], 
                    self.graphics_data_rotate.function() as u8,               
                    self.graphics_bitmask,
                    self.sequencer_map_mask,
                    self.latch_addr
                );
            }
            WriteMode::Mode3 => {

                trace!(self, "WRITE(3) [{:05X}] BYTE: {:02X}",
                    address,
                    byte
                );

                // First, data is rotated as specified by the Rotate Count field of the Data Rotate Register.
                let data_rot = VGACard::rotate_right_u8(byte, self.graphics_data_rotate.count());
                
                // It is then AND'ed with with the Bit Mask Register to form an 8 bit mask...
                let mask = data_rot & self.graphics_bitmask;

                for i in 0..4 {
                    // Select bits all ON or OFF depending on the corresponding value of the set/reset register
                    let all_bits = match self.graphics_set_reset & (0x01 << i) != 0 {
                        true => 0xFF,
                        false => 0x00
                    };

                    // Use the mask calculated earlier to mask the identical bits from the set/reset register
                    self.planes[i].buf[offset] = (self.planes[i].buf[offset] & !mask) | (all_bits & mask);
                }

            }
        }

        0
    }

    fn write_u16(&mut self, address: usize, data: u16, _cycles: u32) -> u32 {
        trace!(self, "16 byte write to VRAM, {:04X} -> {:05X} ", data, address);
        log::warn!("Unsupported 16 bit write to VRAM");
        0
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotate() {
        let data_rot = VGACard::rotate_right_u8(0xFF, 7);

        assert_eq!(data_rot, 0xFF);

        let data_rot = VGACard::rotate_right_u8(0x80, 7);

        assert_eq!(data_rot, 0x01);

        let data_rot = VGACard::rotate_right_u8(0x01, 1);

        assert_eq!(data_rot, 0x80);
    }

    #[test]
    fn test_color_compare() {
        /*
        let mut ega = VGACard::new();

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
        */

    }
}
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

    ega::mod.rs

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

use modular_bitfield::prelude::*;

//#![allow(dead_code)]
use log;

use crate::tracelogger::TraceLogger;

use crate::device_traits::videocard::*;

mod attribute_controller;
mod crtc;
mod draw;
mod graphics_controller;
mod io;
mod mmio;
mod planes;
mod sequencer;
mod tablegen;
mod videocard;
mod vram;

use attribute_controller::*;

use crate::devices::ega::crtc::{EgaCrtc, WordOrByteMode};

use crate::devices::{
    dipswitch::{DipSwitch, DipSwitchSize},
    pic::Pic,
};
use graphics_controller::*;
use sequencer::*;
use tablegen::*;

static DUMMY_PIXEL: [u8; 4] = [0, 0, 0, 0];

pub const EGA_CLOCK0: f64 = 14.13131318;
pub const EGA_CLOCK1: f64 = 16.257;

pub const CGA_MEM_ADDRESS: usize = 0xB8000;
pub const CGA_MEM_WINDOW: usize = 0x08000;
pub const CGA_MEM_END: usize = CGA_MEM_ADDRESS + CGA_MEM_WINDOW - 1;
pub const EGA_MEM_ADDRESS: usize = 0xA0000;

pub const EGA_MEM_WINDOW_64: usize = 0x10000;
pub const EGA_MEM_WINDOW_128: usize = 0x20000;
pub const EGA_MEM_END_64: usize = EGA_MEM_ADDRESS + EGA_MEM_WINDOW_64 - 1;
pub const EGA_MEM_END_128: usize = EGA_MEM_ADDRESS + EGA_MEM_WINDOW_128 - 1;

// pub const CGA_MEM_SIZE: usize = 16384;
pub const EGA_TEXT_PLANE_SIZE: usize = 16384;
pub const EGA_GFX_PLANE_SIZE: usize = 65536;

// For an EGA card connected to an EGA monitor
// See http://www.minuszerodegrees.net/ibm_ega/ibm_ega_switch_settings.htm
// This is inverted (Checkit will report 0110)
// This is the only value that gives high-resolution text 640x350

pub const EGA_DIP_SWITCH_EGA: u8 = 0b1001; // EGA 'enhanced color'
pub const EGA_DIP_SWITCH_MDA: u8 = 0b1011; // MDA emulation
pub const EGA_DIP_SWITCH_NORMAL: u8 = 0b1000; // EGA 'normal color'
pub const EGA_DIP_SWITCH_CGA: u8 = 0b0111; // EGA on CGA monitor

pub const DEFAULT_DIP_SWITCH: u8 = EGA_DIP_SWITCH_EGA;

// Maximum height of an EGA character.
const EGA_CHARACTER_HEIGHT: usize = 32;
// Maximum height of cursor. Equal to maximum height of a character.
const EGA_CURSOR_MAX: usize = EGA_CHARACTER_HEIGHT as usize;

// Toggle cursor blink state after this many frames
const EGA_CURSOR_BLINK_RATE: u32 = 8;

// EGA display field can be calculated via the maximum programmed value in
// H0 of 91. 91+2*8 = 744.  VerticalTotal 364   744x364 = 270816 * 60Hz = 16,248,960

const EGA14_MAX_RASTER_X: u32 = 912;
const EGA14_MAX_RASTER_Y: u32 = 262;
const EGA16_MAX_RASTER_X: u32 = 744; // Maximum scanline width
const EGA16_MAX_RASTER_Y: u32 = 364; // Maximum scanline height

const EGA_MAX_CLOCK14: usize = 912 * 262; // Maximum frame clock for EGA 14Mhz clock (912x262) same as CGA
const EGA_MAX_CLOCK16: usize = 270816; // Maximum frame clock for EGA 16Mhz clock (744x364)
const EGA_MONITOR_VSYNC_MIN: u32 = 0;

// Negative offset to use for CRTC, Feature Control and ISR1 when in Monochrome
// compatibility mode (as controlled by bit 0 in the Miscellaneous Output Register)
const MDA_COMPAT_IO_ADJUST: u16 = 0x20;

/* The attribute address register is multiplexed with the attribute data register
   at the same address. An internal flip-flop controls whether the port reads in
   a register address or data value.
   The flip-flop should be initialized to a known value before any operation.
   The flip-flop can be cleared by reading from Input Status Register 1 (0x3DA)
*/
pub const ATTRIBUTE_REGISTER: u16 = 0x3C0;
/* Incomplete address decoding for the Attribute Register means it can also be
   accessed at 0x3C1. The EGA BIOS requires emulating this behavior.
   See: https://www.vogons.org/viewtopic.php?f=9&t=82050&start=60
*/
pub const ATTRIBUTE_REGISTER_ALT: u16 = 0x3C1;
//ub const ATTRIBUTE_ADDRESS_REGISTER: u16   = 0x3C0;
//pub const ATTRIBUTE_DATA_REGISTER: u16      = 0x3C0;

pub const MISC_OUTPUT_REGISTER: u16 = 0x3C2; // Write-only to 3C2
pub const INPUT_STATUS_REGISTER_0: u16 = 0x3C2; // Read-only from 3C2
pub const INPUT_STATUS_REGISTER_1: u16 = 0x3DA;
pub const FEATURE_CONTROL_REGISTER: u16 = 0x3DA; // Write-only to 3DA
pub const INPUT_STATUS_REGISTER_1_MDA: u16 = 0x3BA; // Used in MDA compatibility mode

pub const SEQUENCER_ADDRESS_REGISTER: u16 = 0x3C4;
pub const SEQUENCER_DATA_REGISTER: u16 = 0x3C5;

pub const CRTC_REGISTER_ADDRESS: u16 = 0x3D4;
pub const CRTC_REGISTER: u16 = 0x3D5;
pub const CRTC_REGISTER_ADDRESS_MDA: u16 = 0x3B4; // Used in MDA compatibility mode
pub const CRTC_REGISTER_MDA: u16 = 0x3B5; // Used in MDA compatibility mode

//pub const CGA_MODE_CONTROL_REGISTER: u16  = 0x3D8;     // This register does not exist on the EGA
//pub const CGA_COLOR_CONTROL_REGISTER: u16 = 0x3D9;     // This register does not exist on the EGA

//pub const CGA_LIGHTPEN_REGISTER: u16      = 0x3DB;

pub const EGA_GRAPHICS_1_POSITION: u16 = 0x3CC;
pub const EGA_GRAPHICS_2_POSITION: u16 = 0x3CA;
pub const EGA_GRAPHICS_ADDRESS: u16 = 0x3CE;
pub const EGA_GRAPHICS_DATA: u16 = 0x3CF;

pub struct EGAFont {
    w:    u32,
    h:    u32,
    span: usize,
    data: &'static [u8],
}

pub enum EgaDefaultColor4Bpp {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    White = 7,
    BlackBright = 8,
    BlueBright = 9,
    GreenBright = 10,
    CyanBright = 11,
    RedBright = 12,
    MagentaBright = 13,
    Yellow = 14,
    WhiteBright = 15,
}

pub enum EgaDefaultColor6Bpp {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    White = 7,
    BlackBright = 0x38,
    BlueBright = 0x39,
    GreenBright = 0x3A,
    CyanBright = 0x3B,
    RedBright = 0x3C,
    MagentaBright = 0x3D,
    Yellow = 0x3E,
    WhiteBright = 0x3F,
}

const ALL_SET64: u64 = 0xFFFFFFFFFFFFFFFF;

const EGA_PALETTE: [u32; 64] = [
    0x000000, // 000 000
    0x0000AA, // 000 001
    0x00AA00, // 000 010
    0x00AAAA, // 000 011
    0xAA0000, // 000 100
    0xAA00AA, // 000 101
    0xAAAA00, // 000 110
    0xAAAAAA, // 000 111
    0x000055, // 001 000
    0x0000FF, // 001 001
    0x00AA55, // 001 010
    0x00AAFF, // 001 011
    0xAA0055, // 001 100
    0xAA00FF, // 001 101
    0xAAAA55, // 001 110
    0xAAAAFF, // 001 111
    0x005500, // 010 000
    0x0055AA, // 010 001
    0x00FF00, // 010 010
    0x00FFAA, // 010 011
    0xAA5500, // 010 100
    0xAA55AA, // 010 101
    0xAAFF00, // 010 110
    0xAAFFAA, // 010 111
    0x005555, // 011 000
    0x0055FF, // 011 001
    0x00FF55, // 011 010
    0x00FFFF, // 011 011
    0xAA5555, // 011 100
    0xAA55FF, // 011 101
    0xAAFF55, // 011 110
    0xAAFFFF, // 011 111
    0x550000, // 100 000
    0x5500AA, // 100 001
    0x55AA00, // 100 010
    0x55AAAA, // 100 011
    0xFF0000, // 100 100
    0xFF00AA, // 100 101
    0xFFAA00, // 100 110
    0xFFAAAA, // 100 111
    0x550055, // 101 000
    0x5500FF, // 101 001
    0x55AA55, // 101 010
    0x55AAFF, // 101 011
    0xFF0055, // 101 100
    0xFF00FF, // 101 101
    0xFFAA55, // 101 110
    0xFFAAFF, // 101 111
    0x555500, // 110 000
    0x5555AA, // 110 001
    0x55FF00, // 110 010
    0x55FFAA, // 110 011
    0xFF5500, // 110 100
    0xFF55AA, // 110 101
    0xFFFF00, // 110 110
    0xFFFFAA, // 110 111
    0x555555, // 111 000
    0x5555FF, // 111 001
    0x55FF55, // 111 010
    0x55FFFF, // 111 011
    0xFF5555, // 111 100
    0xFF55FF, // 111 101
    0xFFFF55, // 111 110
    0xFFFFFF, // 111 111
];

/*

// Solid color spans of 8 pixels.
// Used for drawing overscan fast with bytemuck
const EGA_COLORS_4BPP_U64: [u64; 16] = [
    0x0000000000000000,
    0x0101010101010101,
    0x0202020202020202,
    0x0303030303030303,
    0x0404040404040404,
    0x0505050505050505,
    0x0606060606060606,
    0x0707070707070707,
    0x0808080808080808,
    0x0909090909090909,
    0x0A0A0A0A0A0A0A0A,
    0x0B0B0B0B0B0B0B0B,
    0x0C0C0C0C0C0C0C0C,
    0x0D0D0D0D0D0D0D0D,
    0x0E0E0E0E0E0E0E0E,
    0x0F0F0F0F0F0F0F0F,
];
 */

const CGA_TO_EGA_U64: [u64; 16] = [
    0x0000000000000000,
    0x0101010101010101,
    0x0202020202020202,
    0x0303030303030303,
    0x0404040404040404,
    0x0505050505050505,
    0x1414141414141414,
    0x0707070707070707,
    0x3838383838383838,
    0x3939393939393939,
    0x3A3A3A3A3A3A3A3A,
    0x3B3B3B3B3B3B3B3B,
    0x3C3C3C3C3C3C3C3C,
    0x3D3D3D3D3D3D3D3D,
    0x3E3E3E3E3E3E3E3E,
    0x3F3F3F3F3F3F3F3F,
];

const CGA_TO_EGA_U8: [u8; 16] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x14, 0x07, 0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F,
];

const fn init_ega_6bpp_u64_colors() -> [u64; 64] {
    let mut colors: [u64; 64] = [0; 64];
    let mut i: usize = 0;
    while i < 64 {
        let mut b = 0;
        let mut color: u64 = 0;

        while b < 8 {
            color <<= 8;
            color |= i as u64;
            b += 1;
        }
        colors[i] = color;
        i += 1;
    }
    colors
}

const fn init_ega_4bpp_u64_colors() -> [u64; 64] {
    let mut colors: [u64; 64] = [0; 64];
    let mut i: usize = 0;

    while i < 64 {
        let mut b = 0;
        let mut color: u64 = 0;

        while b < 8 {
            color <<= 8;
            // Color mapping in 16 color mode uses bit #4 for intensity.
            // Bit #3 is 'mono graphics' whatever that is.
            color |= ((i & 0x07) as u64) | ((i & 0x10) >> 1) as u64;
            b += 1;
        }
        colors[i] = color;
        i += 1;
    }
    colors
}

const EGA_COLORS_6BPP_U64: [u64; 64] = init_ega_6bpp_u64_colors();
const EGA_COLORS_4BPP_U64: [u64; 64] = init_ega_4bpp_u64_colors();

// Display apertures for each EGA clock
// In 14Mhz mode, EGA apertures are similar to CGA apertures.
// In 16Mhz mode, there is no difference between NORMAL and FULL apertures.
// Apertures are listed in order:
// NORMAL, FULL, DEBUG

const VERT_ADJUST: u32 = 1;

const EGA14_APERTURE_CROPPED_W: u32 = 640;
const EGA14_APERTURE_CROPPED_H: u32 = 200;
const EGA14_APERTURE_CROPPED_X: u32 = 128;
const EGA14_APERTURE_CROPPED_Y: u32 = 32 + VERT_ADJUST;

const EGA14_APERTURE_ACCURATE_W: u32 = 704;
const EGA14_APERTURE_ACCURATE_H: u32 = 232;
const EGA14_APERTURE_ACCURATE_X: u32 = 96;
const EGA14_APERTURE_ACCURATE_Y: u32 = 16 + VERT_ADJUST;

const EGA14_APERTURE_FULL_W: u32 = 704;
const EGA14_APERTURE_FULL_H: u32 = 232;
const EGA14_APERTURE_FULL_X: u32 = 96;
const EGA14_APERTURE_FULL_Y: u32 = 16 + VERT_ADJUST;

const EGA16_APERTURE_CROPPED_W: u32 = 640;
const EGA16_APERTURE_CROPPED_H: u32 = 350;
const EGA16_APERTURE_CROPPED_X: u32 = 56;
const EGA16_APERTURE_CROPPED_Y: u32 = VERT_ADJUST;

const EGA16_APERTURE_FULL_W: u32 = 640 + 16 + 16;
const EGA16_APERTURE_FULL_H: u32 = 350;
const EGA16_APERTURE_FULL_X: u32 = 40;
const EGA16_APERTURE_FULL_Y: u32 = VERT_ADJUST;

const MDA_MAX_RASTER_X: u32 = 882;
const MDA_MAX_RASTER_Y: u32 = 369; // Actual value works out to 325,140 / 882 or 368.639

const MDA_APERTURE_CROPPED_W: u32 = 720;
const MDA_APERTURE_CROPPED_H: u32 = 350;
const MDA_APERTURE_CROPPED_X: u32 = 9;
const MDA_APERTURE_CROPPED_Y: u32 = 4;

const MDA_APERTURE_NORMAL_W: u32 = 738;
const MDA_APERTURE_NORMAL_H: u32 = 354;
const MDA_APERTURE_NORMAL_X: u32 = 0;
const MDA_APERTURE_NORMAL_Y: u32 = 0;

const MDA_APERTURE_FULL_W: u32 = 738;
const MDA_APERTURE_FULL_H: u32 = 354;
const MDA_APERTURE_FULL_X: u32 = 0;
const MDA_APERTURE_FULL_Y: u32 = 0;

const MDA_APERTURE_DEBUG_W: u32 = MDA_MAX_RASTER_X;
const MDA_APERTURE_DEBUG_H: u32 = MDA_MAX_RASTER_Y;
const MDA_APERTURE_DEBUG_X: u32 = 0;
const MDA_APERTURE_DEBUG_Y: u32 = 0;

const EGA_APERTURES: [[DisplayAperture; 4]; 3] = [
    [
        // 14Mhz CROPPED aperture
        DisplayAperture {
            w: EGA14_APERTURE_CROPPED_W,
            h: EGA14_APERTURE_CROPPED_H,
            x: EGA14_APERTURE_CROPPED_X,
            y: EGA14_APERTURE_CROPPED_Y,
            debug: false,
        },
        // 14Mhz ACCURATE aperture
        DisplayAperture {
            w: EGA14_APERTURE_ACCURATE_W,
            h: EGA14_APERTURE_ACCURATE_H,
            x: EGA14_APERTURE_ACCURATE_X,
            y: EGA14_APERTURE_ACCURATE_Y,
            debug: false,
        },
        // 14Mhz FULL aperture
        DisplayAperture {
            w: EGA14_APERTURE_FULL_W,
            h: EGA14_APERTURE_FULL_H,
            x: EGA14_APERTURE_FULL_X,
            y: EGA14_APERTURE_FULL_Y,
            debug: false,
        },
        // 14Mhz DEBUG aperture
        DisplayAperture {
            w: EGA14_MAX_RASTER_X,
            h: EGA14_MAX_RASTER_Y,
            x: 0,
            y: 0,
            debug: true,
        },
    ],
    [
        // 16Mhz CROPPED aperture
        DisplayAperture {
            w: EGA16_APERTURE_CROPPED_W,
            h: EGA16_APERTURE_CROPPED_H,
            x: EGA16_APERTURE_CROPPED_X,
            y: EGA16_APERTURE_CROPPED_Y,
            debug: false,
        },
        // 16Mhz ACCURATE aperture
        DisplayAperture {
            w: EGA16_APERTURE_CROPPED_W,
            h: EGA16_APERTURE_CROPPED_H,
            x: EGA16_APERTURE_CROPPED_X,
            y: EGA16_APERTURE_CROPPED_Y,
            debug: false,
        },
        // 16Mhz FULL aperture
        DisplayAperture {
            w: EGA16_APERTURE_FULL_W,
            h: EGA16_APERTURE_FULL_H,
            x: EGA16_APERTURE_FULL_X,
            y: EGA16_APERTURE_FULL_Y,
            debug: false,
        },
        // 16Mhz DEBUG aperture
        DisplayAperture {
            w: EGA16_MAX_RASTER_X,
            h: EGA16_MAX_RASTER_Y,
            x: 0,
            y: 2,
            debug: true,
        },
    ],
    [
        // 16Mhz MDA CROPPED aperture
        DisplayAperture {
            w: MDA_APERTURE_CROPPED_W,
            h: MDA_APERTURE_CROPPED_H,
            x: MDA_APERTURE_CROPPED_X,
            y: MDA_APERTURE_CROPPED_Y,
            debug: false,
        },
        // 16Mhz MDA ACCURATE aperture
        DisplayAperture {
            w: MDA_APERTURE_NORMAL_W,
            h: MDA_APERTURE_NORMAL_H,
            x: MDA_APERTURE_NORMAL_X,
            y: MDA_APERTURE_NORMAL_Y,
            debug: false,
        },
        // 16Mhz MDA FULL aperture
        DisplayAperture {
            w: MDA_APERTURE_FULL_W,
            h: MDA_APERTURE_FULL_H,
            x: MDA_APERTURE_FULL_X,
            y: MDA_APERTURE_FULL_Y,
            debug: false,
        },
        // 16Mhz MDA DEBUG aperture
        DisplayAperture {
            w: MDA_APERTURE_DEBUG_W,
            h: MDA_APERTURE_DEBUG_H,
            x: MDA_APERTURE_DEBUG_X,
            y: MDA_APERTURE_DEBUG_Y,
            debug: true,
        },
    ],
];

const EGA_APERTURE_DESCS: [DisplayApertureDesc; 4] = [
    DisplayApertureDesc {
        name: "Cropped",
        aper_enum: DisplayApertureType::Cropped,
    },
    DisplayApertureDesc {
        name: "Accurate",
        aper_enum: DisplayApertureType::Accurate,
    },
    DisplayApertureDesc {
        name: "Full",
        aper_enum: DisplayApertureType::Full,
    },
    DisplayApertureDesc {
        name: "Debug",
        aper_enum: DisplayApertureType::Debug,
    },
];

pub struct EGACard {
    debug: bool,
    debug_draw: bool,

    dip_sw: DipSwitch,

    ticks_accum: f64,
    clock_mode: ClockingMode,
    cycles: u64,
    trace_logger: TraceLogger,

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
    frame: u64,
    scanline_cycles: f32,
    frame_cycles: f32,
    cursor_frames: u32,

    raster_x: u32,
    raster_y: u32,

    cur_char:  u8, // Current character being drawn
    next_char: u8,
    cur_attr:  u8, // Current attribute byte being drawn
    next_attr: u8,
    cur_fg:    u8,   // Current glyph fg color
    cur_bg:    u8,   // Current glyph bg color
    cur_blink: bool, // Current glyph blink attribute

    blink_state: bool, // Blink state for cursor and 'blink' attribute

    cursor_status: bool,
    cursor_slowblink: bool,
    cursor_blink_rate: u32,

    cursor_attr: u8,

    crtc: EgaCrtc,
    vma: usize,
    sequencer: Sequencer,
    gc: GraphicsController,
    ac: AttributeController,

    pel_pan_latch: u8,

    current_font: u8,

    misc_output_register: EMiscellaneousOutputRegister,

    // Direct display buffer stuff
    back_buf: usize,
    front_buf: usize,
    extents: DisplayExtents,
    aperture: usize,
    //buf: Vec<Vec<u8>>,
    buf: [Box<[u8; EGA_MAX_CLOCK16]>; 2],
    rba: usize,

    // Debug colors
    hblank_color:  u8,
    vblank_color:  u8,
    disable_color: u8,

    // Stat counters
    hsync_ct: u64,
    vsync_ct: u64,

    intr: bool,
    last_intr: bool,

    feature_bits: u8,
}

#[bitfield]
#[derive(Copy, Clone)]
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
    vertical_retrace_polarity: RetracePolarity,
}

/// IO Address Select field of External Miscellaneous Register:
/// Bit 0
#[derive(Debug, BitfieldSpecifier)]
pub enum IoAddressSelect {
    CompatMonochrome,
    CompatCGA,
}

/// Clock Select field of External Miscellaneous Register:
/// Bits 2-3
#[derive(Debug, BitfieldSpecifier, PartialEq)]
pub enum ClockSelect {
    Clock14 = 0,
    Clock16 = 1,
    ExternalClock = 2,
    Unused = 3,
}

/// Odd/Even Page Select field of External Miscellaneous Register:
#[derive(Debug, BitfieldSpecifier)]
pub enum PageSelect {
    LowPage,
    HighPage,
}

#[derive(Debug, BitfieldSpecifier)]
pub enum RetracePolarity {
    Positive,
    Negative,
}

impl Default for EGACard {
    fn default() -> Self {
        Self {
            debug: false,
            debug_draw: true,

            dip_sw: DipSwitch::new(DipSwitchSize::Dip4, DEFAULT_DIP_SWITCH).with_invert_bits(false),

            ticks_accum: 0.0,
            clock_mode: ClockingMode::Cycle,
            cycles: 0,

            trace_logger: TraceLogger::None,

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
            frame: 0,
            scanline_cycles: 0.0,

            raster_x: 0,
            raster_y: 0,

            cur_char: 0,
            next_char: 0,
            cur_attr: 0,
            next_attr: 0,
            cur_fg: 0,
            cur_bg: 0,
            cur_blink: false,
            blink_state: false,

            cursor_status: true,
            cursor_slowblink: false,
            cursor_blink_rate: EGA_CURSOR_BLINK_RATE,
            cursor_attr: 0,

            crtc: EgaCrtc::new(),
            vma: 0,
            sequencer: Sequencer::new(),
            gc: GraphicsController::new(),
            ac: AttributeController::new(),

            pel_pan_latch: 0,

            current_font: 0,
            misc_output_register: EMiscellaneousOutputRegister::new(),

            back_buf:  1,
            front_buf: 0,
            extents:   EGACard::get_default_extents(),
            aperture:  0,

            // Theoretically, boxed arrays may have some performance advantages over
            // vectors due to having a fixed size known by the compiler.  However they
            // are a pain to initialize without overflowing the stack.
            buf: [
                vec![0; EGA_MAX_CLOCK16].into_boxed_slice().try_into().unwrap(),
                vec![0; EGA_MAX_CLOCK16].into_boxed_slice().try_into().unwrap(),
            ],
            rba: 0,

            hblank_color:  0,
            vblank_color:  0,
            disable_color: 0,

            hsync_ct: 0,
            vsync_ct: 0,

            intr: false,
            last_intr: false,

            feature_bits: 0,
        }
    }
}

/* Can't do this as somehow the CGA Default is still in scope here.

// EGA implementation of Default for DisplayExtents.
// Each videocard implementation should implement sensible defaults.
// In CGA's case we know the maximum field size and thus row_stride.
impl Default for DisplayExtents {
    fn default() -> Self {
        Self {
            apertures: EGA_APERTURES[1].to_vec(),
            field_w: EGA16_MAX_RASTER_X,
            field_h: EGA16_MAX_RASTER_Y,
            visible_w: 0,
            visible_h: 0,
            row_stride: EGA16_MAX_RASTER_X as usize,
            double_scan: false,
            mode_byte: 0,
        }
    }
}*/

impl EGACard {
    pub fn new(trace_logger: TraceLogger, clock_mode: ClockingMode, video_frame_debug: bool, dip: Option<u8>) -> Self {
        let mut ega = Self::default();

        // If a dip was provided, set the dip switches, otherwise leave them default.
        if let Some(dip) = dip {
            ega.dip_sw.set_physical_state(dip);
        }

        ega.trace_logger = trace_logger;
        ega.debug = video_frame_debug;
        //ega.debug_draw = video_frame_debug;
        ega.debug_draw = true;

        if let ClockingMode::Default = clock_mode {
            ega.clock_mode = ClockingMode::Character;
        }
        else {
            ega.clock_mode = clock_mode;
        }
        ega
    }

    fn get_default_extents() -> DisplayExtents {
        DisplayExtents {
            apertures: EGA_APERTURES[1].to_vec(),
            field_w: EGA16_MAX_RASTER_X,
            field_h: EGA16_MAX_RASTER_Y,
            row_stride: EGA16_MAX_RASTER_X as usize,
            double_scan: false,
            mode_byte: 0,
        }
    }

    /// Reset the EGA card.
    fn reset_private(&mut self) {
        let trace_logger = std::mem::replace(&mut self.trace_logger, TraceLogger::None);

        *self = Self {
            debug: self.debug,
            dip_sw: self.dip_sw,
            debug_draw: self.debug_draw,
            clock_mode: self.clock_mode,
            frame: self.frame,
            trace_logger,
            ..Self::default()
        };
    }

    fn get_cursor_span(&self) -> (u8, u8) {
        self.crtc.get_cursor_span()
    }

    fn get_cursor_address(&self) -> u32 {
        //(self.crtc_cursor_address_ho as u32) << 8 | self.crtc_cursor_address_lo as u32
        0
    }

    fn get_cursor_status(&self) -> bool {
        self.crtc.status.cursor
    }

    /// Handle a write to the External Miscellaneous Output Register, 0x3C2
    fn write_external_misc_output_register(&mut self, byte: u8) {
        let clock_old = self.misc_output_register.clock_select();
        self.misc_output_register = EMiscellaneousOutputRegister::from_bytes([byte]);

        if clock_old != self.misc_output_register.clock_select() {
            // Clock updated.
            self.sequencer.clock_change_pending = true;
            self.update_clock();
        }

        log::trace!(
            "Write to Misc Output Register: {:02X} Address Select: {:?} Clock Select: {:?}, Odd/Even Page bit: {:?}",
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
        let mut byte = 0x0F;

        // Note: DIP switches are wired up in reverse order
        let switch_status = match self.misc_output_register.clock_select() {
            ClockSelect::Unused => self.dip_sw.read() & 0x01,
            ClockSelect::ExternalClock => self.dip_sw.read() >> 1 & 0x01,
            ClockSelect::Clock16 => self.dip_sw.read() >> 2 & 0x01,
            ClockSelect::Clock14 => self.dip_sw.read() >> 3 & 0x01,
        };

        // Set switch sense bit
        byte |= switch_status << 4;

        // Set CRT interrupt bit. Bit is 0 when retrace is occurring.
        byte |= match self.crtc.status.vblank {
            true => 0,
            false => 0x80,
        };

        // Copy in feature bits
        byte |= self.feature_bits << 5;

        log::trace!("Read from Input Status Register 0: {:08b}", byte);
        byte
    }

    /// Handle a read from the Input Status Register One, 0x3DA
    ///
    /// Reading from this register also resets the Attribute Controller flip-flip
    fn read_input_status_register_1(&mut self) -> u8 {
        // Reset Address Register flip-flop
        // false == Address
        self.ac.reset_flipflop();

        let mut byte = 0;

        // Display Enable NOT bit is set to 1 if display is in vsync or hsync period
        // TODO: Some references specifically mention this as HBLANK or VBLANK,
        // but on the CGA is is actually not in active display area, which is different.
        // Which way is it really on the EGA?

        // The IBM EGA bios sets up a very wide border area during its HBLANK count test.
        // The implication there is that we can poll for !DEN not HBLANK.
        //if self.crtc_hblank || self.crtc_vblank {
        if !self.crtc.status.den {
            byte |= 0x01;
        }
        if self.crtc.status.vblank {
            byte |= 0x08;
        }

        // The EGA can feed two lines off the Attribute Controller's color outputs back
        // into the Input Status Register 1 bits 4 & 5. Which lines to feed back are
        // controlled by bits 4 & 5 of the Color Plane Enable Register Video Status
        // Mux Field.
        // The EGA BIOS performs a diagnostic that senses these line transitions after
        // drawing a line of high-intensity white characters to the screen.
        // Currently, we just fake this whole affair by setting the bits to be on during
        // the first few scanlines.

        if self.crtc.scanline() < 9 {
            byte |= 0x30;
        }

        byte
    }

    /// Calculate the current display mode based on the various register parameters of the EGA
    ///
    /// The EGA doesn't have a convenient mode register like the CGA to determine display mode.
    /// Instead, several fields are used to determine the current display mode.
    /// Not all cases are easily detectable - for example, CGA mode 6h is implemented as a standard EGA graphics mode.
    fn recalculate_mode(&mut self) {
        if self.crtc.maximum_scanline() > 7 {
            // Use 8x14 font
            self.current_font = 1;
        }
        else {
            self.current_font = 0;
        }

        match self.ac.mode() {
            AttributeMode::Text => {
                self.display_mode = match (self.crtc.horizontal_display_end(), self.ac.display_type()) {
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
                self.display_mode = match self.gc.memory_map() {
                    MemoryMap::B8000_32K => match self.gc.odd_even() {
                        OddEvenModeComplement::OddEven => DisplayMode::Mode4LowResGraphics,
                        OddEvenModeComplement::Sequential => DisplayMode::Mode6HiResGraphics,
                    },
                    MemoryMap::A0000_64K | MemoryMap::A0000_128k => {
                        match (self.crtc.horizontal_display_end(), self.ac.display_type()) {
                            (00..=39, AttributeDisplayType::Color) => DisplayMode::ModeDEGALowResGraphics,
                            (79, AttributeDisplayType::Color) => match self.crtc.vertical_display_end() {
                                0..=199 => DisplayMode::ModeEEGAMedResGraphics,
                                _ => DisplayMode::Mode10EGAHiResGraphics,
                            },
                            _ => {
                                log::warn!("Unsupported graphics mode.");
                                DisplayMode::Mode3TextCo80
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
    }

    /// Return the 4bpp pixel value from the graphics planes at the specified position
    fn get_pixel(&self, addr: usize, bit: u8) -> u8 {
        let mut bits = 0;

        bits |= self.sequencer.vram.read_u8(0, addr) >> (7 - bit) & 0x01;
        bits |= (self.sequencer.vram.read_u8(1, addr) >> (7 - bit) & 0x01) << 1;
        bits |= (self.sequencer.vram.read_u8(2, addr) >> (7 - bit) & 0x01) << 2;
        bits |= (self.sequencer.vram.read_u8(3, addr) >> (7 - bit) & 0x01) << 3;
        bits
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

    /// Tick the EGA device. This is much simpler than the implementation in the CGA device as
    /// we only support ticking by character clock.
    fn tick(&mut self, ticks: f64, pic: &mut Option<Pic>) {
        self.ticks_accum += ticks;

        // Drain the accumulator while emitting characters.
        // We have two character tick functions, for native and half clocks - the half clock function draws
        // doubles pixels horizontally. This can occur at either the 14Mhz or 16Mhz clock rate, so we pass
        // that clock rate to the tick function. This in turn is passed to the attribute controller for proper
        // palette selection during serialization.
        while self.ticks_accum > self.sequencer.char_clock as f64 {
            match self.sequencer.clocking_mode.dot_clock() {
                DotClock::Native => self.tick_hchar(self.misc_output_register.clock_select()),
                DotClock::HalfClock => self.tick_lchar(self.misc_output_register.clock_select()),
            }
            self.ticks_accum -= self.sequencer.char_clock as f64;

            if self.intr && !self.last_intr {
                // Rising edge of INTR - raise IRQ2
                if let Some(pic) = pic {
                    pic.request_interrupt(2);
                }
            }
            else if !self.intr {
                // Falling edge of INTR - release IRQ2
                if let Some(pic) = pic {
                    //log::debug!("clearing irq2!");
                    pic.clear_interrupt(2);
                }
            }
            self.last_intr = self.intr;
        }
    }

    fn tick_hchar(&mut self, clock_select: ClockSelect) {
        assert_eq!(self.cycles & 0x07, 0);
        assert_eq!(self.sequencer.char_clock, 8);

        self.cycles += 8;

        // Only draw if marty_render buffer address is in bounds.
        if self.rba < (EGA_MAX_CLOCK16 - 8) {
            // Shift the current character span out from the attribute controller and draw it
            let out_span = self.ac.shift_out64();
            self.draw_from_ac(out_span);

            if self.crtc.status.den | self.crtc.status.den_skew | self.crtc.status.hborder {}

            // Load attribute controller unless we are in blanking period
            //if self.crtc.status.den | self.crtc.in_skew() | self.crtc.status.hborder {
            if !self.crtc.in_blanking() {
                match self.ac.mode() {
                    AttributeMode::Text => {
                        self.ac.load(
                            AttributeInput::Parallel64(
                                self.sequencer
                                    .get_glyph_span(self.cur_char, self.current_font, self.crtc.vlc()),
                                //self.sequencer.test_glyph_span(self.crtc.vlc()),
                                self.cur_attr,
                                self.crtc.status.cursor,
                            ),
                            clock_select,
                            self.crtc.status.den | self.crtc.in_skew(),
                        );
                        //self.draw_text_mode_hchar14();
                    }
                    AttributeMode::Graphics => {
                        let ser = self.gc.serialize(&self.sequencer, self.vma);
                        self.ac.load(
                            AttributeInput::Serial(ser),
                            clock_select,
                            self.crtc.status.den | self.crtc.in_skew(),
                        );

                        //self.draw_gfx_mode_hchar_6bpp();
                    }
                }
            }
            /*            else if self.crtc.status.hborder {
                self.ac.load(
                    AttributeInput::SolidColor(EgaDefaultColor6Bpp::Green as u8),
                    clock_select,
                    self.crtc.status.den | self.crtc.in_skew(),
                );

                //self.draw_solid_hchar_6bpp(EgaDefaultColor6Bpp::Magenta as u8);
            }*/

            /*            if self.crtc.status.vborder | self.crtc.status.hborder {
                // High res modes on EGA do not support the overscan color
                //self.ac.shift_in(AttributeInput::Black, ClockSelect::Clock16);
                /*
                self.ac.shift_in(
                    AttributeInput::SolidColor(EgaDefaultColor6Bpp::Green as u8),
                    clock_select,
                );*/
                self.draw_solid_hchar_6bpp(EgaDefaultColor6Bpp::Black as u8);
            }*/

            if self.crtc.status.hsync {
                if self.debug_draw {
                    /*
                    self.ac.shift_in(
                        AttributeInput::SolidColor(EgaDefaultColor6Bpp::BlueBright as u8),
                        clock_select,
                    );*/
                    self.draw_solid_hchar_6bpp(EgaDefaultColor6Bpp::BlueBright as u8);
                }
            }
            else if self.crtc.status.hblank {
                if self.debug_draw {
                    /*
                    self.ac.shift_in(
                        AttributeInput::SolidColor(EgaDefaultColor6Bpp::Blue as u8),
                        clock_select,
                    );*/
                    self.draw_solid_hchar_6bpp(EgaDefaultColor6Bpp::Blue as u8);
                }
            }
            else if self.crtc.status.vblank {
                if self.debug_draw {
                    // Draw vblank in debug color
                    /*
                    self.ac.shift_in(
                        AttributeInput::SolidColor(EgaDefaultColor6Bpp::Magenta as u8),
                        clock_select,
                    );*/
                    self.draw_solid_hchar_6bpp(EgaDefaultColor6Bpp::Magenta as u8);
                }
            }
        }

        // Update position to next pixel and character column.
        self.raster_x += 8 * self.sequencer.clock_divisor as u32;
        self.rba += 8 * self.sequencer.clock_divisor as usize;

        // If we have reached the right edge of the 'monitor', return the raster position
        // to the left side of the screen.
        if self.raster_x >= self.extents.field_w {
            self.raster_x = 0;
            self.raster_y += 1;
            //self.in_monitor_hsync = false;
            self.rba = self.extents.row_stride * self.raster_y as usize;
        }

        if self.update_char_tick() && self.crtc.int_enabled() {
            self.intr = true;
        }
        else if !self.crtc.status.vsync {
            self.intr = false;
        }
    }

    fn tick_lchar(&mut self, clock_select: ClockSelect) {
        //assert_eq!(self.cycles & 0x0F, 0);
        assert_eq!(self.sequencer.char_clock, 16);

        self.cycles += 8;

        // Only draw if buffer address is in bounds.
        if self.rba < (EGA_MAX_CLOCK16 - 16) {
            let (out_span1, outspan2) = self.ac.shift_out64_halfclock();
            self.draw_from_ac_halfclock(out_span1, outspan2);

            if self.crtc.status.den | self.crtc.status.den_skew {}

            // Load attribute controller unless we are in blanking period
            if !self.crtc.in_blanking() {
                match self.ac.mode() {
                    AttributeMode::Text => {
                        self.ac.load(
                            AttributeInput::Parallel64(
                                self.sequencer
                                    .get_glyph_span(self.cur_char, self.current_font, self.crtc.vlc()),
                                //self.sequencer.test_glyph_span(self.crtc.vlc()),
                                self.cur_attr,
                                self.crtc.status.cursor,
                            ),
                            clock_select,
                            self.crtc.status.den | self.crtc.in_skew(),
                        );
                        //self.draw_text_mode_hchar14();
                    }
                    AttributeMode::Graphics => {
                        let ser = self.gc.serialize(&self.sequencer, self.vma);
                        self.ac.load(
                            AttributeInput::Serial(ser),
                            clock_select,
                            self.crtc.status.den | self.crtc.in_skew(),
                        );

                        //let out_span = self.get_gfx_mode_lchar_6pp();
                        //self.ac.shift_in(AttributeInput::Serial64(out_span));
                        //self.draw_gfx_mode_hchar_6bpp();
                    }
                }
            }

            if self.crtc.status.hborder {
                self.draw_overscan_lchar();
            }

            if self.debug_draw {
                if self.crtc.status.hsync {
                    self.draw_solid_lchar_6bpp(EgaDefaultColor6Bpp::BlueBright as u8);
                }
                else if self.crtc.status.hblank {
                    self.draw_solid_lchar_6bpp(EgaDefaultColor6Bpp::Blue as u8);
                }
                else if self.crtc.status.vsync {
                    self.draw_solid_lchar_6bpp(EgaDefaultColor6Bpp::Magenta as u8)
                }
            }
        }

        // Update position to next pixel and character column.
        self.raster_x += 8 * self.sequencer.clock_divisor as u32;
        self.rba += 8 * self.sequencer.clock_divisor as usize;

        if self.update_char_tick() && self.crtc.int_enabled() {
            self.intr = true;
        }
        else if !self.crtc.status.vsync {
            self.intr = false;
        }
    }

    pub fn update_char_tick(&mut self) -> bool {
        let mut did_vsync = false;
        self.vma = self.crtc.tick(self.get_clock_divisor()) as usize;
        if self.crtc.status.begin_vsync {
            self.do_vsync();
            did_vsync = true;
        }
        if self.crtc.status.begin_hsync {
            self.do_hsync();
        }
        self.fetch_char(self.vma as u16);
        did_vsync
    }

    fn do_hsync(&mut self) {
        self.hsync_ct += 1;
        self.scanline += 1;

        // Reset beam to left of screen if we haven't already
        if self.raster_x > 0 {
            self.raster_y += 1;
        }
        self.raster_x = 0;

        let new_rba = self.extents.row_stride * self.raster_y as usize;
        self.rba = new_rba;
    }

    /// Perform a (virtual) vsync. Our virtual raster position (rba) returns to the top of the
    /// display field and we swap the front and back buffer index.
    pub fn do_vsync(&mut self) {
        /*
        self.cycles_per_vsync = self.cur_screen_cycles;
        self.cur_screen_cycles = 0;
        self.last_vsync_cycles = self.cycles;

        if self.cycles_per_vsync > 300000 {
            log::warn!(
                "do_vsync(): Excessively long frame. char_clock: {} cycles: {} beam_y: {}",
                self.char_clock,
                self.cycles_per_vsync,
                self.beam_y
            );
        }
        */

        // Only do a vsync if we are past the minimum scanline #.
        // A monitor will refuse to vsync too quickly.
        if self.raster_y > EGA_MONITOR_VSYNC_MIN {
            // This note is copied from CGA, but may not be accurate from EGA:
            // vblank remains set through the entire last line, including the right overscan of the
            // new screen. So we need to delay resetting vblank flag until then.
            //self.in_crtc_vblank = false;

            self.vsync_ct += 1;
            self.raster_x = 0;
            self.raster_y = 0;
            self.rba = 0;

            //trace_regs!(self);
            //trace!(self, "Leaving vsync and flipping buffers");

            self.scanline = 0;
            self.frame += 1;

            // Swap the display buffers
            self.swap();

            // Toggle blink state. This is toggled every 8 frames by default.
            if (self.frame % EGA_CURSOR_BLINK_RATE as u64) == 0 {
                self.blink_state = !self.blink_state;
            }
        }
    }

    /// Fetch the character and attribute for the current character.
    /// This applies to text mode only, but is computed in all modes at appropriate times.
    fn fetch_char(&mut self, vma: u16) {
        let addr = vma as usize;

        self.cur_char = self.next_char;
        self.cur_attr = self.next_attr;

        if self.crtc.status.cref {
            self.next_char = self.sequencer.read_u8(0, addr, 0);
            match self.crtc.address_mode() {
                WordOrByteMode::Word => {
                    self.next_attr = self.sequencer.read_u8(1, addr + 1, 1);
                }
                WordOrByteMode::Byte => {
                    self.next_attr = self.sequencer.read_u8(1, addr, 0);
                }
            }
        }
        else {
            self.next_char = 0;
            self.next_attr = 0;
        }
    }

    /// Swaps the front and back buffers by exchanging indices.
    fn swap(&mut self) {
        //std::mem::swap(&mut self.back_buf, &mut self.front_buf);

        let tmp = self.back_buf;
        self.back_buf = self.front_buf;
        self.front_buf = tmp;
        self.buf[self.back_buf].fill(0);
    }

    fn update_clock(&mut self) {
        if self.sequencer.clock_change_pending {
            (self.sequencer.clock_divisor, self.sequencer.char_clock) = match self.sequencer.clocking_mode.dot_clock() {
                DotClock::HalfClock => (2, 16),
                DotClock::Native => (1, 8),
            };

            // Update display extents and aperture lists for new clock.
            match self.misc_output_register.clock_select() {
                ClockSelect::Clock14 => {
                    self.extents.field_w = EGA14_MAX_RASTER_X;
                    self.extents.field_h = EGA14_MAX_RASTER_Y;
                    self.extents.row_stride = EGA14_MAX_RASTER_X as usize;
                    self.extents.apertures = EGA_APERTURES[0].to_vec();
                    self.extents.double_scan = true;
                }
                ClockSelect::Clock16 => {
                    match self.sequencer.clocking_mode.character_clock() {
                        // Switch between native EGA (8 dots) and MDA compatibility (9 dots)
                        CharacterClock::EightDots => {
                            self.extents.field_w = EGA16_MAX_RASTER_X;
                            self.extents.field_h = EGA16_MAX_RASTER_Y;
                            self.extents.row_stride = EGA16_MAX_RASTER_X as usize;
                            self.extents.apertures = EGA_APERTURES[1].to_vec();
                            self.extents.double_scan = false;
                        }
                        CharacterClock::NineDots => {
                            self.extents.field_w = MDA_MAX_RASTER_X;
                            self.extents.field_h = MDA_MAX_RASTER_Y;
                            self.extents.row_stride = MDA_MAX_RASTER_X as usize;
                            self.extents.apertures = EGA_APERTURES[2].to_vec();
                            self.extents.double_scan = false;
                        }
                    }
                }
                _ => {
                    // Unsupported
                }
            }
        }

        log::debug!(
            "Updated EGA Clock, new extents: {}x{}",
            self.extents.field_w,
            self.extents.field_h,
        );
    }

    fn ega_to_rgb(egacolor: u8) -> (u8, u8, u8) {
        // EGA color components are 2 bits each
        let i = egacolor as usize;
        let r = (EGA_PALETTE[i] >> 16) as u8;
        let g = (EGA_PALETTE[i] >> 8) as u8;
        let b = EGA_PALETTE[i] as u8;

        (r, g, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_compare() {
        /*        let mut ega = EGACard::new(TraceLogger::None, ClockingMode::Character, false);

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
        assert_eq!(result, 0b00100111);*/
    }
}

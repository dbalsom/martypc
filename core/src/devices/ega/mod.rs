/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

#![allow(dead_code)]
use modular_bitfield::prelude::*;
use std::{collections::HashMap, path::Path};

//#![allow(dead_code)]
use log;

use crate::{
    bus::{BusInterface, DeviceRunTimeUnit},
    tracelogger::TraceLogger,
    videocard::*,
};

mod attribute_regs;
mod crtc;
mod crtc_regs;
mod draw;
mod graphics_regs;
mod io;
mod mmio;
mod planes;
mod sequencer_regs;
mod tablegen;
mod videocard;

use attribute_regs::*;

use crtc_regs::*;
use graphics_regs::*;
use sequencer_regs::*;
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
pub const EGA_DIP_SWITCH: u8 = 0b0000_1001;

const CGA_DEFAULT_CURSOR_BLINK_RATE: f64 = 0.0625;
const CGA_DEFAULT_CURSOR_FRAME_CYCLE: u32 = 8;

// Maximum height of an EGA character.
const EGA_CHARACTER_HEIGHT: u32 = 32;
// Maximum height of cursor. Equal to maximum height of a character.
const EGA_CURSOR_MAX: usize = EGA_CHARACTER_HEIGHT as usize;

// Toggle cursor blink state after this many frames
const EGA_CURSOR_BLINK_RATE: u32 = 8;

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

/*
const CGA_FRAME_CPU_TIME: u32 = 79648;
const CGA_VBLANK_START: u32 = 70314;
const CGA_SCANLINE_CPU_TIME: u32 = 304;
const CGA_HBLANK_START: u32 = 250;

const EGA_FRAME_CPU_TIME: u32 = 70150;
const EGA_VBLANK_START: u32 = 61928;
const EGA_SCANLINE_CPU_TIME: u32 = 267;
const EGA_HBLANK_START: u32 = 220;
*/

// EGA display field can be calculated via the maximum programmed value in
// H0 of 91. 91+2*8 = 744.  VerticalTotal 364   744x364 = 270816 * 60Hz = 16,248,960

const EGA14_MAX_RASTER_X: u32 = 912;
const EGA14_MAX_RASTER_Y: u32 = 262;
const EGA16_MAX_RASTER_X: u32 = 744; // Maximum scanline width
const EGA16_MAX_RASTER_Y: u32 = 364; // Maximum scanline height

const EGA_APERTURE_CROP_LEFT: u32 = 0;
const EGA_APERTURE_CROP_TOP: u32 = 0;
const EGA_MAX_CLOCK14: usize = 912 * 262; // Maximum frame clock for EGA 14Mhz clock (912x262) same as CGA
const EGA_MAX_CLOCK16: usize = 270816; // Maximum frame clock for EGA 16Mhz clock (744x364)
const EGA_MONITOR_VSYNC_MIN: u32 = 0;
const EGA_HCHAR_CLOCK: u8 = 8;

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
const CC_ALT_COLOR_MASK: u8 = 0b0000_0111;
const CC_ALT_INTENSITY: u8 = 0b0000_1000;
// Controls whether palette is high intensity
const CC_BRIGHT_BIT: u8 = 0b0001_0000;
// Controls primary palette between magenta/cyan and red/green
const CC_PALETTE_BIT: u8 = 0b0010_0000;

pub struct VideoTimings {
    cpu_frame:    u32,
    vblank_start: u32,
    cpu_scanline: u32,
    hblank_start: u32,
}

pub struct EGAFont {
    w:    u32,
    h:    u32,
    span: usize,
    data: &'static [u8],
}

const CGA_PALETTES: [[u8; 4]; 6] = [
    [0, 2, 4, 6],    // Red / Green / Brown
    [0, 10, 12, 14], // Red / Green / Brown High Intensity
    [0, 3, 5, 7],    // Cyan / Magenta / White
    [0, 11, 13, 15], // Cyan / Magenta / White High Intensity
    [0, 3, 4, 7],    // Red / Cyan / White
    [0, 11, 12, 15], // Red / Cyan / White High Intensity
];

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

const EGA_DEBUG_COLOR: u8 = EgaDefaultColor6Bpp::Magenta as u8;
const EGA_HBLANK_COLOR: u8 = 0;
const EGA_HBLANK_DEBUG_COLOR: u8 = 1;
const EGA_VBLANK_COLOR: u8 = 0;
const EGA_VBLANK_DEBUG_COLOR: u8 = 14;
const EGA_DISABLE_COLOR: u8 = 0;
const EGA_DISABLE_DEBUG_COLOR: u8 = 2;
const EGA_OVERSCAN_COLOR: u8 = 5;

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

// Solid color spans of 8 pixels.
// Used for drawing debug info into index buffer.
const EGA_DEBUG_U64: [u64; 16] = [
    0x0000000000000000,
    0x1010101010101010,
    0x2020202020202020,
    0x3030303030303030,
    0x4040404040404040,
    0x5050505050505050,
    0x6060606060606060,
    0x7070707070707070,
    0x8080808080808080,
    0x9090909090909090,
    0xA0A0A0A0A0A0A0A0,
    0xB0B0B0B0B0B0B0B0,
    0xC0C0C0C0C0C0C0C0,
    0xD0D0D0D0D0D0D0D0,
    0xE0E0E0E0E0E0E0E0,
    0xF0F0F0F0F0F0F0F0,
];

const EGA_FONT_SPAN: usize = 256;

static EGA_FONTS: [EGAFont; 2] = [
    EGAFont {
        w:    8,
        h:    8,
        span: EGA_FONT_SPAN,
        data: include_bytes!("../../../../assets/ega_8by8.bin"),
    },
    EGAFont {
        w:    8,
        h:    14,
        span: EGA_FONT_SPAN,
        data: include_bytes!("../../../../assets/ega_8by14.bin"),
    },
];

const EGA_FONT8: &'static [u8] = include_bytes!("../../../../assets/ega_8by8.bin");
const EGA_FONT14: &'static [u8] = include_bytes!("../../../../assets/ega_8by14.bin");

// Display apertures for each EGA clock
// In 14Mhz mode, EGA apertures are similar to CGA apertures.
// In 16Mhz mode, there is no difference between NORMAL and FULL apertures.
// Apertures are listed in order:
// NORMAL, FULL, DEBUG

const EGA14_APERTURE_CROPPED_W: u32 = 640;
const EGA14_APERTURE_CROPPED_H: u32 = 200;
const EGA14_APERTURE_CROPPED_X: u32 = 96;
const EGA14_APERTURE_CROPPED_Y: u32 = 4;

const EGA14_APERTURE_FULL_W: u32 = 768;
const EGA14_APERTURE_FULL_H: u32 = 236;
const EGA14_APERTURE_FULL_X: u32 = 48;
const EGA14_APERTURE_FULL_Y: u32 = 0;

const EGA16_APERTURE_CROPPED_W: u32 = 640;
const EGA16_APERTURE_CROPPED_H: u32 = 350;
const EGA16_APERTURE_CROPPED_X: u32 = 40;
const EGA16_APERTURE_CROPPED_Y: u32 = 2;
const EGA16_APERTURE_FULL_W: u32 = 640 + 16 + 16;
const EGA16_APERTURE_FULL_H: u32 = 350;
const EGA16_APERTURE_FULL_X: u32 = 32;
const EGA16_APERTURE_FULL_Y: u32 = 2;

const EGA_APERTURES: [[DisplayAperture; 3]; 2] = [
    [
        // 14Mhz NORMAL aperture
        DisplayAperture {
            w: EGA14_APERTURE_CROPPED_W,
            h: EGA14_APERTURE_CROPPED_H,
            x: EGA14_APERTURE_CROPPED_X,
            y: EGA14_APERTURE_CROPPED_Y,
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
        // 16Mhz NORMAL aperture
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
        // 14Mhz DEBUG aperture
        DisplayAperture {
            w: EGA16_MAX_RASTER_X,
            h: EGA16_MAX_RASTER_Y,
            x: 0,
            y: 2,
            debug: true,
        },
    ],
];

const EGA_APERTURE_DESCS: [DisplayApertureDesc; 3] = [
    DisplayApertureDesc {
        name: "Cropped",
        idx:  0,
    },
    DisplayApertureDesc { name: "Full", idx: 1 },
    DisplayApertureDesc { name: "Debug", idx: 2 },
];

#[derive(Clone)]
pub struct DisplayPlane {
    latch: u8,
    buf:   Box<[u8]>,
}

impl DisplayPlane {
    fn new() -> Self {
        Self {
            latch: 0,
            buf:   Box::new([0; EGA_GFX_PLANE_SIZE]),
        }
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct AttributePaletteEntry {
    six: u8,
    four: u8,
    four_to_six: u8,
    mono: bool,
}

impl AttributePaletteEntry {
    pub fn set(&mut self, byte: u8) {
        self.six = byte & 0x3F;
        self.four = byte & 0x0F | ((byte & 0x10) >> 1);
        self.four_to_six = CGA_TO_EGA_U8[self.four as usize];
        self.mono = (byte & 0x08) != 0;
    }
}

pub struct EGACard {
    ticks_accum: f64,
    char_clock: u32,
    clock_mode: ClockingMode,
    clock_divisor: u32,
    clock_change_pending: bool,
    cycles: u64,
    debug: bool,
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

    cur_char:  u8,   // Current character being drawn
    cur_attr:  u8,   // Current attribute byte being drawn
    cur_fg:    u8,   // Current glyph fg color
    cur_bg:    u8,   // Current glyph bg color
    cur_blink: bool, // Current glyph blink attribute

    blink_state: bool, // Blink state for cursor and 'blink' attribute

    char_col: u8, // Column of character glyph being drawn
    hcc: u8,      // Horizontal character counter (x pos of character)
    vlc: u8,      // Vertical line counter - row of character being drawn
    vcc: u8,      // Vertical character counter (y pos of character)
    hslc: u16,    // Horizontal scanline counter - increments after reaching vertical total
    hsc: u8,      // Horizontal sync counter - counts during hsync period
    vtac_c5: u8,
    in_vta: bool,
    effective_vta: u8,
    vma: usize,    // VMA register - Video memory address
    vma_sl: usize, // VMA of start of scanline
    vma_t: usize,  // VMA' register - Video memory address temporary
    vmws: usize,   // Video memory word size

    cursor_status: bool,
    cursor_slowblink: bool,
    cursor_blink_rate: f64,
    cursor_data: [bool; EGA_CURSOR_MAX],
    cursor_attr: u8,

    cc_register: u8,

    crtc_register_select_byte: u8,
    crtc_register_selected:    CRTCRegister,

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
    crtc_vertical_total: u16,  // R(6) Vertical Total (9-bit value)
    crtc_overflow: u8,         // R(7) Overflow
    crtc_preset_row_scan: u8,  // R(8) Preset Row Scan
    crtc_maximum_scanline: u8, // R(9) Max Scanline
    crtc_cursor_start: u8,     // R(A) Cursor Location (9-bit value)
    crtc_cursor_enabled: bool, // Calculated from R(A) bit 5
    crtc_cursor_end: u8,       // R(B)
    crtc_cursor_skew: u8,      // Calculated from R(B) bits 5-6
    crtc_start_address_ho: u8, // R(C)
    crtc_start_address_lo: u8, // R(D)
    crtc_start_address: u16,   // Calculated from C&D
    start_address_latch: usize,
    crtc_cursor_address_lo: u8, // R(E)
    crtc_cursor_address_ho: u8, // R(F)
    crtc_cursor_address: u16,
    crtc_vertical_retrace_start: u16, // R(10) Vertical Retrace Start (9-bit value)
    crtc_vertical_retrace_end: CVerticalRetraceEnd, // R(11) Vertical Retrace End (5-bit value)
    crtc_vertical_retrace_end_norm: u16, // Vertial Retrace Start value normalized to scanline number
    crtc_vertical_display_end: u16,   // R(12) Vertical Display Enable End (9-bit value)
    crtc_offset: u8,                  // R(13)
    crtc_underline_location: u8,      // R(14)
    crtc_start_vertical_blank: u16,   // R(15) Start Vertical Blank (9-bit value)
    crtc_end_vertical_blank: u16,     // R(16)
    crtc_mode_control: u8,            // R(17)
    crtc_line_compare: u16,           // R(18) Line Compare (9-bit value)

    crtc_den: bool,
    crtc_vblank: bool,
    crtc_hblank: bool,
    crtc_hsync: bool,
    monitor_hsync: bool,
    crtc_vborder: bool,
    crtc_hborder: bool,
    in_display_area: bool,
    in_last_vblank_line: bool,

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
    attribute_palette_registers: [AttributePaletteEntry; 16],
    attribute_palette_index: usize,
    attribute_mode_control: AModeControl,
    attribute_overscan_color: AOverscanColor,
    attribute_color_plane_enable: AColorPlaneEnable,
    attribute_pel_panning: u8,
    pel_pan_latch: u8,

    current_font: usize,

    misc_output_register: EMiscellaneousOutputRegister,

    // Display Planes
    planes: [DisplayPlane; 4],
    chain_buf: Box<[u8; EGA_GFX_PLANE_SIZE * 8]>,
    pixel_buf: [u8; 8],
    pipeline_buf: [u8; 4],
    write_buf: [u8; 4],

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
            ticks_accum: 0.0,
            char_clock: 8,
            clock_divisor: 1,
            clock_change_pending: false,
            clock_mode: ClockingMode::Cycle,
            cycles: 0,
            debug: false,
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

            cur_char: 0,      // Current character being drawn
            cur_attr: 0,      // Current attribute byte being drawn
            cur_fg: 0,        // Current glyph fg color
            cur_bg: 0,        // Current glyph bg color
            cur_blink: false, // Current glyph blink attribute
            blink_state: false,
            char_col: 0,
            hcc: 0,
            vlc: 0,
            vcc: 0,
            hslc: 0,
            hsc: 0,
            vtac_c5: 0,
            in_vta: false,
            effective_vta: 0,
            vma: 0,
            vma_t: 0,
            vma_sl: 0,
            vmws: 1,

            cursor_status: true,
            cursor_slowblink: false,
            cursor_blink_rate: CGA_DEFAULT_CURSOR_BLINK_RATE,
            cursor_data: [false; EGA_CURSOR_MAX],
            cursor_attr: 0,

            cc_register: CC_PALETTE_BIT | CC_BRIGHT_BIT,

            crtc_register_selected:    CRTCRegister::HorizontalTotal,
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
            crtc_retrace_width: 0,
            crtc_vertical_total: DEFAULT_VERTICAL_TOTAL,
            crtc_overflow: DEFAULT_OVERFLOW,
            crtc_preset_row_scan: DEFAULT_PRESET_ROW_SCAN,
            crtc_maximum_scanline: DEFAULT_MAX_SCANLINE,
            crtc_cursor_start: DEFAULT_CURSOR_START_LINE,
            crtc_cursor_enabled: true,
            crtc_cursor_end: DEFAULT_CURSOR_END_LINE,
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
            crtc_mode_control: 0,
            crtc_line_compare: 0,

            crtc_den: false,
            crtc_vblank: false,
            crtc_hblank: false,
            crtc_hsync: false,
            monitor_hsync: false,
            crtc_hborder: false,
            crtc_vborder: false,
            in_display_area: false,
            in_last_vblank_line: false,

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
            attribute_palette_registers: [Default::default(); 16],
            attribute_palette_index: 0,
            attribute_mode_control: AModeControl::new(),
            attribute_overscan_color: AOverscanColor::new(),
            attribute_color_plane_enable: AColorPlaneEnable::new(),
            attribute_pel_panning: 0,
            pel_pan_latch: 0,

            current_font: 0,
            misc_output_register: EMiscellaneousOutputRegister::new(),

            planes: [
                DisplayPlane::new(),
                DisplayPlane::new(),
                DisplayPlane::new(),
                DisplayPlane::new(),
            ],
            chain_buf: Box::new([0; EGA_GFX_PLANE_SIZE * 8]),
            pixel_buf: [0; 8],
            pipeline_buf: [0; 4],
            write_buf: [0; 4],

            back_buf:  1,
            front_buf: 0,
            extents:   EGACard::get_default_extents(),
            aperture:  0,

            //buf: vec![vec![0; (CGA_XRES_MAX * CGA_YRES_MAX) as usize]; 2],

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
        }
    }
}

impl EGACard {
    pub fn new(trace_logger: TraceLogger, clock_mode: ClockingMode, video_frame_debug: bool) -> Self {
        let mut ega = Self::default();

        ega.trace_logger = trace_logger;
        ega.debug = video_frame_debug;
        ega.clock_mode = clock_mode;

        if video_frame_debug {
            // move debugging flag to aperture selection.

            /*
            ega.extents.field_w = EGA16_MAX_RASTER_X;
            ega.extents.field_h = EGA16_MAX_RASTER_Y;
            ega.extents.aperture_w = EGA16_MAX_RASTER_X;
            ega.extents.aperture_h = EGA16_MAX_RASTER_Y;
            ega.extents.aperture_x = 0;
            ega.extents.aperture_y = 0;
            ega.extents.row_stride = EGA16_MAX_RASTER_X as usize;
            ega.vblank_color = EGA_VBLANK_DEBUG_COLOR;
            ega.hblank_color = EGA_HBLANK_DEBUG_COLOR;
            ega.disable_color = EGA_DISABLE_DEBUG_COLOR;
            */
        }

        ega
    }

    fn get_default_extents() -> DisplayExtents {
        DisplayExtents {
            field_w: EGA16_MAX_RASTER_X,
            field_h: EGA16_MAX_RASTER_Y,
            aperture: EGA_APERTURES[1][0].clone(),
            visible_w: 0,
            visible_h: 0,
            row_stride: EGA16_MAX_RASTER_X as usize,
            double_scan: false,
            mode_byte: 0,
        }
    }

    fn reset_private(&mut self) {
        self.mode_byte = 0;
        self.display_mode = DisplayMode::Mode3TextCo80;
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

        // EGA doesn't really have a 'cursor status' bit anywhere, so this is always true.
        self.cursor_status = true;
        self.cursor_slowblink = false;
        self.cursor_blink_rate = CGA_DEFAULT_CURSOR_BLINK_RATE;

        //self.cc_register: CC_PALETTE_BIT | CC_BRIGHT_BIT,

        self.crtc_register_selected = CRTCRegister::HorizontalTotal;
        self.crtc_register_select_byte = 0;

        self.crtc_horizontal_total = DEFAULT_HORIZONTAL_TOTAL;
        self.crtc_horizontal_display_end = DEFAULT_HORIZONTAL_DISPLAYED;
        self.crtc_start_horizontal_blank = DEFAULT_HORIZONTAL_SYNC_POS;
        self.crtc_end_horizontal_blank =
            CEndHorizontalBlank::new().with_end_horizontal_blank(DEFAULT_HORIZONTAL_SYNC_WIDTH);
        self.crtc_display_enable_skew = 0;
        self.crtc_start_horizontal_retrace = 0;
        self.crtc_end_horizontal_retrace = CEndHorizontalRetrace::new();
        self.crtc_vertical_total = DEFAULT_VERTICAL_TOTAL;
        self.crtc_overflow = DEFAULT_OVERFLOW;
        self.crtc_preset_row_scan = DEFAULT_PRESET_ROW_SCAN;
        self.crtc_maximum_scanline = DEFAULT_MAX_SCANLINE;
        self.crtc_cursor_start = DEFAULT_CURSOR_START_LINE;
        self.crtc_cursor_enabled = true;
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
        let clock_old = self.misc_output_register.clock_select();
        self.misc_output_register = EMiscellaneousOutputRegister::from_bytes([byte]);

        if clock_old != self.misc_output_register.clock_select() {
            // Clock updated.
            self.clock_change_pending = true;
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
        let mut byte = 0;

        // These shifts match the EGA BIOS sense switch behavior
        // Switches should be Open, Closed, Closed, Open for EGA Card & Monitor
        let switch_status = match self.misc_output_register.clock_select() {
            ClockSelect::Unused => EGA_DIP_SWITCH >> 3 & 0x01,
            ClockSelect::ExternalClock => EGA_DIP_SWITCH >> 2 & 0x01,
            ClockSelect::Clock16 => EGA_DIP_SWITCH >> 1 & 0x01,
            ClockSelect::Clock14 => EGA_DIP_SWITCH & 0x01,
        };

        // Set switch sense bit
        byte |= switch_status << 4;

        // Set CRT interrupt bit. Bit is 0 when retrace is occurring.
        byte |= match self.crtc_vblank {
            true => 0,
            false => 0x80,
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
        // TODO: Some references specifically mention this as HBLANK or VBLANK,
        // but on the CGA is is actually not in active display area, which is different.
        // Which way is it really on the EGA?

        // The IBM EGA bios sets up a very wide border area during its HBLANK count test.
        // The implication there is that we can poll for !DEN not HBLANK.
        //if self.crtc_hblank || self.crtc_vblank {
        if !self.crtc_den {
            byte |= 0x01;
        }
        if self.crtc_vblank {
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

        if self.hslc < 9 as u16 {
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
                    self.attribute_mode_control.display_type(),
                ) {
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
                    self.attribute_mode_control.display_type(),
                ) {
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
                if let EGA_MEM_ADDRESS..=EGA_MEM_END_128 = address {
                    // 128k aperture is usually used with chain odd/even mode.
                    if self.graphics_micellaneous.chain_odd_even() == true {
                        // Just return the shifted address. We'll use logic elsewhere to determine plane.
                        return Some(((address - EGA_MEM_ADDRESS) >> 1) & 0xFFFF);
                    }
                    else {
                        // Not sure what to do in this case if we're out of bounds of a 64k plane.
                        // So just mask it to 64k for now.
                        return Some((address - EGA_MEM_ADDRESS) & 0xFFFF);
                    }
                }
                else {
                    return None;
                }
            }
            MemoryMap::A0000_64K => {
                if let EGA_MEM_ADDRESS..=EGA_MEM_END_64 = address {
                    return Some(address - EGA_MEM_ADDRESS);
                }
                else {
                    return None;
                }
            }
            MemoryMap::B8000_32K => {
                if let CGA_MEM_ADDRESS..=CGA_MEM_END = address {
                    return Some(address - CGA_MEM_ADDRESS);
                }
                else {
                    return None;
                }
            }
            _ => return None,
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
                false => 0x00,
            };
            plane_comp |= match self.planes[1].latch & (0x01 << i) != 0 {
                true => 0x02,
                false => 0x00,
            };
            plane_comp |= match self.planes[2].latch & (0x01 << i) != 0 {
                true => 0x04,
                false => 0x00,
            };
            plane_comp |= match self.planes[3].latch & (0x01 << i) != 0 {
                true => 0x08,
                false => 0x00,
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

    /// Tick the EGA device. This is much simpler than the implementation in the CGA device as
    /// we only support ticking by character clock.
    fn tick(&mut self, ticks: f64) {
        self.ticks_accum += ticks;

        // Drain the accumulator while emitting chars
        while self.ticks_accum > self.char_clock as f64 {
            match self.sequencer_clocking_mode.dot_clock() {
                DotClock::Native => self.tick_hchar(),
                DotClock::HalfClock => self.tick_lchar(),
            }
            self.ticks_accum -= self.char_clock as f64;
        }
    }

    fn tick_hchar(&mut self) {
        assert_eq!(self.cycles & 0x07, 0);
        assert_eq!(self.char_clock, 8);

        self.cycles += 8;

        // Only draw if marty_render buffer address is in bounds.
        if self.rba < (EGA_MAX_CLOCK16 - 8) {
            if self.in_display_area {
                // Draw current character row
                match self.attribute_mode_control.mode() {
                    AttributeMode::Text => {
                        self.draw_text_mode_hchar14();
                    }
                    AttributeMode::Graphics => {
                        self.draw_gfx_mode_hchar_6bpp();
                    }
                }
            }
            else if self.crtc_hblank {
                if self.extents.aperture.debug {
                    // Draw hblank in debug color
                    if self.monitor_hsync {
                        self.draw_solid_hchar_6bpp(EgaDefaultColor6Bpp::Cyan as u8);
                    }
                    else if self.crtc_hsync {
                        self.draw_solid_hchar_6bpp(EgaDefaultColor6Bpp::BlueBright as u8);
                    }
                    else {
                        self.draw_solid_hchar_6bpp(EgaDefaultColor6Bpp::Blue as u8);
                    }
                }
            }
            else if self.crtc_vblank {
                if self.extents.aperture.debug {
                    // Draw vblank in debug color
                    self.draw_solid_hchar_6bpp(EgaDefaultColor6Bpp::Magenta as u8);
                }
            }
            else if self.crtc_vborder | self.crtc_hborder {
                // Draw overscan
                if self.extents.aperture.debug {
                    self.draw_solid_hchar_6bpp(EgaDefaultColor6Bpp::Green as u8);
                }
                else {
                    self.draw_solid_hchar_6bpp(0);
                }
            }
            else {
                //self.draw_solid_hchar(CGA_DEBUG2_COLOR);
                //log::warn!("invalid display state...");
                //self.dump_status();
                //panic!("invalid display state...");
            }
        }

        // Update position to next pixel and character column.
        self.raster_x += 8 * self.clock_divisor as u32;
        self.rba += 8 * self.clock_divisor as usize;

        // If we have reached the right edge of the 'monitor', return the raster position
        // to the left side of the screen.
        if self.raster_x >= self.extents.field_w {
            self.raster_x = 0;
            self.raster_y += 1;
            //self.in_monitor_hsync = false;
            self.rba = self.extents.row_stride * self.raster_y as usize;
        }

        /*
        if self.cycles & self.char_clock_mask != 0 {
            log::error!("tick_hchar(): calling tick_crtc_char but out of phase with cclock: cycles: {} mask: {}", self.cycles, self.char_clock_mask);
        }
        */
        //self.draw_debug_hchar_at((EGA_MAX_CLOCK16 / 2) - 8, EgaDefaultColor::Yellow as u8);
        //self.draw_debug_hchar_at(EGA_MAX_CLOCK16 - 8, EgaDefaultColor::MagentaBright as u8);
        self.tick_crtc_char();
        //self.update_clock();
    }

    fn tick_lchar(&mut self) {
        //assert_eq!(self.cycles & 0x0F, 0);
        assert_eq!(self.char_clock, 16);

        self.cycles += 8;

        // Only draw if marty_render buffer address is in bounds.
        if self.rba < (EGA_MAX_CLOCK16 - 16) {
            if self.in_display_area {
                match self.attribute_mode_control.mode() {
                    AttributeMode::Text => {
                        // Draw current character row
                        self.draw_text_mode_lchar14();
                    }
                    AttributeMode::Graphics => {
                        self.draw_gfx_mode_lchar_4bpp();
                    }
                }
            }
            else if self.crtc_hblank {
                if self.extents.aperture.debug {
                    if self.crtc_hsync {
                        self.draw_solid_lchar(EgaDefaultColor6Bpp::BlueBright as u8);
                    }
                    else {
                        self.draw_solid_lchar(EgaDefaultColor6Bpp::Blue as u8);
                    }
                }
            }
            else if self.crtc_vblank {
                if self.extents.aperture.debug {
                    // Draw vblank in debug color
                    self.draw_solid_lchar(EgaDefaultColor6Bpp::Magenta as u8);
                }
            }
            else if self.crtc_vborder | self.crtc_hborder {
                // Draw overscan
                if self.extents.aperture.debug {
                    //self.draw_solid_hchar(CGA_OVERSCAN_COLOR);
                    self.draw_solid_lchar(EgaDefaultColor6Bpp::Green as u8);
                }
                else {
                    self.draw_solid_lchar(0);
                }
            }
            else {
                //self.draw_solid_hchar(CGA_DEBUG2_COLOR);
                //log::warn!("invalid display state...");
                //self.dump_status();
                //panic!("invalid display state...");
            }
        }

        // Update position to next pixel and character column.
        self.raster_x += 8 * self.clock_divisor as u32;
        self.rba += 8 * self.clock_divisor as usize;

        // If we have reached the right edge of the 'monitor', return the raster position
        // to the left side of the screen.

        /*
        if self.raster_x >= self.extents.field_w {
            self.raster_x = 0;
            self.raster_y += 1;
            //self.in_monitor_hsync = false;
            self.rba = (self.extents.row_stride * self.raster_y as usize);
        }
        */

        /*
        if self.cycles & self.char_clock_mask != 0 {
            log::error!("tick_hchar(): calling tick_crtc_char but out of phase with cclock: cycles: {} mask: {}", self.cycles, self.char_clock_mask);
        }
        */
        //self.draw_debug_hchar_at((EGA_MAX_CLOCK16 / 2) - 8, EgaDefaultColor::Yellow as u8);
        //self.draw_debug_hchar_at(EGA_MAX_CLOCK16 - 8, EgaDefaultColor::MagentaBright as u8);
        self.tick_crtc_char();
        //self.update_clock();
    }

    //noinspection ALL
    /// Get the 64-bit value representing the specified row of the specified character
    /// glyph in high-resolution text mode.
    #[inline]
    pub fn get_hchar_glyph14_row(&self, glyph: usize, row: usize) -> u64 {
        if self.cur_blink && !self.blink_state {
            EGA_COLORS_U64[self.cur_bg as usize]
        }
        else {
            let glyph_row_base = EGA_HIRES_GLYPH14_TABLE[glyph & 0xFF][row];

            // Combine glyph mask with foreground and background colors.
            glyph_row_base & EGA_COLORS_U64[self.cur_fg as usize]
                | !glyph_row_base & EGA_COLORS_U64[self.cur_bg as usize]
        }
    }

    //noinspection ALL
    /// Get a tuple of 64-bit values representing the specified row of the specified character
    /// glyph in low-resolution (40-column) mode.
    #[inline]
    pub fn get_lchar_glyph14_rows(&self, glyph: usize, row: usize) -> (u64, u64) {
        if self.cur_blink && !self.blink_state {
            let glyph = EGA_COLORS_U64[self.cur_bg as usize];
            (glyph, glyph)
        }
        else {
            let glyph_row_base_0 = EGA_LOWRES_GLYPH14_TABLE[glyph & 0xFF][0][row];
            let glyph_row_base_1 = EGA_LOWRES_GLYPH14_TABLE[glyph & 0xFF][1][row];

            // Combine glyph mask with foreground and background colors.
            let glyph0 = glyph_row_base_0 & EGA_COLORS_U64[self.cur_fg as usize]
                | !glyph_row_base_0 & EGA_COLORS_U64[self.cur_bg as usize];
            let glyph1 = glyph_row_base_1 & EGA_COLORS_U64[self.cur_fg as usize]
                | !glyph_row_base_1 & EGA_COLORS_U64[self.cur_bg as usize];

            (glyph0, glyph1)
        }
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

            // Width is total characters * character width * clock_divisor.
            // This makes the buffer twice as wide as it normally would be in 320 pixel modes, since
            // we scan pixels twice.
            // TODO: We never really use the calculated 'visible' parameters. Can probably remove.
            self.extents.visible_w =
                (self.crtc_horizontal_total + 2) as u32 * EGA_HCHAR_CLOCK as u32 * self.clock_divisor as u32;

            //trace_regs!(self);
            //trace!(self, "Leaving vsync and flipping buffers");

            self.scanline = 0;
            self.frame += 1;

            // Swap the display buffers
            self.swap();
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

    /// Update the cursor data array based on the values of cursor_start_line and cursor_end_line.
    /// TODO: This logic was copied from CGA. EGA likely has different cursor logic.
    fn update_cursor_data(&mut self) {
        // Reset cursor data to 0.
        self.cursor_data.fill(false);

        if self.crtc_cursor_start <= self.crtc_cursor_end {
            // Normal cursor definition. Cursor runs from start_line to end_line.
            for i in self.crtc_cursor_start..=self.crtc_cursor_end {
                self.cursor_data[i as usize] = true;
            }
        }
        else {
            // "Split" cursor.
            for i in 0..self.crtc_cursor_end {
                // First part of cursor is 0->end_line
                self.cursor_data[i as usize] = true;
            }

            for i in (self.crtc_cursor_start as usize)..EGA_CURSOR_MAX {
                // Second part of cursor is start_line->max
                self.cursor_data[i] = true;
            }
        }
    }

    fn update_clock(&mut self) {
        if self.clock_change_pending {
            (self.clock_divisor, self.char_clock) = match self.sequencer_clocking_mode.dot_clock() {
                DotClock::HalfClock => (2, 16),
                DotClock::Native => (1, 8),
            };

            // Update display aperture for new clock.
            match self.misc_output_register.clock_select() {
                ClockSelect::Clock14 => {
                    self.extents.field_w = EGA14_MAX_RASTER_X;
                    self.extents.field_h = EGA14_MAX_RASTER_Y;
                    self.extents.row_stride = EGA14_MAX_RASTER_X as usize;
                    self.extents.aperture = EGA_APERTURES[0][self.aperture].clone();
                    self.extents.double_scan = true;
                }
                ClockSelect::Clock16 => {
                    self.extents.field_w = EGA16_MAX_RASTER_X;
                    self.extents.field_h = EGA16_MAX_RASTER_Y;
                    self.extents.row_stride = EGA16_MAX_RASTER_X as usize;
                    self.extents.aperture = EGA_APERTURES[1][self.aperture].clone();
                    self.extents.double_scan = false;
                }
                _ => {
                    // Unsupported
                }
            }
        }

        log::debug!(
            "Updated EGA Clock, new extents: {},{} aperture: {},{}",
            self.extents.field_w,
            self.extents.field_h,
            self.extents.aperture.w,
            self.extents.aperture.h
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
        let mut ega = EGACard::new(TraceLogger::None, ClockingMode::Character, false);

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

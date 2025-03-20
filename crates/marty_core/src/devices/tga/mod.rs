/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2025 Daniel Balsom

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

    --------------------------------------------------------------------------

    devices::tga::mod.rs

    Implementation of the Tandy Video Gate Array, built around the Motorola MC6845
    display controller. This module will also support the PCJr graphics subsystem
    due to the similarities between the two systems.

    Unlike the CGA, the TGA has a proper low-resolution 160x200 pixel addressable
    mode. Thus we have three clock divisors, 1, 2 and 4 to support 640x200, 320x200
    and 160x200 modes, respectively.  Thus the various character-related functions
    have been renamed hchar, mchar and lchar to match.

*/

#![allow(dead_code)]
use super::{tga::tablegen::*, *};
use crate::{
    bus::{BusInterface, DeviceRunTimeUnit},
    device_traits::videocard::*,
    tracelogger::TraceLogger,
};
use bytemuck;
use const_format::formatcp;
use modular_bitfield::{
    bitfield,
    prelude::{B1, B2, B3, B4},
};
use std::{collections::HashMap, convert::TryInto, path::Path};

#[macro_use]
mod io;
mod draw;
mod mmio;
mod tablegen;
mod videocard;

#[derive(Copy, Clone)]
enum RwSlotType {
    Mem,
    Io,
}

impl Default for RwSlotType {
    fn default() -> Self {
        RwSlotType::Mem
    }
}

// A device can have a maximum of 4 operations to handle between calls to run().
// Up to two IO operations (16-bit IO) or 4 memory operations (16-bit mov)
// We maintain 4 slots of RwSlot structs to keep data about these operations.
// The slot index is reset on call to run().
#[derive(Copy, Clone, Default)]
struct RwSlot {
    t:    RwSlotType,
    data: u8,
    addr: u32,
    tick: u32,
}

#[bitfield]
#[derive(Copy, Clone, Debug)]
pub struct TModeControlRegister {
    pub unused: B2,
    pub border_enable: bool,
    pub twobpp_hires: bool,
    pub fourbpp_mode: bool,
    #[skip]
    unused: B3,
}

#[bitfield]
#[derive(Copy, Clone, Debug)]
pub struct JrModeControlRegister {
    pub bandwidth: bool,
    pub graphics: bool,
    pub bw: bool,
    pub video: bool,
    pub fourbpp_mode: bool,
    #[skip]
    unused: B3,
}

#[bitfield]
#[derive(Copy, Clone, Debug)]
pub struct JrModeControlRegister2 {
    #[skip]
    reserved0: B1,
    pub blink: bool,
    #[skip]
    reserved1: B1,
    pub twobpp_mode: bool,
    #[skip]
    unused: B4,
}

#[bitfield]
#[derive(Copy, Clone, Debug)]
pub struct TPageRegister {
    pub crt_page: B3,
    pub cpu_page: B3,
    pub address_mode: B2,
}

pub enum VideoModeSize {
    Mode16k,
    Mode32k,
}

static DUMMY_PLANE: [u8; 1] = [0];
static DUMMY_PIXEL: [u8; 4] = [0, 0, 0, 0];

// Precalculated waits in system ticks for each of the possible 16 phases of the
// CGA clock could issue a memory request on.
static WAIT_TABLE: [u32; 16] = [14, 13, 12, 11, 10, 9, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15];
// in cpu cycles: 5,5,4,4,4,3,8,8,8,7,7,7,6,6,6,5

pub const TGA_MEM_ADDRESS: usize = 0xB8000;
pub const TGA_MEM_APERTURE: usize = 0x8000; // 32Kb aperture.
pub const TGA_MEM_SIZE: usize = 0x8000; // 32Kb vram
pub const TGA_MEM_MASK: usize = !0x4000;

pub const CGA_MODE_ENABLE_MASK: u8 = 0b11_0111;

// Sensible defaults for CRTC registers. A real CRTC is probably uninitialized.
// 4/5/2023: Changed these values to 40 column mode.
const DEFAULT_HORIZONTAL_TOTAL: u8 = 56;
const DEFAULT_HORIZONTAL_DISPLAYED: u8 = 40;
const DEFAULT_HORIZONTAL_SYNC_POS: u8 = 45;
const DEFAULT_HORIZONTAL_SYNC_WIDTH: u8 = 10;
const DEFAULT_VERTICAL_TOTAL: u8 = 31;
const DEFAULT_VERTICAL_TOTAL_ADJUST: u8 = 6;
const DEFAULT_VERTICAL_DISPLAYED: u8 = 25;
const DEFAULT_VERTICAL_SYNC_POS: u8 = 28;
const DEFAULT_MAXIMUM_SCANLINE: u8 = 7;
const DEFAULT_CURSOR_START_LINE: u8 = 6;
const DEFAULT_CURSOR_END_LINE: u8 = 7;

const DEFAULT_CLOCK_DIVISOR: u8 = 2;
const DEFAULT_CHAR_CLOCK: u32 = 16;
const DEFAULT_CHAR_CLOCK_MASK: u64 = 0x0F;
const DEFAULT_CHAR_CLOCK_ODD_MASK: u64 = 0x1F;

const TGA_IRQ: u8 = 0x05;

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
    This produces vertical overscan borders of 26 pixels and horizontal borders of 96 pixels
    The Area5150 demo manages to squeeze out a 768 pixel horizontal resolution mode from
    the CGA. This is accomplished with a HorizontalDisplayed value of 96. (96 * 8 = 768)
    I am assuming this is the highest value we will actually ever encounter and anything
    wider might not sync to a real monitor.
*/

// Calculate the maximum possible area of buf field (including refresh period)
const CGA_XRES_MAX: u32 = (CRTC_R0_HORIZONTAL_MAX + 1) * TGA_HCHAR_CLOCK as u32;
const CGA_YRES_MAX: u32 = CRTC_SCANLINE_MAX;
pub const CGA_MAX_CLOCK: usize = (CGA_XRES_MAX * CGA_YRES_MAX) as usize; // Should be 238944

// Monitor sync position. The monitor will eventually perform an hsync at a fixed position
// if hsync signal is late from the CGA card.
const CGA_MONITOR_HSYNC_POS: u32 = 832;
const CGA_MONITOR_HSYNC_WIDTH: u32 = 80;
const CGA_MONITOR_VSYNC_POS: u32 = 246;
// Minimum scanline value after which we can perform a vsync. A vsync before this scanline will be ignored.
const CGA_MONITOR_VSYNC_MIN: u32 = 127;

// For derivation of CGA timings, see https://www.vogons.org/viewtopic.php?t=47052
// We run the CGA card independent of the CPU frequency.
// Timings in 4.77Mhz CPU cycles are provided for reference.
const FRAME_TIME_CLOCKS: u32 = 238944;
const FRAME_TIME_US: f64 = 16_688.15452339;
const FRAME_VBLANK_US: f64 = 14_732.45903422;
//const FRAME_CPU_TIME: u32 = 79_648;
//const FRAME_VBLANK_START: u32 = 70_314;

const SCANLINE_TIME_CLOCKS: u32 = 912;
const SCANLINE_TIME_US: f64 = 63.69524627;
const SCANLINE_HBLANK_US: f64 = 52.38095911;
//const SCANLINE_CPU_TIME: u32 = 304;
//const SCANLINE_HBLANK_START: u32 = 250;

const CGA_HBLANK: f64 = 0.1785714;

const CGA_DEFAULT_CURSOR_BLINK_RATE: f64 = 0.0625;
const CGA_CURSOR_BLINK_RATE_CLOCKS: u32 = FRAME_TIME_CLOCKS * 8;
const CGA_CURSOR_BLINK_RATE_US: f64 = FRAME_TIME_US * 8.0;

const CGA_DEFAULT_CURSOR_FRAME_CYCLE: u32 = 8;

const MODE_MATCH_MASK: u8 = 0b0001_1111;
const MODE_HIRES_TEXT: u8 = 0b0000_0001;
const MODE_GRAPHICS: u8 = 0b0000_0010;
const MODE_BW: u8 = 0b0000_0100;
const MODE_ENABLE: u8 = 0b0000_1000;
const MODE_HIRES_GRAPHICS: u8 = 0b0001_0000;
const MODE_BLINKING: u8 = 0b0010_0000;
const VMODE_4BPP: u8 = 0b0010_0000;

const CURSOR_LINE_MASK: u8 = 0b0001_1111;
const CURSOR_ATTR_MASK: u8 = 0b0110_0000;
const CURSOR_ENABLE_MASK: u8 = 0b0010_0000;

// Color control register bits.
// Alt color = Overscan in Text mode, BG color in 320x200 graphics, FG color in 640x200 graphics
const CC_ALT_COLOR_MASK: u8 = 0b0000_0111;
const CC_ALT_INTENSITY: u8 = 0b0000_1000;
const CC_BRIGHT_BIT: u8 = 0b0001_0000; // Controls whether palette is high intensity
const CC_PALETTE_BIT: u8 = 0b0010_0000; // Controls primary palette between magenta/cyan and red/green

const STATUS_DISPLAY_ENABLE: u8 = 0b0000_0001;
const STATUS_LIGHTPEN_TRIGGER_SET: u8 = 0b0000_0010;
const STATUS_LIGHTPEN_SWITCH_STATUS: u8 = 0b0000_0100;
const STATUS_VERTICAL_RETRACE: u8 = 0b0000_1000;
const STATUS_VIDEO_MUX: u8 = 0b0001_0000;

// Include the basic 8x9 TGA font. Technically, TGA has no character ROM, the fonts were accessed
// from the BIOS ROM. We include the font here to reuse CGA methods.
const TGA_FONT: &[u8] = include_bytes!("../../../assets/tga_8by9.bin");
const TGA_FONT_SPAN: usize = 256; // Font bitmap is 2048 bits wide (256 * 8 characters)

const TGA_HCHAR_CLOCK: u8 = 8;
const TGA_MCHAR_CLOCK: u8 = 16;
const TGA_LCHAR_CLOCK: u8 = 32;
const CRTC_FONT_HEIGHT: u8 = 9;
const CRTC_VSYNC_HEIGHT: u8 = 16;

const CRTC_R0_HORIZONTAL_MAX: u32 = 113;
const CRTC_SCANLINE_MAX: u32 = 262;

// The CGA card decodes different numbers of address lines from the CRTC depending on
// whether it is in text or graphics modes. This causes wrapping at 0x2000 bytes in
// text mode, and 0x4000 bytes in graphics modes.
const CGA_TEXT_MODE_WRAP: usize = 0x1FFF;
const CGA_GFX_MODE_WRAP: usize = 0x3FFF;
pub enum CgaColor {
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

const CGA_COLORS_TO_RGB: [(u8, u8, u8); 16] = [
    (0x00u8, 0x00u8, 0x00u8),
    (0x00u8, 0x00u8, 0xAAu8),
    (0x00u8, 0xAAu8, 0x00u8),
    (0x00u8, 0xAAu8, 0xAAu8),
    (0xAAu8, 0x00u8, 0x00u8),
    (0xAAu8, 0x00u8, 0xAAu8),
    (0xAAu8, 0x55u8, 0x00u8),
    (0xAAu8, 0xAAu8, 0xAAu8),
    (0x55u8, 0x55u8, 0x55u8),
    (0x55u8, 0x55u8, 0xFFu8),
    (0x55u8, 0xFFu8, 0x55u8),
    (0x55u8, 0xFFu8, 0xFFu8),
    (0xFFu8, 0x55u8, 0x55u8),
    (0xFFu8, 0x55u8, 0xFFu8),
    (0xFFu8, 0xFFu8, 0x55u8),
    (0xFFu8, 0xFFu8, 0xFFu8),
];

const CGA_PALETTES: [[u8; 4]; 6] = [
    [0, 2, 4, 6],    // Red / Green / Brown
    [0, 10, 12, 14], // Red / Green / Brown High Intensity
    [0, 3, 5, 7],    // Cyan / Magenta / White
    [0, 11, 13, 15], // Cyan / Magenta / White High Intensity
    [0, 3, 4, 7],    // Red / Cyan / White
    [0, 11, 12, 15], // Red / Cyan / White High Intensity
];

const CGA_DEBUG_COLOR: u8 = CgaColor::Magenta as u8;
const CGA_DEBUG2_COLOR: u8 = CgaColor::RedBright as u8;
const CGA_HBLANK_DEBUG_COLOR: u8 = CgaColor::Blue as u8;
const CGA_VBLANK_DEBUG_COLOR: u8 = CgaColor::Yellow as u8;
const CGA_DISABLE_DEBUG_COLOR: u8 = CgaColor::Green as u8;
const CGA_OVERSCAN_DEBUG_COLOR: u8 = CgaColor::Green as u8;

/*
const CGA_FILL_COLOR: u8 = 4;
const CGA_SCANLINE_COLOR: u8 = 13;
*/

const CGA_CURSOR_MAX: usize = 32;

// Solid color spans of 8 pixels.
// Used for drawing overscan fast with bytemuck
const CGA_COLORS_U64: [u64; 16] = [
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

// Solid color spans of 8 pixels.
// Used for drawing debug info into index buffer.
const CGA_DEBUG_U64: [u64; 16] = [
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

// Display apertures.
// CROPPED will show the display area only - no overscan will be visible.
// NORMAL is an attempt to represent the maximum visible display extents, including overscan.
// Anything more is likely to be hidden by the monitor bezel or not shown for some other reason.
// FULL will show the entire overscan area - this is nice for Area 5150 to see the entire extent
// of each effect, although it will display more than a monitor would.
// DEBUG will show the entire display field and will enable coloring of hblank and vblank
// periods.
const TGA_APERTURE_CROPPED_W: u32 = 640;
const TGA_APERTURE_CROPPED_H: u32 = 200;
const TGA_APERTURE_CROPPED_TALL_H: u32 = 225;
const TGA_APERTURE_CROPPED_X: u32 = 112;
const TGA_APERTURE_CROPPED_Y: u32 = 22;
const TGA_APERTURE_CROPPED_TALL_Y: u32 = 12;

const TGA_APERTURE_ACCURATE_W: u32 = 704;
const TGA_APERTURE_ACCURATE_H: u32 = 224;
const TGA_APERTURE_ACCURATE_TALL_H: u32 = 236;
const TGA_APERTURE_ACCURATE_X: u32 = 80;
const TGA_APERTURE_ACCURATE_Y: u32 = 10;
const TGA_APERTURE_ACCURATE_TALL_Y: u32 = 7;

const TGA_APERTURE_FULL_W: u32 = 768;
const TGA_APERTURE_FULL_H: u32 = 235;
const TGA_APERTURE_FULL_TALL_H: u32 = 250;
const TGA_APERTURE_FULL_X: u32 = 48;
const TGA_APERTURE_FULL_Y: u32 = 1;
const TGA_APERTURE_FULL_TALL_Y: u32 = 7;

const TGA_APERTURE_DEBUG_W: u32 = 912;
const TGA_APERTURE_DEBUG_H: u32 = 262;
const TGA_APERTURE_DEBUG_X: u32 = 0;
const TGA_APERTURE_DEBUG_Y: u32 = 0;

const TGA_APERTURES: [[DisplayAperture; 4]; 2] = [
    [
        // 14Mhz CROPPED aperture
        DisplayAperture {
            w: TGA_APERTURE_CROPPED_W,
            h: TGA_APERTURE_CROPPED_H,
            x: TGA_APERTURE_CROPPED_X,
            y: TGA_APERTURE_CROPPED_Y,
            debug: false,
        },
        // 14Mhz ACCURATE aperture
        DisplayAperture {
            w: TGA_APERTURE_ACCURATE_W,
            h: TGA_APERTURE_ACCURATE_H,
            x: TGA_APERTURE_ACCURATE_X,
            y: TGA_APERTURE_ACCURATE_Y,
            debug: false,
        },
        // 14Mhz FULL aperture
        DisplayAperture {
            w: TGA_APERTURE_FULL_W,
            h: TGA_APERTURE_FULL_H,
            x: TGA_APERTURE_FULL_X,
            y: TGA_APERTURE_FULL_Y,
            debug: false,
        },
        // 14Mhz DEBUG aperture
        DisplayAperture {
            w: TGA_APERTURE_DEBUG_W,
            h: TGA_APERTURE_DEBUG_H,
            x: 0,
            y: 0,
            debug: true,
        },
    ],
    [
        // 14Mhz CROPPED TALL aperture
        DisplayAperture {
            w: TGA_APERTURE_CROPPED_W,
            h: TGA_APERTURE_CROPPED_TALL_H,
            x: TGA_APERTURE_CROPPED_X,
            y: TGA_APERTURE_CROPPED_TALL_Y,
            debug: false,
        },
        // 14Mhz ACCURATE TALL aperture
        DisplayAperture {
            w: TGA_APERTURE_ACCURATE_W,
            h: TGA_APERTURE_ACCURATE_TALL_H,
            x: TGA_APERTURE_ACCURATE_X,
            y: TGA_APERTURE_ACCURATE_TALL_Y,
            debug: false,
        },
        // 14Mhz FULL TALL aperture
        DisplayAperture {
            w: TGA_APERTURE_FULL_W,
            h: TGA_APERTURE_FULL_H,
            x: TGA_APERTURE_FULL_X,
            y: TGA_APERTURE_FULL_TALL_Y,
            debug: false,
        },
        // 14Mhz DEBUG aperture
        DisplayAperture {
            w: TGA_APERTURE_DEBUG_W,
            h: TGA_APERTURE_DEBUG_H,
            x: 0,
            y: 0,
            debug: true,
        },
    ],
];

const CROPPED_STRING: &str = &formatcp!("Cropped: {}x{}", TGA_APERTURE_CROPPED_W, TGA_APERTURE_CROPPED_H);
const ACCURATE_STRING: &str = &formatcp!("Accurate: {}x{}", TGA_APERTURE_ACCURATE_W, TGA_APERTURE_ACCURATE_H);
const FULL_STRING: &str = &formatcp!("Full: {}x{}", TGA_APERTURE_FULL_W, TGA_APERTURE_FULL_H);
const DEBUG_STRING: &str = &formatcp!("Debug: {}x{}", TGA_APERTURE_DEBUG_W, TGA_APERTURE_DEBUG_H);

const TGA_APERTURE_DESCS: [DisplayApertureDesc; 4] = [
    DisplayApertureDesc {
        name: CROPPED_STRING,
        aper_enum: DisplayApertureType::Cropped,
    },
    DisplayApertureDesc {
        name: ACCURATE_STRING,
        aper_enum: DisplayApertureType::Accurate,
    },
    DisplayApertureDesc {
        name: FULL_STRING,
        aper_enum: DisplayApertureType::Full,
    },
    DisplayApertureDesc {
        name: DEBUG_STRING,
        aper_enum: DisplayApertureType::Debug,
    },
];

const TGA_DEFAULT_APERTURE: usize = 0;

macro_rules! trace {
    ($self:ident, $($t:tt)*) => {{
        if $self.trace_logger.is_some() {
            $self.trace_logger.print(&format!($($t)*));
            $self.trace_logger.print("\n".to_string());
        }
    }};
}

pub(crate) use trace;

macro_rules! trace_regs {
    ($self:ident) => {
        if $self.trace_logger.is_some() {
            $self.trace_logger.print(&format!(
                "[SL:{:03} HCC:{:03} VCC:{:03} VT:{:03} VS:{:03}] ",
                $self.scanline, $self.hcc_c0, $self.vcc_c4, $self.crtc_vertical_total, $self.crtc_vertical_sync_pos
            ));
        }
    };
}

use crate::devices::pic::Pic;
pub(crate) use trace_regs;

pub struct TGACard {
    subtype: VideoCardSubType,
    debug: bool,
    debug_draw: bool,
    cycles: u64,
    last_vsync_cycles: u64,
    cur_screen_cycles: u64,
    cycles_per_vsync: u64,
    sink_cycles: u32,
    catching_up: bool,

    last_rw_tick: u32,
    rw_slots: [RwSlot; 4],
    slot_idx: usize,

    enable_snow: bool,
    dirty_snow: bool,
    snow_char: u8,
    last_bus_value: u8,
    last_bus_addr: usize,
    snow_count: u64,

    mode_pending: bool,
    clock_pending: bool,
    mode_byte: u8,
    display_mode: DisplayMode,
    mode_enable: bool,
    mode_graphics: bool,
    mode_bw: bool,
    mode_bandwidth: bool,
    mode_hires_gfx: bool,
    mode_hires_txt: bool,
    mode_blinking: bool,
    mode_4bpp: bool,
    cc_palette: usize,
    cc_altcolor: u8,
    cc_overscan_color: u8,
    scanline_us: f64,
    frame_us: f64,

    cursor_frames: u32,

    frame_count:  u64,
    status_reads: u64,

    cursor_status: bool,
    cursor_slowblink: bool,
    cursor_blink_rate: f64,
    cursor_data: [bool; CGA_CURSOR_MAX],
    cursor_attr: u8,

    crtc_register_select_byte: u8,
    crtc_register_selected:    CRTCRegister,

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
    crtc_frame_address: usize,
    in_crtc_hblank: bool,
    in_crtc_vblank: bool,
    in_crtc_vsync: bool,
    in_last_vblank_line: bool,
    hborder: bool,
    vborder: bool,

    cc_register: u8,
    clock_divisor: u8, // Clock divisor is 1 in high resolution text mode, 2 in all other modes
    clock_mode: ClockingMode,
    char_clock: u32,
    char_clock_mask: u64,
    char_clock_odd_mask: u64,

    // Monitor stuff
    beam_x: u32,
    beam_y: u32,
    in_monitor_hsync: bool,
    in_monitor_vblank: bool,
    monitor_hsc: u32,
    scanline: u32,
    missed_hsyncs: u32,

    overscan_left: u32,
    overscan_right_start: u32,
    overscan_right: u32,
    vsync_len: u32,

    in_display_area: bool,
    cur_char: u8,    // Current character being drawn
    cur_attr: u8,    // Current attribute byte being drawn
    cur_fg: u8,      // Current glyph fg color
    cur_bg: u8,      // Current glyph bg color
    cur_blink: bool, // Current glyph blink attribute
    char_col: u8,    // Column of character glyph being drawn
    hcc_c0: u8,      // Horizontal character counter (x pos of character)
    vlc_c9: u8,      // Vertical line counter - row of character being drawn
    vcc_c4: u8,      // Vertical character counter (y pos of character)
    last_row: bool,  // Flag set on last character row of screen
    vsc_c3h: u8,     // Vertical sync counter - counts during vsync period
    hsc_c3l: u8,     // Horizontal sync counter - counts during hsync period
    vtac_c5: u8,
    in_vta: bool,
    effective_vta: u8,
    vma: usize,              // VMA register - Video memory address
    vma_t: usize,            // VMA' register - Video memory address temporary
    vmws: usize,             // Video memory word size
    rba: usize,              // Render buffer address
    blink_state: bool,       // Used to control blinking of cursor and text with blink attribute
    blink_accum_us: f64,     // Microsecond accumulator for blink state flipflop
    blink_accum_clocks: u32, // CGA Clock accumulator for blink state flipflop
    accumulated_us: f64,
    ticks_advanced: u32, // Number of ticks we have advanced mid-instruction via port or mmio access.
    pixel_clocks_owed: u32,
    ticks_accum: u32,
    clocks_accum: u32,

    //mem: Box<[u8; TGA_MEM_SIZE]>,
    back_buf: usize,
    front_buf: usize,
    extents: DisplayExtents,
    aperture: usize,
    //buf: Vec<Vec<u8>>,
    buf: [Box<[u8; CGA_MAX_CLOCK]>; 2],

    debug_color: u8,

    trace_logger:  TraceLogger,
    debug_counter: u64,

    lightpen_latch: bool,
    lightpen_addr:  usize,

    // TGA stuff
    do_vsync: bool,
    intr: bool,
    last_intr: bool,
    intr_enabled: bool,
    video_array_address: usize,
    palette_mask: u8,
    border_color: u8,
    t_mode_control: TModeControlRegister,
    jr_mode_control: JrModeControlRegister,
    jr_mode_control2: JrModeControlRegister2,
    mode_size: VideoModeSize,
    palette_registers: [u8; 16],
    page_register: TPageRegister,
    cpu_page_offset: usize,
    crt_page_offset: usize,
    page_size: usize,
    address_flipflop: bool,
    a0: u8,
    aperture_base: usize,
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
    LightPenPositionL,
}

// TGA implementation of Default for DisplayExtents.
// Each videocard implementation should implement sensible defaults.
// In TGA's case we know the maximum field size and thus row_stride.
trait TgaDefault {
    fn default() -> Self;
}
impl TgaDefault for DisplayExtents {
    fn default() -> Self {
        Self {
            apertures: TGA_APERTURES[0].to_vec(),
            field_w: CGA_XRES_MAX,
            field_h: CGA_YRES_MAX,
            row_stride: CGA_XRES_MAX as usize,
            double_scan: true,
            mode_byte: 0,
        }
    }
}

impl Default for TGACard {
    fn default() -> Self {
        Self {
            subtype: VideoCardSubType::Tandy1000,
            debug: false,
            debug_draw: true,
            cycles: 0,
            last_vsync_cycles: 0,
            cur_screen_cycles: 0,
            cycles_per_vsync: 0,
            sink_cycles: 0,
            catching_up: false,

            last_rw_tick: 0,
            rw_slots: [Default::default(); 4],
            slot_idx: 0,

            enable_snow: false,
            dirty_snow: true,
            snow_char: 0,
            last_bus_value: 0,
            last_bus_addr: 0,
            snow_count: 0,

            mode_byte: 0,
            mode_pending: false,
            clock_pending: false,
            display_mode: DisplayMode::Mode0TextBw40,
            mode_enable: true,
            mode_graphics: false,
            mode_bw: false,
            mode_bandwidth: false,
            mode_hires_gfx: false,
            mode_hires_txt: true,
            mode_blinking: true,
            mode_4bpp: false,
            cc_palette: 0,
            cc_altcolor: 0,
            cc_overscan_color: 0,
            frame_us: 0.0,

            cursor_frames: 0,
            scanline_us:   0.0,

            frame_count:  0,
            status_reads: 0,

            cursor_status: false,
            cursor_slowblink: false,
            cursor_blink_rate: CGA_DEFAULT_CURSOR_BLINK_RATE,
            cursor_data: [false; CGA_CURSOR_MAX],
            cursor_attr: 0,

            crtc_register_selected:    CRTCRegister::HorizontalTotal,
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
            crtc_maximum_scanline_address: DEFAULT_MAXIMUM_SCANLINE,
            crtc_cursor_start_line: DEFAULT_CURSOR_START_LINE,
            crtc_cursor_end_line: DEFAULT_CURSOR_END_LINE,
            crtc_start_address: 0,
            crtc_start_address_ho: 0,
            crtc_start_address_lo: 0,
            crtc_cursor_address_lo: 0,
            crtc_cursor_address_ho: 0,
            crtc_cursor_address: 0,
            crtc_frame_address: 0,

            in_crtc_hblank: false,
            in_crtc_vblank: false,
            in_crtc_vsync: false,
            in_last_vblank_line: false,
            hborder: true,
            vborder: true,

            cc_register: CC_PALETTE_BIT | CC_BRIGHT_BIT,

            clock_divisor: DEFAULT_CLOCK_DIVISOR,
            clock_mode: ClockingMode::Dynamic,
            char_clock: DEFAULT_CHAR_CLOCK,
            char_clock_mask: DEFAULT_CHAR_CLOCK_MASK,
            char_clock_odd_mask: DEFAULT_CHAR_CLOCK_ODD_MASK,
            beam_x: 0,
            beam_y: 0,
            in_monitor_hsync: false,
            in_monitor_vblank: false,
            monitor_hsc: 0,
            scanline: 0,
            missed_hsyncs: 0,

            overscan_left: 0,
            overscan_right_start: 0,
            overscan_right: 0,
            vsync_len: 0,
            in_display_area: false,
            cur_char: 0,
            cur_attr: 0,
            cur_fg: 0,
            cur_bg: 0,
            cur_blink: false,
            char_col: 0,
            hcc_c0: 0,
            vlc_c9: 0,
            vcc_c4: 0,
            last_row: false,
            vsc_c3h: 0,
            hsc_c3l: 0,
            vtac_c5: 0,
            in_vta: false,
            effective_vta: 0,
            vma: 0,
            vma_t: 0,
            vmws: 2,
            rba: 0,
            blink_state: false,
            blink_accum_us: 0.0,
            blink_accum_clocks: 0,

            accumulated_us: 0.0,
            ticks_advanced: 0,
            ticks_accum: 0,
            clocks_accum: 0,
            pixel_clocks_owed: 0,

            //mem: vec![0xFF; TGA_MEM_SIZE].into_boxed_slice().try_into().unwrap(),
            back_buf:  1,
            front_buf: 0,
            extents:   TgaDefault::default(),
            aperture:  TGA_DEFAULT_APERTURE,

            //buf: vec![vec![0; (CGA_XRES_MAX * CGA_YRES_MAX) as usize]; 2],

            // Theoretically, boxed arrays may have some performance advantages over
            // vectors due to having a fixed size known by the compiler.  However they
            // are a pain to initialize without overflowing the stack.
            buf: [
                vec![0; CGA_MAX_CLOCK].into_boxed_slice().try_into().unwrap(),
                vec![0; CGA_MAX_CLOCK].into_boxed_slice().try_into().unwrap(),
            ],

            debug_color: 0,

            trace_logger:  TraceLogger::None,
            debug_counter: 0,

            lightpen_latch: false,
            lightpen_addr:  0,

            // TGA stuff
            do_vsync: false,
            intr: false,
            last_intr: false,
            intr_enabled: true,
            video_array_address: 0x01,
            palette_mask: 0,
            border_color: 0,
            t_mode_control: TModeControlRegister::new(),
            jr_mode_control: JrModeControlRegister::new(),
            jr_mode_control2: JrModeControlRegister2::new(),
            mode_size: VideoModeSize::Mode16k,
            palette_registers: [0; 16],
            page_register: TPageRegister::new(),
            cpu_page_offset: 0,
            crt_page_offset: 0,
            page_size: 0x8000,
            address_flipflop: true,
            a0: 0,
            aperture_base: 0,
        }
    }
}

impl TGACard {
    pub fn new(
        subtype: VideoCardSubType,
        trace_logger: TraceLogger,
        clock_mode: ClockingMode,
        _video_frame_debug: bool,
    ) -> Self {
        let mut cga = TGACard {
            subtype,
            trace_logger,
            ..Self::default()
        };

        //cga.debug = video_frame_debug;

        if let ClockingMode::Default = clock_mode {
            cga.clock_mode = ClockingMode::Dynamic;
        }
        else {
            cga.clock_mode = clock_mode;
        }

        cga
    }

    /// Reset TGA state (on reboot, for example)
    fn reset_private(&mut self) {
        let trace_logger = std::mem::replace(&mut self.trace_logger, TraceLogger::None);

        // Save non-default values
        *self = Self {
            debug: self.debug,
            subtype: self.subtype,
            clock_mode: self.clock_mode,
            enable_snow: self.enable_snow,
            frame_count: self.frame_count, // Keep frame count as to not confuse frontend
            trace_logger,
            extents: self.extents.clone(),

            ..Self::default()
        }
    }

    #[inline(always)]
    pub fn set_a0(&mut self, byte: u8) {
        self.a0 = byte & 0x0F;
        if matches!(self.subtype, VideoCardSubType::Tandy1000) {
            // A0 register bits 0-3 specify the video aperture location in increments of 64K,
            // The aperture can be put anywhere in the 1MB address space, but of course putting
            // it outside existing RAM would not be very useful.
            self.aperture_base = (self.a0 as usize) << 16;
        }
    }

    fn rw_op(&mut self, ticks: u32, data: u8, addr: u32, rwtype: RwSlotType) {
        assert!(self.slot_idx < 4);

        self.rw_slots[self.slot_idx] = RwSlot {
            t: rwtype,
            data,
            addr,
            tick: ticks - self.last_rw_tick,
        };

        self.slot_idx += 1;
        self.last_rw_tick = ticks;
    }

    /*    fn catch_up(&mut self, delta: DeviceRunTimeUnit, debug: bool, cpumem: &[u8]) -> u32 {
            /*
            if self.sink_cycles > 0 {
                // Don't catch up when sinking;
                return
            }
            */

            // Catch up to CPU state.
            if let DeviceRunTimeUnit::SystemTicks(ticks) = delta {
                //log::debug!("Ticking {} clocks on IO read.", ticks);

                self.catching_up = true; // Setting this flag disables mode changes.

                let phase_offset = self.calc_phase_offset();

                // Can we squeeze a character update into the catch-up interval?
                if (ticks > phase_offset) && ((ticks - phase_offset) >= self.char_clock) {
                    //log::warn!("can afford character tick in catch_up()");

                    // Catch up to LCLOCK
                    for _ in 0..phase_offset {
                        self.tick();
                    }

                    if self.calc_phase_offset() != 0 {
                        log::error!("catch up failed: {} + {}", self.cycles, phase_offset);
                    }

                    // Tick a character
                    self.tick_char(cpumem);

                    // Tick any remaining cycles
                    for _ in 0..(ticks - phase_offset - self.char_clock as u32) {
                        self.tick();
                    }
                }
                else {
                    // Not enough ticks for a full character, just catch up
                    for _ in 0..ticks {
                        self.tick();
                    }
                }

                self.ticks_advanced += ticks; // must be +=
                self.pixel_clocks_owed = self.calc_cycles_owed();

                //assert!((self.cycles + self.pixel_clocks_owed as u64) & (CGA_LCHAR_CLOCK as u64) == 0);
                self.catching_up = false;

                if debug && self.rba < (CGA_MAX_CLOCK - 8) {
                    //log::debug!("crtc write!");
                    self.draw_solid_hchar(13);
                }
                return ticks;
            }
            0
        }
    */
    /// Update the number of pixel clocks we must execute before we can return to clocking the
    /// CGA card by character clock.  When an IO read/write occurs, the CGA card is updated to
    /// the current clock cycle by ticking pixels. During run() we then have to tick by pixels
    /// until we are back in phase with the character clock.
    #[inline]
    fn calc_cycles_owed(&mut self) -> u32 {
        if self.ticks_advanced % TGA_MCHAR_CLOCK as u32 > 0 {
            // We have advanced the CGA card out of phase with the character clock. Count
            // how many pixel clocks we need to tick by to be back in phase.
            ((!self.cycles + 1) & 0x0F) as u32
        }
        else {
            0
        }
    }

    #[inline]
    fn calc_phase_offset(&mut self) -> u32 {
        ((!self.cycles + 1) & 0x0F) as u32
    }

    fn set_lp_latch(&mut self) {
        if self.lightpen_latch == false {
            // Low to high transition of light pen latch, set latch addr.
            log::debug!("Updating lightpen latch address");
            self.lightpen_addr = self.vma;
        }

        self.lightpen_latch = true;
    }

    fn clear_lp_latch(&mut self) {
        log::debug!("clearing lightpen latch");
        self.lightpen_latch = false;
    }

    fn get_cursor_span(&self) -> (u8, u8) {
        (self.crtc_cursor_start_line, self.crtc_cursor_end_line)
    }

    /// Update the cursor data array based on the values of cursor_start_line, cursor_end_line, and
    /// crtc_maximum_scanline_address.
    fn update_cursor_data(&mut self) {
        // Reset cursor data to 0.
        self.cursor_data.fill(false);

        // Start line must be reached when iterating through character rows to draw a cursor at all.
        // Therefore if start_line > maximum_scanline, the cursor is disabled.
        if self.crtc_cursor_start_line > self.crtc_maximum_scanline_address {
            return;
        }

        if self.crtc_cursor_start_line <= self.crtc_cursor_end_line {
            // Normal cursor definition. Cursor runs from start_line to end_line.
            for i in self.crtc_cursor_start_line..=self.crtc_cursor_end_line {
                self.cursor_data[i as usize] = true;
            }
        }
        else {
            // "Split" cursor.
            for i in 0..=self.crtc_cursor_end_line {
                // First part of cursor is 0->end_line
                self.cursor_data[i as usize] = true;
            }

            for i in (self.crtc_cursor_start_line as usize)..CGA_CURSOR_MAX {
                // Second part of cursor is start_line->max
                self.cursor_data[i] = true;
            }
        }
    }

    fn get_cursor_address(&self) -> usize {
        self.crtc_cursor_address
    }

    /// Update the CRTC cursor address. Usually called after a CRTC register write updates the HO or LO byte.
    fn update_cursor_address(&mut self) {
        self.crtc_cursor_address = (self.crtc_cursor_address_ho as usize) << 8 | self.crtc_cursor_address_lo as usize
    }

    /// Update the CRTC start address. Usually called after a CRTC register write updates the HO or LO byte.
    fn update_start_address(&mut self) {
        // HO is already masked to 6 bits when set
        self.crtc_start_address = (self.crtc_start_address_ho as usize) << 8 | self.crtc_start_address_lo as usize;

        trace_regs!(self);
        trace!(self, "Start address updated: {:04X}", self.crtc_start_address)
    }

    fn get_cursor_status(&self) -> bool {
        self.cursor_status
    }

    fn handle_crtc_register_select(&mut self, byte: u8) {
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

    fn handle_crtc_register_write(&mut self, byte: u8) {
        //log::debug!("CGA: Write to CRTC register: {:?}: {:02}", self.crtc_register_selected, byte );
        match self.crtc_register_selected {
            CRTCRegister::HorizontalTotal => {
                // (R0) 8 bit write only
                self.crtc_horizontal_total = byte;
            }
            CRTCRegister::HorizontalDisplayed => {
                // (R1) 8 bit write only
                self.crtc_horizontal_displayed = byte;
            }
            CRTCRegister::HorizontalSyncPosition => {
                // (R2) 8 bit write only

                //if byte == 2 {
                //    log::debug!("R2=2, HCC: {}", self.hcc_c0);
                //}
                self.crtc_horizontal_sync_pos = byte;
            }
            CRTCRegister::SyncWidth => {
                // (R3) 8 bit write only

                if self.in_crtc_hblank {
                    log::warn!("Warning: SyncWidth modified during hsync!");
                }
                self.crtc_sync_width = byte;
            }
            CRTCRegister::VerticalTotal => {
                // (R4) 7 bit write only
                self.crtc_vertical_total = byte & 0x7F;

                trace_regs!(self);
                trace!(
                    self,
                    "CRTC Register Write (04h): VerticalTotal updated: {}",
                    self.crtc_vertical_total
                )
            }
            CRTCRegister::VerticalTotalAdjust => {
                // (R5) 5 bit write only
                self.crtc_vertical_total_adjust = byte & 0x1F;
            }
            CRTCRegister::VerticalDisplayed => {
                // (R6) 7 bit write only
                self.crtc_vertical_displayed = byte;
            }
            CRTCRegister::VerticalSync => {
                // (R7) 7 bit write only
                self.crtc_vertical_sync_pos = byte & 0x7F;

                trace_regs!(self);
                trace!(
                    self,
                    "CRTC Register Write (07h): VerticalSync updated: {}",
                    self.crtc_vertical_sync_pos
                )
            }
            CRTCRegister::InterlaceMode => {
                self.crtc_interlace_mode = byte;
            }
            CRTCRegister::MaximumScanLineAddress => {
                self.crtc_maximum_scanline_address = byte;
                self.update_cursor_data();
            }
            CRTCRegister::CursorStartLine => {
                self.crtc_cursor_start_line = byte & CURSOR_LINE_MASK;
                self.cursor_attr = (byte & CURSOR_ATTR_MASK) >> 5;

                match (byte & CURSOR_ATTR_MASK) >> 5 {
                    0b00 => {
                        self.cursor_status = true;
                        self.cursor_slowblink = false;
                    }
                    0b01 => {
                        self.cursor_status = false;
                        self.cursor_slowblink = false;
                    }
                    0b10 => {
                        self.cursor_status = true;
                        self.cursor_slowblink = false;
                    }
                    _ => {
                        self.cursor_status = true;
                        self.cursor_slowblink = true;
                    }
                }

                self.update_cursor_data();
            }
            CRTCRegister::CursorEndLine => {
                self.crtc_cursor_end_line = byte & CURSOR_LINE_MASK;
                self.update_cursor_data();
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
                // Start Address HO register is only 6 bits wide.
                // Entire Start Address register is 14 bits.
                self.crtc_start_address_ho = byte & 0x3F;
                trace_regs!(self);
                trace!(self, "CRTC Register Write (0Ch): StartAddressH updated: {:02X}", byte);
                self.update_start_address();
            }
            CRTCRegister::StartAddressL => {
                self.crtc_start_address_lo = byte;
                trace_regs!(self);
                trace!(self, "CRTC Register Write (0Dh): StartAddressL updated: {:02X}", byte);
                self.update_start_address();
            }
            _ => {
                trace!(
                    self,
                    "Write to unsupported CRTC register {:?}: {:02X}",
                    self.crtc_register_selected,
                    byte
                );
                log::warn!(
                    "CGA: Write to unsupported CRTC register {:?}: {:02X}",
                    self.crtc_register_selected,
                    byte
                );
            }
        }
    }

    fn handle_crtc_register_read(&mut self) -> u8 {
        match self.crtc_register_selected {
            CRTCRegister::CursorStartLine => self.crtc_cursor_start_line,
            CRTCRegister::CursorEndLine => self.crtc_cursor_end_line,
            CRTCRegister::CursorAddressH => {
                //log::debug!("CGA: Read from CRTC register: {:?}: {:02}", self.crtc_register_selected, self.crtc_cursor_address_ho );
                self.crtc_cursor_address_ho
            }
            CRTCRegister::CursorAddressL => {
                //log::debug!("CGA: Read from CRTC register: {:?}: {:02}", self.crtc_register_selected, self.crtc_cursor_address_lo );
                self.crtc_cursor_address_lo
            }
            CRTCRegister::LightPenPositionL => {
                let byte = (self.lightpen_addr & 0xFF) as u8;
                log::debug!("read LpL: {:02X}", byte);
                byte
            }
            CRTCRegister::LightPenPositionH => {
                let byte = ((self.lightpen_addr >> 8) & 0x3F) as u8;
                log::debug!("read LpH: {:02X}", byte);
                byte
            }
            _ => {
                log::debug!(
                    "CGA: Read from unsupported CRTC register: {:?}",
                    self.crtc_register_selected
                );
                0
            }
        }
    }

    /// Return true if the pending mode change defined by mode_byte would change from text mode to
    /// graphics mode, or vice-versa
    fn is_deferred_mode_change(&self, new_mode_byte: u8) -> bool {
        // In general, we can determine whether we are in graphics mode or text mode by
        // checking the graphics bit, however, the graphics bit is allowed to coexist with the
        // HIRES_TEXT bit in an undocumented combination that remains in text mode but allows
        // a selectable background color.

        // If both graphics and hi-res text bits are set, we are still in text mode
        let old_text_mode = (self.mode_byte & 0b01 != 0) || (self.mode_byte & 0b11 == 0b11);
        let old_graphics_mode = self.mode_byte & 0b10 != 0;

        // If both graphics and hi-res text bits are set, we are still in text mode
        let new_text_mode = (new_mode_byte & 0b01 != 0) || (new_mode_byte & 0b11 == 0b11);
        let new_graphics_mode = new_mode_byte & 0b10 != 0;

        // Determine the effective mode for self.mode_byte
        let old_mode_is_graphics = old_graphics_mode && !old_text_mode;
        let old_mode_is_text = !old_mode_is_graphics; // This includes high-resolution text mode and normal text mode

        // Determine the effective mode for new_mode_byte
        let new_mode_is_graphics = new_graphics_mode && !new_text_mode;
        let new_mode_is_text = !new_mode_is_graphics; // This includes high-resolution text mode and normal text mode

        // Return true if the mode is changing between text and graphics or vice versa
        (old_mode_is_text && new_mode_is_graphics) || (old_mode_is_graphics && new_mode_is_text)
    }

    /// Update the CGA graphics mode. This function may be called some time after the mode
    /// register is actually written to, depending on if we are changing from text to graphics mode
    /// or vice versa.
    fn update_mode(&mut self) {
        // Will this mode change the character clock?
        let clock_changed = self.mode_hires_txt != (self.mode_byte & MODE_HIRES_TEXT != 0);

        if clock_changed {
            // Flag the clock for pending change.  The clock can only be changed in phase with
            // LCHAR due to our dynamic clocking logic.
            self.clock_pending = true;
        }

        match self.subtype {
            VideoCardSubType::IbmPCJr => {
                //self.mode_hires_txt = !self.jr_mode_control.graphics() && self.jr_mode_control.bandwidth();

                self.mode_hires_txt = self.jr_mode_control.bandwidth();
                self.mode_graphics = self.jr_mode_control.graphics();
                self.mode_bw = self.jr_mode_control.bw();
                self.mode_enable = self.jr_mode_control.video();
                self.mode_hires_gfx = self.jr_mode_control.graphics()
                    && self.jr_mode_control.bandwidth()
                    && !self.jr_mode_control.fourbpp_mode();
                self.mode_blinking = self.jr_mode_control2.blink();

                // Use color control register value for overscan unless high-res graphics mode,
                // in which case overscan must be black (0).
                self.cc_overscan_color = if self.mode_hires_gfx { 0 } else { self.border_color };

                // Reinterpret the CC register based on new mode.
                self.update_palette();

                // Attempt to update clock.
                self.update_clock();

                // The PCjr doesn't have a traditional CGA mode register, so Synthesize a virtual mode byte
                let mut vmode_byte = 0;
                if self.mode_hires_txt && !self.mode_graphics {
                    // The PCjr's 'hi-res' bit is not specific to text mode. It can also indicate hi-res
                    // 2bpp mode. So only set the hi-res text bit if graphics bit is off.
                    vmode_byte |= MODE_HIRES_TEXT;
                }
                if self.mode_graphics {
                    vmode_byte |= MODE_GRAPHICS;
                }
                if self.mode_bw {
                    vmode_byte |= MODE_BW;
                }
                if self.mode_enable {
                    vmode_byte |= MODE_ENABLE;
                }
                if self.mode_hires_gfx {
                    vmode_byte |= MODE_HIRES_GRAPHICS;
                }
                if self.mode_4bpp {
                    // Replaces blinking bit
                    vmode_byte |= VMODE_4BPP;
                }
                self.display_mode = match vmode_byte & CGA_MODE_ENABLE_MASK {
                    0b00_0100 => DisplayMode::Mode0TextBw40,
                    0b00_0000 => DisplayMode::Mode1TextCo40,
                    0b00_0101 => DisplayMode::Mode2TextBw80,
                    0b00_0001 => DisplayMode::Mode3TextCo80,
                    0b00_0011 => DisplayMode::ModeTextAndGraphicsHack,
                    0b00_0010 => DisplayMode::Mode4LowResGraphics,
                    0b00_0110 => DisplayMode::Mode5LowResAltPalette,
                    0b01_0110 => DisplayMode::Mode6HiResGraphics,
                    0b01_0010 => DisplayMode::Mode6HiResGraphics,
                    0b10_0010 => DisplayMode::Mode8LowResGraphics16,
                    _ => {
                        trace!(
                            self,
                            "Invalid display mode selected: {:02X}",
                            vmode_byte & CGA_MODE_ENABLE_MASK
                        );
                        log::warn!(
                            "TGA: Invalid display mode selected: {:02X}",
                            vmode_byte & CGA_MODE_ENABLE_MASK
                        );
                        DisplayMode::Mode3TextCo80
                    }
                };
            }
            VideoCardSubType::Tandy1000 => {
                self.mode_hires_txt = self.mode_byte & MODE_HIRES_TEXT != 0;
                self.mode_graphics = self.mode_byte & MODE_GRAPHICS != 0;
                self.mode_bw = self.mode_byte & MODE_BW != 0;
                self.mode_enable = self.mode_byte & MODE_ENABLE != 0;
                self.mode_hires_gfx = self.mode_byte & MODE_HIRES_GRAPHICS != 0;
                self.mode_blinking = self.mode_byte & MODE_BLINKING != 0;

                // Use color control register value for overscan unless high-res graphics mode,
                // in which case overscan must be black (0).
                self.cc_overscan_color = if self.mode_hires_gfx { 0 } else { self.border_color };

                // Reinterpret the CC register based on new mode.
                self.update_palette();

                // Attempt to update clock.
                self.update_clock();

                let mut vmode_byte = self.mode_byte;
                if self.mode_4bpp {
                    // Replaces blinking bit
                    vmode_byte |= VMODE_4BPP;
                }
                // Updated mask to exclude the enable bit in mode calculation.
                // "Disabled" isn't really a video mode, it just controls whether
                // the CGA card outputs video at a given moment. This can be toggled on
                // and off during a single frame, such as done in VileR's fontcmp.com
                self.display_mode = match vmode_byte & CGA_MODE_ENABLE_MASK {
                    0b00_0100 => DisplayMode::Mode0TextBw40,
                    0b00_0000 => DisplayMode::Mode1TextCo40,
                    0b00_0101 => DisplayMode::Mode2TextBw80,
                    0b00_0001 => DisplayMode::Mode3TextCo80,
                    0b00_0011 => DisplayMode::ModeTextAndGraphicsHack,
                    0b00_0010 => DisplayMode::Mode4LowResGraphics,
                    0b00_0110 => DisplayMode::Mode5LowResAltPalette,
                    0b01_0110 => DisplayMode::Mode6HiResGraphics,
                    0b01_0010 => DisplayMode::Mode6HiResGraphics,
                    0b10_0010 => DisplayMode::Mode8LowResGraphics16,
                    _ => {
                        trace!(self, "Invalid display mode selected: {:02X}", self.mode_byte & 0x1F);
                        log::warn!("CGA: Invalid display mode selected: {:02X}", self.mode_byte & 0x1F);
                        DisplayMode::Mode3TextCo80
                    }
                };
            }
            _ => {
                panic!("Bad TGA subtype!");
            }
        }

        trace_regs!(self);
        trace!(
            self,
            "Display mode set: {:?}. Mode byte: {:02X} Enabled: {} Clock: {}",
            self.display_mode,
            self.mode_byte,
            self.mode_enable,
            self.clock_divisor
        );

        /* Disabled debug due to noise. Some effects in Area 5150 write mode many times per frame

        log::debug!("CGA: Mode Selected ({:?}:{:02X}) Enabled: {} Clock: {}",
            self.display_mode,
            self.mode_byte,
            self.mode_enable,
            self.clock_divisor
        );
        */
    }

    /// Update the CGA character clock. Can only be done on LCLOCK boundaries to simplify
    /// our logic.
    #[inline]
    fn update_clock(&mut self) {
        match self.subtype {
            VideoCardSubType::IbmPCJr => {
                self.update_clock_tandy();
            }
            VideoCardSubType::Tandy1000 => {
                self.update_clock_tandy();
            }
            _ => {
                panic!("Bad TGA subtype!");
            }
        }
    }

    #[inline]
    fn update_clock_tandy(&mut self) {
        // Wait to update clock until we are in phase with LCHAR.
        if self.clock_pending && (self.cycles & 0x1F == 0) {
            // Clock divisor is 1 in high-res text mode, 2 in medium-res graphics mode,
            // and 4 in low-res graphics mode.
            // We draw pixels twice when clock divisor is 2 and four times when clock divisor is 4.
            (
                self.clock_divisor,
                self.char_clock,
                self.char_clock_mask,
                self.char_clock_odd_mask,
                self.mode_size,
            ) = match (self.mode_graphics, self.mode_hires_txt, self.mode_4bpp) {
                (false, false, false) => {
                    // Low-res text mode (40x25)
                    (2, TGA_MCHAR_CLOCK as u32, 0x0F, 0x1F, VideoModeSize::Mode16k)
                }
                (false, false, true) => {
                    log::warn!("Invalid graphics mode configured. Clock divisor guessed (2)");
                    (2, TGA_MCHAR_CLOCK as u32, 0x0F, 0x1F, VideoModeSize::Mode16k)
                }
                (false, true, false) => {
                    // High-res text mode (80x25)
                    (1, TGA_HCHAR_CLOCK as u32, 0x07, 0x0F, VideoModeSize::Mode16k)
                }
                (false, true, true) => {
                    // High-res bit in 4bpp mode toggles between clock divisors 2 and 4.
                    (2, TGA_MCHAR_CLOCK as u32, 0x0F, 0x1F, VideoModeSize::Mode16k)
                }
                (true, false, false) => {
                    // Medium-res 320x200, 2bpp graphics.
                    (2, TGA_MCHAR_CLOCK as u32, 0x0F, 0x1F, VideoModeSize::Mode16k)
                }
                (true, false, true) => {
                    // Low-res 160x200, 4bpp graphics.
                    (4, TGA_LCHAR_CLOCK as u32, 0x1F, 0x3F, VideoModeSize::Mode16k)
                }
                (true, true, false) => {
                    // High-res 620x200, 2bpp graphics.
                    (1, TGA_HCHAR_CLOCK as u32, 0x07, 0x0F, VideoModeSize::Mode32k)
                }
                (true, true, true) => {
                    // Medium-res 320x200, 4bpp graphics.
                    (2, TGA_MCHAR_CLOCK as u32, 0x0F, 0x1F, VideoModeSize::Mode32k)
                }
            };

            self.clock_pending = false;
        }
    }

    /// Handle a write to the CGA mode register. Defer the mode change if it would change
    /// from graphics mode to text mode or back (Need to measure this on real hardware)
    fn handle_mode_register(&mut self, mode_byte: u8) {
        self.mode_byte = mode_byte;
        if self.is_deferred_mode_change(mode_byte) {
            // Latch the mode change and mark it pending. We will change the mode on next hsync.
            log::trace!("deferring mode change.");
            self.mode_pending = true;
        }
        else {
            // We're not changing from text to graphics or vice versa, so we do not have to
            // defer the update.
            self.update_mode();
        }
    }

    /// Handle a read from the CGA status register. This register has bits to indicate whether
    /// we are in vblank or if the display is in the active display area (enabled)
    fn handle_status_register_read(&mut self) -> u8 {
        self.status_reads += 1;

        if self.in_crtc_vblank {
            trace!(self, "in vblank: vsc: {:03}", self.vsc_c3h);
        }

        let byte = self.calc_status_register();

        trace_regs!(self);
        trace!(
            self,
            "Status register read: byte: {:02X} in_display_area: {} vblank: {} ",
            byte,
            self.in_display_area,
            self.in_crtc_vblank
        );

        // The PCJr's vga address flip-flop is reset on status register reads.
        self.address_flipflop = true;

        byte
    }

    fn calc_status_register(&self) -> u8 {
        // Bit 1 of the status register is tied to the 'Display Enable' line from the 6845, inverted.
        // Thus, it will be 1 when the TGA card is not currently scanning, IE during both horizontal
        // and vertical refresh.
        // https://www.vogons.org/viewtopic.php?t=47052

        // Base register value is now 0xE0 to make room for mux bit on TGA
        let mut byte = if self.in_crtc_vblank {
            //0xF0 | STATUS_VERTICAL_RETRACE | STATUS_DISPLAY_ENABLE
            0xE0 | STATUS_VERTICAL_RETRACE
        }
        else if self.in_display_area {
            0xE1
        }
        else {
            0xE0
        };

        if self.lightpen_latch {
            //log::debug!("returning status read with trigger set");
            byte |= STATUS_LIGHTPEN_TRIGGER_SET;
        }

        // This bit is logically reversed, i.e., 0 is switch on
        //byte |= STATUS_LIGHTPEN_SWITCH_STATUS;

        // Video MUX bits for TGA/PCJr.
        // The PCJr POST tests the mux bit by drawing a line of full-block characters to the top row
        // of the screen. We can essentially fake the mux bits by returning 1 when we are in the top
        // 8 scanlines.
        if self.beam_y < 8 {
            byte |= STATUS_VIDEO_MUX;
        }

        byte
    }

    /// Handle write to the Color Control register. This register controls the palette selection
    /// and background/overscan color (foreground color in high-res graphics mode)
    fn handle_cc_register_write(&mut self, data: u8) {
        self.cc_register = data;
        self.update_palette();

        log::trace!("Write to color control register: {:02X}", data);
    }

    fn update_palette(&mut self) {
        if self.mode_bw && self.mode_graphics && !self.mode_hires_gfx {
            self.cc_palette = 4; // Select Red, Cyan and White palette (undocumented)
        }
        else if self.cc_register & CC_PALETTE_BIT != 0 {
            self.cc_palette = 2; // Select Magenta, Cyan, White palette
        }
        else {
            self.cc_palette = 0; // Select Red, Green, 'Yellow' palette
        }

        if self.cc_register & CC_BRIGHT_BIT != 0 {
            self.cc_palette += 1; // Switch to high-intensity palette
        }

        self.cc_altcolor = self.cc_register & 0x0F;

        if !self.mode_hires_gfx {
            self.cc_overscan_color = self.cc_altcolor;
        }
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

        self.buf[self.back_buf].fill(0);
    }

    /// Return the bit value at (col,row) of the given font glyph
    fn get_glyph_bit(glyph: u8, mut col: u8, row: u8) -> bool {
        debug_assert!(col < TGA_HCHAR_CLOCK);
        if TGACard::is_box_char(glyph) {
            col = if col > 7 { 7 } else { col };
        }
        let row_masked = row & 0x7;

        // Calculate byte offset
        let glyph_offset: usize = (row_masked as usize * TGA_FONT_SPAN) + glyph as usize;
        TGA_FONT[glyph_offset] & (0x01 << (7 - col)) != 0
    }

    #[inline]
    pub fn is_box_char(glyph: u8) -> bool {
        (0xB0u8..=0xDFu8).contains(&glyph)
    }

    /// Set the character attributes for the current character.
    /// This applies to text mode only, but is computed in all modes at appropriate times.
    fn set_char_addr(&mut self, cpu_mem: &[u8]) {
        // Address from CRTC is masked by 0x1FFF by the CGA card (bit 13 ignored) and doubled.
        let addr = (self.vma & CGA_TEXT_MODE_WRAP) << 1;

        // Generate snow if we are in hires mode, have a dirty bus, and HCLOCK is odd
        if self.enable_snow && self.mode_hires_txt && self.dirty_snow && (self.cycles & 0b1000 != 0) {
            self.cur_char = self.snow_char;
            self.cur_attr = self.last_bus_value;
            self.dirty_snow = false;
            self.snow_count += 1;
        }
        else {
            // No snow
            self.cur_char = self.crt_mem(cpu_mem)[addr];
            self.cur_attr = self.crt_mem(cpu_mem)[addr + 1];
        }

        self.cur_fg = self.cur_attr & 0x0F;

        // If blinking is enabled, the bg attribute is only 3 bits and only low-intensity colors
        // are available.
        // If blinking is disabled, all 16 colors are available as background attributes.
        if self.mode_blinking {
            self.cur_bg = (self.cur_attr >> 4) & 0x07;
            self.cur_blink = self.cur_attr & 0x80 != 0;
        }
        else {
            self.cur_bg = self.cur_attr >> 4;
            self.cur_blink = false;
        }

        self.dirty_snow = false;

        //(self.cur_fg, self.cur_bg) = ATTRIBUTE_TABLE[self.cur_attr as usize];
    }

    /// Get the 64-bit value representing the specified row of the specified character
    /// glyph in high-resolution text mode.
    #[inline]
    pub fn get_hchar_glyph_row(&self, glyph: usize, mut row: usize) -> u64 {
        if self.cur_blink && !self.blink_state {
            CGA_COLORS_U64[self.cur_bg as usize]
        }
        else {
            if TGACard::is_box_char(glyph as u8) {
                row = if row > 7 { 7 } else { row };
            }
            let glyph_row_base = TGA_HIRES_GLYPH_TABLE[glyph & 0xFF][row];

            // Combine glyph mask with foreground and background colors.
            glyph_row_base & CGA_COLORS_U64[self.cur_fg as usize]
                | !glyph_row_base & CGA_COLORS_U64[self.cur_bg as usize]
        }
    }

    /// Get a tuple of 64-bit values representing the specified row of the specified character
    /// glyph in low-resolution (40-column) mode.
    #[inline]
    pub fn get_mchar_glyph_rows(&self, glyph: usize, mut row: usize) -> (u64, u64) {
        if self.cur_blink && !self.blink_state {
            let glyph = CGA_COLORS_U64[self.cur_bg as usize];
            (glyph, glyph)
        }
        else {
            if TGACard::is_box_char(glyph as u8) {
                row = if row > 7 { 7 } else { row };
            }
            let glyph_row_base_0 = TGA_LOWRES_GLYPH_TABLE[glyph & 0xFF][0][row];
            let glyph_row_base_1 = TGA_LOWRES_GLYPH_TABLE[glyph & 0xFF][1][row];

            // Combine glyph mask with foreground and background colors.
            let glyph0 = glyph_row_base_0 & CGA_COLORS_U64[self.cur_fg as usize]
                | !glyph_row_base_0 & CGA_COLORS_U64[self.cur_bg as usize];
            let glyph1 = glyph_row_base_1 & CGA_COLORS_U64[self.cur_fg as usize]
                | !glyph_row_base_1 & CGA_COLORS_U64[self.cur_bg as usize];

            (glyph0, glyph1)
        }
    }

    /*
    pub fn draw_text_mode_char(&mut self) {

        let draw_span = (8 * self.clock_divisor) as usize;

        // Do cursor if visible, enabled and defined
        if     self.vma == self.crtc_cursor_address
            && self.cursor_status
            && self.blink_state
            && self.cursor_data[(self.vlc_c9 & 0x1F) as usize]
        {
            self.draw_solid_char(self.cur_fg);
        }
        else if self.mode_enable {
            for i in (0..draw_span).step_by(self.clock_divisor as usize) {
                let new_pixel = match CGACard::get_glyph_bit(self.cur_char, (i as u8 / self.clock_divisor), self.vlc_c9) {
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

                self.buf[self.back_buf][self.rba + i] = new_pixel;
                if self.clock_divisor == 2 {
                    // Double pixels in 40 column mode.
                    self.buf[self.back_buf][self.rba + i + 1] = new_pixel;
                }
            }
        }
        else {
            // When mode bit is disabled in text mode, the CGA acts like VRAM is all 0.
            self.draw_solid_char(0);
        }
    }
    */

    pub fn get_lowres_pixel_color(&self, row: u8, col: u8, cpumem: &[u8]) -> u8 {
        let base_addr = self.get_gfx_addr(row);

        let word = (self.crt_mem(cpumem)[base_addr] as u16) << 8 | self.crt_mem(cpumem)[base_addr + 1] as u16;

        let idx = ((word >> (TGA_MCHAR_CLOCK - (col + 1) * 2)) & 0x03) as usize;

        if idx == 0 {
            self.cc_altcolor
        }
        else {
            CGA_PALETTES[self.cc_palette][idx]
        }
    }

    /// Look up the low res graphics glyphs and masks for the current lo-res graphics mode
    /// byte (vma)
    #[inline]
    pub fn get_lowres_gfx_mchar(&self, row: u8, cpumem: &[u8]) -> (&(u64, u64), &(u64, u64)) {
        let base_addr = self.get_gfx_addr(row);
        (
            &CGA_LOWRES_GFX_TABLE[self.cc_palette as usize][self.crt_mem(cpumem)[base_addr] as usize],
            &CGA_LOWRES_GFX_TABLE[self.cc_palette as usize][self.crt_mem(cpumem)[base_addr + 1] as usize],
        )
    }

    #[inline]
    pub fn get_gfx_addr(&self, row: u8) -> usize {
        match self.mode_size {
            VideoModeSize::Mode16k => self.get_gfx_addr_16k(row),
            VideoModeSize::Mode32k => self.get_gfx_addr_32k(row),
        }
    }

    /// Calculate the byte address given the current value of vma; given that the address
    /// programmed into the CRTC start register is interpreted by the CGA as a word address.
    /// In graphics mode, the row counter determines whether address line A12 from the
    /// CRTC is set. This effectively creates a 0x2000 byte offset for odd character rows.
    #[inline]
    pub fn get_gfx_addr_16k(&self, row: u8) -> usize {
        let row_offset = (row as usize & 0x01) << 12;
        let addr = (self.vma & 0x0FFF | row_offset) << 1;
        addr
    }

    /// Calculate the byte address given the current value of vma; given that the address
    /// programmed into the CRTC start register is interpreted by the CGA as a dword address.
    /// In 4bpp graphics mode, the two lowest bits of the row counter determine whether how the
    /// address lines from the CRTC are modified
    /// This effectively creates 4 banks of video memory at 8k intervals.
    #[inline]
    pub fn get_gfx_addr_32k(&self, row: u8) -> usize {
        let row_offset = (row as usize & 0x03) << 12;
        let addr = (self.vma & 0x0FFF | row_offset) << 1;
        addr
    }

    pub fn get_screen_ticks(&self) -> u64 {
        self.cur_screen_cycles
    }

    /*

    /// Execute one CGA character.
    pub fn tick_char(&mut self) {

        // sink_cycles must be a factor of 8
        //assert!((self.sink_cycles & 0x07) == 0);

        if self.sink_cycles & 0x07 != 0 {
            log::error!("sink_cycles: {} not divisible by 8", self.sink_cycles);
        }

        if self.sink_cycles > 0 {
            self.sink_cycles = self.sink_cycles.saturating_sub(8);
            return
        }

        self.cycles += 8;
        self.cur_screen_cycles += 8;

        // Don't execute even character clocks in low-res mode
        if self.clock_divisor == 2 && (self.cycles & 0x0F == 0) {
            log::trace!("skipping odd hchar: {:X}", self.cycles);
            return
        }

        // Only draw if marty_render buffer address is in bounds.
        if self.rba < (CGA_MAX_CLOCK - (8 * self.clock_divisor) as usize) {
            if self.in_display_area {
                // Draw current character row
                if !self.mode_graphics {
                    self.draw_text_mode_char();
                }
                else if self.mode_hires_gfx {
                    self.draw_hires_gfx_mode_char();
                }
                else {
                    self.draw_lowres_gfx_mode_char();
                }
            }
            else if self.in_crtc_hblank {
                // Draw hblank in debug color
                self.draw_solid_char(self.hblank_color);
            }
            else if self.in_crtc_vblank {
                // Draw vblank in debug color
                self.draw_solid_char(self.vblank_color);
            }
            else if self.vborder | self.hborder {
                // Draw overscan
                self.draw_solid_char(self.cc_overscan_color);
            }
            else {
                log::warn!("invalid display state...");
            }
        }

        // Update position to next pixel and character column.
        self.beam_x += 8 * self.clock_divisor as u32;
        self.rba += 8 * self.clock_divisor as usize;

        // If we have reached the right edge of the 'monitor', return the raster position
        // to the left side of the screen.
        if self.beam_x == CGA_XRES_MAX {
            self.beam_x = 0;
            self.beam_y += 1;
            self.in_monitor_hsync = false;
            self.rba = (CGA_XRES_MAX * self.beam_y) as usize;
        }

        self.tick_crtc_char();
    }
    */

    /// Execute a hires or lowres character clock as appropriate.
    #[inline]
    pub fn tick_char(&mut self, pic: &mut Option<Box<Pic>>, cpumem: &[u8]) {
        match self.clock_divisor {
            1 => self.tick_hchar(cpumem),
            2 => self.tick_mchar(cpumem),
            4 => self.tick_lchar(cpumem),
            _ => {
                log::error!("Invalid clock divisor: {}", self.clock_divisor);
                panic!("Invalid clock divisor: {}", self.clock_divisor);
            }
        }

        if self.intr && !self.last_intr {
            // Rising edge of INTR - raise IRQ5
            if let Some(pic) = pic {
                pic.request_interrupt(TGA_IRQ);
            }
        }
        else if !self.intr {
            // Falling edge of INTR - release IRQ5
            if let Some(pic) = pic {
                //log::debug!("clearing irq2!");
                pic.clear_interrupt(TGA_IRQ);
            }
        }
        self.last_intr = self.intr;
    }

    /// Execute one high resolution character clock.
    pub fn tick_hchar(&mut self, cpumem: &[u8]) {
        // sink_cycles must be a factor of 8
        // assert_eq!(self.sink_cycles & 0x07, 0);

        if self.sink_cycles & 0x07 != 0 {
            log::warn!("sink_cycles: {} not divisible by 8", self.sink_cycles);
        }

        if self.sink_cycles > 0 {
            self.sink_cycles = self.sink_cycles.saturating_sub(8);
            return;
        }

        // Cycles must be a factor of 8 and char_clock == 8
        assert_eq!(self.cycles & 0x07, 0);
        assert_eq!(self.char_clock, 8);

        self.cycles += 8;
        self.cur_screen_cycles += 8;

        // Only draw if marty_render buffer address is in bounds.
        if self.rba < (CGA_MAX_CLOCK - 8) {
            if self.in_display_area {
                // Draw current character row
                if !self.mode_graphics {
                    self.draw_text_mode_hchar();
                }
                else if self.mode_hires_gfx {
                    //self.draw_hires_gfx_mode_char(cpumem);
                    //self.draw_solid_hchar(CGA_HBLANK_DEBUG_COLOR);
                    self.draw_gfx_mode_2bpp_hchar(cpumem);
                }
                else {
                    self.draw_solid_hchar(CGA_VBLANK_DEBUG_COLOR);
                    //self.draw_gfx_mode_2bpp_mchar(cpumem);
                }
            }
            else if self.in_crtc_hblank {
                // Draw hblank in debug color
                if self.debug_draw {
                    self.draw_solid_hchar(CGA_HBLANK_DEBUG_COLOR);
                }
            }
            else if self.in_crtc_vblank {
                // Draw vblank in debug color
                if self.debug_draw {
                    self.draw_solid_hchar(CGA_VBLANK_DEBUG_COLOR);
                }
            }
            else if self.vborder | self.hborder {
                // Draw overscan
                if self.debug_draw {
                    self.draw_solid_hchar(self.cc_overscan_color);
                    //self.draw_solid_hchar(CGA_OVERSCAN_DEBUG_COLOR);
                }
                else {
                    self.draw_solid_hchar(self.cc_overscan_color);
                }
            }
            else {
                self.draw_solid_hchar(CGA_DEBUG2_COLOR);
                //log::warn!("invalid display state...");
                //self.dump_status();
                //panic!("invalid display state...");
            }

            /*            if self.in_vta {
                self.draw_solid_hchar(CGA_DEBUG2_COLOR);
            }*/
        }

        // Update position to next pixel and character column.
        self.beam_x += 8 * self.clock_divisor as u32;
        self.rba += 8 * self.clock_divisor as usize;

        // If we have reached the right edge of the 'monitor', return the raster position
        // to the left side of the screen.
        if self.beam_x >= CGA_XRES_MAX {
            self.beam_x = 0;
            self.beam_y += 1;
            self.in_monitor_hsync = false;
            self.rba = (CGA_XRES_MAX * self.beam_y) as usize;
        }

        if self.cycles & self.char_clock_mask != 0 {
            log::error!(
                "tick_hchar(): calling tick_crtc_char but out of phase with cclock: cycles: {} mask: {}",
                self.cycles,
                self.char_clock_mask
            );
        }

        if self.update_char_tick(cpumem) && self.intr_enabled() {
            self.intr = true;
        }
        else if !self.in_crtc_vblank {
            self.intr = false;
        }

        self.char_col = 0;
        self.update_clock();
    }

    /// Execute one medium resolution (7Mhz) character clock.
    pub fn tick_mchar(&mut self, cpumem: &[u8]) {
        // Cycles must be a factor of 16 and char_clock == 16
        assert_eq!(self.cycles & 0x0F, 0);
        assert_eq!(self.char_clock, 16);

        // sink_cycles must be a factor of 8
        //assert!((self.sink_cycles & 0x07) == 0);

        /*
        if self.sink_cycles & 0x0F != 0 {
            log::error!("sink_cycles: {} not divisible by 16", self.sink_cycles);
        }
        */

        if self.sink_cycles > 0 {
            self.sink_cycles = self.sink_cycles.saturating_sub(16);
            return;
        }

        self.cycles += 16;
        self.cur_screen_cycles += 16;

        // Only draw if marty_render buffer address is in bounds.
        if self.rba < (CGA_MAX_CLOCK - 16) {
            if self.in_display_area {
                // Draw current character row
                if !self.mode_graphics {
                    self.draw_text_mode_mchar();
                }
                else if self.mode_hires_gfx {
                    self.draw_gfx_mode_hchar_1bpp(cpumem);
                }
                else if self.mode_4bpp {
                    self.draw_gfx_mode_4bpp_mchar(cpumem);
                }
                else {
                    self.draw_gfx_mode_2bpp_mchar(cpumem);
                }
            }
            else if self.in_crtc_hblank {
                // Draw hblank in debug color
                if self.debug_draw && self.mode_4bpp {
                    self.draw_solid_4bpp_mchar(CGA_HBLANK_DEBUG_COLOR);
                }
                else if self.debug_draw {
                    self.draw_solid_mchar(CGA_HBLANK_DEBUG_COLOR);
                }
            }
            else if self.in_crtc_vblank {
                // Draw vblank in debug color
                if self.debug_draw && self.mode_4bpp {
                    self.draw_solid_4bpp_mchar(CGA_HBLANK_DEBUG_COLOR);
                }
                else if self.debug_draw {
                    self.draw_solid_mchar(CGA_VBLANK_DEBUG_COLOR);
                }
            }
            else if self.vborder | self.hborder {
                // Draw overscan
                if self.mode_4bpp {
                    self.draw_solid_4bpp_mchar(self.cc_overscan_color);
                }
                else {
                    self.draw_solid_mchar(self.cc_overscan_color);
                }
            }
            else {
                //log::warn!("invalid display state...");
            }
        }

        // Update position to next pixel and character column.
        if self.mode_4bpp {
            self.beam_x += 4 * self.clock_divisor as u32;
            self.rba += 4 * self.clock_divisor as usize;
        }
        else {
            self.beam_x += 8 * self.clock_divisor as u32;
            self.rba += 8 * self.clock_divisor as usize;
        }

        // If we have reached the right edge of the 'monitor', return the raster position
        // to the left side of the screen.
        if self.beam_x >= CGA_XRES_MAX {
            self.beam_x = 0;
            self.beam_y += 1;
            self.in_monitor_hsync = false;
            self.rba = (CGA_XRES_MAX * self.beam_y) as usize;
        }

        if self.cycles & self.char_clock_mask != 0 {
            log::error!(
                "tick_mchar(): calling tick_crtc_char but out of phase with cclock: cycles: {} mask: {}",
                self.cycles,
                self.char_clock_mask
            );
        }

        if self.update_char_tick(cpumem) && self.intr_enabled() {
            self.intr = true;
        }
        else if !self.in_crtc_vblank {
            self.intr = false;
        }
        self.char_col = 0;
        self.update_clock();
    }

    /// Execute one low-resolution (3.5Mhz) character clock. Only 4bpp graphics mode is supported.
    pub fn tick_lchar(&mut self, cpumem: &[u8]) {
        // Cycles must be a factor of 16 and char_clock == 16
        assert_eq!(self.cycles & 0x0F, 0);
        assert_eq!(self.char_clock, 32);

        // sink_cycles must be a factor of 8
        //assert!((self.sink_cycles & 0x07) == 0);

        /*
        if self.sink_cycles & 0x0F != 0 {
            log::error!("sink_cycles: {} not divisible by 16", self.sink_cycles);
        }
        */

        if self.sink_cycles > 0 {
            self.sink_cycles = self.sink_cycles.saturating_sub(16);
            return;
        }

        self.cycles += 32;
        self.cur_screen_cycles += 32;

        // Only draw if marty_render buffer address is in bounds.
        if self.rba < (CGA_MAX_CLOCK - 16) {
            if self.in_display_area {
                self.draw_gfx_mode_4bpp_lchar(cpumem);
            }
            else if self.in_crtc_hblank {
                // Draw hblank in debug color
                if self.debug_draw {
                    self.draw_solid_lchar(CGA_HBLANK_DEBUG_COLOR);
                }
            }
            else if self.in_crtc_vblank {
                // Draw vblank in debug color
                if self.debug_draw {
                    self.draw_solid_lchar(CGA_VBLANK_DEBUG_COLOR);
                }
            }
            else if self.vborder | self.hborder {
                // Draw overscan
                self.draw_solid_lchar(self.cc_overscan_color);
            }
            else {
                //log::warn!("invalid display state...");
            }
        }

        // Update position to next pixel and character column.
        self.beam_x += 4 * self.clock_divisor as u32;
        self.rba += 4 * self.clock_divisor as usize;

        // If we have reached the right edge of the 'monitor', return the raster position
        // to the left side of the screen.
        if self.beam_x >= CGA_XRES_MAX {
            self.beam_x = 0;
            self.beam_y += 1;
            self.in_monitor_hsync = false;
            self.rba = (CGA_XRES_MAX * self.beam_y) as usize;
        }

        if self.cycles & self.char_clock_mask != 0 {
            log::error!(
                "tick_lchar(): calling tick_crtc_char but out of phase with cclock: cycles: {} mask: {}",
                self.cycles,
                self.char_clock_mask
            );
        }

        if self.update_char_tick(cpumem) && self.intr_enabled() {
            self.intr = true;
        }
        else if !self.in_crtc_vblank {
            self.intr = false;
        }
        self.char_col = 0;
        self.update_clock();
    }

    pub fn update_char_tick(&mut self, cpumem: &[u8]) -> bool {
        let mut did_vsync = false;
        self.tick_crtc_char();
        if self.do_vsync {
            self.do_vsync = false;
            did_vsync = true;
        }

        self.set_char_addr(cpumem);
        did_vsync
    }

    pub fn intr_enabled(&self) -> bool {
        self.intr_enabled
    }

    /*    pub fn debug_tick2(&mut self) {
            if self.sink_cycles > 0 {
                self.sink_cycles = self.sink_cycles.saturating_sub(1);
                return;
            }
            self.cycles += 1;
            self.cur_screen_cycles += 1;

            // Don't execute even cycles if we are in half-clock mode
            if self.clock_divisor == 2 && (self.cycles & 0x01 == 0) {
                return;
            }

            let saved_rba = self.rba;

            if self.rba < (CGA_MAX_CLOCK - self.clock_divisor as usize) {
                self.draw_pixel(CGA_DEBUG_COLOR);
            }

            // Update position to next pixel and character column.
            self.beam_x += self.clock_divisor as u32;
            self.rba += self.clock_divisor as usize;
            self.char_col += 1;

            if self.beam_x == CGA_XRES_MAX {
                self.beam_x = 0;
                self.beam_y += 1;
                self.in_monitor_hsync = false;
                self.rba = (CGA_XRES_MAX * self.beam_y) as usize;
            }

            if self.rba != saved_rba + self.clock_divisor as usize {
                log::warn!("bad rba increment");
            }

            // Done with the current character
            if self.char_col == CGA_HCHAR_CLOCK {
                self.tick_crtc_char();
                self.set_char_addr(cpumem);
                self.char_col = 0;
            }
        }
    */

    /*    /// Execute one CGA clock cycle.
    pub fn tick(&mut self) {
        if self.sink_cycles > 0 {
            self.sink_cycles = self.sink_cycles.saturating_sub(1);
            return;
        }
        self.cycles += 1;
        self.cur_screen_cycles += 1;

        // Don't execute odd cycles if we are in half-clock mode
        if self.clock_divisor == 2 && (self.cycles & 0x01 == 1) {
            return;
        }

        let saved_rba = self.rba;

        if self.rba < (CGA_MAX_CLOCK - self.clock_divisor as usize) {
            if self.in_display_area {
                // Draw current pixel
                if !self.mode_graphics {
                    self.draw_text_mode_pixel();
                }
                else if self.mode_hires_gfx {
                    self.draw_hires_gfx_mode_pixel();
                }
                else {
                    self.draw_lowres_gfx_mode_pixel();
                }
            }
            else if self.in_crtc_hblank {
                // Draw hblank in debug color
                if self.debug_draw {
                    self.buf[self.back_buf][self.rba] = CGA_HBLANK_DEBUG_COLOR;
                }
            }
            else if self.in_crtc_vblank {
                // Draw vblank in debug color
                if self.debug_draw {
                    self.buf[self.back_buf][self.rba] = CGA_VBLANK_DEBUG_COLOR;
                }
            }
            else if self.vborder | self.hborder {
                // Draw overscan
                if self.debug_draw {
                    self.draw_overscan_pixel();
                    //self.draw_pixel(CGA_OVERSCAN_DEBUG_COLOR);
                }
                else {
                    self.draw_overscan_pixel();
                }
            }
            else {
                //log::warn!("tick(): invalid display state...");
                self.draw_pixel(CGA_DEBUG2_COLOR);
            }
        }

        // Update position to next pixel and character column.
        self.beam_x += self.clock_divisor as u32;
        self.rba += self.clock_divisor as usize;
        self.char_col += 1;

        // Monitor has a fixed hsync position if no hsync received from card.
        /*
        if self.beam_x == CGA_HSYNC_POS {
            // Enter monitor hsync and init hsync counter
            self.in_monitor_hsync = true;
            self.monitor_hsc = 0;
        }
        else if self.in_monitor_hsync {
            // In hsync, but not on the first cycle. Increment hsync counter.
            self.monitor_hsc += self.clock_divisor as u32;

            if self.monitor_hsc == CGA_HSYNC_WIDTH {
                // We reached the end of the hsync period. Reset the beam to the
                // left, one scanline lower.
                self.beam_x = 0;
                self.beam_y += 1;
                self.in_monitor_hsync = false;
                self.rba = (CGA_XRES_MAX * self.beam_y) as usize;

                // draw diagnostic pixel
                if self.rba < CGA_MAX_CLOCK {
                    self.buf[self.back_buf][self.rba] = 14;
                    self.rba += self.clock_divisor as usize;
                    //self.beam_x += self.clock_divisor as u32;
                }

                self.missed_hsyncs = self.missed_hsyncs.wrapping_add(1);
            }
        }
        */

        if self.beam_x == CGA_XRES_MAX {
            self.beam_x = 0;
            self.beam_y += 1;
            self.in_monitor_hsync = false;
            self.rba = (CGA_XRES_MAX * self.beam_y) as usize;
        }

        if self.rba != saved_rba + self.clock_divisor as usize {
            log::warn!("bad rba increment");
        }

        // Done with the current character
        if self.char_col == CGA_HCHAR_CLOCK {
            if self.cycles & self.char_clock_mask != 0 {
                log::error!(
                    "tick(): calling tick_crtc_char but out of phase with cclock: cycles: {} mask: {}",
                    self.cycles,
                    self.char_clock_mask
                );
            }
            self.tick_crtc_char();
            self.set_char_addr();
            self.char_col = 0;
            self.update_clock();
        }
    }*/

    /// Update the CRTC logic for next character.
    pub fn tick_crtc_char(&mut self) {
        if self.hcc_c0 == 0 {
            self.hborder = false;
            if self.vcc_c4 == 0 {
                // We are at the first character of a CRTC frame. Update start address.
                self.vma = self.crtc_frame_address;
            }
        }

        if self.hcc_c0 == 0 {
            // When C0 < 2 evaluate last_line flag status.
            // LOGON SYSTEM v1.6 pg 73
            if self.vcc_c4 == self.crtc_vertical_total {
                self.last_row = true;
                self.vtac_c5 = 0;
            }
            else {
                //self.last_row = false;
            }
        }

        // Update horizontal character counter
        self.hcc_c0 = self.hcc_c0.wrapping_add(1);

        // Advance video memory address offset
        self.vma += 1;

        // Process horizontal blanking period
        if self.in_crtc_hblank {
            // Increment horizontal sync counter (wrapping)

            /*
            if ((self.hsc_c3l + 1) & 0x0F) != self.hsc_c3l.wrapping_add(1) {
                log::warn!("hsc0: {} hsc1: {}", ((self.hsc_c3l + 1) & 0x0F), self.hsc_c3l.wrapping_add(1));
            }
            */

            //self.hsc_c3l = (self.hsc_c3l + 1) & 0x0F;
            self.hsc_c3l = self.hsc_c3l.wrapping_add(1);

            // Implement a fixed hsync width from the monitor's perspective -
            // A wider programmed hsync width than these values shifts the displayed image to the right.
            // in 4bpp low res mode, clock divisor is 2 but the effective character width is halved.

            let hsync_target: u8 = match self.subtype {
                VideoCardSubType::IbmPCJr => {
                    // PCjr uses wider hsyncs than Tandy or CGA
                    if (self.clock_divisor == 1) || self.mode_4bpp {
                        std::cmp::min(12, self.crtc_sync_width)
                    }
                    else {
                        6
                    }
                }
                _ => {
                    if (self.clock_divisor == 1) || self.mode_4bpp {
                        std::cmp::min(10, self.crtc_sync_width)
                    }
                    else {
                        5
                    }
                }
            };

            // Do a horizontal sync
            if self.hsc_c3l == hsync_target {
                // Update the video mode, if an update is pending.
                // It is important not to change graphics mode while we are catching up during an IO instruction.
                if !self.catching_up && self.mode_pending {
                    self.update_mode();
                    self.mode_pending = false;
                }

                // END OF LOGICAL SCANLINE
                if self.in_crtc_vblank {
                    //if self.vsc_c3h == CRTC_VBLANK_HEIGHT || self.beam_y == CGA_MONITOR_VSYNC_POS {
                    if self.vsc_c3h == CRTC_VSYNC_HEIGHT {
                        // We are leaving vblank period. Generate a frame.

                        // Previously, we generated frames upon reaching vertical total. This was convenient as
                        // the display area would be at the top of the marty_render buffer and both overscan periods
                        // beneath it.
                        // However, CRTC tricks like 8088mph rewrite vertical total; this causes multiple
                        // 'screens' per frame in between vsyncs. To enable these tricks to work, we must marty_render
                        // like a monitor would.
                        self.in_last_vblank_line = true;
                        self.vsc_c3h = 0;
                        self.do_vsync();
                        self.do_vsync = true;
                    }
                }

                self.scanline += 1;

                // Reset beam to left of screen if we haven't already
                if self.beam_x > 0 {
                    self.beam_y += 1;
                }
                self.beam_x = 0;

                let new_rba = (CGA_XRES_MAX * self.beam_y) as usize;
                self.rba = new_rba;
            }

            // End horizontal blank when we reach R3
            if self.hsc_c3l == self.crtc_sync_width {
                self.in_crtc_hblank = false;
                self.hsc_c3l = 0;
            }
        }

        if self.hcc_c0 == self.crtc_horizontal_displayed {
            // C0 == R1. Entering right overscan.

            if self.vlc_c9 == self.crtc_maximum_scanline_address {
                // Save VMA in VMA'
                //log::debug!("Updating vma_t: {:04X}", self.vma_t);
                self.vma_t = self.vma;
            }

            // Save right overscan start position to calculate width of right overscan later
            self.overscan_right_start = self.beam_x;
            self.in_display_area = false;
            self.hborder = true;
        }

        if self.hcc_c0 == self.crtc_horizontal_sync_pos {
            // We entered horizontal blank
            self.in_crtc_hblank = true;
            self.hsc_c3l = 0;
        }

        if self.hcc_c0 == self.crtc_horizontal_total && self.in_last_vblank_line {
            // We are one char away from the beginning of the new frame.
            // Draw one char of border
            self.hborder = true;
        }

        if self.hcc_c0 == self.crtc_horizontal_total + 1 {
            // C0 == R0: Leaving left overscan, finished scanning row

            if self.in_crtc_vblank {
                // If we are in vblank, advance Vertical Sync Counter
                self.vsc_c3h += 1;
            }

            if self.in_last_vblank_line {
                self.in_last_vblank_line = false;
                self.in_crtc_vblank = false;
            }

            // Reset Horizontal Character Counter and increment character row counter
            self.hcc_c0 = 0;
            self.hborder = false;
            self.vlc_c9 += 1;
            // Return video memory address to starting position for next character row
            self.vma = self.vma_t;

            // Reset the current character glyph to start of row
            //self.set_char_addr();

            if !self.in_crtc_vblank && (self.vcc_c4 < self.crtc_vertical_displayed) {
                // Start the new row
                self.in_display_area = true;
            }

            if self.vlc_c9 > self.crtc_maximum_scanline_address {
                // C9 == R9 We finished drawing this row of characters

                self.vlc_c9 = 0;
                // Increment Vertical Character Counter for next row
                self.vcc_c4 = self.vcc_c4.wrapping_add(1);

                // Set vma to starting position for next character row
                //self.vma = (self.vcc_c4 as usize) * (self.crtc_horizontal_displayed as usize) + self.crtc_frame_address;
                self.vma = self.vma_t;

                if self.vcc_c4 == self.crtc_vertical_sync_pos {
                    // C4 == R7: We've reached vertical sync
                    trace_regs!(self);
                    trace!(self, "Entering vsync");
                    self.in_crtc_vblank = true;
                    self.in_display_area = false;
                }

                if self.last_row {
                    // C4 == R4 We are at vertical total, start incrementing vertical total adjust counter.
                    //log::debug!("setting vta at : {}", self.vcc_c4);
                    self.in_vta = true;
                    self.last_row = false;
                }
            }

            if self.vcc_c4 == self.crtc_vertical_displayed {
                // C4 == R6: Enter lower overscan area.
                self.in_display_area = false;
                self.vborder = true;
            }

            if self.vcc_c4 == self.crtc_vertical_total + 1 {
                // We are at vertical total, start incrementing vertical total adjust counter.
                //self.in_vta = true;
                if !self.in_vta {
                    log::debug!(
                        "in last row but no vta? vcc: {} vt: {}",
                        self.vcc_c4,
                        self.crtc_vertical_total
                    );
                }
            }

            if self.in_vta {
                // We are in vertical total adjust.
                if self.vtac_c5 == self.crtc_vertical_total_adjust {
                    // We have reached vertical total adjust. We are at the end of the top overscan.
                    self.in_vta = false;
                    self.vtac_c5 = 0;
                    self.hcc_c0 = 0;
                    self.vcc_c4 = 0;
                    self.vlc_c9 = 0;
                    self.crtc_frame_address = self.crtc_start_address;
                    self.vma = self.crtc_start_address;
                    self.vma_t = self.vma;
                    self.in_display_area = true;
                    self.vborder = false;
                    self.in_crtc_vblank = false;
                }
                else {
                    self.vtac_c5 += 1;
                }
            }
        }
    }

    pub fn do_vsync(&mut self) {
        self.in_crtc_vsync = false;

        self.cycles_per_vsync = self.cur_screen_cycles;
        self.cur_screen_cycles = 0;
        self.last_vsync_cycles = self.cycles;

        if self.cycles_per_vsync > 300000 {
            log::trace!(
                "do_vsync(): Excessively long frame. char_clock: {} cycles: {} beam_y: {}",
                self.char_clock,
                self.cycles_per_vsync,
                self.beam_y
            );
        }

        // Only do a vsync if we are past the minimum scanline #.
        // A monitor will refuse to vsync too quickly.
        if self.beam_y > CGA_MONITOR_VSYNC_MIN {
            // vblank remains set through the entire last line, including the right overscan of the new screen.
            // So we need to delay resetting vblank flag until then.
            //self.in_crtc_vblank = false;

            if self.beam_y > 258 && self.beam_y < 262 {
                // This is a "short" frame. Calculate delta.
                //let delta_y = 262 - self.beam_y;
                //self.sink_cycles = delta_y * 912;

                if self.cycles & self.char_clock_mask != 0 {
                    log::error!(
                        "vsync out of phase with cclock: cycles: {} mask: {}",
                        self.cycles,
                        self.char_clock_mask
                    );
                }
                //log::trace!("sink_cycles: {}", self.sink_cycles);
            }

            self.beam_x = 0;
            self.beam_y = 0;
            self.rba = 0;
            // Write out preliminary DisplayExtents data for new front buffer based on current crtc values.

            trace_regs!(self);
            trace!(self, "Leaving vsync and flipping buffers");

            self.scanline = 0;
            self.frame_count += 1;

            // Save the current mode byte, used for composite rendering.
            // The mode could have changed several times per frame, but I am not sure how the composite rendering should
            // really handle that...
            self.extents.mode_byte = self.mode_byte;

            // Swap the display buffers
            self.swap();
        }
        else {
            // Don't do vsync but reset scanline # so we can keep track in Area5150
            self.scanline = 0;
            self.frame_count += 1;
        }
    }

    pub fn video_array_select(&mut self, data: u8) {
        self.video_array_address = (data & 0x1F) as usize;
        log::debug!("TGA Video Array Select: {:02X}", self.video_array_address);
    }

    pub fn video_array_write(&mut self, data: u8) {
        match (self.video_array_address, self.subtype) {
            (0x00, VideoCardSubType::IbmPCJr) => {
                self.jr_mode_control = JrModeControlRegister::from_bytes([data]);
                log::warn!(
                    "Write to TGA(PCJr) Mode Control: {:02X} {:?}",
                    data,
                    self.jr_mode_control
                );
                self.mode_4bpp = self.jr_mode_control.fourbpp_mode();
                self.mode_graphics = self.jr_mode_control.graphics();
                self.mode_hires_txt = self.jr_mode_control.bandwidth();

                self.mode_pending = true;
                self.clock_pending = true;
            }
            (0x01, _) => {
                // Palette Mask register
                self.palette_mask = data & 0x0F;
            }
            (0x02, _) => {
                // Border color register
                self.border_color = data & 0x0F;
            }

            (0x03, VideoCardSubType::Tandy1000) => {
                // Tandy1000 Mode control register
                self.t_mode_control = TModeControlRegister::from_bytes([data]);
                log::warn!("Write to TGA Mode Control: {:02X} {:?}", data, self.t_mode_control);
                self.mode_4bpp = self.t_mode_control.fourbpp_mode();
                self.mode_pending = true;
                self.clock_pending = true;
            }
            (0x03, VideoCardSubType::IbmPCJr) => {
                // Tandy1000 Mode control register
                self.jr_mode_control2 = JrModeControlRegister2::from_bytes([data]);
                log::warn!("Write to TGA Mode Control: {:02X} {:?}", data, self.jr_mode_control2);
                self.mode_blinking = self.jr_mode_control2.blink();

                self.mode_pending = true;
                self.clock_pending = true;
            }

            (0x10..=0x1F, _) => {
                log::debug!("Write to TGA palette register: {:02X}", data);
                let pal_idx = self.video_array_address - 0x10;
                self.palette_registers[pal_idx] = data & 0x0F;
            }
            _ => {}
        }
    }

    pub fn page_register_write(&mut self, data: u8) {
        self.page_register = TPageRegister::from_bytes([data]);
        log::debug!("TGA Page Register: {:?}", self.page_register);
        match self.mode_size {
            VideoModeSize::Mode16k => {
                // Select 16K page for CPU
                self.cpu_page_offset = self.page_register.cpu_page() as usize * 0x4000;
            }
            VideoModeSize::Mode32k => {
                // Select 32K page for CPU} // Select 32K page for CPU
                // 32K page is chosen by ignoring bit 0 of the CPU page register
                self.cpu_page_offset = (self.page_register.cpu_page() & 0x0E) as usize * 0x4000;
            }
        }

        self.crt_page_offset = self.page_register.crt_page() as usize * 0x4000; // Select 16K page for TGA
        log::debug!(
            "New page offsets: CPU {:05X} TGA {:05X}",
            self.cpu_page_offset,
            self.crt_page_offset
        );
    }

    // Recalculate extents based on current CRTC values. This should be called after the maximum scanline address
    // is changed, so that the screen can be enlarged for the Tandy's 8x9 character cell mode.
    pub fn recalc_extents(&mut self) {
        // TGA: Modify apertures for text mode if we are using 9 pixel height glyphs
        if self.crtc_maximum_scanline_address == 8 {
            self.extents.apertures = TGA_APERTURES[1].to_vec();
        }
        else {
            self.extents.apertures = TGA_APERTURES[0].to_vec();
        }
    }

    #[inline]
    pub fn crt_mem<'a>(&'a self, cpumem: &'a [u8]) -> &'a [u8] {
        let start = self.aperture_base + self.crt_page_offset;
        cpumem[start..start + self.page_size].as_ref()
    }

    #[inline]
    pub fn crt_memmut<'a>(&'a self, cpumem: &'a mut [u8]) -> &'a mut [u8] {
        let start = self.aperture_base + self.crt_page_offset;
        cpumem[start..start + self.page_size].as_mut()
    }

    #[inline]
    pub fn cpu_mem<'a>(&'a self, cpumem: &'a [u8]) -> &'a [u8] {
        let start = self.aperture_base + self.cpu_page_offset;
        cpumem[start..start + self.page_size].as_ref()
    }

    #[inline]
    pub fn cpu_memmut<'a>(&'a self, cpumem: &'a mut [u8]) -> &'a mut [u8] {
        let start = self.aperture_base + self.cpu_page_offset;
        cpumem[start..start + self.page_size].as_mut()
    }
}

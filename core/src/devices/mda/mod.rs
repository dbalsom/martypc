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

    --------------------------------------------------------------------------

    devices::mda::mod.rs

    Implementation of the IBM MDA card, built around the Motorola MC6845
    display controller.

*/

#![allow(dead_code)]
use bytemuck;
use const_format::formatcp;
use modular_bitfield::{bitfield, prelude::*};
use std::{collections::HashMap, convert::TryInto, path::Path};

#[macro_use]
mod io;
mod attr;
mod draw;
mod mmio;
mod tablegen;
mod videocard;

use crate::{
    bus::{BusInterface, DeviceRunTimeUnit},
    tracelogger::TraceLogger,
    videocard::*,
};

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

static DUMMY_PLANE: [u8; 1] = [0];
static DUMMY_PIXEL: [u8; 4] = [0, 0, 0, 0];

// Precalculated waits in system ticks for each of the possible 16 phases of the
// CGA clock could issue a memory request on.
static WAIT_TABLE: [u32; 16] = [14, 13, 12, 11, 10, 9, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15];
// in cpu cycles: 5,5,4,4,4,3,8,8,8,7,7,7,6,6,6,5

pub const MDA_MEM_ADDRESS: usize = 0xB0000;
// MDA memory is repeated from B0000-B7FFFF due to incomplete address decoding.
pub const MDA_MEM_APERTURE: usize = 0x8000;
pub const MDA_MEM_SIZE: usize = 0x1000; // 4096 bytes
pub const MDA_MEM_MASK: usize = 0x0FFF; // Applying this mask will implement memory mirror.

// Sensible defaults for MDA CRTC registers. A real CRTC is probably uninitialized.
const DEFAULT_HORIZONTAL_TOTAL: u8 = 97;
const DEFAULT_HORIZONTAL_DISPLAYED: u8 = 80;
const DEFAULT_HORIZONTAL_SYNC_POS: u8 = 82;
const DEFAULT_HORIZONTAL_SYNC_WIDTH: u8 = 15;
const DEFAULT_VERTICAL_TOTAL: u8 = 25;
const DEFAULT_VERTICAL_TOTAL_ADJUST: u8 = 6;
const DEFAULT_VERTICAL_DISPLAYED: u8 = 25;
const DEFAULT_VERTICAL_SYNC_POS: u8 = 25;
const DEFAULT_MAXIMUM_SCANLINE: u8 = 13;
const DEFAULT_CURSOR_START_LINE: u8 = 11;
const DEFAULT_CURSOR_END_LINE: u8 = 12;
const DEFAULT_CLOCK_DIVISOR: u8 = 1; // On the MDA these are fixed and do not change.
const DEFAULT_CHAR_CLOCK: u32 = 9; // On the MDA these are fixed and do not change.

//const DEFAULT_CHAR_CLOCK_MASK: u64 = 0x0F;      // MDA's 9-dot character clock is not easily represented in binary
//const DEFAULT_CHAR_CLOCK_ODD_MASK: u64 = 0x1F;

// Unlike the CGA, the MDA has its own on-board crystal and does not run at the system bus clock.
// MDA is clocked at 16.257Mhz and runs at 50Hz refresh rate and 18.432kHz horizontal scan rate.
//  16,257,000 / 50 = 325,140 dots per frame
//  325,140 / 18.432kHz = 882 dots per scanline
//  882 / 9 = 98 maximum horizontal total characters
//  325,140 / 882 = ~368.639 scanlines per frame (??)
//const CDA_CLOCK: f64 = 14.318180;
const MDA_CLOCK: f64 = 16.257;
const US_PER_CLOCK: f64 = 1.0 / MDA_CLOCK;
const US_PER_FRAME: f64 = 1.0 / 50.0;

pub const MDA_MAX_CLOCK: usize = (MDA_XRES_MAX * MDA_YRES_MAX) as usize;
//pub const MDA_MAX_CLOCK: usize = 325140; // 16,257,000 / 50

// Calculate the maximum possible area of display field (including refresh period)
const MDA_XRES_MAX: u32 = (CRTC_R0_HORIZONTAL_MAX + 1) * MDA_CHAR_CLOCK as u32; // 882
const MDA_YRES_MAX: u32 = 369; // Actual value works out to 325,140 / 882 or 368.639

// Monitor sync position. The monitor will eventually perform an hsync at a fixed position
// if hsync signal is late from the CGA card.
const MDA_MONITOR_HSYNC_POS: u32 = 832;
const MDA_MONITOR_HSYNC_WIDTH: u32 = 80;
//const MDA_MONITOR_VSYNC_POS: u32 = 246;
// Minimum scanline value after which we can perform a vsync. A vsync before this scanline will be ignored.
const MDA_MONITOR_VSYNC_MIN: u32 = 0;

const MDA_DEFAULT_CURSOR_BLINK_RATE: f64 = 0.0625;
const MDA_DEFAULT_CURSOR_FRAME_CYCLE: u64 = 8;

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

// Include the standard 8x8 CGA font.
// TODO: Support alternate font with thinner glyphs? It was normally not accessable except
// by soldering a jumper
const MDA_FONT: &'static [u8] = include_bytes!("../../../../assets/mda_8by14.bin");
const MDA_FONT_SPAN: usize = 256; // Font bitmap is 2048 bits wide (256 * 8 characters)

const MDA_CHAR_CLOCK: u8 = 9;
const CRTC_FONT_HEIGHT: u8 = 14;
const CRTC_VBLANK_HEIGHT: u8 = 16;

const CRTC_R0_HORIZONTAL_MAX: u32 = 97;

// The MDA card decodes 11 address lines off the CRTC chip. This produces 2048 word addresses (4096 bytes)
const MDA_TEXT_MODE_WRAP: usize = 0x07FF;

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
const CGA_HBLANK_DEBUG_COLOR: u8 = CgaColor::BlueBright as u8;
const CGA_VBLANK_DEBUG_COLOR: u8 = CgaColor::Yellow as u8;
const CGA_DISABLE_DEBUG_COLOR: u8 = CgaColor::Green as u8;
const CGA_OVERSCAN_DEBUG_COLOR: u8 = CgaColor::Green as u8;

/*
const CGA_FILL_COLOR: u8 = 4;
const CGA_SCANLINE_COLOR: u8 = 13;
*/

const MDA_CURSOR_MAX: usize = 32;

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

// Display aperatures.
// CROPPED will show the display area only - no overscan will be visible.
// NORMAL is an attempt to represent the maximum visible display extents, including overscan.
// Anything more is likely to be hidden by the monitor bezel or not shown for some other reason.
// FULL will show the entire overscan area - this is nice for Area 5150 to see the entire extent
// of each effect, although it will display more than a monitor would.
// DEBUG will show the entire display field and will enable coloring of hblank and vblank
// periods.
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

const MDA_APERTURE_DEBUG_W: u32 = MDA_XRES_MAX;
const MDA_APERTURE_DEBUG_H: u32 = MDA_YRES_MAX;
const MDA_APERTURE_DEBUG_X: u32 = 0;
const MDA_APERTURE_DEBUG_Y: u32 = 0;

const MDA_APERTURES: [DisplayAperture; 4] = [
    // 14Mhz CROPPED aperture
    DisplayAperture {
        w: MDA_APERTURE_CROPPED_W,
        h: MDA_APERTURE_CROPPED_H,
        x: MDA_APERTURE_CROPPED_X,
        y: MDA_APERTURE_CROPPED_Y,
        debug: false,
    },
    // 14Mhz ACCURATE aperture
    DisplayAperture {
        w: MDA_APERTURE_NORMAL_W,
        h: MDA_APERTURE_NORMAL_H,
        x: MDA_APERTURE_NORMAL_X,
        y: MDA_APERTURE_NORMAL_Y,
        debug: false,
    },
    // 14Mhz FULL aperture
    DisplayAperture {
        w: MDA_APERTURE_FULL_W,
        h: MDA_APERTURE_FULL_H,
        x: MDA_APERTURE_FULL_X,
        y: MDA_APERTURE_FULL_Y,
        debug: false,
    },
    // 14Mhz DEBUG aperture
    DisplayAperture {
        w: MDA_APERTURE_DEBUG_W,
        h: MDA_APERTURE_DEBUG_H,
        x: 0,
        y: 0,
        debug: true,
    },
];

const CROPPED_STRING: &str = &formatcp!("Cropped: {}x{}", MDA_APERTURE_CROPPED_W, MDA_APERTURE_CROPPED_H);
const ACCURATE_STRING: &str = &formatcp!("Accurate: {}x{}", MDA_APERTURE_NORMAL_W, MDA_APERTURE_NORMAL_H);
const FULL_STRING: &str = &formatcp!("Full: {}x{}", MDA_APERTURE_FULL_W, MDA_APERTURE_FULL_H);
const DEBUG_STRING: &str = &formatcp!("Debug: {}x{}", MDA_APERTURE_DEBUG_W, MDA_APERTURE_DEBUG_H);

const MDA_APERTURE_DESCS: [DisplayApertureDesc; 4] = [
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

const MDA_DEFAULT_APERTURE: usize = 0;

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

use crate::devices::mda::attr::MDA_ATTR_TABLE;
pub(crate) use trace_regs;

#[bitfield]
#[derive(Copy, Clone)]
pub struct MdaModeRegister {
    pub high_res: bool,
    pub bw: bool,
    #[skip]
    pub bit2: bool,
    pub display_enable: bool,
    pub blinking: bool,
    #[skip]
    pub unused: B3,
}

pub struct MDACard {
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

    mode_pending:  bool,
    clock_pending: bool,

    mode_byte: u8,
    mode: MdaModeRegister,
    display_mode: DisplayMode,
    mode_enable: bool,
    mode_graphics: bool,
    mode_bw: bool,
    mode_hires_gfx: bool,
    mode_hires_txt: bool,
    mode_blinking: bool,
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
    cursor_data: [bool; MDA_CURSOR_MAX],
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
    in_last_vblank_line: bool,
    hborder: bool,
    vborder: bool,

    cc_register:   u8,
    clock_divisor: u8, // Clock divisor is 1 in high resolution text mode, 2 in all other modes
    clock_mode:    ClockingMode,
    char_clock:    u32,

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

    mem: Box<[u8; MDA_MEM_SIZE]>,

    back_buf: usize,
    front_buf: usize,
    extents: DisplayExtents,
    aperture: usize,
    //buf: Vec<Vec<u8>>,
    buf: [Box<[u8; MDA_MAX_CLOCK]>; 2],

    debug_color: u8,

    trace_logger:  TraceLogger,
    debug_counter: u64,

    lightpen_latch: bool,
    lightpen_addr:  usize,
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

// CGA implementation of Default for DisplayExtents.
// Each videocard implementation should implement sensible defaults.
// In CGA's case we know the maximum field size and thus row_stride.
trait MdaDefault {
    fn default() -> Self;
}
impl MdaDefault for DisplayExtents {
    fn default() -> Self {
        Self {
            apertures: MDA_APERTURES.to_vec(),
            field_w: MDA_XRES_MAX,
            field_h: MDA_YRES_MAX,
            row_stride: MDA_XRES_MAX as usize,
            double_scan: false,
            mode_byte: 0,
        }
    }
}

impl Default for MDACard {
    fn default() -> Self {
        Self {
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
            mode: MdaModeRegister::new(),
            mode_pending: false,
            clock_pending: false,
            display_mode: DisplayMode::Mode0TextBw40,
            mode_enable: true,
            mode_graphics: false,
            mode_bw: false,
            mode_hires_gfx: false,
            mode_hires_txt: true,
            mode_blinking: true,
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
            cursor_blink_rate: MDA_DEFAULT_CURSOR_BLINK_RATE,
            cursor_data: [false; MDA_CURSOR_MAX],
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
            in_last_vblank_line: false,
            hborder: true,
            vborder: true,

            cc_register: CC_PALETTE_BIT | CC_BRIGHT_BIT,

            clock_divisor: DEFAULT_CLOCK_DIVISOR,
            clock_mode: ClockingMode::Character,
            char_clock: DEFAULT_CHAR_CLOCK,
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

            mem: vec![0; MDA_MEM_SIZE].into_boxed_slice().try_into().unwrap(),

            back_buf:  1,
            front_buf: 0,
            extents:   MdaDefault::default(),
            aperture:  MDA_DEFAULT_APERTURE,

            //buf: vec![vec![0; (CGA_XRES_MAX * CGA_YRES_MAX) as usize]; 2],

            // Theoretically, boxed arrays may have some performance advantages over
            // vectors due to having a fixed size known by the compiler.  However they
            // are a pain to initialize without overflowing the stack.
            buf: [
                vec![0; MDA_MAX_CLOCK].into_boxed_slice().try_into().unwrap(),
                vec![0; MDA_MAX_CLOCK].into_boxed_slice().try_into().unwrap(),
            ],

            debug_color: 0,

            trace_logger:  TraceLogger::None,
            debug_counter: 0,

            lightpen_latch: false,
            lightpen_addr:  0,
        }
    }
}

impl MDACard {
    pub fn new(trace_logger: TraceLogger, clock_mode: ClockingMode, video_frame_debug: bool) -> Self {
        let mut cga = Self::default();

        cga.trace_logger = trace_logger;
        cga.debug = video_frame_debug;
        cga.clock_mode = clock_mode;

        cga
    }

    /// Reset CGA state (on reboot, for example)
    fn reset_private(&mut self) {
        let trace_logger = std::mem::replace(&mut self.trace_logger, TraceLogger::None);

        // Save non-default values
        *self = Self {
            debug: self.debug,
            clock_mode: self.clock_mode,
            enable_snow: self.enable_snow,
            frame_count: self.frame_count, // Keep frame count as to not confuse frontend
            trace_logger,
            extents: self.extents.clone(),

            ..Self::default()
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

    fn catch_up(&mut self, delta: DeviceRunTimeUnit, debug: bool) -> u32 {
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
                self.tick_hchar();

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

            if debug && self.rba < (MDA_MAX_CLOCK - 8) {
                //log::debug!("crtc write!");
                self.draw_solid_hchar(13);
            }
            return ticks;
        }
        0
    }

    /// Update the number of pixel clocks we must execute before we can return to clocking the
    /// CGA card by character clock.  When an IO read/write occurs, the CGA card is updated to
    /// the current clock cycle by ticking pixels. During run() we then have to tick by pixels
    /// until we are back in phase with the character clock.
    #[inline]
    fn calc_cycles_owed(&mut self) -> u32 {
        if self.ticks_advanced % MDA_CHAR_CLOCK as u32 > 0 {
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
            // Low to high transaition of light pen latch, set latch addr.
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

    /// Update the cursor data array based on the values of cursor_start_line and cursor_end_line.
    fn update_cursor_data(&mut self) {
        // Reset cursor data to 0.
        self.cursor_data.fill(false);

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

            for i in (self.crtc_cursor_start_line as usize)..MDA_CURSOR_MAX {
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
            CRTCRegister::MaximumScanLineAddress => self.crtc_maximum_scanline_address = byte,
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
        // HIREST_TEXT bit in an undocumented combination that remains in text mode but allows
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

    /*
        /// Update the CGA graphics mode. This function may be called some time after the mode
        /// register is actually written to, depending on if we are changing from text to graphics mode
        /// or vice versa.
        fn update_mode(&mut self) {
            // Will this mode change change the character clock?
            let clock_changed = self.mode_hires_txt != (self.mode_byte & MODE_HIRES_TEXT != 0);

            if clock_changed {
                // Flag the clock for pending change.  The clock can only be changed in phase with
                // LCHAR due to our dynamic clocking logic.
                self.clock_pending = true;
            }

            self.mode_hires_txt = self.mode_byte & MODE_HIRES_TEXT != 0;
            self.mode_graphics = self.mode_byte & MODE_GRAPHICS != 0;
            self.mode_bw = self.mode_byte & MODE_BW != 0;
            self.mode_enable = self.mode_byte & MODE_ENABLE != 0;
            self.mode_hires_gfx = self.mode_byte & MODE_HIRES_GRAPHICS != 0;
            self.mode_blinking = self.mode_byte & MODE_BLINKING != 0;

            self.vmws = 2;

            // Use color control register value for overscan unless high res graphics mode,
            // in which case overscan must be black (0).
            self.cc_overscan_color = if self.mode_hires_gfx { 0 } else { self.cc_altcolor };

            // Reinterpret the CC register based on new mode.
            self.update_palette();

            // Attempt to update clock.
            self.update_clock();

            // Updated mask to exclude enable bit in mode calculation.
            // "Disabled" isn't really a video mode, it just controls whether
            // the CGA card outputs video at a given moment. This can be toggled on
            // and off during a single frame, such as done in VileR's fontcmp.com
            self.display_mode = match self.mode_byte & CGA_MODE_ENABLE_MASK {
                0b0_0100 => DisplayMode::Mode0TextBw40,
                0b0_0000 => DisplayMode::Mode1TextCo40,
                0b0_0101 => DisplayMode::Mode2TextBw80,
                0b0_0001 => DisplayMode::Mode3TextCo80,
                0b0_0011 => DisplayMode::ModeTextAndGraphicsHack,
                0b0_0010 => DisplayMode::Mode4LowResGraphics,
                0b0_0110 => DisplayMode::Mode5LowResAltPalette,
                0b1_0110 => DisplayMode::Mode6HiResGraphics,
                0b1_0010 => DisplayMode::Mode7LowResComposite,
                _ => {
                    trace!(self, "Invalid display mode selected: {:02X}", self.mode_byte & 0x1F);
                    log::warn!("CGA: Invalid display mode selected: {:02X}", self.mode_byte & 0x1F);
                    DisplayMode::Mode3TextCo80
                }
            };

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
    */

    /// Handle a write to the MDA mode register. Two of the bits are basically useless (0 & 1)
    /// leaving bit 3, which enables or disables video, and Bit 5, which controls blinking.
    fn handle_mode_register(&mut self, mode_byte: u8) {
        self.mode = MdaModeRegister::from_bytes([mode_byte]);
    }

    /// Handle a read from the MDA status register. This register has bits to indicate whether
    /// we are in vblank or if the display is in the active display area (enabled)
    fn handle_status_register_read(&mut self) -> u8 {
        // Bit 1 of the status register is set when the CGA can be safely written to without snow.
        // It is tied to the 'Display Enable' line from the CGA card, inverted.
        // Thus it will be 1 when the CGA card is not currently scanning, IE during both horizontal
        // and vertical refresh.

        // https://www.vogons.org/viewtopic.php?t=47052

        // Addendum: The DE line is from the MC6845, and actually includes anything outside of the
        // active display area. This gives a much wider window to hit for scanline wait loops.

        let mut byte = if self.in_crtc_vblank {
            0xF0 | STATUS_VERTICAL_RETRACE | STATUS_DISPLAY_ENABLE
        }
        else if !self.in_display_area {
            0xF0 | STATUS_DISPLAY_ENABLE
        }
        else {
            if self.vborder || self.hborder {
                log::warn!("in border but returning 0");
            }
            0xF0
        };

        if self.in_crtc_vblank {
            trace!(self, "in vblank: vsc: {:03}", self.vsc_c3h);
        }

        self.status_reads += 1;

        if self.lightpen_latch {
            //log::debug!("returning status read with trigger set");
            byte |= STATUS_LIGHTPEN_TRIGGER_SET;
        }

        // This bit is logically reversed, i.e., 0 is switch on
        //byte |= STATUS_LIGHTPEN_SWITCH_STATUS;

        trace_regs!(self);
        trace!(
            self,
            "Status register read: byte: {:02X} in_display_area: {} vblank: {} ",
            byte,
            self.in_display_area,
            self.in_crtc_vblank
        );

        byte
    }

    /*
    /// Handle write to the Color Control register. This register controls the palette selection
    /// and background/overscan color (foreground color in high res graphics mode)
    fn handle_cc_register_write(&mut self, data: u8) {
        self.cc_register = data;
        self.update_palette();

        log::trace!("Write to color control register: {:02X}", data);
    }

     */

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
    fn get_glyph_bit(glyph: u8, col: u8, row: u8) -> bool {
        let col = if col > 7 { 7 } else { col };
        //debug_assert!(row < CRTC_CHAR_CLOCK);
        let row_masked = row & 0xF; // Font was padded to 16 pixels high.

        // Calculate byte offset
        let glyph_offset: usize = (row_masked as usize * MDA_FONT_SPAN) + glyph as usize;
        let pixel = (MDA_FONT[glyph_offset] & (0x80 >> col)) != 0;
        pixel
    }

    /// Set the character attributes for the current character.
    fn set_char_addr(&mut self) {
        let addr = ((self.vma & MDA_TEXT_MODE_WRAP) << 1);
        self.cur_char = self.mem[addr];
        self.cur_attr = self.mem[addr + 1];

        if self.mode_blinking {
            self.cur_blink = self.cur_attr & 0x80 != 0;
        }
        else {
            self.cur_blink = false;
        }

        (self.cur_fg, self.cur_bg) = MDA_ATTR_TABLE[self.cur_attr as usize];
    }
    /*
       /// Get the 64-bit value representing the specified row of the specified character
       /// glyph in high-resolution text mode.
       #[inline]
       pub fn get_hchar_glyph_row(&self, glyph: usize, row: usize) -> u64 {
           if self.cur_blink && !self.blink_state {
               CGA_COLORS_U64[self.cur_bg as usize]
           }
           else {
               let glyph_row_base = CGA_HIRES_GLYPH_TABLE[glyph & 0xFF][row];

               // Combine glyph mask with foreground and background colors.
               glyph_row_base & CGA_COLORS_U64[self.cur_fg as usize]
                   | !glyph_row_base & CGA_COLORS_U64[self.cur_bg as usize]
           }
       }

    */

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

    /// Calculate the byte address given the current value of vma; given that the address
    /// programmed into the CRTC start register is interpreted by the CGA as a word address.
    /// In graphics mode, the row counter determines whether address line A12 from the
    /// CRTC is set. This effectively creates a 0x2000 byte offset for odd character rows.
    #[inline]
    pub fn get_gfx_addr(&self, row: u8) -> usize {
        let row_offset = (row as usize & 0x01) << 12;
        let addr = (self.vma & 0x0FFF | row_offset) << 1;
        addr
    }

    pub fn get_screen_ticks(&self) -> u64 {
        self.cur_screen_cycles
    }

    /// Execute one high resolution character clock.
    pub fn tick_hchar(&mut self) {
        self.cycles += MDA_CHAR_CLOCK as u64;
        self.cur_screen_cycles += MDA_CHAR_CLOCK as u64;

        // Only draw if marty_render buffer address is in bounds.
        if self.rba < (MDA_MAX_CLOCK - MDA_CHAR_CLOCK as usize) {
            if self.in_display_area {
                self.draw_text_mode_hchar_slow();
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
                self.draw_solid_hchar(self.cc_overscan_color);
            }
            else {
                self.draw_solid_hchar(CGA_DEBUG2_COLOR);
            }
        }

        // Update position to next pixel and character column.
        self.beam_x += MDA_CHAR_CLOCK as u32;
        self.rba += MDA_CHAR_CLOCK as usize;

        // If we have reached the right edge of the 'monitor', return the raster position
        // to the left side of the screen.
        if self.beam_x >= MDA_XRES_MAX {
            self.beam_x = 0;
            self.beam_y += 1;
            self.in_monitor_hsync = false;
            self.rba = (MDA_XRES_MAX * self.beam_y) as usize;
        }

        /*
        if self.cycles & self.char_clock_mask != 0 {
            log::error!(
                "tick_hchar(): calling tick_crtc_char but out of phase with cclock: cycles: {} mask: {}",
                self.cycles,
                self.char_clock_mask
            );
        }
         */

        self.tick_crtc_char();
        //self.update_clock();
    }

    pub fn debug_tick2(&mut self) {
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

        if self.rba < (MDA_MAX_CLOCK - self.clock_divisor as usize) {
            self.draw_pixel(CGA_DEBUG_COLOR);
        }

        // Update position to next pixel and character column.
        self.beam_x += self.clock_divisor as u32;
        self.rba += self.clock_divisor as usize;
        self.char_col += 1;

        if self.beam_x == MDA_XRES_MAX {
            self.beam_x = 0;
            self.beam_y += 1;
            self.in_monitor_hsync = false;
            self.rba = (MDA_XRES_MAX * self.beam_y) as usize;
        }

        if self.rba != saved_rba + self.clock_divisor as usize {
            log::warn!("bad rba increment");
        }

        // Done with the current character
        if self.char_col == MDA_CHAR_CLOCK {
            self.tick_crtc_char();
        }
    }

    /// Execute one CGA clock cycle.
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

        if self.rba < (MDA_MAX_CLOCK - self.clock_divisor as usize) {
            if self.in_display_area {
                // Draw current pixel
                self.draw_text_mode_pixel();
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
                    //self.draw_pixel(CGA_OVERSCAN_COLOR);
                    self.draw_pixel(CGA_OVERSCAN_DEBUG_COLOR);
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

        if self.beam_x == MDA_XRES_MAX {
            self.beam_x = 0;
            self.beam_y += 1;
            self.in_monitor_hsync = false;
            self.rba = (MDA_XRES_MAX * self.beam_y) as usize;
        }

        if self.rba != saved_rba + self.clock_divisor as usize {
            log::warn!("bad rba increment");
        }

        // Done with the current character
        if self.char_col == MDA_CHAR_CLOCK {
            if self.cycles % MDA_CHAR_CLOCK as u64 != 0 {
                log::error!(
                    "tick(): calling tick_crtc_char but out of phase with cclock: cycles: {}",
                    self.cycles,
                );
            }
            self.tick_crtc_char();
            //self.update_clock();
        }
    }

    /// Update the CRTC logic for next character.
    pub fn tick_crtc_char(&mut self) {
        // Update horizontal character counter
        self.hcc_c0 = self.hcc_c0.wrapping_add(1);
        if self.hcc_c0 == 0 {
            self.hborder = false;
            if self.vcc_c4 == 0 {
                // We are at the first character of a CRTC frame. Update start address.
                self.vma = self.crtc_frame_address;
            }
        }

        if self.hcc_c0 == 0 && self.vcc_c4 == 0 {
            // We are at the first character of a CRTC frame. Update start address.
            self.vma = self.crtc_frame_address;
        }

        // Advance video memory address offset and grab the next character + attr
        self.vma += 1;
        self.set_char_addr();

        // Glyph column reset to 0 for next char
        self.char_col = 0;

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
            let hsync_target = self.crtc_sync_width;

            // Do a horizontal sync
            if self.hsc_c3l == hsync_target {
                // Update the video mode, if an update is pending.
                // It is important not to change graphics mode while we are catching up during an IO instruction.
                if !self.catching_up && self.mode_pending {
                    //self.update_mode();
                    self.mode_pending = false;
                }

                // END OF LOGICAL SCANLINE
                if self.in_crtc_vblank {
                    //if self.vsc_c3h == CRTC_VBLANK_HEIGHT || self.beam_y == CGA_MONITOR_VSYNC_POS {
                    if self.vsc_c3h == CRTC_VBLANK_HEIGHT {
                        self.in_last_vblank_line = true;
                        // We are leaving vblank period. Generate a frame.

                        // Previously, we generated frames upon reaching vertical total. This was convenient as
                        // the display area would be at the top of the marty_render buffer and both overscan periods
                        // beneath it.
                        // However, CRTC tricks like 8088mph rewrite vertical total; this causes multiple
                        // 'screens' per frame in between vsyncs. To enable these tricks to work, we must marty_render
                        // like a monitor would.

                        self.vsc_c3h = 0;
                        self.do_vsync();
                        return;
                    }
                }

                self.scanline += 1;

                // Reset beam to left of screen if we haven't already
                if self.beam_x > 0 {
                    self.beam_y += 1;
                }
                self.beam_x = 0;
                self.char_col = 0;

                let new_rba = (MDA_XRES_MAX * self.beam_y) as usize;
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
            // Leaving left overscan, finished scanning row

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
            self.set_char_addr();

            if !self.in_crtc_vblank {
                // Start the new row
                if self.vcc_c4 < self.crtc_vertical_displayed {
                    self.in_display_area = true;
                }
            }

            if self.vlc_c9 > self.crtc_maximum_scanline_address {
                // C9 == R9 We finished drawing this row of characters

                self.vlc_c9 = 0;
                // Advance Vertical Character Counter
                self.vcc_c4 = self.vcc_c4.wrapping_add(1);

                // Set vma to starting position for next character row
                //self.vma = (self.vcc_c4 as usize) * (self.crtc_horizontal_displayed as usize) + self.crtc_frame_address;
                self.vma = self.vma_t;

                // Load next char + attr
                self.set_char_addr();

                if self.vcc_c4 == self.crtc_vertical_sync_pos {
                    // We've reached vertical sync
                    trace_regs!(self);
                    trace!(self, "Entering vsync");
                    self.in_crtc_vblank = true;
                    self.in_display_area = false;
                }
            }

            if self.vcc_c4 == self.crtc_vertical_displayed {
                // Enter lower overscan area.
                self.in_display_area = false;
                self.vborder = true;
            }

            /*
            if self.vcc_c4 >= (self.crtc_vertical_total + 1)  {

                // We are at vertical total, start incrementing vertical total adjust counter.
                self.vtac_c5 += 1;

                if self.vtac_c5 > self.crtc_vertical_total_adjust {
                    // We have reached vertical total adjust. We are at the end of the top overscan.
                    self.hcc_c0 = 0;
                    self.vcc_c4 = 0;
                    self.vtac_c5 = 0;
                    self.vlc_c9 = 0;
                    self.char_col = 0;
                    self.crtc_frame_address = self.crtc_start_address;
                    self.vma = self.crtc_start_address;
                    self.vma_t = self.vma;
                    self.in_display_area = true;
                    self.vborder = false;
                    self.in_crtc_vblank = false;

                    // Load first char + attr
                    self.set_char_addr();
                }
            }
            */

            if self.vcc_c4 == self.crtc_vertical_total + 1 {
                // We are at vertical total, start incrementing vertical total adjust counter.
                self.in_vta = true;
            }

            if self.in_vta {
                // We are in vertical total adjust.
                self.vtac_c5 += 1;

                if self.vtac_c5 > self.crtc_vertical_total_adjust {
                    // We have reached vertical total adjust. We are at the end of the top overscan.
                    self.in_vta = false;
                    self.vtac_c5 = 0;

                    self.hcc_c0 = 0;
                    self.vcc_c4 = 0;
                    self.vlc_c9 = 0;
                    self.char_col = 0;
                    self.crtc_frame_address = self.crtc_start_address;
                    self.vma = self.crtc_start_address;
                    self.vma_t = self.vma;
                    self.in_display_area = true;
                    self.vborder = false;
                    self.in_crtc_vblank = false;

                    // Load first char + attr
                    self.set_char_addr();
                }
            }
        }
    }

    pub fn do_vsync(&mut self) {
        self.cycles_per_vsync = self.cur_screen_cycles;
        self.cur_screen_cycles = 0;
        self.last_vsync_cycles = self.cycles;

        if self.cycles_per_vsync > 400000 {
            log::warn!(
                "do_vsync(): Excessively long frame. char_clock: {} cycles: {} beam_y: {}",
                self.char_clock,
                self.cycles_per_vsync,
                self.beam_y
            );
        }

        // Only do a vsync if we are past the minimum scanline #.
        // A monitor will refuse to vsync too quickly.
        if self.beam_y > MDA_MONITOR_VSYNC_MIN {
            // vblank remains set through the entire last line, including the right overscan of the new screen.
            // So we need to delay resetting vblank flag until then.
            //self.in_crtc_vblank = false;

            if self.beam_y > 258 && self.beam_y < 262 {
                // This is a "short" frame. Calculate delta.
                let _delta_y = 262 - self.beam_y;

                //self.sink_cycles = delta_y * 912;

                /*
                if self.cycles & self.char_clock_mask != 0 {
                    log::error!(
                        "vsync out of phase with cclock: cycles: {} mask: {}",
                        self.cycles,
                        self.char_clock_mask
                    );
                }

                 */
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

            // Toggle blink state. This is toggled every 8 frames by default.
            if (self.frame_count % MDA_DEFAULT_CURSOR_FRAME_CYCLE) == 0 {
                self.blink_state = !self.blink_state;
            }

            // Swap the display buffers
            self.swap();
        }
    }

    pub fn dump_status(&self) {
        println!("{}", self.hcc_c0);
        println!("{}", self.vlc_c9);
        println!("{}", self.vcc_c4);
        println!("{}", self.vsc_c3h);
        println!("{}", self.hsc_c3l);
        println!("{}", self.vtac_c5);
    }
}

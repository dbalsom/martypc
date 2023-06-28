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

    devices::cga::mod.rs

    Implementation of the IBM CGA card, built around the Motorola MC6845 
    display controller.

    This implementation is a bit complex due to being able to clock the CGA
    by a single tick/pixel or by character/8 pixels.

*/

#![allow(dead_code)]
use std::{
    collections::HashMap,
    path::Path,
    convert::TryInto
};

use bytemuck;

#[macro_use]
mod io;
mod mmio;
mod tablegen;
mod videocard;

use crate::devices::cga::tablegen::*;

use crate::bus::{BusInterface, DeviceRunTimeUnit};
use crate::config::VideoType;
use crate::tracelogger::TraceLogger;
use crate::videocard::*;

static DUMMY_PLANE: [u8; 1] = [0];
static DUMMY_PIXEL: [u8; 4] = [0, 0, 0, 0];

// Precalculated waits in system ticks for each of the possible 16 phases of the 
// CGA clock could issue a memory request on. 
static WAIT_TABLE: [u32; 16] = [14,13,12,11,10,9,24,23,22,21,20,19,18,17,16,15];
// in cpu cycles: 5,5,4,4,4,3,8,8,8,7,7,7,6,6,6,5

pub const CGA_MEM_ADDRESS: usize = 0xB8000;
// CGA memory is repeated twice due to incomplete address decoding.
pub const CGA_MEM_APERTURE: usize = 0x8000;
pub const CGA_MEM_SIZE: usize = 0x4000; // 16384 bytes
pub const CGA_MEM_MASK: usize = !0x4000; // Applying this mask will implement memory mirror.

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
const CGA_XRES_MAX: u32 = (CRTC_R0_HORIZONTAL_MAX + 1) * CGA_HCHAR_CLOCK as u32;
const CGA_YRES_MAX: u32 = CRTC_SCANLINE_MAX;
pub const CGA_MAX_CLOCK: usize = (CGA_XRES_MAX * CGA_YRES_MAX) as usize; // Should be 238944

// Monitor sync position. The monitor will eventually perform an hsync at a fixed position 
// if hsync signal is late from the CGA card.
const CGA_MONITOR_HSYNC_POS: u32 = 832;
const CGA_MONITOR_HSYNC_WIDTH: u32 = 80;
const CGA_MONITOR_VSYNC_POS: u32 = 246;
// Minimum scanline value after which we can perform a vsync. A vsync before this scanline will be ignored.
const CGA_MONITOR_VSYNC_MIN: u32 = 0; 

// Display aperature. This is an attempt to represent the maximum visible display extents,
// including overscan. Anything more is likely to be hidden by the monitor bezel or not 
// shown for some other reason. This is mostly calculated based off Area5150's highest
// resolution modes.
const CGA_APERTURE_EXTENT_X: u32 = 768;
const CGA_APERTURE_EXTENT_Y: u32 = 236;

const CGA_APERTURE_CROP_LEFT: u32 = 48;
const CGA_APERTURE_CROP_TOP: u32 = 0;

// For derivision of CGA timings, see https://www.vogons.org/viewtopic.php?t=47052
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

const MODE_MATCH_MASK: u8       = 0b0001_1111;
const MODE_HIRES_TEXT: u8       = 0b0000_0001;
const MODE_GRAPHICS: u8         = 0b0000_0010;
const MODE_BW: u8               = 0b0000_0100;
const MODE_ENABLE: u8           = 0b0000_1000;
const MODE_HIRES_GRAPHICS: u8   = 0b0001_0000;
const MODE_BLINKING: u8         = 0b0010_0000;

const CURSOR_LINE_MASK: u8      = 0b0001_1111;
const CURSOR_ATTR_MASK: u8      = 0b0110_0000;
const CURSOR_ENABLE_MASK: u8    = 0b0010_0000;

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

// Include the standard 8x8 CGA font.
// TODO: Support alternate font with thinner glyphs? It was normally not accessable except 
// by soldering a jumper
const CGA_FONT: &'static [u8] = include_bytes!("../../../../assets/cga_8by8.bin");
const CGA_FONT_SPAN: usize = 256; // Font bitmap is 2048 bits wide (256 * 8 characters)

const CGA_HCHAR_CLOCK: u8 = 8;
const CGA_LCHAR_CLOCK: u8 = 16;
const CRTC_FONT_HEIGHT: u8 = 8;
const CRTC_VBLANK_HEIGHT: u8 = 16;

const CRTC_R0_HORIZONTAL_MAX: u32 = 113;
const CRTC_SCANLINE_MAX: u32 = 262;

// The CGA card decodes different numbers of address lines from the CRTC depending on 
// whether it is in text or graphics modes. This causes wrapping at 0x2000 bytes in 
// text mode, and 0x4000 bytes in graphics modes.
const CGA_TEXT_MODE_WRAP: usize = 0x1FFF;
const CGA_GFX_MODE_WRAP: usize = 0x3FFF;

/*
pub enum CGAColor {
    Black,
    Blue,
    Green,
    Cyan,
    Red,
    Magenta,
    Brown,
    White,
    BlackBright,
    BlueBright,
    GreenBright,
    CyanBright,
    RedBright,
    MagentaBright,
    Yellow,
    WhiteBright
} */

const CGA_PALETTES: [[u8; 4]; 6] = [
    [0, 2, 4, 6],       // Red / Green / Brown
    [0, 10, 12, 14],    // Red / Green / Brown High Intensity
    [0, 3, 5, 7],       // Cyan / Magenta / White
    [0, 11, 13, 15],    // Cyan / Magenta / White High Intensity
    [0, 3, 4, 7],       // Red / Cyan / White
    [0, 11, 12, 15],    // Red / Cyan / White High Intensity
];

const CGA_DEBUG_COLOR: u8 = 5;
const CGA_HBLANK_COLOR: u8 = 0;
const CGA_HBLANK_DEBUG_COLOR: u8 = 1;
const CGA_VBLANK_COLOR: u8 = 0;
const CGA_VBLANK_DEBUG_COLOR: u8 = 14;
const CGA_DISABLE_COLOR: u8 = 0;
const CGA_DISABLE_DEBUG_COLOR: u8 = 2;
const CGA_OVERSCAN_COLOR: u8 = 5;
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

#[derive (Copy, Clone, Debug, PartialEq)]
pub enum ClockMode {
    Pixel,
    Character
}

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
            $self.trace_logger.print(
                &format!(
                    "[SL:{:03} HCC:{:03} VCC:{:03} VT:{:03} VS:{:03}] ", 
                    $self.scanline,
                    $self.hcc_c0,
                    $self.vcc_c4,
                    $self.crtc_vertical_total,
                    $self.crtc_vertical_sync_pos
                )
            );
        }
    };
}

pub(crate) use trace_regs;

pub struct CGACard {
    
    debug: bool,
    cycles: u64,
    last_vsync_cycles: u64,
    cur_screen_cycles: u64,
    cycles_per_vsync: u64,
    sink_cycles: u32,
    catching_up: bool,

    mode_pending: bool,
    clock_pending: bool,
    mode_byte: u8,
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

    frame_count: u64,
    status_reads: u64,

    cursor_status: bool,
    cursor_slowblink: bool,
    cursor_blink_rate: f64,
    cursor_data: [bool; CGA_CURSOR_MAX],
    cursor_attr: u8,

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
    crtc_frame_address: usize,
    in_crtc_hblank: bool,
    in_crtc_vblank: bool,
    in_last_vblank_line: bool,
    hborder: bool,
    vborder: bool,


    cc_register: u8,
    clock_divisor: u8,              // Clock divisor is 1 in high resolution text mode, 2 in all other modes
    clock_mode: ClockMode,
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

    hblank_color: u8,
    vblank_color: u8,
    disable_color: u8,

    overscan_left: u32,
    overscan_right_start: u32,
    overscan_right: u32,
    vsync_len: u32,

    in_display_area: bool,
    cur_char: u8,                   // Current character being drawn
    cur_attr: u8,                   // Current attribute byte being drawn
    cur_fg: u8,                     // Current glyph fg color
    cur_bg: u8,                     // Current glyph bg color
    cur_blink: bool,                // Current glyph blink attribute
    char_col: u8,                   // Column of character glyph being drawn
    hcc_c0: u8,                     // Horizontal character counter (x pos of character)
    vlc_c9: u8,                     // Vertical line counter - row of character being drawn
    vcc_c4: u8,                     // Vertical character counter (y pos of character)
    vsc_c3h: u8,                    // Vertical sync counter - counts during vsync period
    hsc_c3l: u8,                    // Horizontal sync counter - counts during hsync period
    vtac_c5: u8,
    effective_vta: u8,
    vma: usize,                     // VMA register - Video memory address
    vma_t: usize,                   // VMA' register - Video memory address temporary
    vmws: usize,                    // Video memory word size
    rba: usize,                     // Render buffer address
    blink_state: bool,              // Used to control blinking of cursor and text with blink attribute
    blink_accum_us: f64,            // Microsecond accumulator for blink state flipflop
    blink_accum_clocks: u32,        // CGA Clock accumulator for blink state flipflop
    accumulated_us: f64,
    ticks_advanced: u32,            // Number of ticks we have advanced mid-instruction via port or mmio access.
    pixel_clocks_owed: u32,
    ticks_accum: u32,
    clocks_accum: u32,

    mem: Box<[u8; CGA_MEM_SIZE]>,

    back_buf: usize,
    front_buf: usize,
    extents: [DisplayExtents; 2],
    //buf: Vec<Vec<u8>>,
    buf: [Box<[u8; CGA_MAX_CLOCK]>; 2],

    debug_color: u8,

    trace_logger: TraceLogger,
    debug_counter: u64,
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


// CGA implementation of Default for DisplayExtents.
// Each videocard implementation should implement sensible defaults.
// In CGA's case we know the maximum field size and thus row_stride.
impl Default for DisplayExtents {
    fn default() -> Self {
        Self {
            field_w: CGA_XRES_MAX,
            field_h: CGA_YRES_MAX,
            aperture_w: CGA_APERTURE_EXTENT_X,
            aperture_h: CGA_APERTURE_EXTENT_Y,
            aperture_x: CGA_APERTURE_CROP_LEFT,
            aperture_y: CGA_APERTURE_CROP_TOP,
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

    pub fn new(trace_logger: TraceLogger, video_frame_debug: bool) -> Self {

        let mut cga = Self {

            debug: video_frame_debug,
            cycles: 0,
            last_vsync_cycles: 0,
            cur_screen_cycles: 0,
            cycles_per_vsync: 0,
            sink_cycles: 0,
            catching_up: false,

            mode_byte: 0,
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
            scanline_us: 0.0,

            frame_count: 0,
            status_reads: 0,

            cursor_status: false,
            cursor_slowblink: false,
            cursor_blink_rate: CGA_DEFAULT_CURSOR_BLINK_RATE,
            cursor_data: [false; CGA_CURSOR_MAX],
            cursor_attr: 0,

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
            clock_mode: ClockMode::Pixel,
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

            hblank_color: CGA_HBLANK_COLOR,
            vblank_color: CGA_VBLANK_COLOR,
            disable_color: CGA_DISABLE_COLOR,

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

            mem: vec![0; CGA_MEM_SIZE].into_boxed_slice().try_into().unwrap(),

            back_buf: 1,
            front_buf: 0,
            extents: [Default::default(); 2],
            //buf: vec![vec![0; (CGA_XRES_MAX * CGA_YRES_MAX) as usize]; 2],

            // Theoretically, boxed arrays may have some performance advantages over 
            // vectors due to having a fixed size known by the compiler.  However they 
            // are a pain to initialize without overflowing the stack.
            buf: [  
                vec![0; CGA_MAX_CLOCK].into_boxed_slice().try_into().unwrap(),
                vec![0; CGA_MAX_CLOCK].into_boxed_slice().try_into().unwrap()
            ],

            debug_color: 0,

            trace_logger,
            debug_counter: 0
        };

        if video_frame_debug {
            cga.extents[0].aperture_w = CGA_XRES_MAX;
            cga.extents[1].aperture_w = CGA_XRES_MAX;
            cga.extents[0].aperture_h = CGA_YRES_MAX;
            cga.extents[1].aperture_h = CGA_YRES_MAX;
            cga.vblank_color = CGA_VBLANK_DEBUG_COLOR;
            cga.hblank_color = CGA_HBLANK_DEBUG_COLOR;
            cga.disable_color = CGA_DISABLE_DEBUG_COLOR;
        }
        cga
    }

    fn catch_up(&mut self, delta: DeviceRunTimeUnit) {

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
                    log::error!("catch up failed: {} + {}" , self.cycles, phase_offset );
                }

                // Tick a character
                self.tick_char();

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
            self.catching_up = false;
        }
    }

    /// Update the number of pixel clocks we must execute before we can return to clocking the 
    /// CGA card by character clock.  When an IO read/write occurs, the CGA card is updated to
    /// the current clock cycle by ticking pixels. During run() we then have to tick by pixels
    /// until we are back in phase with the character clock.
    #[inline]
    fn calc_cycles_owed(&mut self) -> u32 {

        if self.ticks_advanced % CGA_LCHAR_CLOCK as u32 > 0 {
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
        trace!(
            self,
            "Start address updated: {:04X}",
            self.crtc_start_address
        )
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

                if self.in_crtc_hblank {
                    log::warn!("Warning: SyncWidth modified during hsync!");
                }
                self.crtc_sync_width = byte;
            },
            CRTCRegister::VerticalTotal => {
                // (R4) 7 bit write only
                self.crtc_vertical_total = byte & 0x7F;

                trace_regs!(self);
                trace!(
                    self,
                    "CRTC Register Write (04h): VerticalTotal updated: {}",
                    self.crtc_vertical_total
                )
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

                trace_regs!(self);
                trace!(
                    self,
                    "CRTC Register Write (07h): VerticalSync updated: {}",
                    self.crtc_vertical_sync_pos
                )
            },
            CRTCRegister::InterlaceMode => {
                self.crtc_interlace_mode = byte;
            },            
            CRTCRegister::MaximumScanLineAddress => {
                self.crtc_maximum_scanline_address = byte
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
                trace!(
                    self,
                    "CRTC Register Write (0Ch): StartAddressH updated: {:02X}",
                    byte
                );
                self.update_start_address();
            }
            CRTCRegister::StartAddressL => {
                self.crtc_start_address_lo = byte;
                trace_regs!(self);
                trace!(
                    self,
                    "CRTC Register Write (0Dh): StartAddressL updated: {:02X}",
                    byte
                );                
                self.update_start_address();
            }
            _ => {
                trace!(self, "Write to unsupported CRTC register {:?}: {:02X}", self.crtc_register_selected, byte);
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
        self.mode_graphics  = self.mode_byte & MODE_GRAPHICS != 0;
        self.mode_bw        = self.mode_byte & MODE_BW != 0;
        self.mode_enable    = self.mode_byte & MODE_ENABLE != 0;
        self.mode_hires_gfx = self.mode_byte & MODE_HIRES_GRAPHICS != 0;
        self.mode_blinking  = self.mode_byte & MODE_BLINKING != 0;

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
        self.display_mode = match self.mode_byte & 0b1_0111 {
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

    /// Update the CGA character clock. Can only be done on LCLOCK boundaries to simplify 
    /// our logic.
    #[inline]
    fn update_clock(&mut self) {

        if self.clock_pending && (self.cycles & 0x0F == 0) {
            // Clock divisor is 1 in high res text mode, 2 in all other modes
            // We draw pixels twice when clock divisor is 2 to simulate slower scanning.
            (
                self.clock_divisor, 
                self.char_clock, 
                self.char_clock_mask, 
                self.char_clock_odd_mask
            ) = if self.mode_hires_txt {
                (1, CGA_HCHAR_CLOCK as u32, 0x07, 0x0F)
            }
            else {
                (2, (CGA_HCHAR_CLOCK as u32) * 2, 0x0F, 0x1F)
            };

            self.clock_pending = false;
        }
    }

    /// Handle a write to the CGA mode register. Defer the mode change if it would change 
    /// from graphics mode to text mode or back (Need to measure this on real hardware)
    fn handle_mode_register(&mut self, mode_byte: u8) {
        
        if self.is_deferred_mode_change(mode_byte) {
            // Latch the mode change and mark it pending. We will change the mode on next hsync.
            self.mode_pending = true;
            self.mode_byte = mode_byte;
        }
        else {
            // We're not changing from text to graphcis or vice versa, so we do not have to 
            // defer the update.
            self.mode_byte = mode_byte;
            self.update_mode();
        }
    }

    /// Handle a read from the CGA status register. This register has bits to indicate whether
    /// we are in vblank or if the display is in the active display area (enabled)
    fn handle_status_register_read(&mut self) -> u8 {

        // Bit 1 of the status register is set when the CGA can be safely written to without snow.
        // It is tied to the 'Display Enable' line from the CGA card, inverted.
        // Thus it will be 1 when the CGA card is not currently scanning, IE during both horizontal
        // and vertical refresh.

        // https://www.vogons.org/viewtopic.php?t=47052
        
        // Addendum: The DE line is from the MC6845, and actually includes anything outside of the 
        // active display area. This gives a much wider window to hit for scanline wait loops.
        let byte = if self.in_crtc_vblank {
            STATUS_VERTICAL_RETRACE | STATUS_DISPLAY_ENABLE
        }
        else if !self.in_display_area {
            STATUS_DISPLAY_ENABLE
        }
        else {
            0
        };

        trace_regs!(self);
        trace!(
            self,
            "Status register read: byte: {:02X} in_display_area: {} vblank: {} ",
            byte,
            self.in_display_area, 
            self.in_crtc_vblank
        );

        if self.in_crtc_vblank {
            trace!(
                self,
                "in vblank: vsc: {:03}",
                self.vsc_c3h
            );            
        }

        self.status_reads += 1;

        byte
    }

    /// Handle write to the Color Control register. This register controls the palette selection
    /// and background/overscan color (foreground color in high res graphics mode)
    fn handle_cc_register_write(&mut self, data: u8) {

        self.cc_register = data;
        self.update_palette();

        log::trace!("Write to color control register: {:02X}", data);
    }

    fn update_palette(&mut self) {

        if self.mode_bw && self.mode_graphics && !self.mode_hires_gfx {
            self.cc_palette = 4; // Select Red, Cyan and White palette (undocumented)
        }
        else {
            if self.cc_register & CC_PALETTE_BIT != 0 {
                self.cc_palette = 2; // Select Magenta, Cyan, White palette
            }
            else {
                self.cc_palette = 0; // Select Red, Green, 'Yellow' palette
            }
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
    fn get_glyph_bit(glyph: u8, col: u8, row: u8) -> bool {

        debug_assert!(col < CGA_HCHAR_CLOCK);
        //debug_assert!(row < CRTC_CHAR_CLOCK);
        let row_masked = row & 0x7;

        // Calculate byte offset 
        let glyph_offset: usize = (row_masked as usize * CGA_FONT_SPAN) + glyph as usize;
        CGA_FONT[glyph_offset] & (0x01 << (7 - col)) != 0
    }

    /// Set the character attributes for the current character.
    /// This applies to text mode only, but is computed in all modes at appropriate times.
    fn set_char_addr(&mut self) {

        // Address from CRTC is masked by 0x1FFF by the CGA card (bit 13 ignored) and doubled.
        let addr = (self.vma & CGA_TEXT_MODE_WRAP) << 1;

        if addr < CGA_MEM_SIZE - 1 {
            self.cur_char = self.mem[addr];
            self.cur_attr = self.mem[addr + 1];
    
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
        }
        else {
            log::warn!("Character read out of range!");
        }
        
        //(self.cur_fg, self.cur_bg) = ATTRIBUTE_TABLE[self.cur_attr as usize];
    }

    pub fn draw_overscan_pixel(&mut self) {
        self.buf[self.back_buf][self.rba] = self.cc_overscan_color;

        if self.clock_divisor == 2 {
            // If we are in a 320 column mode, duplicate the last pixel drawn
            self.buf[self.back_buf][self.rba + 1] = self.cc_overscan_color;
        }
    }

    pub fn draw_pixel(&mut self, color: u8) {

        self.buf[self.back_buf][self.rba] = color & 0x0F;

        if self.clock_divisor == 2 {
            // If we are in a 320 column mode, duplicate the last pixel drawn
            self.buf[self.back_buf][self.rba + 1] = color & 0x0F;
        }
    }    

    /*
    #[inline]
    pub fn draw_solid_char(&mut self, color: u8) {

        let draw_span = (8 * self.clock_divisor) as usize;

        for i in 0..draw_span {
            self.buf[self.back_buf][self.rba + i] = color;
        }
    }
    */

    /// Draw a character (8 or 16 pixels) using a single solid color.
    /// Since all pixels are the same we can draw 64 bits at a time.
    #[inline]
    pub fn draw_solid_char(&mut self, color: u8) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);

        frame_u64[self.rba >> 3] = CGA_COLORS_U64[(color & 0x0F) as usize];
        if self.clock_divisor == 2 {
            frame_u64[(self.rba >> 3) + 1] = CGA_COLORS_U64[(color & 0x0F) as usize];
        }
    }

    /// Draw a character in hires mode (8 pixels) using a single solid color.
    /// Since all pixels are the same we can draw 64 bits at a time.
    #[inline]
    pub fn draw_solid_hchar(&mut self, color: u8) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        frame_u64[self.rba >> 3] = CGA_COLORS_U64[(color & 0x0F) as usize];
    }

    /// Draw a character in lowres mode (16 pixels) using a single solid color.
    /// Since all pixels are the same we can draw 64 bits at a time.
    #[inline]
    pub fn draw_solid_lchar(&mut self, color: u8) {
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
        frame_u64[self.rba >> 3] = CGA_COLORS_U64[(color & 0x0F) as usize];
        frame_u64[(self.rba >> 3) + 1] = CGA_COLORS_U64[(color & 0x0F) as usize];
    }    

    /// Draw a single character glyph column pixel in text mode, doubling the pixel if 
    /// in 40 column mode.
    pub fn draw_text_mode_pixel(&mut self) {
        let mut new_pixel = match CGACard::get_glyph_bit(self.cur_char, self.char_col, self.vlc_c9) {
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

        // Do cursor
        if (self.vma == self.crtc_cursor_address) && self.cursor_status && self.blink_state {
            // This cell has the cursor address, cursor is enabled and not blinking
            if self.cursor_data[(self.vlc_c9 & 0x1F) as usize] {
                new_pixel = self.cur_fg;
            }
        }

        if !self.mode_enable {
            new_pixel = 0;
        }

        self.buf[self.back_buf][self.rba] = new_pixel;

        if self.clock_divisor == 2 {
            // If we are in a 320 column mode, duplicate the last pixel drawn
            self.buf[self.back_buf][self.rba + 1] = new_pixel;
        }
    }

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
            glyph_row_base & CGA_COLORS_U64[self.cur_fg as usize] | !glyph_row_base & CGA_COLORS_U64[self.cur_bg as usize]
        }
    }

    /// Get a tuple of 64-bit values representing the specified row of the specified character
    /// glyph in low-resolution (40-column) mode.
    #[inline]
    pub fn get_lchar_glyph_rows(&self, glyph: usize, row: usize) -> (u64, u64) {

        if self.cur_blink && !self.blink_state {
            let glyph = CGA_COLORS_U64[self.cur_bg as usize];
            (glyph, glyph)
        }
        else {
            let glyph_row_base_0 = CGA_LOWRES_GLYPH_TABLE[glyph & 0xFF][0][row];
            let glyph_row_base_1 = CGA_LOWRES_GLYPH_TABLE[glyph & 0xFF][1][row];

            // Combine glyph mask with foreground and background colors.
            let glyph0 = glyph_row_base_0 & CGA_COLORS_U64[self.cur_fg as usize] | !glyph_row_base_0 & CGA_COLORS_U64[self.cur_bg as usize];
            let glyph1 = glyph_row_base_1 & CGA_COLORS_U64[self.cur_fg as usize] | !glyph_row_base_1 & CGA_COLORS_U64[self.cur_bg as usize];

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

    /// Draw an entire character row in high resolution text mode (8 pixels)
    pub fn draw_text_mode_hchar(&mut self) {

        //let draw_span = (8 * self.clock_divisor) as usize;

        // Do cursor if visible, enabled and defined
        if     self.vma == self.crtc_cursor_address
            && self.cursor_status 
            && self.blink_state
            && self.cursor_data[(self.vlc_c9 & 0x1F) as usize] 
        {
            self.draw_solid_hchar(self.cur_fg);
        }
        else if self.mode_enable {

            let glyph_row: u64;
            // Get the u64 glyph row to draw for the current fg and bg colors and character row (vlc)
            glyph_row = self.get_hchar_glyph_row(self.cur_char as usize, self.vlc_c9 as usize);
    
            let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
            frame_u64[self.rba >> 3] = glyph_row;
        }
        else {
            // When mode bit is disabled in text mode, the CGA acts like VRAM is all 0.
            self.draw_solid_hchar(0);
        }
    }

    /// Draw an entire character row in low resolution text mode (16 pixels)
    pub fn draw_text_mode_lchar(&mut self) {

        //let draw_span = (8 * self.clock_divisor) as usize;

        // Do cursor if visible, enabled and defined
        if     self.vma == self.crtc_cursor_address
            && self.cursor_status 
            && self.blink_state
            && self.cursor_data[(self.vlc_c9 & 0x1F) as usize] 
        {
            self.draw_solid_lchar(self.cur_fg);
        }
        else if self.mode_enable {
            // Get the two u64 glyph row components to draw for the current fg and bg colors and character row (vlc)
            let (glyph_row0, glyph_row1) = self.get_lchar_glyph_rows(self.cur_char as usize, self.vlc_c9 as usize);
    
            let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
            frame_u64[self.rba >> 3] = glyph_row0;
            frame_u64[(self.rba >> 3) + 1] = glyph_row1;
        }
        else {
            // When mode bit is disabled in text mode, the CGA acts like VRAM is all 0.
            self.draw_solid_lchar(0);
        }
    }


    /// Draw a pixel in low resolution graphics mode (320x200)
    /// In this mode, pixels are doubled
    pub fn draw_lowres_gfx_mode_pixel(&mut self) {
        let mut new_pixel = self.get_lowres_pixel_color(self.vlc_c9, self.char_col);

        if self.rba >= CGA_MAX_CLOCK - 2 {
            return;
        }

        if !self.mode_enable {
            new_pixel = self.cc_altcolor;
        }

        self.buf[self.back_buf][self.rba] = new_pixel;
        self.buf[self.back_buf][self.rba + 1] = new_pixel;
    }

    /* old implementation

    /// Draw a character column in low resolution graphics mode (320x200)
    /// In this mode, pixels are doubled
    pub fn draw_lowres_gfx_mode_char(&mut self) {

        if self.mode_enable {
            let draw_span = 8 as usize;

            for i in 0..draw_span {
                let new_pixel = self.get_lowres_pixel_color(self.vlc_c9, i as u8);
                self.buf[self.back_buf][self.rba + (i << 1)] = new_pixel;
                self.buf[self.back_buf][self.rba + (i << 1) + 1] = new_pixel;
            }
        }
        else {
            self.draw_solid_char(self.cc_altcolor);
        }
    }
    */

    /// Draw 16 pixels in low res graphics mode (320x200)
    /// This routine uses precalculated lookups and masks to generate two u64
    /// values to write to the index frame buffer directly.
    pub fn draw_lowres_gfx_mode_char(&mut self) {

        if self.mode_enable {

            let lchar_dat = self.get_lowres_gfx_lchar(self.vlc_c9);
            let color0 = lchar_dat.0.0;
            let color1 = lchar_dat.1.0;
            let mask0 = lchar_dat.0.1;
            let mask1 = lchar_dat.1.1;

            let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);
            
            frame_u64[self.rba >> 3] = color0 | (mask0 & CGA_COLORS_U64[self.cc_altcolor as usize]);
            frame_u64[(self.rba >> 3) + 1] = color1 | (mask1 & CGA_COLORS_U64[self.cc_altcolor as usize]);
        }
        else {
            self.draw_solid_char(self.cc_altcolor);
        }
    }    

    /// Draw pixels in high resolution graphics mode. (640x200)
    /// In this mode, two pixels are drawn at the same time.
    pub fn draw_hires_gfx_mode_pixel(&mut self) {

        let base_addr = self.get_gfx_addr(self.vlc_c9);

        let word = (self.mem[base_addr] as u16) << 8 | self.mem[base_addr + 1] as u16;
        
        let bit1 = ((word >> CGA_LCHAR_CLOCK - (self.char_col * 2 + 1))) & 0x01;
        let bit2 = ((word >> CGA_LCHAR_CLOCK - (self.char_col * 2 + 2))) & 0x01;

        if self.mode_enable {
            if bit1 == 0 {
                self.buf[self.back_buf][self.rba] = 0;
            }
            else {
                self.buf[self.back_buf][self.rba] = self.cc_altcolor;
            }

            if bit2 == 0 {
                self.buf[self.back_buf][self.rba + 1] = 0;
            }
            else {
                self.buf[self.back_buf][self.rba + 1] = self.cc_altcolor;
            }
        }
        else {
            self.buf[self.back_buf][self.rba] = self.disable_color;
            self.buf[self.back_buf][self.rba + 1] = self.disable_color;
        }
    }    

    /*
    /// Draw a single character column in high resolution graphics mode (640x200)
    pub fn draw_hires_gfx_mode_char(&mut self) {

        let offset = if self.vlc_c9 > 0 { 0x2000 } else { 0 };
        let base_addr = (((self.vma & 0x3FFF) << 1) + offset) & 0x3FFF;

        if self.rba >= CGA_MAX_CLOCK - 2 {
            return;
        }

        let word = (self.mem[base_addr] as u16) << 8 | self.mem[base_addr + 1] as u16;

        if self.mode_enable {
            for i in 0..CGA_LCHAR_CLOCK {
                let bit1 = ((word >> (CGA_LCHAR_CLOCK - (i + 1))) & 0x01) as usize;
                let bit2 = ((word >> (CGA_LCHAR_CLOCK - (i + 2))) & 0x01) as usize;

                if bit1 == 0 {
                    self.buf[self.back_buf][self.rba + i as usize] = 0;
                }
                else {
                    self.buf[self.back_buf][self.rba + i as usize] = self.cc_altcolor;
                }
    
                if bit2 == 0 {
                    self.buf[self.back_buf][self.rba + i as usize + 1] = 0;
                }
                else {
                    self.buf[self.back_buf][self.rba + i as usize + 1] = self.cc_altcolor;
                }                
            }
        }
        else {
            for i in 0..(CGA_LCHAR_CLOCK as usize) {
                self.buf[self.back_buf][self.rba + i] = self.disable_color;
            }
        } 
    }
    */

    /// Draw a single character column in high resolution graphics mode (640x200)
    pub fn draw_hires_gfx_mode_char(&mut self) {
        
        let base_addr = self.get_gfx_addr(self.vlc_c9);
        let frame_u64: &mut [u64] = bytemuck::cast_slice_mut(&mut *self.buf[self.back_buf]);

        if self.mode_enable {
    
            let byte0 = self.mem[base_addr];
            let byte1 = self.mem[base_addr + 1];

            frame_u64[self.rba >> 3] = CGA_HIRES_GFX_TABLE[self.cc_altcolor as usize][byte0 as usize];
            frame_u64[(self.rba >> 3) + 1] = CGA_HIRES_GFX_TABLE[self.cc_altcolor as usize][byte1 as usize];
        }
        else {
            frame_u64[self.rba >> 3] = 0;
            frame_u64[(self.rba >> 3) + 1] = 0;
        }
    }

    pub fn get_lowres_pixel_color(&self, row: u8, col: u8) -> u8 {

        let base_addr = self.get_gfx_addr(row);

        let word = (self.mem[base_addr] as u16) << 8 | self.mem[base_addr + 1] as u16;

        let idx = ((word >> (CGA_LCHAR_CLOCK - (col + 1) * 2)) & 0x03) as usize;

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
    pub fn get_lowres_gfx_lchar(&self, row: u8) -> (&(u64, u64), &(u64, u64)) {

        let base_addr = self.get_gfx_addr(row);
        (
            &CGA_LOWRES_GFX_TABLE[self.cc_palette as usize][self.mem[base_addr] as usize],
            &CGA_LOWRES_GFX_TABLE[self.cc_palette as usize][self.mem[base_addr + 1] as usize]
        )
    }

    /// Calculate the byte address given the current value of vma; given that the address
    /// programmed into the CRTC start register is interpreted by the CGA as a word address.
    /// In graphics mode, the row counter determines whether address line A12 from the 
    /// CRTC is set. This effectively creates a 0x2000 byte offset for odd character rows.
    #[inline]
    pub fn get_gfx_addr(&self, row: u8) -> usize {
        let row_offset = if (row & 0x01) != 0 { 0x1000 } else { 0 };
        let addr = (self.vma & 0x0FFF | row_offset) << 1;
        addr 
    }

    pub fn get_screen_ticks(&self) -> u64 {
        self.cur_screen_cycles
    }
    
    /*

    /// Execute one CGA character.
    pub fn tick_char(&mut self) {
        
        // sink_cycles must be factor of 8
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

        // Only draw if render buffer address is in bounds.
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
    pub fn tick_char(&mut self) {
        if self.clock_divisor == 2 {
            self.tick_lchar();
        }
        else {
            self.tick_hchar();
        }
    }

    /// Execute one high resolution character clock.
    pub fn tick_hchar(&mut self) {

        // sink_cycles must be factor of 8
        // assert_eq!(self.sink_cycles & 0x07, 0);

        if self.sink_cycles & 0x07 != 0 {
            log::error!("sink_cycles: {} not divisible by 8", self.sink_cycles);
        }

        if self.sink_cycles > 0 {
            self.sink_cycles = self.sink_cycles.saturating_sub(8);
            return
        }

        // Cycles must be a factor of 8 and char_clock == 8
        assert_eq!(self.cycles & 0x07, 0);
        assert_eq!(self.char_clock, 8);        

        self.cycles += 8;
        self.cur_screen_cycles += 8;

        // Only draw if render buffer address is in bounds.
        if self.rba < (CGA_MAX_CLOCK - 8) {
            if self.in_display_area {
                // Draw current character row
                if !self.mode_graphics {
                    self.draw_text_mode_hchar();
                }
                else if self.mode_hires_gfx {
                    self.draw_hires_gfx_mode_char();
                }
                else {
                    self.draw_solid_hchar(self.cc_overscan_color);
                }
            }
            else if self.in_crtc_hblank {
                // Draw hblank in debug color
                self.draw_solid_hchar(self.hblank_color);
            }
            else if self.in_crtc_vblank {
                // Draw vblank in debug color
                self.draw_solid_hchar(self.vblank_color);
            }
            else if self.vborder | self.hborder {
                // Draw overscan
                if self.debug {
                    self.draw_solid_hchar(CGA_OVERSCAN_COLOR);
                }
                else {
                    self.draw_solid_hchar(self.cc_overscan_color);
                }
            }
            else {
                //log::warn!("invalid display state...");
            }
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
            log::error!("tick_hchar(): calling tick_crtc_char but out of phase with cclock: cycles: {} mask: {}", self.cycles, self.char_clock_mask);
        }  

        self.tick_crtc_char();
        self.update_clock();
    }

    /// Execute one low resolution character clock.
    pub fn tick_lchar(&mut self) {
        
        // Cycles must be a factor of 16 and char_clock == 16
        assert_eq!(self.cycles & 0x0F, 0);
        assert_eq!(self.char_clock, 16);

        // sink_cycles must be factor of 8
        //assert!((self.sink_cycles & 0x07) == 0);

        if self.sink_cycles & 0x0F != 0 {
            log::error!("sink_cycles: {} not divisible by 16", self.sink_cycles);
        }

        if self.sink_cycles > 0 {
            self.sink_cycles = self.sink_cycles.saturating_sub(16);
            return
        }

        self.cycles += 16;
        self.cur_screen_cycles += 16;

        // Only draw if render buffer address is in bounds.
        if self.rba < (CGA_MAX_CLOCK - 16) {
            if self.in_display_area {
                // Draw current character row

                if !self.mode_graphics {
                    self.draw_text_mode_lchar();
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
                self.draw_solid_lchar(self.hblank_color);
            }
            else if self.in_crtc_vblank {
                // Draw vblank in debug color
                self.draw_solid_lchar(self.vblank_color);
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
        self.beam_x += 16;
        self.rba += 16;

        // If we have reached the right edge of the 'monitor', return the raster position
        // to the left side of the screen.
        if self.beam_x >= CGA_XRES_MAX {
            self.beam_x = 0;
            self.beam_y += 1;
            self.in_monitor_hsync = false;
            self.rba = (CGA_XRES_MAX * self.beam_y) as usize;
        }

        if self.cycles & self.char_clock_mask != 0 {
            log::error!("tick_lchar(): calling tick_crtc_char but out of phase with cclock: cycles: {} mask: {}", self.cycles, self.char_clock_mask);
        }  

        self.tick_crtc_char();
        self.update_clock();
    }

    pub fn debug_tick2(&mut self) {
        if self.sink_cycles > 0 {
            self.sink_cycles = self.sink_cycles.saturating_sub(1);
            return
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
        }    
    }

    /// Execute one CGA clock cycle.
    pub fn tick(&mut self) {

        if self.sink_cycles > 0 {
            self.sink_cycles = self.sink_cycles.saturating_sub(1);
            return
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
                self.buf[self.back_buf][self.rba] = self.hblank_color;
            }
            else if self.in_crtc_vblank {
                // Draw vblank in debug color
                self.buf[self.back_buf][self.rba] = self.vblank_color;
            }
            else if self.vborder | self.hborder {
                // Draw overscan
                if self.debug {
                    self.draw_pixel(CGA_OVERSCAN_COLOR);
                }
                else {
                    self.draw_overscan_pixel();
                }
            }
            else {
                //log::warn!("invalid display state...");
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
                log::error!("tick(): calling tick_crtc_char but out of phase with cclock: cycles: {} mask: {}", self.cycles, self.char_clock_mask);
            }            
            self.tick_crtc_char();
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
            let hsync_target = if self.clock_divisor == 1 { 
                std::cmp::min(10, self.crtc_sync_width)
            }
            else {
                5
            };

            // Do a horizontal sync
            if self.hsc_c3l == hsync_target {
                // Update the video mode, if an update is pending.
                // It is important not to change graphics mode while we are catching up during an IO instruction.
                if !self.catching_up && self.mode_pending {
                    self.update_mode();
                    self.mode_pending = false;
                }

                if self.in_crtc_vblank {

                    // If we are in vblank, advance Vertical Sync Counter
                    self.vsc_c3h += 1;
                
                    //if self.vsc_c3h == CRTC_VBLANK_HEIGHT || self.beam_y == CGA_MONITOR_VSYNC_POS {
                    if self.vsc_c3h == CRTC_VBLANK_HEIGHT {

                        self.in_last_vblank_line = true;
                        // We are leaving vblank period. Generate a frame.

                        // Previously, we generated frames upon reaching vertical total. This was convenient as 
                        // the display area would be at the top of the render buffer and both overscan periods
                        // beneath it.
                        // However, CRTC tricks like 8088mph rewrite vertical total; this causes multiple 
                        // 'screens' per frame in between vsyncs. To enable these tricks to work, we must render 
                        // like a monitor would.                        

                        self.vsc_c3h = 0;
                        self.do_vsync();
                        return
                    }                        
                }                    
                
                self.scanline += 1;
                
                // Reset beam to left of screen if we haven't already
                if self.beam_x > 0 {
                    self.beam_y += 1;
                }
                self.beam_x = 0;
                self.char_col = 0;

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

            // Save width of right overscan
            if self.beam_x > self.overscan_right_start {
                self.extents[self.front_buf].overscan_r = self.beam_x - self.overscan_right_start;
            }
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

            if self.in_last_vblank_line {
                self.in_last_vblank_line = false;
                self.in_crtc_vblank = false;
            }

            // Reset Horizontal Character Counter and increment character row counter
            self.hcc_c0 = 0;
            self.hborder = false;
            self.vlc_c9 += 1;
            self.extents[self.front_buf].overscan_l = self.beam_x;
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
            
            if self.vlc_c9 > self.crtc_maximum_scanline_address  {
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
                // This represents reaching the lowest visible scanline, so save the scanline in extents.
                self.extents[self.front_buf].visible_h = self.scanline;
                self.in_display_area = false;
                self.vborder = true;
            }
            
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
        }   
    }

    pub fn do_vsync(&mut self) {

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

        // Only do a vsync if we are past the minimum scanline #.
        // A monitor will refuse to vsync too quickly.
        if self.beam_y > CGA_MONITOR_VSYNC_MIN {

            // vblank remains set through the entire last line, including the right overscan of the new screen.
            // So we need to delay resetting vblank flag until then.
            //self.in_crtc_vblank = false;
            
            if self.beam_y > 258 && self.beam_y < 262 {
                // This is a "short" frame. Calculate delta.
                let delta_y = 262 - self.beam_y;
                self.sink_cycles = delta_y * 912;

                if self.cycles & self.char_clock_mask != 0 {
                    log::error!("vsync out of phase with cclock: cycles: {} mask: {}", self.cycles, self.char_clock_mask);
                }
                //log::trace!("sink_cycles: {}", self.sink_cycles);
            }

            self.beam_x = 0;
            self.beam_y = 0;
            self.rba = 0;
            // Write out preliminary DisplayExtents data for new front buffer based on current crtc values.

            // Width is total characters * character width * clock_divisor.
            // This makes the buffer twice as wide as it normally would be in 320 pixel modes, since we scan pixels twice.
            self.extents[self.front_buf].visible_w = 
                self.crtc_horizontal_displayed as u32 * CGA_HCHAR_CLOCK as u32 * self.clock_divisor as u32;

            trace_regs!(self);
            trace!(self, "Leaving vsync and flipping buffers");

            self.scanline = 0;
            self.frame_count += 1;

            // Swap the display buffers
            self.swap();   
        }
    }

}


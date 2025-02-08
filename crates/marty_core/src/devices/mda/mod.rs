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

    --------------------------------------------------------------------------

    devices::mda::mod.rs

    Implementation of the IBM MDA card, built around the Motorola MC6845
    display controller.

*/

use super::mda::attr::*;

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
    device_traits::videocard::*,
    tracelogger::TraceLogger,
};

/*
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
*/

static DUMMY_PLANE: [u8; 1] = [0];
static DUMMY_PIXEL: [u8; 4] = [0, 0, 0, 0];

// Precalculated waits in system ticks for each of the possible 16 phases of the
// CGA clock could issue a memory request on.
static WAIT_TABLE: [u32; 16] = [14, 13, 12, 11, 10, 9, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15];
// in cpu cycles: 5,5,4,4,4,3,8,8,8,7,7,7,6,6,6,5

pub const MDA_MEM_ADDRESS: usize = 0xB0000;
// MDA memory is repeated from B0000-B7FFF due to incomplete address decoding.
pub const MDA_MEM_APERTURE: usize = 0x8000;
pub const MDA_MEM_SIZE: usize = 0x1000; // 4096 bytes
pub const HGC_MEM_SIZE: usize = 0x10000; // 65536 bytes
pub const MDA_MEM_MASK: usize = 0x0FFF; // Applying this mask will implement memory mirror.

pub const HGC_MEM_APERTURE_HALF: usize = 0x8000;
pub const HGC_MEM_APERTURE_FULL: usize = 0x10000;
pub const HGC_MEM_MASK_HALF: usize = 0x7FFF;
pub const HGC_MEM_MASK_FULL: usize = 0xFFFF;

pub const MDA_REPEAT_COL_MASK: u8 = 0b1110_0000;
pub const MDA_REPEAT_COL_VAL: u8 = 0b1100_0000;

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
const HGC_CLOCK: f64 = 16.000;

const US_PER_CLOCK: f64 = 1.0 / MDA_CLOCK;
const US_PER_FRAME: f64 = 1.0 / 50.0;

pub const MDA_BLINK_FAST_RATE: u8 = 8;

pub const MDA_MAX_CLOCK: usize = ((MDA_XRES_MAX * MDA_YRES_MAX) as usize & !0x07) + 8;
pub const HGC_MAX_CLOCK: usize = ((HGC_XRES_MAX * MDA_YRES_MAX) as usize & !0x07) + 8;
//pub const MDA_MAX_CLOCK: usize = 325140; // 16,257,000 / 50

// Calculate the maximum possible area of display field (including refresh period)
const MDA_XRES_MAX: u32 = (CRTC_R0_HORIZONTAL_MAX + 1) * MDA_CHAR_CLOCK as u32; // 882
                                                                                //const HGC_XRES_MAX: u32 = MDA_XRES_MAX - 2;
const HGC_XRES_MAX: u32 = 912;
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

const STATUS_HRETRACE: u8 = 0b0000_0001;
const STATUS_VIDEO: u8 = 0b0000_1000;
// Hercules only
const STATUS_NO_VRETRACE: u8 = 0b1000_0000;

const MDA_FONT: &'static [u8] = include_bytes!("../../../assets/mda_8by14.bin");
const MDA_FONT_SPAN: usize = 256; // Font bitmap is 2048 bits wide (256 * 8 characters)

const MDA_CHAR_CLOCK: u8 = 9;
const HGC_CHAR_CLOCK: u8 = 8;
const CRTC_FONT_HEIGHT: u8 = 14;
const CRTC_VBLANK_HEIGHT: u8 = 16;

const CRTC_R0_HORIZONTAL_MAX: u32 = 97;

// The MDA card decodes 11 address lines off the CRTC chip. This produces 2048 word addresses (4096 bytes)
const MDA_TEXT_MODE_WRAP: usize = 0x07FF;

const HGC_PAGE_SIZE: usize = 0x8000;

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

const MDA_DEBUG_COLOR: u8 = 12;
const MDA_HBLANK_DEBUG_COLOR: u8 = 8;
const MDA_VBLANK_DEBUG_COLOR: u8 = 4;

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

// Display apertures.
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
    // 16Mhz CROPPED aperture
    DisplayAperture {
        w: MDA_APERTURE_CROPPED_W,
        h: MDA_APERTURE_CROPPED_H,
        x: MDA_APERTURE_CROPPED_X,
        y: MDA_APERTURE_CROPPED_Y,
        debug: false,
    },
    // 16Mhz ACCURATE aperture
    DisplayAperture {
        w: MDA_APERTURE_NORMAL_W,
        h: MDA_APERTURE_NORMAL_H,
        x: MDA_APERTURE_NORMAL_X,
        y: MDA_APERTURE_NORMAL_Y,
        debug: false,
    },
    // 16Mhz FULL aperture
    DisplayAperture {
        w: MDA_APERTURE_FULL_W,
        h: MDA_APERTURE_FULL_H,
        x: MDA_APERTURE_FULL_X,
        y: MDA_APERTURE_FULL_Y,
        debug: false,
    },
    // 16Mhz DEBUG aperture
    DisplayAperture {
        w: MDA_APERTURE_DEBUG_W,
        h: MDA_APERTURE_DEBUG_H,
        x: MDA_APERTURE_DEBUG_X,
        y: MDA_APERTURE_DEBUG_Y,
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

#[allow(unused_macros)]
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

use crate::devices::{
    lpt_port::ParallelPort,
    mc6845::{Crtc6845, CrtcStatus, HBlankCallback},
    mda::io::LPT_DEFAULT_IO_BASE,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, BitfieldSpecifier)]
pub enum PageSelect {
    PageB000,
    PageB800,
}

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
    pub unused: B2,
    pub page_select: PageSelect, // HGC Page Select (Half/Full)
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct HercConfigSwitch {
    pub enable_gfx: bool,
    pub enable_page: bool,
    #[skip]
    pub unused: B6,
}

pub struct MDACard {
    subtype: VideoCardSubType,
    mem_mask: usize,
    debug: bool,
    debug_draw: bool,
    cycles: u64,
    last_vsync_cycles: u64,
    cur_screen_cycles: u64,
    cycles_per_vsync: u64,
    sink_cycles: u32,
    catching_up: bool,

    last_rw_tick: u32,
    //rw_slots: [RwSlot; 4],
    slot_idx: usize,

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
    last_bit: bool,

    crtc: Crtc6845,

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
    row_span: u32,
    missed_hsyncs: u32,

    overscan_left: u32,
    overscan_right_start: u32,
    overscan_right: u32,
    vsync_len: u32,

    cur_char:  u8,   // Current character being drawn
    cur_attr:  u8,   // Current attribute byte being drawn
    cur_fg:    u8,   // Current glyph fg color
    cur_bg:    u8,   // Current glyph bg color
    cur_blink: bool, // Current glyph blink attribute
    cur_ul:    bool, // Current glyph underline attribute
    char_col:  u8,   // Column of character glyph being drawn
    hcc_c0:    u8,   // Horizontal character counter (x pos of character)

    vma: usize,               // VMA register - Video memory address
    vmws: usize,              // Video memory word size
    rba: usize,               // Render buffer address
    cursor_blink_state: bool, // Used to control blinking of cursor and text with blink attribute
    text_blink_state: bool,

    accumulated_us: f64,
    ticks_advanced: u32, // Number of ticks we have advanced mid-instruction via port or mmio access.
    pixel_clocks_owed: u32,
    ticks_accum: f64,
    clocks_accum: u32,

    mem: Box<[u8; HGC_MEM_SIZE]>,

    back_buf: usize,
    front_buf: usize,
    extents: DisplayExtents,
    aperture: usize,
    //buf: Vec<Vec<u8>>,
    buf: [Box<[u8; HGC_MAX_CLOCK]>; 2],

    debug_color: u8,

    trace_logger:  TraceLogger,
    debug_counter: u64,

    lightpen_latch: bool,
    lightpen_addr:  usize,

    hblank_fn: Box<HBlankCallback>,

    lpt_port_base: u16,
    lpt: Option<ParallelPort>,

    tmp_color: u8,

    hgc_config: HercConfigSwitch,
    hgc_page_offset: usize,
    hgc_page_flips: u32,
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
            subtype: VideoCardSubType::None,
            mem_mask: MDA_MEM_MASK,
            debug: false,
            debug_draw: true,
            cycles: 0,
            last_vsync_cycles: 0,
            cur_screen_cycles: 0,
            cycles_per_vsync: 0,
            sink_cycles: 0,
            catching_up: false,

            last_rw_tick: 0,
            //rw_slots: [Default::default(); 4],
            slot_idx: 0,

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
            last_bit: false,

            crtc: Crtc6845::new(TraceLogger::None),

            clock_divisor: DEFAULT_CLOCK_DIVISOR,
            clock_mode: ClockingMode::Character,
            char_clock: DEFAULT_CHAR_CLOCK,
            beam_x: 0,
            beam_y: 0,
            in_monitor_hsync: false,
            in_monitor_vblank: false,
            monitor_hsc: 0,
            scanline: 0,
            row_span: MDA_XRES_MAX,
            missed_hsyncs: 0,

            overscan_left: 0,
            overscan_right_start: 0,
            overscan_right: 0,
            vsync_len: 0,
            cur_char: 0,
            cur_attr: 0,
            cur_fg: 0,
            cur_bg: 0,
            cur_blink: false,
            cur_ul: false,
            char_col: 0,
            hcc_c0: 0,

            vma: 0,
            vmws: 2,
            rba: 0,
            cursor_blink_state: false,
            text_blink_state: false,

            accumulated_us: 0.0,
            ticks_advanced: 0,
            ticks_accum: 0.0,
            clocks_accum: 0,
            pixel_clocks_owed: 0,

            mem: vec![0; HGC_MEM_SIZE].into_boxed_slice().try_into().unwrap(),

            back_buf:  1,
            front_buf: 0,
            extents:   MdaDefault::default(),
            aperture:  MDA_DEFAULT_APERTURE,

            //buf: vec![vec![0; (CGA_XRES_MAX * CGA_YRES_MAX) as usize]; 2],

            // Theoretically, boxed arrays may have some performance advantages over
            // vectors due to having a fixed size known by the compiler.  However they
            // are a pain to initialize without overflowing the stack.
            buf: [
                vec![0; HGC_MAX_CLOCK].into_boxed_slice().try_into().unwrap(),
                vec![0; HGC_MAX_CLOCK].into_boxed_slice().try_into().unwrap(),
            ],

            debug_color: 0,

            trace_logger:  TraceLogger::None,
            debug_counter: 0,

            lightpen_latch: false,
            lightpen_addr:  0,

            hblank_fn: Box::new(|| 10),

            lpt_port_base: LPT_DEFAULT_IO_BASE,
            lpt: None,

            tmp_color: 0,

            hgc_config: HercConfigSwitch::new(),
            hgc_page_offset: 0,
            hgc_page_flips: 0,
        }
    }
}

impl MDACard {
    pub fn new(
        subtype: VideoCardSubType,
        trace_logger: TraceLogger,
        clock_mode: ClockingMode,
        lpt: bool,
        video_frame_debug: bool,
    ) -> Self {
        let mut mda = Self::default();

        mda.subtype = subtype;
        if let VideoCardSubType::Hercules = subtype {
            mda.mem_mask = HGC_MEM_MASK_FULL;
        }
        mda.trace_logger = trace_logger;
        mda.debug = video_frame_debug;

        if let ClockingMode::Default = clock_mode {
            mda.clock_mode = ClockingMode::Character;
        }
        else {
            mda.clock_mode = clock_mode;
        }

        if lpt {
            // None IRQ will use default which is 7, correct for MDA LPT
            mda.lpt = Some(ParallelPort::new(None, TraceLogger::None));
        }

        // MDA does not need to cut hblank short for any reason, so always return a big value
        // for hsync width.
        mda.hblank_fn = Box::new(|| 100);

        mda
    }

    /// Reset CGA state (on reboot, for example)
    fn reset_private(&mut self) {
        let trace_logger = std::mem::replace(&mut self.trace_logger, TraceLogger::None);
        let hblank_fn = std::mem::replace(&mut self.hblank_fn, Box::new(|| 10));
        let lpt = std::mem::replace(&mut self.lpt, None);

        // Save non-default values
        *self = Self {
            subtype: self.subtype,
            debug: self.debug,
            clock_mode: self.clock_mode,
            frame_count: self.frame_count, // Keep frame count as to not confuse frontend
            trace_logger,
            extents: self.extents.clone(),
            hblank_fn,
            lpt,
            ..Self::default()
        }
    }

    /*
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
    */

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
        self.crtc.cursor_extents()
    }

    fn get_cursor_address(&self) -> usize {
        self.crtc.cursor_address() as usize
    }

    /// Handle a write to the MDA mode register. Two of the bits are basically useless (0 & 1)
    /// leaving bit 3, which enables or disables video, and Bit 5, which controls blinking.
    fn handle_mode_register(&mut self, mode_byte: u8) {
        log::debug!("Write to MDA mode register: {:02X}", mode_byte);

        let old_bw = self.mode.bw();
        let old_page = self.mode.page_select();

        self.mode = MdaModeRegister::from_bytes([mode_byte]);

        // Don't allow these bits to be set unless enabled and we are a HGC
        if !self.hgc_config.enable_gfx() {
            self.mode.set_bw(old_bw);
        }
        if !self.hgc_config.enable_page() {
            self.mode.set_page_select(old_page);
        }

        if let VideoCardSubType::Hercules = self.subtype {
            let current_page = self.mode.page_select();

            if self.hgc_config.enable_page() {
                self.mem_mask = HGC_MEM_MASK_FULL;

                if current_page != old_page {
                    self.hgc_page_flips += 1;
                }

                // Offset VMA base address based on selected page.
                match current_page {
                    PageSelect::PageB000 => {
                        self.hgc_page_offset = 0;
                    }
                    PageSelect::PageB800 => {
                        self.hgc_page_offset = HGC_PAGE_SIZE;
                    }
                }
            }
            else {
                self.mem_mask = HGC_MEM_MASK_HALF;
            }
        }

        self.mode_graphics = self.mode.bw();
        if self.mode_graphics {
            self.char_clock = HGC_CHAR_CLOCK as u32;
            self.row_span = HGC_XRES_MAX;
            self.extents.field_w = HGC_XRES_MAX;
            self.extents.row_stride = HGC_XRES_MAX as usize;
            //self.clock_divisor = 2;
        }
        else {
            self.char_clock = MDA_CHAR_CLOCK as u32;
            self.row_span = MDA_XRES_MAX;
            self.extents.field_w = MDA_XRES_MAX;
            self.extents.row_stride = MDA_XRES_MAX as usize;
            //self.clock_divisor = 1;
        }
    }

    fn handle_hgc_config_switch(&mut self, data: u8) {
        log::debug!("Write to Hercules configuration switch: {:02X}", data);
        self.hgc_config = HercConfigSwitch::from_bytes([data]);
    }

    /// Handle a read from the MDA status register. This register has bits to indicate whether
    /// we are in vblank or if the display is in the active display area (enabled)
    fn handle_status_register_read(&mut self) -> u8 {
        // Bit 1 of the status register is set when a pixel is being drawn on the screen at that moment.
        // It is essentially similar to the video mux bits on the EGA. It is difficult to emulate this bit
        // when clocking by character. We can record whether a pixel was drawn during the last character tick
        // and use that value.  IBM diagnostics primarily use this bit, but draw a large white box on the screen
        // to give the bit ample time to be detected toggling on and off.

        // Bit 3 is set when the horizontal retrace is active.

        let mut byte = 0x70;
        if let VideoCardSubType::Hercules = self.subtype {
            byte = 0x00;
        }

        if let VideoCardSubType::Hercules = self.subtype {
            if !self.crtc.den() {
                byte |= STATUS_HRETRACE;
            }
        }
        else if self.crtc.hblank() {
            byte |= STATUS_HRETRACE
        }

        if self.last_bit {
            byte |= STATUS_VIDEO;
        }

        if let VideoCardSubType::Hercules = self.subtype {
            if !self.crtc.vblank() {
                byte |= STATUS_NO_VRETRACE;
                // Temporary hack for hercules (Road Runner)
                //byte |= STATUS_VIDEO;
            }
        }

        self.status_reads += 1;

        //trace_regs!(self);
        trace!(
            self,
            "Status register read: byte: {:02X} in_display_area: {} vblank: {} ",
            byte,
            self.crtc.den(),
            self.crtc.vblank()
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
    fn get_glyph_bit(glyph: u8, mut col: u8, row: u8) -> bool {
        if MDACard::is_box_char(glyph) {
            col = if col > 7 { 7 } else { col };
        }
        //debug_assert!(row < CRTC_CHAR_CLOCK);
        let row_masked = row & 0xF; // Font was padded to 16 pixels high.

        // Calculate byte offset
        let glyph_offset: usize = (row_masked as usize * MDA_FONT_SPAN) + glyph as usize;
        let pixel = (MDA_FONT[glyph_offset] & (0x80 >> col)) != 0;
        pixel
    }

    /// Fetch the character and attribute for the specified CRTC address.
    fn fetch_char(&mut self, vma: u16) {
        let addr = (vma as usize & MDA_TEXT_MODE_WRAP) << 1;
        self.cur_char = self.mem[addr];
        self.cur_attr = self.mem[addr + 1];

        if self.mode_blinking {
            self.cur_blink = self.cur_attr & 0x80 != 0;
        }
        else {
            self.cur_blink = false;
        }
        // Bits 0-2 determine underline status
        self.cur_ul = self.cur_attr & 0x03 == 1;
        // Look up fg/bg from attribute table as the logic isn't regular.
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
        let row_offset = (row as usize & 0x03) << 12;
        let addr = ((self.vma & 0x0FFF) | row_offset) << 1;
        addr + self.hgc_page_offset
    }

    #[inline]
    pub fn is_box_char(glyph: u8) -> bool {
        (0xB0u8..=0xDFu8).contains(&glyph)
    }

    pub fn get_screen_ticks(&self) -> u64 {
        self.cur_screen_cycles
    }

    /// Execute one high resolution character clock.
    pub fn tick_hchar(&mut self) {
        self.cycles += self.char_clock as u64;
        self.cur_screen_cycles += self.char_clock as u64;
        self.last_bit = false;

        // Only draw if render address is within display field
        if self.rba < (MDA_MAX_CLOCK - self.char_clock as usize) {
            if self.crtc.den() {
                self.draw_text_mode_hchar_slow();
            }
            else if self.crtc.hblank() {
                // Draw hblank in debug color
                if self.debug_draw {
                    self.draw_solid_hchar(MDA_HBLANK_DEBUG_COLOR);
                }
            }
            else if self.crtc.vblank() {
                // Draw vblank in debug color
                if self.debug_draw {
                    self.draw_solid_hchar(MDA_VBLANK_DEBUG_COLOR);
                }
            }
            else if self.crtc.border() {
                // Draw overscan
                self.draw_solid_hchar(0);
            }
            else {
                self.draw_solid_hchar(MDA_DEBUG_COLOR);
            }
        }

        // Update position to next pixel and character column.
        self.beam_x += self.char_clock as u32;
        self.rba += self.char_clock as usize;

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

        self.handle_crtc_tick();
    }

    /// Execute one high resolution character clock in graphics mode.
    pub fn tick_gchar(&mut self) {
        self.cycles += self.char_clock as u64;
        self.cur_screen_cycles += self.char_clock as u64;
        self.last_bit = false;

        // Only draw if render address is within display field
        if self.rba < (HGC_MAX_CLOCK - self.char_clock as usize) {
            if self.crtc.den() {
                self.draw_hires_gfx_mode_char();
                //self.draw_solid_gchar(MDA_DEBUG_COLOR);
            }
            else if self.crtc.hblank() {
                // Draw hblank in debug color
                if self.debug_draw {
                    self.draw_solid_gchar(MDA_HBLANK_DEBUG_COLOR);
                }
            }
            else if self.crtc.vblank() {
                // Draw vblank in debug color
                if self.debug_draw {
                    self.draw_solid_gchar(MDA_VBLANK_DEBUG_COLOR);
                }
            }
            else if self.crtc.border() {
                // Draw overscan
                //self.draw_solid_gchar(0);
            }
            else {
                self.draw_solid_gchar(MDA_DEBUG_COLOR);
            }
        }

        // Update position to next pixel and character column.
        self.beam_x += 16;
        self.rba += 16;

        // If we have reached the right edge of the 'monitor', return the raster position
        // to the left side of the screen.
        if self.beam_x >= HGC_XRES_MAX {
            self.beam_x = 0;
            self.beam_y += 1;
            self.in_monitor_hsync = false;
            self.rba = (HGC_XRES_MAX * self.beam_y) as usize;
        }

        self.handle_crtc_tick();
    }

    /// Handle the CRTC status after ticking.
    pub fn handle_crtc_tick(&mut self) {
        let (status, vma) = self.crtc.tick(&mut self.hblank_fn);
        // Destructure status so that we can drop the borrow
        let CrtcStatus { hsync, vsync, .. } = *status;
        if vsync {
            //log::warn!(" ************** VSYNC ****************** ");
            self.do_vsync();
        }
        if hsync {
            self.do_hsync();
        }
        self.fetch_char(vma);
        self.vma = vma as usize;
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
            self.draw_pixel(MDA_DEBUG_COLOR);
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
            self.handle_crtc_tick();
        }
    }

    pub fn do_ticks(&mut self, ticks: f64) {
        self.ticks_accum += ticks;
        // Drain the accumulator while emitting chars
        while self.ticks_accum > self.char_clock as f64 {
            if !self.mode_graphics {
                self.tick_hchar();
            }
            else {
                self.tick_gchar();
            }
            self.ticks_accum -= self.char_clock as f64;
        }
    }

    /// Execute one MDA clock cycle.
    pub fn tick(&mut self) {
        if self.sink_cycles > 0 {
            self.sink_cycles = self.sink_cycles.saturating_sub(1);
            return;
        }
        self.cycles += 1;
        self.cur_screen_cycles += 1;

        let saved_rba = self.rba;

        if self.rba < (MDA_MAX_CLOCK - self.clock_divisor as usize) {
            if self.crtc.den() {
                // Draw current pixel
                self.draw_text_mode_pixel();
            }
            else if self.crtc.hblank() {
                // Draw hblank in debug color
                if self.debug_draw {
                    self.buf[self.back_buf][self.rba] = MDA_HBLANK_DEBUG_COLOR;
                }
            }
            else if self.crtc.vblank() {
                // Draw vblank in debug color
                if self.debug_draw {
                    self.buf[self.back_buf][self.rba] = MDA_VBLANK_DEBUG_COLOR;
                }
            }
            else if self.crtc.border() {
                // Draw overscan
                self.draw_overscan_pixel();
            }
            else {
                //log::warn!("tick(): invalid display state...");
                self.draw_pixel(MDA_DEBUG_COLOR);
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
            self.handle_crtc_tick();
            //self.update_clock();
        }
    }

    pub fn do_hsync(&mut self) {
        self.scanline += 1;
        // Reset beam to left of screen if we haven't already
        if self.beam_x > 0 {
            self.beam_y += 1;
        }
        self.beam_x = 0;
        let new_rba = (self.row_span * self.beam_y) as usize;
        self.rba = new_rba;
    }

    pub fn do_vsync(&mut self) {
        self.cycles_per_vsync = self.cur_screen_cycles;
        self.cur_screen_cycles = 0;
        self.last_vsync_cycles = self.cycles;

        self.tmp_color = (self.tmp_color + 1) & 0x0F;

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

            //trace_regs!(self);
            trace!(self, "Leaving vsync and flipping buffers");

            self.scanline = 0;
            self.frame_count += 1;

            // Save the current mode byte, used for composite rendering.
            // The mode could have changed several times per frame, but I am not sure how the composite rendering should
            // really handle that...
            self.extents.mode_byte = self.mode_byte;

            // Toggle blink state. This is toggled every 8 frames by default.
            if (self.frame_count % MDA_DEFAULT_CURSOR_FRAME_CYCLE) == 0 {
                self.cursor_blink_state = !self.cursor_blink_state;
                // Text blink state is 1/2 cursor blink state
                if self.cursor_blink_state {
                    self.text_blink_state = !self.text_blink_state
                }
            }

            // Swap the display buffers
            self.swap();
        }
    }
}

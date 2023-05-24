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

    cga.rs

    Implementation of the IBM CGA card, built around the Motorola MC6845 
    display controller.

*/


#![allow(dead_code)]
use std::collections::HashMap;

use crate::bus::{BusInterface, IoDevice, MemoryMappedDevice, DeviceRunTimeUnit};
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
const CGA_XRES_MAX: u32 = (CRTC_R0_HORIZONTAL_MAX + 1) * CRTC_CHAR_CLOCK as u32;
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
const CGA_DISPLAY_EXTENT_X: u32 = 768;
const CGA_DISPLAY_EXTENT_Y: u32 = 236;

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
const CC_BRIGHT_BIT: u8         = 0b0001_0000; // Controls whether palette is high intensity
const CC_PALETTE_BIT: u8        = 0b0010_0000; // Controls primary palette between magenta/cyan and red/green

const STATUS_DISPLAY_ENABLE: u8         = 0b0000_0001;
const STATUS_LIGHTPEN_TRIGGER_SET: u8   = 0b0000_0010;
const STATUS_LIGHTPEN_SWITCH_STATUS: u8 = 0b0000_0100;
const STATUS_VERTICAL_RETRACE: u8       = 0b0000_1000;

// Include the standard 8x8 CGA font.
// TODO: Support alternate font with thinner glyphs? It was normally not accessable except 
// by soldering a jumper
static CGA_FONT: &'static [u8] = include_bytes!("../assets/cga_8by8.bin");
const CGA_FONT_SPAN: usize = 256; // Font bitmap is 2048 bits wide (256 * 8 characters)

const CRTC_CHAR_CLOCK: u8 = 8;
const CRTC_FONT_HEIGHT: u8 = 8;
const CRTC_VBLANK_HEIGHT: u8 = 16;

const CRTC_R0_HORIZONTAL_MAX: u32 = 113;
const CRTC_SCANLINE_MAX: u32 = 262;

// The CGA card decodes different numbers of address lines from the CRTC depending on 
// whether it is in text or graphics modes. This causes wrapping at 0x2000 bytes in 
// text mode, and 0x4000 bytes in graphics modes.
const CGA_TEXT_MODE_WRAP: usize = 0x1FFF;
const CGA_GFX_MODE_WRAP: usize = 0x3FFF;

const CGA_PALETTES: [[u8; 4]; 6] = [
    [0, 2, 4, 6],       // Red / Green / Brown
    [0, 10, 12, 14],    // Red / Green / Brown High Intensity
    [0, 3, 5, 7],       // Cyan / Magenta / White
    [0, 11, 13, 15],    // Cyan / Magenta / White High Intensity
    [0, 2, 3, 7],       // Red / Cyan / White
    [0, 10, 11, 15],    // Red / Cyan / White High Intensity
];

const CGA_DEBUG_COLOR: u8 = 5;
const CGA_HBLANK_COLOR: u8 = 0;
const CGA_HBLANK_DEBUG_COLOR: u8 = 1;
const CGA_VBLANK_COLOR: u8 = 0;
const CGA_VBLANK_DEBUG_COLOR: u8 = 14;
const CGA_DISABLE_COLOR: u8 = 0;
const CGA_DISABLE_DEBUG_COLOR: u8 = 2;

/*
const CGA_OVERSCAN_COLOR: u8 = 1;
const CGA_FILL_COLOR: u8 = 4;
const CGA_SCANLINE_COLOR: u8 = 13;
*/

macro_rules! trace {
    ($self:ident, $($t:tt)*) => {{
        $self.trace_logger.print(&format!($($t)*));
        $self.trace_logger.print("\n".to_string());
    }};
}

macro_rules! trace_regs {
    ($self:ident) => {
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
    };
}

pub struct CGACard {
    
    debug: bool,
    cycles: u64,
    last_vsync_cycles: u64,
    cur_screen_cycles: u64,
    cycles_per_vsync: u64,
    sink_cycles: u64,

    mode_pending: bool,
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

    mem: Box<[u8; CGA_MEM_SIZE]>,

    back_buf: usize,
    front_buf: usize,
    extents: [DisplayExtents; 2],
    //buf: Vec<Vec<u8>>,
    buf: [Box<[u8; CGA_MAX_CLOCK]>; 2],

    debug_color: u8,

    trace_logger: TraceLogger,
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
    fn read_u8(&mut self, port: u16, delta: DeviceRunTimeUnit) -> u8 {

        // Catch up to CPU state.
        if let DeviceRunTimeUnit::SystemTicks(ticks) = delta {
            //log::debug!("Ticking {} clocks on IO read.", ticks);
            for _ in 0..ticks {
                self.tick();
            }
            self.ticks_advanced += ticks;
        }

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

    fn write_u8(&mut self, port: u16, data: u8, _bus: Option<&mut BusInterface>, delta: DeviceRunTimeUnit) {

        // Catch up to CPU state.
        if let DeviceRunTimeUnit::SystemTicks(ticks) = delta {
            for _ in 0..ticks {
                self.tick();
            }
            self.ticks_advanced += ticks;
        }

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

    fn port_list(&self) -> Vec<u16> {
        vec![
            CRTC_REGISTER_SELECT,
            CRTC_REGISTER,
            CGA_MODE_CONTROL_REGISTER,
            CGA_COLOR_CONTROL_REGISTER,
            CGA_LIGHTPEN_REGISTER,
            CGA_STATUS_REGISTER,
        ]
    }

}

// CGA implementation of Default for DisplayExtents.
// Each videocard implementation should implement sensible defaults.
// In CGA's case we know the maximum field size and thus row_stride.
impl Default for DisplayExtents {
    fn default() -> Self {
        Self {
            field_w: CGA_XRES_MAX,
            field_h: CGA_YRES_MAX,
            aperture_w: CGA_DISPLAY_EXTENT_X,
            aperture_h: CGA_DISPLAY_EXTENT_Y,
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

            mode_byte: 0,
            mode_pending: false,
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

            clock_divisor: 1,

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

            trace_logger
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

    fn get_cursor_span(&self) -> (u8, u8) {
        (self.crtc_cursor_start_line, self.crtc_cursor_end_line)
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

        self.crtc_start_address = ((self.crtc_start_address_ho as usize) << 8 | self.crtc_start_address_lo as usize) & 0x3FFF;

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

    fn update_mode(&mut self) {

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

        // Clock divisor is 1 in high res text mode, 2 in all other modes
        // We draw pixels twice when clock divisor is 2 to simulate slower scanning.
        self.clock_divisor = if self.mode_hires_txt { 1 } else { 2 };

        // Updated mask to exclude enable bit in mode calculation.
        // "Disabled" isn't really a video mode, it just controls whether
        // the CGA card outputs video at a given moment. This can be toggled on
        // and off during a single frame, such as done in VileR's fontcmp.com
        self.display_mode = match self.mode_byte & 0b1_0111 {
            0b0_0100 => DisplayMode::Mode0TextBw40,
            0b0_0000 => DisplayMode::Mode1TextCo40,
            0b0_0101 => DisplayMode::Mode2TextBw80,
            0b0_0001 => DisplayMode::Mode3TextCo80,
            0b0_0010 => DisplayMode::Mode4LowResGraphics,
            0b0_0110 => DisplayMode::Mode5LowResAltPalette,
            0b1_0110 => DisplayMode::Mode6HiResGraphics,
            0b1_0010 => DisplayMode::Mode7LowResComposite,
            _ => {
                trace!(self, "Invalid display mode selected: {:02X}", self.mode_byte & 0x1F);
                log::error!("CGA: Invalid display mode selected: {:02X}", self.mode_byte & 0x1F);
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

        log::debug!("CGA: Mode Selected ({:?}:{:02X}) Enabled: {} Clock: {}", 
            self.display_mode,
            self.mode_byte, 
            self.mode_enable,
            self.clock_divisor
        );
    }

    fn handle_mode_register(&mut self, mode_byte: u8) {

        // Latch the mode change and mark it pending. We will change the mode on next hsync.
        self.mode_byte = mode_byte;
        self.mode_pending = true;
    }

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

    fn handle_cc_register_write(&mut self, data: u8) {
        //log::trace!("Write to color control register: {:02X}", data);

        if data & CC_PALETTE_BIT != 0 {
            self.cc_palette = 2; // Select Magenta, Cyan, White palette
        }
        else {
            self.cc_palette = 0; // Select Red, Green, 'Yellow' palette
        }

        if data & CC_BRIGHT_BIT != 0 {
            self.cc_palette += 1; // Switch to high-intensity palette
        }

        self.cc_altcolor = data & 0x0F;

        if !self.mode_hires_gfx {
            self.cc_overscan_color = self.cc_altcolor;
        }

        self.cc_register = data;
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
        
        self.buf[self.back_buf].fill(0x00);
    }    

    /// Return the bit value at (col,row) of the given font glyph
    fn get_glyph_bit(glyph: u8, col: u8, row: u8) -> bool {

        debug_assert!(col < CRTC_CHAR_CLOCK);
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
            // If blinking is enabled, the bg attribute is only 3 bytes and only low-intensity colors 
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

    pub fn reset_beam(&mut self) {

    }

    pub fn draw_overscan_pixel(&mut self) {
        self.buf[self.back_buf][self.rba] = self.cc_overscan_color;

        if self.clock_divisor == 2 {
            // If we are in a 320 column mode, duplicate the last pixel drawn
            self.buf[self.back_buf][self.rba + 1] = self.cc_overscan_color;
        }
    }

    pub fn draw_debug_pixel(&mut self) {
        //self.buf[self.back_buf][self.rba] = self.cc_overscan_color;
        if self.rba < CGA_MAX_CLOCK - 1 {
            self.buf[self.back_buf][self.rba] = CGA_DEBUG_COLOR;

            if self.clock_divisor == 2 {
                // If we are in a 320 column mode, duplicate the last pixel drawn
                self.buf[self.back_buf][self.rba + 1] = CGA_DEBUG_COLOR;
            }
        }
    }    

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
        if self.vma == self.crtc_cursor_address {
            // This cell has the cursor address
            if self.vlc_c9 >= self.crtc_cursor_start_line && self.vlc_c9 <= self.crtc_cursor_end_line {
                // We are in defined cursor boundaries
                if self.blink_state {
                    // Cursor is not blinked
                    new_pixel = self.cur_fg;
                }
            }
        }

        if !self.mode_enable {
            new_pixel = self.cc_altcolor;
        }

        self.buf[self.back_buf][self.rba] = new_pixel;

        if self.clock_divisor == 2 {
            // If we are in a 320 column mode, duplicate the last pixel drawn
            self.buf[self.back_buf][self.rba + 1] = new_pixel;
        }
    }

    /// Draw a character column in low resolution graphics mode (320x200)
    /// In this mode, one pixel is drawn twice for each character column.
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

    /// Draw a character column in high resolution graphics mode. (640x200)
    /// In this mode, two pixels are drawn for each character column.
    pub fn draw_hires_gfx_mode_pixel(&mut self) {
        let offset = if self.vlc_c9 > 0 { 0x2000 } else { 0 };
        let base_addr = (((self.vma & 0x3FFF) << 1) + offset) & 0x3FFF;

        if self.rba >= CGA_MAX_CLOCK - 2 {
            return;
        }

        let word = (self.mem[base_addr] as u16) << 8 | self.mem[base_addr + 1] as u16;
        
        let bit1 = ((word >> ((CRTC_CHAR_CLOCK * 2) - (self.char_col * 2 + 1))) & 0x01) as usize;
        let bit2 = ((word >> ((CRTC_CHAR_CLOCK * 2) - (self.char_col * 2 + 2))) & 0x01) as usize;

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

    pub fn get_lowres_pixel_color(&self, row: u8, col: u8) -> u8 {

        let offset = if row > 0 { 0x2000 } else { 0 };
        let base_addr = (((self.vma & 0x3FFF) << 1) + offset) & 0x3FFF;

        let word = (self.mem[base_addr] as u16) << 8 | self.mem[base_addr + 1] as u16;

        let idx = ((word >> ((CRTC_CHAR_CLOCK * 2) - (col + 1) * 2)) & 0x03) as usize;

        if idx == 0 {
            self.cc_altcolor
        }
        else {
            CGA_PALETTES[self.cc_palette][idx]
        }
    }

    pub fn get_screen_ticks(&self) -> u64 {
        self.cur_screen_cycles
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
        if self.clock_divisor == 2 && self.cycles & 0x01 == 0 {
            return;
        }

        if self.in_display_area {
            // Draw current pixel
            if self.rba < (CGA_MAX_CLOCK - self.clock_divisor as usize) {

                if !self.is_graphics_mode() {
                    self.draw_text_mode_pixel();
                }
                else if self.mode_hires_gfx {
                    self.draw_hires_gfx_mode_pixel();
                }   
                else {
                    self.draw_lowres_gfx_mode_pixel();
                }
            }
        }
        else if self.in_crtc_hblank {
            // Draw hblank in debug color
            if self.rba < (CGA_MAX_CLOCK - self.clock_divisor as usize) {
                self.buf[self.back_buf][self.rba] = self.hblank_color;
            }
        }
        else if self.in_crtc_vblank {
            // Draw vblank in debug color
            if self.rba < (CGA_MAX_CLOCK - self.clock_divisor as usize) {
                self.buf[self.back_buf][self.rba] = self.vblank_color;
            }
        }
        else if self.vborder | self.hborder {
            // Draw overscan
            if self.rba < (CGA_MAX_CLOCK - self.clock_divisor as usize) {
                if self.debug {
                    self.draw_debug_pixel();
                }
                else {
                    self.draw_overscan_pixel();
                }
            }
        }
        else {
            //log::warn!("invalid display state...");
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

        if self.hcc_c0 == 0 && self.vcc_c4 == 0 {
            // We are at the first character of a CRTC frame. Update start address.
            self.vma = self.crtc_start_address;
        }

        // Done with the current character      
        if self.char_col == CRTC_CHAR_CLOCK {
            
            // Update horizontal character counter
            self.hcc_c0 = self.hcc_c0.wrapping_add(1);
            if self.hcc_c0 == 0 {
                self.hborder = false;
            }

            // Advance video memory address offset and grab the next character + attr
            self.vma += 1;
            self.set_char_addr();

            // Glyph colun reset to 0 for next char
            self.char_col = 0;

            if self.in_crtc_hblank {
                // Increment horizontal sync counter.
                self.hsc_c3l = self.hsc_c3l.wrapping_add(1);

                // End horizontal sync when we reach R3
                if self.hsc_c3l == self.crtc_sync_width {
                    // We've left horizontal blank, enter left overscan.

                    // Update the video mode, if an update is pending.
                    if self.mode_pending {
                        self.update_mode();
                        self.mode_pending = false;
                    }

                    self.hsc_c3l = 0;
                    self.in_crtc_hblank = false;

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
    
                    //log::trace!("hsync!");
                    
                    let new_rba = (CGA_XRES_MAX * self.beam_y) as usize;
                    
                    if new_rba < self.rba {
                        //log::warn!("Warning: Render buffer index would go backwards: old:{:04X} new:{:04X}", self.rba, new_rba );
                        self.rba = new_rba;
                    }
                    else {
                        self.rba = new_rba;
                    }
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
                /*
                if self.vlc_c9 < self.crtc_maximum_scanline_address {
                    // Character row in progress. Load VMA from VMA'
                    self.vma = self.vma_t;
                }
                */

                self.vlc_c9 += 1;

                self.extents[self.front_buf].overscan_l = self.beam_x;

                // Return video memory address to starting position for next character row

                //self.vma = self.crtc_frame_address + (self.vcc_c4 as usize) * (self.crtc_horizontal_displayed as usize);
                self.vma = self.vma_t;
                
                // Reset the current character glyph to start of row
                self.set_char_addr();

                if self.in_crtc_vblank {

                }
                else {
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
                        
                        if self.in_crtc_vblank {
                            // If a vblank is in process, end it
                            //self.vsc_c3h = CRTC_VBLANK_HEIGHT - 1;
                        }

                        if self.crtc_vertical_total > self.crtc_vertical_sync_pos {
                            // Completed a frame.
                            self.hcc_c0 = 0;
                            self.vcc_c4 = 0;
                            self.vtac_c5 = 0;
                            //self.beam_x = 0;
                            self.vlc_c9 = 0;
                            self.char_col = 0;
                            self.crtc_frame_address = self.crtc_start_address;
                            self.vma = self.crtc_start_address;
                            self.vma_t = self.vma;
                            self.in_display_area = true;
                            self.vborder = false;

                            // Load first char + attr
                            self.set_char_addr();
                        }
                        else {
                            // VBlank suppressed by CRTC register shenanigans. 
                            trace_regs!(self);
                            trace!(self, "Vertical total reached: Vblank suppressed");

                            self.hcc_c0 = 0;
                            self.vcc_c4 = 0;
                            self.vtac_c5 = 0;
                            //self.beam_x = 0;
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

                /*
                if self.scanline == CRTC_SCANLINE_MAX {
                    // We have somehow reached the maximum number of possible scanlines in a NTSC field.
                    // I am not sure what happens on real hardware, but in our case, we have to force a frame generation
                    // or we would run off the end of the render buffer.

                    trace_regs!(self);
                    trace!(self, "Maximum scanline reached, frame generation forced.");
                    self.frame_count += 1;

                    // Width is total characters * character width * clock_divisor.
                    // This makes the buffer twice as wide as it normally would be in 320 pixel modes, since we scan pixels twice.
                    self.extents[self.front_buf].visible_w = 
                        self.crtc_horizontal_displayed as u32 * CRTC_CHAR_CLOCK as u32 * self.clock_divisor as u32;

                    // Save last scanline into extents
                    self.extents[self.front_buf].visible_h = self.scanline;          

                    self.scanline = 0;

                    self.hcc_c0 = 0;
                    self.vcc_c4 = 0;
                    self.beam_x = 0;
                    self.vma = 0;
                    self.rba = 0;
                    self.in_display_area = true;

                    // Swap the display buffers
                    self.swap();            
                }
                */
            }
        }        
    }

    pub fn do_vsync(&mut self) {
        
        self.cycles_per_vsync = self.cur_screen_cycles;
        self.cur_screen_cycles = 0;
        self.last_vsync_cycles = self.cycles;

        // Only do a vsync if we are past the minimum scanline #.
        // A monitor will refuse to vsync too quickly.
        if self.beam_y > CGA_MONITOR_VSYNC_MIN {

            // vblank remains set through the entire last line, including the right overscan of the new screen.
            // So we need to delay resetting vblank flag until then.
            //self.in_crtc_vblank = false;
            

            if self.beam_y > 258 && self.beam_y < 262 {
                // This is a "short" frame. Calculate delta.
                let delta_y = 262 - self.beam_y;
                self.sink_cycles = (delta_y * 912) as u64;
            }

            self.beam_x = 0;
            self.beam_y = 0;
            self.rba = 0;
            // Write out preliminary DisplayExtents data for new front buffer based on current crtc values.

            // Width is total characters * character width * clock_divisor.
            // This makes the buffer twice as wide as it normally would be in 320 pixel modes, since we scan pixels twice.
            self.extents[self.front_buf].visible_w = 
                self.crtc_horizontal_displayed as u32 * CRTC_CHAR_CLOCK as u32 * self.clock_divisor as u32;

            trace_regs!(self);
            trace!(self, "Leaving vsync and flipping buffers");

            self.scanline = 0;
            self.frame_count += 1;

            // Swap the display buffers
            self.swap();   
        }
    }

}

// Helper macro for pushing video card state entries. 
// For CGA, we put the decorator first as there is only one register file an we use it to show the register index.
macro_rules! push_reg_str {
    ($vec: expr, $reg: expr, $decorator: expr, $val: expr ) => {
        $vec.push((format!("{} {:?}", $decorator, $reg ), VideoCardStateEntry::String(format!("{}", $val))))
    };
}

/*
macro_rules! push_reg_str_bin8 {
    ($vec: expr, $reg: expr, $decorator: expr, $val: expr ) => {
        $vec.push((format!("{:?} {}", $reg, $decorator), VideoCardStateEntry::String(format!("{:08b}", $val))))
    };
}
*/

macro_rules! push_reg_str_enum {
    ($vec: expr, $reg: expr, $decorator: expr, $val: expr ) => {
        $vec.push((format!("{:?} {}", $reg, $decorator), VideoCardStateEntry::String(format!("{:?}", $val))))
    };
}   

impl VideoCard for CGACard {

    fn get_video_type(&self) -> VideoType {
        VideoType::CGA
    }

    fn get_render_mode(&self) -> RenderMode {
        RenderMode::Direct
    }

    fn get_display_mode(&self) -> DisplayMode {
        self.display_mode
    }

    fn get_display_size(&self) -> (u32, u32) {

        // CGA supports a single fixed 8x8 font. The size of the displayed window 
        // is always HorizontalDisplayed * (VerticalDisplayed * (MaximumScanlineAddress + 1))
        // (Excepting fancy CRTC tricks that delay vsync)
        let mut width = self.crtc_horizontal_displayed as u32 * CRTC_CHAR_CLOCK as u32;
        let height = self.crtc_vertical_displayed as u32 * (self.crtc_maximum_scanline_address as u32 + 1);

        if self.mode_hires_gfx {
            width = width * 2;
        }
        (width, height)
    }

    fn get_display_extents(&self) -> &DisplayExtents {
        &self.extents[self.back_buf]
    }

    fn get_display_aperture(&self) -> (u32, u32) {
        (self.extents[0].aperture_w, self.extents[0].aperture_h)
    }

    /// Get the position of the electron beam.
    fn get_beam_pos(&self) -> Option<(u32, u32)> {
        Some((self.beam_x, self.beam_y))
    }

    /// Tick the CGA the specified number of video clock cycles.
    fn debug_tick(&mut self, ticks: u32) {

        for _ in 0..ticks {
            self.tick();
        }
    }

    #[inline]
    fn get_overscan_color(&self) -> u8 {
        if self.mode_hires_gfx {
            // In highres mode, the color control register controls the foreground color, not overscan
            // so overscan must be black.
            0
        }
        else {
            self.cc_altcolor
        }
    }

    /// Get the current scanline being rendered.
    fn get_scanline(&self) -> u32 {
        self.scanline
    }

    /// Return whether or not to double scanlines for this video device. For CGA, this is always
    /// true.
    fn get_scanline_double(&self) -> bool {
        true
    }

    /// Return the u8 slice representing the front buffer of the device. (Direct rendering only)
    fn get_display_buf(&self) -> &[u8] {
        &self.buf[self.front_buf][..]
    }

    /// Return the u8 slice representing the back buffer of the device. (Direct rendering only)
    /// This is used during debug modes when the cpu is paused/stepping so we can follow drawing
    /// progress.    
    fn get_back_buf(&self) -> &[u8] {

        &self.buf[self.back_buf][..]
    }  

    /// Get the current display refresh rate of the device. For CGA, this is always 60.
    fn get_refresh_rate(&self) -> u32 {
        60
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
                    pos_x: (addr % 40) as u32,
                    pos_y: (addr / 40) as u32,
                    line_start: self.crtc_cursor_start_line,
                    line_end: self.crtc_cursor_end_line,
                    visible: self.get_cursor_status()
                }
            }
            DisplayMode::Mode2TextBw80 | DisplayMode::Mode3TextCo80 => {
                CursorInfo{
                    addr,
                    pos_x: (addr % 80) as u32,
                    pos_y: (addr / 80) as u32,
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
            w: CRTC_CHAR_CLOCK as u32,
            h: CRTC_FONT_HEIGHT as u32,
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

    fn get_videocard_string_state(&self) -> HashMap<String, Vec<(String, VideoCardStateEntry)>> {

        let mut map = HashMap::new();
        
        let mut general_vec = Vec::new();

        general_vec.push((format!("Adapter Type:"), VideoCardStateEntry::String(format!("{:?}", self.get_video_type()))));
        general_vec.push((format!("Display Mode:"), VideoCardStateEntry::String(format!("{:?}", self.get_display_mode()))));
        general_vec.push((format!("Video Enable:"), VideoCardStateEntry::String(format!("{:?}", self.mode_enable))));
        general_vec.push((format!("Clock Divisor:"), VideoCardStateEntry::String(format!("{}", self.clock_divisor))));
        general_vec.push((format!("Frame Count:"), VideoCardStateEntry::String(format!("{}", self.frame_count))));
        map.insert("General".to_string(), general_vec);

        let mut crtc_vec = Vec::new();

        push_reg_str!(crtc_vec, CRTCRegister::HorizontalTotal, "[R0]", self.crtc_horizontal_total);
        push_reg_str!(crtc_vec, CRTCRegister::HorizontalDisplayed, "[R1]", self.crtc_horizontal_displayed);
        push_reg_str!(crtc_vec, CRTCRegister::HorizontalSyncPosition, "[R2]", self.crtc_horizontal_sync_pos);
        push_reg_str!(crtc_vec, CRTCRegister::SyncWidth, "[R3]", self.crtc_sync_width);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalTotal, "[R4]", self.crtc_vertical_total);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalTotalAdjust, "[R5]", self.crtc_vertical_total_adjust);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalDisplayed, "[R6]", self.crtc_vertical_displayed);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalSync, "[R7]", self.crtc_vertical_sync_pos);
        push_reg_str!(crtc_vec, CRTCRegister::InterlaceMode, "[R8]", self.crtc_interlace_mode);
        push_reg_str!(crtc_vec, CRTCRegister::MaximumScanLineAddress, "[R9]", self.crtc_maximum_scanline_address);
        push_reg_str!(crtc_vec, CRTCRegister::CursorStartLine, "[R10]", self.crtc_cursor_start_line);
        push_reg_str!(crtc_vec, CRTCRegister::CursorEndLine, "[R11]", self.crtc_cursor_end_line);
        push_reg_str!(crtc_vec, CRTCRegister::StartAddressH, "[R12]", self.crtc_start_address_ho);
        push_reg_str!(crtc_vec, CRTCRegister::StartAddressL, "[R13]", self.crtc_start_address_lo);
        crtc_vec.push(("Start Address".to_string(), VideoCardStateEntry::String(format!("{:04X}", self.crtc_start_address))));
        push_reg_str!(crtc_vec, CRTCRegister::CursorAddressH, "[R14]", self.crtc_cursor_address_ho);
        push_reg_str!(crtc_vec, CRTCRegister::CursorAddressL, "[R15]", self.crtc_cursor_address_lo);
        map.insert("CRTC".to_string(), crtc_vec);

        let mut internal_vec = Vec::new();

        internal_vec.push((format!("hcc_c0:"), VideoCardStateEntry::String(format!("{}", self.hcc_c0))));
        internal_vec.push((format!("vlc_c9:"), VideoCardStateEntry::String(format!("{}", self.vlc_c9))));
        internal_vec.push((format!("vcc_c4:"), VideoCardStateEntry::String(format!("{}", self.vcc_c4))));
        internal_vec.push((format!("scanline:"), VideoCardStateEntry::String(format!("{}", self.scanline))));
        internal_vec.push((format!("vsc_c3h:"), VideoCardStateEntry::String(format!("{}", self.vsc_c3h))));
        internal_vec.push((format!("hsc_c3l:"), VideoCardStateEntry::String(format!("{}", self.hsc_c3l))));
        internal_vec.push((format!("vtac_c5:"), VideoCardStateEntry::String(format!("{}", self.vtac_c5))));
        internal_vec.push((format!("vma:"), VideoCardStateEntry::String(format!("{:04X}", self.vma))));
        internal_vec.push((format!("vma':"), VideoCardStateEntry::String(format!("{:04X}", self.vma_t))));
        internal_vec.push((format!("vmws:"), VideoCardStateEntry::String(format!("{}", self.vmws))));
        internal_vec.push((format!("rba:"), VideoCardStateEntry::String(format!("{:04X}", self.rba))));
        internal_vec.push((format!("de:"), VideoCardStateEntry::String(format!("{}", self.in_display_area))));
        internal_vec.push((format!("crtc_hblank:"), VideoCardStateEntry::String(format!("{}", self.in_crtc_hblank))));
        internal_vec.push((format!("crtc_vblank:"), VideoCardStateEntry::String(format!("{}", self.in_crtc_vblank))));
        internal_vec.push((format!("beam_x:"), VideoCardStateEntry::String(format!("{}", self.beam_x))));
        internal_vec.push((format!("beam_y:"), VideoCardStateEntry::String(format!("{}", self.beam_y))));
        internal_vec.push((format!("border:"), VideoCardStateEntry::String(format!("{}", self.hborder))));
        internal_vec.push((format!("s_reads:"), VideoCardStateEntry::String(format!("{}", self.status_reads))));
        internal_vec.push((format!("missed_hsyncs:"), VideoCardStateEntry::String(format!("{}", self.missed_hsyncs))));
        internal_vec.push((format!("vsync_cycles:"), VideoCardStateEntry::String(format!("{}", self.cycles_per_vsync))));
        internal_vec.push((format!("cur_screen_cycles:"), VideoCardStateEntry::String(format!("{}", self.cur_screen_cycles))));
        internal_vec.push((format!("phase:"), VideoCardStateEntry::String(format!("{}", self.cycles & 0x0F))));

        map.insert("Internal".to_string(), internal_vec);

        map       
    }

    fn run(&mut self, time: DeviceRunTimeUnit) {

        let mut clocks = if let DeviceRunTimeUnit::SystemTicks(ticks) = time {
            ticks
        }
        else {
            panic!("CGA requires SystemTicks time unit.")
        };

        if clocks == 0 {
            panic!("CGA run() with 0 ticks");
        }

        if self.ticks_advanced > clocks {
            panic!("Impossible condition: ticks_advanced > clocks");
        }

        clocks -= self.ticks_advanced;
        self.ticks_advanced = 0;

        /*
        self.accumulated_us += us;

        // Handle blinking. 
        self.blink_accum_us += us;
        if self.blink_accum_us > CGA_CURSOR_BLINK_RATE_US {
            self.blink_state = !self.blink_state;
            self.blink_accum_us -= CGA_CURSOR_BLINK_RATE_US;
        }

        // Tick the CRTC. Since the CGA is much faster clocked than the CPU this will 
        // probably happen several times per CPU instruction.
        while self.accumulated_us > (US_PER_CLOCK * self.clock_divisor as f64) {

            self.tick();
            self.accumulated_us -= (US_PER_CLOCK * self.clock_divisor as f64);
        }        
        */

        // Handle blinking. TODO: Move blink handling into tick().
        self.blink_accum_clocks += clocks;
        if self.blink_accum_clocks > CGA_CURSOR_BLINK_RATE_CLOCKS {
            self.blink_state = !self.blink_state;
            self.blink_accum_clocks -= CGA_CURSOR_BLINK_RATE_CLOCKS;
        }

        // Tick the card.
        for _ in 0..clocks {
            self.tick();
        }

    }

    fn reset(&mut self) {
        log::debug!("Resetting")
    }

    fn get_pixel(&self, _x: u32, _y:u32) -> &[u8] {
        &DUMMY_PIXEL
    }

    fn get_pixel_raw(&self, _x: u32, _y:u32) -> u8 {
        0
    }

    fn get_plane_slice(&self, _plane: usize) -> &[u8] {
        &DUMMY_PLANE
    }

    fn get_frame_count(&self) -> u64 {
        self.frame_count
    }

    fn dump_mem(&self) {
        let filename = format!("./dumps/cga.bin");
        
        match std::fs::write(filename.clone(), &*self.mem) {
            Ok(_) => {
                log::debug!("Wrote memory dump: {}", filename)
            }
            Err(e) => {
                log::error!("Failed to write memory dump '{}': {}", filename, e)
            }
        }
    }

    fn write_trace_log(&mut self, msg: String) {
        self.trace_logger.print(msg);
    }    

    fn trace_flush(&mut self) {
        self.trace_logger.flush();
    }

}

/// Unlike the EGA or VGA the CGA doesn't do any operations on video memory on read/write,
/// but we handle the mirroring of VRAM this way, and for consistency with other devices
impl MemoryMappedDevice for CGACard {

    fn get_read_wait(&mut self, _address: usize, cycles: u32) -> u32 {
        // Look up wait states given the last ticked clock cycle + elapsed cycles
        // passed in.
        let phase = (self.cycles + cycles as u64 + 1) as usize & (0x0F as usize);
        let waits = WAIT_TABLE[phase];

        trace!(
            self, 
            "READ_U8 (T2): PHASE: {:02X}, WAITS: {}", 
            phase,
            waits
        );
        waits
    }

    fn get_write_wait(&mut self, _address: usize, cycles: u32) -> u32 {
        // Look up wait states given the last ticked clock cycle + elapsed cycles
        // passed in.
        let phase = (self.cycles + cycles as u64 + 1) as usize & (0x0F as usize);
        let waits = WAIT_TABLE[phase];

        trace!(
            self, 
            "WRITE_U8 (T2): PHASE: {:02X}, WAITS: {}", 
            phase,
            waits
        );
        waits
    }

    fn read_u8(&mut self, address: usize, cycles: u32) -> (u8, u32) {

        let a_offset = (address & CGA_MEM_MASK) - CGA_MEM_ADDRESS;
        if a_offset < CGA_MEM_SIZE {
            // Read within memory range

            // Look up wait states given the last ticked clock cycle + elapsed cycles
            // passed in.
            let phase = (self.cycles + cycles as u64 + 1) as usize & (0x0F as usize);
            let waits = WAIT_TABLE[phase];

            trace!(
                self, 
                "READ_U8: {:04X}:{:02X} PHASE: {:02X}, WAITS: {}", 
                a_offset, 
                self.mem[a_offset],
                phase,
                waits
            );
            (self.mem[a_offset], waits)
        }
        else {
            // Read out of range, shouldn't happen...
            (0xFF, 0)
        }
    }

    fn write_u8(&mut self, address: usize, byte: u8, cycles: u32) -> u32 {
        let a_offset = (address & CGA_MEM_MASK) - CGA_MEM_ADDRESS;
        if a_offset < CGA_MEM_SIZE {
            self.mem[a_offset] = byte;

            // Look up wait states given the last ticked clock cycle + elapsed cycles
            // passed in.
            let phase = (self.cycles + cycles as u64 + 1) as usize & (0x0F as usize);
            trace!(
                self, 
                "WRITE_U8: {:04X}:{:02X} PHASE: {:02X}, WAITS: {}", 
                a_offset, 
                byte,
                phase,
                WAIT_TABLE[phase]
            );            
            WAIT_TABLE[phase]
        }
        else {
            // Write out of range, shouldn't happen...
            0
        }
    }

    fn read_u16(&mut self, address: usize, _cycles: u32) -> (u16, u32) {

        let (lo_byte, wait1) = MemoryMappedDevice::read_u8(self, address, 0);
        let (ho_byte, wait2) = MemoryMappedDevice::read_u8(self, address + 1, 0);

        log::warn!("Unsupported 16 bit read from VRAM");
        return ((ho_byte as u16) << 8 | lo_byte as u16, wait1 + wait2)
    }    

    fn write_u16(&mut self, _address: usize, _data: u16, _cycles: u32) -> u32 {
        //trace!(self, "16 byte write to VRAM, {:04X} -> {:05X} ", data, address);
        log::warn!("Unsupported 16 bit write to VRAM");
        0
    }

}
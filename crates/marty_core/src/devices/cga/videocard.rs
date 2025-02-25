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

    devices::cga::videocard.rs

    Implements the VideoCard trait for the IBM CGA card.

*/

use super::*;
use crate::{device_traits::videocard::*, devices::pic::Pic};

// Helper macro for pushing video card state entries.
// For CGA, we put the decorator first as there is only one register file an we use it to show the register index.
macro_rules! push_reg_str {
    ($vec: expr, $reg: expr, $decorator: expr, $val: expr ) => {
        $vec.push((
            format!("{} {:?}", $decorator, $reg),
            VideoCardStateEntry::String(format!("{}", $val)),
        ))
    };
}

/*
macro_rules! push_reg_str_bin8 {
    ($vec: expr, $reg: expr, $decorator: expr, $val: expr ) => {
        $vec.push((String::from("{:?} {}", $reg, $decorator), VideoCardStateEntry::String(format!("{:08b}", $val))))
    };
}

macro_rules! push_reg_str_enum {
    ($vec: expr, $reg: expr, $decorator: expr, $val: expr ) => {
        $vec.push((String::from("{:?} {}", $reg, $decorator), VideoCardStateEntry::String(format!("{:?}", $val))))
    };
}
*/

impl VideoCard for CGACard {
    fn get_sync(&self) -> (bool, bool, bool, bool) {
        (
            self.in_crtc_vblank,
            self.in_crtc_hblank,
            self.in_display_area,
            self.hborder | self.vborder,
        )
    }

    fn set_video_option(&mut self, opt: VideoOption) {
        match opt {
            VideoOption::EnableSnow(state) => {
                log::debug!("VideoOption::EnableSnow set to: {}", state);
                self.enable_snow = state;
            }
            VideoOption::DebugDraw(state) => {
                log::debug!("VideoOption::DebugDraw set to: {}", state);
                self.debug_draw = state;
            }
        }
    }

    fn get_video_type(&self) -> VideoType {
        VideoType::CGA
    }

    fn get_render_mode(&self) -> RenderMode {
        RenderMode::Direct
    }

    fn get_render_depth(&self) -> RenderBpp {
        RenderBpp::Four
    }

    fn get_display_mode(&self) -> DisplayMode {
        self.display_mode
    }

    fn set_clocking_mode(&mut self, mode: ClockingMode) {
        // TODO: Switching from cycle clocking mode to character clocking mode
        // must be deferred until character-clock boundaries.
        // For now we only support falling back to cycle clocking mode and
        // staying there.
        log::debug!("Clocking mode set to: {:?}", mode);
        self.clock_mode = mode;
    }

    fn get_display_size(&self) -> (u32, u32) {
        // CGA supports a single fixed 8x8 font. The size of the displayed window
        // is always HorizontalDisplayed * (VerticalDisplayed * (MaximumScanlineAddress + 1))
        // (Excepting fancy CRTC tricks that delay vsync)
        let mut width = self.crtc_horizontal_displayed as u32 * CGA_HCHAR_CLOCK as u32;
        let height = self.crtc_vertical_displayed as u32 * (self.crtc_maximum_scanline_address as u32 + 1);

        if self.mode_hires_gfx {
            width = width * 2;
        }
        (width, height)
    }

    fn get_display_extents(&self) -> &DisplayExtents {
        &self.extents
    }

    fn list_display_apertures(&self) -> Vec<DisplayApertureDesc> {
        CGA_APERTURE_DESCS.to_vec()
    }

    fn get_display_apertures(&self) -> Vec<DisplayAperture> {
        self.extents.apertures.clone()
    }

    /// Get the position of the electron beam.
    fn get_beam_pos(&self) -> Option<(u32, u32)> {
        Some((self.beam_x, self.beam_y))
    }

    /// Tick the CGA the specified number of video clock cycles.
    fn debug_tick(&mut self, ticks: u32, _cpumem: Option<&[u8]>) {
        match self.clock_mode {
            ClockingMode::Character | ClockingMode::Dynamic => {
                let pixel_ticks = ticks % CGA_LCHAR_CLOCK as u32;
                let lchar_ticks = ticks / CGA_LCHAR_CLOCK as u32;

                assert_eq!(ticks, pixel_ticks + (lchar_ticks * 16));

                for _ in 0..pixel_ticks {
                    self.tick();
                }
                for _ in 0..lchar_ticks {
                    if self.clock_divisor == 2 {
                        self.tick_lchar();
                    } else {
                        self.tick_hchar();
                        self.tick_hchar();
                    }
                }
            }
            ClockingMode::Cycle => {
                for _ in 0..ticks {
                    self.tick();
                }
            }
            _ => {}
        }

        log::warn!(
            "debug_tick(): new cur_screen_cycles: {} beam_x: {} beam_y: {}",
            self.cur_screen_cycles,
            self.beam_x,
            self.beam_y
        );
    }

    #[inline]
    fn get_overscan_color(&self) -> u8 {
        if self.mode_hires_gfx {
            // In highres mode, the color control register controls the foreground color, not overscan
            // so overscan must be black.
            0
        } else {
            self.cc_altcolor
        }
    }

    /// Get the current scanline being rendered.
    fn get_scanline(&self) -> u32 {
        self.scanline
    }

    /// Return whether to double scanlines for this video device. For CGA, this is always true.
    fn get_scanline_double(&self) -> bool {
        true
    }

    /// Return the u8 slice representing the requested buffer type.
    fn get_buf(&self, buf_select: BufferSelect) -> &[u8] {
        match buf_select {
            BufferSelect::Back => &self.buf[self.back_buf][..],
            BufferSelect::Front => &self.buf[self.front_buf][..],
        }
    }

    /// Return the u8 slice representing the front buffer of the device. (Direct rendering only)
    fn get_display_buf(&self) -> &[u8] {
        &self.buf[self.front_buf][..]
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
            _ => false,
        }
    }

    #[inline]
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
            DisplayMode::Mode0TextBw40 | DisplayMode::Mode1TextCo40 => CursorInfo {
                addr,
                pos_x: (addr % 40) as u32,
                pos_y: (addr / 40) as u32,
                line_start: self.crtc_cursor_start_line,
                line_end: self.crtc_cursor_end_line,
                visible: self.get_cursor_status(),
            },
            DisplayMode::Mode2TextBw80 | DisplayMode::Mode3TextCo80 => CursorInfo {
                addr,
                pos_x: (addr % 80) as u32,
                pos_y: (addr / 80) as u32,
                line_start: self.crtc_cursor_start_line,
                line_end: self.crtc_cursor_end_line,
                visible: self.get_cursor_status(),
            },
            _ => {
                // Not a valid text mode
                CursorInfo {
                    addr: 0,
                    pos_x: 0,
                    pos_y: 0,
                    line_start: 0,
                    line_end: 0,
                    visible: false,
                }
            }
        }
    }

    fn get_clock_divisor(&self) -> u32 {
        1
    }

    fn get_current_font(&self) -> Option<FontInfo> {
        Some(FontInfo {
            w: CGA_HCHAR_CLOCK as u32,
            h: CRTC_FONT_HEIGHT as u32,
            font_data: CGA_FONT,
        })
    }

    fn get_palette(&self) -> Option<Vec<[u8; 4]>> {
        None
    }

    fn get_character_height(&self) -> u8 {
        self.crtc_maximum_scanline_address + 1
    }

    #[rustfmt::skip]
    fn get_videocard_string_state(&self) -> HashMap<String, Vec<(String, VideoCardStateEntry)>> {
        let mut map = HashMap::new();

        let mut general_vec = Vec::new();

        general_vec.push((String::from("Adapter Type:"), VideoCardStateEntry::String(format!("{:?}", self.get_video_type()))));
        general_vec.push((String::from("Display Mode:"), VideoCardStateEntry::String(format!("{:?}", self.get_display_mode()))));
        general_vec.push((String::from("Video Enable:"), VideoCardStateEntry::String(format!("{:?}", self.mode_enable))));
        general_vec.push((String::from("Clock Divisor:"), VideoCardStateEntry::String(format!("{}", self.clock_divisor))));
        general_vec.push((String::from("Frame Count:"), VideoCardStateEntry::String(format!("{}", self.frame_count))));
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

        internal_vec.push((String::from("hcc_c0:"), VideoCardStateEntry::String(format!("{}", self.hcc_c0))));
        internal_vec.push((String::from("vlc_c9:"), VideoCardStateEntry::String(format!("{}", self.vlc_c9))));
        internal_vec.push((String::from("last_row:"), VideoCardStateEntry::String(format!("{}", self.last_row))));
        internal_vec.push((String::from("last_line:"), VideoCardStateEntry::String(format!("{}", self.last_line))));
        internal_vec.push((String::from("vcc_c4:"), VideoCardStateEntry::String(format!("{}", self.vcc_c4))));
        internal_vec.push((String::from("scanline:"), VideoCardStateEntry::String(format!("{}", self.scanline))));
        internal_vec.push((String::from("vsc_c3h:"), VideoCardStateEntry::String(format!("{}", self.vsc_c3h))));
        internal_vec.push((String::from("hsc_c3l:"), VideoCardStateEntry::String(format!("{}", self.hsc_c3l))));
        internal_vec.push((String::from("vtac_c5:"), VideoCardStateEntry::String(format!("{}", self.vtac_c5))));
        internal_vec.push((String::from("vma:"), VideoCardStateEntry::String(format!("{:04X}", self.vma))));
        internal_vec.push((String::from("vma':"), VideoCardStateEntry::String(format!("{:04X}", self.vma_t))));
        internal_vec.push((String::from("rba:"), VideoCardStateEntry::String(format!("{:04X}", self.rba))));
        internal_vec.push((String::from("de:"), VideoCardStateEntry::String(format!("{}", self.in_display_area))));
        internal_vec.push((String::from("crtc_hblank:"), VideoCardStateEntry::String(format!("{}", self.in_crtc_hblank))));
        internal_vec.push((String::from("crtc_vblank:"), VideoCardStateEntry::String(format!("{}", self.in_crtc_vblank))));
        internal_vec.push((String::from("beam_x:"), VideoCardStateEntry::String(format!("{}", self.beam_x))));
        internal_vec.push((String::from("beam_y:"), VideoCardStateEntry::String(format!("{}", self.beam_y))));
        internal_vec.push((String::from("border:"), VideoCardStateEntry::String(format!("{}", self.hborder))));
        internal_vec.push((String::from("s_reads:"), VideoCardStateEntry::String(format!("{}", self.status_reads))));
        internal_vec.push((String::from("missed_hsyncs:"), VideoCardStateEntry::String(format!("{}", self.missed_hsyncs))));
        internal_vec.push((String::from("vsync_cycles:"), VideoCardStateEntry::String(format!("{}", self.cycles_per_vsync))));
        internal_vec.push((String::from("cur_screen_cycles:"), VideoCardStateEntry::String(format!("{}", self.cur_screen_cycles))));
        internal_vec.push((String::from("phase:"), VideoCardStateEntry::String(format!("{}", self.cycles & 0x0F))));
        internal_vec.push((String::from("cursor attr:"), VideoCardStateEntry::String(format!("{:02b}", self.cursor_attr))));
        internal_vec.push((String::from("snowflakes:"), VideoCardStateEntry::String(format!("{}", self.snow_count))));
        map.insert("Internal".to_string(), internal_vec);

        let mut external_vec = Vec::new();
        external_vec.push(("Mode Register".to_string(), VideoCardStateEntry::String(format!("{:08b}", self.mode_byte))));
        external_vec.push(("Hires Text".to_string(), VideoCardStateEntry::String(format!("{:?}", self.mode_hires_txt))));
        external_vec.push(("Graphics".to_string(), VideoCardStateEntry::String(format!("{:?}", self.mode_graphics))));
        external_vec.push(("BW".to_string(), VideoCardStateEntry::String(format!("{:?}", self.mode_bw))));
        external_vec.push(("Enable".to_string(), VideoCardStateEntry::String(format!("{:?}", self.mode_enable))));
        external_vec.push(("Hires Gfx".to_string(), VideoCardStateEntry::String(format!("{:?}", self.mode_hires_gfx))));
        external_vec.push(("Blinking".to_string(), VideoCardStateEntry::String(format!("{:?}", self.mode_blinking))));
        
        map.insert("External".to_string(), external_vec);
        
        map
    }

    fn run(&mut self, time: DeviceRunTimeUnit, _pic: &mut Option<Pic>, _cpumem: Option<&[u8]>) {
        /*
        if self.scanline > 1000 {
            log::error!("run(): scanlines way too high: {}", self.scanline);
        }
        */

        let mut hdots = if let DeviceRunTimeUnit::SystemTicks(ticks) = time {
            ticks
        } else {
            panic!("CGA requires SystemTicks time unit.")
        };

        if hdots == 0 {
            panic!("CGA run() with 0 ticks");
        }

        if self.ticks_advanced > hdots {
            panic!(
                "Invalid condition: ticks_advanced: {} > clocks: {}",
                self.ticks_advanced, hdots
            );
        }

        let orig_cycles = self.cycles;
        let orig_ticks_advanced = self.ticks_advanced;
        let orig_clocks_accum = self.clocks_accum;
        let orig_clocks_owed = self.pixel_clocks_owed;

        hdots -= self.ticks_advanced;
        self.clocks_accum += hdots;
        self.ticks_advanced = 0;

        if let ClockingMode::Character | ClockingMode::Dynamic = self.clock_mode {
            if (self.cycles + self.pixel_clocks_owed as u64) & self.char_clock_mask != 0 {
                log::error!(
                    "pixel_clocks_owed incorrect: does not put clock back in phase. \
                    cycles: {} owed: {} mask: {:X}",
                    self.cycles,
                    self.pixel_clocks_owed,
                    self.char_clock_mask
                );
            }
        }

        // Clock by pixel clock to catch up with character clock.
        let mut tick_count = 0;

        while self.pixel_clocks_owed > 0 {
            self.tick();
            tick_count += 1;
            self.pixel_clocks_owed -= 1;
            self.clocks_accum = self.clocks_accum.saturating_sub(1);

            if self.clocks_accum == 0 {
                //log::warn!("exhausted accumulator trying to catch up to lclock");

                self.slot_idx = 0;
                return;
            }
        }

        // We should be back in phase with character clock now.

        match self.clock_mode {
            ClockingMode::Character | ClockingMode::Dynamic => {
                if self.cycles & self.char_clock_mask as u64 != 0 {
                    log::warn!(
                        "out of phase with char clock: {} mask: {:02X} \
                        cycles: {} out of phase: {} \
                        cycles: {} advanced: {} owed: {} accum: {} tick_ct: {}",
                        self.char_clock,
                        self.char_clock_mask,
                        self.cycles,
                        self.cycles % self.char_clock as u64,
                        orig_cycles,
                        orig_ticks_advanced,
                        orig_clocks_owed,
                        orig_clocks_accum,
                        tick_count
                    );
                }

                // Drain accumulator and tick by character clock.
                while self.clocks_accum > self.char_clock {
                    if self.clocks_accum > 10000 {
                        log::error!("excessive clocks in accumulator: {}", self.clocks_accum);
                    }

                    /*
                    if self.debug_counter >= 3638297 {
                        log::error!("Break on me");
                    }
                    */

                    // Handle blinking. TODO: Move blink handling into tick().
                    self.blink_accum_clocks += self.char_clock;
                    if self.blink_accum_clocks > CGA_CURSOR_BLINK_RATE_CLOCKS {
                        self.blink_state = !self.blink_state;
                        self.blink_accum_clocks -= CGA_CURSOR_BLINK_RATE_CLOCKS;
                    }

                    // Char clock may update after tick_char() with deferred mode change, so save the
                    // current clock.
                    let old_char_clock = self.char_clock;

                    if self.clock_divisor == 2 {
                        self.tick_lchar();
                    } else {
                        self.tick_hchar();
                    }

                    /*
                    if self.debug_counter >= 3638298 {
                        log::error!("{} < {}", self.clocks_accum, self.char_clock);
                    }
                    self.debug_counter += 1;
                    */

                    self.clocks_accum = self.clocks_accum.saturating_sub(old_char_clock);
                }
            }
            ClockingMode::Cycle => {
                while self.clocks_accum > 0 {
                    // Handle blinking. TODO: Move blink handling into tick().
                    self.blink_accum_clocks += 1;
                    if self.blink_accum_clocks > CGA_CURSOR_BLINK_RATE_CLOCKS {
                        self.blink_state = !self.blink_state;
                        self.blink_accum_clocks -= CGA_CURSOR_BLINK_RATE_CLOCKS;
                    }

                    self.tick();
                    self.clocks_accum = self.clocks_accum.saturating_sub(1);
                }
            }
            _ => {
                panic!("Unsupported ClockingMode: {:?}", self.clock_mode);
            }
        }

        // Reset rwop slots for next CPU step.
        self.last_rw_tick = 0;
        self.slot_idx = 0;
    }

    fn reset(&mut self) {
        log::debug!("Resetting");
        self.reset_private();
    }

    fn get_pixel(&self, _x: u32, _y: u32) -> &[u8] {
        &DUMMY_PIXEL
    }

    fn get_pixel_raw(&self, _x: u32, _y: u32) -> u8 {
        0
    }

    fn get_plane_slice(&self, _plane: usize) -> &[u8] {
        &DUMMY_PLANE
    }

    fn get_frame_count(&self) -> u64 {
        self.frame_count
    }

    fn dump_mem(&self, path: &Path) {
        let mut filename = path.to_path_buf();
        filename.push("cga_mem.bin");

        match std::fs::write(filename.clone(), &*self.mem) {
            Ok(_) => {
                log::debug!("Wrote memory dump: {}", filename.display())
            }
            Err(e) => {
                log::error!("Failed to write memory dump '{}': {}", filename.display(), e)
            }
        }
    }

    fn write_trace_log(&mut self, msg: String) {
        self.trace_logger.print(msg);
    }

    fn trace_flush(&mut self) {
        self.trace_logger.flush();
    }

    fn get_text_mode_strings(&self) -> Vec<String> {
        let mut strings = Vec::new();

        let start_addr = self.crtc_start_address;
        let columns = self.crtc_horizontal_displayed as usize;
        let rows = self.crtc_vertical_displayed as usize;

        let mut row_addr = start_addr;

        for _ in 0..rows {
            let mut line = String::new();
            line.extend(
                self.mem[row_addr..(row_addr + (columns * 2) & 0x3fff)]
                    .iter()
                    .step_by(2)
                    .filter_map(|&byte| {
                        let ascii_byte = match byte {
                            0x00..=0x1F => 0x20,
                            0x80..=0xFF => 0x20,
                            _ => byte,
                        };
                        Some(ascii_byte as char)
                    }),
            );
            row_addr += columns * 2;
            strings.push(line);
        }

        strings
    }
}

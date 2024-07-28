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

    devices::mda::videocard.rs

    Implements the VideoCard trait for the IBM MDA card.

*/
use super::*;
use crate::{device_traits::videocard::*, devices::pic::Pic};

impl VideoCard for MDACard {
    fn get_sync(&self) -> (bool, bool, bool, bool) {
        (
            self.crtc.vblank(),
            self.crtc.hblank(),
            self.crtc.den(),
            self.crtc.border(),
        )
    }

    fn set_video_option(&mut self, opt: VideoOption) {
        match opt {
            VideoOption::EnableSnow(state) => {
                log::warn!("VideoOption::EnableSnow not supported for MDA.");
            }
            VideoOption::DebugDraw(state) => {
                log::debug!("VideoOption::DebugDraw set to: {}", state);
                self.debug_draw = state;
            }
        }
    }

    fn get_video_type(&self) -> VideoType {
        VideoType::MDA
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
        // MDA supports a single fixed 8x14 font. The size of the displayed window
        // is always HorizontalDisplayed * (VerticalDisplayed * (MaximumScanlineAddress + 1))
        // (Excepting fancy CRTC tricks that delay vsync)
        let width = self.crtc.reg[0] as u32 * MDA_CHAR_CLOCK as u32;
        let height = self.crtc.reg[6] as u32 * (self.crtc.reg[9] as u32 + 1);
        (width, height)
    }

    fn get_display_extents(&self) -> &DisplayExtents {
        &self.extents
    }

    fn list_display_apertures(&self) -> Vec<DisplayApertureDesc> {
        MDA_APERTURE_DESCS.to_vec()
    }

    fn get_display_apertures(&self) -> Vec<DisplayAperture> {
        self.extents.apertures.clone()
    }

    /// Get the position of the electron beam.
    fn get_beam_pos(&self) -> Option<(u32, u32)> {
        Some((self.beam_x, self.beam_y))
    }

    /// Tick the MDA the specified number of video clock cycles.
    fn debug_tick(&mut self, ticks: u32, cpumem: Option<&[u8]>) {
        match self.clock_mode {
            ClockingMode::Character | ClockingMode::Dynamic => {
                let pixel_ticks = ticks % MDA_CHAR_CLOCK as u32;
                let char_ticks = ticks / MDA_CHAR_CLOCK as u32;

                assert_eq!(ticks, pixel_ticks + (char_ticks * 9));

                for _ in 0..pixel_ticks {
                    self.tick();
                }
                for _ in 0..char_ticks {
                    self.tick_hchar();
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
        0
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
        return self.crtc.start_address();
    }

    fn get_cursor_info(&self) -> CursorInfo {
        let addr = self.get_cursor_address();

        match self.display_mode {
            DisplayMode::Mode2TextBw80 | DisplayMode::Mode3TextCo80 => CursorInfo {
                addr,
                pos_x: (addr % 80) as u32,
                pos_y: (addr / 80) as u32,
                line_start: self.crtc.cursor_extents().0,
                line_end: self.crtc.cursor_extents().1,
                visible: self.crtc.cursor_status(),
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
            w: MDA_CHAR_CLOCK as u32,
            h: CRTC_FONT_HEIGHT as u32,
            font_data: MDA_FONT,
        })
    }

    fn get_character_height(&self) -> u8 {
        self.crtc.reg[9] + 1
    }

    fn get_palette(&self) -> Option<Vec<[u8;4]>> {
        None
    }

    // /// Return the current palette number, intensity attribute bit, and alt color
    // fn get_cga_palette(&self) -> (CGAPalette, bool) {
    //     let intensity = self.cc_register & CC_BRIGHT_BIT != 0;
    //
    //     // Get background color
    //     let alt_color = match self.cc_register & 0x0F {
    //         0b0000 => CGAColor::Black,
    //         0b0001 => CGAColor::Blue,
    //         0b0010 => CGAColor::Green,
    //         0b0011 => CGAColor::Cyan,
    //         0b0100 => CGAColor::Red,
    //         0b0101 => CGAColor::Magenta,
    //         0b0110 => CGAColor::Brown,
    //         0b0111 => CGAColor::White,
    //         0b1000 => CGAColor::BlackBright,
    //         0b1001 => CGAColor::BlueBright,
    //         0b1010 => CGAColor::GreenBright,
    //         0b1011 => CGAColor::CyanBright,
    //         0b1100 => CGAColor::RedBright,
    //         0b1101 => CGAColor::MagentaBright,
    //         0b1110 => CGAColor::Yellow,
    //         _ => CGAColor::WhiteBright,
    //     };
    //
    //     // Are we in high res mode?
    //     if self.mode_hires_gfx {
    //         return (CGAPalette::Monochrome(alt_color), true);
    //     }
    //
    //     let mut palette = match self.cc_register & CC_PALETTE_BIT != 0 {
    //         true => CGAPalette::MagentaCyanWhite(alt_color),
    //         false => CGAPalette::RedGreenYellow(alt_color),
    //     };
    //
    //     // Check for 'hidden' palette - Black & White mode bit in lowres graphics selects Red/Cyan palette
    //     if self.mode_bw && self.mode_graphics && !self.mode_hires_gfx {
    //         palette = CGAPalette::RedCyanWhite(alt_color);
    //     }
    //
    //     (palette, intensity)
    // }

    #[rustfmt::skip]
    fn get_videocard_string_state(&self) -> HashMap<String, Vec<(String, VideoCardStateEntry)>> {
        let mut map = HashMap::new();

        let mut general_vec = Vec::new();

        general_vec.push(("Adapter Type:".to_string(), VideoCardStateEntry::String(format!("{:?} ({:?})", self.get_video_type(), self.subtype))));
        general_vec.push(("Display Mode:".to_string(), VideoCardStateEntry::String(format!("{:?}", self.get_display_mode()))));
        general_vec.push(("Video Enable:".to_string(), VideoCardStateEntry::String(format!("{:?}", self.mode_enable))));
        general_vec.push(("Clock Divisor:".to_string(), VideoCardStateEntry::String(format!("{}", self.clock_divisor))));
        general_vec.push(("Frame Count:".to_string(), VideoCardStateEntry::String(format!("{}", self.frame_count))));
        map.insert("General".to_string(), general_vec);

        let crtc_vec = self.crtc.get_reg_state();
        map.insert("CRTC".to_string(), crtc_vec);

        let crtc_counter_vec = self.crtc.get_counter_state();
        let mut internal_vec = Vec::new();

        internal_vec.extend(crtc_counter_vec);
        
        //internal_vec.push(("hcc_c0:".to_string(), VideoCardStateEntry::String(format!("{}", self.hcc_c0))));
        //internal_vec.push((format!("vlc_c9:"), VideoCardStateEntry::String(format!("{}", self.vlc_c9))));
        //internal_vec.push((format!("vcc_c4:"), VideoCardStateEntry::String(format!("{}", self.vcc_c4))));
        internal_vec.push(("scanline:".to_string(), VideoCardStateEntry::String(format!("{}", self.scanline))));
        //internal_vec.push((format!("vsc_c3h:"), VideoCardStateEntry::String(format!("{}", self.vsc_c3h))));
        //internal_vec.push((format!("hsc_c3l:"), VideoCardStateEntry::String(format!("{}", self.hsc_c3l))));
        //internal_vec.push((format!("vtac_c5:"), VideoCardStateEntry::String(format!("{}", self.vtac_c5))));
        internal_vec.push(("vma:".to_string(), VideoCardStateEntry::String(format!("{:04X}", self.vma))));
        //internal_vec.push((format!("vma':"), VideoCardStateEntry::String(format!("{:04X}", self.vma_t))));
        internal_vec.push(("vmws:".to_string(), VideoCardStateEntry::String(format!("{}", self.vmws))));
        internal_vec.push(("rba:".to_string(), VideoCardStateEntry::String(format!("{:04X}", self.rba))));
        internal_vec.push(("de:".to_string(), VideoCardStateEntry::String(format!("{}", self.crtc.den()))));
        internal_vec.push(("crtc_hblank:".to_string(), VideoCardStateEntry::String(format!("{}", self.crtc.hblank()))));
        internal_vec.push(("crtc_vblank:".to_string(), VideoCardStateEntry::String(format!("{}", self.crtc.vblank()))));
        internal_vec.push(("beam_x:".to_string(), VideoCardStateEntry::String(format!("{}", self.beam_x))));
        internal_vec.push(("beam_y:".to_string(), VideoCardStateEntry::String(format!("{}", self.beam_y))));
        internal_vec.push(("border:".to_string(), VideoCardStateEntry::String(format!("{}", self.crtc.border()))));
        internal_vec.push(("s_reads:".to_string(), VideoCardStateEntry::String(format!("{}", self.status_reads))));
        internal_vec.push(("missed_hsyncs:".to_string(), VideoCardStateEntry::String(format!("{}", self.missed_hsyncs))));
        internal_vec.push(("vsync_cycles:".to_string(), VideoCardStateEntry::String(format!("{}", self.cycles_per_vsync))));
        internal_vec.push(("cur_screen_cycles:".to_string(), VideoCardStateEntry::String(format!("{}", self.cur_screen_cycles))));
        internal_vec.push(("phase:".to_string(), VideoCardStateEntry::String(format!("{}", self.cycles & 0x0F))));
        internal_vec.push(("cursor attr:".to_string(), VideoCardStateEntry::String(format!("{:02b}", self.cursor_attr))));
        
        if let VideoCardSubType::Hercules = self.subtype {
            internal_vec.push(("HGC Display Page:".to_string(), VideoCardStateEntry::String(format!("{:?}", self.mode.page_select()))));
            internal_vec.push(("HGC Page Flips:".to_string(), VideoCardStateEntry::String(format!("{}", self.hgc_page_flips))));
        }
        
        map.insert("Internal".to_string(), internal_vec);

        map
    }

    fn run(&mut self, time: DeviceRunTimeUnit, _pic: &mut Option<Pic>, _cpumem: Option<&[u8]>) {
        /*
        if self.scanline > 1000 {
            log::error!("run(): scanlines way too high: {}", self.scanline);
        }
        */

        let ticks = if let DeviceRunTimeUnit::Microseconds(us) = time {
            us * MDA_CLOCK
        }
        else {
            panic!("MDA requires Microseconds time unit.");
        };

        self.do_ticks(ticks);

        // if self.ticks_advanced > hdots {
        //     panic!(
        //         "Invalid condition: ticks_advanced: {} > clocks: {}",
        //         self.ticks_advanced, hdots
        //     );
        // }

        // let orig_cycles = self.cycles;
        // let orig_ticks_advanced = self.ticks_advanced;
        // let orig_clocks_accum = self.clocks_accum;
        // let orig_clocks_owed = self.pixel_clocks_owed;
        //
        // hdots -= self.ticks_advanced;
        // self.clocks_accum += hdots;
        // self.ticks_advanced = 0.0;
        //
        // if let ClockingMode::Character | ClockingMode::Dynamic = self.clock_mode {
        //     if (self.cycles + self.pixel_clocks_owed as u64) % MDA_CHAR_CLOCK as u64 != 0 {
        //         log::error!(
        //             "pixel_clocks_owed incorrect: does not put clock back in phase. \
        //             cycles: {} owed: {}",
        //             self.cycles,
        //             self.pixel_clocks_owed,
        //         );
        //     }
        // }
        //
        // // Clock by pixel clock to catch up with character clock.
        // let mut tick_count = 0;
        //
        // while self.pixel_clocks_owed > 0 {
        //     self.tick();
        //     tick_count += 1;
        //     self.pixel_clocks_owed -= 1;
        //     self.clocks_accum = self.clocks_accum.saturating_sub(1);
        //
        //     if self.clocks_accum == 0 {
        //         //log::warn!("exhausted accumulator trying to catch up to lclock");
        //
        //         self.slot_idx = 0;
        //         return;
        //     }
        // }
        //
        // // We should be back in phase with character clock now.
        //
        // match self.clock_mode {
        //     ClockingMode::Character | ClockingMode::Dynamic => {
        //         if self.cycles % MDA_CHAR_CLOCK as u64 != 0 {
        //             log::warn!(
        //                 "out of phase with char clock: {}\
        //                 cycles: {} out of phase: {} \
        //                 cycles: {} advanced: {} owed: {} accum: {} tick_ct: {}",
        //                 self.char_clock,
        //                 self.cycles,
        //                 self.cycles % self.char_clock as u64,
        //                 orig_cycles,
        //                 orig_ticks_advanced,
        //                 orig_clocks_owed,
        //                 orig_clocks_accum,
        //                 tick_count
        //             );
        //         }
        //
        //         // Drain accumulator and tick by character clock.
        //         while self.clocks_accum > self.char_clock {
        //             if self.clocks_accum > 10000 {
        //                 log::error!("excessive clocks in accumulator: {}", self.clocks_accum);
        //             }
        //
        //             // Char clock may update after tick_char() with deferred mode change, so save the
        //             // current clock.
        //             let old_char_clock = self.char_clock;
        //             self.tick_hchar();
        //
        //             /*
        //             if self.debug_counter >= 3638298 {
        //                 log::error!("{} < {}", self.clocks_accum, self.char_clock);
        //             }
        //             self.debug_counter += 1;
        //             */
        //
        //             self.clocks_accum = self.clocks_accum.saturating_sub(old_char_clock);
        //         }
        //     }
        //     ClockingMode::Cycle => {
        //         while self.clocks_accum > 0 {
        //             self.tick();
        //             self.clocks_accum = self.clocks_accum.saturating_sub(1);
        //         }
        //     }
        //     _ => {
        //         panic!("Unsupported ClockingMode: {:?}", self.clock_mode);
        //     }
        // }
        //
        // // Reset rwop slots for next CPU step.
        // self.last_rw_tick = 0;
        // self.slot_idx = 0;
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
        filename.push("mda_mem.bin");

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
        let start_addr = self.crtc.start_address();
        let columns = self.crtc.reg[1] as usize;
        let rows = self.crtc.reg[6] as usize;
        let mut row_addr = start_addr as usize;

        for _ in 0..rows {
            let mut line = String::new();
            line.extend(
                self.mem[row_addr..(row_addr + (columns * 2) & 0x1fff)]
                    .iter()
                    .step_by(2)
                    .filter_map(|&byte| {
                        let ascii_byte = match byte {
                            0x00..=0x1F => 0x20,
                            0x80..=0xFF => 0x20,
                            _ => byte,
                        };
                        Some(ascii_byte as u8 as char)
                    }),
            );
            row_addr += columns * 2;
            strings.push(line);
        }

        strings
    }
}

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

    ega::videocard.rs

    Implement the VideoCard trait for EGA

*/

use super::*;
use crate::bus::DeviceRunTimeUnit;
use std::{collections::HashMap, path::Path};

// Helper macro for pushing video card state entries.
macro_rules! push_reg_str {
    ($vec: expr, $reg: expr, $decorator: expr, $val: expr ) => {
        $vec.push((
            format!("{} {:?}", $decorator, $reg),
            VideoCardStateEntry::String(format!("{}", $val)),
        ))
    };
}

impl VideoCard for EGACard {
    fn get_sync(&self) -> (bool, bool, bool, bool) {
        (false, false, false, false)
    }

    fn set_video_option(&mut self, opt: VideoOption) {
        match opt {
            VideoOption::EnableSnow(state) => {
                log::warn!("VideoOption::EnableSnow not supported for EGA");
            }
            VideoOption::DebugDraw(state) => {
                log::debug!("VideoOption::DebugDraw set to: {}", state);
                self.debug_draw = state;
            }
        }
    }

    fn get_video_type(&self) -> VideoType {
        VideoType::EGA
    }

    fn get_render_mode(&self) -> RenderMode {
        RenderMode::Direct
    }

    fn get_render_depth(&self) -> RenderBpp {
        RenderBpp::Six
    }

    fn get_display_mode(&self) -> DisplayMode {
        self.display_mode
    }

    fn set_clocking_mode(&mut self, _mode: ClockingMode) {
        // not implemented
    }

    fn get_display_size(&self) -> (u32, u32) {
        /*        // EGA supports multiple fonts.

        let font_w = EGA_FONTS[self.current_font].w;
        let _font_h = EGA_FONTS[self.current_font].h;

        // Clock divisor effectively doubles the CRTC register values
        let _clock_divisor = match self.sequencer.clocking_mode.dot_clock() {
            DotClock::Native => 1,
            DotClock::HalfClock => 2,
        };

        //let width = (self.crtc_horizontal_display_end as u32 + 1) * clock_divisor * font_w as u32;
        let width = (self.crtc_horizontal_display_end as u32 + 1) * font_w as u32;
        let height = self.crtc_vertical_display_end as u32 + 1;
        (width, height)*/
        (320, 200)
    }

    /// Return the 16-bit value computed from the CRTC's pair of Page Address registers.
    fn get_start_address(&self) -> u16 {
        return self.crtc.start_address();
    }

    /// Unimplemented for indirect rendering.
    fn get_display_extents(&self) -> &DisplayExtents {
        &self.extents
    }

    /// Unimplemented for indirect rendering.
    fn get_beam_pos(&self) -> Option<(u32, u32)> {
        Some((self.raster_x, self.raster_y))
    }

    /// Unimplemented
    fn debug_tick(&mut self, _ticks: u32) {}

    /// Get the current scanline being rendered.
    fn get_scanline(&self) -> u32 {
        0
    }

    /// Return whether to double scanlines produced by this adapter.
    /// For EGA, this is false in 16Mhz modes and true in 14Mhz modes
    fn get_scanline_double(&self) -> bool {
        self.extents.double_scan
    }

    /// Return the u8 slice representing the requested buffer type.
    fn get_buf(&self, buf_select: BufferSelect) -> &[u8] {
        match buf_select {
            BufferSelect::Back => &self.buf[self.back_buf][..],
            BufferSelect::Front => &self.buf[self.front_buf][..],
        }
    }

    fn get_display_buf(&self) -> &[u8] {
        &self.buf[self.front_buf][..]
    }

    fn list_display_apertures(&self) -> Vec<DisplayApertureDesc> {
        EGA_APERTURE_DESCS.to_vec()
    }

    fn get_display_apertures(&self) -> Vec<DisplayAperture> {
        self.extents.apertures.clone()
    }

    fn get_overscan_color(&self) -> u8 {
        0
    }

    /// Return the current refresh rate.
    /// TODO: Handle VGA 70Hz modes.
    fn get_refresh_rate(&self) -> u32 {
        60
    }

    fn get_clock_divisor(&self) -> u32 {
        match self.sequencer.clocking_mode.dot_clock() {
            DotClock::Native => 1,
            DotClock::HalfClock => 2,
        }
    }

    fn is_40_columns(&self) -> bool {
        match self.display_mode {
            DisplayMode::Mode0TextBw40 => true,
            DisplayMode::Mode1TextCo40 => true,
            DisplayMode::Mode4LowResGraphics => true,
            DisplayMode::Mode5LowResAltPalette => true,
            _ => false,
        }
    }

    fn is_graphics_mode(&self) -> bool {
        self.mode_graphics
    }

    fn get_cursor_info(&self) -> CursorInfo {
        let addr = self.get_cursor_address();

        let span = self.crtc.get_cursor_span();
        match self.display_mode {
            DisplayMode::Mode0TextBw40 | DisplayMode::Mode1TextCo40 => CursorInfo {
                addr: addr as usize,
                pos_x: addr % 40,
                pos_y: addr / 40,
                line_start: span.0,
                line_end: span.1,
                visible: self.get_cursor_status(),
            },
            DisplayMode::Mode2TextBw80 | DisplayMode::Mode3TextCo80 => CursorInfo {
                addr: addr as usize,
                pos_x: addr % 80,
                pos_y: addr / 80,
                line_start: span.0,
                line_end: span.1,
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

    fn get_current_font(&self) -> FontInfo {
        let w = EGA_FONTS[self.current_font as usize].w;
        let h = EGA_FONTS[self.current_font as usize].h;
        let data = EGA_FONTS[self.current_font as usize].data;

        FontInfo { w, h, font_data: data }
    }

    fn get_character_height(&self) -> u8 {
        self.crtc.maximum_scanline() + 1
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
            _ => CGAColor::WhiteBright,
        };

        // Are we in high res mode?
        if self.mode_hires_gfx {
            return (CGAPalette::Monochrome(alt_color), true);
        }

        let mut palette = match self.cc_register & CC_PALETTE_BIT != 0 {
            true => CGAPalette::MagentaCyanWhite(alt_color),
            false => CGAPalette::RedGreenYellow(alt_color),
        };

        // Check for 'hidden' palette - Black & White mode bit in lowres graphics selects Red/Cyan palette
        if self.mode_bw && self.mode_graphics && !self.mode_hires_gfx {
            palette = CGAPalette::RedCyanWhite(alt_color);
        }

        (palette, intensity)
    }

    #[rustfmt::skip]
    #[allow(dead_code)]
    /// Returns a string representation of all the CRTC Registers.
    fn get_videocard_string_state(&self) -> HashMap<String, Vec<(String, VideoCardStateEntry)>> {
        let mut map = HashMap::new();

        let mut general_vec = Vec::new();
        general_vec.push(("Adapter Type:".to_string(), VideoCardStateEntry::String(format!("{:?}", self.get_video_type()))));
        general_vec.push(("Display Mode:".to_string(), VideoCardStateEntry::String(format!("{:?}", self.get_display_mode()))));
        general_vec.push(("Pixel Clock:".to_string(), VideoCardStateEntry::String(format!("{:?}", self.misc_output_register.clock_select()))));
        general_vec.push(("Clock Divisor:".to_string(), VideoCardStateEntry::String(format!("{:?}", self.sequencer.clock_divisor))));
        general_vec.push((
            "Field:".to_string(),
            VideoCardStateEntry::String(format!("{}x{}", self.extents.field_w, self.extents.field_h)),
        ));

        map.insert("General".to_string(), general_vec);
        map.insert("CRTC".to_string(), self.crtc.get_state());

        let mut external_vec = Vec::new();
        external_vec.push(("Misc Output".to_string(), VideoCardStateEntry::String(format!("{:?}", self.misc_output_register.into_bytes()[0]))));
        external_vec.push(("Misc Output [ios]".to_string(), VideoCardStateEntry::String(format!("{:?}", self.misc_output_register.io_address_select()))));
        external_vec.push(("Misc Output [er]".to_string(), VideoCardStateEntry::String(format!("{:?}", self.misc_output_register.enable_ram()))));
        external_vec.push(("Misc Output [cs]".to_string(), VideoCardStateEntry::String(format!("{:?}", self.misc_output_register.clock_select()))));
        external_vec.push(("Misc Output [div]".to_string(), VideoCardStateEntry::String(format!("{:?}", self.misc_output_register.disable_internal_drivers()))));
        external_vec.push(("Misc Output [pb]".to_string(), VideoCardStateEntry::String(format!("{:?}", self.misc_output_register.oddeven_page_select()))));
        external_vec.push(("Misc Output [hrp]".to_string(), VideoCardStateEntry::String(format!("{:?}", self.misc_output_register.horizontal_retrace_polarity()))));
        external_vec.push(("Misc Output [vrp]".to_string(), VideoCardStateEntry::String(format!("{:?}", self.misc_output_register.vertical_retrace_polarity()))));
        map.insert("External".to_string(), external_vec);

        map.insert("Sequencer".to_string(), self.sequencer.get_state());
        map.insert("Graphics".to_string(), self.gc.get_state());

        let mut attribute_pal_vec = Vec::new();
        for i in 0..16 {
            // Attribute palette entries are interpreted differently depending on the current clock speed
            // Low resolution modes use 4BPP palette entries, high resolution modes use 6bpp.
            let pal_resolved = match self.misc_output_register.clock_select() {
                ClockSelect::Clock14 => self.ac.palette_registers[i].four_to_six,
                _ => self.ac.palette_registers[i].six,
            };

            let (r, g, b) = EGACard::ega_to_rgb(pal_resolved);
            attribute_pal_vec.push((
                format!("{}", i),
                VideoCardStateEntry::Color(format!("{:06b}", self.ac.palette_registers[i].six), r, g, b),
            ));
        }
        map.insert("AttributePalette".to_string(), attribute_pal_vec);
        map.insert("Attribute".to_string(), self.ac.get_state());
        map.insert("CRTC Counters".to_string(), self.crtc.get_counter_state());

        let mut internal_vec = Vec::new();
        internal_vec.push(("scanline:".to_string(), VideoCardStateEntry::String(format!("{}", self.scanline))));
        internal_vec.push(("vma:".to_string(), VideoCardStateEntry::String(format!("{:04X}", self.vma))));
        internal_vec.push(("rba:".to_string(), VideoCardStateEntry::String(format!("{:04X}", self.rba))));
        internal_vec.push(("raster_x:".to_string(), VideoCardStateEntry::String(format!("{}", self.raster_x))));
        internal_vec.push(("raster_y:".to_string(), VideoCardStateEntry::String(format!("{}", self.raster_y))));

        //internal_vec.push((format!("s_reads:"), VideoCardStateEntry::String(format!("{}", self.status_reads))));
        //internal_vec.push((format!("missed_hsyncs:"), VideoCardStateEntry::String(format!("{}", self.missed_hsyncs))));
        //internal_vec.push((format!("vsync_cycles:"), VideoCardStateEntry::String(format!("{}", self.cycles_per_vsync))));
        //internal_vec.push((format!("cur_screen_cycles:"), VideoCardStateEntry::String(format!("{}", self.cur_screen_cycles))));
        //internal_vec.push((format!("phase:"), VideoCardStateEntry::String(format!("{}", self.cycles & 0x0F))));

        internal_vec.push(("blink state:".to_string(), VideoCardStateEntry::String(format!("{}", self.blink_state))));
        internal_vec.push(("hsync_ct:".to_string(), VideoCardStateEntry::String(format!("{}", self.hsync_ct))));
        internal_vec.push(("vsync_ct:".to_string(), VideoCardStateEntry::String(format!("{}", self.vsync_ct))));

        map.insert("Internal".to_string(), internal_vec);

        map
    }

    fn run(&mut self, time: DeviceRunTimeUnit) {
        if let DeviceRunTimeUnit::Microseconds(us) = time {
            // Select the appropriate timings based on the current clocking mode
            let ticks = match self.misc_output_register.clock_select() {
                ClockSelect::Clock14 => us * EGA_CLOCK0,
                ClockSelect::Clock16 => us * EGA_CLOCK1,
                _ => 0.0,
            };

            self.tick(ticks)
        }
    }

    /*
    fn run(&mut self, cpu_cycles: u32) {

        self.frame_cycles += cpu_cycles as f32;
        self.scanline_cycles += cpu_cycles as f32;

        // Select the appropriate timings based on the current clocking mode
        let ti = match self.misc_output_register.clock_select() {
            ClockSelect::Clock14 => 0,
            ClockSelect::Clock16 => 1,
            _ => 0
        };

        if self.frame_cycles > self.timings[ti].cpu_frame as f32 {
            self.frame_cycles -= self.timings[ti].cpu_frame as f32;
            self.cursor_frames += 1;
            // Blink the cursor
            let cursor_cycle = CGA_DEFAULT_CURSOR_FRAME_CYCLE * (self.cursor_slowblink as u32 + 1);
            if self.cursor_frames > cursor_cycle {
                self.cursor_frames -= cursor_cycle;
                self.cursor_status = !self.cursor_status;
            }
        }

        // CyclesPerFrame / VerticalTotal = CyclesPerScanline
        let cpu_scanline = self.timings[ti].cpu_frame as f32 / (self.crtc_vertical_total + 1 ) as f32;

        while self.scanline_cycles > cpu_scanline {
            self.scanline_cycles -= cpu_scanline ;
            if !self.in_vblank {
                self.scanline += 1;
            }
        }

        let hblank_start;
        let vblank_start;

        // Are we in HBLANK interval?
        if self.crtc_start_horizontal_retrace > 0 && self.crtc_horizontal_total > 0 {
            hblank_start = ((self.crtc_start_horizontal_retrace as f32 * 8.0) / (self.crtc_horizontal_total as f32 * 8.0) * cpu_scanline as f32) as u32;
            self.in_hblank = self.scanline_cycles > hblank_start as f32;
        }
        // Are we in VBLANK interval?
        if self.crtc_start_vertical_blank > 0 && self.crtc_vertical_total > 0 {
            vblank_start = ((self.crtc_start_vertical_blank as f32 / (self.crtc_vertical_total + 1) as f32) * self.timings[ti].cpu_frame as f32) as u32;
            self.in_vblank = self.frame_cycles > vblank_start as f32;

            if self.in_vblank {
                self.scanline = 0;
            }
        }
        //self.in_hblank = self.scanline_cycles > self.timings[ti].hblank_start;
        // Are we in VBLANK interval?
        //self.in_vblank = self.frame_cycles > self.timings[ti].vblank_start;
    }
    */

    fn reset(&mut self) {
        self.reset_private();
    }

    fn get_pixel(&self, _x: u32, _y: u32) -> &[u8] {
        &DUMMY_PIXEL
    }

    fn get_pixel_raw(&self, x: u32, y: u32) -> u8 {
        /*        let mut byte = 0;

        let x_byte_offset = (x + self.attribute_pel_panning as u32) / 8;
        let x_bit_offset = (x + self.attribute_pel_panning as u32) % 8;

        // Get the current width of screen + offset
        // let span = (self.crtc_horizontal_display_end + 1 + 64) as u32;
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
            read_offset = (y_offset + x_byte_offset + self.crtc_start_address as u32) as usize;
        }

        if read_offset < self.sequencer.vram.plane_len() {
            for i in 0..4 {
                let read_byte = self.sequencer.vram.read_u8(i, read_offset);
                let read_bit = match read_byte & (0x01 << (7 - x_bit_offset)) != 0 {
                    true => 1,
                    false => 0,
                };

                //byte |= read_bit << (3 - i);
                byte |= read_bit << i;
            }
            // return self.attribute_palette_registers[byte & 0x0F].into_bytes()[0];
            return self.attribute_palette_registers[byte & 0x0F].six;
        }*/
        0
    }

    fn get_plane_slice(&self, plane: usize) -> &[u8] {
        self.sequencer.vram.plane_slice(plane)
    }

    fn dump_mem(&self, path: &Path) {
        for i in 0..4 {
            let mut filename = path.to_path_buf();
            filename.push(format!("ega_plane{}.bin", i));

            match std::fs::write(filename.clone(), &self.get_plane_slice(i)) {
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
        self.frame
    }

    fn write_trace_log(&mut self, _msg: String) {
        //self.trace_logger.print(msg);
    }

    fn trace_flush(&mut self) {
        //self.trace_logger.print(msg);
    }

    fn get_text_mode_strings(&self) -> Vec<String> {
        Vec::new()
    }
}

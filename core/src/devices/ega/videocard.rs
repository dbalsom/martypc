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

    ega::videocard.rs

    Implement the VideoCard trait for EGA

*/

use crate::devices::ega::EGACard;
use crate::bus::DeviceRunTimeUnit;

use crate::videocard::*;
use crate::devices::ega::*;
use crate::devices::ega::attribute_regs::*;
use crate::devices::ega::crtc_regs::*;
use crate::devices::ega::graphics_regs::*;
use crate::devices::ega::sequencer_regs::*;

// Helper macro for pushing video card state entries. 
macro_rules! push_reg_str {
    ($vec: expr, $reg: expr, $decorator: expr, $val: expr ) => {
        $vec.push((format!("{} {:?}", $decorator, $reg ), VideoCardStateEntry::String(format!("{}", $val))))
    };
}

impl VideoCard for EGACard {

    fn get_sync(&self) -> (bool, bool, bool, bool) {
        (false, false, false, false)
    }

    fn set_video_option(&mut self, _opt: VideoOption) {
        // No options implemented
    }

    fn get_video_type(&self) -> VideoType {
        VideoType::EGA
    }

    fn get_render_mode(&self) -> RenderMode {
        RenderMode::Direct
    }

    fn get_display_mode(&self) -> DisplayMode {
        self.display_mode
    }

    fn set_clocking_mode(&mut self, _mode: ClockingMode) {
        // not implemented
    }

    fn get_display_size(&self) -> (u32, u32) {

        // EGA supports multiple fonts.

        let font_w = EGA_FONTS[self.current_font].w;
        let _font_h = EGA_FONTS[self.current_font].h;

        // Clock divisor effectively doubles the CRTC register values
        let _clock_divisor = match self.sequencer_clocking_mode.dot_clock() {
            DotClock::Native => 1,
            DotClock::HalfClock => 2
        };

        //let width = (self.crtc_horizontal_display_end as u32 + 1) * clock_divisor * font_w as u32;
        let width = (self.crtc_horizontal_display_end as u32 + 1) * font_w as u32;
        let height = self.crtc_vertical_display_end as u32 + 1;
        (width, height)
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
    fn debug_tick(&mut self, _ticks: u32) {
    }

    /// Get the current scanline being rendered.
    fn get_scanline(&self) -> u32 {
        0
    }

    /// Return whether to double scanlines produced by this adapter.
    /// For EGA, this is false.
    fn get_scanline_double(&self) -> bool {
        false
    }

    fn get_display_buf(&self) -> &[u8] {
        &self.buf[self.front_buf][..]
    }

    fn get_back_buf(&self) -> &[u8] {
        &self.buf[self.back_buf][..]
    }      
    
    fn get_display_aperture(&self) -> (u32, u32) {
        (self.extents.aperture.w, self.extents.aperture.h)
    }

    fn list_display_apertures(&self) -> (Vec<DisplayApertureDesc>, usize) {
        (EGA_APERTURE_DESCS.to_vec(), 0)
    }

    fn set_aperture(&mut self, aperture: u32) {
        let new_aperture = aperture as usize;
        if new_aperture < EGA_APERTURE_DESCS.len() {
            self.aperture = new_aperture;
        }

        log::debug!("Setting aperture to {}", EGA_APERTURE_DESCS[new_aperture].name);
        self.extents.aperture = EGA_APERTURES[(self.misc_output_register.clock_select() as usize) & 0x01][new_aperture];
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
        match self.sequencer_clocking_mode.dot_clock() {
            DotClock::Native => 1,
            DotClock::HalfClock => 2
        }
    }

    fn is_40_columns(&self) -> bool {
        match self.display_mode {
            DisplayMode::Mode0TextBw40 => true,
            DisplayMode::Mode1TextCo40 => true,
            DisplayMode::Mode4LowResGraphics => true,
            DisplayMode::Mode5LowResAltPalette => true,
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
                    addr: addr as usize,
                    pos_x: addr % 40,
                    pos_y: addr / 40,
                    line_start: self.crtc_cursor_start,
                    line_end: self.crtc_cursor_end,
                    visible: self.get_cursor_status()
                }
            }
            DisplayMode::Mode2TextBw80 | DisplayMode::Mode3TextCo80 => {
                CursorInfo{
                    addr: addr as usize,
                    pos_x: addr % 80,
                    pos_y: addr / 80,
                    line_start: self.crtc_cursor_start,
                    line_end: self.crtc_cursor_end,
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

    fn get_current_font(&self) -> FontInfo {

        let w = EGA_FONTS[self.current_font].w;
        let h = EGA_FONTS[self.current_font].h;
        let data = EGA_FONTS[self.current_font].data;

        FontInfo {
            w,
            h,
            font_data: data
        }
    }

    fn get_character_height(&self) -> u8 {
        self.crtc_maximum_scanline + 1
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

    #[allow (dead_code)]
    /// Returns a string representation of all the CRTC Registers.
    fn get_videocard_string_state(&self) -> HashMap<String, Vec<(String, VideoCardStateEntry)>> {

        let mut map = HashMap::new();
        
        let mut general_vec = Vec::new();
        general_vec.push(("Adapter Type:".to_string(), VideoCardStateEntry::String(format!("{:?}", self.get_video_type()))));
        general_vec.push(("Display Mode:".to_string(), VideoCardStateEntry::String(format!("{:?}", self.get_display_mode()))));
        general_vec.push(("Pixel Clock:".to_string(), VideoCardStateEntry::String(format!("{:?}", self.misc_output_register.clock_select()))));
        general_vec.push(("Clock Divisor:".to_string(), VideoCardStateEntry::String(format!("{:?}", self.clock_divisor))));
        general_vec.push((
            "Field:".to_string(), 
            VideoCardStateEntry::String(
                format!(
                    "{}x{}", 
                    self.extents.field_w,
                    self.extents.field_h
                )
            )
        ));
        general_vec.push((
            "Aperture:".to_string(), 
            VideoCardStateEntry::String(
                format!(
                    "{}x{}", 
                    self.extents.aperture.w,
                    self.extents.aperture.h
                )
            )
        ));
        map.insert("General".to_string(), general_vec);

        let mut crtc_vec = Vec::new();
        push_reg_str!(crtc_vec, CRTCRegister::HorizontalTotal, "[R00]", self.crtc_horizontal_total);
        push_reg_str!(crtc_vec, CRTCRegister::HorizontalDisplayEnd, "[R01]", self.crtc_horizontal_display_end);
        push_reg_str!(crtc_vec, CRTCRegister::StartHorizontalBlank, "[R02]", self.crtc_start_horizontal_blank);
        push_reg_str!(crtc_vec, CRTCRegister::EndHorizontalBlank, "[R03]", self.crtc_end_horizontal_blank.end_horizontal_blank());
        push_reg_str!(crtc_vec, CRTCRegister::EndHorizontalBlank, "[R03:des]", self.crtc_end_horizontal_blank.display_enable_skew());
        push_reg_str!(crtc_vec, CRTCRegister::EndHorizontalBlank, "[R03:norm]", self.crtc_end_horizontal_blank_norm);
        push_reg_str!(crtc_vec, CRTCRegister::StartHorizontalRetrace, "[R04]", self.crtc_start_horizontal_retrace);
        push_reg_str!(crtc_vec, CRTCRegister::EndHorizontalRetrace, "[R05]", self.crtc_end_horizontal_retrace.end_horizontal_retrace());
        push_reg_str!(crtc_vec, CRTCRegister::EndHorizontalRetrace, "[R05:hrd]", self.crtc_end_horizontal_retrace.horizontal_retrace_delay());
        push_reg_str!(crtc_vec, CRTCRegister::EndHorizontalRetrace, "[R05:norm]", self.crtc_end_horizontal_retrace_norm);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalTotal, "[R06]", self.crtc_vertical_total);
        push_reg_str!(crtc_vec, CRTCRegister::Overflow, "[R07]", self.crtc_overflow);
        push_reg_str!(crtc_vec, CRTCRegister::PresetRowScan, "[R08]", self.crtc_preset_row_scan);
        push_reg_str!(crtc_vec, CRTCRegister::MaximumScanLine, "[R09]", self.crtc_maximum_scanline);
        push_reg_str!(crtc_vec, CRTCRegister::CursorStartLine, "[R0A]", self.crtc_cursor_start);
        push_reg_str!(crtc_vec, CRTCRegister::CursorEndLine, "[R0B]", self.crtc_cursor_end);
        push_reg_str!(crtc_vec, CRTCRegister::StartAddressH, "[R0C]", self.crtc_start_address_ho);
        push_reg_str!(crtc_vec, CRTCRegister::StartAddressL, "[R0D]", self.crtc_start_address_lo);
        push_reg_str!(crtc_vec, CRTCRegister::CursorAddressH, "[R0E]", self.crtc_cursor_address_ho);
        push_reg_str!(crtc_vec, CRTCRegister::CursorAddressL, "[R0F]", self.crtc_cursor_address_lo);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalRetraceStart, "[R10]", self.crtc_vertical_retrace_start);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalRetraceEnd, "[R11]", self.crtc_vertical_retrace_end.vertical_retrace_end());
        push_reg_str!(crtc_vec, CRTCRegister::VerticalRetraceEnd, "[R11:norm]", self.crtc_vertical_retrace_end_norm);
        push_reg_str!(crtc_vec, CRTCRegister::VerticalDisplayEnd, "[R12]", self.crtc_vertical_display_end);
        push_reg_str!(crtc_vec, CRTCRegister::Offset, "[R13]", self.crtc_offset);
        push_reg_str!(crtc_vec, CRTCRegister::UnderlineLocation, "[R14]", self.crtc_underline_location);
        push_reg_str!(crtc_vec, CRTCRegister::StartVerticalBlank, "[R15]", self.crtc_start_vertical_blank);
        push_reg_str!(crtc_vec, CRTCRegister::EndVerticalBlank, "[R16]", self.crtc_end_vertical_blank);
        push_reg_str!(crtc_vec, CRTCRegister::ModeControl, "[R17]", self.crtc_mode_control);
        push_reg_str!(crtc_vec, CRTCRegister::LineCompare, "[R18]", self.crtc_line_compare);
        map.insert("CRTC".to_string(), crtc_vec);

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

        
        let mut sequencer_vec = Vec::new();
        
        sequencer_vec.push((format!("{:?}", SequencerRegister::Reset), VideoCardStateEntry::String(format!("{:02b}", self.sequencer_reset))));
        sequencer_vec.push((format!("{:?}", SequencerRegister::ClockingMode), VideoCardStateEntry::String(format!("{:08b}", self.sequencer_clocking_mode.into_bytes()[0]))));           
        sequencer_vec.push((format!("{:?} [cc]", SequencerRegister::ClockingMode), VideoCardStateEntry::String(format!("{:?}", self.sequencer_clocking_mode.character_clock()))));
        sequencer_vec.push((format!("{:?} [bw]", SequencerRegister::ClockingMode), VideoCardStateEntry::String(format!("{}", self.sequencer_clocking_mode.bandwidth()))));
        sequencer_vec.push((format!("{:?} [sl]", SequencerRegister::ClockingMode), VideoCardStateEntry::String(format!("{}", self.sequencer_clocking_mode.shift_load()))));
        sequencer_vec.push((format!("{:?} [dc]", SequencerRegister::ClockingMode), VideoCardStateEntry::String(format!("{:?}", self.sequencer_clocking_mode.dot_clock()))));

        sequencer_vec.push((format!("{:?}", SequencerRegister::MapMask), VideoCardStateEntry::String(format!("{:04b}", self.sequencer_map_mask))));
        sequencer_vec.push((format!("{:?}", SequencerRegister::CharacterMapSelect), VideoCardStateEntry::String(format!("{}", self.sequencer_character_map_select))));
        sequencer_vec.push((format!("{:?}", SequencerRegister::MemoryMode), VideoCardStateEntry::String(format!("{}", self.sequencer_memory_mode))));
        map.insert("Sequencer".to_string(), sequencer_vec);

        let mut graphics_vec = Vec::new();
        graphics_vec.push((format!("{:?}", GraphicsRegister::SetReset), VideoCardStateEntry::String(format!("{:04b}", self.graphics_set_reset))));
        graphics_vec.push((format!("{:?}", GraphicsRegister::EnableSetReset), VideoCardStateEntry::String(format!("{:04b}", self.graphics_enable_set_reset))));
        graphics_vec.push((format!("{:?}", GraphicsRegister::ColorCompare), VideoCardStateEntry::String(format!("{:04b}", self.graphics_color_compare))));
        graphics_vec.push((format!("{:?} [fn]", GraphicsRegister::DataRotate), VideoCardStateEntry::String(format!("{:?}", self.graphics_data_rotate.function()))));
        graphics_vec.push((format!("{:?} [ct]", GraphicsRegister::DataRotate), VideoCardStateEntry::String(format!("{:?}", self.graphics_data_rotate.count()))));              
        graphics_vec.push((format!("{:?}", GraphicsRegister::ReadMapSelect), VideoCardStateEntry::String(format!("{:03b}", self.graphics_read_map_select))));

        graphics_vec.push((format!("{:?} [sr]", GraphicsRegister::Mode), VideoCardStateEntry::String(format!("{:?}", self.graphics_mode.shift_mode()))));
        graphics_vec.push((format!("{:?} [o/e]", GraphicsRegister::Mode), VideoCardStateEntry::String(format!("{:?}", self.graphics_mode.odd_even()))));
        graphics_vec.push((format!("{:?} [rm]", GraphicsRegister::Mode), VideoCardStateEntry::String(format!("{:?}",self.graphics_mode.read_mode()))));
        graphics_vec.push((format!("{:?} [tc]", GraphicsRegister::Mode), VideoCardStateEntry::String(format!("{:?}", self.graphics_mode.test_condition()))));
        graphics_vec.push((format!("{:?} [wm]", GraphicsRegister::Mode), VideoCardStateEntry::String(format!("{:?}", self.graphics_mode.write_mode()))));

        graphics_vec.push((format!("{:?} [gm]", GraphicsRegister::Miscellaneous), VideoCardStateEntry::String(format!("{:?}", self.graphics_micellaneous.graphics_mode()))));
        graphics_vec.push((format!("{:?} [com]", GraphicsRegister::Miscellaneous), VideoCardStateEntry::String(format!("{:?}", self.graphics_micellaneous.chain_odd_even()))));
        graphics_vec.push((format!("{:?} [mm]", GraphicsRegister::Miscellaneous), VideoCardStateEntry::String(format!("{:?}", self.graphics_micellaneous.memory_map()))));            

        graphics_vec.push((format!("{:?}", GraphicsRegister::ColorDontCare), VideoCardStateEntry::String(format!("{:04b}", self.graphics_color_dont_care))));
        graphics_vec.push((format!("{:?}", GraphicsRegister::BitMask), VideoCardStateEntry::String(format!("{:08b}", self.graphics_bitmask))));
        map.insert("Graphics".to_string(), graphics_vec);

        /* old-style attribute palette
        let mut attribute_pal_vec = Vec::new();
        for i in 0..16 {
            attribute_pal_vec.push((format!("Palette register {}", i), 
                VideoCardStateEntry::String(format!("{:06b}", self.attribute_palette_registers[i]))
            ));
        }
        map.insert("AttributePalette".to_string(), attribute_pal_vec);
        */

        let mut attribute_pal_vec = Vec::new();
        for i in 0..16 {

            let (r, g, b) = EGACard::ega_to_rgb(self.attribute_palette_registers[i]);
            attribute_pal_vec.push((
                format!("{}", i), 
                VideoCardStateEntry::Color(format!("{:06b}", self.attribute_palette_registers[i]), r, g, b)
            ));
        }
        map.insert("AttributePalette".to_string(), attribute_pal_vec);

        let mut attribute_vec = Vec::new();
        attribute_vec.push((format!("{:?} mode:", AttributeRegister::ModeControl), VideoCardStateEntry::String(format!("{:?}", self.attribute_mode_control.mode()))));
        attribute_vec.push((format!("{:?} disp:", AttributeRegister::ModeControl), VideoCardStateEntry::String(format!("{:?}", self.attribute_mode_control.display_type()))));
        attribute_vec.push((format!("{:?} elgc:", AttributeRegister::ModeControl), VideoCardStateEntry::String(format!("{:?}", self.attribute_mode_control.enable_line_character_codes()))));
        attribute_vec.push((format!("{:?} attr:", AttributeRegister::ModeControl), VideoCardStateEntry::String(format!("{:?}", self.attribute_mode_control.enable_blink_or_intensity()))));            

        let (r, g, b) = EGACard::ega_to_rgb( self.attribute_overscan_color.into_bytes()[0]);
        attribute_vec.push((format!("{:?}", AttributeRegister::OverscanColor), VideoCardStateEntry::Color(format!("{:06b}", self.attribute_overscan_color.into_bytes()[0]), r, g, b)));
            
        attribute_vec.push((format!("{:?} en:", AttributeRegister::ColorPlaneEnable), VideoCardStateEntry::String(format!("{:04b}", self.attribute_color_plane_enable.enable_plane()))));           
        attribute_vec.push((format!("{:?} mux:", AttributeRegister::ColorPlaneEnable), VideoCardStateEntry::String(format!("{:02b}", self.attribute_color_plane_enable.video_status_mux()))));                
        attribute_vec.push((format!("{:?}", AttributeRegister::HorizontalPelPanning), VideoCardStateEntry::String(format!("{}", self.attribute_pel_panning))));     
        //attribute_overscan_color: AOverscanColor::new(),
        //attribute_color_plane_enable: AColorPlaneEnable::new(),
        map.insert("Attribute".to_string(), attribute_vec);

        let mut internal_vec = Vec::new();

        internal_vec.push((format!("hcc:"), VideoCardStateEntry::String(format!("{}", self.hcc))));
        internal_vec.push((format!("vlc:"), VideoCardStateEntry::String(format!("{}", self.vlc))));
        internal_vec.push((format!("vcc:"), VideoCardStateEntry::String(format!("{}", self.vcc))));
        internal_vec.push((format!("hslc:"), VideoCardStateEntry::String(format!("{}", self.hslc))));
        internal_vec.push((format!("scanline:"), VideoCardStateEntry::String(format!("{}", self.scanline))));
        internal_vec.push((format!("hsc:"), VideoCardStateEntry::String(format!("{}", self.hsc))));
        internal_vec.push((format!("vma:"), VideoCardStateEntry::String(format!("{:04X}", self.vma))));
        internal_vec.push((format!("vma':"), VideoCardStateEntry::String(format!("{:04X}", self.vma_t))));
        internal_vec.push((format!("vmws:"), VideoCardStateEntry::String(format!("{}", self.vmws))));
        internal_vec.push((format!("rba:"), VideoCardStateEntry::String(format!("{:04X}", self.rba))));
        internal_vec.push((format!("de:"), VideoCardStateEntry::String(format!("{}", self.in_display_area))));
        internal_vec.push((format!("crtc_hblank:"), VideoCardStateEntry::String(format!("{}", self.crtc_hblank))));
        internal_vec.push((format!("crtc_vblank:"), VideoCardStateEntry::String(format!("{}", self.crtc_vblank))));
        internal_vec.push((format!("raster_x:"), VideoCardStateEntry::String(format!("{}", self.raster_x))));
        internal_vec.push((format!("raster_y:"), VideoCardStateEntry::String(format!("{}", self.raster_y))));
        internal_vec.push((format!("border:"), VideoCardStateEntry::String(format!("{}", self.crtc_hborder))));
        //internal_vec.push((format!("s_reads:"), VideoCardStateEntry::String(format!("{}", self.status_reads))));
        //internal_vec.push((format!("missed_hsyncs:"), VideoCardStateEntry::String(format!("{}", self.missed_hsyncs))));
        //internal_vec.push((format!("vsync_cycles:"), VideoCardStateEntry::String(format!("{}", self.cycles_per_vsync))));
        //internal_vec.push((format!("cur_screen_cycles:"), VideoCardStateEntry::String(format!("{}", self.cur_screen_cycles))));
        //internal_vec.push((format!("phase:"), VideoCardStateEntry::String(format!("{}", self.cycles & 0x0F))));
        //internal_vec.push((format!("cursor attr:"), VideoCardStateEntry::String(format!("{:02b}", self.cursor_attr))));

        internal_vec.push((format!("hsync_ct:"), VideoCardStateEntry::String(format!("{}", self.hsync_ct))));
        internal_vec.push((format!("vsync_ct:"), VideoCardStateEntry::String(format!("{}", self.vsync_ct))));

        map.insert("Internal".to_string(), internal_vec);

        map
    }

    fn run(&mut self, time: DeviceRunTimeUnit) {

        if let DeviceRunTimeUnit::Microseconds(us) = time {

            // Select the appropriate timings based on the current clocking mode
            let ticks = match self.misc_output_register.clock_select() {
                ClockSelect::Clock14 => us * EGA_CLOCK0,
                ClockSelect::Clock16 => us * EGA_CLOCK1,
                _ => 0.0
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

    fn get_pixel(&self, _x: u32, _y: u32 ) -> &[u8] {
        &DUMMY_PIXEL
    }

    fn get_pixel_raw(&self, x: u32, y:u32) -> u8 {
        
        let mut byte = 0;

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
            read_offset = (y_offset + x_byte_offset + self.crtc_start_address as u32 ) as usize;
        }
        
        if read_offset < self.planes[0].buf.len() {

            for i in 0..4 {
            
                let read_byte = self.planes[i].buf[read_offset];
                let read_bit = match read_byte & (0x01 << (7-x_bit_offset)) != 0 {
                    true => 1,
                    false => 0
                };
    
                //byte |= read_bit << (3 - i);
                byte |= read_bit << i;
            }
            // return self.attribute_palette_registers[byte & 0x0F].into_bytes()[0];
            return self.attribute_palette_registers[byte & 0x0F];
        }
        0
    }

    fn get_plane_slice(&self, plane: usize) -> &[u8] {

        &self.planes[plane].buf
    }

    fn dump_mem(&self, path: &Path) {
        
        for i in 0..4 {

            let mut filename = path.to_path_buf();
            filename.push(format!("ega_plane{}.bin", i));
            
            match std::fs::write(filename.clone(), &self.planes[i].buf) {
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

}

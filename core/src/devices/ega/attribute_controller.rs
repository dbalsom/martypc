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

    ega::attribute_controller.rs

    Implements the EGA Attribute Controller

*/

use super::*;

#[derive(Copy, Clone, Debug)]
pub enum AttributeRegister {
    Palette0,
    Palette1,
    Palette2,
    Palette3,
    Palette4,
    Palette5,
    Palette6,
    Palette7,
    Palette8,
    Palette9,
    PaletteA,
    PaletteB,
    PaletteC,
    PaletteD,
    PaletteE,
    PaletteF,
    ModeControl,
    OverscanColor,
    ColorPlaneEnable,
    HorizontalPelPanning,
}

#[derive(Debug)]
pub enum AttributeRegisterFlipFlop {
    Address,
    Data,
}

#[derive(Debug, BitfieldSpecifier)]
pub enum AttributeMode {
    Text,
    Graphics,
}

#[derive(Debug, BitfieldSpecifier)]
pub enum AttributeDisplayType {
    Color,
    Monochrome,
}

#[derive(Debug, BitfieldSpecifier)]
pub enum AttributeBlinkOrIntensity {
    BackgroundIntensity,
    Blink,
}

#[bitfield]
#[allow(dead_code)]
pub struct AttributeAddress {
    address: B5,
    address_source: B1,
    unused: B2,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct APaletteRegister {
    pub blue: B1,
    pub green: B1,
    pub red: B1,
    pub secondary_blue: B1,
    pub secondary_green: B1,
    pub secondary_red: B1,
    #[skip]
    unused: B2,
}

#[bitfield]
pub struct AModeControl {
    #[bits = 1]
    pub mode: AttributeMode,
    #[bits = 1]
    pub display_type: AttributeDisplayType,
    pub enable_line_character_codes: bool,
    #[bits = 1]
    pub enable_blink_or_intensity: AttributeBlinkOrIntensity,
    #[skip]
    unused: B4,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct AOverscanColor {
    pub blue: B1,
    pub green: B1,
    pub red: B1,
    pub secondary_blue: B1,
    pub secondary_green: B1,
    pub secondary_red: B1,
    #[skip]
    unused: B2,
}

#[bitfield]
pub struct AColorPlaneEnable {
    pub enable_plane: B4,
    pub video_status_mux: B2,
    #[skip]
    unused: B2,
}

pub enum AttributeInput<'a> {
    Black,
    SolidColor(u8),
    HBlank,
    VBlank,
    Border,
    Serial(&'a [u8]),
    Serial64(u64),
    Parallel(&'a [u8], u8, bool),
    Parallel64(u64, u8, bool),
}

#[derive(Copy, Clone, Debug, Default)]
pub struct AttributePaletteEntry {
    pub six: u8,
    pub four: u8,
    pub four_to_six: u8,
    pub mono: bool,
}

impl AttributePaletteEntry {
    pub fn set(&mut self, byte: u8) {
        self.six = byte & 0x3F;
        self.four = byte & 0x0F | ((byte & 0x10) >> 1);
        self.four_to_six = CGA_TO_EGA_U8[self.four as usize];
        self.mono = (byte & 0x08) != 0;
    }
}

pub struct AttributeController {
    register_flipflop: AttributeRegisterFlipFlop,
    register_select_byte: u8,
    register_selected: AttributeRegister,
    pub palette_registers: [AttributePaletteEntry; 16],
    palette_index: usize,
    mode_control: AModeControl,
    pub overscan_color: AttributePaletteEntry,
    overscan_color64: u64,
    color_plane_enable: AColorPlaneEnable,
    color_plane_enable64: u64,
    pel_panning: u8,
    blink_state: bool,
    last_den: bool,
    shift_reg: u128,
    shift_buf: [u8; 8],
}

impl Default for AttributeController {
    fn default() -> Self {
        Self {
            register_flipflop: AttributeRegisterFlipFlop::Address,
            register_select_byte: 0,
            register_selected: AttributeRegister::Palette0,
            palette_registers: [Default::default(); 16],
            palette_index: 0,
            mode_control: AModeControl::new(),
            overscan_color: AttributePaletteEntry::default(),
            overscan_color64: 0,
            color_plane_enable: AColorPlaneEnable::new(),
            color_plane_enable64: !0,
            pel_panning: 0,
            blink_state: false,
            last_den: false,
            shift_reg: 0,
            shift_buf: [0; 8],
        }
    }
}

impl AttributeController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset_flipflop(&mut self) {
        self.register_flipflop = AttributeRegisterFlipFlop::Address;
    }
    /// Handle a write to the Attribute Register 0x3C0.
    ///
    /// Unlike the other register files on the EGA, the Attribute Register doesn't have an
    /// address port. Instead, it maintains a flipflop that determines whether the port 0x3C0
    /// is in address or data mode. The flipflop is reset to a known state by reading 0x3DA.
    pub fn write_attribute_register(&mut self, byte: u8) {
        match self.register_flipflop {
            AttributeRegisterFlipFlop::Address => {
                if byte <= 0x0F {
                    self.palette_index = byte as usize;
                }
                self.register_selected = match byte & 0x1F {
                    0x00 => AttributeRegister::Palette0,
                    0x01 => AttributeRegister::Palette1,
                    0x02 => AttributeRegister::Palette2,
                    0x03 => AttributeRegister::Palette3,
                    0x04 => AttributeRegister::Palette4,
                    0x05 => AttributeRegister::Palette5,
                    0x06 => AttributeRegister::Palette6,
                    0x07 => AttributeRegister::Palette7,
                    0x08 => AttributeRegister::Palette8,
                    0x09 => AttributeRegister::Palette9,
                    0x0A => AttributeRegister::PaletteA,
                    0x0B => AttributeRegister::PaletteB,
                    0x0C => AttributeRegister::PaletteC,
                    0x0D => AttributeRegister::PaletteD,
                    0x0E => AttributeRegister::PaletteE,
                    0x0F => AttributeRegister::PaletteF,
                    0x10 => AttributeRegister::ModeControl,
                    0x11 => AttributeRegister::OverscanColor,
                    0x12 => AttributeRegister::ColorPlaneEnable,
                    0x13 => AttributeRegister::HorizontalPelPanning,
                    _ => {
                        log::warn!("Invalid attribute register selected: {:02X}", byte);
                        self.register_selected
                    }
                };

                self.register_flipflop = AttributeRegisterFlipFlop::Data;
            }
            AttributeRegisterFlipFlop::Data => {
                match self.register_selected {
                    AttributeRegister::Palette0
                    | AttributeRegister::Palette1
                    | AttributeRegister::Palette2
                    | AttributeRegister::Palette3
                    | AttributeRegister::Palette4
                    | AttributeRegister::Palette5
                    | AttributeRegister::Palette6
                    | AttributeRegister::Palette7
                    | AttributeRegister::Palette8
                    | AttributeRegister::Palette9
                    | AttributeRegister::PaletteA
                    | AttributeRegister::PaletteB
                    | AttributeRegister::PaletteC
                    | AttributeRegister::PaletteD
                    | AttributeRegister::PaletteE
                    | AttributeRegister::PaletteF => {
                        //self.palette_registers[self.palette_index] = APaletteRegister::from_bytes([byte]);
                        //log::debug!("set palette index {} to {:08b}", self.palette_index, byte );
                        self.palette_registers[self.palette_index].set(byte);
                    }
                    AttributeRegister::ModeControl => {
                        self.mode_control = AModeControl::from_bytes([byte]);
                    }
                    AttributeRegister::OverscanColor => {
                        self.overscan_color.set(byte);
                    }
                    AttributeRegister::ColorPlaneEnable => {
                        self.color_plane_enable = AColorPlaneEnable::from_bytes([byte]);
                        self.recalculate_plane_enable();
                    }
                    AttributeRegister::HorizontalPelPanning => {
                        self.pel_panning = byte & 0x0F;
                        //log::debug!("pel panning set to {}", self.pel_panning);
                    }
                }

                // IBM: "The flip-flop toggles each time an OUT is issued to the Attribute Controller"
                self.register_flipflop = AttributeRegisterFlipFlop::Address;
            }
        }
    }

    fn recalculate_plane_enable(&mut self) {
        self.color_plane_enable64 = 0;
        for i in 0..8 {
            self.color_plane_enable64 |= (self.color_plane_enable.enable_plane() as u64) << (i * 8);
        }
    }

    pub fn mode(&self) -> AttributeMode {
        self.mode_control.mode()
    }

    pub fn display_type(&self) -> AttributeDisplayType {
        self.mode_control.display_type()
    }

    /// Load the attribute controller with a new AttributeInput.
    /// Should be called after shift_outX to make room for the new character clock worth of data.
    pub fn load(&mut self, input: AttributeInput, clock_select: ClockSelect, den: bool) {
        let mut ai = input;
        // The attribute controller will emit the border color when display enable is low.
        if !den && (den == self.last_den) {
            // Delay border by one character clock. I can't tell if this is an ugly hack or something the
            // attribute controller actually does, but it's necessary to get the text mode to align and for
            // the pel panning to work properly at the right edge of the screen.
            ai = AttributeInput::Border;
        }
        self.last_den = den;

        match ai {
            AttributeInput::Black => {
                // If we do nothing - black will be produced
            }
            AttributeInput::HBlank => {
                // If we do nothing - hblank remains black
            }
            AttributeInput::VBlank => {
                // If we do nothing - vblank remains black
            }
            AttributeInput::SolidColor(color) => {
                // Draw a character span of solid color
                self.shift_reg |= BYTE_EXTEND_TABLE64[color as usize] as u128;
            }
            AttributeInput::Border => {
                // In border area, shift in overscan color

                //self.shift_reg |= BYTE_EXTEND_TABLE64[EgaDefaultColor6Bpp::GreenBright as usize] as u128;
                self.shift_reg |= BYTE_EXTEND_TABLE64[self.overscan_color.six as usize] as u128;
            }
            AttributeInput::Serial(data) => match clock_select {
                ClockSelect::Clock14 => {
                    for (i, byte) in data.iter().enumerate() {
                        let color = *byte & self.color_plane_enable.enable_plane();
                        self.shift_reg |= (self.palette_registers[color as usize].four_to_six as u128) << ((7 - i) * 8);
                    }
                }
                _ => {
                    for (i, byte) in data.iter().enumerate() {
                        let color = *byte & self.color_plane_enable.enable_plane();
                        self.shift_reg |= (self.palette_registers[color as usize].six as u128) << ((7 - i) * 8);
                    }
                }
            },
            AttributeInput::Serial64(data) => {
                self.shift_reg |= (data & self.color_plane_enable64) as u128;
            }
            AttributeInput::Parallel(data, attr, cursor) => {
                let mut resolved_glyph = 0;
                if cursor {
                    resolved_glyph = BYTE_EXTEND_TABLE64[0xFF];
                }
                else {
                    for (i, byte) in data.iter().enumerate() {
                        resolved_glyph |= (*byte as u64) << ((7 - i) * 8);
                    }
                }
                resolved_glyph = self.apply_attribute(resolved_glyph, attr, clock_select);
                self.shift_reg |= resolved_glyph as u128;
            }
            AttributeInput::Parallel64(data, attr, cursor) => {
                let resolved_glyph = if cursor { ALL_SET64 } else { data };
                self.shift_reg |= self.apply_attribute(resolved_glyph, attr, clock_select) as u128;
            }
        }
    }

    pub fn shift_out64(&mut self) -> u64 {
        let out_data = ((self.shift_reg << ((self.pel_panning & 0x07) * 8)) >> 64) as u64;

        // Shift the attribute data 64 bits to make room for next character clock
        self.shift_reg <<= 64;

        out_data.to_be()
    }

    #[rustfmt::skip]
    pub fn shift_out64_halfclock(&mut self) -> (u64, u64) {
        let mut out_data0 = 0;
        let mut out_data1 = 0;

        let out_data = ((self.shift_reg << (std::cmp::min(self.pel_panning, 0x07) * 8)) >> 64) as u64;

        // Shift the attribute data 64 bits to make room for next character clock
        self.shift_reg <<= 64;

        out_data0 |= (out_data & 0xFF00000000000000) >> 56; // -> 0x00000000000000FF
        out_data0 |= (out_data & 0xFF00000000000000) >> 48; // -> 0x000000000000FF00
        out_data0 |= (out_data & 0x00FF000000000000) >> 32; // -> 0x0000000000FF0000
        out_data0 |= (out_data & 0x00FF000000000000) >> 24; // -> 0x00000000FF000000
        out_data0 |= (out_data & 0x0000FF0000000000) >> 8;  // -> 0x000000FF00000000
        out_data0 |= out_data & 0x0000FF0000000000;         // -> 0x0000FF0000000000
        out_data0 |= (out_data & 0x000000FF00000000) << 16; // -> 0x00FF000000000000
        out_data0 |= (out_data & 0x000000FF00000000) << 24; // -> 0xFF00000000000000

        out_data1 |= (out_data & 0x00000000FF000000) >> 24; // -> 0x00000000000000FF
        out_data1 |= (out_data & 0x00000000FF000000) >> 16; // -> 0x000000000000FF00
        out_data1 |= out_data & 0x0000000000FF0000;         // -> 0x0000000000FF0000
        out_data1 |= (out_data & 0x0000000000FF0000) << 8;  // -> 0x00000000FF000000
        out_data1 |= (out_data & 0x000000000000FF00) << 24; // -> 0x000000FF00000000
        out_data1 |= (out_data & 0x000000000000FF00) << 32; // -> 0x0000FF0000000000
        out_data1 |= (out_data & 0x00000000000000FF) << 48; // -> 0x00FF000000000000
        out_data1 |= (out_data & 0x00000000000000FF) << 56; // -> 0xFF00000000000000

        (out_data0, out_data1)
    }

    /*    pub fn shift_out64_halfclock2(&mut self) -> (u64, u64) {
        let out_data = ((self.shift_reg << (self.pel_panning * 8)) >> 64) as u64;
        let mut output1: u64 = 0;
        let mut output2: u64 = 0;

        for i in 0..4 {
            // Extract each byte (pixel)
            let pixel = (out_data >> (56 - i * 8)) & 0xFF;
            // Double the pixel (still fits within u64)
            let doubled_pixel = pixel | (pixel << 8);

            // Place the doubled pixels in the output values
            output1 |= (doubled_pixel as u64) << (16 * i);
        }
        for i in 0..4 {
            // Extract each byte (pixel)
            let pixel = (out_data >> (56 - (i + 4) * 8)) & 0xFF;
            // Double the pixel (still fits within u64)
            let doubled_pixel = pixel | (pixel << 8);

            // Place the doubled pixels in the output values
            output2 |= (doubled_pixel as u64) << (16 * i);
        }

        (output1.to_le(), output2.to_le())
    }*/

    pub fn shift_out_8(&mut self) -> &[u8] {
        let out_data = ((self.shift_reg << ((self.pel_panning & 0x07) * 8)) >> 64) as u64;
        self.shift_buf = out_data.to_le_bytes();
        &self.shift_buf
    }

    #[inline]
    pub fn apply_attribute(&self, glyph_row_base: u64, attribute: u8, clock_select: ClockSelect) -> u64 {
        let mut fg_index = (attribute & 0x0F) as usize;
        let mut bg_index = (attribute >> 4) as usize;

        // If blinking is enabled, the bg attribute is only 3 bits and only low-intensity colors
        // are available.
        // If blinking is disabled, all 16 colors are available as background attributes.
        if let AttributeBlinkOrIntensity::Blink = self.mode_control.enable_blink_or_intensity() {
            bg_index = ((attribute >> 4) & 0x07) as usize;
            let char_blink = attribute & 0x80 != 0;
            if char_blink && self.blink_state {
                // Blinking on the EGA is implemented by toggling the MSB of the color index
                bg_index |= 0x08;
                fg_index ^= 0x08;
            }
        }

        let fg_color;
        let bg_color;

        match clock_select {
            ClockSelect::Clock14 => {
                fg_color = self.palette_registers[fg_index].four_to_six as usize;
                bg_color = self.palette_registers[bg_index].four_to_six as usize;
            }
            _ => {
                fg_color = self.palette_registers[fg_index].six as usize;
                bg_color = self.palette_registers[bg_index].six as usize;
            }
        }

        // Combine glyph mask with foreground and background colors.
        glyph_row_base & EGA_COLORS_U64[fg_color] | !glyph_row_base & EGA_COLORS_U64[bg_color]
    }

    pub fn palette(&self, pel: u8) -> u8 {
        self.palette_registers[(pel & 0x0F) as usize].six
    }

    pub fn palette_four(&self, pel: u8) -> u8 {
        self.palette_registers[(pel & 0x0F) as usize].four_to_six
    }

    #[rustfmt::skip]
    pub fn get_state(&self) -> Vec<(String, VideoCardStateEntry)> {
        let mut attribute_vec = Vec::new();
        attribute_vec.push((format!("{:?} [mode]", AttributeRegister::ModeControl), VideoCardStateEntry::String(format!("{:?}", self.mode_control.mode()))));
        attribute_vec.push((format!("{:?} [disp]", AttributeRegister::ModeControl), VideoCardStateEntry::String(format!("{:?}", self.mode_control.display_type()))));
        attribute_vec.push((format!("{:?} [elgc]", AttributeRegister::ModeControl), VideoCardStateEntry::String(format!("{:?}", self.mode_control.enable_line_character_codes()))));
        attribute_vec.push((format!("{:?} [attr]", AttributeRegister::ModeControl), VideoCardStateEntry::String(format!("{:?}", self.mode_control.enable_blink_or_intensity()))));

        let (r, g, b) = EGACard::ega_to_rgb(self.overscan_color.six);
        attribute_vec.push((format!("{:?}", AttributeRegister::OverscanColor), VideoCardStateEntry::Color(format!("{:06b}", self.overscan_color.six), r, g, b)));

        attribute_vec.push((format!("{:?} [en]", AttributeRegister::ColorPlaneEnable), VideoCardStateEntry::String(format!("{:04b}", self.color_plane_enable.enable_plane()))));
        attribute_vec.push((format!("{:?} [mux]", AttributeRegister::ColorPlaneEnable), VideoCardStateEntry::String(format!("{:02b}", self.color_plane_enable.video_status_mux()))));
        attribute_vec.push((format!("{:?}", AttributeRegister::HorizontalPelPanning), VideoCardStateEntry::String(format!("{}", self.pel_panning))));

        attribute_vec
    }
}

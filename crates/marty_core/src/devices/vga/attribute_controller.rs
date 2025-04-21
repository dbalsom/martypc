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

    ---------------------------------------------------------------------------
*/

//! Implements the VGA's Attribute Controller.

use super::*;
use std::cmp::PartialEq;

pub const DAC_STATE_READ: u8 = 0;
pub const DAC_STATE_WRITE: u8 = 0x03;

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
    ColorSelect,
}

#[derive(Debug)]
pub enum AttributeRegisterFlipFlop {
    Address,
    Data,
}

#[derive(Debug, PartialEq, BitfieldSpecifier)]
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

#[derive(Debug, BitfieldSpecifier)]
pub enum PixelClockSelect {
    EveryCycle,
    EveryOtherCycle,
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
#[derive(Copy, Clone)]
pub struct AModeControl {
    #[bits = 1]
    pub mode: AttributeMode,
    #[bits = 1]
    pub display_type: AttributeDisplayType,
    pub enable_line_character_codes: bool,
    #[bits = 1]
    pub enable_blink_or_intensity: AttributeBlinkOrIntensity,
    #[skip]
    unused: B1,
    pixel_pan_compatibility: B1,
    pixel_clock_select: PixelClockSelect,
    internal_palette_size: B1,
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
#[derive(Copy, Clone)]
pub struct AColorPlaneEnable {
    pub enable_plane: B4,
    pub video_status_mux: B2,
    #[skip]
    unused: B2,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct AColorSelectRegister {
    pub c45: B2,
    pub c67: B2,
    #[skip]
    unused:  B4,
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
    Parallel64(u64, u8, u8, bool),
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

pub struct ColorRegister {
    pub(crate) native: [u8; 3],
    pub(crate) rgba:   [u8; 4],
    pub(crate) u32:    u32,
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
    color_select: AColorSelectRegister,
    pel_panning: u8,
    blink_state: bool,
    last_den: bool,
    shift_reg: u128,
    shift_reg9: u64,
    shift_buf: [u8; 8],

    shift_flipflop: bool,

    pub color_registers: [[u8; 3]; 256],
    pub color_registers_rgba: [[u8; 4]; 256],
    pub color_registers_u32: [u32; 256],
    color_pel_write_address: u8,
    color_pel_write_address_color: u8,
    color_pel_read_address: u8,
    color_pel_read_address_color: u8,
    color_pel_mask: u8,
    color_dac_state: u8,
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
            color_select: AColorSelectRegister::new(),
            pel_panning: 0,
            blink_state: false,
            last_den: false,
            shift_reg: 0,
            shift_reg9: 0,
            shift_buf: [0; 8],

            shift_flipflop: false,

            color_registers: [[0; 3]; 256],
            color_registers_rgba: [[0; 4]; 256],
            color_registers_u32: [0; 256],
            color_pel_write_address: 0,
            color_pel_write_address_color: 0,
            color_pel_read_address: 0,
            color_pel_read_address_color: 0,
            color_pel_mask: 0,
            color_dac_state: 0,
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

    /// On the VGA, the Attribute Controller registers are readable.
    /// Titles like QBasic will rely on this to properly set their text modes, or else they
    /// will inadvertently set a graphics mode.
    pub fn read_attribute_register(&mut self) -> u8 {
        match self.register_flipflop {
            AttributeRegisterFlipFlop::Address => self.register_selected as u8,
            AttributeRegisterFlipFlop::Data => match self.register_selected {
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
                | AttributeRegister::PaletteF => self.palette_registers[self.palette_index].six,
                AttributeRegister::ModeControl => self.mode_control.into_bytes()[0],
                AttributeRegister::OverscanColor => self.overscan_color.six,
                AttributeRegister::ColorPlaneEnable => self.color_plane_enable.into_bytes()[0],
                AttributeRegister::HorizontalPelPanning => self.pel_panning,
                AttributeRegister::ColorSelect => self.color_select.into_bytes()[0],
            },
        }
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
                    AttributeRegister::ColorSelect => {
                        self.color_select = AColorSelectRegister::from_bytes([byte]);
                        //log::debug!("color select set to {:08b}", byte);
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

    #[inline(always)]
    pub fn palette_lookup(&self, index: u8) -> u32 {
        let color_index = match self.mode_control.internal_palette_size() {
            0 => {
                // Substitute bits 6-7 of the palette register with bits 0-1 of the color register
                (self.palette_registers[(index & 0x0F) as usize].six | (self.color_select.c67() << 6)) as usize
            }
            _ => {
                // Substitute bits 4-5 of the palette register with bits 0-1 of the color register
                ((self.palette_registers[(index & 0x0F) as usize].six & 0x0F) | (self.color_select.c45() << 4)) as usize
            }
        };

        self.color_registers_u32[color_index]
    }

    #[inline]
    pub fn mode(&self) -> AttributeMode {
        self.mode_control.mode()
    }

    #[inline]
    pub fn is_text_mode(&self) -> bool {
        self.mode_control.mode() == AttributeMode::Text
    }

    #[inline]
    pub fn pixel_clock(&self) -> PixelClockSelect {
        self.mode_control.pixel_clock_select()
    }

    #[inline]
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
        self.shift_reg9 = 0;

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
                ClockSelect::Clock25 => match self.mode_control.pixel_clock_select() {
                    PixelClockSelect::EveryCycle => {
                        for (i, byte) in data.iter().enumerate() {
                            let color = *byte & self.color_plane_enable.enable_plane();
                            self.shift_reg |= (color as u128) << ((7 - i) * 8);
                        }
                    }
                    PixelClockSelect::EveryOtherCycle => {
                        let shift_idx = if !self.shift_flipflop { 7 } else { 3 };
                        for (i, b) in data.chunks_exact(2).enumerate() {
                            self.shift_reg |= ((b[0] as u128) << ((shift_idx - i) * 8)) << 4;
                            self.shift_reg |= (b[1] as u128) << ((shift_idx - i) * 8);
                        }
                        self.shift_flipflop = !self.shift_flipflop;
                    }
                },
                _ => {
                    for (i, byte) in data.iter().enumerate() {
                        let color = *byte & self.color_plane_enable.enable_plane();
                        self.shift_reg |= (color as u128) << ((7 - i) * 8);
                    }
                }
            },
            AttributeInput::Serial64(data) => {
                self.shift_reg |= (data & self.color_plane_enable64) as u128;
            }
            AttributeInput::Parallel(data, attr, cursor) => {
                let mut resolved_glyph = 0;
                if cursor {
                    resolved_glyph = ALL_SET64;
                }
                else {
                    for (i, byte) in data.iter().enumerate() {
                        resolved_glyph |= (*byte as u64) << ((7 - i) * 8);
                    }
                }
                resolved_glyph = self.apply_attribute_8col(resolved_glyph, attr, clock_select);
                self.shift_reg |= resolved_glyph as u128;
            }
            AttributeInput::Parallel64(data, char, attr, cursor) => {
                let resolved_glyph = if cursor { ALL_SET64 } else { data };

                // If character is a line drawing character
                let col9 = if char & LINE_CHAR_MASK == LINE_CHAR_TEST {
                    (data & 0xFF) as u8
                }
                else {
                    0
                };

                let attr = self.apply_attribute_9col(resolved_glyph, col9, attr, clock_select);
                self.shift_reg9 = (attr.0 & 0xFF) << 8 | (attr.1 as u64);
                self.shift_reg |= attr.0 as u128 >> 8;
            }
        }
    }

    pub fn shift_out64(&mut self) -> u64 {
        let out_data = ((self.shift_reg << ((self.pel_panning & 0x07) * 8)) >> 64) as u64;

        // Shift the attribute data 64 bits to make room for next character clock
        self.shift_reg <<= 64;
        out_data.to_be()
    }

    pub fn shift_out64_9(&mut self) -> (u64, u8) {
        let shifted_reg = self.shift_reg << ((self.pel_panning & 0x07) * 8);
        let out_data_64 = (shifted_reg >> 64) as u64;
        let out_data_8 = (shifted_reg >> 56) as u8;

        // Shift the attribute data 72 bits to make room for next character clock
        self.shift_reg <<= 72;
        // Add the 16 bits from the spillover register
        self.shift_reg |= ((self.shift_reg9 & 0xFFFF) as u128) << 56;
        (out_data_64.to_be(), out_data_8)
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

    #[rustfmt::skip]
    pub fn shift_out64_halfclock_9col(&mut self) -> (u64, u64, u8) {
        let mut out_data0 = 0;
        let mut out_data1 = 0;

        let shifted_reg = self.shift_reg << ((self.pel_panning & 0x07) * 8);
        let out_data = (shifted_reg >> 64) as u64;
        let out_data_8 = (shifted_reg >> 56) as u8;

        // Shift the attribute data 72 bits to make room for next character clock
        self.shift_reg <<= 72;
        // Add the 16 bits from the spillover register
        self.shift_reg |= ((self.shift_reg9 & 0xFFFF) as u128) << 56;

        out_data0 |= (out_data & 0xFF00000000000000) >> 56; // -> 0x00000000000000FF
        out_data0 |= (out_data & 0xFF00000000000000) >> 48; // -> 0x000000000000FF00
        out_data0 |= (out_data & 0x00FF000000000000) >> 32; // -> 0x0000000000FF0000
        out_data0 |= (out_data & 0x00FF000000000000) >> 24; // -> 0x00000000FF000000
        out_data0 |= (out_data & 0x0000FF0000000000) >> 8;  // -> 0x000000FF00000000
        out_data0 |=  out_data & 0x0000FF0000000000;        // -> 0x0000FF0000000000
        out_data0 |= (out_data & 0x000000FF00000000) << 16; // -> 0x00FF000000000000
        out_data0 |= (out_data & 0x000000FF00000000) << 24; // -> 0xFF00000000000000

        out_data1 |= (out_data & 0x00000000FF000000) >> 24; // -> 0x00000000000000FF
        out_data1 |= (out_data & 0x00000000FF000000) >> 16; // -> 0x000000000000FF00
        out_data1 |=  out_data & 0x0000000000FF0000;        // -> 0x0000000000FF0000
        out_data1 |= (out_data & 0x0000000000FF0000) << 8;  // -> 0x00000000FF000000
        out_data1 |= (out_data & 0x000000000000FF00) << 24; // -> 0x000000FF00000000
        out_data1 |= (out_data & 0x000000000000FF00) << 32; // -> 0x0000FF0000000000
        out_data1 |= (out_data & 0x00000000000000FF) << 48; // -> 0x00FF000000000000
        out_data1 |= (out_data & 0x00000000000000FF) << 56; // -> 0xFF00000000000000

        (out_data0, out_data1, out_data_8)
    }

    #[rustfmt::skip]
    pub fn shift_out64_mode13(&mut self) -> u64 {
        let mut out_data0 = 0;
        let mut out_data1 = 0;

        let out_data = ((self.shift_reg << (std::cmp::min(self.pel_panning, 0x07) * 8)) >> 64) as u64;

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

        if !self.shift_flipflop {
            out_data1
        }
        else {
            self.shift_reg <<= 64;
            out_data0
        }
    }

    pub fn shift_out_8(&mut self) -> &[u8] {
        let out_data = ((self.shift_reg << ((self.pel_panning & 0x07) * 8)) >> 64) as u64;
        self.shift_buf = out_data.to_be_bytes();
        self.shift_reg <<= 64;
        &self.shift_buf
    }

    #[inline]
    pub fn apply_attribute_8col(&self, glyph_row_base: u64, attribute: u8, clock_select: ClockSelect) -> u64 {
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
            ClockSelect::Clock25 => {
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

    #[inline]
    pub fn apply_attribute_9col(
        &self,
        glyph_row_base: u64,
        glyph_col_9: u8,
        attribute: u8,
        clock_select: ClockSelect,
    ) -> (u64, u8) {
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
            ClockSelect::Clock25 => {
                fg_color = self.palette_registers[fg_index].four_to_six as usize;
                bg_color = self.palette_registers[bg_index].four_to_six as usize;
            }
            _ => {
                fg_color = self.palette_registers[fg_index].six as usize;
                bg_color = self.palette_registers[bg_index].six as usize;
            }
        }

        // Combine glyph mask with foreground and background colors.
        let glyph_u64 = glyph_row_base & EGA_COLORS_U64[fg_color] | !glyph_row_base & EGA_COLORS_U64[bg_color];
        let glyph_u8 = glyph_col_9 & fg_color as u8 | !glyph_col_9 & bg_color as u8;
        (glyph_u64, glyph_u8)
    }

    pub fn palette(&self, pel: u8) -> u8 {
        self.palette_registers[(pel & 0x0F) as usize].six
    }

    pub fn palette_four(&self, pel: u8) -> u8 {
        self.palette_registers[(pel & 0x0F) as usize].four_to_six
    }

    pub fn read_pel_address_write_mode(&self) -> u8 {
        self.color_pel_write_address
    }

    pub fn write_pel_address_write_mode(&mut self, data: u8) {
        self.color_pel_write_address = data;
    }

    pub fn write_pel_address_read_mode(&mut self, data: u8) {
        self.color_pel_read_address = data;
    }

    pub fn read_color_dac_state(&self) -> u8 {
        self.color_dac_state
    }

    pub fn read_pel_mask(&self) -> u8 {
        self.color_pel_mask
    }

    pub fn write_pel_mask(&mut self, data: u8) {
        self.color_pel_mask = data;
    }

    pub fn read_pel_data(&mut self) -> u8 {
        let color = self.color_pel_read_address as usize;
        let rgb_idx = self.color_pel_read_address_color as usize;

        let byte = self.color_registers[color][rgb_idx];

        // Automatically increment to next color register, cycling through
        // Red, Green and Blue registers per Read Index
        self.color_pel_read_address_color += 1;
        if self.color_pel_read_address_color == 3 {
            self.color_pel_read_address_color = 0;
            // Done with all colors, so go to next palette entry

            /*
                There's an apparent 'bug' in the IBM VGA BIOS palette register test, where 768 test
                values are written to the color registers.
                These are then read back and tested, but the register address is not initialized to
                zero first. This implies that the palette address wraps around on increment.
            */
            self.color_pel_read_address = self.color_pel_read_address.wrapping_add(1);
        }

        self.color_dac_state = DAC_STATE_READ;
        byte
    }

    pub fn write_pel_data(&mut self, byte: u8) {
        let color = self.color_pel_write_address as usize;
        let rgb_idx = self.color_pel_write_address_color as usize;

        self.color_registers[color][rgb_idx] = byte;

        // Automatically increment to next color register, cycling through
        // Red, Green and Blue registers per Read Index
        self.color_pel_write_address_color += 1;
        if self.color_pel_write_address_color == 3 {
            // Save converted RGBA palette entries along with native ones
            let r = ((self.color_registers[color][0] as u32 * 255) / 63) as u8;
            let g = ((self.color_registers[color][1] as u32 * 255) / 63) as u8;
            let b = ((self.color_registers[color][2] as u32 * 255) / 63) as u8;
            self.color_registers_rgba[color][0] = r;
            self.color_registers_rgba[color][1] = g;
            self.color_registers_rgba[color][2] = b;
            self.color_registers_rgba[color][3] = 0xFF;
            self.color_registers_u32[color] = 0xFF << 24 | (b as u32) << 16 | (g as u32) << 8 | r as u32;

            /*
            trace!(
                self,
                "Wrote color register [{}] ({:02X},{:02X},{:02X})",
                color,
                self.color_registers[color][0],
                self.color_registers[color][1],
                self.color_registers[color][2]
            );*/

            /*
            log::trace!("Wrote color register [{}] ({:02X},{:02X},{:02X})",
                color,
                self.color_registers[color][0],
                self.color_registers[color][1],
                self.color_registers[color][2]);
            */
            self.color_pel_write_address_color = 0;
            // Done with all colors, so go to next palette entry
            self.color_pel_write_address = self.color_pel_write_address.wrapping_add(1);
        }

        self.color_dac_state = DAC_STATE_WRITE;
    }

    #[rustfmt::skip]
    pub fn get_state(&self) -> Vec<(String, VideoCardStateEntry)> {
        let mut attribute_vec = Vec::new();
        attribute_vec.push((format!("{:?}", AttributeRegister::ModeControl), VideoCardStateEntry::String(format!("{:08b}", self.mode_control.into_bytes()[0]))));
        attribute_vec.push((format!("{:?} [mode]", AttributeRegister::ModeControl), VideoCardStateEntry::String(format!("{:?}", self.mode_control.mode()))));
        attribute_vec.push((format!("{:?} [disp]", AttributeRegister::ModeControl), VideoCardStateEntry::String(format!("{:?}", self.mode_control.display_type()))));
        attribute_vec.push((format!("{:?} [elgc]", AttributeRegister::ModeControl), VideoCardStateEntry::String(format!("{:?}", self.mode_control.enable_line_character_codes()))));
        attribute_vec.push((format!("{:?} [attr]", AttributeRegister::ModeControl), VideoCardStateEntry::String(format!("{:?}", self.mode_control.enable_blink_or_intensity()))));
        attribute_vec.push((format!("{:?} [pcs]", AttributeRegister::ModeControl), VideoCardStateEntry::String(format!("{:?}", self.mode_control.pixel_clock_select()))));

        let (r, g, b) = VGACard::ega_to_rgb(self.overscan_color.six);
        attribute_vec.push((format!("{:?}", AttributeRegister::OverscanColor), VideoCardStateEntry::Color(format!("{:06b}", self.overscan_color.six), r, g, b)));

        attribute_vec.push((format!("{:?} [en]", AttributeRegister::ColorPlaneEnable), VideoCardStateEntry::String(format!("{:04b}", self.color_plane_enable.enable_plane()))));
        attribute_vec.push((format!("{:?} [mux]", AttributeRegister::ColorPlaneEnable), VideoCardStateEntry::String(format!("{:02b}", self.color_plane_enable.video_status_mux()))));
        attribute_vec.push((format!("{:?}", AttributeRegister::HorizontalPelPanning), VideoCardStateEntry::String(format!("{}", self.pel_panning))));

        attribute_vec
    }
}

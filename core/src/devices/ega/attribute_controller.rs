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

pub struct AttributeController {
    attribute_register_flipflop: AttributeRegisterFlipFlop,
    attribute_register_select_byte: u8,
    attribute_register_selected: AttributeRegister,
    pub attribute_palette_registers: [AttributePaletteEntry; 16],
    attribute_palette_index: usize,
    attribute_mode_control: AModeControl,
    pub attribute_overscan_color: AOverscanColor,
    attribute_color_plane_enable: AColorPlaneEnable,
    attribute_pel_panning: u8,
}

impl Default for AttributeController {
    fn default() -> Self {
        Self {
            attribute_register_flipflop: AttributeRegisterFlipFlop::Address,
            attribute_register_select_byte: 0,
            attribute_register_selected: AttributeRegister::Palette0,
            attribute_palette_registers: [Default::default(); 16],
            attribute_palette_index: 0,
            attribute_mode_control: AModeControl::new(),
            attribute_overscan_color: AOverscanColor::new(),
            attribute_color_plane_enable: AColorPlaneEnable::new(),
            attribute_pel_panning: 0,
        }
    }
}

impl AttributeController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset_flipflop(&mut self) {
        self.attribute_register_flipflop = AttributeRegisterFlipFlop::Address;
    }
    /// Handle a write to the Attribute Register 0x3C0.
    ///
    /// Unlike the other register files on the EGA, the Attribute Register doesn't have an
    /// address port. Instead, it maintains a flipflop that determines whether the port 0x3C0
    /// is in address or data mode. The flipflop is reset to a known state by reading 0x3DA.
    pub fn write_attribute_register(&mut self, byte: u8) {
        match self.attribute_register_flipflop {
            AttributeRegisterFlipFlop::Address => {
                if byte <= 0x0F {
                    self.attribute_palette_index = byte as usize;
                }
                self.attribute_register_selected = match byte & 0x1F {
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
                        self.attribute_register_selected
                    }
                };

                self.attribute_register_flipflop = AttributeRegisterFlipFlop::Data;
            }
            AttributeRegisterFlipFlop::Data => {
                match self.attribute_register_selected {
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
                        //self.attribute_palette_registers[self.attribute_palette_index] = APaletteRegister::from_bytes([byte]);
                        //log::debug!("set palette index {} to {:08b}", self.attribute_palette_index, byte );
                        self.attribute_palette_registers[self.attribute_palette_index].set(byte);
                    }
                    AttributeRegister::ModeControl => {
                        self.attribute_mode_control = AModeControl::from_bytes([byte]);
                    }
                    AttributeRegister::OverscanColor => {
                        self.attribute_overscan_color = AOverscanColor::from_bytes([byte]);
                    }
                    AttributeRegister::ColorPlaneEnable => {
                        self.attribute_color_plane_enable = AColorPlaneEnable::from_bytes([byte]);
                    }
                    AttributeRegister::HorizontalPelPanning => {
                        self.attribute_pel_panning = byte & 0x0F;
                    }
                }

                // IBM: "The flip flop toggles each time an OUT is issued to the Attribute Controller"
                self.attribute_register_flipflop = AttributeRegisterFlipFlop::Address;
            }
        }
    }

    pub fn mode(&self) -> AttributeMode {
        self.attribute_mode_control.mode()
    }

    pub fn display_type(&self) -> AttributeDisplayType {
        self.attribute_mode_control.display_type()
    }

    #[rustfmt::skip]
    pub fn get_state(&self) -> Vec<(String, VideoCardStateEntry)> {
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

        attribute_vec
    }
}

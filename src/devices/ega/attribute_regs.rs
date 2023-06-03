/*
    Marty PC Emulator 
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

    ---------------------------------------------------------------------------

    ega::attribute_regs.rs

    Implements the EGA attribute registers.

*/
use modular_bitfield::prelude::*;
use crate::devices::ega::EGACard;

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
    Data
}

#[derive(Debug, BitfieldSpecifier)]
pub enum AttributeMode {
    Text,
    Graphics
}

#[derive(Debug, BitfieldSpecifier)]
pub enum AttributeDisplayType {
    Color,
    Monochrome
}

#[derive(Debug, BitfieldSpecifier)]
pub enum AttributeBlinkOrIntensity {
    BackgroundIntensity,
    Blink
}

#[bitfield]
pub struct AttributeAddress {
    address: B5,
    address_source: B1,
    unused: B2
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
    unused: B2
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
    unused: B4
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
    unused: B2
}

#[bitfield]
pub struct AColorPlaneEnable {
    pub enable_plane: B4,
    pub video_status_mux: B2,
    #[skip]
    unused: B2
}

impl EGACard {
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
                    AttributeRegister::Palette0 | AttributeRegister::Palette1 | AttributeRegister::Palette2 |
                    AttributeRegister::Palette3 | AttributeRegister::Palette4 | AttributeRegister::Palette5 |
                    AttributeRegister::Palette6 | AttributeRegister::Palette7 | AttributeRegister::Palette8 |
                    AttributeRegister::Palette9 | AttributeRegister::PaletteA | AttributeRegister::PaletteB |
                    AttributeRegister::PaletteC | AttributeRegister::PaletteD | AttributeRegister::PaletteE |
                    AttributeRegister::PaletteF => {
                        //self.attribute_palette_registers[self.attribute_palette_index] = APaletteRegister::from_bytes([byte]);
                        self.attribute_palette_registers[self.attribute_palette_index] = byte;
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
        self.recalculate_mode();
    }    
}
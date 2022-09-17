/*
    vga_attribute_regs.rs

    Implement the Attribute Registers of the IBM VGA Card

*/
use modular_bitfield::prelude::*;
use crate::vga::VGACard;

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
    ModeControl,                // (10)
    OverscanColor,              // (11)
    ColorPlaneEnable,           // (12)
    HorizontalPelPanning,       // (13)
    ColorSelect                 // (14)
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

#[derive(Debug, BitfieldSpecifier)]
pub enum PaletteAddressSource {
    HostAccess,
    DisplayAccess
}

#[derive(Debug, BitfieldSpecifier)]
pub enum PixelClock {
    EveryCycle,
    EveryOtherCycle
}

#[derive(Debug, BitfieldSpecifier)]
pub enum PaletteSize {
    PaletteRegister45,
    ColorRegister45
}

#[bitfield]
pub struct AttributeAddress {
    pub address: B5,
    #[bits = 1]
    pub palette_address_source: PaletteAddressSource,
    #[skip]
    pub unused: B2
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
    pub unused: B2
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
    unused: B1,
    pub pixel_panning_compatibility: bool,
    #[bits = 1]
    pub pixel_clock_select: PixelClock,
    #[bits = 1]
    pub internal_palette_size: PaletteSize
}

#[bitfield]
#[derive (Copy, Clone)]
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
    pub video_status_mux: B2, // Unused on VGA
    #[skip]
    unused: B2
}

#[bitfield]
pub struct AColorSelect {
    pub c45: B2,
    pub c67: B2, // Unused on VGA
    #[skip]
    unused: B4
}

impl VGACard {
    /// Handle a write to the Attribute Register 0x3C0.
    /// 
    /// Unlike the other register files on the VGA, the Attribute Register doesn't have an 
    /// address port. Instead, it maintains a flipflop that determines whether the port 0x3C0
    /// is in address or data mode. The flipflop is reset to a known state by reading 0x3DA.
    pub fn write_attribute_register(&mut self, byte: u8) {

        match self.attribute_flipflop {
            AttributeRegisterFlipFlop::Address => {

                self.attribute_address = AttributeAddress::from_bytes([byte]);

                if self.attribute_address.address() <= 0x0F {
                    self.attribute_palette_index = self.attribute_address.address() as usize;
                }
                self.attribute_selected = match self.attribute_address.address() {
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
                    0x14 => AttributeRegister::ColorSelect,
                    _ => {
                        log::warn!("Invalid attribute register selected: {:02X}h", byte);
                        self.attribute_selected
                    }
                };

                self.attribute_flipflop = AttributeRegisterFlipFlop::Data;
            }
            AttributeRegisterFlipFlop::Data => {

                match self.attribute_selected {
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
                    AttributeRegister::ColorSelect => {
                        self.attribute_color_select = AColorSelect::from_bytes([byte]);
                    }
                }

                // IBM: "The flip flop toggles each time an OUT is issued to the Attribute Controller"
                self.attribute_flipflop = AttributeRegisterFlipFlop::Address;
            }
        }
        self.recalculate_mode();
    }
}
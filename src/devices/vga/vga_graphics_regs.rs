/*
    vga_graphics_regs.rs

    Implement the Graphics Registers of the IBM VGA Card

*/

use modular_bitfield::prelude::*;
use crate::vga::*;

#[derive(Copy, Clone, Debug)]
pub enum GraphicsRegister {
    SetReset,
    EnableSetReset,
    ColorCompare,
    DataRotate,
    ReadMapSelect,
    Mode,
    Miscellaneous,
    ColorDontCare,
    BitMask
}

#[bitfield]
pub struct GDataRotateRegister {
    pub count: B3,
    #[bits = 2]
    pub function: RotateFunction,
    #[skip]
    unused: B3
}

#[bitfield]
pub struct GModeRegister {
    #[bits = 2]
    pub write_mode: WriteMode,
    pub test_condition: bool,
    #[bits = 1]
    pub read_mode: ReadMode,
    pub odd_even: bool,
    #[bits = 2]
    pub shift_mode: ShiftMode,
    #[skip]
    unused: B1
}

#[bitfield]
pub struct GMiscellaneousRegister {
    pub graphics_mode: bool,    
    pub chain_odd_maps: bool,
    #[bits = 2]
    pub memory_map: MemoryMap,
    #[skip]
    unused: B4
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum MemoryMap {
    A0000_128k,
    A0000_64K,
    B0000_32K,
    B8000_32K
}

#[derive(Copy, Clone, Debug, PartialEq, BitfieldSpecifier)]
pub enum RotateFunction {
    Unmodified,
    And,
    Or,
    Xor
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum WriteMode {
    Mode0,
    Mode1,
    Mode2,
    Mode3,
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum ReadMode {
    ReadSelectedPlane,
    ReadComparedPlanes,
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum ShiftMode {
    Standard,
    CGACompatible,
    EightBits,
    Reserved
}

impl VGACard {
    /// Handle a write to one of the Graphics Position Registers.
    /// 
    /// According to IBM documentation, both these registers should be set to
    /// specific values, so we don't really do anything with them other than 
    /// log if we see an unexpected value written.
    pub fn write_graphics_position(&mut self, reg: u32, byte: u8) {

        match reg {
            1 => {
                if byte != 0 {
                    log::warn!("Non-zero value written to Graphics 1 Position register.")
                }
            }
            2 => {
                if byte != 1 {
                    log::warn!("Non-1 value written to Graphics 2 Position register.")
                }
            }
            _ => {}
        }
    }

    /// Handle a write to the Graphics Address Register
    pub fn write_graphics_address(&mut self, byte: u8) {
        self.graphics_register_address = byte & 0x0F;

        self.graphics_register_selected = match self.graphics_register_address {
            0x00 => GraphicsRegister::SetReset,
            0x01 => GraphicsRegister::EnableSetReset,
            0x02 => GraphicsRegister::ColorCompare,
            0x03 => GraphicsRegister::DataRotate,
            0x04 => GraphicsRegister::ReadMapSelect,
            0x05 => GraphicsRegister::Mode,
            0x06 => GraphicsRegister::Miscellaneous,
            0x07 => GraphicsRegister::ColorDontCare,
            0x08 => GraphicsRegister::BitMask,
            _ => self.graphics_register_selected
        };

        trace!(self, "Write to Graphics::Address: {:02X}", self.graphics_register_address);
    }

    pub fn write_graphics_data(&mut self, byte: u8 ) {
        match self.graphics_register_selected {
            GraphicsRegister::SetReset => {
                // Bits 0-3: Set/Reset Bits 0-3
                self.graphics_set_reset = byte & 0x0F;
                trace!(self, "Write to {:?}: sr: {:01X}",
                    self.graphics_register_selected,
                    self.graphics_set_reset
                );                  
            }
            GraphicsRegister::EnableSetReset => {
                // Value must be 1 to enable writing
                self.graphics_enable_set_reset = byte & 0x0F;
                trace!(self, "Write to {:?}: esr: {:01x}",
                    self.graphics_register_selected,
                    self.graphics_enable_set_reset
                );                  
            },
            GraphicsRegister::ColorCompare => {
                // Bits 0-3: Color Compare 0-3
                self.graphics_color_compare = byte & 0x0F;
                trace!(self, "Write to {:?}: cc: {:01x}",
                    self.graphics_register_selected,
                    self.graphics_color_compare
                );                  
            },
            GraphicsRegister::DataRotate => {
                // Bits 0-2: Rotate Count
                // Bits 3-4: Function Select
                self.graphics_data_rotate = GDataRotateRegister::from_bytes([byte]);

                if byte == 0xFF {
                    log::warn!("Invalid write to DataRotate register!");
                }

                trace!(self, "Write to {:?}:{:02X} rot: {:?} rop:{:?}",
                    self.graphics_register_selected,
                    byte,
                    self.graphics_data_rotate.count(),
                    self.graphics_data_rotate.function(),
                );                
            },
            GraphicsRegister::ReadMapSelect => {
                // Bits 0-2: Map Select 0-2
                self.graphics_read_map_select = byte & 0x03;
                trace!(self, "Write to {:?}: rms: {:?}",
                    self.graphics_register_selected,
                    self.graphics_read_map_select
                );                
            },

            GraphicsRegister::Mode => {
                // Bits 0-1: Write Mode
                // Bit 2: Test Condition
                // Bit 3: Read Mode
                // Bit 4: Odd/Even
                // Bit 5: Shift Register Mode
                self.graphics_mode = GModeRegister::from_bytes([byte]);
                trace!(self, "Write to {:?}: {:02X}",
                    self.graphics_register_selected,
                    byte
                );                  
            },
            GraphicsRegister::Miscellaneous => {
                self.graphics_micellaneous = GMiscellaneousRegister::from_bytes([byte]);

                trace!(self, "Write to {:?}: gm: {:?} com:{:?} mm:{:?}",
                    self.graphics_register_selected,
                    self.graphics_micellaneous.graphics_mode(),
                    self.graphics_micellaneous.chain_odd_maps(),
                    self.graphics_micellaneous.memory_map()
                );
            }
            GraphicsRegister::ColorDontCare => {
                // Bits 0-3: Color Don't Care

                trace!(self, "Write to {:?}: {:01X}",
                    self.graphics_register_selected,
                    self.graphics_color_dont_care
                );                
                self.graphics_color_dont_care = byte & 0x0F;
            },
            GraphicsRegister::BitMask => {
                // Bits 0-7: Bit Mask
                self.graphics_bitmask = byte;
                trace!(self, "Write to {:?}: {:01X}",
                    self.graphics_register_selected,
                    self.graphics_bitmask
                );                   
            },
        }
    }

    pub fn read_graphics_data(&self) -> u8 {
        match self.graphics_register_selected {
            GraphicsRegister::SetReset => {
                self.graphics_set_reset
            }
            GraphicsRegister::EnableSetReset => {
                self.graphics_enable_set_reset     
            },
            GraphicsRegister::ColorCompare => {
                self.graphics_color_compare
            },
            GraphicsRegister::DataRotate => {
                self.graphics_data_rotate.bytes[0]
            },
            GraphicsRegister::ReadMapSelect => {
                self.graphics_read_map_select
            },
            GraphicsRegister::Mode => {
                self.graphics_mode.bytes[0]
            },
            GraphicsRegister::Miscellaneous => {
                self.graphics_micellaneous.bytes[0]
            }
            GraphicsRegister::ColorDontCare => {             
                self.graphics_color_dont_care
            },
            GraphicsRegister::BitMask => {
                self.graphics_bitmask            
            },
        }        
    }
}
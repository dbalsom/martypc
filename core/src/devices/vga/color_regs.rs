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

    vga::color_regs.rs

    Implement the VGA Color registers.

*/

use crate::devices::vga::*;

pub const DAC_STATE_READ: u8 = 0;
pub const DAC_STATE_WRITE: u8 = 0x03;

impl VGACard {
    
    pub fn read_pel_data(&mut self) -> u8 {
        let byte;
        let color = self.color_pel_read_address as usize;
        let rgb_idx = self.color_pel_read_address_color as usize;

        byte = self.color_registers[color][rgb_idx];

        // Automatically increment to next color register, cycling through 
        // Red, Green and Blue registers per Read Index
        self.color_pel_read_address_color += 1;
        if self.color_pel_read_address_color == 3 {
            self.color_pel_read_address_color = 0;
            // Done with all colors, so go to next palette entry

            /*
                There's an apparent 'bug' in the IBM VGA BIOS palette register test, where 768 test
                values are written to the color registers. 
                These are then read back and tested, but the register address is not initalized to
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
            self.color_registers_rgba[color][0] = ((self.color_registers[color][0] as u32 * 255) / 63) as u8;
            self.color_registers_rgba[color][1] = ((self.color_registers[color][1] as u32 * 255) / 63) as u8;
            self.color_registers_rgba[color][2] = ((self.color_registers[color][2] as u32 * 255) / 63) as u8;
            self.color_registers_rgba[color][3] = 0xFF;

            trace!(self, "Wrote color register [{}] ({:02X},{:02X},{:02X})", 
                color,
                self.color_registers[color][0],
                self.color_registers[color][1],
                self.color_registers[color][2]);

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
}
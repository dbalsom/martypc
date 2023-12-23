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

    ega::mmio.rs

    Implement the EGA MMIO Interface

*/

use super::*;
use crate::bus::MemoryMappedDevice;

impl MemoryMappedDevice for EGACard {
    fn get_read_wait(&mut self, _address: usize, _cycles: u32) -> u32 {
        0
    }

    fn get_write_wait(&mut self, _address: usize, _cycles: u32) -> u32 {
        0
    }

    fn mmio_read_u8(&mut self, address: usize, _cycles: u32) -> (u8, u32) {
        // RAM Enable disables memory mapped IO
        if !self.misc_output_register.enable_ram() {
            return (0, 0);
        }

        // Validate address is within current memory map and get the offset
        let offset = match self.plane_bounds_check(address) {
            Some(offset) => offset,
            None => {
                return (0, 0);
            }
        };

        // Load all the latches regardless of selected plane
        for i in 0..4 {
            self.planes[i].latch = self.planes[i].buf[offset];
        }

        // Reads are controlled by the Read Mode bit in the Mode register of the Graphics Controller.
        match self.graphics_mode.read_mode() {
            ReadMode::ReadSelectedPlane => {
                // In Read Mode 0, the processor reads data from the memory plane selected
                // by the read map select register.
                let plane = (self.graphics_read_map_select & 0x03) as usize;
                let byte = self.planes[plane].buf[offset];
                return (byte, 0);
            }
            ReadMode::ReadComparedPlanes => {
                // In Read Mode 1, the processor reads the result of a comparison with the value in the
                // Color Compare register, from the set of enabled planes in the Color Dont Care register
                self.get_pixels(offset);
                let comparison = self.pixel_op_compare();
                return (comparison, 0);
            }
        }
    }

    fn mmio_read_u16(&mut self, address: usize, cycles: u32) -> (u16, u32) {
        let (lo_byte, wait1) = MemoryMappedDevice::mmio_read_u8(self, address, cycles);
        let (ho_byte, wait2) = MemoryMappedDevice::mmio_read_u8(self, address + 1, cycles);

        //log::warn!("Unsupported 16 bit read from VRAM");
        ((ho_byte as u16) << 8 | lo_byte as u16, wait1 + wait2)
    }

    fn mmio_peek_u8(&self, address: usize) -> u8 {
        // RAM Enable disables memory mapped IO
        if !self.misc_output_register.enable_ram() {
            return 0;
        }

        // Validate address is within current memory map and get the offset into VRAM
        let offset = match self.plane_bounds_check(address) {
            Some(offset) => offset,
            None => return 0,
        };

        self.planes[0].buf[offset]
    }

    fn mmio_peek_u16(&self, address: usize) -> u16 {
        // RAM Enable disables memory mapped IO
        if !self.misc_output_register.enable_ram() {
            return 0;
        }

        // Validate address is within current memory map and get the offset into VRAM
        let offset = match self.plane_bounds_check(address) {
            Some(offset) => offset,
            None => return 0,
        };

        (self.planes[0].buf[offset] as u16) << 8 | self.planes[0].buf[offset + 1] as u16
    }

    fn mmio_write_u8(&mut self, address: usize, byte: u8, _cycles: u32) -> u32 {
        // RAM Enable disables memory mapped IO
        if !self.misc_output_register.enable_ram() {
            return 0;
        }

        // Validate address is within current memory map and get the offset
        let offset = match self.plane_bounds_check(address) {
            Some(offset) => offset,
            None => return 0,
        };

        match self.graphics_mode.write_mode() {
            WriteMode::Mode0 => {
                // Write mode 0 performs a pipeline of operations:
                // First, data is rotated as specified by the Rotate Count field of the Data Rotate Register.
                let data_rot = EGACard::rotate_right_u8(byte, self.graphics_data_rotate.count());

                // Second, data is is either passed through to the next stage or replaced by a value determined
                // by the Set/Reset register. The bits in the Enable Set/Reset register controls whether this occurs.
                for i in 0..4 {
                    if self.graphics_enable_set_reset & (0x01 << i) != 0 {
                        // If the Set/Reset Enable bit is set, use expansion of corresponding Set/Reset register bit
                        self.pipeline_buf[i] = match self.graphics_set_reset & (0x01 << i) != 0 {
                            true => 0xFF,
                            false => 0x00,
                        }
                    }
                    else {
                        // Set/Reset Enable bit not set, use data from rotate step
                        self.pipeline_buf[i] = data_rot
                    }
                }

                // Third, the operation specified by the Logical Operation field of the Data Rotate register
                // is performed on the data for each plane and the latch read register.
                // A 1 bit in the Graphics Bit Mask register will use the bit result of the Logical Operation.
                // A 0 bit in the Graphics Bit Mask register will use the bit unchanged from the Read Latch register.
                for i in 0..4 {
                    self.pipeline_buf[i] = match self.graphics_data_rotate.function() {
                        RotateFunction::Unmodified => {
                            // Clear masked bits from pipeline, set them with mask bits from latch
                            (self.pipeline_buf[i] & self.graphics_bitmask)
                                | (!self.graphics_bitmask & self.planes[i].latch)
                        }
                        RotateFunction::And => (self.pipeline_buf[i] | !self.graphics_bitmask) & self.planes[i].latch,
                        RotateFunction::Or => (self.pipeline_buf[i] & self.graphics_bitmask) | self.planes[i].latch,
                        RotateFunction::Xor => (self.pipeline_buf[i] & self.graphics_bitmask) ^ self.planes[i].latch,
                    }
                }
                // Fourth, the value of the Bit Mask register is used: A set bit in the Mask register will pass
                // the bit from the data pipeline, a 0 bit will pass a bit from the read latch register.
                //for i in 0..4 {
                //
                //    self.write_buf[i] = 0;
                //
                //    for k in 0..8 {
                //        if self.graphics_bitmask & (0x01 << k) != 0 {
                //            // If a bit is set in the mask register, pass the bit from the previous stage
                //            self.write_buf[i] |= self.pipeline_buf[i] & (0x01 << k);
                //        }
                //        else {
                //            // Otherwise, pass the corresponding bit from the read latch register
                //            self.write_buf[i] |= self.planes[i].latch & (0x01 << k);
                //        }
                //    }
                //}

                // Finally, write data to the planes enabled in the Memory Plane Write Enable field of
                // the Sequencer Map Mask register.
                for i in 0..4 {
                    if self.sequencer_map_mask & (0x01 << i) != 0 {
                        self.plane_set(i, offset, self.pipeline_buf[i]);
                        //self.planes[i].buf[offset] = self.pipeline_buf[i];
                    }
                }
            }
            WriteMode::Mode1 => {
                // Write the contents of the latches to their corresponding planes. This assumes that the latches
                // were loaded propery via a previous read operation.

                for i in 0..4 {
                    // Only write to planes enabled in the Sequencer Map Mask.
                    if self.sequencer_map_mask & (0x01 << i) != 0 {
                        self.plane_set(i, offset, self.planes[i].latch);
                        //self.planes[i].buf[offset] = self.planes[i].latch;
                    }
                }
            }
            WriteMode::Mode2 => {
                for i in 0..4 {
                    // Only write to planes enabled in the Sequencer Map Mask.
                    if self.sequencer_map_mask & (0x01 << i) != 0 {
                        // Extend the bit for this plane to 8 bits.
                        let bit_span: u8 = match byte & (0x01 << i) != 0 {
                            true => 0xFF,
                            false => 0x00,
                        };

                        // Clear bits not masked
                        self.plane_and(i, offset, !self.graphics_bitmask);
                        //self.planes[i].buf[offset] &= !self.graphics_bitmask;

                        // Mask off bits not to set
                        let set_bits = bit_span & self.graphics_bitmask;

                        self.plane_or(i, offset, set_bits);
                        //self.planes[i].buf[offset] |= set_bits;
                    }
                }

                //log::warn!("Unimplemented write mode 2")
            }
            WriteMode::Invalid => {
                log::warn!("Invalid write mode!");
                return 0;
            }
        }

        0
    }

    fn mmio_write_u16(&mut self, _address: usize, _data: u16, _cycles: u32) -> u32 {
        log::warn!("Unsupported 16 bit write to VRAM");
        0
    }
}

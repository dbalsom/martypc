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

    ega::graphics_controller.rs

    Implement the EGA Graphics Controllers. Although there are two physical LSI
    chips on the IBM EGA, we treat them as one functional unit here.

*/

use super::*;

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
    BitMask,
}

#[allow(dead_code)]
#[bitfield]
pub struct GDataRotateRegister {
    pub count: B3,
    #[bits = 2]
    pub function: LogicFunction,
    #[skip]
    unused: B3,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct GModeRegister {
    #[bits = 2]
    pub write_mode: WriteMode,
    pub test_condition: bool,
    #[bits = 1]
    pub read_mode: ReadMode,
    pub odd_even: OddEvenModeComplement,
    #[bits = 1]
    pub shift_mode: ShiftMode,
    #[skip]
    unused: B2,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct GMiscellaneousRegister {
    pub graphics_mode: bool,
    pub chain_odd_even: bool,
    pub memory_map: MemoryMap,
    #[skip]
    pub unused: B4,
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum OddEvenModeComplement {
    Sequential,
    OddEven,
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum MemoryMap {
    A0000_128k,
    A0000_64K,
    B0000_32K,
    B8000_32K,
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum LogicFunction {
    Unmodified,
    And,
    Or,
    Xor,
}

#[derive(Copy, Clone, Debug, BitfieldSpecifier)]
pub enum WriteMode {
    Mode0,
    Mode1,
    Mode2,
    Invalid,
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
}

pub struct GraphicsController {
    graphics_register_select_byte: u8,
    graphics_register_selected: GraphicsRegister,
    graphics_set_reset: u8,
    graphics_enable_set_reset: u8,
    graphics_color_compare: u8,
    graphics_data_rotate: GDataRotateRegister,
    graphics_data_rotate_function: LogicFunction,
    graphics_read_map_select: u8,
    graphics_mode: GModeRegister,
    graphics_micellaneous: GMiscellaneousRegister,
    graphics_color_dont_care: u8,
    graphics_bitmask: u8,

    latches: [u8; 4],

    pixel_buf: [u8; 8],
    pipeline_buf: [u8; 4],
    serialize_buf: [u8; 8],
}

//pub const TEST_SEQUENCE: [u8; 8] = [0x01, 0x02, 0x01, 0x02, 0x01, 0x03, 0x01, 0x03];

impl Default for GraphicsController {
    fn default() -> Self {
        Self {
            graphics_register_select_byte: 0,
            graphics_register_selected: GraphicsRegister::SetReset,
            graphics_set_reset: 0,
            graphics_enable_set_reset: 0,
            graphics_color_compare: 0,
            graphics_data_rotate: GDataRotateRegister::new(),
            graphics_data_rotate_function: LogicFunction::Unmodified,
            graphics_read_map_select: 0,
            graphics_mode: GModeRegister::new(),
            graphics_micellaneous: GMiscellaneousRegister::new(),
            graphics_color_dont_care: 0,
            graphics_bitmask: 0,

            latches: [0; 4],

            pixel_buf: [0; 8],
            pipeline_buf: [0; 4],
            serialize_buf: [0; 8],
        }
    }
}

impl GraphicsController {
    pub fn new() -> Self {
        GraphicsController::default()
    }

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

    #[inline]
    pub fn chain(&self) -> bool {
        self.graphics_micellaneous.chain_odd_even()
    }

    /// Handle a write to the Graphics Address Register
    pub fn write_graphics_address(&mut self, byte: u8) {
        self.graphics_register_select_byte = byte & 0x0F;

        self.graphics_register_selected = match self.graphics_register_select_byte {
            0x00 => GraphicsRegister::SetReset,
            0x01 => GraphicsRegister::EnableSetReset,
            0x02 => GraphicsRegister::ColorCompare,
            0x03 => GraphicsRegister::DataRotate,
            0x04 => GraphicsRegister::ReadMapSelect,
            0x05 => GraphicsRegister::Mode,
            0x06 => GraphicsRegister::Miscellaneous,
            0x07 => GraphicsRegister::ColorDontCare,
            0x08 => GraphicsRegister::BitMask,
            _ => self.graphics_register_selected,
        }
    }

    pub fn write_graphics_data(&mut self, byte: u8) {
        match self.graphics_register_selected {
            GraphicsRegister::SetReset => {
                // Bits 0-3: Set/Reset Bits 0-3
                self.graphics_set_reset = byte & 0x0F;
            }
            GraphicsRegister::EnableSetReset => {
                // Bits 0-3: Enable Set/Reset Bits 0-3
                self.graphics_enable_set_reset = byte & 0x0F;
            }
            GraphicsRegister::ColorCompare => {
                // Bits 0-3: Color Compare 0-3
                self.graphics_color_compare = byte & 0x0F;
            }
            GraphicsRegister::DataRotate => {
                // Bits 0-2: Rotate Count
                // Bits 3-4: Function Select
                self.graphics_data_rotate = GDataRotateRegister::from_bytes([byte]);
            }
            GraphicsRegister::ReadMapSelect => {
                // Bits 0-2: Map Select 0-2
                self.graphics_read_map_select = byte & 0x03;
            }
            GraphicsRegister::Mode => {
                // Bits 0-1: Write Mode
                // Bit 2: Test Condition
                // Bit 3: Read Mode
                // Bit 4: Odd/Even
                // Bit 5: Shift Register Mode
                self.graphics_mode = GModeRegister::from_bytes([byte]);
            }
            GraphicsRegister::Miscellaneous => {
                self.graphics_micellaneous = GMiscellaneousRegister::from_bytes([byte]);
            }
            GraphicsRegister::ColorDontCare => {
                // Bits 0-3: Color Don't Care
                self.graphics_color_dont_care = byte & 0x0F;
            }
            GraphicsRegister::BitMask => {
                // Bits 0-7: Bit Mask
                self.graphics_bitmask = byte;
            }
        }
    }

    /// Implement the serializer output of the Graphics Controller, for graphics modes.
    /// Unlike CPU reads, this does not set the latches, however it performs address manipulation
    /// and allows for processing such as CGA compatibility shifting.
    pub fn serialize<'a>(&'a mut self, seq: &'a Sequencer, address: usize) -> &'a [u8] {
        let offset = address;

        if let ShiftMode::CGACompatible = self.graphics_mode.shift_mode() {
            // CGA compatible mode. 2bpp linear pixels are unpacked across two bytes
            let mut byte = seq.read_u8(0, offset, address & 0x01);
            for i in 0..4 {
                // Mask and extract each sequence of two bits, in left-to-right order
                self.serialize_buf[i] = (byte & (0xC0 >> (i * 2))) >> (6 - i * 2);
            }
            byte = seq.read_u8(1, offset + 1, 1);
            for i in 0..4 {
                self.serialize_buf[i + 4] = (byte & (0xC0 >> (i * 2))) >> (6 - i * 2);
            }
            //&TEST_SEQUENCE
            &self.serialize_buf
        }
        else {
            // Normal EGA mode
            seq.serialize_linear(offset)
        }
    }

    pub fn parallel<'a>(&'a mut self, seq: &'a Sequencer, address: usize, row: u8) -> (&'a [u8], u8) {
        let glyph = seq.read_u8(0, address, address & 0x01);
        let attr = seq.read_u8(1, address + 1, (address + 1) & 0x01);
        let glyph_span_addr = seq.get_glyph_address(glyph, 0, row);
        let glyph_span = seq.vram.read_u8(2, glyph_span_addr);
        let glyph_unpacked = &BYTE_EXTEND_TABLE[glyph_span as usize];
        (glyph_unpacked, attr)
    }

    /// Implement a read of the Graphics Controller via the CPU. This sets the latches, performs
    /// address manipulation, and executes the pixel pipeline.
    pub fn cpu_read_u8(&mut self, seq: &Sequencer, address: usize, page_select: PageSelect) -> u8 {
        // Validate address is within current memory map and get the offset
        let (offset, a0) = match self.map_address(address, page_select) {
            Some((offset, a0)) => (offset, a0),
            None => {
                return 0;
            }
        };

        /*        if self.graphics_mode.odd_even() {
            //offset >>= 1;
        }*/

        // Load all the latches regardless of selected plane
        for i in 0..4 {
            self.latches[i] = seq.read_u8(i, offset, a0);
        }

        // Reads are controlled by the Read Mode bit in the Mode register of the Graphics Controller.
        match self.graphics_mode.read_mode() {
            ReadMode::ReadSelectedPlane => {
                // Read Mode 0
                // In Sequential mode, the processor reads data from the memory plane selected
                // by the read map select register.
                // In Odd/Even mode, the memory plane is chosen by using the read map select to determine which
                // graphics controller to use, and the address bit 0 to determine which plane to use.
                match self.graphics_mode.odd_even() {
                    OddEvenModeComplement::Sequential => {
                        let plane = self.graphics_read_map_select as usize;

                        seq.read_u8(plane, offset, a0)
                    }
                    OddEvenModeComplement::OddEven => {
                        // If selected plane is 0 or 1, choose 0 or 1 based on a0.
                        // If selected plane is 2 or 3, choose 2 or 3 based on a0.
                        let plane = (self.graphics_read_map_select as usize & !0x01) | a0;

                        seq.read_u8(plane, offset, a0)
                    }
                }
            }
            ReadMode::ReadComparedPlanes => {
                // In Read Mode 1, the processor reads the result of a comparison with the value in the
                // Color Compare register, from the set of enabled planes in the Color Don't Care register
                self.get_pixels(seq, offset);

                self.pixel_op_compare()
            }
        }
    }

    /// Perform a read via the Graphics Controller, allowing for address manipulation, but no side effects such as
    /// setting latches.
    pub fn cpu_peek_u8(&self, seq: &Sequencer, address: usize, page_select: PageSelect) -> u8 {
        // Validate address is within current memory map and get the offset
        let (offset, a0) = match self.map_address(address, page_select) {
            Some((offset, a0)) => (offset, a0),
            None => {
                return 0;
            }
        };

        if let OddEvenModeComplement::OddEven = self.graphics_mode.odd_even() {
            //offset >>= 1;
        }

        seq.read_u8(0, offset, a0)
    }

    pub fn cpu_write_u8(&mut self, seq: &mut Sequencer, address: usize, page_select: PageSelect, byte: u8) {
        // Validate address is within current memory map and get the offset
        let (offset, a0) = match self.map_address(address, page_select) {
            Some((offset, a0)) => (offset, a0),
            None => return,
        };

        match self.graphics_mode.write_mode() {
            WriteMode::Mode0 => {
                // Write mode 0 performs a pipeline of operations:
                // First, data is rotated as specified by the Rotate Count field of the Data Rotate Register.
                let data_rot = EGACard::rotate_right_u8(byte, self.graphics_data_rotate.count());

                // Second, data is either passed through to the next stage or replaced by a value determined
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
                    self.apply_logic_fn(i);
                }

                // Finally, write data to the planes enabled in the Memory Plane Write Enable field of
                // the Sequencer Map Mask register.

                self.foreach_plane(seq, a0, |gc, seq, plane| {
                    seq.plane_set(plane, offset, a0, gc.pipeline_buf[plane]);
                });
            }
            WriteMode::Mode1 => {
                // Write the contents of the latches to their corresponding planes. This assumes that the latches
                // were loaded properly via a previous read operation.
                self.foreach_plane(seq, a0, |gc, seq, plane| {
                    seq.plane_set(plane, offset, a0, gc.latches[plane]);
                });
            }
            WriteMode::Mode2 => {
                self.foreach_plane(seq, a0, |gc, seq, plane| {
                    // Extend the bit for this plane to 8 bits.
                    gc.pipeline_buf[plane] = match byte & (0x01 << plane) != 0 {
                        true => 0xFF,
                        false => 0x00,
                    };
                    gc.apply_logic_fn(plane);
                    seq.plane_set(plane, offset, a0, gc.pipeline_buf[plane]);
                });
            }
            WriteMode::Invalid => {
                log::warn!("Invalid write mode!");
            }
        }
    }

    #[inline]
    fn apply_logic_fn(&mut self, p: usize) {
        self.pipeline_buf[p] = match self.graphics_data_rotate.function() {
            LogicFunction::Unmodified => {
                // Clear masked bits from pipeline, set them with mask bits from latch
                (self.pipeline_buf[p] & self.graphics_bitmask) | (!self.graphics_bitmask & self.latches[p])
            }
            LogicFunction::And => (self.pipeline_buf[p] | !self.graphics_bitmask) & self.latches[p],
            LogicFunction::Or => (self.pipeline_buf[p] & self.graphics_bitmask) | self.latches[p],
            LogicFunction::Xor => (self.pipeline_buf[p] & self.graphics_bitmask) ^ self.latches[p],
        }
    }

    #[inline]
    fn foreach_plane<F>(&mut self, seq: &mut Sequencer, a0: usize, mut f: F)
    where
        F: FnMut(&mut GraphicsController, &mut Sequencer, usize),
    {
        match self.graphics_mode.odd_even() {
            OddEvenModeComplement::Sequential => {
                for plane in 0..4 {
                    f(self, seq, plane);
                }
            }
            OddEvenModeComplement::OddEven => {
                f(self, seq, a0);
                f(self, seq, 2 + a0);
            }
        }
    }

    /// Fill a slice of 8 elements with the 4bpp pixel values at the specified memory
    /// address.
    fn get_pixels(&mut self, seq: &Sequencer, addr: usize) {
        for p in 0..8 {
            self.pixel_buf[p] |= seq.vram.read_u8(0, addr) >> (7 - p) & 0x01;
            self.pixel_buf[p] |= (seq.vram.read_u8(1, addr) >> (7 - p) & 0x01) << 1;
            self.pixel_buf[p] |= (seq.vram.read_u8(2, addr) >> (7 - p) & 0x01) << 2;
            self.pixel_buf[p] |= (seq.vram.read_u8(3, addr) >> (7 - p) & 0x01) << 3;
        }
    }

    /// Compare the pixels in pixel_buf with the Color Compare and Color Don't Care registers.
    fn pixel_op_compare(&self) -> u8 {
        let cmp = self.graphics_color_compare & self.graphics_color_dont_care;
        let dc = self.graphics_color_dont_care;
        let mut mask = 0xFF;

        for plane in 0..4 {
            if (dc >> plane) & 1 != 0 {
                let bit = (cmp >> plane) & 1;
                // avoid multiply by 0xFF
                let cmpbit = 0u8.wrapping_sub(bit);
                let plane_match = !(self.latches[plane] ^ cmpbit);
                mask &= plane_match;
            }
        }

        mask
    }

    pub fn map_address(&self, address: usize, page_select: PageSelect) -> Option<(usize, usize)> {
        let offset;
        match self.graphics_micellaneous.memory_map() {
            MemoryMap::A0000_128k => {
                if let EGA_MEM_ADDRESS..=EGA_MEM_END_128 = address {
                    // 128k aperture is usually used with chain odd/even mode.
                    if self.graphics_micellaneous.chain_odd_even() {
                        if address > 0xFFFF {
                            // Replace bit 0 with bit 16
                            offset = (address & !1) | (((address & 0x10000) >> 16) & 1);
                        }
                        else {
                            // Replace bit 0 with bit 14
                            offset = (address & !1) | (((address & 0x04000) >> 14) & 1);
                        }
                    }
                    else {
                        // Not sure what to do in this case if we're out of bounds of a 64k plane.
                        // So just mask it to 64k for now.
                        offset = address & 0xFFFF;
                    }
                }
                else {
                    return None;
                }
            }
            MemoryMap::A0000_64K => {
                if let EGA_MEM_ADDRESS..=EGA_MEM_END_64 = address {
                    if self.graphics_micellaneous.chain_odd_even() {
                        // Replace bit 0 with the page select bit
                        offset = (address & !1) | page_select as usize;
                    }
                    else {
                        offset = address - EGA_MEM_ADDRESS;
                    }
                }
                else {
                    return None;
                }
            }
            MemoryMap::B8000_32K => {
                if let CGA_MEM_ADDRESS..=CGA_MEM_END = address {
                    offset = address - CGA_MEM_ADDRESS;
                }
                else {
                    return None;
                }
            }
            _ => return None,
        }

        Some((offset & 0xFFFF, address & 1))
    }

    pub(crate) fn memory_map(&self) -> MemoryMap {
        self.graphics_micellaneous.memory_map()
    }

    pub(crate) fn odd_even(&self) -> OddEvenModeComplement {
        self.graphics_mode.odd_even()
    }

    #[inline]
    pub(crate) fn shift_mode(&self) -> ShiftMode {
        self.graphics_mode.shift_mode()
    }

    #[rustfmt::skip]
    pub fn get_state(&self) -> Vec<(String, VideoCardStateEntry)> {
        let mut graphics_vec = Vec::new();
        graphics_vec.push((format!("{:?}", GraphicsRegister::SetReset), VideoCardStateEntry::String(format!("{:04b}", self.graphics_set_reset))));
        graphics_vec.push((format!("{:?}", GraphicsRegister::EnableSetReset), VideoCardStateEntry::String(format!("{:04b}", self.graphics_enable_set_reset))));
        graphics_vec.push((format!("{:?}", GraphicsRegister::ColorCompare), VideoCardStateEntry::String(format!("{:04b}", self.graphics_color_compare))));
        graphics_vec.push((format!("{:?} [fn]", GraphicsRegister::DataRotate), VideoCardStateEntry::String(format!("{:?}", self.graphics_data_rotate.function()))));
        graphics_vec.push((format!("{:?} [ct]", GraphicsRegister::DataRotate), VideoCardStateEntry::String(format!("{:?}", self.graphics_data_rotate.count()))));
        graphics_vec.push((format!("{:?}", GraphicsRegister::ReadMapSelect), VideoCardStateEntry::String(format!("{:03b}", self.graphics_read_map_select))));

        graphics_vec.push((format!("{:?}", GraphicsRegister::Mode), VideoCardStateEntry::String(format!("{:06b}", self.graphics_mode.into_bytes()[0]))));
        graphics_vec.push((format!("{:?} [sr]", GraphicsRegister::Mode), VideoCardStateEntry::String(format!("{:?}", self.graphics_mode.shift_mode()))));
        graphics_vec.push((format!("{:?} [o/e]", GraphicsRegister::Mode), VideoCardStateEntry::String(format!("{:?}", self.graphics_mode.odd_even()))));
        graphics_vec.push((format!("{:?} [rm]", GraphicsRegister::Mode), VideoCardStateEntry::String(format!("{:?}",self.graphics_mode.read_mode()))));
        graphics_vec.push((format!("{:?} [tc]", GraphicsRegister::Mode), VideoCardStateEntry::String(format!("{:?}", self.graphics_mode.test_condition()))));
        graphics_vec.push((format!("{:?} [wm]", GraphicsRegister::Mode), VideoCardStateEntry::String(format!("{:?}", self.graphics_mode.write_mode()))));

        graphics_vec.push((format!("{:?} [gm]", GraphicsRegister::Miscellaneous), VideoCardStateEntry::String(format!("{:?}", self.graphics_micellaneous.graphics_mode()))));
        graphics_vec.push((format!("{:?} [coe]", GraphicsRegister::Miscellaneous), VideoCardStateEntry::String(format!("{:?}", self.graphics_micellaneous.chain_odd_even()))));
        graphics_vec.push((format!("{:?} [mm]", GraphicsRegister::Miscellaneous), VideoCardStateEntry::String(format!("{:?}", self.graphics_micellaneous.memory_map()))));

        graphics_vec.push((format!("{:?}", GraphicsRegister::ColorDontCare), VideoCardStateEntry::String(format!("{:04b}", self.graphics_color_dont_care))));
        graphics_vec.push((format!("{:?}", GraphicsRegister::BitMask), VideoCardStateEntry::String(format!("{:08b}", self.graphics_bitmask))));

        graphics_vec
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exact translation of your original, per-bit loop implementation.
    fn naive_pixel_compare(latches: [u8; 4], graphics_color_compare: u8, graphics_color_dont_care: u8) -> u8 {
        let mut comparison = 0u8;
        for i in 0..8 {
            let mut plane_comp = 0;
            if latches[0] & (1 << i) != 0 {
                plane_comp |= 0x01;
            }
            if latches[1] & (1 << i) != 0 {
                plane_comp |= 0x02;
            }
            if latches[2] & (1 << i) != 0 {
                plane_comp |= 0x04;
            }
            if latches[3] & (1 << i) != 0 {
                plane_comp |= 0x08;
            }

            let masked_cmp = graphics_color_compare & graphics_color_dont_care;
            if (plane_comp & graphics_color_dont_care) == masked_cmp {
                comparison |= 1 << i;
            }
        }
        comparison
    }

    #[test]
    fn pixel_op_compare_equivalence() {
        // A small set of interesting latch‐patterns:
        let samples = [
            [0x00, 0x00, 0x00, 0x00],
            [0xFF, 0xFF, 0xFF, 0xFF],
            [0b1010_1010, 0b0101_0101, 0b1111_0000, 0b0000_1111],
            [0x12, 0x34, 0x56, 0x78],
        ];

        for &latches in &samples {
            for cc in 0..=0x0F {
                for dc in 0..=0x0F {
                    // Build a VGA state with these registers:
                    let mut ega = EGACard::default();
                    ega.gc.latches = latches;
                    ega.gc.graphics_color_compare = cc;
                    ega.gc.graphics_color_dont_care = dc;

                    let fast = ega.gc.pixel_op_compare();
                    let slow = naive_pixel_compare(latches, cc, dc);

                    assert_eq!(
                        fast, slow,
                        "Mismatch: latches={:?}, compare=0x{:X}, dont_care=0x{:X} => fast=0x{:02X}, naive=0x{:02X}",
                        latches, cc, dc, fast, slow
                    );
                }
            }
        }
    }
}

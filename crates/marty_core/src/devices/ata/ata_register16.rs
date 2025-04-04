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

    --------------------------------------------------------------------------
*/

//! An implementation of a 16-bit ATA register for 8-bit devices.

#[derive(Default, Debug)]
pub struct AtaRegister16 {
    pub bytes: [Option<u8>; 2],
}

impl AtaRegister16 {
    pub fn new() -> Self {
        Self::default()
    }
    /// Set the entire register to a 16-bit value.
    #[inline]
    pub fn set_16(&mut self, value: u16) {
        self.bytes[0] = Some((value & 0xFF) as u8);
        self.bytes[1] = Some((value >> 8) as u8);
    }
    /// Set the high byte of the register.
    #[inline]
    pub fn set_hi(&mut self, byte: u8) {
        self.bytes[1] = Some(byte);
    }
    /// Set the low byte of the register.
    #[inline]
    pub fn set_lo(&mut self, byte: u8) {
        self.bytes[0] = Some(byte);
    }
    /// Return a bool representing whether the register has been fully written to.
    #[inline]
    pub fn ready(&self) -> bool {
        self.bytes[0].is_some() && self.bytes[1].is_some()
    }
    /// If a full value has been written, return Some(value) and clear the register. Otherwise, return None.
    #[inline]
    pub fn get(&mut self) -> Option<u16> {
        if self.ready() {
            let value = Some(u16::from_le_bytes([self.bytes[0].unwrap(), self.bytes[1].unwrap()]));
            *self = Self::default();
            value
        }
        else {
            None
        }
    }
    #[inline]
    pub fn get_bytes(&mut self) -> Option<[u8; 2]> {
        if self.ready() {
            let bytes = [self.bytes[0].unwrap(), self.bytes[1].unwrap()];
            *self = Self::default();
            Some(bytes)
        }
        else {
            None
        }
    }
}

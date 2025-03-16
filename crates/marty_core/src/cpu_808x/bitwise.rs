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

    cpu_808x::bitwise.rs

    Implement bitwise operations (Shifts, rotations)

*/

use crate::{
    cpu_808x::*,
    cpu_common::{
        alu::{
            AluRotateCarryLeft,
            AluRotateCarryRight,
            AluRotateLeft,
            AluRotateRight,
            AluShiftArithmeticRight,
            AluShiftLeftAf,
            AluShiftRight,
        },
        Mnemonic,
    },
};

impl Intel808x {
    /*    pub(crate) fn shl_u8_with_carry(mut byte: u8, mut count: u8) -> (u8, bool) {
        let mut carry = false;
        while count > 0 {
            carry = byte & 0x80 != 0;
            byte <<= 1;
            count -= 1;
        }
        (byte, carry)
    }

    pub(crate) fn shl_u16_with_carry(mut word: u16, mut count: u8) -> (u16, bool) {
        let mut carry = false;
        while count > 0 {
            carry = word & 0x8000 != 0;
            word <<= 1;
            count -= 1;
        }
        (word, carry)
    }

    pub(crate) fn shr_u8_with_carry(mut byte: u8, mut count: u8) -> (u8, bool) {
        let mut carry = false;
        while count > 0 {
            carry = byte & 0x01 != 0;
            byte >>= 1;
            count -= 1;
        }
        (byte, carry)
    }

    pub(crate) fn shr_u16_with_carry(mut word: u16, mut count: u8) -> (u16, bool) {
        let mut carry = false;
        while count > 0 {
            carry = word & 0x0001 != 0;
            word >>= 1;
            count -= 1;
        }
        (word, carry)
    }

    pub(crate) fn rcr_u8_with_carry(mut byte: u8, mut count: u8, carry_flag: bool) -> (u8, bool) {
        let mut saved_carry = carry_flag;
        let mut new_carry;

        while count > 0 {
            new_carry = byte & 0x01 != 0;
            byte >>= 1;
            if saved_carry {
                byte |= 0x80;
            }
            saved_carry = new_carry;
            count -= 1;
        }

        (byte, saved_carry)
    }

    pub(crate) fn rcr_u16_with_carry(mut word: u16, mut count: u8, carry_flag: bool) -> (u16, bool) {
        let mut saved_carry = carry_flag;
        let mut new_carry;

        while count > 0 {
            new_carry = word & 0x0001 != 0;
            word >>= 1;
            if saved_carry {
                word |= 0x8000;
            }
            saved_carry = new_carry;
            count -= 1;
        }

        (word, saved_carry)
    }

    pub(crate) fn rcl_u8_with_carry(mut byte: u8, mut count: u8, carry_flag: bool) -> (u8, bool) {
        let mut saved_carry = carry_flag;
        let mut new_carry;

        while count > 0 {
            new_carry = byte & 0x80 != 0;
            byte <<= 1;
            if saved_carry {
                byte |= 0x01;
            }
            saved_carry = new_carry;
            count -= 1;
        }

        (byte, saved_carry)
    }

    pub(crate) fn rcl_u16_with_carry(mut word: u16, mut count: u8, carry_flag: bool) -> (u16, bool) {
        let mut saved_carry = carry_flag;
        let mut new_carry;

        while count > 0 {
            new_carry = word & 0x8000 != 0;
            word <<= 1;
            if saved_carry {
                word |= 0x0001;
            }
            saved_carry = new_carry;
            count -= 1;
        }

        (word, saved_carry)
    }

    pub(crate) fn ror_u8_with_carry(mut byte: u8, mut count: u8) -> (u8, bool) {
        let mut carry = false;
        while count > 0 {
            carry = byte & 0x01 != 0;
            byte >>= 1;
            if carry {
                byte |= 0x80;
            }
            count -= 1;
        }
        (byte, carry)
    }

    pub(crate) fn ror_u16_with_carry(mut word: u16, mut count: u8) -> (u16, bool) {
        let mut carry = false;
        while count > 0 {
            carry = word & 0x0001 != 0;
            word >>= 1;
            if carry {
                word |= 0x8000;
            }
            count -= 1;
        }
        (word, carry)
    }

    pub(crate) fn rol_u8_with_carry(mut byte: u8, mut count: u8) -> (u8, bool) {
        let mut carry = false;
        while count > 0 {
            carry = byte & 0x80 != 0;
            byte <<= 1;
            if carry {
                byte |= 0x01;
            }
            count -= 1;
        }

        (byte, carry)
    }

    pub(crate) fn rol_u16_with_carry(mut word: u16, mut count: u8) -> (u16, bool) {
        let mut carry = false;
        while count > 0 {
            carry = word & 0x8000 != 0;
            word <<= 1;
            if carry {
                word |= 0x0001;
            }
            count -= 1;
        }

        (word, carry)
    }

    pub(crate) fn sar_u8_with_carry(mut byte: u8, mut count: u8) -> (u8, bool) {
        let mut carry = false;
        let ho_bit = byte & 0x80;
        while count > 0 {
            carry = byte & 0x01 != 0;
            byte >>= 1;
            byte |= ho_bit;
            count -= 1;
        }
        (byte, carry)
    }

    pub(crate) fn sar_u16_with_carry(mut word: u16, mut count: u8) -> (u16, bool) {
        let mut carry = false;
        let ho_bit = word & 0x8000;
        while count > 0 {
            carry = word & 0x0001 != 0;
            word >>= 1;
            word |= ho_bit;
            count -= 1;
        }
        (word, carry)
    }*/
}

#[cfg(test)]
mod tests {
    /*
    use super::*;

    #[test]
    fn test_shr() {
        let (result, carry) = Cpu::shr_u8_with_carry(0x80, 7);
        assert_eq!(result, 1);
        assert_eq!(carry, false);
        let (result, carry) = Cpu::shr_u8_with_carry(0x04, 3);
        assert_eq!(result, 0);
        assert_eq!(carry, true);
        let (result, carry) = Cpu::shr_u8_with_carry(0x04, 4);
        assert_eq!(result, 0);
        assert_eq!(carry, false);

        let (result16, carry) = Cpu::shr_u16_with_carry(0x0101, 1);
        assert_eq!(result16, 0x0080);
        assert_eq!(carry, true);
        let (result16, carry) = Cpu::shr_u16_with_carry(0xFF00, 8);
        assert_eq!(result16, 0x00FF);
        assert_eq!(carry, false);
    }

    #[test]
    fn test_shl() {
        let (result, carry) = Cpu::shl_u8_with_carry(0x80, 1);
        assert_eq!(result, 0);
        assert_eq!(carry, true);
        let (result, carry) = Cpu::shl_u8_with_carry(0x01, 7);
        assert_eq!(result, 0x80);
        assert_eq!(carry, false);

        let (result, carry) = Cpu::shl_u16_with_carry(0x0080, 1);
        assert_eq!(result, 0x0100);
        assert_eq!(carry, false);
        let (result, carry) = Cpu::shl_u16_with_carry(0xFF00, 8);
        assert_eq!(result, 0x0000);
        assert_eq!(carry, true);
    }

    #[test]
    fn test_sar_u8() {
        let (result, carry) = Cpu::sar_u8_with_carry(0x80, 3);
        assert_eq!(result, 0xF0);
        assert_eq!(carry, false);
        let (result, carry) = Cpu::sar_u8_with_carry(0x80, 8);
        assert_eq!(result, 0xFF);
        assert_eq!(carry, true);

        let (result, carry) = Cpu::sar_u16_with_carry(0x8000, 2);
        assert_eq!(result, 0xE000);
        assert_eq!(carry, false);
        let (result, carry) = Cpu::sar_u16_with_carry(0x8001, 1);
        assert_eq!(result, 0xC000);
        assert_eq!(carry, true);
    }

    #[test]
    fn test_rcr() {
        let (result, carry) = Cpu::rcr_u8_with_carry(0x01, 1, false);
        assert_eq!(result, 0x00);
        assert_eq!(carry, true);
        let (result, carry) = Cpu::rcr_u8_with_carry(0x01, 3, false);
        assert_eq!(result, 0x40);
        assert_eq!(carry, false);
        let (result, carry) = Cpu::rcr_u8_with_carry(0x00, 1, true);
        assert_eq!(result, 0x80);
        assert_eq!(carry, false);

        // Test overflow
        let mut existing_carry = false;
        let mut operand = 0x80;
        let (result, carry) = Cpu::rcr_u8_with_carry(operand, 1, existing_carry);
        let overflow = (operand & 0x80 == 0 && existing_carry) || (operand & 0x80 != 0 && !existing_carry);
        assert_eq!(result, 0x40);
        assert_eq!(carry, false);
        assert_eq!(overflow, true); // Overflow should be set because HO bit changed from 1 to 0

        operand = 0x04;
        existing_carry = true;

        let (result, carry) = Cpu::rcr_u8_with_carry(operand, 1, existing_carry);
        let overflow = (operand & 0x80 == 0 && existing_carry) || (operand & 0x80 != 0 && !existing_carry);
        assert_eq!(result, 0x82);
        assert_eq!(carry, false);
        assert_eq!(overflow, true); // Overflow should be set because HO bit changed from 0 to 1
    }

    #[test]
    fn test_rcl() {
        let (result, carry) = Cpu::rcl_u8_with_carry(0x80, 1, false);
        assert_eq!(result, 0x00);
        assert_eq!(carry, true);
        let (result, carry) = Cpu::rcl_u8_with_carry(0x80, 2, false);
        assert_eq!(result, 0x01);
        assert_eq!(carry, false);

        // RCL 17 should result in same value
        let (result, carry) = Cpu::rcl_u16_with_carry(0xDEAD, 17, false);
        assert_eq!(result, 0xDEAD);
        assert_eq!(carry, false);

        let (result, carry) = Cpu::rcl_u16_with_carry(0xC8A7, 255, false);
        assert_eq!(result, 0xC8A7);
        assert_eq!(carry, false);
    }

    #[test]
    fn test_ror() {
        let (result, carry) = Cpu::ror_u8_with_carry(0xAA, 8);
        assert_eq!(result, 0xAA);
        assert_eq!(carry, true);

        let (result, carry) = Cpu::ror_u8_with_carry(0x01, 1);
        assert_eq!(result, 0x80);
        assert_eq!(carry, true);
    }*/
}

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

    cpu_vx0::bitwise.rs

    Implement bitwise operations (Shifts, rotations)

*/

use crate::{cpu_common::Mnemonic, cpu_vx0::*};

impl NecVx0 {
    pub(crate) fn shl_u8_with_carry(mut byte: u8, mut count: u8) -> (u8, bool) {
        let mut carry = false;
        while count > 0 {
            carry = byte & 0x80 != 0;
            byte <<= 1;
            count -= 1;
        }
        (byte, carry)
    }

    pub(crate) fn shl_u8_with_carry_overflow(mut byte: u8, mut count: u8) -> (u8, bool, bool) {
        let mut carry = false;
        let mut overflow = false;
        while count > 0 {
            carry = byte & 0x80 != 0;
            byte <<= 1;
            overflow = carry ^ (byte & 0x80 != 0);
            count -= 1;
        }
        (byte, carry, overflow)
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

    pub(crate) fn shl_u16_with_carry_overflow(mut word: u16, mut count: u8) -> (u16, bool, bool) {
        let mut carry = false;
        let mut overflow = false;
        while count > 0 {
            carry = word & 0x8000 != 0;
            word <<= 1;
            overflow = carry ^ (word & 0x8000 != 0);
            count -= 1;
        }
        (word, carry, overflow)
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

    pub(crate) fn rcr_u8_with_carry_overflow(mut byte: u8, mut count: u8, carry_flag: bool) -> (u8, bool, bool) {
        let mut saved_carry = carry_flag;
        let mut new_carry;
        let mut overflow = false;

        while count > 0 {
            new_carry = byte & 0x01 != 0;
            byte >>= 1;
            if saved_carry {
                byte |= 0x80;
            }
            overflow = saved_carry ^ (byte & 0x40 != 0);
            saved_carry = new_carry;
            count -= 1;
        }

        (byte, saved_carry, overflow)
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

    pub(crate) fn rcr_u16_with_carry_overflow(mut word: u16, mut count: u8, carry_flag: bool) -> (u16, bool, bool) {
        let mut saved_carry = carry_flag;
        let mut new_carry;
        let mut overflow = false;

        while count > 0 {
            new_carry = word & 0x0001 != 0;
            word >>= 1;
            if saved_carry {
                word |= 0x8000;
            }
            overflow = saved_carry ^ (word & 0x4000 != 0);
            saved_carry = new_carry;
            count -= 1;
        }

        (word, saved_carry, overflow)
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

    pub(crate) fn rcl_u8_with_carry_overflow(mut byte: u8, mut count: u8, carry_flag: bool) -> (u8, bool, bool) {
        let mut saved_carry = carry_flag;
        let mut new_carry;
        let mut overflow = false;

        while count > 0 {
            new_carry = byte & 0x80 != 0;
            byte <<= 1;
            if saved_carry {
                byte |= 0x01;
            }
            saved_carry = new_carry;
            overflow = saved_carry ^ (byte & 0x80 != 0);
            count -= 1;
        }

        (byte, saved_carry, overflow)
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

    pub(crate) fn rcl_u16_with_carry_overflow(mut word: u16, mut count: u8, carry_flag: bool) -> (u16, bool, bool) {
        let mut saved_carry = carry_flag;
        let mut new_carry;
        let mut overflow = false;

        while count > 0 {
            new_carry = word & 0x8000 != 0;
            word <<= 1;
            if saved_carry {
                word |= 0x0001;
            }
            saved_carry = new_carry;
            overflow = saved_carry ^ (word & 0x8000 != 0);
            count -= 1;
        }

        (word, saved_carry, overflow)
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

    pub(crate) fn ror_u8_with_carry_overflow(mut byte: u8, mut count: u8) -> (u8, bool, bool) {
        let mut carry = false;
        let mut overflow = false;
        while count > 0 {
            carry = byte & 0x01 != 0;
            byte >>= 1;
            overflow = carry ^ (byte & 0x40 != 0);
            if carry {
                byte |= 0x80;
            }
            count -= 1;
        }
        (byte, carry, overflow)
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

    pub(crate) fn ror_u16_with_carry_overflow(mut word: u16, mut count: u8) -> (u16, bool, bool) {
        let mut carry = false;
        let mut overflow = false;
        while count > 0 {
            carry = word & 0x0001 != 0;
            word >>= 1;
            overflow = carry ^ (word & 0x4000 != 0);
            if carry {
                word |= 0x8000;
            }
            count -= 1;
        }
        (word, carry, overflow)
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

    pub(crate) fn rol_u8_with_carry_overflow(mut byte: u8, mut count: u8) -> (u8, bool, bool) {
        let mut carry = false;
        let mut overflow = false;
        while count > 0 {
            carry = byte & 0x80 != 0;
            byte <<= 1;
            overflow = carry ^ (byte & 0x80 != 0);
            if carry {
                byte |= 0x01;
            }
            count -= 1;
        }

        (byte, carry, overflow)
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

    pub(crate) fn rol_u16_with_carry_overflow(mut word: u16, mut count: u8) -> (u16, bool, bool) {
        let mut carry = false;
        let mut overflow = false;
        while count > 0 {
            carry = word & 0x8000 != 0;
            word <<= 1;
            overflow = carry ^ (word & 0x8000 != 0);
            if carry {
                word |= 0x0001;
            }
            count -= 1;
        }

        (word, carry, overflow)
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
    }

    /// Perform various 8-bit binary shift operations
    pub fn bitshift_op8(&mut self, opcode: Mnemonic, operand1: u8, operand2: u8) -> u8 {
        if operand2 == 0 {
            // Flags are not changed if shift amount is 0
            return operand1;
        }

        let result: u8;
        let carry: bool;
        let overflow: bool;

        match opcode {
            Mnemonic::ROL => {
                (result, carry, overflow) = NecVx0::rol_u8_with_carry_overflow(operand1, operand2);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
            }
            Mnemonic::ROR => {
                (result, carry, overflow) = NecVx0::ror_u8_with_carry_overflow(operand1, operand2);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
            }
            Mnemonic::RCL => {
                // Rotate with Carry Left
                // Flags: For left rotates, the OF flag is set to the exclusive OR of the CF bit (after the rotate)
                // and the most-significant bit of the result.
                let existing_carry = self.get_flag(Flag::Carry);
                (result, carry, overflow) = NecVx0::rcl_u8_with_carry_overflow(operand1, operand2, existing_carry);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
            }
            Mnemonic::RCR => {
                let existing_carry = self.get_flag(Flag::Carry);
                (result, carry, overflow) = NecVx0::rcr_u8_with_carry_overflow(operand1, operand2, existing_carry);
                // Only set overflow on SHL of 1
                if operand2 == 1 {
                    // Set overflow to XOR of MSB and CF
                    self.set_flag_state(Flag::Overflow, ((operand1 & 0x80) != 0) ^ existing_carry);
                }
                else {
                    self.set_flag_state(Flag::Overflow, overflow);
                }

                self.set_flag_state(Flag::Carry, carry);
            }
            Mnemonic::SHL => {
                (result, carry, overflow) = NecVx0::shl_u8_with_carry_overflow(operand1, operand2);
                // Set state of Carry Flag
                self.set_flag_state(Flag::Carry, carry);

                // Only set overflow on SHL of 1
                if operand2 == 1 {
                    // If the two highest order bits were different, then they will change on shift
                    // and overflow should be set
                    self.set_flag_state(Flag::Overflow, (operand1 & 0xC0 == 0x80) || (operand1 & 0xC0 == 0x40));
                }
                else {
                    self.set_flag_state(Flag::Overflow, overflow);
                }

                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u8(result);
            }
            Mnemonic::SHR => {
                (result, carry) = NecVx0::shr_u8_with_carry(operand1, operand2);
                // Set state of Carry Flag
                self.set_flag_state(Flag::Carry, carry);

                // Only set overflow on SHR of 1
                if operand2 == 1 {
                    // Only time SHR sets overflow is if HO was 1 and becomes 0, which it always will,
                    // so set overflow flag if it was set.
                    self.set_flag_state(Flag::Overflow, operand1 & 0x80 != 0);
                }
                else {
                    self.clear_flag(Flag::Overflow);
                }
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u8(result);
            }
            Mnemonic::SAR => {
                (result, carry) = NecVx0::sar_u8_with_carry(operand1, operand2);
                // Set Carry Flag
                self.set_flag_state(Flag::Carry, carry);
                self.clear_flag(Flag::Overflow);
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u8(result);
            }
            _ => panic!("Invalid opcode provided to bitshift_op8()"),
        }

        // Return result
        result
    }

    /// Perform various 16-bit binary shift operations
    pub fn bitshift_op16(&mut self, opcode: Mnemonic, operand1: u16, operand2: u8) -> u16 {
        if operand2 == 0 {
            // Flags are not changed if shift amount is 0
            return operand1;
        }

        let result: u16;
        let carry: bool;
        let overflow: bool;

        match opcode {
            Mnemonic::ROL => {
                // Rotate Left
                (result, carry, overflow) = NecVx0::rol_u16_with_carry_overflow(operand1, operand2);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
            }
            Mnemonic::ROR => {
                // Rotate Right
                (result, carry, overflow) = NecVx0::ror_u16_with_carry_overflow(operand1, operand2);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
            }
            Mnemonic::RCL => {
                // Rotate with Carry Left
                let existing_carry = self.get_flag(Flag::Carry);
                (result, carry, overflow) = NecVx0::rcl_u16_with_carry_overflow(operand1, operand2, existing_carry);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow)
            }
            Mnemonic::RCR => {
                // Rotate with Carry Right
                let existing_carry = self.get_flag(Flag::Carry);
                (result, carry, overflow) = NecVx0::rcr_u16_with_carry_overflow(operand1, operand2, existing_carry);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow)
            }
            Mnemonic::SHL => {
                // Shift left
                (result, carry, overflow) = NecVx0::shl_u16_with_carry_overflow(operand1, operand2);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u16(result);
            }
            Mnemonic::SHR => {
                // Shift right
                (result, carry) = NecVx0::shr_u16_with_carry(operand1, operand2);
                // Set state of Carry Flag
                self.set_flag_state(Flag::Carry, carry);

                // Only set overflow on SHR of 1
                if operand2 == 1 {
                    // Only time SHR sets overflow is if HO was 1 and becomes 0, which it always will,
                    // so set overflow flag if it was set.
                    self.set_flag_state(Flag::Overflow, operand1 & 0x8000 != 0);
                }
                else {
                    self.clear_flag(Flag::Overflow);
                }
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u16(result);
            }
            Mnemonic::SAR => {
                (result, carry) = NecVx0::sar_u16_with_carry(operand1, operand2);
                // Set Carry Flag
                self.set_flag_state(Flag::Carry, carry);

                // Clear overflow flag if shift count is 1
                // AoA 6.6.2.2 SAR
                self.clear_flag(Flag::Overflow);
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u16(result);
            }
            _ => panic!("Invalid opcode provided to bitshift_op16()"),
        }

        // Return result
        result
    }
}
/*
#[cfg(test)]
mod tests {
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
    }
}
*/

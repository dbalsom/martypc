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

    cpu_808x::alu.rs

    Implements common ALU operations.

*/
#![allow(dead_code)]

use crate::{
    cpu_808x::{mnemonic::Mnemonic, *},
    cpu_common::alu::*,
};

//use num_traits::PrimInt;

impl Cpu {
    #[inline(always)]
    fn set_parity_flag_from_u8(&mut self, operand: u8) {
        self.set_flag_state(Flag::Parity, PARITY_TABLE[operand as usize]);
    }

    #[inline(always)]
    fn set_parity_flag_from_u16(&mut self, operand: u16) {
        self.set_flag_state(Flag::Parity, PARITY_TABLE[(operand & 0xFF) as usize]);
    }

    /*
    #[inline(always)]
    fn set_parity_flag<T: PrimInt>(&mut self, result: T) {
        self.set_flag_state(Flag::Parity, PARITY_TABLE[result.to_usize().unwrap() & 0xFF]);
    }
    */

    /*
        #[inline(always)]
        pub fn set_szp_flags_from_result<T: PrimInt>(&mut self, result: T) {

            // Set Sign flag to state of Sign (HO) bit
            self.set_flag_state(Flag::Sign, result & (T::one() << (std::mem::size_of::<T>() - 1)) != T::zero());

            // Set Zero flag if result is 0, clear it if not
            self.set_flag_state(Flag::Zero, result == T::zero());

            // Set Parity Flag
            self.set_parity_flag(result);
        }
    */
    pub fn set_szp_flags_from_result_u8(&mut self, result: u8) {
        // Set Sign flag to state of Sign (HO) bit
        self.set_flag_state(Flag::Sign, result & 0x80 != 0);

        // Set Zero flag if result is 0, clear it if not
        self.set_flag_state(Flag::Zero, result == 0);

        // Set Parity Flag
        self.set_parity_flag_from_u8(result);
    }

    pub fn set_szp_flags_from_result_u16(&mut self, result: u16) {
        // Set Sign flag to state of Sign (HO) bit
        self.set_flag_state(Flag::Sign, result & 0x8000 != 0);

        // Set Zero flag if result is 0, clear it if not
        self.set_flag_state(Flag::Zero, result == 0);

        // Set Parity Flag
        self.set_parity_flag_from_u16(result);
    }

    pub fn add_u8(byte1: u8, byte2: u8, carry_in: bool) -> (u8, bool, bool, bool) {
        // OVERFLOW flag indicates signed overflow
        // CARRY flag indicates unsigned overflow
        let mut carry = false;
        let mut overflow = false;
        let mut aux_carry = false;

        // Check for overflow in first nibble
        let nibble_sum = (byte1 & 0x0F).wrapping_add(byte2 & 0x0F);
        if nibble_sum & 0xF0 != 0 {
            aux_carry = true;
        }

        let sum_u16 = (byte1 as u16).wrapping_add(byte2 as u16).wrapping_add(carry_in as u16);
        if sum_u16 > u8::MAX as u16 {
            // Unsigned overflow occurred
            carry = true;
        }

        let sum_i16 = (byte1 as i8 as i16)
            .wrapping_add(byte2 as i8 as i16)
            .wrapping_add(carry_in as i16);
        if sum_i16 > i8::MAX as i16 || sum_i16 < i8::MIN as i16 {
            // Signed overflow occurred
            overflow = true;
        }

        let sum = byte1.wrapping_add(byte2.wrapping_add(carry_in as u8));
        (sum, carry, overflow, aux_carry)
    }

    pub fn add_u16(word1: u16, word2: u16, carry_in: bool) -> (u16, bool, bool, bool) {
        // OVERFLOW flag indicates signed overflow
        // CARRY flag indicates unsigned overflow
        let mut carry = false;
        let mut overflow = false;
        let mut aux_carry = false;

        // Check for overflow in first nibble
        let nibble_sum = (word1 & 0x0F).wrapping_add(word2 & 0x0F);
        if nibble_sum & 0xF0 != 0 {
            aux_carry = true;
        }

        let sum_u32 = (word1 as u32).wrapping_add(word2 as u32).wrapping_add(carry_in as u32);
        if sum_u32 > u16::MAX as u32 {
            // Unsigned overflow occurred
            carry = true;
        }

        let sum_i32 = (word1 as i16 as i32)
            .wrapping_add(word2 as i16 as i32)
            .wrapping_add(carry_in as i32);
        if (sum_i32 > i16::MAX as i32) || (sum_i32 < i16::MIN as i32) {
            // Signed overflow occurred
            overflow = true;
        }

        let sum = word1.wrapping_add(word2.wrapping_add(carry_in as u16));
        (sum, carry, overflow, aux_carry)
    }

    // TODO: Handle Aux Carry Flag
    pub fn sub_u8(byte1: u8, byte2: u8, carry_in: bool) -> (u8, bool, bool, bool) {
        // OVERFLOW flag indicates signed overflow
        // CARRY flag indicates unsigned overflow
        let mut carry = false;
        let mut overflow = false;
        let mut aux_carry = false;

        // Aux flag is set if borrow into first nibble
        if (byte1 & 0x0F).wrapping_sub(byte2 & 0x0F) & 0x10 != 0 {
            aux_carry = true;
        }

        let diff_u16 = (byte1 as u16).wrapping_sub(byte2 as u16);
        let diff_u16 = diff_u16.wrapping_sub(carry_in as u16);
        if diff_u16 > u8::MAX as u16 {
            // Unsigned overflow occurred
            carry = true;
        }

        let diff_i16 = (byte1 as i8 as i16).wrapping_sub(byte2 as i8 as i16);
        let diff_i16 = diff_i16.wrapping_sub(carry_in as i16);
        if diff_i16 > i8::MAX as i16 || diff_i16 < i8::MIN as i16 {
            // Signed overflow occurred
            overflow = true;
        }

        let diff = byte1.wrapping_sub(byte2);
        let diff = diff.wrapping_sub(carry_in as u8);
        (diff, carry, overflow, aux_carry)
    }

    // TODO: Make overflow checks more efficient?
    pub fn sub_u16(word1: u16, word2: u16, carry_in: bool) -> (u16, bool, bool, bool) {
        // OVERFLOW flag indicates signed overflow
        // CARRY flag indicates unsigned overflow
        let mut carry = false;
        let mut overflow = false;
        let mut aux_carry = false;

        // Aux flag is set if borrow into first nibble
        if (word1 & 0x0F).wrapping_sub(word2 & 0x0F) & 0x10 != 0 {
            aux_carry = true;
        }

        let diff_u32 = (word1 as u32).wrapping_sub(word2 as u32);
        let diff_u32 = diff_u32.wrapping_sub(carry_in as u32);
        if diff_u32 > u16::MAX as u32 {
            // Unsigned overflow occurred
            carry = true;
        }

        let diff_i32 = (word1 as i16 as i32).wrapping_sub(word2 as i16 as i32);
        let diff_i32 = diff_i32.wrapping_sub(carry_in as i32);
        if diff_i32 > i16::MAX as i32 || diff_i32 < i16::MIN as i32 {
            // Signed overflow occurred
            overflow = true;
        }

        let diff = word1.wrapping_sub(word2);
        let diff = diff.wrapping_sub(carry_in as u16);
        (diff, carry, overflow, aux_carry)
    }

    /// Unsigned Multiply, 8 bit
    /// Flags: If the high-order bits of the product are 0, the CF and OF flags are cleared;
    /// otherwise, the flags are set. The SF, ZF, AF, and PF flags are undefined.
    pub fn multiply_u8(&mut self, operand1: u8) {
        // 8 bit operand => 16 bit product
        let product: u16 = self.a.l() as u16 * operand1 as u16;

        // Set carry and overflow if product wouldn't fit in u8
        if product & 0xFF00 == 0 {
            self.clear_flag(Flag::Carry);
            self.clear_flag(Flag::Overflow);
        }
        else {
            self.set_flag(Flag::Carry);
            self.set_flag(Flag::Overflow);
        }

        // Note: Does not set Sign or Zero flags
        self.set_register16(Register16::AX, product);
    }

    /// Unsigned Multiply, 16 bits
    /// Flags: If the high-order bits of the product are 0, the CF and OF flags are cleared;
    /// otherwise, the flags are set. The SF, ZF, AF, and PF flags are undefined.
    pub fn multiply_u16(&mut self, operand1: u16) {
        // 16 bit operand => 32bit product
        let product: u32 = self.a.x() as u32 * operand1 as u32;

        // Set carry and overflow if product wouldn't fit in u16
        if product & 0xFFFF0000 == 0 {
            self.clear_flag(Flag::Carry);
            self.clear_flag(Flag::Overflow);
        }
        else {
            self.set_flag(Flag::Carry);
            self.set_flag(Flag::Overflow);
        }

        // Note: Does not set Sign or Zero flags
        let ho_word = (product >> 16) as u16;
        let lo_word = (product & 0x0000FFFF) as u16;

        self.set_register16(Register16::DX, ho_word);
        self.set_register16(Register16::AX, lo_word);
    }

    /// Signed Multiply, 8 bits
    /// Flags: The CF and OF flags are set when significant bits are carried into the upper half
    /// of the result and cleared when the result fits exactly in the lower half of the result.
    /// The SF, ZF, AF, and PF flags are undefined.
    pub fn multiply_i8(&mut self, operand1: i8) {
        // 8 bit operand => 16 bit product
        let product: i16 = (self.a.l() as i8 as i16) * (operand1 as i16);

        // Set carry and overflow if product wouldn't fit in i8
        if product < i8::MIN.into() || product > i8::MAX.into() {
            self.set_flag(Flag::Carry);
            self.set_flag(Flag::Overflow);
        }
        else {
            self.clear_flag(Flag::Carry);
            self.clear_flag(Flag::Overflow);
        }

        // Note: Does not set Sign or Zero flags
        self.set_register16(Register16::AX, product as u16);
    }

    /// Signed Multiply, 16 bits
    /// Flags: The CF and OF flags are set when significant bits are carried into the upper half
    /// of the result and cleared when the result fits exactly in the lower half of the result.
    /// The SF, ZF, AF, and PF flags are undefined.
    pub fn multiply_i16(&mut self, operand1: i16) {
        // 16 bit operand => 32 bit product
        let product: i32 = (self.a.x() as i16 as i32) * (operand1 as i32);

        // Set carry and overflow if product wouldn't fit in i16
        if product < i16::MIN.into() || product > i16::MAX.into() {
            self.set_flag(Flag::Carry);
            self.set_flag(Flag::Overflow);
        }
        else {
            self.clear_flag(Flag::Carry);
            self.clear_flag(Flag::Overflow);
        }

        // Note: Does not set Sign or Zero flags
        // Store 32-bit product in DX:AX
        self.set_register16(Register16::DX, ((product as u32) >> 16 & 0xFFFF) as u16);
        self.set_register16(Register16::AX, ((product as u32) & 0xFFFF) as u16);
    }

    // DIV r/m8 instruction
    // Divide can fail on div by 0 or overflow - (on which we would trigger an exception)
    pub fn divide_u8(&mut self, operand1: u8) -> bool {
        // Divide by 0 returns failure
        if operand1 == 0 {
            return false;
        }

        let quotient = self.a.x() / operand1 as u16;
        let remainder = self.a.x() % operand1 as u16;

        if quotient & 0xFF00 != 0 {
            return false;
        }

        self.set_register8(Register8::AL, quotient as u8);
        self.set_register8(Register8::AH, remainder as u8);

        return true;
    }

    // DIV r/m16 instruction
    // Divide can fail on div by 0 or overflow - (on which we would trigger an exception)
    pub fn divide_u16(&mut self, operand1: u16) -> bool {
        // Divide by 0 returns failure
        if operand1 == 0 {
            return false;
        }

        let dividend = (self.d.x() as u32) << 16 | self.a.x() as u32;

        let quotient = dividend / operand1 as u32;
        let remainder = dividend % operand1 as u32;

        if quotient & 0xFFFF0000 != 0 {
            // Quotient overflow
            return false;
        }
        self.set_register16(Register16::AX, quotient as u16);
        self.set_register16(Register16::DX, remainder as u16);

        return true;
    }

    // Signed DIV r/m8 instruction
    // Divide can fail on div by 0 or overflow - (on which we would trigger an exception)
    pub fn divide_i8(&mut self, operand1: u8) -> bool {
        // Divide by 0 returns failure
        if operand1 == 0 {
            return false;
        }

        let dividend = self.a.x() as i16;

        let quotient = dividend / operand1 as i8 as i16;
        let remainder = dividend % operand1 as i8 as i16;

        if quotient < i8::MIN as i16 || quotient > i8::MAX as i16 {
            // Quotient overflow
            return false;
        }

        // TODO: should we return without modifying regs on failure?
        self.set_register8(Register8::AL, quotient as u8);
        self.set_register8(Register8::AH, remainder as u8);

        return true;
    }

    // Signed DIV r/m16 instruction
    // Divide can fail on div by 0 or overflow - (on which we would trigger an exception)
    pub fn divide_i16(&mut self, operand1: u16) -> bool {
        // Divide by 0 returns failure
        if operand1 == 0 {
            return false;
        }

        let dividend: i32 = ((self.d.x() as u32) << 16 | self.a.x() as u32) as i32;

        // Double cast to sign-extend operand properly
        let quotient = dividend / operand1 as i16 as i32;
        let remainder = dividend % operand1 as i16 as i32;

        if quotient < i16::MIN as i32 || quotient > i16::MAX as i32 {
            // Quotient overflow
            return false;
        }
        self.set_register16(Register16::AX, quotient as u16);
        self.set_register16(Register16::DX, remainder as u16);

        // Return false if overflow
        return true;
    }

    /// Sign extend AL into AX
    pub fn sign_extend_al(&mut self) {
        if self.a.l() & 0x80 != 0 {
            self.a.set_h(0xFF);
        }
        else {
            self.a.set_h(0);
        }
    }

    /// Sign extend AX ito DX:AX
    pub fn sign_extend_ax(&mut self) {
        self.cycles(3);
        if self.a.x() & 0x8000 == 0 {
            self.d.set_x(0x0000);
        }
        else {
            self.cycle(); // Microcode jump @ 05a
            self.d.set_x(0xFFFF);
        }
    }

    /// Perform various 8-bit math operations
    pub fn math_op8(&mut self, opcode: Mnemonic, operand1: u8, operand2: u8) -> u8 {
        match opcode {
            Mnemonic::ADD => {
                let (result, carry, overflow, aux_carry) = operand1.alu_add(operand2);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u8(result);
                result
            }
            Mnemonic::ADC => {
                let (result, carry, overflow, aux_carry) = operand1.alu_adc(operand2, self.get_flag(Flag::Carry));
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u8(result);
                result
            }
            Mnemonic::SUB => {
                //let (result, carry, overflow, aux_carry) = Cpu::sub_u8(operand1, operand2, false );

                let (result, carry, overflow, aux_carry) = operand1.alu_sub(operand2);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u8(result);
                result
            }
            Mnemonic::SBB => {
                // Get value of carry flag
                let carry_in = self.get_flag(Flag::Carry);
                // And pass it to SBB
                //let (result, carry, overflow, aux_carry) = Cpu::sub_u8(operand1, operand2, carry_in );

                let (result, carry, overflow, aux_carry) = operand1.alu_sbb(operand2, carry_in);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u8(result);
                result
            }
            Mnemonic::NEG => {
                // Compute (0-operand)
                // Flags: The CF flag set to 0 if the source operand is 0; otherwise it is set to 1.
                // The OF, SF, ZF, AF, and PF flags are set according to the result.
                let (result, _carry, overflow, aux_carry) = 0u8.alu_sub(operand1);

                self.set_flag_state(Flag::Carry, operand1 != 0);
                // NEG Updates AF, SF, PF, ZF
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u8(result);
                result
            }
            Mnemonic::INC => {
                // INC acts like add xx, 1, however does not set carry flag
                let (result, _carry, overflow, aux_carry) = operand1.alu_add(1);
                // DO NOT set carry Flag
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u8(result);
                result
            }
            Mnemonic::DEC => {
                // DEC acts like sub xx, 1, however does not set carry flag
                let (result, _carry, overflow, aux_carry) = operand1.alu_sub(1);
                // DEC does NOT set carry Flag
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u8(result);
                result
            }
            Mnemonic::OR => {
                let result = operand1 | operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_szp_flags_from_result_u8(result);
                result
            }
            Mnemonic::AND => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_szp_flags_from_result_u8(result);
                result
            }
            Mnemonic::TEST => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_szp_flags_from_result_u8(result);
                // TEST does not modify operand1
                operand1
            }
            Mnemonic::XOR => {
                let result = operand1 ^ operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_szp_flags_from_result_u8(result);
                result
            }
            Mnemonic::NOT => {
                // Flags: None
                let result = !operand1;
                result
            }
            Mnemonic::CMP => {
                // CMP behaves like SUB except we do not store the result
                let (result, carry, overflow, aux_carry) = operand1.alu_sub(operand2);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u8(result);
                // Return the operand1 unchanged
                operand1
            }
            _ => panic!("cpu::math_op8(): Invalid opcode: {:?}", opcode),
        }
    }

    /// Perform various 16-bit math operations
    pub fn math_op16(&mut self, opcode: Mnemonic, operand1: u16, operand2: u16) -> u16 {
        match opcode {
            Mnemonic::ADD => {
                let (result, carry, overflow, aux_carry) = operand1.alu_add(operand2);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            Mnemonic::ADC => {
                let (result, carry, overflow, aux_carry) = operand1.alu_adc(operand2, self.get_flag(Flag::Carry));
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            Mnemonic::SUB => {
                //let (result, carry, overflow, aux_carry) = Cpu::sub_u16(operand1, operand2, false );
                let (result, carry, overflow, aux_carry) = operand1.alu_sub(operand2);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            Mnemonic::SBB => {
                // Get value of carry flag
                let carry_in = self.get_flag(Flag::Carry);
                // And pass it to SBB
                //let (result, carry, overflow, aux_carry) = Cpu::sub_u16(operand1, operand2, carry_in );
                let (result, carry, overflow, aux_carry) = operand1.alu_sbb(operand2, carry_in);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            Mnemonic::NEG => {
                // Compute (0-operand)
                // Flags: The CF flag set to 0 if the source operand is 0; otherwise it is set to 1.
                // The OF, SF, ZF, AF, and PF flags are set according to the result.
                let (result, _carry, overflow, aux_carry) = 0u16.alu_sub(operand1);

                self.set_flag_state(Flag::Carry, operand1 != 0);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            Mnemonic::INC => {
                // INC acts like add xx, 1, however does not set carry flag
                let (result, _carry, overflow, aux_carry) = operand1.alu_add(1);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            Mnemonic::DEC => {
                // DEC acts like sub xx, 1, however does not set carry flag
                let (result, _carry, overflow, aux_carry) = operand1.alu_sub(1);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            Mnemonic::OR => {
                let result = operand1 | operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            Mnemonic::AND => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            Mnemonic::TEST => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_szp_flags_from_result_u16(result);
                // Do not modify operand
                operand1
            }
            Mnemonic::XOR => {
                let result = operand1 ^ operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            Mnemonic::NOT => {
                // Flags: None
                let result = !operand1;
                result
            }
            Mnemonic::CMP => {
                // CMP behaves like SUB except we do not store the result
                let (result, carry, overflow, aux_carry) = operand1.alu_sub(operand2);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u16(result);
                // Return the operand1 unchanged
                operand1
            }
            _ => panic!("cpu::math_op16(): Invalid opcode: {:?}", opcode),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu_808x::CpuType;

    #[test]

    fn test_mul() {
        /*
        let mut cpu = Cpu::new(CpuType::Cpu8088, TraceMode::None, None::<Write>);

        cpu.set_register16(Register16::AX, 1);

        for _ in 0..7 {
            cpu.multiply_u8(2);
        }
        assert_eq!(cpu.al, 128);
        cpu.multiply_u8(2);
        assert_eq!(cpu.ax, 256);

        cpu.set_register16(Register16::AX, 1);

        for _ in 0..15 {
            cpu.multiply_u16(2);
        }
        assert_eq!(cpu.ax, 32768);
        cpu.multiply_u16(2);
        assert_eq!(cpu.ax, 0);
        assert_eq!(cpu.dx, 1); // dx will contain overflow from ax @ 65536
        */
    }
}

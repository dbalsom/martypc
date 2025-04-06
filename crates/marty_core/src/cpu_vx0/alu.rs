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

    cpu_vx0::alu.rs

    Implements common ALU operations.

*/
#![allow(dead_code)]

use crate::{
    cpu_common::{alu::*, Mnemonic},
    cpu_vx0::*,
};

//use num_traits::PrimInt;

impl NecVx0 {
    #[inline(always)]
    pub fn set_parity_flag_from_u8(&mut self, operand: u8) {
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
                // The V20 clears the Aux Carry flag
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u8(result);
                result
            }
            Mnemonic::AND => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                // The V20 clears the Aux Carry flag
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u8(result);
                result
            }
            Mnemonic::TEST => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                // The V20 clears the Aux Carry flag
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u8(result);
                // TEST does not modify operand1
                operand1
            }
            Mnemonic::XOR => {
                let result = operand1 ^ operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                // The V20 clears the Aux Carry flag
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u8(result);
                result
            }
            Mnemonic::NOT => {
                // Flags: None
                !operand1
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
                // The V20 clears the Aux Carry flag
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            Mnemonic::AND => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                // The V20 clears the Aux Carry flag
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            Mnemonic::TEST => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                // The V20 clears the Aux Carry flag
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u16(result);
                // Do not modify operand
                operand1
            }
            Mnemonic::XOR => {
                let result = operand1 ^ operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                // The V20 clears the Aux Carry flag
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            Mnemonic::NOT => {
                // Flags: None
                !operand1
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

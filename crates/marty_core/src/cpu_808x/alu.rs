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

    cpu_808x::alu.rs

    Implements common ALU operations.

*/
#![allow(dead_code)]

use crate::{
    cpu_808x::*,
    cpu_common::{alu::*, InstructionWidth, Mnemonic},
};
//use num_traits::PrimInt;

impl Intel808x {
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

    pub fn set_szp_flags_from_result(&mut self, result: u16) {
        // Set Sign flag to state of Sign (HO) bit
        self.set_flag_state(Flag::Sign, result & self.i.width.sign_mask() != 0);
        // Set Zero flag if result is 0, clear it if not
        self.set_flag_state(Flag::Zero, result == 0);
        // Set Parity Flag
        self.set_parity_flag_from_u16(result);
    }

    pub fn alu_op(&mut self, xi: Xi, operand1: u16, operand2: u16) -> u16 {
        use InstructionWidth::*;
        use Xi::*;
        match (xi, &self.i.width) {
            (ADD, Byte) => {
                let (result, carry, overflow, aux_carry) = (operand1 as u8).alu_add(operand2 as u8);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u8(result);
                result as u16
            }
            (ADD, Word) => {
                let (result, carry, overflow, aux_carry) = operand1.alu_add(operand2);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            (ADC, Byte) => {
                let (result, carry, overflow, aux_carry) =
                    (operand1 as u8).alu_adc(operand2 as u8, self.get_flag(Flag::Carry));
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u8(result);
                result as u16
            }
            (ADC, Word) => {
                let (result, carry, overflow, aux_carry) = operand1.alu_adc(operand2, self.get_flag(Flag::Carry));
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            (SUB, Byte) => {
                let (result, carry, overflow, aux_carry) = (operand1 as u8).alu_sub(operand2 as u8);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u8(result);
                result as u16
            }
            (SUB, Word) => {
                let (result, carry, overflow, aux_carry) = operand1.alu_sub(operand2);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            (SBB, Byte) => {
                let carry_in = self.get_flag(Flag::Carry);
                let (result, carry, overflow, aux_carry) = (operand1 as u8).alu_sbb(operand2 as u8, carry_in);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u8(result);
                result as u16
            }
            (SBB, Word) => {
                // Get value of carry flag
                let carry_in = self.get_flag(Flag::Carry);
                // And pass it to SBB
                //let (result, carry, overflow, aux_carry) = Cpu::sub_u8(operand1, operand2, carry_in );

                let (result, carry, overflow, aux_carry) = operand1.alu_sbb(operand2, carry_in);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            (NEG, Byte) => {
                // Compute (0-operand)
                // Flags: The CF flag set to 0 if the source operand is 0; otherwise it is set to 1.
                // The OF, SF, ZF, AF, and PF flags are set according to the result.
                let (result, _carry, overflow, aux_carry) = 0u8.alu_sub(operand1 as u8);
                self.set_flag_state(Flag::Carry, operand1 != 0);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u8(result);
                result as u16
            }
            (NEG, Word) => {
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
            (INC, Byte) => {
                // INC acts like add xx, 1, however does not set carry flag
                let (result, _carry, overflow, aux_carry) = (operand1 as u8).alu_add(1);
                // DO NOT set carry Flag
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u8(result);
                result as u16
            }
            (INC, Word) => {
                // INC acts like add xx, 1, however does not set carry flag
                let (result, _carry, overflow, aux_carry) = operand1.alu_add(1);
                // DO NOT set carry Flag
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            (DEC, Byte) => {
                // DEC acts like sub xx, 1, however does not set carry flag
                let (result, _carry, overflow, aux_carry) = (operand1 as u8).alu_sub(1);
                // DEC does NOT set carry Flag
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u8(result);
                result as u16
            }
            (DEC, Word) => {
                // DEC acts like sub xx, 1, however does not set carry flag
                let (result, _carry, overflow, aux_carry) = operand1.alu_sub(1);
                // DEC does NOT set carry Flag
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            (OR, _) => {
                let result = operand1 | operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result(result);
                result
            }
            (AND, _) => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result(result);
                result
            }
            (XOR, _) => {
                let result = operand1 ^ operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result(result);
                result
            }
            (NOT, _) => {
                // Flags: None
                !operand1
            }
            (CMP, Byte) => {
                // CMP behaves like SUB except we do not store the result
                let (result, carry, overflow, aux_carry) = (operand1 as u8).alu_sub(operand2 as u8);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u8(result);
                // Return the operand1 unchanged
                operand1
            }
            (CMP, Word) => {
                // CMP behaves like SUB except we do not store the result
                let (result, carry, overflow, aux_carry) = operand1.alu_sub(operand2);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u16(result);
                // Return the operand1 unchanged
                operand1
            }
            _ => panic!("alu_op(): Invalid Xi/Width combination: {:?}", (xi, &self.i.width)),
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
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u8(result);
                result
            }
            Mnemonic::AND => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u8(result);
                result
            }
            Mnemonic::TEST => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
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
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            Mnemonic::AND => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u16(result);
                result
            }
            Mnemonic::TEST => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
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

    /// Perform various 8-bit binary shift operations
    pub fn bitshift_op8(&mut self, opcode: Mnemonic, operand1: u8, operand2: u8) -> u8 {
        // Operand2 will either be 1 or value of CL register on 8088
        if operand2 == 0 {
            // Flags are not changed if shift amount is 0
            return operand1;
        }
        let result: u8;
        let carry: bool;
        let overflow: bool;
        let aux_carry: bool;
        let rot_count = operand2;

        match opcode {
            Mnemonic::ROL => {
                // Rotate Left
                (result, carry, overflow) = operand1.alu_rol(rot_count);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::Carry, carry);
            }
            Mnemonic::ROR => {
                // Rotate Right
                (result, carry, overflow) = operand1.alu_ror(rot_count);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::Carry, carry);
            }
            Mnemonic::RCL => {
                // Rotate through Carry Left
                (result, carry, overflow) = operand1.alu_rcl(rot_count, self.get_flag(Flag::Carry));
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::Carry, carry);
            }
            Mnemonic::RCR => {
                // Rotate through Carry Right
                (result, carry, overflow) = operand1.alu_rcr(rot_count, self.get_flag(Flag::Carry));
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::Carry, carry);
            }
            Mnemonic::SETMO => {
                // Undocumented: SETMO sets all bits in result.
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::AuxCarry);
                self.clear_flag(Flag::Overflow);
                result = 0xFF;
                self.set_szp_flags_from_result_u8(result);
            }
            Mnemonic::SETMOC => {
                // Undocumented: SETMOC sets all bits in result if count > 0
                if self.c.l() != 0 {
                    self.clear_flag(Flag::Carry);
                    self.clear_flag(Flag::AuxCarry);
                    self.clear_flag(Flag::Overflow);
                    result = 0xFF;
                    self.set_szp_flags_from_result_u8(result);
                }
                else {
                    result = operand1;
                }
            }
            Mnemonic::SHL => {
                // Shift Left
                (result, carry, overflow, aux_carry) = operand1.alu_shl_af(operand2);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_szp_flags_from_result_u8(result);
            }
            Mnemonic::SHR => {
                // Shift Right
                (result, carry) = operand1.alu_shr(operand2);
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
                // Shift Arithmetic Right
                (result, carry) = operand1.alu_sar(operand2);
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

    pub fn alu_bitshift_op(&mut self, xi: Xi, operand1: u16, operand2: u8) -> u16 {
        use InstructionWidth::*;
        use Xi::*;

        // Operand2 will either be 1 or value of CL register on 8088
        if operand2 == 0 {
            // Flags are not changed if shift amount is 0
            return operand1;
        }

        let result_u8: u8;
        let mut result: u16 = 0;
        let carry: bool;
        let overflow: bool;
        let aux_carry: bool;
        let rot_count = operand2;

        match (xi, &self.i.width) {
            (ROL, Byte) => {
                // Rotate Left
                (result_u8, carry, overflow) = (operand1 as u8).alu_rol(rot_count);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                result = result_u8 as u16;
            }
            (ROL, Word) => {
                // Rotate Left
                (result, carry, overflow) = operand1.alu_rol(rot_count);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
            }
            (ROR, Byte) => {
                // Rotate Right
                (result_u8, carry, overflow) = (operand1 as u8).alu_ror(rot_count);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::Carry, carry);
                result = result_u8 as u16;
            }
            (ROR, Word) => {
                // Rotate Right
                (result, carry, overflow) = operand1.alu_ror(rot_count);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
            }
            (RCL, Byte) => {
                // Rotate through Carry Left
                (result_u8, carry, overflow) = (operand1 as u8).alu_rcl(rot_count, self.get_flag(Flag::Carry));
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::Carry, carry);
                result = result_u8 as u16;
            }
            (RCL, Word) => {
                // Rotate through Carry Left
                (result, carry, overflow) = operand1.alu_rcl(rot_count, self.get_flag(Flag::Carry));
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
            }
            (RCR, Byte) => {
                // Rotate through Carry Right
                (result_u8, carry, overflow) = (operand1 as u8).alu_rcr(rot_count, self.get_flag(Flag::Carry));
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::Carry, carry);
                result = result_u8 as u16;
            }
            (RCR, Word) => {
                // Rotate through Carry Right
                (result, carry, overflow) = operand1.alu_rcr(rot_count, self.get_flag(Flag::Carry));
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
            }
            (SETMO, _) => {
                // Undocumented: SETMO sets all bits in result.
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.clear_flag(Flag::AuxCarry);
                result = 0xFFFF;
                self.set_szp_flags_from_result_u16(result);
            }
            (SETMOC, _) => {
                // Undocumented: SETMOC sets all bits in result if count > 0
                if self.c.l() != 0 {
                    self.clear_flag(Flag::Carry);
                    self.clear_flag(Flag::Overflow);
                    self.clear_flag(Flag::AuxCarry);
                    result = 0xFFFF;
                    self.set_szp_flags_from_result_u16(result);
                }
                else {
                    result = operand1;
                }
            }
            (SHL, Byte) => {
                // Shift Left
                (result_u8, carry, overflow, aux_carry) = (operand1 as u8).alu_shl_af(operand2);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_szp_flags_from_result_u8(result_u8);
                result = result_u8 as u16;
            }
            (SHL, Word) => {
                (result, carry, overflow, aux_carry) = operand1.alu_shl_af(operand2);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_szp_flags_from_result_u16(result);
            }
            (SHR, Byte) => {
                // Shift Right
                (result_u8, carry) = (operand1 as u8).alu_shr(operand2);
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
                self.set_szp_flags_from_result_u8(result_u8);
                result = result_u8 as u16;
            }
            (SHR, Word) => {
                (result, carry) = operand1.alu_shr(operand2);
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
            (SAR, Byte) => {
                // Shift Arithmetic Right
                (result_u8, carry) = (operand1 as u8).alu_sar(operand2);
                self.set_flag_state(Flag::Carry, carry);
                self.clear_flag(Flag::Overflow);
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u8(result_u8);
                result = result_u8 as u16;
            }
            (SAR, Word) => {
                (result, carry) = operand1.alu_sar(operand2);
                self.set_flag_state(Flag::Carry, carry);
                self.clear_flag(Flag::Overflow);
                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u16(result);
            }
            _ => {}
        }

        result
    }

    /// Perform various 16-bit binary shift operations
    pub fn bitshift_op16(&mut self, opcode: Mnemonic, operand1: u16, operand2: u8) -> u16 {
        // Operand2 will either be 1 or value of CL register on 8088
        if operand2 == 0 {
            // Flags are not changed if shift amount is 0
            return operand1;
        }

        let result: u16;
        let carry: bool;
        let overflow: bool;
        let aux_carry: bool;
        let rot_count = operand2;

        match opcode {
            Mnemonic::ROL => {
                // Rotate Left
                (result, carry, overflow) = operand1.alu_rol(rot_count);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
            }
            Mnemonic::ROR => {
                // Rotate Right
                (result, carry, overflow) = operand1.alu_ror(rot_count);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
            }
            Mnemonic::RCL => {
                // Rotate through Carry Left
                (result, carry, overflow) = operand1.alu_rcl(rot_count, self.get_flag(Flag::Carry));
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
            }
            Mnemonic::RCR => {
                // Rotate through Carry Right
                (result, carry, overflow) = operand1.alu_rcr(rot_count, self.get_flag(Flag::Carry));
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::Carry, carry);
            }
            Mnemonic::SETMO => {
                // Undocumented: SETMO sets all bits in result.
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::AuxCarry);
                self.clear_flag(Flag::Overflow);
                result = 0xFFFF;
                self.set_szp_flags_from_result_u16(result);
            }
            Mnemonic::SETMOC => {
                // Undocumented: SETMOC sets all bits in result if count > 0
                if self.c.l() != 0 {
                    self.clear_flag(Flag::Carry);
                    self.clear_flag(Flag::AuxCarry);
                    self.clear_flag(Flag::Overflow);
                    result = 0xFFFF;
                    self.set_szp_flags_from_result_u16(result);
                }
                else {
                    result = operand1;
                }
            }
            Mnemonic::SHL => {
                (result, carry, overflow, aux_carry) = operand1.alu_shl_af(operand2);
                // Set state of Carry Flag
                self.set_flag_state(Flag::Carry, carry);

                // Only set overflow on SHL of 1
                /*                if operand2 == 1 {
                    // If the two highest order bits were different, then they will change on shift
                    // and overflow should be set
                    //self.set_flag_state(Flag::Overflow, (operand1 & 0xC0 == 0x80) || (operand1 & 0xC0 == 0x40));
                    self.set_flag_state(Flag::AuxCarry, operand1 & 0x08 != 0);
                }
                else {
                    self.clear_flag(Flag::AuxCarry);
                }*/

                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_szp_flags_from_result_u16(result);
            }
            Mnemonic::SHR => {
                (result, carry) = operand1.alu_shr(operand2);
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
                (result, carry) = operand1.alu_sar(operand2);
                self.set_flag_state(Flag::Carry, carry);
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

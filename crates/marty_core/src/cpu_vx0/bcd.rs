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

    cpu_vx0::bcd.rs

    Implements BCD (Binary Coded Decimal) routines.

*/

use crate::cpu_vx0::{muldiv::*, *};

impl NecVx0 {
    /// Ascii Adjust after Addition
    /// Flags: AuxCarry and Carry are set per operation. The OF, SF, ZF, and PF flags are undefined.
    pub fn aaa(&mut self) {
        self.cycles_i(6, &[0x148, 0x149, 0x14a, 0x14b, 0x14c, 0x14d]);

        let old_al = self.a.l();
        let new_al;

        if ((self.a.l() & 0x0F) > 9) || self.get_flag(Flag::AuxCarry) {
            // Intel documentation shows AX := AX + 106 for AAA, but this does not lead to correct
            // behavior if AL carries to AH. Mistake on intel's part(?)
            self.set_register8(Register8::AH, self.a.h().wrapping_add(1));
            new_al = self.a.l().wrapping_add(6);
            self.set_register8(Register8::AL, new_al & 0x0F);
            self.set_flag(Flag::AuxCarry);
            self.set_flag(Flag::Carry);
            //self.cycle_i(0x14e);
        }
        else {
            new_al = self.a.l();
            self.set_register8(Register8::AL, self.a.l() & 0x0F);
            self.clear_flag(Flag::AuxCarry);
            self.clear_flag(Flag::Carry);
            self.cycle_i(MC_JUMP);
        }

        // Handle undefined flag behavior. Determined by testing against real 8088.
        self.clear_flag(Flag::Overflow);
        self.clear_flag(Flag::Zero);
        self.clear_flag(Flag::Sign);
        if new_al == 0 {
            self.set_flag(Flag::Zero);
        }
        if (0x7A..=0x7F).contains(&old_al) {
            self.set_flag(Flag::Overflow);
        }
        if (0x7A..=0xF9).contains(&old_al) {
            self.set_flag(Flag::Sign);
        }

        self.set_flag_state(Flag::Parity, PARITY_TABLE[new_al as usize]);
    }

    /// Ascii Adjust after Subtraction
    /// Flags: AuxCarry and Carry are set per operation. The OF, SF, ZF, and PF flags are undefined.
    pub fn aas(&mut self) {
        let old_al = self.a.l();
        let old_af = self.get_flag(Flag::AuxCarry);
        let new_al;

        self.cycles_i(6, &[0x148, 0x149, 0x14a, 0x14b, MC_JUMP, 0x14d]);
        if ((self.a.l() & 0x0F) > 9) || old_af {
            // Intel documentation shows AX := AX - 6 for AAS, but the microcode only reads AL not AX
            // before calling XI.  Mistake on intel's part(?)
            new_al = self.a.l().wrapping_sub(6);
            self.set_register8(Register8::AH, self.a.h().wrapping_sub(1));
            self.set_register8(Register8::AL, new_al & 0x0F);
            self.set_flag(Flag::AuxCarry);
            self.set_flag(Flag::Carry);
            //self.cycle_i(0x14e);
        }
        else {
            new_al = self.a.l();
            self.set_register8(Register8::AL, self.a.l() & 0x0F);
            self.clear_flag(Flag::Carry);
            self.clear_flag(Flag::AuxCarry);
            self.cycle_i(MC_JUMP);
        }

        // Handle undefined flag behavior. Determined by testing against real 8088.
        self.clear_flag(Flag::Overflow);
        self.clear_flag(Flag::Zero);
        self.clear_flag(Flag::Sign);
        if new_al == 0 {
            self.set_flag(Flag::Zero);
        }
        if old_af && (0x80..=0x85).contains(&old_al) {
            self.set_flag(Flag::Overflow);
        }
        if !old_af && old_al >= 0x80 {
            self.set_flag(Flag::Sign);
        }
        if old_af && ((old_al <= 0x05) || (old_al >= 0x86)) {
            self.set_flag(Flag::Sign);
        }

        self.set_flag_state(Flag::Parity, PARITY_TABLE[new_al as usize]);
    }

    /// Ascii adjust before Division
    /// Flags: The SF, ZF, and PF flags are set according to the resulting binary value in the AL register
    pub fn aad(&mut self, imm8: u8) {
        self.cycles_i(3, &[0x170, 0x171, MC_JUMP]);
        let (_, product) = 0u8.corx(self, self.a.h() as u16, imm8 as u16, false);

        self.set_register8(Register8::AL, self.a.l().wrapping_add(product as u8));
        self.set_register8(Register8::AH, 0);

        self.cycles_i(2, &[0x172, 0x173]);

        // Other sources set flags from AX register. Intel's documentation specifies AL
        self.set_szp_flags_from_result_u8(self.a.l());
    }

    /// DAA — Decimal Adjust AL after Addition
    /// Flags: The SF, ZF, and PF flags are set according to the result. OF is undefined.
    /// See https://www.righto.com/2023/01/understanding-x86s-decimal-adjust-after.html for
    /// clarification on intel's pseudocode for this function.
    pub fn daa(&mut self) {
        let old_cf = self.get_flag(Flag::Carry);
        let old_af = self.get_flag(Flag::AuxCarry);
        let old_al = self.a.l();

        self.clear_flag(Flag::Carry);

        // DAA on the 8088 has different behavior from the pseudocode when AF==1. This was validated against hardware.
        // It is probably something you'd only discover from fuzzing.
        let al_check = match old_af {
            true => 0x9F,
            false => 0x99,
        };

        //log::debug!(" >>>> daa: af: {} cf: {} of: {}", old_af, old_cf, self.get_flag(Flag::Overflow));

        // Handle undefined overflow flag behavior. Observed from testing against real cpu.
        self.clear_flag(Flag::Overflow);
        if old_cf {
            if self.a.l() >= 0x1a && self.a.l() <= 0x7F {
                self.set_flag(Flag::Overflow);
            }
        }
        else if self.a.l() >= 0x7a && self.a.l() <= 0x7F {
            self.set_flag(Flag::Overflow);
        }

        if (self.a.l() & 0x0F) > 9 || self.get_flag(Flag::AuxCarry) {
            self.set_register8(Register8::AL, self.a.l().wrapping_add(6));
            self.set_flag(Flag::AuxCarry);
        }
        else {
            self.clear_flag(Flag::AuxCarry);
        }

        if (old_al > al_check) || old_cf {
            self.set_register8(Register8::AL, self.a.l().wrapping_add(0x60));
            self.set_flag(Flag::Carry);
        }
        else {
            self.clear_flag(Flag::Carry);
        }

        self.set_szp_flags_from_result_u8(self.a.l());
    }

    /// DAA — Decimal Adjust AL after Addition
    /// Used by V20 string BCD instruction ADD4S
    pub fn daa_indirect(
        &mut self,
        result: u8,
        mut carry: bool,
        mut overflow: bool,
        mut aux_carry: bool,
    ) -> (u8, bool, bool, bool) {
        let old_cf = carry;
        let old_af = aux_carry;
        let old_al = result;
        let mut new_result = result;

        // DAA on the 8088 has different behavior from the pseudocode when AF==1. This was validated against hardware.
        // It is probably something you'd only discover from fuzzing.
        let al_check = match old_af {
            true => 0x9F,
            false => 0x99,
        };

        //log::debug!(" >>>> daa: af: {} cf: {} of: {}", old_af, old_cf, self.get_flag(Flag::Overflow));

        if old_cf {
            if (0x1a..=0x7F).contains(&result) {
                overflow = true;
            }
        }
        else if (0x7a..=0x7F).contains(&result) {
            overflow = true;
        }

        if (result & 0x0F) > 9 || aux_carry {
            new_result = new_result.wrapping_add(6);
            aux_carry = true;
        }
        else {
            aux_carry = false;
        }

        if (old_al > al_check) || old_cf {
            new_result = new_result.wrapping_add(0x60);
            carry = true;
        }
        else {
            carry = false;
        }

        (new_result, carry, overflow, aux_carry)
    }

    /// DAS — Decimal Adjust AL after Subtraction
    /// Flags: The SF, ZF, and PF flags are set according to the result.
    pub fn das_indirect(
        &mut self,
        result: u8,
        mut carry: bool,
        mut overflow: bool,
        mut aux_carry: bool,
    ) -> (u8, bool, bool, bool) {
        let old_al = result;
        let old_af = aux_carry;
        let old_cf = carry;
        let mut new_result = result;

        let al_check = match old_af {
            true => 0x9F,
            false => 0x99,
        };

        match (old_af, old_cf) {
            (false, false) => if let 0x9A..=0xDF = result { overflow = true },
            (true, false) => match result {
                0x80..=0x85 | 0xA0..=0xE5 => overflow = true,
                _ => {}
            },
            (false, true) => if let 0x80..=0xDF = result { overflow = true },
            (true, true) => if let 0x80..=0xE5 = result { overflow = true },
        }

        if (result & 0x0F) > 9 || aux_carry {
            new_result = new_result.wrapping_sub(6);
            aux_carry = true;
        }
        else {
            aux_carry = false;
        }

        if (old_al > al_check) || old_cf {
            new_result = new_result.wrapping_sub(0x60);
            carry = true;
        }
        else {
            carry = false;
        }

        (new_result, carry, overflow, aux_carry)
    }

    /// DAS — Decimal Adjust AL after Subtraction
    /// Flags: The SF, ZF, and PF flags are set according to the result.
    pub fn das(&mut self) {
        let old_al = self.a.l();
        let old_af = self.get_flag(Flag::AuxCarry);
        let old_cf = self.get_flag(Flag::Carry);

        let al_check = match old_af {
            true => 0x9F,
            false => 0x99,
        };

        // Handle undefined overflow flag behavior. Observed from testing against real cpu.
        self.clear_flag(Flag::Overflow);

        match (old_af, old_cf) {
            (false, false) => if let 0x9A..=0xDF = self.a.l() { self.set_flag(Flag::Overflow) },
            (true, false) => match self.a.l() {
                0x80..=0x85 | 0xA0..=0xE5 => self.set_flag(Flag::Overflow),
                _ => {}
            },
            (false, true) => if let 0x80..=0xDF = self.a.l() { self.set_flag(Flag::Overflow) },
            (true, true) => if let 0x80..=0xE5 = self.a.l() { self.set_flag(Flag::Overflow) },
        }

        self.clear_flag(Flag::Carry);
        if (self.a.l() & 0x0F) > 9 || self.get_flag(Flag::AuxCarry) {
            self.set_register8(Register8::AL, self.a.l().wrapping_sub(6));
            self.set_flag(Flag::AuxCarry);
        }
        else {
            self.clear_flag(Flag::AuxCarry);
        }

        if (old_al > al_check) || old_cf {
            self.set_register8(Register8::AL, self.a.l().wrapping_sub(0x60));
            self.set_flag(Flag::Carry);
        }
        else {
            self.clear_flag(Flag::Carry);
        }

        self.set_szp_flags_from_result_u8(self.a.l());
    }

    /// AAM - Ascii adjust AX After multiply
    /// Flags: The SF, ZF, and PF flags are set according to the resulting binary value in the AL register
    /// As AAM is implemented via CORD, it can throw an exception. This is indicated by a return value
    /// of false.
    pub fn aam(&mut self, imm8: u8) -> bool {
        self.cycles_i(3, &[0x175, 0x176, MC_JUMP]);
        // 176: A->tmpc   | UNC CORD
        // Jump delay

        match 0u8.cord(self, 0, imm8 as u16, self.a.l() as u16) {
            Ok((quotient, remainder, _)) => {
                // 177:          | COM1 tmpc
                self.set_register8(Register8::AH, !(quotient as u8));
                self.set_register8(Register8::AL, remainder as u8);
                self.cycle_i(0x177);
                // Other sources set flags from AX register. Intel's documentation specifies AL
                self.set_szp_flags_from_result_u8(self.a.l());
                true
            }
            Err(_) => false,
        }
    }
}

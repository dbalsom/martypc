use crate::arch::{Opcode, Register8, Register16};
use crate::cpu::{Cpu, Flag};

impl Cpu {

    /// Ascii adjust before Divison
    /// Flags: The SF, ZF, and PF flags are set according to the resulting binary value in the AL register
    pub fn aad(&mut self) {

        self.set_register8(Register8::AL, self.ah * 10 + self.al);
        self.set_register8(Register8::AH, 0);

        self.set_flags_from_result_u8(self.al);
    }

    /// DAA — Decimal Adjust AL after Addition
    /// Flags: The SF, ZF, and PF flags are set according to the result.
    pub fn daa(&mut self) {

        if (self.al & 0x0F) > 9 || self.get_flag(Flag::AuxCarry) {
            self.set_register8(Register8::AL, self.al.wrapping_add(6));
            self.set_flag(Flag::AuxCarry);
        }
        else {
            self.clear_flag(Flag::AuxCarry);
        }

        if (self.al > 0x99) || self.get_flag(Flag::Carry) {
            self.set_register8(Register8::AL, self.al.wrapping_add(0x60));
            self.set_flag(Flag::Carry);
        }
        else {
            self.clear_flag(Flag::Carry);
        }

        self.set_flags_from_result_u8(self.al);
    }

    /// AAM - Ascii adjust AX After multiply
    /// Flags: The SF, ZF, and PF flags are set according to the resulting binary value in the AL register
    pub fn aam(&mut self, imm8: u8) {

        let temp_al = self.al;
        self.set_register8(Register8::AH, temp_al / imm8);
        self.set_register8(Register8::AL, temp_al % imm8);

        self.set_flags_from_result_u8(self.al);

    }
    
}
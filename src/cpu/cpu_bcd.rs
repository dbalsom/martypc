use crate::cpu::*;

impl<'a> Cpu<'a> {

    /// Ascii Adjust after Addition
    /// Flags: AuxCarry and Carry are set per operation. The OF, SF, ZF, and PF flags are undefined.
    pub fn aaa(&mut self) {
        self.cycles_i(6, &[0x148, 0x149, 0x14a, 0x14b, 0x14c, 0x14d]);
        if ((self.al & 0x0F) > 9) || self.get_flag(Flag::AuxCarry) {
            self.set_register16(Register16::AX, self.ax.wrapping_add(0x106));
            self.set_flag(Flag::AuxCarry);
            self.set_flag(Flag::Carry);
            //self.cycle_i(0x14e);
        }
        else {
            self.clear_flag(Flag::AuxCarry);
            self.clear_flag(Flag::Carry);
            self.cycle_i(MC_JUMP);
        }
        self.set_register8(Register8::AL, self.al & 0x0F);
    }

    /// Ascii Adjust after Subtraction
    /// Flags: AuxCarry and Carry are set per operation. The OF, SF, ZF, and PF flags are undefined.
    pub fn aas(&mut self) {    
        self.cycles_i(6, &[0x148, 0x149, 0x14a, 0x14b, MC_JUMP, 0x14d]);
        if ((self.al & 0x0F) > 9) || self.get_flag(Flag::AuxCarry) {
            self.set_register16(Register16::AX, self.ax.wrapping_sub(6));
            self.set_register8(Register8::AH, self.ah.wrapping_sub(1));
            self.set_register8(Register8::AL, self.al & 0x0F);
            self.set_flag(Flag::AuxCarry);
            self.set_flag(Flag::Carry);
            //self.cycle_i(0x14e);
            
        }
        else {
            self.set_register8(Register8::AL, self.al & 0x0F);
            self.clear_flag(Flag::Carry);
            self.clear_flag(Flag::AuxCarry);
            self.cycle_i(MC_JUMP);
        }
    }

    /// Ascii adjust before Divison
    /// Flags: The SF, ZF, and PF flags are set according to the resulting binary value in the AL register
    pub fn aad(&mut self, imm8: u8) {

        self.cycles_i(3, &[0x170, 0x171, MC_JUMP]);
        let product_native = (self.ah as u16).wrapping_mul(imm8 as u16) as u8;
        let (_, product) = self.corx8(self.ah, imm8, false);
        assert!((product as u8) == product_native);

        self.set_register8(Register8::AL, self.al.wrapping_add(product as u8));
        self.set_register8(Register8::AH, 0);
        
        self.cycles_i(2, &[0x172, 0x173]);

        // Other sources set flags from AX register. Intel's documentation specifies AL
        self.set_flags_from_result_u8(self.al);
    }

    /// DAA — Decimal Adjust AL after Addition
    /// Flags: The SF, ZF, and PF flags are set according to the result.
    pub fn daa(&mut self) {

        let old_cf = self.get_flag(Flag::Carry);
        self.clear_flag(Flag::Carry);
        if (self.al & 0x0F) > 9 || self.get_flag(Flag::AuxCarry) {
            let temp16: u16 = self.al.wrapping_add(6) as u16;
            self.set_register8(Register8::AL, (temp16 & 0xFF) as u8);
            // Set carry flag on overflow from AL + 6
            self.set_flag_state(Flag::Carry, old_cf || temp16 & 0xFF00 != 0);
            self.set_flag(Flag::AuxCarry);
        }
        else {
            self.clear_flag(Flag::AuxCarry);
        }

        // Different sources show this value 0x99 or 0x9F, does it matter?
        // Current intel documents show 0x99
        if (self.al > 0x99) || self.get_flag(Flag::Carry) {
            self.set_register8(Register8::AL, self.al.wrapping_add(0x60));
            self.set_flag(Flag::Carry);
        }
        else {
            self.clear_flag(Flag::Carry);
        }

        self.set_flags_from_result_u8(self.al);
    }

    /// DAS — Decimal Adjust AL after Subtraction
    /// Flags: The SF, ZF, and PF flags are set according to the result.
    pub fn das(&mut self) {
        let old_al = self.al;
        let old_cf = self.get_flag(Flag::Carry);
        self.clear_flag(Flag::Carry);
        if (self.al & 0x0F) > 9 || self.get_flag(Flag::AuxCarry) {
            let temp16: u16 = self.al.wrapping_sub(6) as u16;
            self.set_register8(Register8::AL, self.al.wrapping_sub(6));
            self.set_flag_state(Flag::Carry, old_cf || temp16 & 0x100 != 0);
            self.set_flag(Flag::AuxCarry);
        }
        else {
            self.clear_flag(Flag::AuxCarry);
        }

        // Different sources show this value 0x99 or 0x9F, does it matter?
        // Current intel documents show 0x99
        if (old_al > 0x99) || (old_cf) {
            self.set_register8(Register8::AL, self.al.wrapping_sub(0x60));
            self.set_flag(Flag::Carry);
        }

        self.set_flags_from_result_u8(self.al);
    }

    /// AAM - Ascii adjust AX After multiply
    /// Flags: The SF, ZF, and PF flags are set according to the resulting binary value in the AL register
    pub fn aam(&mut self, imm8: u8) {

        if imm8 == 0 {
            return;
        }
        let temp_al = self.al;
        self.set_register8(Register8::AH, temp_al / imm8);
        self.set_register8(Register8::AL, temp_al % imm8);

        // Other sources set flags from AX register. Intel's documentation specifies AL
        self.set_flags_from_result_u8(self.al);
    }
    
}
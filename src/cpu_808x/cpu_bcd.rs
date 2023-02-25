use crate::cpu_808x::*;
use crate::cpu_808x::cpu_muldiv::*;

impl<'a> Cpu<'a> {

    /// Ascii Adjust after Addition
    /// Flags: AuxCarry and Carry are set per operation. The OF, SF, ZF, and PF flags are undefined.
    pub fn aaa(&mut self) {
        self.cycles_i(6, &[0x148, 0x149, 0x14a, 0x14b, 0x14c, 0x14d]);

        let old_al = self.al;
        let new_al;

        if ((self.al & 0x0F) > 9) || self.get_flag(Flag::AuxCarry) {
            // Intel documentation shows AX := AX + 106 for AAA, but this does not lead to correct
            // behavior if AL carries to AH. Mistake on intel's part(?)
            self.set_register8(Register8::AH, self.ah.wrapping_add(1));
            new_al = self.al.wrapping_add(6);
            self.set_register8(Register8::AL, new_al & 0x0F);
            self.set_flag(Flag::AuxCarry);
            self.set_flag(Flag::Carry);
            //self.cycle_i(0x14e);
        }
        else {
            new_al = self.al;
            self.set_register8(Register8::AL, self.al & 0x0F);
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
        if old_al >= 0x7A && old_al <= 0x7F {
            self.set_flag(Flag::Overflow);
        }
        if old_al >= 0x7A && old_al <= 0xF9 {
            self.set_flag(Flag::Sign);
        }

        self.set_flag_state(Flag::Parity, PARITY_TABLE[new_al as usize]);
    }

    /// Ascii Adjust after Subtraction
    /// Flags: AuxCarry and Carry are set per operation. The OF, SF, ZF, and PF flags are undefined.
    pub fn aas(&mut self) {    

        let old_al = self.al;
        let old_af = self.get_flag(Flag::AuxCarry);
        let new_al;

        self.cycles_i(6, &[0x148, 0x149, 0x14a, 0x14b, MC_JUMP, 0x14d]);
        if ((self.al & 0x0F) > 9) || old_af {
            // Intel documentation shows AX := AX - 6 for AAS, but the microcode only reads AL not AX
            // before calling XI.  Mistake on intel's part(?)
            new_al = self.al.wrapping_sub(6);
            self.set_register8(Register8::AH, self.ah.wrapping_sub(1));
            self.set_register8(Register8::AL, new_al & 0x0F);
            self.set_flag(Flag::AuxCarry);
            self.set_flag(Flag::Carry);
            //self.cycle_i(0x14e);
            
        }
        else {
            new_al = self.al;
            self.set_register8(Register8::AL, self.al & 0x0F);
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
        if old_af && old_al >= 0x80 && old_al <= 0x85 {
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

    /// Ascii adjust before Divison
    /// Flags: The SF, ZF, and PF flags are set according to the resulting binary value in the AL register
    pub fn aad(&mut self, imm8: u8) {

        self.cycles_i(3, &[0x170, 0x171, MC_JUMP]);
        let product_native = (self.ah as u16).wrapping_mul(imm8 as u16) as u8;
        let (_, product) = 0u8.corx(self, self.ah as u16, imm8 as u16, false);
        assert!((product as u8) == product_native);

        self.set_register8(Register8::AL, self.al.wrapping_add(product as u8));
        self.set_register8(Register8::AH, 0);
        
        self.cycles_i(2, &[0x172, 0x173]);

        // Other sources set flags from AX register. Intel's documentation specifies AL
        self.set_szp_flags_from_result_u8(self.al);
    }

    /// DAA — Decimal Adjust AL after Addition
    /// Flags: The SF, ZF, and PF flags are set according to the result. OF is undefined.
    /// See https://www.righto.com/2023/01/understanding-x86s-decimal-adjust-after.html for
    /// clarification on intel's pseudocode for this function.
    pub fn daa(&mut self) {

        let old_cf = self.get_flag(Flag::Carry);
        let old_af = self.get_flag(Flag::AuxCarry);
        let old_al = self.al;

        self.clear_flag(Flag::Carry);

        // DAA on the 8088 has different behavior from the pseudocode when AF==1. This was validated against hardware.
        // It is probably something you'd only discover from fuzzing.
        let al_check = match old_af {
            true => 0x9F,
            false => 0x99
        };

        //log::debug!(" >>>> daa: af: {} cf: {} of: {}", old_af, old_cf, self.get_flag(Flag::Overflow));

        // Handle undefined overflow flag behavior. Observed from testing against real cpu.
        self.clear_flag(Flag::Overflow);
        if old_cf {
            if self.al >= 0x1a && self.al <= 0x7F {
                self.set_flag(Flag::Overflow);
            }            
        }
        else {
            if self.al >= 0x7a && self.al <= 0x7F {
                self.set_flag(Flag::Overflow);
            }
        }

        if (self.al & 0x0F) > 9 || self.get_flag(Flag::AuxCarry) {
            self.set_register8(Register8::AL, self.al.wrapping_add(6));
            self.set_flag(Flag::AuxCarry);
        }
        else {
            self.clear_flag(Flag::AuxCarry);
        }

        if (old_al > al_check) || old_cf {
            self.set_register8(Register8::AL, self.al.wrapping_add(0x60));
            self.set_flag(Flag::Carry);
        }
        else {
            self.clear_flag(Flag::Carry);
        }

        self.set_szp_flags_from_result_u8(self.al);
    }

    /// DAS — Decimal Adjust AL after Subtraction
    /// Flags: The SF, ZF, and PF flags are set according to the result.
    pub fn das(&mut self) {

        let old_al = self.al;
        let old_af = self.get_flag(Flag::AuxCarry);
        let old_cf = self.get_flag(Flag::Carry);

        let al_check = match old_af {
            true => 0x9F,
            false => 0x99
        };

        // Handle undefined overflow flag behavior. Observed from testing against real cpu.
        self.clear_flag(Flag::Overflow);

        match (old_af, old_cf) {
            (false, false) => match self.al {
                0x9A..=0xDF => self.set_flag(Flag::Overflow),
                _ => {}
            },
            (true, false) => match self.al {
                0x80..=0x85 | 0xA0..=0xE5 => self.set_flag(Flag::Overflow),
                _ => {}
            },
            (false, true) => match self.al {
                0x80..=0xDF => self.set_flag(Flag::Overflow),
                _ => {}
            },
            (true, true) => match self.al {
                0x80..=0xE5 => self.set_flag(Flag::Overflow),
                _ => {}
            }
        }

        self.clear_flag(Flag::Carry);
        if (self.al & 0x0F) > 9 || self.get_flag(Flag::AuxCarry) {
            self.set_register8(Register8::AL, self.al.wrapping_sub(6));
            self.set_flag(Flag::AuxCarry);
        }
        else {
            self.clear_flag(Flag::AuxCarry);
        }

        if (old_al > al_check) || old_cf {
            self.set_register8(Register8::AL, self.al.wrapping_sub(0x60) as u8);
            self.set_flag(Flag::Carry);
        }
        else {
            self.clear_flag(Flag::Carry);
        }

        self.set_szp_flags_from_result_u8(self.al);
    }

    /// AAM - Ascii adjust AX After multiply
    /// Flags: The SF, ZF, and PF flags are set according to the resulting binary value in the AL register
    /// As AAM is implemented via CORD, it can throw an exception. This is indicated by a return value
    /// of false.
    pub fn aam(&mut self, imm8: u8) -> bool {

        self.cycles_i(3, &[0x175, 0x176, MC_JUMP]);
        // 176: A->tmpc   | UNC CORD
        // Jump delay

        match 0u8.cord(self, 0, imm8 as u16, self.al as u16) {
            Ok((quotient, remainder, _)) => {

                // 177:          | COM1 tmpc
                self.set_register8(Register8::AH, !(quotient as u8));
                self.set_register8(Register8::AL, remainder as u8);
                self.cycle_i(0x177);
                // Other sources set flags from AX register. Intel's documentation specifies AL
                self.set_szp_flags_from_result_u8(self.al);
                return true
            }
            Err(_) => {
                return false
            }
        }

        


        /*
        let temp_al = self.al;
        self.set_register8(Register8::AH, temp_al / imm8);
        self.set_register8(Register8::AL, temp_al % imm8);
        */



        // Other sources set flags from AX register. Intel's documentation specifies AL
        // self.set_szp_flags_from_result_u8(self.al);
    }
    
}
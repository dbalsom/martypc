use crate::cpu::*;
use crate::cpu::cpu_mnemonic::Mnemonic;

impl<'a> Cpu<'a> {

    pub(crate) fn shl_u8_with_carry(mut byte: u8, mut count: u8) -> (u8, bool) {

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

    pub(crate) fn rol_u8_with_carry(mut byte: u8, mut count: u8 ) -> (u8, bool) {

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

    pub(crate) fn rol_u16_with_carry(mut word: u16, mut count: u8 ) -> (u16, bool) {

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

        // All processors after 8086 mask the rotation count to 5 bits (31 maximum)
        /*
        let rot_count = match self.cpu_type {
            CpuType::Cpu8088 | CpuType::Cpu8086 => operand2,
            _=> operand2 & 0x1F
        };
        */

        let rot_count = operand2;

        match opcode {
            Mnemonic::ROL => {
                (result, carry) = Cpu::rol_u8_with_carry(operand1, rot_count);
                self.set_flag_state(Flag::Carry, carry);
                // Only set overflow on ROL of 1
                if rot_count == 1 {
                    // Set overflow to XOR of MSB and CF
                    self.set_flag_state(Flag::Overflow, ((result & 0x80) != 0) ^ carry);
                }
            }
            Mnemonic::ROR => {
                (result, carry) = Cpu::ror_u8_with_carry(operand1, rot_count);
                self.set_flag_state(Flag::Carry, carry);
                // Only set overflow on ROR of 1
                if rot_count == 1 {
                    // Set overflow to XOR of two MS bits
                    self.set_flag_state(Flag::Overflow, ((result & 0x80) != 0) ^ ((result & 0x40) != 0));
                }          
            }
            Mnemonic::RCL => {
                // Rotate with Carry Left
                // Flags: For left rotates, the OF flag is set to the exclusive OR of the CF bit (after the rotate) 
                // and the most-significant bit of the result. 
                let existing_carry = self.get_flag(Flag::Carry);
                (result, carry) = Cpu::rcl_u8_with_carry(operand1, rot_count, existing_carry);
                self.set_flag_state(Flag::Carry, carry);
                // Only set overflow on SHL of 1
                if rot_count == 1 {
                    // Set overflow to XOR of MSB and CF
                    self.set_flag_state(Flag::Overflow, ((result & 0x80) != 0) ^ carry);
                }             
            }
            Mnemonic::RCR => {
                let existing_carry = self.get_flag(Flag::Carry);
                // Only set overflow on SHL of 1
                if rot_count == 1 {
                    // Set overflow to XOR of MSB and CF
                    self.set_flag_state(Flag::Overflow, ((operand1 & 0x80) != 0) ^ existing_carry);
                }               

                (result, carry) = Cpu::rcr_u8_with_carry(operand1, rot_count, existing_carry);
                self.set_flag_state(Flag::Carry, carry);
            }
            Mnemonic::SETMO => {
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::AuxCarry);
                self.clear_flag(Flag::Zero);
                self.clear_flag(Flag::Overflow);

                self.set_flag(Flag::Parity);
                self.set_flag(Flag::Sign);

                result = 0xFF;
            }
            Mnemonic::SETMOC => {

                if self.cl != 0 {
                    self.clear_flag(Flag::Carry);
                    self.clear_flag(Flag::AuxCarry);
                    self.clear_flag(Flag::Zero);
                    self.clear_flag(Flag::Overflow);

                    self.set_flag(Flag::Parity);
                    self.set_flag(Flag::Sign);
                    result = 0xFF;
                }
                else {
                    result = operand1;
                }
            }            
            Mnemonic::SHL => {
                (result, carry) = Cpu::shl_u8_with_carry(operand1, operand2);
                // Set state of Carry Flag
                self.set_flag_state(Flag::Carry, carry);

                // Only set overflow on SHL of 1
                if operand2 == 1 {
                    // If the two highest order bits were different, then they will change on shift
                    // and overflow should be set
                    self.set_flag_state(Flag::Overflow, (operand1 & 0xC0 == 0x80) || (operand1 & 0xC0 == 0x40));
                }
                
                self.set_flags_from_result_u8(result);
            }
            Mnemonic::SHR => {
                (result, carry) = Cpu::shr_u8_with_carry(operand1, operand2);
                // Set state of Carry Flag
                self.set_flag_state(Flag::Carry, carry);

                // Only set overflow on SHR of 1
                if operand2 == 1 {
                    // Only time SHR sets overflow is if HO was 1 and becomes 0, which it always will,
                    // so set overflow flag if it was set. 
                    self.set_flag_state(Flag::Overflow, operand1 & 0x80 != 0 );
                }
                self.set_flags_from_result_u8(result);
            }
            Mnemonic::SAR => {
                (result, carry) = Cpu::sar_u8_with_carry(operand1, operand2);
                // Set Carry Flag
                self.set_flag_state(Flag::Carry, carry);

                // Clear overflow flag if shift count is 1
                // AoA 6.6.2.2 SAR
                if operand2 == 1 {
                    self.clear_flag(Flag::Overflow);
                }
                self.set_flags_from_result_u8(result);
            }
            _=> panic!("Invalid opcode provided to bitshift_op8()")
        }

        // Return result        
        result
    }

    /// Peform various 16-bit binary shift operations
    pub fn bitshift_op16(&mut self, opcode: Mnemonic, operand1: u16, operand2: u8) -> u16 {

        // Operand2 will either be 1 or value of CL register on 8088
        if operand2 == 0 {
            // Flags are not changed if shift amount is 0
            return operand1;
        }

        let result: u16;
        let carry: bool;

        // All processors after 8086 mask the rotation count to 5 bits (31 maximum)
        let rot_count = match self.cpu_type {
            CpuType::Cpu8088 | CpuType::Cpu8086 => operand2,
            _=> operand2 & 0x1F
        };

        match opcode {
            Mnemonic::ROL => {
                // Rotate Left
                // Flags: For left rotates, the OF flag is set to the exclusive OR of the CF bit (after the rotate) 
                // and the most-significant bit of the result. 
                (result, carry) = Cpu::rol_u16_with_carry(operand1, rot_count);
                self.set_flag_state(Flag::Carry, carry);

                // Overflow only defined for ROL of 1
                if rot_count == 1 {
                    // Set overflow to XOR of MSB and CF*
                    self.set_flag_state(Flag::Overflow, ((result & 0x8000) != 0) ^ carry);
                }
            }
            Mnemonic::ROR => {
                // Rotate Right
                // Flags: For right rotates, the OF flag is set to the exclusive OR of the two most-significant bits of the result.
                (result, carry) = Cpu::ror_u16_with_carry(operand1, rot_count);
                self.set_flag_state(Flag::Carry, carry);
                
                // Overflow only defined for ROR of 1
                if rot_count == 1 {
                    // Set overflow to XOR of two MS bits*
                    self.set_flag_state(Flag::Overflow, ((result & 0x8000) != 0) ^ ((result & 0x4000) != 0));
                }
            }
            Mnemonic::RCL => {
                // Rotate with Carry Left
                // Flags: For left rotates, the OF flag is set to the exclusive OR of the CF bit (after the rotate) 
                // and the most-significant bit of the result. 

                let existing_carry = self.get_flag(Flag::Carry);
                (result, carry) = Cpu::rcl_u16_with_carry(operand1, rot_count, existing_carry);
                self.set_flag_state(Flag::Carry, carry);
                // Overflow only defined for RCL of 1
                if rot_count == 1 {
                    // Set overflow to XOR of MSB and CF*
                    self.set_flag_state(Flag::Overflow, ((result & 0x8000) != 0) ^ carry);
                }
            }
            Mnemonic::RCR => {
                // Rotate with Carry Right
                // Flags: For right rotates, the OF flag is set to the exclusive OR of the two most-significant bits of the result.

                // Only set overflow on SHL of 1
                let existing_carry = self.get_flag(Flag::Carry);

                // Overflow only defined for RCL of 1
                if rot_count == 1 {
                    // Set overflow to XOR of MSB and CF*
                    self.set_flag_state(Flag::Overflow, ((operand1 & 0x8000) != 0) ^ existing_carry);
                }

                (result, carry) = Cpu::rcr_u16_with_carry(operand1, rot_count, existing_carry);
                self.set_flag_state(Flag::Carry, carry);

                // The rcr instruction does not affect the zero, sign, parity, or auxiliary carry flags.
                // AoA 6.6.3.2
            }
            Mnemonic::SETMO => {
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::AuxCarry);
                self.clear_flag(Flag::Zero);
                self.clear_flag(Flag::Overflow);

                self.set_flag(Flag::Parity);
                self.set_flag(Flag::Sign);

                result = 0xFFFF;
            }
            Mnemonic::SETMOC => {

                if self.cl != 0 {
                    self.clear_flag(Flag::Carry);
                    self.clear_flag(Flag::AuxCarry);
                    self.clear_flag(Flag::Zero);
                    self.clear_flag(Flag::Overflow);

                    self.set_flag(Flag::Parity);
                    self.set_flag(Flag::Sign);
                    result = 0xFFFF;
                }
                else {
                    result = operand1;
                }
            }            
            Mnemonic::SHL => {
                (result, carry) = Cpu::shl_u16_with_carry(operand1, operand2);
                // Set state of Carry Flag
                self.set_flag_state(Flag::Carry, carry);

                // Only set overflow on SHL of 1
                if operand2 == 1 {
                    // If the two highest order bits were different, then they will change on shift
                    // and overflow should be set
                    self.set_flag_state(Flag::Overflow, (operand1 & 0xC000 == 0x8000) || (operand1 & 0xC000 == 0x4000));
                }
                self.set_flags_from_result_u16(result);
            }
            Mnemonic::SHR => {
                (result, carry) = Cpu::shr_u16_with_carry(operand1, operand2);
                // Set state of Carry Flag
                self.set_flag_state(Flag::Carry, carry);

                // Only set overflow on SHR of 1
                if operand2 == 1 {
                    // Only time SHR sets overflow is if HO was 1 and becomes 0, which it always will,
                    // so set overflow flag if it was set. 
                    self.set_flag_state(Flag::Overflow, operand1 & 0x8000 != 0 );
                }
                self.set_flags_from_result_u16(result);
            }
            Mnemonic::SAR => {
                (result, carry) = Cpu::sar_u16_with_carry(operand1, operand2);
                // Set Carry Flag
                self.set_flag_state(Flag::Carry, carry);

                // Clear overflow flag if shift count is 1
                // AoA 6.6.2.2 SAR
                if operand2 == 1 {
                    self.clear_flag(Flag::Overflow);
                }
                self.set_flags_from_result_u16(result);
            }
            _=> panic!("Invalid opcode provided to bitshift_op16()")
        }

        // Return result        
        result
    }        
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_shr() {
        let (result, carry) = Cpu::shr_u8_with_carry( 0x80, 7 );
        assert_eq!(result, 1);
        assert_eq!(carry, false);
        let (result, carry) = Cpu::shr_u8_with_carry( 0x04, 3);
        assert_eq!(result, 0);
        assert_eq!(carry, true);
        let (result, carry) = Cpu::shr_u8_with_carry( 0x04, 4);
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

        let (result,carry) = Cpu::shl_u8_with_carry(0x80, 1);
        assert_eq!(result, 0);
        assert_eq!(carry, true);
        let (result,carry) = Cpu::shl_u8_with_carry(0x01, 7);
        assert_eq!(result, 0x80);
        assert_eq!(carry, false);

        let (result,carry) = Cpu::shl_u16_with_carry(0x0080, 1);
        assert_eq!(result, 0x0100);
        assert_eq!(carry, false);
        let (result,carry) = Cpu::shl_u16_with_carry(0xFF00, 8);
        assert_eq!(result, 0x0000);
        assert_eq!(carry, true);
    }

    #[test]
    fn test_sar_u8() {
        let (result,carry) = Cpu::sar_u8_with_carry(0x80, 3);
        assert_eq!(result, 0xF0);
        assert_eq!(carry, false);
        let (result,carry) = Cpu::sar_u8_with_carry(0x80, 8);
        assert_eq!(result, 0xFF);
        assert_eq!(carry, true);

        let (result,carry) = Cpu::sar_u16_with_carry(0x8000, 2);
        assert_eq!(result, 0xE000);
        assert_eq!(carry, false);
        let (result,carry) = Cpu::sar_u16_with_carry(0x8001, 1);
        assert_eq!(result, 0xC000);
        assert_eq!(carry, true);        
    }

    #[test]
    fn test_rcr() {
        let (result, carry) = Cpu::rcr_u8_with_carry(0x01, 1, false);
        assert_eq!(result, 0x00);
        assert_eq!(carry, true);
        let (result, carry) = Cpu::rcr_u8_with_carry(0x01, 3, false );
        assert_eq!(result, 0x40);
        assert_eq!(carry, false);
        let (result, carry) = Cpu::rcr_u8_with_carry(0x00, 1, true);
        assert_eq!(result, 0x80);
        assert_eq!(carry, false);

        // Test overflow
        let mut existing_carry = false;
        let mut operand = 0x80;
        let(result,carry) = Cpu::rcr_u8_with_carry(operand, 1, existing_carry);
        let overflow = (operand & 0x80 == 0 && existing_carry) || (operand & 0x80 != 0 && !existing_carry);
        assert_eq!(result, 0x40);
        assert_eq!(carry, false);
        assert_eq!(overflow, true); // Overflow should be set because HO bit changed from 1 to 0

        operand = 0x04;
        existing_carry = true;
        
        let(result,carry) = Cpu::rcr_u8_with_carry(operand, 1, existing_carry);
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


        let (result, carry) = Cpu::rcl_u16_with_carry(0xC8a7, 255, false);
        assert_eq!(result, 0xC8a7);
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
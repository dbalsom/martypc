use crate::cpu::*;
use crate::cpu::cpu_mnemonic::Mnemonic;

const PARITY_TABLE: [bool; 256] = {
    let mut table = [false; 256];
    
    // can't do for loop in const
    let mut index = 0;
    loop {
        table[index] = index.count_ones() % 2 == 0;
        index += 1;
        
        if index == 256 {
            break;
        }
    }

    table
};

impl<'a> Cpu<'a> {

    #[inline(always)]
    fn set_parity_flag_from_u8(&mut self, operand: u8) {
        self.set_flag_state(Flag::Parity, PARITY_TABLE[operand as usize]);
    }

    #[inline(always)]
    fn set_parity_flag_from_u16(&mut self, operand: u16) {
        self.set_flag_state(Flag::Parity, PARITY_TABLE[(operand & 0xFF) as usize]);
    }

    pub fn set_flags_from_result_u8(&mut self, result: u8) {
        // Set Sign flag to state of Sign (HO) bit
        self.set_flag_state(Flag::Sign, result & 0x80 != 0);

        // Set Zero flag if result is 0, clear it if not
        self.set_flag_state(Flag::Zero, result == 0);

        // Set Parity Flag
        self.set_parity_flag_from_u8(result);
    }

    pub fn set_flags_from_result_u16(&mut self, result: u16) {
        // Set Sign flag to state of Sign (HO) bit
        self.set_flag_state(Flag::Sign, result & 0x8000 != 0);

        // Set Zero flag if result is 0, clear it if not
        self.set_flag_state(Flag::Zero, result == 0);

        // Set Parity Flag
        self.set_parity_flag_from_u16(result);
    }

    fn add_u8(byte1: u8, byte2: u8, carry_in: bool) -> (u8, bool, bool, bool) {
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

        let sum_i16 = (byte1 as i8 as i16).wrapping_add( byte2 as i8 as i16).wrapping_add(carry_in as i16);
        if sum_i16 > i8::MAX as i16 || sum_i16 < i8::MIN as i16 {
            // Signed overflow occurred
            overflow = true;
        }

        let sum = byte1.wrapping_add(byte2.wrapping_add(carry_in as u8));
        (sum, carry, overflow, aux_carry)
    }

    fn add_u16(word1: u16, word2: u16, carry_in: bool) -> (u16, bool, bool, bool) {
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

        let sum_i32 = (word1 as i16 as i32).wrapping_add(word2 as i16 as i32).wrapping_add(carry_in as i32);
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
        let product: u16 = self.al as u16 * operand1 as u16;

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
        let product: u32 = self.ax as u32 * operand1 as u32;

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
        let product: i16 = (self.al as i8 as i16) * (operand1 as i16);

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
        let product: i32 = (self.ax as i16 as i32) * (operand1 as i32);

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
        self.set_register16(Register16::DX, ((product as u32) >> 16 & 0xFFFF) as u16 );
        self.set_register16(Register16::AX, ((product as u32) & 0xFFFF) as u16 );        
    }

    // DIV r/m8 instruction
    // Divide can fail on div by 0 or overflow - (on which we would trigger an exception)
    pub fn divide_u8(&mut self, operand1: u8) -> bool {

        // Divide by 0 returns failure
        if operand1 == 0 {
            return false;
        }

        let quotient = self.ax / operand1 as u16;
        let remainder  = self.ax % operand1 as u16;

        if quotient & 0xFF00 != 0 {
            return false;
        }
        
        self.set_register8(Register8::AL, quotient as u8);
        self.set_register8(Register8::AH, remainder as u8);

        return true
    }

    // DIV r/m16 instruction
    // Divide can fail on div by 0 or overflow - (on which we would trigger an exception)
    pub fn divide_u16(&mut self, operand1: u16) -> bool {

        // Divide by 0 returns failure
        if operand1 == 0 {
            return false;
        }

        let dividend = (self.dx as u32) << 16 | self.ax as u32;

        let quotient = dividend / operand1 as u32;
        let remainder  = dividend % operand1 as u32;

        if quotient & 0xFFFF0000 != 0 {
            // Quotient overflow
            return false
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

        let dividend = self.ax as i16;

        let quotient = dividend / operand1 as i8 as i16;
        let remainder  = dividend % operand1 as i8 as i16;

        if quotient < i8::MIN as i16 || quotient > i8::MAX as i16 {
            // Quotient overflow
            return false
        }

        // TODO: should we return without modifying regs on failure?
        self.set_register8(Register8::AL, quotient as u8);
        self.set_register8(Register8::AH, remainder as u8);

        return true
    }

    // Signed DIV r/m16 instruction
    // Divide can fail on div by 0 or overflow - (on which we would trigger an exception)
    pub fn divide_i16(&mut self, operand1: u16) -> bool {

        // Divide by 0 returns failure
        if operand1 == 0 {
            return false;
        }

        let dividend: i32 = ((self.dx as u32) << 16 | self.ax as u32) as i32 ;

        // Double cast to sign-extend operand properly
        let quotient = dividend / operand1 as i16 as i32;
        let remainder  = dividend % operand1 as i16 as i32;

        if quotient < i16::MIN as i32 || quotient > i16::MAX as i32 {
            // Quotient overflow
            return false;
        }
        self.set_register16(Register16::AX, quotient as u16);
        self.set_register16(Register16::DX, remainder as u16);

        // Return false if overflow
        return true
    }

    /// Sign extend AL into AX
    pub fn sign_extend_al(&mut self) {

        if self.al & 0x80 != 0 {
            self.ah = 0xFF;
            self.ax |= 0xFF00;
        }
        else {
            self.ah = 0x00;
            self.ax &= 0x00FF;
        }
    }

    /// Sign extend AX ito DX:AX
    pub fn sign_extend_ax(&mut self) {

        if self.ax & 0x8000 != 0 {
            self.dx = 0xFFFF;
            self.dl = 0xFF;
            self.dh = 0xFF;
        }
        else {
            self.dx = 0x0000;
            self.dl = 0x00;
            self.dh = 0x00;
        }
    }

    /// Perform various 8-bit math operations
    pub fn math_op8(&mut self, opcode: Mnemonic, operand1: u8, operand2: u8) -> u8 {

        match opcode {
            Mnemonic::ADD => {
                let (result, carry, overflow, aux_carry) = Cpu::add_u8(operand1, operand2, false);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u8(result);
                result
            }
            Mnemonic::ADC => {
                // Get value of carry flag
                let carry_in = self.get_flag(Flag::Carry);
                // And pass it to ADC
                let (result, carry, overflow, aux_carry) = Cpu::add_u8(operand1, operand2, carry_in);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u8(result);
                result
            }
            Mnemonic::SUB => {
                let (result, carry, overflow, aux_carry) = Cpu::sub_u8(operand1, operand2, false );
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u8(result);
                result                
            }
            Mnemonic::SBB => {
                // Get value of carry flag
                let carry_in = self.get_flag(Flag::Carry);
                // And pass it to SBB                
                let (result, carry, overflow, aux_carry) = Cpu::sub_u8(operand1, operand2, carry_in );
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u8(result);
                result    
            }
            Mnemonic::NEG => {
                // Compute (0-operand)
                // Flags: The CF flag set to 0 if the source operand is 0; otherwise it is set to 1. 
                // The OF, SF, ZF, AF, and PF flags are set according to the result.
                let (result, _carry, overflow, aux_carry) = Cpu::sub_u8(0, operand1, false);
                
                self.set_flag_state(Flag::Carry, operand1 != 0);
                // NEG Updates AF, SF, PF, ZF
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u8(result);
                result
            }
            Mnemonic::INC => {
                // INC acts like add xx, 1, however does not set carry flag
                let (result, _carry, overflow, aux_carry) = Cpu::add_u8(operand1, 1, false);
                // DO NOT set carry Flag
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u8(result);
                result
            }
            Mnemonic::DEC => {
                // DEC acts like sub xx, 1, however does not set carry flag
                let (result, _carry, overflow, aux_carry) = Cpu::sub_u8(operand1, 1, false);
                // DEC does NOT set carry Flag
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u8(result);
                result
            }              
            Mnemonic::OR => {
                let result = operand1 | operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u8(result);
                result
            }
            Mnemonic::AND => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u8(result);
                result
            }
            Mnemonic::TEST => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u8(result);
                // TEST does not modify operand1
                operand1
            }
            Mnemonic::XOR => {
                let result = operand1 ^ operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u8(result);
                result
            }
            Mnemonic::NOT => {
                // Flags: None
                let result = !operand1;
                result
            }
            Mnemonic::CMP => {
                // CMP behaves like SUB except we do not store the result
                let (result, carry, overflow, aux_carry) = Cpu::sub_u8(operand1, operand2, false );
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u8(result);
                // Return the operand1 unchanged
                operand1
            }                        
            _=> panic!("cpu::math_op8(): Invalid opcode: {:?}", opcode)
        }
    }

    /// Perform various 16-bit math operations
    pub fn math_op16(&mut self, opcode: Mnemonic, operand1: u16, operand2: u16) -> u16 {

        match opcode {
            Mnemonic::ADD => {
                let (result, carry, overflow, aux_carry) = Cpu::add_u16(operand1, operand2, false);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u16(result);
                result
            }
            Mnemonic::ADC => {
                // Get value of carry flag
                let carry_in = self.get_flag(Flag::Carry);
                // And pass it to ADC
                let (result, carry, overflow, aux_carry) = Cpu::add_u16(operand1, operand2, carry_in);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u16(result);
                result
            }
            Mnemonic::SUB => {
                let (result, carry, overflow, aux_carry) = Cpu::sub_u16(operand1, operand2, false );
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u16(result);
                result                
            }
            Mnemonic::SBB => {
                // Get value of carry flag
                let carry_in = self.get_flag(Flag::Carry);
                // And pass it to SBB                
                let (result, carry, overflow, aux_carry) = Cpu::sub_u16(operand1, operand2, carry_in );
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u16(result);
                result    
            }
            Mnemonic::NEG => {
                // Compute (0-operand)
                // Flags: The CF flag set to 0 if the source operand is 0; otherwise it is set to 1. 
                // The OF, SF, ZF, AF, and PF flags are set according to the result.
                let (result, _carry, overflow, aux_carry) = Cpu::sub_u16(0, operand1, false);
                
                self.set_flag_state(Flag::Carry, operand1 != 0);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u16(result);
                result
            }            
            Mnemonic::INC => {
                // INC acts like add xx, 1, however does not set carry flag
                let (result, _carry, overflow, aux_carry) = Cpu::add_u16(operand1, 1, false);
                // INC does NOT set carry Flag
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u16(result);
                result
            }
            Mnemonic::DEC => {
                // DEC acts like sub xx, 1, however does not set carry flag
                let (result, _carry, overflow, aux_carry) = Cpu::sub_u16(operand1, 1, false);
                // DEC does NOT set carry Flag
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u16(result);
                result
            }            
            Mnemonic::OR => {
                let result = operand1 | operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u16(result);
                result
            }
            Mnemonic::AND => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u16(result);
                result
            }        
            Mnemonic::TEST => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u16(result);
                // Do not modify operand
                operand1  
            }    
            Mnemonic::XOR => {
                let result = operand1 ^ operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u16(result);
                result
            }
            Mnemonic::NOT => {
                // Flags: None
                let result = !operand1;
                result
            }            
            Mnemonic::CMP => {
                // CMP behaves like SUB except we do not store the result
                let (result, carry, overflow, aux_carry) = Cpu::sub_u16(operand1, operand2, false );
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u16(result);
                // Return the operand1 unchanged
                operand1
            }           
            _=> panic!("cpu::math_op16(): Invalid opcode: {:?}", opcode)
        }
    }    

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::CpuType;

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
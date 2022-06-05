use crate::cpu::{Cpu, Flag};
use crate::arch::{Opcode, Register8, Register16};

impl Cpu {

    fn calc_parity_flag_u8(operand: u8) -> bool {
        let mut bits_set = 0;
        for i in 0..8 {
            if operand & (0x01 << i) != 0 {
                bits_set += 1;
            }
        }
        // even number of bits set?
        bits_set % 2 == 0
    }

    fn calc_parity_flag_u16(operand: u16) -> bool {
        // Parity flag only ever looks at lowest 8 bits in operand
        Cpu::calc_parity_flag_u8(operand as u8)
    }

    fn set_parity_flag_from_u8(&mut self, operand: u8) {

        self.set_flag_state(Flag::Parity, Cpu::calc_parity_flag_u8(operand));
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

    fn set_parity_flag_from_u16(&mut self, operand: u16) {

        self.set_flag_state(Flag::Parity, Cpu::calc_parity_flag_u16(operand));
    }

    fn shl_u8_with_carry(mut byte: u8, mut count: u8) -> (u8, bool) {

        let mut carry = false;
        while count > 0 {
            carry = byte & 0x80 != 0;
            byte <<= 1;
            count -= 1;
        }
        (byte, carry)
    }    

    fn shl_u16_with_carry(mut word: u16, mut count: u8) -> (u16, bool) {
        let mut carry = false;
        while count > 0 {
            carry = word & 0x8000 != 0;
            word <<= 1;
            count -= 1;
        }
        (word, carry)
    }        

    fn shr_u8_with_carry(mut byte: u8, mut count: u8) -> (u8, bool) {

        let mut carry = false;
        while count > 0 {
            carry = byte & 0x01 != 0;
            byte >>= 1;
            count -= 1;
        }
        (byte, carry)
    }

    fn shr_u16_with_carry(mut word: u16, mut count: u8) -> (u16, bool) {

        let mut carry = false;
        while count > 0 {
            carry = word & 0x0001 != 0;
            word >>= 1;
            count -= 1;
        }
        (word, carry)
    }    

    fn rcr_u8_with_carry(mut byte: u8, mut count: u8, carry_flag: bool) -> (u8, bool) {

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

    fn rcr_u16_with_carry(mut word: u16, mut count: u8, carry_flag: bool) -> (u16, bool) {

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

    fn rcl_u8_with_carry(mut byte: u8, mut count: u8, carry_flag: bool) -> (u8, bool) {

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

    fn rcl_u16_with_carry(mut word: u16, mut count: u8, carry_flag: bool) -> (u16, bool) {

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

    fn ror_u8_with_carry(mut byte: u8, mut count: u8) -> (u8, bool) {
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

    fn ror_u16_with_carry(mut word: u16, mut count: u8) -> (u16, bool) {
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

    fn rol_u8_with_carry(mut byte: u8, mut count: u8 ) -> (u8, bool) {

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

    fn rol_u16_with_carry(mut word: u16, mut count: u8 ) -> (u16, bool) {

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

    fn sar_u8_with_carry(mut byte: u8, mut count: u8) -> (u8, bool) {

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

    fn sar_u16_with_carry(mut word: u16, mut count: u8) -> (u16, bool) {

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

        let sum_i16 = (byte1 as i16).wrapping_add( byte2 as i16).wrapping_add(carry_in as i16);
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

        let sum_i32 = (word1 as i32).wrapping_add(word2 as i32).wrapping_add(carry_in as i32);
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

        // do aux flag here

        let sum_u16 = (byte1 as u16).wrapping_sub(byte2 as u16);
        let sum_u16 = sum_u16.wrapping_sub(carry_in as u16);
        if sum_u16 > u8::MAX as u16 {
            // Unsigned overflow occurred
            carry = true;
        }

        let sum_i16 = (byte1 as i16).wrapping_sub(byte2 as i16);
        let sum_i16 = sum_i16.wrapping_sub(carry_in as i16);
        if sum_i16 > i8::MAX as i16 || sum_i16 < i8::MIN as i16 {
            // Signed overflow occurred
            overflow = true;
        }

        let sum = byte1.wrapping_sub(byte2);
        let sum = sum.wrapping_sub(carry_in as u8);
        (sum, carry, overflow, aux_carry)
    }

    // TODO: Handle Aux Carry Flag
    // TODO: Make overflow checks more efficient?
    pub fn sub_u16(word1: u16, word2: u16, carry_in: bool) -> (u16, bool, bool, bool) {
        // OVERFLOW flag indicates signed overflow
        // CARRY flag indicates unsigned overflow
        let mut carry = false;
        let mut overflow = false;
        let mut aux_carry = false;

        // do aux flag here

        let sum_u32 = (word1 as u32).wrapping_sub(word2 as u32);
        let sum_u32 = sum_u32.wrapping_sub(carry_in as u32);
        if sum_u32 > u16::MAX as u32 {
            // Unsigned overflow occurred
            carry = true;
        }

        let sum_i32 = (word1 as i32).wrapping_sub(word2 as i32);
        let sum_i32 = sum_i32.wrapping_sub(carry_in as i32);
        if sum_i32 > i16::MAX as i32 || sum_i32 < i16::MIN as i32 {
            // Signed overflow occurred
            overflow = true;
        }

        let sum = word1.wrapping_sub(word2);
        let sum = sum.wrapping_sub(carry_in as u16);
        (sum, carry, overflow, aux_carry)
    }

    pub fn multiply_u8(&mut self, operand1: u8) {
        
        // 8 bit operand => 16 bit product
        let product: u16 = self.al as u16 * operand1 as u16;

        // Set carry and overflow if product wouldn't fit in u8
        if product & 0xFF00 != 0 {
            self.set_flag(Flag::Carry);
            self.set_flag(Flag::Overflow);
        }

        // Note: Does not set Sign or Zero flags
        self.set_register16(Register16::AX, product);
    }    

    /// Unsigned Multiply, 16 bits
    pub fn multiply_u16(&mut self, operand1: u16) {
        
        // 16 bit operand => 32bit product
        let product: u32 = self.ax as u32 * operand1 as u32;

        // Set carry and overflow if product wouldn't fit in u16
        if product & 0xFFFF0000 != 0 {
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
    pub fn multiply_i8(&mut self, operand1: i8) {
        
        // 8 bit operand => 16 bit product
        let product: i16 = (self.al as i8 as i16) * (operand1 as i16);

        // Set carry and overflow if product wouldn't fit in i8
        if product < i8::MIN.into() || product > i8::MAX.into() {
            self.set_flag(Flag::Carry);
            self.set_flag(Flag::Overflow);
        }

        // Note: Does not set Sign or Zero flags
        self.set_register16(Register16::AX, product as u16);
    }  

    /// Signed Multiply, 16 bits
    pub fn multiply_i16(&mut self, operand1: i16) {

        // 16 bit operand => 32 bit product
        let product: i32 = (self.ax as i16 as i32) * (operand1 as i32);

        // Set carry and overflow if product wouldn't fit in i16
        if product < i16::MIN.into() || product > i16::MAX.into() {
            self.set_flag(Flag::Carry);
            self.set_flag(Flag::Overflow);
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

        // TODO: should we return without modifying AL on failure??
        self.set_register8(Register8::AL, quotient as u8);
        self.set_register8(Register8::AH, remainder as u8);

        // Return false if overflow
        return quotient & 0xFF00 == 0;
    }

    // DIV r/m16 instruction
    // Divide can fail on div by 0 or overflow - (on which we would trigger an exception)
    pub fn divide_u16(&mut self, operand1: u16) -> bool {

        // Divide by 0 returns failure
        if operand1 == 0 {
            return false;
        }

        let numerator = (self.dx as u32) << 16 | self.ax as u32;

        let quotient = numerator / operand1 as u32;
        let remainder  = numerator % operand1 as u32;

        // TODO: should we return without modifying AL on failure??
        self.set_register16(Register16::AX, quotient as u16);
        self.set_register16(Register16::DX, remainder as u16);

        // Return false if overflow
        return quotient & 0xFFFF0000 == 0;
    }

    // Integer DIV r/m16 instruction
    // Divide can fail on div by 0 or overflow - (on which we would trigger an exception)
    pub fn divide_i16(&mut self, operand1: u16) -> bool {

        // Divide by 0 returns failure
        if operand1 == 0 {
            return false;
        }

        let numerator: i32 = ((self.dx as u32) << 16 | self.ax as u32) as i32 ;

        let quotient = numerator / operand1 as i32;
        let remainder  = numerator % operand1 as i32;

        // TODO: should we return without modifying AL on failure??
        self.set_register16(Register16::AX, quotient as u16);
        self.set_register16(Register16::DX, remainder as u16);

        // Return false if overflow
        return quotient as u32 & 0xFFFF0000 == 0;
    }

    /// Sign extend AL into AX
    pub fn sign_extend_al(&mut self) {

        if self.al & 0x80 != 0 {
            self.ah = 0xFF;
            self.ax &= 0xFF00;
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
    pub fn math_op8(&mut self, opcode: Opcode, operand1: u8, operand2: u8) -> u8 {

        match opcode {
            Opcode::ADD => {
                let (result, carry, overflow, aux_carry) = Cpu::add_u8(operand1, operand2, false);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u8(result);
                result
            }
            Opcode::ADC => {
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
            Opcode::SUB => {
                let (result, carry, overflow, aux_carry) = Cpu::sub_u8(operand1, operand2, false );
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u8(result);
                result                
            }
            Opcode::SBB => {
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
            Opcode::NEG => {
                // Compute (0-operand)
                // AoA 6.5.5
                let (result, _carry, _overflow, aux_carry) = Cpu::sub_u8(0, operand1, false);
                
                // NEG Updates AF, SF, PF, ZF
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u8(result);
                result
            }
            Opcode::INC => {
                // INC acts like add xx, 1, however does not set carry flag
                let (result, _carry, overflow, aux_carry) = Cpu::add_u8(operand1, 1, false);
                // DO NOT set carry Flag
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u8(result);
                result
            }
            Opcode::DEC => {
                // DEC acts like sub xx, 1, however does not set carry flag
                let (result, _carry, overflow, aux_carry) = Cpu::sub_u8(operand1, 1, false);
                // DEC does NOT set carry Flag
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u8(result);
                result
            }              
            Opcode::OR => {
                let result = operand1 | operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u8(result);
                result
            }
            Opcode::AND => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u8(result);
                result
            }
            Opcode::TEST => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u8(result);
                // TEST does not modify operand1
                operand1
            }
            Opcode::XOR => {
                let result = operand1 ^ operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u8(result);
                result
            }
            Opcode::NOT => {
                let result = !operand1;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u8(result);
                result
            }
            Opcode::CMP => {
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
    pub fn math_op16(&mut self, opcode: Opcode, operand1: u16, operand2: u16) -> u16 {

        match opcode {
            Opcode::ADD => {
                let (result, carry, overflow, aux_carry) = Cpu::add_u16(operand1, operand2, false);
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u16(result);
                result
            }
            Opcode::ADC => {
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
            Opcode::SUB => {
                let (result, carry, overflow, aux_carry) = Cpu::sub_u16(operand1, operand2, false );
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u16(result);
                result                
            }
            Opcode::SBB => {
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
            Opcode::NEG => {
                // Compute (0-operand)
                // AoA 6.5.5
                let (result, _carry, _overflow, aux_carry) = Cpu::sub_u16(0, operand1, false);
                
                // NEG Updates AF, SF, PF, ZF
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u16(result);
                result
            }            
            Opcode::INC => {
                // INC acts like add xx, 1, however does not set carry flag
                let (result, _carry, overflow, aux_carry) = Cpu::add_u16(operand1, 1, false);
                // INC does NOT set carry Flag
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u16(result);
                result
            }
            Opcode::DEC => {
                // DEC acts like sub xx, 1, however does not set carry flag
                let (result, _carry, overflow, aux_carry) = Cpu::sub_u16(operand1, 1, false);
                // DEC does NOT set carry Flag
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u16(result);
                result
            }            
            Opcode::OR => {
                let result = operand1 | operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u16(result);
                result
            }
            Opcode::AND => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u16(result);
                result
            }        
            Opcode::TEST => {
                let result = operand1 & operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u16(result);
                // Do not modify operand
                operand1  
            }    
            Opcode::XOR => {
                let result = operand1 ^ operand2;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u16(result);
                result
            }
            Opcode::NOT => {
                let result = !operand1;
                // Clear carry, overflow
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u16(result);
                result
            }            
            Opcode::CMP => {
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

    /// Perform various 8-bit binary shift operations
    pub fn bitshift_op8(&mut self, opcode: Opcode, operand1: u8, operand2: u8) -> u8 {

        // Operand2 will either be 1 or value of CL register on 8088
        if operand2 == 0 {
            // Flags are not changed if shift amount is 0
            return operand1;
        }

        let result: u8;
        let carry: bool;

        match opcode {
            Opcode::ROL => {
                (result, carry) = Cpu::rol_u8_with_carry(operand1, operand2);
                self.set_flag_state(Flag::Carry, carry);
                // Only set overflow on ROL of 1
                if operand2 == 1 {
                    // Only set overflow if HO bit changed
                    self.set_flag_state(Flag::Overflow, (operand1 & 0x80) != (result & 0x80));
                }
            }
            Opcode::ROR => {
                (result, carry) = Cpu::ror_u8_with_carry(operand1, operand2);
                self.set_flag_state(Flag::Carry, carry);
                // Only set overflow on ROR of 1
                if operand2 == 1 {
                    // Only set overflow if HO bit changed
                    self.set_flag_state(Flag::Overflow, (operand1 & 0x80) != (result & 0x80));
                }
            }
            Opcode::RCL => {
                let existing_carry = self.get_flag(Flag::Carry);
                (result, carry) = Cpu::rcl_u8_with_carry(operand1, operand2, existing_carry);
                self.set_flag_state(Flag::Carry, carry);
                // Only set overflow on SHL of 1
                if operand2 == 1 {
                    // Set overflow if HO bit and Carry flag are different
                    self.set_flag_state(Flag::Overflow, (operand1 & 0x80) != (result & 0x80))
                }
            }
            Opcode::RCR => {
                let existing_carry = self.get_flag(Flag::Carry);
                (result, carry) = Cpu::rcr_u8_with_carry(operand1, operand2, existing_carry);
                self.set_flag_state(Flag::Carry, carry);
                // Only set overflow on SHL of 1
                if operand2 == 1 {
                    // Set overflow if HO bit and Carry flag are different
                    let overflow = (operand1 & 0x80 == 0 && existing_carry) || (operand1 & 0x80 != 0 && !existing_carry);
                    self.set_flag_state(Flag::Overflow, overflow)
                }
                // The rcr instruction does not affect the zero, sign, parity, or auxiliary carry flags.
                // AoA 6.6.3.2
            }
            Opcode::SHL => {
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
            Opcode::SHR => {
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
            Opcode::SAR => {
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
    pub fn bitshift_op16(&mut self, opcode: Opcode, operand1: u16, operand2: u8) -> u16 {

        // Operand2 will either be 1 or value of CL register on 8088
        if operand2 == 0 {
            // Flags are not changed if shift amount is 0
            return operand1;
        }

        let result: u16;
        let carry: bool;

        match opcode {
            Opcode::ROL => {
                (result, carry) = Cpu::rol_u16_with_carry(operand1, operand2);
                self.set_flag_state(Flag::Carry, carry);
                // Only set overflow on ROL of 1
                if operand2 == 1 {
                    // Only set overflow if HO bit changed
                    self.set_flag_state(Flag::Overflow, (operand1 & 0x8000) != (result & 0x8000));
                }
            }
            Opcode::ROR => {
                (result, carry) = Cpu::ror_u16_with_carry(operand1, operand2);
                self.set_flag_state(Flag::Carry, carry);
                // Only set overflow on ROR of 1
                if operand2 == 1 {
                    // Only set overflow if HO bit changed
                    self.set_flag_state(Flag::Overflow, (operand1 & 0x8000) != (result & 0x8000));
                }
            }
            Opcode::RCL => {
                let existing_carry = self.get_flag(Flag::Carry);
                (result, carry) = Cpu::rcl_u16_with_carry(operand1, operand2, existing_carry);
                self.set_flag_state(Flag::Carry, carry);
                // Only set overflow on SHL of 1
                if operand2 == 1 {
                    // Set overflow if HO bit and Carry flag are different
                    self.set_flag_state(Flag::Overflow, (operand1 & 0x80) != (result & 0x80))
                }
            }
            Opcode::RCR => {
                let existing_carry = self.get_flag(Flag::Carry);
                (result, carry) = Cpu::rcr_u16_with_carry(operand1, operand2, existing_carry);
                self.set_flag_state(Flag::Carry, carry);
                // Only set overflow on SHL of 1
                if operand2 == 1 {
                    // Set overflow if HO bit and Carry flag are different
                    let overflow = (operand1 & 0x8000 == 0 && existing_carry) || (operand1 & 0x8000 != 0 && !existing_carry);
                    self.set_flag_state(Flag::Overflow, overflow)
                }
                // The rcr instruction does not affect the zero, sign, parity, or auxiliary carry flags.
                // AoA 6.6.3.2
            }
            Opcode::SHL => {
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
            Opcode::SHR => {
                (result, carry) = Cpu::shr_u16_with_carry(operand1, operand2);
                // Set state of Carry Flag
                self.set_flag_state(Flag::Carry, carry);

                // Only set overflow on SHR of 1
                if operand2 == 1 {
                    // Only time SHR sets overflow is if HO was 1 and becomes 0, which it always will,
                    // so set overflow flag if it was set. 
                    self.set_flag_state(Flag::Overflow, operand1 & 0x80 != 0 );
                }
                self.set_flags_from_result_u16(result);
            }
            Opcode::SAR => {
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

    #[test]
    
    fn test_mul() {
        let mut cpu = Cpu::new(0);

        cpu.set_register16(Register16::AX, 1);

        for i in 0..7 {
            cpu.multiply_u8(2);
        }
        assert_eq!(cpu.al, 128);
        cpu.multiply_u8(2);
        assert_eq!(cpu.ax, 256);

        cpu.set_register16(Register16::AX, 1);

        for i in 0..15 {
            cpu.multiply_u16(2);
        }
        assert_eq!(cpu.ax, 32768);
        cpu.multiply_u16(2);
        assert_eq!(cpu.ax, 0);
        assert_eq!(cpu.dx, 1); // dx will contain overflow from ax @ 65536
    }
}
use crate::cpu::*;

impl<'a> Cpu<'a> {

    pub fn neg_u16(a: u16) -> (u16, bool) {
    
        let sigma;
        let carry;
    
        //sigma = 0u16.wrapping_sub(a);
        (sigma, carry, _, _) = Cpu::sub_u16(0, a, false);
        //carry = if (a & 0x8000) != (sigma & 0x8000) { true } else { false };
        
        (sigma, carry)
    }

    /// Microcode routine for multiplication, 8 bit
    /// Accepts tmpb, tmpc, returns tmpa, tmpc, cycle count
    /// 
    /// This microcode routine is described in detail in Intel's patent 4,363,091 Section 31.
    //              "Microcode instructions 3 through 11 and CORX represent the
    //               execution of the normal binary multiplication algorithm
    //               wherein the TMP A is used as an accumulator."
    pub fn corx8(&mut self, b: u8, c: u8, mut carry: bool) -> (u16, u16) {

        let mut internal_counter;

        let tmpb: u16 = b as u16;
        let mut tmpc: u16 = c as u16;
        let mut tmpa: u16; 
        let mut sigma8: u8;

        (sigma8, carry) = Cpu::rcr_u8_with_carry(tmpc as u8, 1, carry); // 17f: ZERO->tmpa  | RRCY tmpc
        tmpa = 0; 
        tmpc = sigma8 as u16; // 180: SIGMA->tmpc
        internal_counter = 7; // 180: MAXC
        self.cycles_i(2, &[0x17f, 0x180]);

        // The main corx loop is between 181-186.
        loop { 

            self.cycle_i(0x181); // 181:    | NCY 8 (jump if no carry)

            if carry {
                (sigma8, carry, _, _ ) = Cpu::add_u8(tmpa as u8, tmpb as u8, false); // 182:             | ADD tmpa 
                tmpa = sigma8 as u16; // 183: SIGMA->tmpa    | F
                self.cycles_i(2, &[0x182, 0x183]);
                // SET FLAGS HERE
            }
            else {
                // Jump delay for skipping to line 8
                self.cycle_i(MC_JUMP);
            }
        
            (sigma8, carry) = Cpu::rcr_u8_with_carry(tmpa as u8, 1, carry); // 184:             | RRCY tmpa
            tmpa = sigma8 as u16; // 185
            (sigma8, carry) = Cpu::rcr_u8_with_carry(tmpc as u8, 1, carry); // 185: SIGMA->tmpa | RRCY tmpc
            tmpc = sigma8 as u16; // 186: SIGMA->tmpc | NCZ 5

            self.cycles_i(3, &[0x184, 0x185, 0x186]);

            if internal_counter == 0 {
                break; // 186: no-jump
            }

            // It's not explicitly explained where the internal counter is updated.
            // I am just assuming it is decremented once per loop here.        
            internal_counter -= 1;

            self.cycle_i(MC_JUMP); // 186: (jump) 1 cycle delay to return to top of loop.
        }

        // Fall through line 186
        self.cycles_i(2, &[0x187, MC_JUMP]); // 187 'RTN', return delay cycle

        (tmpa, tmpc)
    }

    /// Microcode routine for multiplication, 16 bit
    /// Accepts tmpb, tmpc, returns tmpa, tmpc, cycle count
    /// 
    /// This microcode routine is described in detail in Intel's patent 4,363,091 Section 31.
    //              "Microcode instructions 3 through 11 and CORX represent the
    //               execution of the normal binary multiplication algorithm
    //               wherein the TMP A is used as an accumulator."
    pub fn corx16(&mut self, b: u16, c: u16, mut carry: bool) -> (u16, u16) {

        let mut internal_counter;
        let mut sigma;

        let tmpb: u16 = b;
        let mut tmpc: u16 = c;
        let mut tmpa: u16; // 17f

        (sigma, carry) = Cpu::rcr_u16_with_carry(tmpc, 1, carry); // 17f: ZERO->tmpa  | RRCY tmpc
        tmpa = 0; 
        tmpc = sigma; // 180: SIGMA->tmpc
        internal_counter = 15; // 180: MAXC
        self.cycles_i(2, &[0x17f, 0x180]);

        // The main corx loop is between 181-186.
        loop { 
        
            self.cycle_i(0x181); // 181:    | NCY 8 (jump if no carry)

            if carry {
                (sigma, carry, _, _ ) = Cpu::add_u16(tmpa, tmpb, false); // 182:             | ADD tmpa 
                tmpa = sigma; // 183 SIGMA->tmpa | F
                self.cycles_i(2, &[0x182, 0x183]);
                // SET FLAGS HERE
            }
            else {
                // Jump delay for skipping to line 8
                self.cycle_i(MC_JUMP);
            }
        
            (sigma, carry) = Cpu::rcr_u16_with_carry(tmpa, 1, carry); // 184:             | RRCY tmpa
            tmpa = sigma; // 185
            (sigma, carry) = Cpu::rcr_u16_with_carry(tmpc, 1, carry); // 185: SIGMA->tmpa | RRCY tmpc
            tmpc = sigma;  // 186: SIGMA->tmpc | NCZ 5

            self.cycles_i(3, &[0x184, 0x185, 0x186]);

            if internal_counter == 0 {
                break; // 186: no-jump
            }

            // It's not explicitly explained where the internal counter is updated.
            // I am just assuming it is decremented once per loop here.        
            internal_counter -= 1;

            self.cycle_i(MC_JUMP); // 186: (jump) 1 cycle delay to return to top of loop.
        }

        // Fall through line 186
        self.cycles_i(2, &[0x187, MC_JUMP]); // 187 'RTN', return delay cycle

        (tmpa, tmpc)
    }


    /// Microcode routine for multiplication, 8 bit
    /// Accepts al and 8-bit operand, returns 16 bit product (for AX)
    pub fn mul8(&mut self, al: u8, operand: u8, signed: bool, mut negate: bool) -> (u16, u16) {

        let mut mul_cycles = 0;
        let mut sigma: u16;
        let sigma8: u8;

        let mut tmpa: u16;
        let mut tmpc: u16 = al as u16; // 150 A->tmpc     | LRCY tmpc
        let mut carry;
        let zf;

        //(_, carry) = rcl_u8_with_carry(tmpc as u8, 1, carry); 
        carry = tmpc & 0x80 != 0; // LRCY is just checking MSB of tmpc
        let mut tmpb: u16 = operand as u16; // 151: M->tmpb    | X0 PREIMUL
        self.cycles_i(2, &[0x150, 0x151]);

        // PREIMUL if signed == true
        // -------------------------------------------------------------------------
        if signed {
            // JMP PREIMUL
            (sigma, _) = Cpu::neg_u16(tmpc); // 1c0: SIGMA->.   | NEG tmpc
            // 1c1             | NCY 7

            self.cycles_i(3, &[MC_JUMP, 0x1c0, 0x1c1]);

            if carry {
                tmpc = sigma;
                negate = !negate; // 1c2: SIGMA->tmpc | CF1   (flip F1 flag)
                self.cycles_i(3, &[0x1c2, 0x1c3, MC_JUMP]);
            }
            else {
                self.cycle_i(MC_JUMP);
            }

            // 1bb:              | LRCY tmpb
            // 1bc: SIGMA->tmpb  | NEG tmpb
            //(_, carry) = rcl_u8_with_carry(tmpb as u8, 1, carry); // Set carry flag if operand is negative
            carry = tmpb & 0x80 != 0; // LRCY is just checking msb of tmpb
            (sigma, _) = Cpu::neg_u16(tmpb);

            self.cycles_i(2, &[0x1bb, 0x1bc]);

            // 1bd:             | NCY 11
            if !carry {
                // Operand is positive
                self.cycles_i(2, &[0x1bd, MC_JUMP]); // 1bf:         | RTN
            }
            else {
                // Operand is negative
                tmpb = sigma; // 1be: SIGMA->tmpb  | CF1 RTN
                negate = !negate;
                self.cycle_i(0x1be);
            }
            // RTN delay
            self.cycle_i(MC_JUMP);
        }

        // 152:            | UNC CORX
        let (accum, tmpc8) = self.corx8(tmpb as u8, tmpc as u8, carry);
        //println!("corx: {}, {}", accum, tmpc8);
        tmpa = accum as u16;
        tmpc = tmpc8 as u16;

        self.cycles_i(2, &[0x152, MC_JUMP]);

        // 153:            | F1 NEGATE  (REP prefix negates product)
        // NEGATE if REP
        // -------------------------------------------------------------------------
        if negate {
            self.cycle_i(MC_JUMP); // Jump to NEGATE

            //sigma = -(tmpc as  i8) as u8;
            //carry = if (tmpc & 0x80) != (sigma & 0x80) { true } else { false };

            (sigma, carry) = Cpu::neg_u16(tmpc); // 1b6

            //println!("corx_carry: {} neg_carry: {}", corx_carry, carry);
            tmpc = sigma;

            if carry {
                sigma = !tmpa; // 1b8, jump, 1ba: SIGMA->tmpa | CF1 
                self.cycles_i(5, &[0x1b6, 0x1b7, 0x1b8, MC_JUMP, 0x1ba]);
            }
            else {
                (sigma, _) = Cpu::neg_u16(tmpa); // 1b8, 1b9, 1ba: SIGMA->tmpa | CF1 
                self.cycles_i(5, &[0x1b6, 0x1b7, 0x1b8, 0x1b9, 0x1ba]);
            }

            tmpa = sigma; // 1ba
            //negate = !negate;

            // 1bb:     | LRCY tmpb
            // 1bc: SIGMA->tmpb  | NEG tmpb
            //(_, carry) = rcl_u8_with_carry(tmpb as u8, 1, carry); // Set carry flag if tmpb is negative
            carry = tmpb & 0x80 != 0; // LRCY is just checking msb of tmpb
            (_, _) = Cpu::neg_u16(tmpb as u16);

            self.cycles_i(2, &[0x1bb, 0x1bc]);

            // 1bd:             | NCY 11
            if !carry { 
                // tmpb was positive, Jump to 11
                self.cycles_i(4, &[0x1bd, MC_JUMP, 0x1bf, MC_JUMP]); // RTN delay
            }
            else {
                // tmpb was negative
                // 1be: SIGMA->tmpb  | CF1 RTN
                //tmpb = sigma; (unused after PREIMUL)
                //negate = !negate; (unused after PREIMUL)
                self.cycles_i(3, &[0x1bd, 0x1be, MC_JUMP]); // RTN delay
            }
        }

        // 154:                | X0 IMULCOF
        // IMULFCOF if signed
        // -------------------------------------------------------------------------
        self.cycle_i(0x154);

        if signed {

            self.cycle_i(MC_JUMP); // JMP
            tmpb = 0; 
            //(_, carry) = rcl_u8_with_carry(tmpc as u8, 1, carry);  // Test if tmpc is negative
            carry = tmpc & 0x80 != 0; // LRCY is just checking msb of tmpc
            (sigma8, _, _, _) = Cpu::add_u8(tmpa as u8, tmpb as u8, carry);
            self.cycles_i(2, &[0x1ce, 0x1cf]);
            // SET FLAGS HERE

            // 1d0:             | Z 8
            if sigma8 == 0 {
                // CLEAR CARRY, OVERFLOW FLAGS HERE
                self.cycles_i(4, &[0x1d0, MC_JUMP, 0x1cc, MC_JUMP]);
            }
            else {
                // 1d1:              | SCOF RTN
                // SET CARRY, OVERFLOW FLAGS HERE
                self.cycles_i(3 , &[0x1d0, 0x1d1, MC_JUMP]);
            }

            // 155: tmpc -> A      | X0 7
            // JUMP
            // 157: tmpa -> X      | RNI
            self.cycles_i(3, &[0x155, MC_JUMP, 0x157]);
            let product = tmpa << 8 | (tmpc & 0xFF); 
            return (product, mul_cycles)        
        }

        // 155: tmpc -> A      | X0 7 
        // 156:                | UNC MULCOF
        // JMP

        // MULCOF
        // -------------------------------------------------------------------------
        // 1d2:                | PASS tmpa (tmpa->sigma)
        sigma = tmpa;

        // 1d3: SIGMA->.       | UNC 12  | F  (Set flags)
        // JMP

        self.cycles_i(6, &[0x155, 0x156, MC_JUMP, 0x1d2, 0x1d3, MC_JUMP]);
        zf = sigma == 0;

        // 1d0:                | Z 8  (jump if zero)
        if zf {
            // JMP
            // 1cc:             | CCOF RTN
            // Clear carry & overflow flags here
            self.cycles_i(4, &[0x1d0, MC_JUMP, 0x1cc, MC_JUMP]);
        }
        else {
            // 1d1:             | SCOF RTN
            // Set carry & overflow flags here
            self.cycles_i(3, &[0x1d0, 0x1d1, MC_JUMP]);
        }

        self.cycle_i(0x157); // 157: tmpa-> X        | RNI

        let product = tmpa << 8 | (tmpc & 0xFF); 
        (product, mul_cycles)
    }

    /// Microcode routine for multiplication, 16 bit
    /// Accepts ax and 16-bit operand, returns 32 bit product in two parts (for DX:AX)
    pub fn mul16(&mut self, ax: u16, operand: u16, signed: bool, mut negate: bool) -> (u16, u16, u16) {

        let mut mul_cycles = 0;
        let mut sigma: u16;

        let mut tmpa: u16;
        let mut tmpc: u16 = ax; // 158 XA->tmpc     | LRCY tmpc
        let mut carry;
        let zf;

        //(_, carry) = rcl_u16_with_carry(tmpc, 1, carry); // SIGMA isn't used? Just setting carry flag(?)
        carry = tmpc & 0x8000 != 0; // LRCY is just checking msb
        let mut tmpb: u16 = operand as u16; // 159: M->tmpb    | X0 PREIMUL
        self.cycles_i(2, &[0x158, 0x159]);

        // PREIMUL if signed == true
        // -------------------------------------------------------------------------
        if signed {
            // JMP PREIMUL
            (sigma, _) = Cpu::neg_u16(tmpc); // 1c0: SIGMA->.   | NEG tmpc
            // 1c1             | NCY 7
            self.cycles_i(3, &[MC_JUMP, 0x1c0, 0x1c1]);

            if carry {
                tmpc = sigma;
                negate = !negate; // 1c2: SIGMA->tmpc | CF1   (flip F1 flag)
                self.cycles_i(3, &[0x1c2, 0x1c3, MC_JUMP]);
            }
            else {
                self.cycle_i(MC_JUMP);
            }

            // 1bb:              | LRCY tmpb
            // 1bc: SIGMA->tmpb  | NEG tmpb
            //(_, carry) = rcl_u16_with_carry(tmpb, 1, carry); // Test if operand is negative
            carry = tmpb & 0x8000 != 0; // LRCY is just checking msb
            (sigma, _) = Cpu::neg_u16(tmpb);

            self.cycles_i(2, &[0x1bb, 0x1bc]);

            // 1bd:             | NCY 11
            if !carry {
                // Jump to 11
                // 1bf:         | RTN
                self.cycles_i(2, &[0x1bd, MC_JUMP]); // 1bf:         | RTN
            }
            else {
                // 1be: SIGMA->tmpb  | CF1 RTN
                tmpb = sigma;
                negate = !negate;
                self.cycle_i(0x1be);
            }
            // RTN delay
            self.cycle_i(MC_JUMP);
        }

        // 15a:            | UNC CORX
        (tmpa, tmpc) = self.corx16(tmpb, tmpc, carry);
        self.cycles_i(2, &[0x15a, MC_JUMP]);

        // 15b:            | F1 NEGATE  (REP prefix negates product)
        // NEGATE if REP
        // -------------------------------------------------------------------------
        if negate {
            self.cycle_i(MC_JUMP); // Jump to NEGATE

            //sigma = -(tmpc as  i8) as u8;
            //carry = if (tmpc & 0x80) != (sigma & 0x80) { true } else { false };
            (sigma, carry) = Cpu::neg_u16(tmpc);

            tmpc = sigma;

            if carry {
                sigma = !tmpa; // 1b8, jump, 1ba: SIGMA->tmpa | CF1 
                self.cycles_i(5, &[0x1b6, 0x1b7, 0x1b8, MC_JUMP, 0x1ba]);
            }
            else {
                (sigma, _) = Cpu::neg_u16(tmpa); // 1b8, 1b9, 1ba: SIGMA->tmpa | CF1 
                self.cycles_i(5, &[0x1b6, 0x1b7, 0x1b8, 0x1b9, 0x1ba]);
            }

            tmpa = sigma; // 1ba
            //negate = !negate;
            mul_cycles += 3;

            // 1bb:     | LRCY tmpb
            mul_cycles += 1;

            // 1bc: SIGMA->tmpb  | NEG tmpb
            //(_, carry) = rcl_u8_with_carry(tmpb as u8, 1, carry);
            carry = tmpb & 0x8000 != 0; // LRCY is just checking msb
            (_, _) = Cpu::neg_u16(tmpb as u16);

            self.cycles_i(2, &[0x1bb, 0x1bc]);

            // 1bd:             | NCY 11
            if !carry {
                // tmpb was positive, Jump to 11
                self.cycles_i(4, &[0x1bd, MC_JUMP, 0x1bf, MC_JUMP]); // RTN delay
            }
            else {
                // tmpb was negative
                // 1be: SIGMA->tmpb  | CF1 RTN
                //tmpb = sigma; (unused after PREIMUL)
                //negate = !negate; (unused after PREIMUL)
                self.cycles_i(3, &[0x1bd, 0x1be, MC_JUMP]); // RTN delay
            }
        }

        // 15c:                | X0 IMULCOF
        // IMULFCOF if signed
        // -------------------------------------------------------------------------
        self.cycle_i(0x15c);

        if signed {

            self.cycle_i(MC_JUMP); // JMP
            tmpb = 0; 
            //(_, carry) = rcl_u16_with_carry(tmpc, 1, carry);  // Test if tmpc is negative
            carry = tmpc & 0x8000 != 0; // LRCY is just checking msb of tmpc
            (sigma, _, _, _) = Cpu::add_u16(tmpa, tmpb, carry);
            self.cycles_i(2, &[0x1ce, 0x1cf]);
            // Set flags here

            // 1d0:             | Z 8
            if sigma == 0 {
                // CLEAR CARRY, OVERFLOW FLAGS HERE
                self.cycles_i(4, &[0x1d0, MC_JUMP, 0x1cc, MC_JUMP]);
            }
            else {
                // 1d1:              | SCOF RTN
                // SET CARRY, OVERFLOW FLAGS HERE
                self.cycles_i(3 , &[0x1d0, 0x1d1, MC_JUMP]);
            }

            // 15d: tmpc -> A      | X0 7
            // JUMP
            // 15f: tmpa -> X      | RNI
            self.cycles_i(3, &[0x15d, MC_JUMP, 0x15f]);
            return (tmpa, tmpc, mul_cycles)            
        }
        // 15d: tmpc -> A      | X0 7 
        // 15e:                | UNC MULCOF
        // JMP

        // MULCOF
        // -------------------------------------------------------------------------
        // 1d2:                | PASS tmpa (tmpa->sigma)
        sigma = tmpa;
        // 1d3: SIGMA->.       | UNC 12  | F  (Set flags)
        // JMP
        self.cycles_i(6, &[0x15d, 0x15e, MC_JUMP, 0x1d2, 0x1d3, MC_JUMP]);
        zf = sigma == 0;

        // 1d0:                | Z 8  (jump if zero)
        if zf {
            // JMP
            // 1cc:             | CCOF RTN
            // Clear carry & overflow flags here
            self.cycles_i(4, &[0x1d0, MC_JUMP, 0x1cc, MC_JUMP]);
        }
        else {
            // 1d1:             | SCOF RTN
            // Set carry & overflow flags here
            self.cycles_i(3, &[0x1d0, 0x1d1, MC_JUMP]);
        }

        // 157: tmpa-> X        | RNI
        self.cycle_i(0x15f);

        (tmpa, tmpc, mul_cycles)
    }

}
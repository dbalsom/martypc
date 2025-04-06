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

    cpu_vx0::muldiv.rs

    This module implements the microcode algorithm for multiplication and
    division on the 8088 for accurate cycle timings.

*/

use crate::{cpu_common::alu::*, cpu_vx0::*};
pub trait Cord<B = Self>: Sized {
    fn cord(self, cpu: &mut NecVx0, a: u16, b: u16, c: u16) -> Result<(u16, u16, bool), bool>;
}

macro_rules! impl_cord {
    ($prim:ty) => {
        impl Cord for $prim {
            /// Implementation of the 8088 microcode CORD division co-routine.
            /// Implemented for either 8 bit or 16 bit operand.
            fn cord(self, cpu: &mut NecVx0, a: u16, b: u16, c: u16) -> Result<(u16, u16, bool), bool> {
                let mut internal_counter;

                let mut tmpa: u16 = a;
                let tmpb: u16 = b;
                let mut tmpc: u16 = c;
                let mut sigma: u16;
                let mut sigma_s: Self;

                let mut carry;
                let mut carry_sub;

                // 188:           | SUBT tmpa
                (_, carry, _, _) = (tmpa as Self).alu_sub(tmpb as Self);
                // 189: SIGMA->.  | MAXC
                internal_counter = Self::BITS;

                cpu.cycles_i(3, &[0x188, 0x189, 0x18a]);

                // 18a:           | NCY INT0
                if !carry {
                    // Jump delay to INT0 procedure
                    cpu.cycle_i(MC_JUMP);
                    //log::debug!("cord: div overflow");
                    return Err(false);
                }

                // The main CORD loop is between 18b and 196.
                while internal_counter > 0 {
                    //println!("ic: {} tmpa: {} tmpb: {} tmpc: {}", internal_counter, tmpa, tmpb, tmpc);
                    // 18b:
                    (sigma_s, carry, _) = (tmpc as Self).alu_rcl(1, carry);
                    tmpc = sigma_s as u16;

                    // 18c:
                    (sigma_s, carry, _) = (tmpa as Self).alu_rcl(1, carry);

                    // 18d:
                    tmpa = sigma_s as u16;
                    (sigma_s, carry_sub, _, _) = (tmpa as Self).alu_sub(tmpb as Self);
                    sigma = sigma_s as u16;

                    cpu.cycles_i(4, &[0x18b, 0x18c, 0x18d, 0x18e]);

                    // 18e:
                    if carry {
                        // Jump delay
                        cpu.cycles_i(3, &[MC_JUMP, 0x195, 0x196]);
                        // 195:
                        carry = false;
                        // 196: SIGMA->tmpa  | NCZ 3
                        tmpa = sigma;
                        internal_counter -= 1;
                        if internal_counter > 0 {
                            // 196: SIGMA->tmpa  | NCZ 3
                            cpu.cycle_i(MC_JUMP);
                            //println!("  cord(): in CORD: tmpa: {:04x} tmpc: {:04x}", tmpa, tmpc);
                            continue;
                        }
                        else {
                            // Continue to 197:
                            cpu.cycles_i(2, &[0x197, MC_JUMP]);
                        }
                    }
                    else {
                        // 18f: SIGMA->.     | F
                        carry = carry_sub;
                        // SET FLAGS HERE

                        cpu.cycles_i(2, &[0x18f, 0x190]);

                        // 190:    NCY 14
                        if !carry {
                            // JMP delay

                            // 196: SIGMA->tmpa    | NCZ 3
                            tmpa = sigma;
                            cpu.cycles_i(2, &[MC_JUMP, 0x196]);
                            internal_counter -= 1;
                            if internal_counter > 0 {
                                // 196: SIGMA->tmpa    | NCZ 3
                                cpu.cycle_i(MC_JUMP);
                                //println!("  cord(): in CORD: tmpa: {:04x} tmpc: {:04x}", tmpa, tmpc);
                                continue; // JMP to 3
                            }
                            else {
                                // Continue to 197:
                                cpu.cycles_i(2, &[0x197, MC_JUMP]);
                            }
                        }
                        else {
                            cpu.cycle_i(0x191);
                            // 191:           | NCZ 3
                            internal_counter -= 1;
                            if internal_counter > 0 {
                                cpu.cycle_i(MC_JUMP);
                                //println!("  cord(): in CORD: tmpa: {:04x} tmpc: {:04x}", tmpa, tmpc);
                                continue; // JMP to 3
                            }
                            else {
                                // Continue to 192:
                            }
                        }
                    }
                    //println!("  cord(): in CORD: tmpa: {:04x} tmpc: {:04x}", tmpa, tmpc);
                }

                // 192
                (sigma_s, carry, _) = (tmpc as Self).alu_rcl(1, carry);

                // 193: SIGMA->tmpc
                tmpc = sigma_s as u16;

                // 194: SIGMA->no dest | RTN
                (_, carry, _) = (tmpc as Self).alu_rcl(1, carry);

                cpu.cycles_i(4, &[0x192, 0x193, 0x194, MC_RTN]);
                //println!("cord_finish(): tmpc: {} tmpa: {}", tmpc, tmpa);

                Ok((tmpc, tmpa, carry))
            }
        }
    };
}

impl_cord!(u8);
impl_cord!(u16);

pub trait Corx<B = Self>: Sized {
    fn corx(self, cpu: &mut NecVx0, b: u16, c: u16, carry: bool) -> (u16, u16);
}

macro_rules! impl_corx {
    ($prim:ty) => {
        impl Corx for $prim {
            /// Implementation of the 8088 microcode CORX multiplication co-routine.
            /// Implemented for either 8 bit or 16 bit operands.
            /// tmpa is used to select the size of the operation, but the value is not used.
            fn corx(self, cpu: &mut NecVx0, b: u16, c: u16, mut carry: bool) -> (u16, u16) {
                let mut internal_counter;
                let mut tmpa: u16;
                let tmpb: u16 = b;
                let mut tmpc: u16 = c;
                let mut sigma_s: Self;

                (sigma_s, carry, _) = (tmpc as Self).alu_rcr(1, carry); // 17f: ZERO->tmpa  | RRCY tmpc
                tmpa = 0;
                tmpc = sigma_s as u16; // 180: SIGMA->tmpc
                internal_counter = Self::BITS - 1; // 180: MAXC
                cpu.cycles_i(2, &[0x17f, 0x180]);

                // The main corx loop is between 181-186.
                loop {
                    //println!("  >> impl corx: tmpa: {} tmpc: {}", tmpa, tmpc);
                    cpu.cycle_i(0x181); // 181:    | NCY 8 (jump if no carry)

                    if carry {
                        (sigma_s, carry, _, _) = (tmpa as Self).alu_add(tmpb as Self); // 182:             | ADD tmpa
                        tmpa = sigma_s as u16; // 183: SIGMA->tmpa    | F
                        cpu.cycles_i(2, &[0x182, 0x183]);
                        // SET FLAGS HERE
                    }
                    else {
                        // Jump delay for skipping to line 8
                        cpu.cycle_i(MC_JUMP);
                    }

                    (sigma_s, carry, _) = (tmpa as Self).alu_rcr(1, carry); // 184:             | RRCY tmpa
                    tmpa = sigma_s as u16; // 185
                    (sigma_s, carry, _) = (tmpc as Self).alu_rcr(1, carry); // 185: SIGMA->tmpa | RRCY tmpc
                    tmpc = sigma_s as u16; // 186: SIGMA->tmpc | NCZ 5

                    cpu.cycles_i(3, &[0x184, 0x185, 0x186]);

                    if internal_counter == 0 {
                        break; // 186: no-jump
                    }

                    // It's not explicitly explained where the internal counter is updated.
                    // I am just assuming it is decremented once per loop here.
                    internal_counter -= 1;

                    cpu.cycle_i(MC_JUMP); // 186: (jump) 1 cycle delay to return to top of loop.
                }

                // Fall through line 186
                cpu.cycles_i(2, &[0x187, MC_RTN]); // 187 'RTN', return delay cycle

                (tmpa, tmpc)
            }
        }
    };
}

impl_corx!(u8);
impl_corx!(u16);

pub trait CorNegate: Sized {
    fn cor_negate(self, cpu: &mut NecVx0, b: u16, c: u16, neg_flag: bool, skip: bool) -> (u16, u16, u16, bool, bool);
}

macro_rules! impl_cor_negate {
    ($prim:ty) => {
        impl CorNegate for $prim {
            /// Implementation of the Microcode NEGATE co-routine used by signed multiplication and division.
            /// Accepts tmpa (self), tmpb, tmpc, neg_flag (passed in first time by f1 flag), and skip which
            /// effectively enters the NEGATE routine at line 7 (for division)
            ///
            /// Returns tmpa, tmpb, tmpc, carry and negate flag.
            fn cor_negate(
                self,
                cpu: &mut NecVx0,
                mut tmpb: u16,
                mut tmpc: u16,
                mut neg_flag: bool,
                skip: bool,
            ) -> (u16, u16, u16, bool, bool) {
                let mut tmpa = self as u16; // Sign-extend
                let mut sigma: u16;
                let mut carry: bool;
                let next_carry: bool;

                // Skip flag will skip to 1bb (line 7), such as when entering NEGATE from PREIDIV
                if !skip {
                    (sigma, carry, _, _) = tmpc.alu_neg();
                    //(sigma_s, carry, _, _) = (tmpc as Self).alu_neg(); // 1b6
                    //sigma = sigma_s as u16;
                    tmpc = sigma;

                    if carry {
                        sigma = !tmpa; // 1b8, jump, 1ba: SIGMA->tmpa | CF1
                        cpu.cycles_i(5, &[0x1b6, 0x1b7, 0x1b8, MC_JUMP, 0x1ba]);
                    }
                    else {
                        (sigma, _, _, _) = tmpa.alu_neg();
                        //(sigma_s, _, _, _) = (tmpa as Self).alu_neg(); // 1b8, 1b9, 1ba: SIGMA->tmpa | CF1
                        //sigma = sigma_s as u16;
                        cpu.cycles_i(5, &[0x1b6, 0x1b7, 0x1b8, 0x1b9, 0x1ba]);
                    }

                    tmpa = sigma; // 1ba
                    neg_flag = !neg_flag; // 1ba
                }

                // 1bb:     | LRCY tmpb
                // 1bc: SIGMA->tmpb  | NEG tmpb
                //(_, carry) = rcl_u8_with_carry(tmpb as u8, 1, carry); // Set carry flag if tmpb is negative
                carry = tmpb & (1 << (<$prim>::BITS - 1)) != 0; // LRCY is just checking msb of tmpb

                //println!("  NEGATE: tmpb: {:04X} carry: {}", tmpb, carry);

                (sigma, next_carry, _, _) = tmpb.alu_neg();
                //(sigma, next_carry, _, _) = (tmpb as Self).alu_neg();
                //sigma = sigma_s as u16;

                cpu.cycles_i(3, &[0x1bb, 0x1bc, 0x1bd]);
                //println!("  NEGATE: a: {:04x} b: {:04x} c:{:04x} tmpb Carry flag is : {}", tmpa, tmpb, tmpc, carry);
                // 1bd:             | NCY 11
                if !carry {
                    // tmpb was positive

                    // Jump to 11
                    // 1bf:         | RTN
                    cpu.cycles_i(3, &[MC_JUMP, 0x1bf, MC_RTN]);
                }
                else {
                    // tmpb was negative
                    //println!("  NEGATE: tmpb was negative");
                    // 1be: SIGMA->tmpb  | CF1 RTN
                    _ = next_carry;
                    tmpb = sigma; // tmpb = NEG tmpb
                    neg_flag = !neg_flag; // 1be
                    cpu.cycles_i(2, &[0x1be, MC_RTN]);
                }

                (tmpa, tmpb, tmpc, carry, neg_flag)
            }
        }
    };
}

impl_cor_negate!(u8);
impl_cor_negate!(u16);

impl NecVx0 {
    #[allow(dead_code)]
    #[allow(unused_assignments)] // This isn't pretty but we are trying to mirror the microcode
    /// Microcode routine for multiplication, 8 bit
    /// Accepts al and 8-bit operand, returns 16 bit product (for AX)
    pub fn mul8(&mut self, al: u8, operand: u8, signed: bool, mut negate: bool) -> u16 {
        let mut sigma: u16;
        let sigma8: u8;

        let mut tmpa: u16;
        let mut tmpc: u16 = al as u16; // 150 A->tmpc     | LRCY tmpc
        let mut carry;
        

        //(_, carry) = rcl_u8_with_carry(tmpc as u8, 1, carry);
        carry = tmpc & 0x80 != 0; // LRCY is just checking MSB of tmpc
        let mut tmpb: u16 = operand as u16; // 151: M->tmpb    | X0 PREIMUL
        self.cycles_i(2, &[0x150, 0x151]);

        // PREIMUL if signed == true
        // -------------------------------------------------------------------------
        if signed {
            // JMP PREIMUL
            (sigma, _, _, _) = tmpc.alu_neg(); // 1c0: SIGMA->.   | NEG tmpc
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

            // Call negate with skip flag to enter at line 7 (tmpa unused)
            (_, tmpb, tmpc, carry, negate) = (0u8).cor_negate(self, tmpb, tmpc, negate, true);
        }

        // 152:            | UNC CORX
        self.cycles_i(2, &[0x152, MC_JUMP]);

        (tmpa, tmpc) = (tmpb as u8).corx(self, tmpb, tmpc, carry);
        //let (accum, tmpc8) = self.corx8(tmpb as u8, tmpc as u8, carry);

        //println!("impl corx: {} {}, corx8: {} {}", tmpa2, tmpc2, accum, tmpc8);
        //println!("corx: {}, {}", accum, tmpc8);
        //tmpa = accum as u16;
        //tmpc = tmpc8 as u16;

        // 153:            | F1 NEGATE  (REP prefix negates product)
        self.cycle_i(0x153);

        // NEGATE if REP
        // -------------------------------------------------------------------------
        if negate {
            self.cycle_i(MC_JUMP); // Jump to NEGATE
                                   //println!("PRE-NEG: a: {:04x} b: {:04x} c:{:04x} tmpb Carry flag is : {}", tmpa, tmpb, tmpc, carry);
            (tmpa, tmpb, tmpc, carry, negate) = (tmpa as u8).cor_negate(self, tmpb, tmpc, negate, false);
            //println!("POST-NEG2: a: {:04x} b: {:04x} c:{:04x} tmpb Carry flag is : {}", tmpa2, tmpb2, tmpc2, carry2);
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
            (sigma8, _, _, _) = (tmpa as u8).alu_adc(tmpb as u8, carry);
            self.cycles_i(3, &[0x1cd, 0x1ce, 0x1cf]);
            // SET FLAGS HERE

            // 1d0:             | Z 8
            if sigma8 == 0 {
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.cycles_i(4, &[0x1d0, MC_JUMP, 0x1cc, MC_JUMP]);
            }
            else {
                // 1d1:              | SCOF RTN
                self.set_flag(Flag::Carry);
                self.set_flag(Flag::Overflow);
                self.cycles_i(3, &[0x1d0, 0x1d1, MC_JUMP]);
            }

            // 155: tmpc -> A      | X0 7
            // JUMP
            // 157: tmpa -> X      | RNI

            //self.cycles_i(3, &[0x155, MC_JUMP, 0x157]);
            self.cycles_i(2, &[0x155, MC_JUMP]);

            let product = tmpa << 8 | (tmpc & 0xFF);
            return product;
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
        let zf = sigma == 0;

        // 1d0:                | Z 8  (jump if zero)
        if zf {
            // JMP
            // 1cc:             | CCOF RTN
            self.clear_flag(Flag::Carry);
            self.clear_flag(Flag::Overflow);
            self.cycles_i(4, &[0x1d0, MC_JUMP, 0x1cc, MC_JUMP]);
        }
        else {
            // 1d1:             | SCOF RTN
            self.set_flag(Flag::Carry);
            self.set_flag(Flag::Overflow);
            self.cycles_i(3, &[0x1d0, 0x1d1, MC_JUMP]);
        }

        //self.cycle_i(0x157); // 157: tmpa-> X        | RNI

        
        tmpa << 8 | (tmpc & 0xFF)
    }

    #[allow(unused_assignments)] // This isn't pretty but we are trying to mirror the microcode
    /// Microcode routine for multiplication, 16 bit
    /// Accepts ax and 16-bit operand, returns 32 bit product in two parts (for DX:AX)
    pub fn mul16(&mut self, ax: u16, operand: u16, signed: bool, mut negate: bool) -> (u16, u16) {
        let mut sigma: u16;

        let mut tmpa: u16;
        let mut tmpc: u16 = ax; // 158 XA->tmpc     | LRCY tmpc
        let mut carry;
        

        //(_, carry) = rcl_u16_with_carry(tmpc, 1, carry); // SIGMA isn't used? Just setting carry flag(?)
        carry = tmpc & 0x8000 != 0; // LRCY is just checking msb
        let mut tmpb: u16 = operand; // 159: M->tmpb    | X0 PREIMUL
        self.cycles_i(2, &[0x158, 0x159]);

        // PREIMUL if signed == true
        // -------------------------------------------------------------------------
        if signed {
            // JMP PREIMUL
            (sigma, _, _, _) = tmpc.alu_neg(); // 1c0: SIGMA->.   | NEG tmpc
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

            // Call negate with skip flag to enter at line 7
            (_, tmpb, tmpc, carry, negate) = 0u16.cor_negate(self, tmpb, tmpc, negate, true);
        }

        // 15a:            | UNC CORX
        self.cycles_i(2, &[0x15a, MC_JUMP]);

        (tmpa, tmpc) = tmpb.corx(self, tmpb, tmpc, carry);
        //(tmpa, tmpc) = self.corx16(tmpb, tmpc, carry);
        //println!("a: {} c: {} , a: {} c: {}", tmpa2, tmpc2, tmpa, tmpc);

        // 15b:            | F1 NEGATE  (REP prefix negates product)
        self.cycle_i(0x15b);

        // NEGATE if REP
        // -------------------------------------------------------------------------
        if negate {
            self.cycle_i(MC_JUMP); // Jump to NEGATE
            (tmpa, tmpb, tmpc, carry, negate) = tmpa.cor_negate(self, tmpb, tmpc, negate, false);
        }

        // 15c:                | X0 IMULCOF
        // IMULFCOF if signed
        // -------------------------------------------------------------------------
        self.cycle_i(0x15c);

        if signed {
            self.cycle_i(MC_JUMP); // JMP
            tmpb = 0; // 1cd
                      //(_, carry) = rcl_u16_with_carry(tmpc, 1, carry);  // Test if tmpc is negative
            carry = tmpc & 0x8000 != 0; // 1cd: LRCY is just checking msb of tmpc
            (sigma, _, _, _) = tmpa.alu_adc(tmpb, carry);
            self.cycles_i(3, &[0x1cd, 0x1ce, 0x1cf]);
            // Set flags here

            // 1d0:             | Z 8
            if sigma == 0 {
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.cycles_i(4, &[0x1d0, MC_JUMP, 0x1cc, MC_JUMP]);
            }
            else {
                // 1d1:              | SCOF RTN
                self.set_flag(Flag::Carry);
                self.set_flag(Flag::Overflow);
                self.cycles_i(3, &[0x1d0, 0x1d1, MC_JUMP]);
            }

            // 15d: tmpc -> A      | X0 7
            // JUMP
            // 15f: tmpa -> X      | RNI
            //self.cycles_i(3, &[0x15d, MC_JUMP, 0x15f]);
            self.cycles_i(2, &[0x15d, MC_JUMP]);
            return (tmpa, tmpc);
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
        let zf = sigma == 0;

        // 1d0:                | Z 8  (jump if zero)
        if zf {
            // JMP
            // 1cc:             | CCOF RTN
            self.clear_flag(Flag::Carry);
            self.clear_flag(Flag::Overflow);
            self.cycles_i(4, &[0x1d0, MC_JUMP, 0x1cc, MC_JUMP]);
        }
        else {
            // 1d1:             | SCOF RTN
            self.set_flag(Flag::Carry);
            self.set_flag(Flag::Overflow);
            self.cycles_i(3, &[0x1d0, 0x1d1, MC_JUMP]);
        }

        // 157: tmpa-> X        | RNI
        //self.cycle_i(0x157);

        (tmpa, tmpc)
    }

    #[allow(dead_code)]
    #[allow(unused_assignments)] // This isn't pretty but we are trying to mirror the microcode
    /// Microcode routine for 8-bit division.
    /// Accepts 16-bit dividend, 8-bit divisor. Returns 8 bit quotient and remainder, or Err() on divide error
    /// so that an int0 can be triggered.
    pub fn div8(&mut self, dividend: u16, divisor: u8, signed: bool, mut negate: bool) -> Result<(u8, u8), bool> {
        let mut tmpa: u16 = dividend >> 8; // 160
        let mut tmpc: u16 = dividend & 0xFF; // 161
        let mut tmpb = divisor as u16; // 162

        let mut sigma16: u16;
        let sigma_next16: u16;

        let mut carry: bool;
        let carry_next: bool;

        self.cycles_i(3, &[0x160, 0x161, 0x162]);

        //log::debug!("  div8: a: {:04x}, b: {:04x}, c: {:04x}, n: {}", tmpa, tmpb, tmpc, negate);

        // Is dividend negative?
        (_, carry_next, _) = (tmpa as u8).alu_rcl(1, false);

        // Do PREIDIV if signed
        if signed {
            // 1b4: SIGMA->.    |
            //sigma16 = sigma8 as u16;
            carry = carry_next;

            self.cycles_i(3, &[MC_JUMP, 0x1b4, 0x1b5]);

            // 1b5:             | NCY 7
            if !carry {
                // Dividend is positive
                // Jump into NEGATE @ 7 (skip == true)
                self.cycle_i(MC_JUMP);
                (tmpa, tmpb, tmpc, _, negate) = (tmpa as u8).cor_negate(self, tmpb, tmpc, negate, true);
            }
            else {
                // Dividend is negative
                // Fall through to NEGATE
                (tmpa, tmpb, tmpc, _, negate) = (tmpa as u8).cor_negate(self, tmpb, tmpc, negate, false);
            }

            //log::debug!("  div8: post-negate: a: {:04x}, b: {:04x}, c: {:04x}, n: {}", tmpa, tmpb, tmpc, negate);
        }

        // 163
        self.cycles_i(2, &[0x163, MC_JUMP]);
        (tmpc, tmpa, carry) = match (tmpa as u8).cord(self, tmpa, tmpb, tmpc) {
            Ok((tmpc, tmpa, carry)) => (tmpc, tmpa, carry),
            Err(_) => {
                return Err(false);
            }
        };

        // 164         | COM1 tmpc
        sigma16 = !tmpc;

        // 165 X->tmpb | X0 POSTDIV
        tmpb = dividend >> 8;

        self.cycles_i(2, &[0x164, 0x165]);

        // Call POSTIDIV if signed
        if signed {
            self.cycles_i(2, &[MC_JUMP, 0x1c4]);
            //log::debug!("  div8: POSTIDIV");

            // 1c4:         | NCY INT0
            if !carry {
                self.cycle_i(MC_JUMP);
                return Err(false);
            }

            // 1c5:
            (_, carry, _) = (tmpb as u8).alu_rcl(1, false);
            // 1c6:

            //(sigma_next8, _, _, _) = (tmpa as u8).alu_neg();
            //sigma_next16 = sigma_next8 as u16;
            (sigma_next16, _, _, _) = tmpa.alu_neg();

            // 1c7:

            self.cycles_i(3, &[0x1c5, 0x1c6, 0x1c7]);

            if !carry {
                // divisor is positive
                self.cycle_i(MC_JUMP); // jump delay to 5
            }
            else {
                // divisor is negative

                //log::debug!("  div8: tmpb was negative in POSTIDIV");
                sigma16 = sigma_next16; // 1c8 SIGMA->tmpa
                tmpa = sigma16; // if tmpb was negative (msb was set), set tmpa to NEG tempa (flip sign)
                self.cycle_i(0x1c8);
            }

            // 1c9              | INC tmpc
            sigma16 = tmpc.wrapping_add(1);

            self.cycles_i(2, &[0x1c9, 0x1ca]);
            // 1ca              | F1 8
            if !negate {
                //log::debug!("  div8: negate flag not set: tmpc = !tmpc");
                sigma16 = !tmpc; // 1cb:        | COM tmpc
                self.cycle_i(0x1cb);
            }
            else {
                //log::debug!("  div8: negate flag was set: tmpc = NEG tmpa + 1");
                self.cycle_i(MC_JUMP);
            }

            // clear carry, overflow flag here
            self.cycles_i(2, &[0x1cc, MC_RTN]);
        }

        tmpc = sigma16; // 166: SIGMA -> AL  (Quotient)

        //log::debug!("  div8: done: a: {} b: {} c:{} ", tmpa, tmpb, tmpc);

        Ok((tmpc as u8, tmpa as u8))
    }

    /// Microcode routine for 16-bit division.
    /// Accepts 32-bit dividend, 16-bit divisor. Returns 16 bit quotient and remainder, or Err() on divide error
    /// so that an int0 can be triggered.
    pub fn div16(&mut self, dividend: u32, divisor: u16, signed: bool, mut negate: bool) -> Result<(u16, u16), bool> {
        let mut tmpa: u16 = (dividend >> 16) as u16; // 160
        let mut tmpc: u16 = (dividend & 0xFFFF) as u16; // 161
        let mut tmpb = divisor; // 162

        let mut sigma16: u16;
        let sigma_next: u16;

        let mut carry: bool;
        let carry_next: bool;

        self.cycles_i(3, &[0x168, 0x169, 0x16a]);

        //log::debug!("  div16: a: {:04x}, b: {:04x}, c: {:04x}, n: {}", tmpa, tmpb, tmpc, negate);

        (_, carry_next, _) = tmpa.alu_rcl(1, false);

        // Do PREIDIV if signed
        if signed {
            // 1b4: SIGMA->.    |
            //sigma16 = sigma8 as u16;
            carry = carry_next;

            self.cycles_i(3, &[MC_JUMP, 0x1b4, 0x1b5]);

            // 1b5:             | NCY 7
            if !carry {
                // Jump into NEGATE @ 7 (skip == true)
                self.cycle_i(MC_JUMP);
                (tmpa, tmpb, tmpc, _, negate) = tmpa.cor_negate(self, tmpb, tmpc, negate, true);
            }
            else {
                // Fall through to NEGATE
                (tmpa, tmpb, tmpc, _, negate) = tmpa.cor_negate(self, tmpb, tmpc, negate, false);
            }

            //log::debug!("  div16: post-negate: a: {:04x}, b: {:04x}, c: {:04x}, n: {}", tmpa, tmpb, tmpc, negate);
        }

        // 16b:
        self.cycles_i(2, &[0x163, MC_JUMP]);
        (tmpc, tmpa, carry) = match tmpa.cord(self, tmpa, tmpb, tmpc) {
            Ok((tmpc, tmpa, carry)) => (tmpc, tmpa, carry),
            Err(_) => return Err(false),
        };

        // 16c        | COM1 tmpc
        sigma16 = !tmpc;

        // 16d DE->tmpb | X0 POSTDIV
        tmpb = (dividend >> 16) as u16;

        self.cycles_i(2, &[0x16c, 0x16d]);

        // Call POSTIDIV if signed
        if signed {
            self.cycles_i(2, &[MC_JUMP, 0x1c4]);
            //log::debug!("  div16: POSTIDIV");

            // 1c4:         | NCY INT0
            if !carry {
                self.cycle_i(MC_JUMP);
                return Err(false);
            }

            // 1c5:
            (_, carry, _) = tmpb.alu_rcl(1, false);
            // 1c6:

            (sigma_next, _, _, _) = tmpa.alu_neg();
            // 1c7:

            self.cycles_i(3, &[0x1c5, 0x1c6, 0x1c7]);

            if !carry {
                // divisor is positive
                // jump delay to 5
                self.cycle_i(MC_JUMP);
            }
            else {
                // divisor is negative
                //log::debug!("  div16: tmpb was negative in POSTIDIV");
                sigma16 = sigma_next; // 1c8 SIGMA->tmpa
                tmpa = sigma16; // if tmpb was negative (msb was set), set tmpa to NEG tempa (flip sign)
                self.cycle_i(0x1c8);
            }

            // 1c9              | INC tmpc
            sigma16 = tmpc.wrapping_add(1);

            self.cycles_i(2, &[0x1c9, 0x1ca]);
            // 1ca              | F1 8
            if !negate {
                //log::debug!("  div16 negate flag not set: COM tmpc");
                sigma16 = !tmpc; // 1cb:        | COM1 tmpc
                self.cycle_i(0x1cb);
            }
            else {
                //log::debug!("  div16: negate flag was set");
                self.cycle_i(MC_JUMP);
            }

            // clear carry, overflow flag here
            self.cycles_i(2, &[0x1cc, MC_RTN]);
        }

        tmpc = sigma16; // 16e: SIGMA -> AX (Quotient)

        //log::debug!("  div16: done: a: {} b: {} c:{} ", tmpa, tmpb, tmpc);

        // 16f: tmpa -> DX (Remainder)
        Ok((tmpc, tmpa))
    }
}

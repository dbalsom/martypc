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
*/

//! This module provides direct translations of the microcode routines responsible for
//! multiplication and division operations on the 8088.
//!
//! The 8088 microcode is complex and involves a number of co-routines to handle signed/
//! unsigned variations. These co-routines are implemented as traits for the 8 and 16-bit unsigned
//! integer types.
//!
//! The main routine for multiplication is `CORX`, and for division, `CORD`.
//! Multiplication has signed co-routines `PREIMUL` and `IMULCOF`.
//! Division has `PREIDIV` and `POSTIDIV`.
//!
//! The methods shown here could potentially be optimized further. It may not be necessary to
//! exactly execute the microcode step by step, as long as the results, flag state and number of
//! cycles spent are correct.
//!
//! The routines here will produce the correct flag state for both defined and undefined flags
//! for both signed and unsigned operations. Emulating the unsigned flag state for division is
//! tricky - the carry flag especially is updated in-situ during the LRCY operations used to
//! check the MSB. Making a more efficient DIV routine would need to account for this somehow.

use crate::{cpu_808x::*, cpu_common::alu::*, cycles_mc};

/// Implement the PREIDIV routine.
/// This is a simple routine run before signed division to check if the dividend is negative.
/// It simply controls the entry point to the NEGATE routine.
pub trait PreIdiv<B = Self>: Sized {
    //
    fn pre_idiv(self, cpu: &mut Intel808x, tmpb: u16, tmpc: u16, negate: bool) -> (u16, u16, u16, bool, bool);
}

macro_rules! impl_preidiv {
    ($prim:ty) => {
        impl PreIdiv for $prim {
            #[inline]
            fn pre_idiv(self, cpu: &mut Intel808x, tmpb: u16, tmpc: u16, negate: bool) -> (u16, u16, u16, bool, bool) {
                // 1b4: SIGMA->.    |
                // (Is dividend negative?)
                let (_, carry, _) = self.alu_rcl(1, false);
                cycles_mc!(cpu, 0x1b4, 0x1b5);
                // 1b5:             | NCY 7
                if !carry {
                    // Dividend is positive
                    // Jump into NEGATE @ 7 (skip == true)
                    cpu.cycle_i(MC_JUMP);
                    self.cor_negate(cpu, tmpb, tmpc, negate, true)
                }
                else {
                    // Dividend is negative
                    // Fall through to NEGATE
                    self.cor_negate(cpu, tmpb, tmpc, negate, false)
                }
            }
        }
    };
}

impl_preidiv!(u8);
impl_preidiv!(u16);

/// Implement the POSTIDIV routine. Called after completion of signed division.
pub trait PostIdiv<B = Self>: Sized {
    fn post_idiv(
        self,
        cpu: &mut Intel808x,
        tmpb: u16,
        tmpc: u16,
        carry: bool,
        negate: bool,
    ) -> Result<(u16, u16), bool>;
}

macro_rules! impl_postidiv {
    ($prim:ty) => {
        impl PostIdiv for $prim {
            #[inline]
            fn post_idiv(
                self,
                cpu: &mut Intel808x,
                tmpb: u16,
                tmpc: u16,
                mut carry: bool,
                negate: bool,
            ) -> Result<(u16, u16), bool> {
                let mut tmpa = self as u16;
                let mut sigma: u16;

                cpu.cycle_i(0x1c4);
                // 1c4:         | NCY INT0
                if !carry {
                    // Division exception (divide by zero or underflow)
                    cpu.cycle_i(MC_JUMP);
                    return Err(false);
                }

                // 1c5:                | LRCY tmpb
                // 1c6: SIGMA->.       | NEG tmpa
                (_, carry, _) = (tmpb as Self).alu_rcl(1, false);
                (sigma, _, _, _) = tmpa.alu_neg();

                cycles_mc!(cpu, 0x1c5, 0x1c6, 0x1c7);
                // 1c7:                | NCY 5
                if !carry {
                    // Divisor is positive
                    cpu.cycle_i(MC_JUMP); // jump to 5
                }
                else {
                    // Divisor is negative
                    // 1c8: SIGMA->tmpa
                    tmpa = sigma; // if tmpb was negative (msb was set), set tmpa to NEG tempa (flip sign)
                    cpu.cycle_i(0x1c8);
                }

                // 1c9              | INC tmpc
                sigma = tmpc.wrapping_add(1) as u16;

                cycles_mc!(cpu, 0x1c9, 0x1ca);
                // 1ca              | F1 8
                if !negate {
                    //log::debug!("  div8: negate flag not set: tmpc = !tmpc");
                    sigma = !tmpc; // 1cb:        | COM tmpc
                    cpu.cycle_i(0x1cb);
                }
                else {
                    //log::debug!("  div8: negate flag was set: tmpc = NEG tmpa + 1");
                    cpu.cycle_i(MC_JUMP);
                }

                // 1cc:             | CCOF RTN
                cpu.clear_flag(Flag::Carry);
                cpu.clear_flag(Flag::Overflow);
                cycles_mc!(cpu, 0x1cc, MC_RTN);

                Ok((tmpa, sigma))
            }
        }
    };
}

impl_postidiv!(u8);
impl_postidiv!(u16);

pub trait Cord<B = Self>: Sized {
    fn cord(self, cpu: &mut Intel808x, a: u16, b: u16, c: u16) -> Result<(u16, u16, bool), bool>;
}

macro_rules! impl_cord {
    ($prim:ty) => {
        impl Cord for $prim {
            /// Implementation of the 8088 microcode CORD division co-routine.
            /// Implemented for either 8 bit or 16 bit operand.
            fn cord(self, cpu: &mut Intel808x, a: u16, b: u16, c: u16) -> Result<(u16, u16, bool), bool> {
                let mut internal_counter;

                let mut tmpa: u16 = a;
                let tmpb: u16 = b;
                let mut tmpc: u16 = c;

                let mut sigma_s: Self;

                let mut carry;
                let mut overflow;
                let mut aux_carry;

                // 188:           | SUBT tmpa
                (sigma_s, carry, overflow, aux_carry) = (tmpa as Self).alu_sub(tmpb as Self);

                // 189: SIGMA->.  | MAXC
                internal_counter = Self::BITS;
                // SET FLAGS HERE
                // WIP these aren't correct yet.
                cpu.set_flag_state(Flag::AuxCarry, aux_carry);
                cpu.set_flag_state(Flag::Overflow, overflow);
                cpu.set_flag_state(Flag::Carry, carry);
                cpu.set_szp_flags_from_result(sigma_s as u16);

                cycles_mc!(cpu, 0x188, 0x189, 0x18a);

                // 18a:           | NCY INT0
                if !carry {
                    // Jump delay to INT0 procedure
                    cpu.cycle_i(MC_JUMP);
                    //log::debug!("cord: div overflow");
                    return Err(false);
                }

                // The main CORD loop is between 18b and 196.
                while internal_counter > 0 {
                    // 18c: SIGMA->tmpc | RCLY tmpa
                    (sigma_s, carry, _) = (tmpc as Self).alu_rcl(1, carry);
                    tmpc = sigma_s as u16;

                    // 18d: SIGMA->tmpa | SUBT tmpa
                    (sigma_s, carry, _) = (tmpa as Self).alu_rcl(1, carry);
                    tmpa = sigma_s as u16;

                    cycles_mc!(cpu, 0x18b, 0x18c, 0x18d, 0x18e);

                    // 18e:
                    if carry {
                        // Jump delay
                        cycles_mc!(cpu, MC_JUMP, 0x195, 0x196);
                        // 195:              | RCY
                        carry = false;
                        // 196: SIGMA->tmpa  | NCZ 3
                        (sigma_s, _, _, _) = (tmpa as Self).alu_sub(tmpb as Self);
                        tmpa = sigma_s as u16;

                        internal_counter -= 1;
                        if internal_counter > 0 {
                            // 196: SIGMA->tmpa  | NCZ 3
                            cpu.cycle_i(MC_JUMP);
                            continue;
                        }
                        else {
                            // Continue to 197:
                            cycles_mc!(cpu, 0x197, MC_JUMP);
                        }
                    }
                    else {
                        // 18f: SIGMA->no dest     | F
                        (sigma_s, carry, overflow, aux_carry) = (tmpa as Self).alu_sub(tmpb as Self);

                        // SET FLAGS HERE
                        // WIP these aren't correct yet.
                        cpu.set_flag_state(Flag::AuxCarry, aux_carry);
                        cpu.set_flag_state(Flag::Overflow, overflow);
                        cpu.set_flag_state(Flag::Carry, carry);
                        cpu.set_szp_flags_from_result(sigma_s as u16);

                        cycles_mc!(cpu, 0x18f, 0x190);

                        // 190:    NCY 14
                        if !carry {
                            cycles_mc!(cpu, MC_JUMP, 0x196);
                            // 196: SIGMA->tmpa    | NCZ 3
                            (sigma_s, _, _, _) = (tmpa as Self).alu_sub(tmpb as Self);
                            tmpa = sigma_s as u16;
                            internal_counter -= 1;
                            if internal_counter > 0 {
                                cpu.cycle_i(MC_JUMP);
                                continue; // JMP to 3
                            }
                            else {
                                // Continue to 197:
                                cycles_mc!(cpu, 0x197, MC_JUMP);
                            }
                        }
                        else {
                            cpu.cycle_i(0x191);
                            // 191:           | NCZ 3
                            internal_counter -= 1;
                            if internal_counter > 0 {
                                cpu.cycle_i(MC_JUMP);
                                continue; // JMP to 3
                            }
                            else {
                                // Continue to 192:
                            }
                        }
                    }
                }

                // 192
                (sigma_s, carry, _) = (tmpc as Self).alu_rcl(1, carry);

                // 193: SIGMA->tmpc
                tmpc = sigma_s as u16;

                // 194: SIGMA->no dest | RTN
                (_, carry, _) = (tmpc as Self).alu_rcl(1, carry);
                cpu.set_flag_state(Flag::Carry, carry);
                cycles_mc!(cpu, 0x192, 0x193, 0x194, MC_RTN);
                Ok((tmpc, tmpa, carry))
            }
        }
    };
}

impl_cord!(u8);
impl_cord!(u16);

pub trait Corx<B = Self>: Sized {
    fn corx(self, cpu: &mut Intel808x, b: u16, c: u16, carry: bool) -> (u16, u16);
}

macro_rules! impl_corx {
    ($prim:ty) => {
        impl Corx for $prim {
            /// Implementation of the 8088 microcode CORX multiplication co-routine.
            /// Implemented for either 8 bit or 16 bit operands.
            /// tmpa is used to select the size of the operation, but the value is not used.
            fn corx(self, cpu: &mut Intel808x, b: u16, c: u16, mut carry: bool) -> (u16, u16) {
                let mut internal_counter;
                let mut tmpa: u16;
                let tmpb: u16 = b;
                let mut tmpc: u16 = c;
                let mut sigma_s: Self;

                (sigma_s, carry, _) = (tmpc as Self).alu_rcr(1, carry); // 17f: ZERO->tmpa  | RRCY tmpc
                tmpa = 0;
                tmpc = sigma_s as u16; // 180: SIGMA->tmpc
                internal_counter = Self::BITS - 1; // 180: MAXC
                cycles_mc!(cpu, 0x17f, 0x180);

                // The main corx loop is between 181-186.
                loop {
                    cpu.cycle_i(0x181); // 181:    | NCY 8 (jump if no carry)

                    if carry {
                        (sigma_s, carry, _, _) = (tmpa as Self).alu_add(tmpb as Self); // 182:             | ADD tmpa
                        tmpa = sigma_s as u16; // 183: SIGMA->tmpa    | F
                        cycles_mc!(cpu, 0x182, 0x183);
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

                    cycles_mc!(cpu, 0x184, 0x185, 0x186);

                    if internal_counter == 0 {
                        break; // 186: no-jump
                    }

                    // It's not explicitly explained where the internal counter is updated.
                    // I am just assuming it is decremented once per loop here.
                    internal_counter -= 1;
                    cpu.cycle_i(MC_JUMP); // 186: (jump) 1 cycle delay to return to top of loop.
                }

                // Fall through line 186
                cycles_mc!(cpu, 0x187, MC_RTN); // 187 'RTN', return delay cycle
                (tmpa, tmpc)
            }
        }
    };
}

impl_corx!(u8);
impl_corx!(u16);

pub trait CorNegate: Sized {
    fn cor_negate(self, cpu: &mut Intel808x, b: u16, c: u16, neg_flag: bool, skip: bool)
        -> (u16, u16, u16, bool, bool);
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
                cpu: &mut Intel808x,
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
                    (sigma, carry, _, _) = tmpc.alu_neg(); // 1b6
                    tmpc = sigma;

                    if carry {
                        sigma = !tmpa; // 1b8, jump, 1ba: SIGMA->tmpa | CF1
                        cycles_mc!(cpu, 0x1b6, 0x1b7, 0x1b8, MC_JUMP, 0x1ba);
                    }
                    else {
                        (sigma, _, _, _) = tmpa.alu_neg(); // 1b8, 1b9, 1ba: SIGMA->tmpa | CF1
                        cycles_mc!(cpu, 0x1b6, 0x1b7, 0x1b8, 0x1b9, 0x1ba);
                    }

                    tmpa = sigma; // 1ba
                    neg_flag = !neg_flag; // 1ba
                }

                // 1bb:     | LRCY tmpb
                // 1bc: SIGMA->tmpb  | NEG tmpb
                carry = tmpb & (1 << (<$prim>::BITS - 1)) != 0; // LRCY is just checking msb of tmpb
                cpu.set_flag_state(Flag::Carry, carry);
                (sigma, next_carry, _, _) = tmpb.alu_neg();

                cycles_mc!(cpu, 0x1bb, 0x1bc, 0x1bd);
                // 1bd:             | NCY 11
                if !carry {
                    // tmpb was positive

                    // Jump to 11
                    // 1bf:         | RTN
                    cycles_mc!(cpu, MC_JUMP, 0x1bf, MC_RTN);
                }
                else {
                    // tmpb was negative

                    // 1be: SIGMA->tmpb  | CF1 RTN
                    _ = next_carry;
                    tmpb = sigma; // tmpb = NEG tmpb
                    neg_flag = !neg_flag; // 1be
                    cycles_mc!(cpu, 0x1be, MC_RTN);
                }

                (tmpa, tmpb, tmpc, carry, neg_flag)
            }
        }
    };
}

impl_cor_negate!(u8);
impl_cor_negate!(u16);

impl Intel808x {
    /// Microcode routine for multiplication, 8 bit
    /// Accepts al and 8-bit operand, returns 16 bit product (for AX)
    pub fn mul8(&mut self, al: u8, operand: u8, signed: bool, mut negate: bool) -> u16 {
        let mut sigma: u16;
        let _sigma8: u8;

        let mut tmpa: u16;
        let mut tmpc: u16 = al as u16; // 150 A->tmpc     | LRCY tmpc
        let mut carry;
        let aux_carry: bool;
        

        carry = tmpc & 0x80 != 0; // LRCY is just checking MSB of tmpc
        let mut tmpb: u16 = operand as u16; // 151: M->tmpb    | X0 PREIMUL
        cycles_mc!(self, 0x150, 0x151);

        // PREIMUL if signed == true
        // -------------------------------------------------------------------------
        if signed {
            // JMP PREIMUL
            (sigma, _, _, _) = tmpc.alu_neg(); // 1c0: SIGMA->.   | NEG tmpc
                                               // 1c1             | NCY 7
            cycles_mc!(self, MC_JUMP, 0x1c0, 0x1c1);

            if carry {
                tmpc = sigma;
                negate = !negate; // 1c2: SIGMA->tmpc | CF1   (flip F1 flag)
                cycles_mc!(self, 0x1c2, 0x1c3, MC_JUMP);
            }
            else {
                self.cycle_i(MC_JUMP);
            }

            // Call negate with skip flag to enter at line 7 (tmpa unused)
            (_, tmpb, tmpc, carry, negate) = (0u8).cor_negate(self, tmpb, tmpc, negate, true);
        }

        // 152:            | UNC CORX
        cycles_mc!(self, 0x152, MC_JUMP);
        (tmpa, tmpc) = (tmpb as u8).corx(self, tmpb, tmpc, carry);

        // 153:            | F1 NEGATE  (REP prefix negates product)
        self.cycle_i(0x153);

        // NEGATE if REP
        // -------------------------------------------------------------------------
        if negate {
            self.cycle_i(MC_JUMP); // Jump to NEGATE
            (tmpa, _, tmpc, _, _) = (tmpa as u8).cor_negate(self, tmpb, tmpc, negate, false);
        }

        // 154:                | X0 IMULCOF
        // IMULFCOF if signed
        // -------------------------------------------------------------------------
        self.cycle_i(0x154);

        if signed {
            self.cycle_i(MC_JUMP); // JMP
            tmpb = 0;
            carry = tmpc & 0x80 != 0; // LRCY is just checking msb of tmpc

            (sigma, _, _, aux_carry) = tmpa.alu_adc(tmpb, carry);
            cycles_mc!(self, 0x1cd, 0x1ce, 0x1cf);
            // SET FLAGS HERE
            self.set_flag_state(Flag::AuxCarry, aux_carry);
            self.set_szp_flags_from_result_u16(sigma);

            // 1d0:             | Z 8
            //if sigma8 == 0 {
            if sigma == 0 {
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                cycles_mc!(self, 0x1d0, MC_JUMP, 0x1cc, MC_JUMP);
            }
            else {
                // 1d1:              | SCOF RTN
                self.set_flag(Flag::Carry);
                self.set_flag(Flag::Overflow);
                cycles_mc!(self, 0x1d0, 0x1d1, MC_JUMP);
            }

            // 155: tmpc -> A      | X0 7
            // JUMP
            // 157: tmpa -> X      | RNI
            cycles_mc!(self, 0x155, MC_JUMP);

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

        cycles_mc!(self, 0x155, 0x156, MC_JUMP, 0x1d2, 0x1d3, MC_JUMP);
        let zf = sigma == 0;

        // 1d0:                | Z 8  (jump if zero)
        if zf {
            // JMP
            // 1cc:             | CCOF RTN
            self.clear_flag(Flag::Carry);
            self.clear_flag(Flag::Overflow);
            cycles_mc!(self, 0x1d0, MC_JUMP, 0x1cc, MC_JUMP);
        }
        else {
            // 1d1:             | SCOF RTN
            self.set_flag(Flag::Carry);
            self.set_flag(Flag::Overflow);
            cycles_mc!(self, 0x1d0, 0x1d1, MC_JUMP);
        }

        // 157: tmpa-> X        | RNI
        
        tmpa << 8 | (tmpc & 0xFF)
    }

    /// Microcode routine for multiplication, 16 bit
    /// Accepts ax and 16-bit operand, returns 32 bit product in two parts (for DX:AX)
    pub fn mul16(&mut self, ax: u16, operand: u16, signed: bool, mut negate: bool) -> (u16, u16) {
        let mut sigma: u16;

        let mut tmpa: u16;
        let mut tmpc: u16 = ax; // 158 XA->tmpc     | LRCY tmpc
        let mut carry;
        

        carry = tmpc & 0x8000 != 0; // LRCY is just checking msb
        let mut tmpb: u16 = operand; // 159: M->tmpb    | X0 PREIMUL
        cycles_mc!(self, 0x158, 0x159);

        // PREIMUL if signed == true
        // -------------------------------------------------------------------------
        if signed {
            // JMP PREIMUL
            (sigma, _, _, _) = tmpc.alu_neg(); // 1c0: SIGMA->.   | NEG tmpc
                                               // 1c1             | NCY 7
            cycles_mc!(self, MC_JUMP, 0x1c0, 0x1c1);

            if carry {
                tmpc = sigma;
                negate = !negate; // 1c2: SIGMA->tmpc | CF1   (flip F1 flag)
                cycles_mc!(self, 0x1c2, 0x1c3, MC_JUMP);
            }
            else {
                self.cycle_i(MC_JUMP);
            }

            // Call negate with skip flag to enter at line 7
            (_, tmpb, tmpc, carry, negate) = 0u16.cor_negate(self, tmpb, tmpc, negate, true);
        }

        // 15a:            | UNC CORX
        cycles_mc!(self, 0x15a, MC_JUMP);
        (tmpa, tmpc) = tmpb.corx(self, tmpb, tmpc, carry);

        // 15b:            | F1 NEGATE  (REP prefix negates product)
        self.cycle_i(0x15b);

        // NEGATE if REP
        // -------------------------------------------------------------------------
        if negate {
            self.cycle_i(MC_JUMP); // Jump to NEGATE
            (tmpa, _, tmpc, _, _) = tmpa.cor_negate(self, tmpb, tmpc, negate, false);
        }

        // 15c:                | X0 IMULCOF
        // IMULFCOF if signed
        // -------------------------------------------------------------------------
        self.cycle_i(0x15c);

        if signed {
            self.cycle_i(MC_JUMP); // JMP
            tmpb = 0; // 1cd
            carry = tmpc & 0x8000 != 0; // 1cd: LRCY is just checking sign of tmpc
            let aux_carry: bool;
            (sigma, _, _, aux_carry) = tmpa.alu_adc(tmpb, carry);
            cycles_mc!(self, 0x1cd, 0x1ce, 0x1cf);
            // Set flags here
            self.set_flag_state(Flag::AuxCarry, aux_carry);
            self.set_szp_flags_from_result_u16(sigma);

            // 1d0:             | Z 8
            if sigma == 0 {
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                cycles_mc!(self, 0x1d0, MC_JUMP, 0x1cc, MC_JUMP);
            }
            else {
                // 1d1:              | SCOF RTN
                self.set_flag(Flag::Carry);
                self.set_flag(Flag::Overflow);
                cycles_mc!(self, 0x1d0, 0x1d1, MC_JUMP);
            }

            // 15d: tmpc -> A      | X0 7
            // JUMP
            // 15f: tmpa -> X      | RNI
            cycles_mc!(self, 0x15d, MC_JUMP);
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
        cycles_mc!(self, 0x15d, 0x15e, MC_JUMP, 0x1d2, 0x1d3, MC_JUMP);
        let zf = sigma == 0;

        // 1d0:                | Z 8  (jump if zero)
        if zf {
            // JMP
            // 1cc:             | CCOF RTN
            self.clear_flag(Flag::Carry);
            self.clear_flag(Flag::Overflow);
            cycles_mc!(self, 0x1d0, MC_JUMP, 0x1cc, MC_JUMP);
        }
        else {
            // 1d1:             | SCOF RTN
            self.set_flag(Flag::Carry);
            self.set_flag(Flag::Overflow);
            cycles_mc!(self, 0x1d0, 0x1d1, MC_JUMP);
        }

        // 157: tmpa-> X        | RNI

        (tmpa, tmpc)
    }

    #[allow(dead_code)]
    #[allow(unused_assignments)]
    /// 8-bit division operation
    /// A more-or-less direct translation of the microcode routine.
    /// Accepts 16-bit dividend, 8-bit divisor. Returns 8 bit quotient and remainder, or Err() on
    /// divide error so that an int0 can be triggered.
    pub fn div8(&mut self, dividend: u16, divisor: u8, signed: bool, mut negate: bool) -> Result<(u8, u8), bool> {
        let mut tmpa: u16 = dividend >> 8; // 160
        let mut tmpc: u16 = dividend & 0xFF; // 161
        let mut tmpb = divisor as u16; // 162
        let mut sigma16: u16;
        let carry: bool;

        cycles_mc!(self, 0x160, 0x161, 0x162);

        //log::debug!("  div8: a: {:04x}, b: {:04x}, c: {:04x}, n: {}", tmpa, tmpb, tmpc, negate);

        // Do PREIDIV if signed
        if signed {
            cycles_mc!(self, MC_JUMP);
            (tmpa, tmpb, tmpc, _, negate) = (tmpa as u8).pre_idiv(self, tmpb, tmpc, negate);
        }

        // 163:                | UNC CORD
        cycles_mc!(self, 0x163, MC_JUMP);
        (tmpc, tmpa, carry) = (tmpa as u8).cord(self, tmpa, tmpb, tmpc)?;

        // 164         | COM1 tmpc
        sigma16 = !tmpc;

        // 165 X->tmpb | X0 POSTDIV
        tmpb = dividend >> 8;

        cycles_mc!(self, 0x164, 0x165);

        // Call POSTIDIV if signed
        if signed {
            cycles_mc!(self, MC_JUMP);
            (tmpa, sigma16) = (tmpa as u8).post_idiv(self, tmpb, tmpc, carry, negate)?;
        }

        // 166: SIGMA -> AL  (Quotient)
        tmpc = sigma16;
        Ok((tmpc as u8, tmpa as u8))
    }

    /// Microcode routine for 16-bit division.
    /// Accepts 32-bit dividend, 16-bit divisor. Returns 16 bit quotient and remainder, or Err() on divide error
    /// so that an int0 can be triggered.
    pub fn div16(&mut self, dividend: u32, divisor: u16, signed: bool, mut negate: bool) -> Result<(u16, u16), bool> {
        let mut tmpa: u16 = (dividend >> 16) as u16; // 168
        let mut tmpc: u16 = (dividend & 0xFFFF) as u16; // 169
        let mut tmpb = divisor; // 16a
        let mut sigma16: u16;
        let carry: bool;

        cycles_mc!(self, 0x168, 0x169, 0x16a);

        //log::debug!("  div16: a: {:04x}, b: {:04x}, c: {:04x}, n: {}", tmpa, tmpb, tmpc, negate);

        // Do PREIDIV if signed
        if signed {
            // 1b4: SIGMA->.    |
            cycles_mc!(self, MC_JUMP);
            (tmpa, tmpb, tmpc, _, negate) = tmpa.pre_idiv(self, tmpb, tmpc, negate);
        }

        // 16b:                | UNC CORD
        cycles_mc!(self, 0x16b, MC_JUMP);
        (tmpc, tmpa, carry) = tmpa.cord(self, tmpa, tmpb, tmpc)?;

        // 16c        | COM1 tmpc
        sigma16 = !tmpc;

        // 16d DE->tmpb | X0 POSTDIV
        tmpb = (dividend >> 16) as u16;
        cycles_mc!(self, 0x16c, 0x16d);

        // Call POSTIDIV if signed
        if signed {
            cycles_mc!(self, MC_JUMP);
            (tmpa, sigma16) = tmpa.post_idiv(self, tmpb, tmpc, carry, negate)?;
        }

        // 16e: SIGMA -> AX (Quotient)
        tmpc = sigma16;
        // 16f: tmpa -> DX (Remainder)
        Ok((tmpc, tmpa))
    }
}

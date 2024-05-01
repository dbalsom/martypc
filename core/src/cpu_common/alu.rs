/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2024 Daniel Balsom

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

    cpu_common::alu.rs

    This module implements traits for alu operations of different widths
    common across CPU types.

*/

/* ----------------------- Addition & Subtraction ------------------------- */

pub trait AluNeg: Sized {
    fn alu_neg(self) -> (Self, bool, bool, bool);
}

pub trait AluSub<Rhs = Self>: Sized {
    fn alu_sub(self, rhs: Rhs) -> (Self, bool, bool, bool);
}

pub trait AluSbb<Rhs = Self>: Sized {
    fn alu_sbb(self, rhs: Rhs, carry: bool) -> (Self, bool, bool, bool);
}
pub trait AluAdd<Rhs = Self>: Sized {
    fn alu_add(self, rhs: Rhs) -> (Self, bool, bool, bool);
}

pub trait AluAdc<Rhs = Self>: Sized {
    fn alu_adc(self, rhs: Rhs, carry: bool) -> (Self, bool, bool, bool);
}

macro_rules! impl_neg {
    ($prim:ty) => {
        impl AluNeg for $prim {
            /// Negation
            ///
            /// Implemented as Sub(0 - Self)
            /// Flags are identical for Sub
            fn alu_neg(self) -> (Self, bool, bool, bool) {
                0.alu_sub(self)
            }
        }
    };
}

macro_rules! impl_sub {
    ($prim:ty) => {
        impl AluSub for $prim {
            /// Subtraction
            ///
            /// Carry flag is set if Unsigned overflow occurred
            /// Overflow flag is set if Signed overflow occurred
            /// AF flag is set if borrow from top nibble
            fn alu_sub(self, rhs: Self) -> (Self, bool, bool, bool) {
                let (result, carry) = self.overflowing_sub(rhs);
                let overflow = (self ^ rhs) & (self ^ result) & (1 << (<$prim>::BITS - 1)) != 0;
                let aux_carry = ((self ^ rhs ^ result) & 0x10) != 0;
                (result, carry, overflow, aux_carry)
            }
        }
    };
}

macro_rules! impl_sbb {
    ($prim:ty) => {
        impl AluSbb for $prim {
            /// Subtraction with borrow from carry flag
            ///
            /// Carry flag is set if Unsigned overflow occurred
            /// Overflow flag is set if Signed overflow occurred
            /// AF flag is set if borrow from top nibble
            fn alu_sbb(self, rhs: Self, carry_in: bool) -> (Self, bool, bool, bool) {
                let lhs_w: u32 = self as u32;
                let rhs_w: u32 = rhs as u32;
                let result: u32;
                let mut carry: bool;

                (result, carry) = lhs_w.overflowing_sub(rhs_w.wrapping_add(carry_in as u32)); // DEST := (DEST – (SRC + CF));
                carry = if <$prim>::BITS == 32 {
                    carry
                }
                else {
                    result & (0xFFFFFFFF << <$prim>::BITS) != 0
                };

                let overflow = (lhs_w ^ rhs_w) & (lhs_w ^ result) & (1 << (<$prim>::BITS - 1)) != 0; // Signed overflow
                let aux_carry = ((lhs_w ^ rhs_w ^ result) & 0x10) != 0; // Borrow from upper nibble

                (result as Self, carry, overflow, aux_carry)
            }
        }
    };
}

macro_rules! impl_add {
    ($prim:ty) => {
        impl AluAdd for $prim {
            /// Addition
            ///
            /// Carry flag is set if Unsigned overflow occurred
            /// Overflow flag is set if Signed overflow occurred
            /// AF flag is set if borrow from top nibble
            fn alu_add(self, rhs: Self) -> (Self, bool, bool, bool) {
                let (result, carry) = self.overflowing_add(rhs);
                let overflow = (self ^ result) & (rhs ^ result) & (1 << (<$prim>::BITS - 1)) != 0;
                let aux_carry = ((self ^ rhs ^ result) & 0x10) != 0;
                (result, carry, overflow, aux_carry)
            }
        }
    };
}

macro_rules! impl_adc {
    ($prim:ty) => {
        impl AluAdc for $prim {
            /// Addition with carry from carry flag
            ///
            /// Carry flag is set if Unsigned overflow occurred
            /// Overflow flag is set if Signed overflow occurred
            /// AF flag is set if borrow from top nibble
            fn alu_adc(self, rhs: Self, carry_in: bool) -> (Self, bool, bool, bool) {
                let lhs_w: u32 = self as u32;
                let rhs_w: u32 = rhs as u32;
                let result: u32;
                let mut carry: bool;

                (result, carry) = lhs_w.overflowing_add(rhs_w.wrapping_add(carry_in as u32)); // DEST := (DEST + (SRC + CF));
                carry = if <$prim>::BITS == 32 {
                    carry
                }
                else {
                    result & (0xFFFFFFFF << <$prim>::BITS) != 0
                };

                let overflow = (lhs_w ^ result) & (rhs_w ^ result) & (1 << (<$prim>::BITS - 1)) != 0; // Signed overflow
                let aux_carry = ((lhs_w ^ rhs_w ^ result) & 0x10) != 0; // Borrow from upper nibble

                (result as Self, carry, overflow, aux_carry)
            }
        }
    };
}

impl_neg!(u8);
impl_neg!(u16);
impl_sub!(u8);
impl_sub!(u16);
impl_sbb!(u8);
impl_sbb!(u16);
impl_add!(u8);
impl_add!(u16);
impl_adc!(u8);
impl_adc!(u16);

/* ------------------------- Bitwise operations ---------------------------- */

pub trait AluShiftLeft: Sized {
    fn alu_shl(self, count: u8) -> (Self, bool);
}

macro_rules! impl_shl {
    ($prim:ty) => {
        impl AluShiftLeft for $prim {
            fn alu_shl(mut self, mut count: u8) -> (Self, bool) {
                let mut carry = false;
                while count > 0 {
                    carry = self >> (<$prim>::BITS - 1) != 0;
                    self <<= 1;
                    count -= 1;
                }
                (self, carry)
            }
        }
    };
}

pub trait AluShiftRight: Sized {
    fn alu_shr(self, count: u8) -> (Self, bool);
}

macro_rules! impl_shr {
    ($prim:ty) => {
        impl AluShiftRight for $prim {
            fn alu_shr(mut self, mut count: u8) -> (Self, bool) {
                let mut carry = false;
                while count > 0 {
                    carry = self & 0x01 != 0;
                    self >>= 1;
                    count -= 1;
                }
                (self, carry)
            }
        }
    };
}

pub trait AluRotateLeft: Sized {
    fn alu_rol(self, count: u8) -> (Self, bool);
}

macro_rules! impl_rol {
    ($prim:ty) => {
        impl AluRotateLeft for $prim {
            fn alu_rol(mut self, count: u8) -> (Self, bool) {
                let mut carry = 0 as $prim;
                for _ in 0..count {
                    carry = self & (1 << (<$prim>::BITS - 1));
                    self <<= 1;
                    self |= carry >> (<$prim>::BITS - 1);
                }
                (self, carry != 0)
            }
        }
    };
}

pub trait AluRotateCarryLeft: Sized {
    fn alu_rcl(self, count: u8, carry: bool) -> (Self, bool);
}

macro_rules! impl_rcl {
    ($prim:ty) => {
        impl AluRotateCarryLeft for $prim {
            fn alu_rcl(mut self, count: u8, carry: bool) -> (Self, bool) {
                let mut carry = carry as $prim;
                for _ in 0..count {
                    let saved_carry = carry;
                    carry = self >> (<$prim>::BITS - 1);
                    self <<= 1;
                    self |= saved_carry;
                }
                (self, carry != 0)
            }
        }
    };
}

pub trait AluRotateRight: Sized {
    fn alu_ror(self, count: u8) -> (Self, bool);
}

macro_rules! impl_ror {
    ($prim:ty) => {
        impl AluRotateRight for $prim {
            fn alu_ror(mut self, count: u8) -> (Self, bool) {
                let mut carry = 0 as $prim;
                for _ in 0..count {
                    carry = self & 0x01;
                    self >>= 1;
                    self |= carry << (<$prim>::BITS - 1);
                }
                (self, carry != 0)
            }
        }
    };
}

pub trait AluRotateCarryRight: Sized {
    fn alu_rcr(self, count: u8, carry: bool) -> (Self, bool);
}

macro_rules! impl_rcr {
    ($prim:ty) => {
        impl AluRotateCarryRight for $prim {
            fn alu_rcr(mut self, count: u8, carry: bool) -> (Self, bool) {
                let mut carry = carry as $prim << (<$prim>::BITS - 1);
                for _ in 0..count {
                    let saved_carry = carry;
                    carry = (self & 1) << (<$prim>::BITS - 1);   // (<$prim>::BITS - 1);
                    self >>= 1;
                    self |= saved_carry;
                }
                (self, carry != 0)
            }
        }
    }
}

impl_shl!(u8);
impl_shl!(u16);
impl_shr!(u8);
impl_shr!(u16);
impl_rol!(u8);
impl_rol!(u16);
impl_rcl!(u8);
impl_rcl!(u16);
impl_ror!(u8);
impl_ror!(u16);
impl_rcr!(u8);
impl_rcr!(u16);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alu_shr() {
        let (result, carry) = 0x80u8.alu_shr(7);
        assert_eq!(result, 1);
        assert_eq!(carry, false);
        let (result, carry) = 0x04u8.alu_shr(3);
        assert_eq!(result, 0);
        assert_eq!(carry, true);
        let (result, carry) = 0x04u8.alu_shr(4);
        assert_eq!(result, 0);
        assert_eq!(carry, false);

        let (result16, carry) = 0x0101u16.alu_shr(1);
        assert_eq!(result16, 0x0080);
        assert_eq!(carry, true);
        let (result16, carry) = 0xFF00u16.alu_shr(8);
        assert_eq!(result16, 0x00FF);
        assert_eq!(carry, false);
    }

    #[test]
    fn test_alu_shl() {
        let (result, carry) = 0x80u8.alu_shl(1);
        assert_eq!(result, 0);
        assert_eq!(carry, true);
        let (result, carry) = 0x01u8.alu_shl(7);
        assert_eq!(result, 0x80);
        assert_eq!(carry, false);

        let (result, carry) = 0x0080u16.alu_shl(1);
        assert_eq!(result, 0x0100);
        assert_eq!(carry, false);
        let (result, carry) = 0xFF00u16.alu_shl(8);
        assert_eq!(result, 0x0000);
        assert_eq!(carry, true);
    }

    /*
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
    */

    #[test]
    fn test_alu_rcr() {
        let (result, carry) = 0x01u8.alu_rcr(1, false);
        assert_eq!(result, 0x00);
        assert_eq!(carry, true);
        let (result, carry) = 0x01u8.alu_rcr(3, false);
        assert_eq!(result, 0x40);
        assert_eq!(carry, false);
        let (result, carry) = 0x00u8.alu_rcr(1, true);
        assert_eq!(result, 0x80);
        assert_eq!(carry, false);

        // Test overflow
        let mut existing_carry = false;
        let mut operand: u8 = 0x80;
        let (result, carry) = operand.alu_rcr(1, existing_carry);
        let overflow = (operand & 0x80 == 0 && existing_carry) || (operand & 0x80 != 0 && !existing_carry);
        assert_eq!(result, 0x40);
        assert_eq!(carry, false);
        assert_eq!(overflow, true); // Overflow should be set because HO bit changed from 1 to 0

        operand = 0x04;
        existing_carry = true;

        let (result, carry) = operand.alu_rcr(1, existing_carry);
        let overflow = (operand & 0x80 == 0 && existing_carry) || (operand & 0x80 != 0 && !existing_carry);
        assert_eq!(result, 0x82);
        assert_eq!(carry, false);
        assert_eq!(overflow, true); // Overflow should be set because HO bit changed from 0 to 1
    }

    #[test]
    fn test_alu_rcl() {
        let (result, carry) = 0x80u8.alu_rcl(1, false);
        assert_eq!(result, 0x00);
        assert_eq!(carry, true);
        let (result, carry) = 0x80u8.alu_rcl(2, false);
        assert_eq!(result, 0x01);
        assert_eq!(carry, false);

        // RCL 17 should result in same value
        let (result, carry) = 0xDEADu16.alu_rcl(17, false);
        assert_eq!(result, 0xDEAD);
        assert_eq!(carry, false);

        let (result, carry) = 0xC8A7u16.alu_rcl(255, false);
        assert_eq!(result, 0xC8A7);
        assert_eq!(carry, false);
    }

    #[test]
    fn test_rol() {
        let (result, carry) = 0xAAu8.alu_rol(8);
        assert_eq!(result, 0xAA);
        assert_eq!(carry, false);

        let (result, carry) = 0x55u8.alu_rol(16);
        assert_eq!(result, 0x55);
        assert_eq!(carry, true);

        let (result, carry) = 0x80u8.alu_rol(1);
        assert_eq!(result, 0x01);
        assert_eq!(carry, true);

        let (result, carry) = 0xAAAAu16.alu_rol(16);
        assert_eq!(result, 0xAAAA);
        assert_eq!(carry, false);

        let (result, carry) = 0x5555u16.alu_rol(16);
        assert_eq!(result, 0x5555);
        assert_eq!(carry, true);

        let (result, carry) = 0x8000u16.alu_rol(1);
        assert_eq!(result, 0x0001);
        assert_eq!(carry, true);
    }

    #[test]
    fn test_ror() {
        let (result, carry) = 0xAAu8.alu_ror(8);
        assert_eq!(result, 0xAA);
        assert_eq!(carry, true);

        let (result, carry) = 0x55u8.alu_ror(8);
        assert_eq!(result, 0x55);
        assert_eq!(carry, false);

        let (result, carry) = 0x01u8.alu_ror(1);
        assert_eq!(result, 0x80);
        assert_eq!(carry, true);

        let (result, carry) = 0xAAAAu16.alu_ror(16);
        assert_eq!(result, 0xAAAA);
        assert_eq!(carry, true);

        let (result, carry) = 0x5555u16.alu_ror(16);
        assert_eq!(result, 0x5555);
        assert_eq!(carry, false);

        let (result, carry) = 0x0001u16.alu_ror(1);
        assert_eq!(result, 0x8000);
        assert_eq!(carry, true);
    }
}

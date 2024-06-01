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

    cpu_common::operands.rs

    This module defines operand types common between CPU types.

*/

use crate::cpu_common::{AddressingMode, Register16, Register8};

#[derive(Copy, Clone, Debug)]
pub enum OperandType {
    Immediate8(u8),
    Immediate16(u16),
    Immediate8s(i8),
    Relative8(i8),
    Relative16(i16),
    Offset8(u16),
    Offset16(u16),
    Register8(Register8),
    Register16(Register16),
    AddressingMode(AddressingMode),
    FarAddress(u16, u16),
    M16Pair(u16, u16),
    NoOperand,
    InvalidOperand,
}

#[derive(Copy, Clone, Default, PartialEq)]
pub enum OperandSize {
    #[default]
    NoOperand,
    NoSize,
    Operand8,
    Operand16,
}

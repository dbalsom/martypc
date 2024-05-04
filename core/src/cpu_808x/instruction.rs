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

    cpu_808x::instruction.rs

    Definition of the Instruction struct and related methods.

*/

use crate::cpu_808x::{
    decode::{InstTemplate, DECODE},
    gdr::GdrEntry,
    mnemonic::Mnemonic,
    OperandSize,
    OperandType,
    Segment,
};

#[derive(Clone)]
pub struct Instruction {
    pub decode_idx: usize,
    pub opcode: u8,
    pub prefixes: u32,
    pub address: u32,
    pub size: u32,
    pub mnemonic: Mnemonic,
    pub segment_override: Option<Segment>,
    pub operand1_type: OperandType,
    pub operand1_size: OperandSize,
    pub operand2_type: OperandType,
    pub operand2_size: OperandSize,
}

impl Default for Instruction {
    fn default() -> Self {
        Self {
            decode_idx: 0,
            opcode: 0,
            prefixes: 0,
            address: 0,
            size: 1,
            mnemonic: Mnemonic::NOP,
            segment_override: None,
            operand1_type: OperandType::NoOperand,
            operand1_size: OperandSize::NoOperand,
            operand2_type: OperandType::NoOperand,
            operand2_size: OperandSize::NoOperand,
        }
    }
}

impl Instruction {
    #[inline(always)]
    pub fn decode_ref(&self) -> &InstTemplate {
        &DECODE[self.decode_idx]
    }
    #[inline(always)]
    pub fn gdr(&self) -> &GdrEntry {
        &self.decode_ref().gdr
    }
}

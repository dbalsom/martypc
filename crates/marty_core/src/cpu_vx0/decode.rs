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
use crate::{
    cpu_common::{alu::Xi, Mnemonic, Register16, Register8},
    cpu_vx0::{decode_8080::DECODE_8080, decode_v20::DECODE, gdr::GdrEntry},
};

#[derive(Copy, Clone, Default, PartialEq)]
pub enum OperandTemplate {
    #[default]
    NoTemplate,
    NoOperand,
    Register8Encoded,
    Register16Encoded,
    Immediate8,
    Immediate16,
    Relative8,
    Relative16,
    Register8(Register8),
    Register16(Register16),
    Register16Indirect(Register16),
    Address16,
}

#[derive(Copy, Clone, Default)]
pub struct InstTemplate {
    pub grp: u8,
    pub gdr: GdrEntry,
    pub mc: u16,
    pub xi: Option<Xi>,
    pub mnemonic: Mnemonic,
    pub operand1: crate::cpu_vx0::decode_v20::OperandTemplate,
    pub operand2: crate::cpu_vx0::decode_v20::OperandTemplate,
}
impl InstTemplate {
    pub(crate) const fn constdefault() -> Self {
        Self {
            grp: 0,
            gdr: GdrEntry(0),
            mc: 0,
            xi: None,
            mnemonic: Mnemonic::Invalid,
            operand1: crate::cpu_vx0::decode_v20::OperandTemplate::NoOperand,
            operand2: crate::cpu_vx0::decode_v20::OperandTemplate::NoOperand,
        }
    }
}

pub struct DecodeTable {
    pub table: &'static [InstTemplate],
}

impl Default for DecodeTable {
    fn default() -> Self {
        DecodeTable { table: &DECODE }
    }
}

impl DecodeTable {
    #[inline(always)]
    pub(crate) fn table(&self) -> &'static [InstTemplate] {
        self.table
    }

    #[inline]
    pub(crate) fn set_emulation_table(&mut self) {
        self.table = DECODE_8080.as_slice()
    }
    #[inline]
    pub(crate) fn set_native_table(&mut self) {
        self.table = &DECODE;
    }
}

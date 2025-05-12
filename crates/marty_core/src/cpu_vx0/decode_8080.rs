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

//! Decoding module for the VX0 CPU's 8080 compatibility mode instruction set.

use std::{error::Error, fmt::Display};

use crate::cpu_vx0::{modrm::ModRmByte, *};

use crate::{
    bytequeue::*,
    cpu_vx0::{gdr::GdrEntry},
    cpu_common::{AddressingMode, Instruction, alu::Xi},
};
use crate::cpu_common::{Mnemonic, Segment, OperandType, OPCODE_PREFIX_ES_OVERRIDE, OPCODE_PREFIX_CS_OVERRIDE, OPCODE_PREFIX_SS_OVERRIDE, OPCODE_PREFIX_DS_OVERRIDE, OPCODE_PREFIX_LOCK, OPCODE_PREFIX_REP1, OPCODE_PREFIX_REP2, OPCODE_PREFIX_REP3, OPCODE_PREFIX_REP4, OPCODE_PREFIX_0F, Register8_8080, Register16_8080};
use crate::cpu_common::mnemonic::Mnemonic8080;
use crate::cpu_common::operands::OperandSize;

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

type Ot = OperandTemplate;

#[allow(dead_code)]
#[derive(Debug)]
pub enum InstructionDecodeError {
    UnsupportedOpcode(u8),
    InvalidSegmentRegister,
    ReadOutOfBounds,
    GeneralDecodeError(u8),
    Unimplemented(u8),
}

impl Error for InstructionDecodeError {}
impl Display for InstructionDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            InstructionDecodeError::UnsupportedOpcode(o) => {
                write!(f, "An unsupported opcode was encountered: {:#2x}.", o)
            }
            InstructionDecodeError::InvalidSegmentRegister => {
                write!(f, "An invalid segment register was specified.")
            }
            InstructionDecodeError::ReadOutOfBounds => {
                write!(f, "Unexpected buffer exhaustion while decoding instruction.")
            }
            InstructionDecodeError::GeneralDecodeError(o) => {
                write!(f, "General error decoding opcode {:#2x}.", o)
            }
            InstructionDecodeError::Unimplemented(o) => {
                write!(f, "Decoding of instruction {:#2x} not implemented.", o)
            }
        }
    }
}

#[derive(Copy, Clone, Default)]
pub struct InstTemplate {
    pub gdr: GdrEntry,
    pub mc: u16,
    pub xi: Option<Xi>,
    pub mnemonic: Mnemonic8080,
    pub operand1: OperandTemplate,
    pub operand2: OperandTemplate,
}
impl InstTemplate {
    const fn constdefault() -> Self {
        Self {
            gdr: GdrEntry(0),
            mc: 0,
            xi: None,
            mnemonic: Mnemonic8080::Invalid,
            operand1: Ot::NoOperand,
            operand2: Ot::NoOperand,
        }
    }
}

macro_rules! inst_skip {
    ($init:ident, $ct:literal) => {
        $init.idx += $ct;
    };

}
macro_rules! inst {
    ($opcode:literal, $init:ident, $gdr:literal, $mc:literal, $xi:ident, $m:ident, $o1:expr, $o2:expr) => {
        $init.table[$init.idx] = InstTemplate {
            gdr: GdrEntry($gdr),
            mc: $mc,
            xi: Some(Xi::$xi),
            mnemonic: Mnemonic8080::$m,
            operand1: $o1,
            operand2: $o2,
        };
        $init.idx += 1;
    };
    ($opcode:literal, $init:ident, $gdr:literal, $mc:literal, $m:ident, $o1:expr, $o2:expr) => {
        $init.table[$init.idx] = InstTemplate {
            gdr: GdrEntry($gdr),
            mc: $mc,
            xi: None,
            mnemonic: Mnemonic8080::$m,
            operand1: $o1,
            operand2: $o2,
        };
        $init.idx += 1;
    };
}

pub const REGULAR_OPS_LEN: usize = 368;
pub const TOTAL_OPS_LEN: usize = REGULAR_OPS_LEN + 256;

pub struct TableInitializer {
    pub idx: usize,
    pub table: [InstTemplate; TOTAL_OPS_LEN],
}

impl TableInitializer {
    const fn new() -> Self {
        Self {
            idx: 0,
            table: [InstTemplate::constdefault(); TOTAL_OPS_LEN],
        }
    }
}

#[rustfmt::skip]
pub static DECODE: [InstTemplate; TOTAL_OPS_LEN] = {
    use Register8_8080::*;
    use Register16_8080::*;
    let mut o: TableInitializer = TableInitializer::new();
    inst!( 0x00, o, 0b0100101000000000, 0x000,         NOP,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0x01, o, 0b0100101000000000, 0x000,         LXI,     Ot::Register8(Register8::from_r8_8080(B)),         Ot::Immediate16);
    inst!( 0x02, o, 0b0100101000000000, 0x000,         STAX,    Ot::Register16Indirect(Register16::from_r16_8080(BC)),  Ot::NoOperand);
    inst!( 0x03, o, 0b0100101000000000, 0x000, INC   , INX,     Ot::Register8(Register8::from_r8_8080(B)),         Ot::NoOperand);
    inst!( 0x04, o, 0b0100100010010010, 0x000, INC   , INR,     Ot::Register8(Register8::from_r8_8080(B)),         Ot::NoOperand);
    inst!( 0x05, o, 0b0100100010010010, 0x000, DEC   , DCR,     Ot::Register8(Register8::from_r8_8080(B)),         Ot::NoOperand);
    inst!( 0x06, o, 0b0100000000110010, 0x000,         MVI,     Ot::Register8(Register8::from_r8_8080(B)),         Ot::Immediate8);
    inst!( 0x07, o, 0b0100000000110010, 0x000,         RLC,     Ot::NoOperand,                                          Ot::NoOperand);

    inst!( 0x08, o, 0b0100101000000000, 0x000,         NOP,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0x09, o, 0b0100101000000000, 0x000,         DAD,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0x0A, o, 0b0100101000000000, 0x000,         LDAX,    Ot::Register16Indirect(Register16::from_r16_8080(BC)),  Ot::NoOperand);
    inst!( 0x0B, o, 0b0100101000000000, 0x000, DEC   , DCX,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0x0C, o, 0b0100100010010010, 0x000, INC   , INR,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0x0D, o, 0b0100100010010010, 0x000, DEC   , DCR,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0x0E, o, 0b0100000000110010, 0x000,         MVI,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0x0F, o, 0b0000000000000000, 0x000,         RRC,     Ot::NoOperand,                                          Ot::NoOperand);

    inst!( 0x10, o, 0b0100101000000000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x11, o, 0b0100101000000000, 0x000,         LXI,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x12, o, 0b0100101000000000, 0x000,         STAX,    Ot::Register16Indirect(Register16::from_r16_8080(DE)),                Ot::NoOperand);
    inst!( 0x13, o, 0b0100101000000000, 0x000, INC   , INX,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x14, o, 0b0100100010010010, 0x000, INC   , INR,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x15, o, 0b0100100010010010, 0x000, DEC   , DCR,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x16, o, 0b0100000000110010, 0x000,         MVI,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x17, o, 0b0100000000110010, 0x000,         RAL,     Ot::NoOperand,                Ot::NoOperand);

    inst!( 0x18, o, 0b0100101000000000, 0x000, SBB   , NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x19, o, 0b0100101000000000, 0x000, SBB   , DAD,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x1A, o, 0b0100101000000000, 0x000, SBB   , LDAX,    Ot::Register16Indirect(Register16::from_r16_8080(DE)),                Ot::NoOperand);
    inst!( 0x1B, o, 0b0100101000000000, 0x000, SBB   , DCX,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x1C, o, 0b0100100010010010, 0x000, SBB   , INR,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x1D, o, 0b0100100010010010, 0x000, SBB   , DCR,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x1E, o, 0b0100000000110010, 0x000,         MVI,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x1F, o, 0b0100000000110010, 0x000,         RAR,     Ot::NoOperand,                Ot::NoOperand);

    inst!( 0x20, o, 0b0100101000000000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x21, o, 0b0100101000000000, 0x000, AND   , LXI,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x22, o, 0b0100101000000000, 0x000, AND   , SHLD,    Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x23, o, 0b0100101000000000, 0x000, AND   , INX,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x24, o, 0b0100100010010010, 0x000, AND   , INR,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x25, o, 0b0100100010010010, 0x000, AND   , DCR,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x26, o, 0b0100010000111010, 0x000,         MVI,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x27, o, 0b0101000000110010, 0x000, DAA   , DAA,     Ot::NoOperand,                Ot::NoOperand);

    inst!( 0x28, o, 0b0100101000000000, 0x000, SUB   , NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x29, o, 0b0100101000000000, 0x000, SUB   , DAD,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x2A, o, 0b0100101000000000, 0x000, SUB   , LHLD,    Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x2B, o, 0b0100101000000000, 0x000, SUB   , DCX,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x2C, o, 0b0100100010010010, 0x000, SUB   , INR,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x2D, o, 0b0100100010010010, 0x000, SUB   , DCR,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x2E, o, 0b0100010000111010, 0x000,         MVI,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x2F, o, 0b0101000000110010, 0x000, DAS   , CMA,     Ot::NoOperand,                Ot::NoOperand);

    inst!( 0x30, o, 0b0100101000000000, 0x000, XOR   , NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x31, o, 0b0100101000000000, 0x000, XOR   , LXI,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x32, o, 0b0100101000000000, 0x000, XOR   , STA,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x33, o, 0b0100101000000000, 0x000, XOR   , INX,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x34, o, 0b0100100010010010, 0x000, XOR   , INR,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x35, o, 0b0100100010010010, 0x000, XOR   , DCR,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x36, o, 0b0100010000111010, 0x000,         MVI,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x37, o, 0b0101000000110010, 0x000, AAA   , STC,     Ot::NoOperand,                Ot::NoOperand);

    inst!( 0x38, o, 0b0100101000000000, 0x000, CMP   , NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x39, o, 0b0100101000000000, 0x000, CMP   , DAD,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x3A, o, 0b0100101000000000, 0x000, CMP   , LDA,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x3B, o, 0b0100101000000000, 0x000, CMP   , DCX,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x3C, o, 0b0100100010010010, 0x000, CMP   , INR,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x3D, o, 0b0100100010010010, 0x000, CMP   , DCR,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x3E, o, 0b0100010000111010, 0x000,         MVI,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0x3F, o, 0b0101000000110010, 0x000, AAS   , CMC,     Ot::NoOperand,                Ot::NoOperand);

    inst!( 0x40, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(B)),             Ot::Register8(Register8::from_r8_8080(B)));
    inst!( 0x41, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(B)),             Ot::Register8(Register8::from_r8_8080(C)));
    inst!( 0x42, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(B)),             Ot::Register8(Register8::from_r8_8080(D)));
    inst!( 0x43, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(B)),             Ot::Register8(Register8::from_r8_8080(E)));
    inst!( 0x44, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(B)),             Ot::Register8(Register8::from_r8_8080(H)));
    inst!( 0x45, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(B)),             Ot::Register8(Register8::from_r8_8080(L)));
    inst!( 0x46, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(B)),             Ot::Register16Indirect(Register16::from_r16_8080(HL)));
    inst!( 0x47, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(B)),             Ot::Register8(Register8::from_r8_8080(AC)));

    inst!( 0x48, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(C)),             Ot::Register8(Register8::from_r8_8080(B)));
    inst!( 0x49, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(C)),             Ot::Register8(Register8::from_r8_8080(C)));
    inst!( 0x4A, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(C)),             Ot::Register8(Register8::from_r8_8080(D)));
    inst!( 0x4B, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(C)),             Ot::Register8(Register8::from_r8_8080(E)));
    inst!( 0x4C, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(C)),             Ot::Register8(Register8::from_r8_8080(H)));
    inst!( 0x4D, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(C)),             Ot::Register8(Register8::from_r8_8080(L)));
    inst!( 0x4E, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(C)),             Ot::Register16Indirect(Register16::from_r16_8080(HL)));
    inst!( 0x4F, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(C)),             Ot::Register8(Register8::from_r8_8080(AC)));

    inst!( 0x50, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(D)),             Ot::Register8(Register8::from_r8_8080(B)));
    inst!( 0x51, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(D)),             Ot::Register8(Register8::from_r8_8080(C)));
    inst!( 0x52, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(D)),             Ot::Register8(Register8::from_r8_8080(D)));
    inst!( 0x53, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(D)),             Ot::Register8(Register8::from_r8_8080(E)));
    inst!( 0x54, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(D)),             Ot::Register8(Register8::from_r8_8080(H)));
    inst!( 0x55, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(D)),             Ot::Register8(Register8::from_r8_8080(L)));
    inst!( 0x56, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(D)),             Ot::Register16Indirect(Register16::from_r16_8080(HL)));
    inst!( 0x57, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(D)),             Ot::Register8(Register8::from_r8_8080(AC)));

    inst!( 0x58, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(E)),             Ot::Register8(Register8::from_r8_8080(B)));
    inst!( 0x59, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(E)),             Ot::Register8(Register8::from_r8_8080(C)));
    inst!( 0x5A, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(E)),             Ot::Register8(Register8::from_r8_8080(D)));
    inst!( 0x5B, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(E)),             Ot::Register8(Register8::from_r8_8080(E)));
    inst!( 0x5C, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(E)),             Ot::Register8(Register8::from_r8_8080(H)));
    inst!( 0x5D, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(E)),             Ot::Register8(Register8::from_r8_8080(L)));
    inst!( 0x5E, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(E)),             Ot::Register16Indirect(Register16::from_r16_8080(HL)));
    inst!( 0x5F, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(E)),             Ot::Register8(Register8::from_r8_8080(AC)));

    inst!( 0x60, o, 0b0000000000010000, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(H)),             Ot::Register8(Register8::from_r8_8080(B)));
    inst!( 0x61, o, 0b0000000000010000, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(H)),             Ot::Register8(Register8::from_r8_8080(C)));
    inst!( 0x62, o, 0b0000000000000000, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(H)),             Ot::Register8(Register8::from_r8_8080(D)));
    inst!( 0x63, o, 0b0000000000000000, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(H)),             Ot::Register8(Register8::from_r8_8080(E)));
    inst!( 0x64, o, 0b0000000000011000, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(H)),             Ot::Register8(Register8::from_r8_8080(H)));
    inst!( 0x65, o, 0b0000000000011000, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(H)),             Ot::Register8(Register8::from_r8_8080(L)));
    inst!( 0x66, o, 0b0000000000000000, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(H)),             Ot::Register16Indirect(Register16::from_r16_8080(HL)));
    inst!( 0x67, o, 0b0000000000000000, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(H)),             Ot::Register8(Register8::from_r8_8080(AC)));

    inst!( 0x68, o, 0b0000000000010000, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(L)),             Ot::Register8(Register8::from_r8_8080(B)));
    inst!( 0x69, o, 0b0000000000010000, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(L)),             Ot::Register8(Register8::from_r8_8080(C)));
    inst!( 0x6A, o, 0b0000000000010000, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(L)),             Ot::Register8(Register8::from_r8_8080(D)));
    inst!( 0x6B, o, 0b0000000000000000, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(L)),             Ot::Register8(Register8::from_r8_8080(E)));
    inst!( 0x6C, o, 0b0000000000010000, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(L)),             Ot::Register8(Register8::from_r8_8080(H)));
    inst!( 0x6D, o, 0b0000000000010000, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(L)),             Ot::Register8(Register8::from_r8_8080(L)));
    inst!( 0x6E, o, 0b0000000000010000, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(L)),             Ot::Register16Indirect(Register16::from_r16_8080(HL)));
    inst!( 0x6F, o, 0b0000000000010000, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(L)),             Ot::Register8(Register8::from_r8_8080(AC)));

    inst!( 0x70, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),      Ot::Register8(Register8::from_r8_8080(B)));
    inst!( 0x71, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),      Ot::Register8(Register8::from_r8_8080(C)));
    inst!( 0x72, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),      Ot::Register8(Register8::from_r8_8080(D)));
    inst!( 0x73, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),      Ot::Register8(Register8::from_r8_8080(E)));
    inst!( 0x74, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),      Ot::Register8(Register8::from_r8_8080(H)));
    inst!( 0x75, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),      Ot::Register8(Register8::from_r8_8080(L)));
    inst!( 0x76, o, 0b0000000000110010, 0x000,         HLT,     Ot::NoOperand,                                              Ot::NoOperand);
    inst!( 0x77, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),      Ot::Register8(Register8::from_r8_8080(AC)));

    inst!( 0x78, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(AC)),            Ot::Register8(Register8::from_r8_8080(B)));
    inst!( 0x79, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(AC)),            Ot::Register8(Register8::from_r8_8080(C)));
    inst!( 0x7A, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(AC)),            Ot::Register8(Register8::from_r8_8080(D)));
    inst!( 0x7B, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(AC)),            Ot::Register8(Register8::from_r8_8080(E)));
    inst!( 0x7C, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(AC)),            Ot::Register8(Register8::from_r8_8080(H)));
    inst!( 0x7D, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(AC)),            Ot::Register8(Register8::from_r8_8080(L)));
    inst!( 0x7E, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(AC)),            Ot::Register16Indirect(Register16::from_r16_8080(HL)));
    inst!( 0x7F, o, 0b0000000000110010, 0x000,         MOV,     Ot::Register8(Register8::from_r8_8080(AC)),            Ot::Register8(Register8::from_r8_8080(AC)));

    inst!( 0x80, o, 0b0110100000000000, 0x000, ADD   , ADD,     Ot::Register8(Register8::from_r8_8080(B)),             Ot::NoOperand);
    inst!( 0x81, o, 0b0110100000000000, 0x000, ADD   , ADD,     Ot::Register8(Register8::from_r8_8080(C)),             Ot::NoOperand);
    inst!( 0x82, o, 0b0110100000000000, 0x000, ADD   , ADD,     Ot::Register8(Register8::from_r8_8080(D)),             Ot::NoOperand);
    inst!( 0x83, o, 0b0110100000000000, 0x000, ADD   , ADD,     Ot::Register8(Register8::from_r8_8080(E)),             Ot::NoOperand);
    inst!( 0x84, o, 0b0110100000000000, 0x000, ADD   , ADD,     Ot::Register8(Register8::from_r8_8080(H)),             Ot::NoOperand);
    inst!( 0x85, o, 0b0110100000000000, 0x000, ADD   , ADD,     Ot::Register8(Register8::from_r8_8080(L)),             Ot::NoOperand);
    inst!( 0x86, o, 0b0110100000000000, 0x000, ADD   , ADD,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),      Ot::NoOperand);
    inst!( 0x87, o, 0b0110100000000000, 0x000, ADD   , ADD,     Ot::Register8(Register8::from_r8_8080(AC)),            Ot::NoOperand);

    inst!( 0x88, o, 0b0100101000100010, 0x000, ADC   , ADC,     Ot::Register8(Register8::from_r8_8080(B)),             Ot::NoOperand);
    inst!( 0x89, o, 0b0100101000100010, 0x000, ADC   , ADC,     Ot::Register8(Register8::from_r8_8080(C)),             Ot::NoOperand);
    inst!( 0x8A, o, 0b0100101000100000, 0x000, ADC   , ADC,     Ot::Register8(Register8::from_r8_8080(D)),             Ot::NoOperand);
    inst!( 0x8B, o, 0b0100101000100000, 0x000, ADC   , ADC,     Ot::Register8(Register8::from_r8_8080(E)),             Ot::NoOperand);
    inst!( 0x8C, o, 0b0100001100100010, 0x000, ADC   , ADC,     Ot::Register8(Register8::from_r8_8080(H)),             Ot::NoOperand);
    inst!( 0x8D, o, 0b0100000000100010, 0x000, ADC   , ADC,     Ot::Register8(Register8::from_r8_8080(L)),             Ot::NoOperand);
    inst!( 0x8E, o, 0b0100001100100000, 0x000, ADC   , ADC,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),      Ot::NoOperand);
    inst!( 0x8F, o, 0b0100000000100010, 0x000, ADC   , ADC,     Ot::Register8(Register8::from_r8_8080(AC)),            Ot::NoOperand);

    inst!( 0x90, o, 0b0100000000110010, 0x000, SUB   , SUB,     Ot::Register8(Register8::from_r8_8080(B)),             Ot::NoOperand);
    inst!( 0x91, o, 0b0100000000110010, 0x000, SUB   , SUB,     Ot::Register8(Register8::from_r8_8080(C)),             Ot::NoOperand);
    inst!( 0x92, o, 0b0100000000110010, 0x000, SUB   , SUB,     Ot::Register8(Register8::from_r8_8080(D)),             Ot::NoOperand);
    inst!( 0x93, o, 0b0100000000110010, 0x000, SUB   , SUB,     Ot::Register8(Register8::from_r8_8080(E)),             Ot::NoOperand);
    inst!( 0x94, o, 0b0100000000110010, 0x000, SUB   , SUB,     Ot::Register8(Register8::from_r8_8080(H)),             Ot::NoOperand);
    inst!( 0x95, o, 0b0100000000110010, 0x000, SUB   , SUB,     Ot::Register8(Register8::from_r8_8080(L)),             Ot::NoOperand);
    inst!( 0x96, o, 0b0100000000110010, 0x000, SUB   , SUB,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),      Ot::NoOperand);
    inst!( 0x97, o, 0b0100000000110010, 0x000, SUB   , SUB,     Ot::Register8(Register8::from_r8_8080(AC)),            Ot::NoOperand);

    inst!( 0x98, o, 0b0100000000110010, 0x000, SBB   , SBB,     Ot::Register8(Register8::from_r8_8080(B)),             Ot::NoOperand);
    inst!( 0x99, o, 0b0100000000110010, 0x000, SBB   , SBB,     Ot::Register8(Register8::from_r8_8080(C)),             Ot::NoOperand);
    inst!( 0x9A, o, 0b0100000000110010, 0x000, SBB   , SBB,     Ot::Register8(Register8::from_r8_8080(D)),             Ot::NoOperand);
    inst!( 0x9B, o, 0b0100000000110010, 0x000, SBB   , SBB,     Ot::Register8(Register8::from_r8_8080(E)),             Ot::NoOperand);
    inst!( 0x9C, o, 0b0100000000110010, 0x000, SBB   , SBB,     Ot::Register8(Register8::from_r8_8080(H)),             Ot::NoOperand);
    inst!( 0x9D, o, 0b0100000000110010, 0x000, SBB   , SBB,     Ot::Register8(Register8::from_r8_8080(L)),             Ot::NoOperand);
    inst!( 0x9E, o, 0b0100000000110010, 0x000, SBB   , SBB,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),      Ot::NoOperand);
    inst!( 0x9F, o, 0b0100000000110010, 0x000, SBB   , SBB,     Ot::Register8(Register8::from_r8_8080(AC)),            Ot::NoOperand);

    inst!( 0xA0, o, 0b0100100010110010, 0x000,         ANA,     Ot::Register8(Register8::from_r8_8080(B)),             Ot::NoOperand);
    inst!( 0xA1, o, 0b0100100010110010, 0x000,         ANA,     Ot::Register8(Register8::from_r8_8080(C)),             Ot::NoOperand);
    inst!( 0xA2, o, 0b0100100010110010, 0x000,         ANA,     Ot::Register8(Register8::from_r8_8080(D)),             Ot::NoOperand);
    inst!( 0xA3, o, 0b0100100010110010, 0x000,         ANA,     Ot::Register8(Register8::from_r8_8080(E)),             Ot::NoOperand);
    inst!( 0xA4, o, 0b0100100010110010, 0x000,         ANA,     Ot::Register8(Register8::from_r8_8080(H)),             Ot::NoOperand);
    inst!( 0xA5, o, 0b0100100010110010, 0x000,         ANA,     Ot::Register8(Register8::from_r8_8080(L)),             Ot::NoOperand);
    inst!( 0xA6, o, 0b0100100010110010, 0x000,         ANA,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),      Ot::NoOperand);
    inst!( 0xA7, o, 0b0100100010110010, 0x000,         ANA,     Ot::Register8(Register8::from_r8_8080(AC)),            Ot::NoOperand);

    inst!( 0xA8, o, 0b0100100010110010, 0x000,         XRA,     Ot::Register8(Register8::from_r8_8080(B)),             Ot::NoOperand);
    inst!( 0xA9, o, 0b0100100010110010, 0x000,         XRA,     Ot::Register8(Register8::from_r8_8080(C)),             Ot::NoOperand);
    inst!( 0xAA, o, 0b0100100010110010, 0x000,         XRA,     Ot::Register8(Register8::from_r8_8080(D)),             Ot::NoOperand);
    inst!( 0xAB, o, 0b0100100010110010, 0x000,         XRA,     Ot::Register8(Register8::from_r8_8080(E)),             Ot::NoOperand);
    inst!( 0xAC, o, 0b0100100010110010, 0x000,         XRA,     Ot::Register8(Register8::from_r8_8080(H)),             Ot::NoOperand);
    inst!( 0xAD, o, 0b0100100010110010, 0x000,         XRA,     Ot::Register8(Register8::from_r8_8080(L)),             Ot::NoOperand);
    inst!( 0xAE, o, 0b0100100010110010, 0x000,         XRA,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),      Ot::NoOperand);
    inst!( 0xAF, o, 0b0100100010110010, 0x000,         XRA,     Ot::Register8(Register8::from_r8_8080(AC)),            Ot::NoOperand);

    inst!( 0xB0, o, 0b0100000000110010, 0x000,         ORA,     Ot::Register8(Register8::from_r8_8080(B)),             Ot::NoOperand);
    inst!( 0xB1, o, 0b0100000000110010, 0x000,         ORA,     Ot::Register8(Register8::from_r8_8080(C)),             Ot::NoOperand);
    inst!( 0xB2, o, 0b0100000000110010, 0x000,         ORA,     Ot::Register8(Register8::from_r8_8080(D)),             Ot::NoOperand);
    inst!( 0xB3, o, 0b0100000000110010, 0x000,         ORA,     Ot::Register8(Register8::from_r8_8080(E)),             Ot::NoOperand);
    inst!( 0xB4, o, 0b0100000000110010, 0x000,         ORA,     Ot::Register8(Register8::from_r8_8080(H)),             Ot::NoOperand);
    inst!( 0xB5, o, 0b0100000000110010, 0x000,         ORA,     Ot::Register8(Register8::from_r8_8080(L)),             Ot::NoOperand);
    inst!( 0xB6, o, 0b0100000000110010, 0x000,         ORA,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),      Ot::NoOperand);
    inst!( 0xB7, o, 0b0100000000110010, 0x000,         ORA,     Ot::Register8(Register8::from_r8_8080(AC)),            Ot::NoOperand);

    inst!( 0xB8, o, 0b0100000000110010, 0x000,         CMP,     Ot::Register8(Register8::from_r8_8080(B)),             Ot::NoOperand);
    inst!( 0xB9, o, 0b0100000000110010, 0x000,         CMP,     Ot::Register8(Register8::from_r8_8080(C)),             Ot::NoOperand);
    inst!( 0xBA, o, 0b0100000000110010, 0x000,         CMP,     Ot::Register8(Register8::from_r8_8080(D)),             Ot::NoOperand);
    inst!( 0xBB, o, 0b0100000000110010, 0x000,         CMP,     Ot::Register8(Register8::from_r8_8080(E)),             Ot::NoOperand);
    inst!( 0xBC, o, 0b0100000000110010, 0x000,         CMP,     Ot::Register8(Register8::from_r8_8080(H)),             Ot::NoOperand);
    inst!( 0xBD, o, 0b0100000000110010, 0x000,         CMP,     Ot::Register8(Register8::from_r8_8080(L)),             Ot::NoOperand);
    inst!( 0xBE, o, 0b0100000000110010, 0x000,         CMP,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),      Ot::NoOperand);
    inst!( 0xBF, o, 0b0100000000110010, 0x000,         CMP,     Ot::Register8(Register8::from_r8_8080(AC)),            Ot::NoOperand);

    inst!( 0xC0, o, 0b0100000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xC1, o, 0b0100000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xC2, o, 0b0100000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xC3, o, 0b0100000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xC4, o, 0b0100000000100000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xC5, o, 0b0100000000100000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xC6, o, 0b0100100000100010, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xC7, o, 0b0100100000100010, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);

    inst!( 0xC8, o, 0b0100000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xC9, o, 0b0100000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xCA, o, 0b0100000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xCB, o, 0b0100000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xCC, o, 0b0100000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xCD, o, 0b0100000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xCE, o, 0b0100000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xCF, o, 0b0100000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);

    inst!( 0xD0, o, 0b0100100000000000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xD1, o, 0b0100100000000000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xD2, o, 0b0100100000000000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xD3, o, 0b0100100000000000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xD4, o, 0b0101000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xD5, o, 0b0101000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xD6, o, 0b0101000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xD7, o, 0b0101000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);

    inst!( 0xD8, o, 0b0100000000100000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xD9, o, 0b0100000000100000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xDA, o, 0b0100000000100000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xDB, o, 0b0100000000100000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xDC, o, 0b0100000000100000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xDD, o, 0b0100000000100000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xDE, o, 0b0100000000100000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xDF, o, 0b0100000000100000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);

    inst!( 0xE0, o, 0b0110000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xE1, o, 0b0110000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xE2, o, 0b0110000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xE3, o, 0b0110000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xE4, o, 0b0100100010110011, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xE5, o, 0b0100100010110011, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xE6, o, 0b0100100010110011, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xE7, o, 0b0100100010110011, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);

    inst!( 0xE8, o, 0b0110000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xE9, o, 0b0110000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xEA, o, 0b0110000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xEB, o, 0b0110000000110000, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xEC, o, 0b0100100010110011, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xED, o, 0b0100100010110011, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xEE, o, 0b0100100010110011, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xEF, o, 0b0100100010110011, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);

    inst!( 0xF0, o, 0b0100010000111010, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xF1, o, 0b0100010000111010, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xF2, o, 0b0100010000111010, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xF3, o, 0b0100010000111010, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xF4, o, 0b0100010000110010, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xF5, o, 0b0100010000110010, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xF6, o, 0b0100100000100100, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xF7, o, 0b0100100000100100, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);

    inst!( 0xF8, o, 0b0100010001110010, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xF9, o, 0b0100010001110010, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xFA, o, 0b0100010001110010, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xFB, o, 0b0100010001110010, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xFC, o, 0b0100010001110010, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xFD, o, 0b0100010001110010, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xFE, o, 0b0000100000100100, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);
    inst!( 0xFF, o, 0b0000100000100100, 0x000,         NOP,     Ot::NoOperand,                Ot::NoOperand);


    o.table
};

impl NecVx0 {
    #[rustfmt::skip]
    pub fn decode(bytes: &mut impl ByteQueue, peek: bool) -> Result<Instruction, Box<dyn std::error::Error>> {
        let mut operand1_type: OperandType = OperandType::NoOperand;
        let mut operand2_type: OperandType = OperandType::NoOperand;
        
        let mut opcode = bytes.q_read_u8(QueueType::First, QueueReader::Biu);
        let mut size: u32 = 1;
        let mut op_prefixes: u32 = 0;
        let mut op_segment_override = None;
        let mut decode_idx: usize = 0;

        let mut op_prefix_ct = 0;
        // Read in opcode prefixes until exhausted
        loop {
            // Set flags for all prefixes encountered...
            op_prefixes |= match opcode {
                0x0F => {
                    op_prefixes |= OPCODE_PREFIX_0F;
                    // 0F prefixed-instructions exist in table after all regular Intel instructions
                    // Nothing can follow an 0F prefix; so start instruction now. Fetching the
                    // extended opcode counts as a Subsequent write based on queue status flags.
                    
                    // One cycle delay after reading 0F prefix.
                    bytes.wait(1);
                    opcode = bytes.q_read_u8(QueueType::Subsequent, QueueReader::Biu);
                    decode_idx = REGULAR_OPS_LEN;
                    size += 1;
                    break;
                }
                0x26 => OPCODE_PREFIX_ES_OVERRIDE,
                0x2E => OPCODE_PREFIX_CS_OVERRIDE,
                0x36 => OPCODE_PREFIX_SS_OVERRIDE,
                0x3E => OPCODE_PREFIX_DS_OVERRIDE,
                0xF0 => OPCODE_PREFIX_LOCK,
                0xF1 => OPCODE_PREFIX_LOCK,
                0xF2 => OPCODE_PREFIX_REP1,
                0xF3 => OPCODE_PREFIX_REP2,
                0x64 => OPCODE_PREFIX_REP3,
                0x65 => OPCODE_PREFIX_REP4,
                _=> {
                    break;
                }
            };
            op_prefix_ct += 1;

            // ... but only store the last segment override prefix seen
            op_segment_override = match opcode {
                0x26 => Some(Segment::ES),
                0x2E => Some(Segment::CS),
                0x36 => Some(Segment::SS),
                0x3E => Some(Segment::DS),
                _=> op_segment_override
            };

            // Reading a segment override prefix takes two cycles
            bytes.wait(1);

            // Reset first-fetch flag on each prefix read
            opcode = bytes.q_read_u8(QueueType::First, QueueReader::Biu);
            size += 1;
        }

        // Pack number of prefixes decoded into prefix field (maximum of 3)
        op_prefixes |= std::cmp::min(op_prefix_ct, 3) & 0x03;

        decode_idx += opcode as usize;
        let op_lu = &DECODE[decode_idx];
    
        // Resolve operand templates into OperandTypes
        let mut match_op = |op_template| -> (OperandType, OperandSize) {
            match (op_template, peek) {
                (OperandTemplate::Register8Encoded, _) => {
                    let operand_type = match opcode & OPCODE_REGISTER_SELECT_MASK {
                        0x00 => OperandType::Register8(Register8::AL),
                        0x01 => OperandType::Register8(Register8::CL),
                        0x02 => OperandType::Register8(Register8::DL),
                        0x03 => OperandType::Register8(Register8::BL),
                        0x04 => OperandType::Register8(Register8::AH),
                        0x05 => OperandType::Register8(Register8::CH),
                        0x06 => OperandType::Register8(Register8::DH),
                        0x07 => OperandType::Register8(Register8::BH),
                        _ => OperandType::InvalidOperand
                    };
                    (operand_type, OperandSize::Operand8)
                }
                (OperandTemplate::Register16Encoded, _) => {
                    let operand_type = match opcode & OPCODE_REGISTER_SELECT_MASK {
                        0x00 => OperandType::Register16(Register16::AX),
                        0x01 => OperandType::Register16(Register16::CX),
                        0x02 => OperandType::Register16(Register16::DX),
                        0x03 => OperandType::Register16(Register16::BX),
                        0x04 => OperandType::Register16(Register16::SP),
                        0x05 => OperandType::Register16(Register16::BP),
                        0x06 => OperandType::Register16(Register16::SI),
                        0x07 => OperandType::Register16(Register16::DI),
                        _ => OperandType::InvalidOperand
                    };
                    (operand_type, OperandSize::Operand16)
                }
                (OperandTemplate::Immediate8, true) => {
                    // Peek at immediate value now, fetch during execute
                    let operand = bytes.q_peek_u8();
                    size += 1;
                    (OperandType::Immediate8(operand), OperandSize::Operand8)
                }
                (OperandTemplate::Immediate8, false) => {
                    size += 1;
                    (OperandType::Immediate8(0), OperandSize::Operand8)
                }
                (OperandTemplate::Immediate16, true) => {
                    // Peek at immediate value now, fetch during execute
                    let operand = bytes.q_peek_u16();
                    size += 2;
                    (OperandType::Immediate16(operand), OperandSize::Operand16)
                }
                (OperandTemplate::Immediate16, false) => {
                    size += 2;
                    (OperandType::Immediate16(0), OperandSize::Operand16)
                }
                (OperandTemplate::Relative8, true) => {
                    // Peek at rel8 value now, fetch during execute
                    let operand = bytes.q_peek_i8();
                    size += 1;
                    (OperandType::Relative8(operand), OperandSize::Operand8)
                }
                (OperandTemplate::Relative8, false) => {
                    size += 1;
                    (OperandType::Relative8(0), OperandSize::Operand8)
                }
                (OperandTemplate::Relative16, true) => {
                    // Peek at rel16 value now, fetch during execute
                    let operand = bytes.q_peek_i16();
                    size += 2;
                    (OperandType::Relative16(operand), OperandSize::Operand16)
                }
                (OperandTemplate::Relative16, false) => {
                    size += 2;
                    (OperandType::Relative16(0), OperandSize::Operand16)
                }
                (OperandTemplate::Register8(r8), _) => {
                    (OperandType::Register8(r8), OperandSize::Operand8)
                }
                (OperandTemplate::Register16(r16), _) => {
                    (OperandType::Register16(r16), OperandSize::Operand16)
                }
                _ => (OperandType::NoOperand,OperandSize::NoOperand)
            }
        };

        if !matches!(op_lu.operand1, OperandTemplate::NoTemplate) {
            (operand1_type, _) = match_op(op_lu.operand1);
        }
        if !matches!(op_lu.operand2, OperandTemplate::NoTemplate) {
            (operand2_type, _) = match_op(op_lu.operand2);
        }

        Ok(Instruction {
            decode_idx,
            opcode,
            prefixes: op_prefixes,
            address: 0,
            size,
            width: op_lu.gdr.width(opcode),
            mnemonic: Mnemonic::I8080(op_lu.mnemonic),
            xi: op_lu.xi,
            segment_override: op_segment_override,
            operand1_type,
            operand2_type,
        })
    }
}

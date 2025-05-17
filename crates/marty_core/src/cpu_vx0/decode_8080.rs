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

use crate::cpu_vx0::{*};

use crate::{
    bytequeue::*,
    cpu_vx0::{gdr::GdrEntry, decode::InstTemplate},
    cpu_common::{AddressingMode, Instruction, alu::Xi},
};
use crate::cpu_common::{Mnemonic, Segment, OperandType, Register8_8080, Register16_8080, OPCODE_PREFIX_ED};
use crate::cpu_common::mnemonic::Mnemonic8080;
use crate::cpu_common::operands::OperandSize;

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

macro_rules! inst {
    ($opcode:literal, $init:ident, $gdr:literal, $mc:literal, $m:ident, $o1:expr, $o2:expr) => {
        $init.table[$init.idx] = InstTemplate {
            grp: 0,
            gdr: GdrEntry($gdr),
            mc: $mc,
            xi: None,
            mnemonic: Mnemonic::I8080(Mnemonic8080::$m),
            operand1: $o1,
            operand2: $o2,
        };
        $init.idx += 1;
    };
}

pub const TOTAL_OPS_LEN: usize = 256;

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
pub static DECODE_8080_ED: [InstTemplate; TOTAL_OPS_LEN] = {
    use Register8_8080::*;
    use Register16_8080::*;
    let mut o: TableInitializer = TableInitializer::new();
    inst!( 0xED, o, 0b0000000000000000, 0x000,         CALLN,     Ot::Immediate8, Ot::NoOperand);
    inst!( 0xFD, o, 0b0000000000000000, 0x000,         RETEM,     Ot::NoOperand,  Ot::NoOperand);
    inst!( 0xFF, o, 0b0000000000000000, 0x000,         Invalid,   Ot::NoOperand,  Ot::NoOperand);
    o.table
};

#[rustfmt::skip]
pub static DECODE_8080: [InstTemplate; TOTAL_OPS_LEN] = {
    use Register8_8080::*;
    use Register16_8080::*;
    let mut o: TableInitializer = TableInitializer::new();
    inst!( 0x00, o, 0b0000000000000000, 0x000, NOP,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0x01, o, 0b0000000000000000, 0x000, LXI,     Ot::FixedRegister16(Register16::from_r16_8080(BC)),          Ot::Immediate16);
    inst!( 0x02, o, 0b0000000000000000, 0x000, STAX,    Ot::Register16Indirect(Register16::from_r16_8080(BC)),  Ot::NoOperand);
    inst!( 0x03, o, 0b0000000000000000, 0x000, INX,     Ot::FixedRegister16(Register16::from_r16_8080(BC)),          Ot::NoOperand);
    inst!( 0x04, o, 0b0000000000000000, 0x000, INR,     Ot::FixedRegister8(Register8::from_r8_8080(B)),              Ot::NoOperand);
    inst!( 0x05, o, 0b0000000000000000, 0x000, DCR,     Ot::FixedRegister8(Register8::from_r8_8080(B)),              Ot::NoOperand);
    inst!( 0x06, o, 0b0000000000000000, 0x000, MVI,     Ot::FixedRegister8(Register8::from_r8_8080(B)),              Ot::Immediate8);
    inst!( 0x07, o, 0b0000000000000000, 0x000, RLC,     Ot::NoOperand,                                          Ot::NoOperand);

    inst!( 0x08, o, 0b0000000000000000, 0x000, NOP,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0x09, o, 0b0000000000000000, 0x000, DAD,     Ot::FixedRegister16(Register16::from_r16_8080(BC)),          Ot::NoOperand);
    inst!( 0x0A, o, 0b0000000000000000, 0x000, LDAX,    Ot::Register16Indirect(Register16::from_r16_8080(BC)),  Ot::NoOperand);
    inst!( 0x0B, o, 0b0000000000000000, 0x000, DCX,     Ot::FixedRegister16(Register16::from_r16_8080(BC)),          Ot::NoOperand);
    inst!( 0x0C, o, 0b0000000000000000, 0x000, INR,     Ot::FixedRegister8(Register8::from_r8_8080(C)),              Ot::NoOperand);
    inst!( 0x0D, o, 0b0000000000000000, 0x000, DCR,     Ot::FixedRegister8(Register8::from_r8_8080(C)),              Ot::NoOperand);
    inst!( 0x0E, o, 0b0000000000000000, 0x000, MVI,     Ot::FixedRegister8(Register8::from_r8_8080(C)),              Ot::Immediate8);
    inst!( 0x0F, o, 0b0000000000000000, 0x000, RRC,     Ot::NoOperand,                                          Ot::NoOperand);

    inst!( 0x10, o, 0b0000000000000000, 0x000, NOP,     Ot::Immediate16,                                          Ot::NoOperand);
    inst!( 0x11, o, 0b0000000000000000, 0x000, LXI,     Ot::FixedRegister16(Register16::from_r16_8080(DE)),          Ot::Immediate16);
    inst!( 0x12, o, 0b0000000000000000, 0x000, STAX,    Ot::Register16Indirect(Register16::from_r16_8080(DE)),  Ot::NoOperand);
    inst!( 0x13, o, 0b0000000000000000, 0x000, INX,     Ot::FixedRegister16(Register16::from_r16_8080(DE)),          Ot::NoOperand);
    inst!( 0x14, o, 0b0000000000000000, 0x000, INR,     Ot::FixedRegister8(Register8::from_r8_8080(D)),              Ot::NoOperand);
    inst!( 0x15, o, 0b0000000000000000, 0x000, DCR,     Ot::FixedRegister8(Register8::from_r8_8080(D)),              Ot::NoOperand);
    inst!( 0x16, o, 0b0000000000000000, 0x000, MVI,     Ot::FixedRegister8(Register8::from_r8_8080(D)),              Ot::Immediate8);
    inst!( 0x17, o, 0b0000000000000000, 0x000, RAL,     Ot::NoOperand,                                          Ot::NoOperand);

    inst!( 0x18, o, 0b0000000000000000, 0x000, NOP,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0x19, o, 0b0000000000000000, 0x000, DAD,     Ot::Register16Indirect(Register16::from_r16_8080(DE)),  Ot::NoOperand);
    inst!( 0x1A, o, 0b0000000000000000, 0x000, LDAX,    Ot::Register16Indirect(Register16::from_r16_8080(DE)),  Ot::NoOperand);
    inst!( 0x1B, o, 0b0000000000000000, 0x000, DCX,     Ot::FixedRegister16(Register16::from_r16_8080(DE)),          Ot::NoOperand);
    inst!( 0x1C, o, 0b0000000000000000, 0x000, INR,     Ot::FixedRegister8(Register8::from_r8_8080(E)),              Ot::NoOperand);
    inst!( 0x1D, o, 0b0000000000000000, 0x000, DCR,     Ot::FixedRegister8(Register8::from_r8_8080(E)),              Ot::NoOperand);
    inst!( 0x1E, o, 0b0000000000000000, 0x000, MVI,     Ot::FixedRegister8(Register8::from_r8_8080(E)),              Ot::Immediate8);
    inst!( 0x1F, o, 0b0000000000000000, 0x000, RAR,     Ot::NoOperand,                                          Ot::NoOperand);

    inst!( 0x20, o, 0b0000000000000000, 0x000, NOP,     Ot::Immediate16,                                          Ot::NoOperand);
    inst!( 0x21, o, 0b0000000000000000, 0x000, LXI,     Ot::FixedRegister16(Register16::from_r16_8080(HL)),          Ot::Immediate16);
    inst!( 0x22, o, 0b0000000000000000, 0x000, SHLD,    Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0x23, o, 0b0000000000000000, 0x000, INX,     Ot::FixedRegister16(Register16::from_r16_8080(HL)),          Ot::NoOperand);
    inst!( 0x24, o, 0b0000000000000000, 0x000, INR,     Ot::FixedRegister8(Register8::from_r8_8080(H)),              Ot::NoOperand);
    inst!( 0x25, o, 0b0000000000000000, 0x000, DCR,     Ot::FixedRegister8(Register8::from_r8_8080(H)),              Ot::NoOperand);
    inst!( 0x26, o, 0b0000000000000000, 0x000, MVI,     Ot::FixedRegister8(Register8::from_r8_8080(H)),              Ot::Immediate8);
    inst!( 0x27, o, 0b0000000000000000, 0x000, DAA,     Ot::NoOperand,                                          Ot::NoOperand);

    inst!( 0x28, o, 0b0000000000000000, 0x000, NOP,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0x29, o, 0b0000000000000000, 0x000, DAD,     Ot::FixedRegister16(Register16::from_r16_8080(HL)),          Ot::NoOperand);
    inst!( 0x2A, o, 0b0000000000000000, 0x000, LHLD,    Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0x2B, o, 0b0000000000000000, 0x000, DCX,     Ot::FixedRegister16(Register16::from_r16_8080(HL)),          Ot::NoOperand);
    inst!( 0x2C, o, 0b0000000000000000, 0x000, INR,     Ot::FixedRegister8(Register8::from_r8_8080(L)),              Ot::NoOperand);
    inst!( 0x2D, o, 0b0000000000000000, 0x000, DCR,     Ot::FixedRegister8(Register8::from_r8_8080(L)),              Ot::NoOperand);
    inst!( 0x2E, o, 0b0000000000000000, 0x000, MVI,     Ot::FixedRegister8(Register8::from_r8_8080(L)),              Ot::Immediate8);
    inst!( 0x2F, o, 0b0000000000000000, 0x000, CMA,     Ot::NoOperand,                                          Ot::NoOperand);

    inst!( 0x30, o, 0b0000000000000000, 0x000, NOP,     Ot::Immediate16,                                          Ot::NoOperand);
    inst!( 0x31, o, 0b0000000000000000, 0x000, LXI,     Ot::FixedRegister16(Register16::from_r16_8080(SP)),          Ot::Immediate16);
    inst!( 0x32, o, 0b0000000000000000, 0x000, STA,     Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0x33, o, 0b0000000000000000, 0x000, INX,     Ot::FixedRegister16(Register16::from_r16_8080(SP)),          Ot::NoOperand);
    inst!( 0x34, o, 0b0000000000000000, 0x000, INR,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),  Ot::NoOperand);
    inst!( 0x35, o, 0b0000000000000000, 0x000, DCR,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),  Ot::NoOperand);
    inst!( 0x36, o, 0b0000000000000000, 0x000, MVI,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),  Ot::Immediate8);
    inst!( 0x37, o, 0b0000000000000000, 0x000, STC,     Ot::NoOperand,                                          Ot::NoOperand);

    inst!( 0x38, o, 0b0000000000000000, 0x000, NOP,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0x39, o, 0b0000000000000000, 0x000, DAD,     Ot::FixedRegister16(Register16::from_r16_8080(SP)),          Ot::NoOperand);
    inst!( 0x3A, o, 0b0000000000000000, 0x000, LDA,     Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0x3B, o, 0b0000000000000000, 0x000, DCX,     Ot::FixedRegister16(Register16::from_r16_8080(SP)),          Ot::NoOperand);
    inst!( 0x3C, o, 0b0000000000000000, 0x000, INR,     Ot::FixedRegister8(Register8::from_r8_8080(AC)),             Ot::NoOperand);
    inst!( 0x3D, o, 0b0000000000000000, 0x000, DCR,     Ot::FixedRegister8(Register8::from_r8_8080(AC)),             Ot::NoOperand);
    inst!( 0x3E, o, 0b0000000000000000, 0x000, MVI,     Ot::FixedRegister8(Register8::from_r8_8080(AC)),             Ot::Immediate8);
    inst!( 0x3F, o, 0b0000000000000000, 0x000, CMC,     Ot::NoOperand,                                          Ot::NoOperand);

    inst!( 0x40, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(B)),              Ot::FixedRegister8(Register8::from_r8_8080(B)));
    inst!( 0x41, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(B)),              Ot::FixedRegister8(Register8::from_r8_8080(C)));
    inst!( 0x42, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(B)),              Ot::FixedRegister8(Register8::from_r8_8080(D)));
    inst!( 0x43, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(B)),              Ot::FixedRegister8(Register8::from_r8_8080(E)));
    inst!( 0x44, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(B)),              Ot::FixedRegister8(Register8::from_r8_8080(H)));
    inst!( 0x45, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(B)),              Ot::FixedRegister8(Register8::from_r8_8080(L)));
    inst!( 0x46, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(B)),              Ot::Register16Indirect(Register16::from_r16_8080(HL)));
    inst!( 0x47, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(B)),              Ot::FixedRegister8(Register8::from_r8_8080(AC)));

    inst!( 0x48, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(C)),              Ot::FixedRegister8(Register8::from_r8_8080(B)));
    inst!( 0x49, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(C)),              Ot::FixedRegister8(Register8::from_r8_8080(C)));
    inst!( 0x4A, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(C)),              Ot::FixedRegister8(Register8::from_r8_8080(D)));
    inst!( 0x4B, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(C)),              Ot::FixedRegister8(Register8::from_r8_8080(E)));
    inst!( 0x4C, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(C)),              Ot::FixedRegister8(Register8::from_r8_8080(H)));
    inst!( 0x4D, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(C)),              Ot::FixedRegister8(Register8::from_r8_8080(L)));
    inst!( 0x4E, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(C)),              Ot::Register16Indirect(Register16::from_r16_8080(HL)));
    inst!( 0x4F, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(C)),              Ot::FixedRegister8(Register8::from_r8_8080(AC)));

    inst!( 0x50, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(D)),              Ot::FixedRegister8(Register8::from_r8_8080(B)));
    inst!( 0x51, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(D)),              Ot::FixedRegister8(Register8::from_r8_8080(C)));
    inst!( 0x52, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(D)),              Ot::FixedRegister8(Register8::from_r8_8080(D)));
    inst!( 0x53, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(D)),              Ot::FixedRegister8(Register8::from_r8_8080(E)));
    inst!( 0x54, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(D)),              Ot::FixedRegister8(Register8::from_r8_8080(H)));
    inst!( 0x55, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(D)),              Ot::FixedRegister8(Register8::from_r8_8080(L)));
    inst!( 0x56, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(D)),              Ot::Register16Indirect(Register16::from_r16_8080(HL)));
    inst!( 0x57, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(D)),              Ot::FixedRegister8(Register8::from_r8_8080(AC)));

    inst!( 0x58, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(E)),              Ot::FixedRegister8(Register8::from_r8_8080(B)));
    inst!( 0x59, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(E)),              Ot::FixedRegister8(Register8::from_r8_8080(C)));
    inst!( 0x5A, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(E)),              Ot::FixedRegister8(Register8::from_r8_8080(D)));
    inst!( 0x5B, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(E)),              Ot::FixedRegister8(Register8::from_r8_8080(E)));
    inst!( 0x5C, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(E)),              Ot::FixedRegister8(Register8::from_r8_8080(H)));
    inst!( 0x5D, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(E)),              Ot::FixedRegister8(Register8::from_r8_8080(L)));
    inst!( 0x5E, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(E)),              Ot::Register16Indirect(Register16::from_r16_8080(HL)));
    inst!( 0x5F, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(E)),              Ot::FixedRegister8(Register8::from_r8_8080(AC)));

    inst!( 0x60, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(H)),              Ot::FixedRegister8(Register8::from_r8_8080(B)));
    inst!( 0x61, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(H)),              Ot::FixedRegister8(Register8::from_r8_8080(C)));
    inst!( 0x62, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(H)),              Ot::FixedRegister8(Register8::from_r8_8080(D)));
    inst!( 0x63, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(H)),              Ot::FixedRegister8(Register8::from_r8_8080(E)));
    inst!( 0x64, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(H)),              Ot::FixedRegister8(Register8::from_r8_8080(H)));
    inst!( 0x65, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(H)),              Ot::FixedRegister8(Register8::from_r8_8080(L)));
    inst!( 0x66, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(H)),              Ot::Register16Indirect(Register16::from_r16_8080(HL)));
    inst!( 0x67, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(H)),              Ot::FixedRegister8(Register8::from_r8_8080(AC)));

    inst!( 0x68, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(L)),              Ot::FixedRegister8(Register8::from_r8_8080(B)));
    inst!( 0x69, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(L)),              Ot::FixedRegister8(Register8::from_r8_8080(C)));
    inst!( 0x6A, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(L)),              Ot::FixedRegister8(Register8::from_r8_8080(D)));
    inst!( 0x6B, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(L)),              Ot::FixedRegister8(Register8::from_r8_8080(E)));
    inst!( 0x6C, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(L)),              Ot::FixedRegister8(Register8::from_r8_8080(H)));
    inst!( 0x6D, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(L)),              Ot::FixedRegister8(Register8::from_r8_8080(L)));
    inst!( 0x6E, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(L)),              Ot::Register16Indirect(Register16::from_r16_8080(HL)));
    inst!( 0x6F, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(L)),              Ot::FixedRegister8(Register8::from_r8_8080(AC)));

    inst!( 0x70, o, 0b0000000000000000, 0x000, MOV,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),  Ot::FixedRegister8(Register8::from_r8_8080(B)));
    inst!( 0x71, o, 0b0000000000000000, 0x000, MOV,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),  Ot::FixedRegister8(Register8::from_r8_8080(C)));
    inst!( 0x72, o, 0b0000000000000000, 0x000, MOV,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),  Ot::FixedRegister8(Register8::from_r8_8080(D)));
    inst!( 0x73, o, 0b0000000000000000, 0x000, MOV,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),  Ot::FixedRegister8(Register8::from_r8_8080(E)));
    inst!( 0x74, o, 0b0000000000000000, 0x000, MOV,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),  Ot::FixedRegister8(Register8::from_r8_8080(H)));
    inst!( 0x75, o, 0b0000000000000000, 0x000, MOV,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),  Ot::FixedRegister8(Register8::from_r8_8080(L)));
    inst!( 0x76, o, 0b0000000000000000, 0x000, HLT,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0x77, o, 0b0000000000000000, 0x000, MOV,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),  Ot::FixedRegister8(Register8::from_r8_8080(AC)));

    inst!( 0x78, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(AC)),             Ot::FixedRegister8(Register8::from_r8_8080(B)));
    inst!( 0x79, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(AC)),             Ot::FixedRegister8(Register8::from_r8_8080(C)));
    inst!( 0x7A, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(AC)),             Ot::FixedRegister8(Register8::from_r8_8080(D)));
    inst!( 0x7B, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(AC)),             Ot::FixedRegister8(Register8::from_r8_8080(E)));
    inst!( 0x7C, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(AC)),             Ot::FixedRegister8(Register8::from_r8_8080(H)));
    inst!( 0x7D, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(AC)),             Ot::FixedRegister8(Register8::from_r8_8080(L)));
    inst!( 0x7E, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(AC)),             Ot::Register16Indirect(Register16::from_r16_8080(HL)));
    inst!( 0x7F, o, 0b0000000000000000, 0x000, MOV,     Ot::FixedRegister8(Register8::from_r8_8080(AC)),             Ot::FixedRegister8(Register8::from_r8_8080(AC)));

    inst!( 0x80, o, 0b0000000000000000, 0x000, ADD,     Ot::FixedRegister8(Register8::from_r8_8080(B)),              Ot::NoOperand);
    inst!( 0x81, o, 0b0000000000000000, 0x000, ADD,     Ot::FixedRegister8(Register8::from_r8_8080(C)),              Ot::NoOperand);
    inst!( 0x82, o, 0b0000000000000000, 0x000, ADD,     Ot::FixedRegister8(Register8::from_r8_8080(D)),              Ot::NoOperand);
    inst!( 0x83, o, 0b0000000000000000, 0x000, ADD,     Ot::FixedRegister8(Register8::from_r8_8080(E)),              Ot::NoOperand);
    inst!( 0x84, o, 0b0000000000000000, 0x000, ADD,     Ot::FixedRegister8(Register8::from_r8_8080(H)),              Ot::NoOperand);
    inst!( 0x85, o, 0b0000000000000000, 0x000, ADD,     Ot::FixedRegister8(Register8::from_r8_8080(L)),              Ot::NoOperand);
    inst!( 0x86, o, 0b0000000000000000, 0x000, ADD,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),  Ot::NoOperand);
    inst!( 0x87, o, 0b0000000000000000, 0x000, ADD,     Ot::FixedRegister8(Register8::from_r8_8080(AC)),             Ot::NoOperand);

    inst!( 0x88, o, 0b0000000000000000, 0x000, ADC,     Ot::FixedRegister8(Register8::from_r8_8080(B)),              Ot::NoOperand);
    inst!( 0x89, o, 0b0000000000000000, 0x000, ADC,     Ot::FixedRegister8(Register8::from_r8_8080(C)),              Ot::NoOperand);
    inst!( 0x8A, o, 0b0000000000000000, 0x000, ADC,     Ot::FixedRegister8(Register8::from_r8_8080(D)),              Ot::NoOperand);
    inst!( 0x8B, o, 0b0000000000000000, 0x000, ADC,     Ot::FixedRegister8(Register8::from_r8_8080(E)),              Ot::NoOperand);
    inst!( 0x8C, o, 0b0000000000000000, 0x000, ADC,     Ot::FixedRegister8(Register8::from_r8_8080(H)),              Ot::NoOperand);
    inst!( 0x8D, o, 0b0000000000000000, 0x000, ADC,     Ot::FixedRegister8(Register8::from_r8_8080(L)),              Ot::NoOperand);
    inst!( 0x8E, o, 0b0000000000000000, 0x000, ADC,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),  Ot::NoOperand);
    inst!( 0x8F, o, 0b0000000000000000, 0x000, ADC,     Ot::FixedRegister8(Register8::from_r8_8080(AC)),             Ot::NoOperand);

    inst!( 0x90, o, 0b0000000000000000, 0x000, SUB,     Ot::FixedRegister8(Register8::from_r8_8080(B)),              Ot::NoOperand);
    inst!( 0x91, o, 0b0000000000000000, 0x000, SUB,     Ot::FixedRegister8(Register8::from_r8_8080(C)),              Ot::NoOperand);
    inst!( 0x92, o, 0b0000000000000000, 0x000, SUB,     Ot::FixedRegister8(Register8::from_r8_8080(D)),              Ot::NoOperand);
    inst!( 0x93, o, 0b0000000000000000, 0x000, SUB,     Ot::FixedRegister8(Register8::from_r8_8080(E)),              Ot::NoOperand);
    inst!( 0x94, o, 0b0000000000000000, 0x000, SUB,     Ot::FixedRegister8(Register8::from_r8_8080(H)),              Ot::NoOperand);
    inst!( 0x95, o, 0b0000000000000000, 0x000, SUB,     Ot::FixedRegister8(Register8::from_r8_8080(L)),              Ot::NoOperand);
    inst!( 0x96, o, 0b0000000000000000, 0x000, SUB,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),  Ot::NoOperand);
    inst!( 0x97, o, 0b0000000000000000, 0x000, SUB,     Ot::FixedRegister8(Register8::from_r8_8080(AC)),             Ot::NoOperand);

    inst!( 0x98, o, 0b0000000000000000, 0x000, SBB,     Ot::FixedRegister8(Register8::from_r8_8080(B)),              Ot::NoOperand);
    inst!( 0x99, o, 0b0000000000000000, 0x000, SBB,     Ot::FixedRegister8(Register8::from_r8_8080(C)),              Ot::NoOperand);
    inst!( 0x9A, o, 0b0000000000000000, 0x000, SBB,     Ot::FixedRegister8(Register8::from_r8_8080(D)),              Ot::NoOperand);
    inst!( 0x9B, o, 0b0000000000000000, 0x000, SBB,     Ot::FixedRegister8(Register8::from_r8_8080(E)),              Ot::NoOperand);
    inst!( 0x9C, o, 0b0000000000000000, 0x000, SBB,     Ot::FixedRegister8(Register8::from_r8_8080(H)),              Ot::NoOperand);
    inst!( 0x9D, o, 0b0000000000000000, 0x000, SBB,     Ot::FixedRegister8(Register8::from_r8_8080(L)),              Ot::NoOperand);
    inst!( 0x9E, o, 0b0000000000000000, 0x000, SBB,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),  Ot::NoOperand);
    inst!( 0x9F, o, 0b0000000000000000, 0x000, SBB,     Ot::FixedRegister8(Register8::from_r8_8080(AC)),             Ot::NoOperand);

    inst!( 0xA0, o, 0b0000000000000000, 0x000, ANA,     Ot::FixedRegister8(Register8::from_r8_8080(B)),              Ot::NoOperand);
    inst!( 0xA1, o, 0b0000000000000000, 0x000, ANA,     Ot::FixedRegister8(Register8::from_r8_8080(C)),              Ot::NoOperand);
    inst!( 0xA2, o, 0b0000000000000000, 0x000, ANA,     Ot::FixedRegister8(Register8::from_r8_8080(D)),              Ot::NoOperand);
    inst!( 0xA3, o, 0b0000000000000000, 0x000, ANA,     Ot::FixedRegister8(Register8::from_r8_8080(E)),              Ot::NoOperand);
    inst!( 0xA4, o, 0b0000000000000000, 0x000, ANA,     Ot::FixedRegister8(Register8::from_r8_8080(H)),              Ot::NoOperand);
    inst!( 0xA5, o, 0b0000000000000000, 0x000, ANA,     Ot::FixedRegister8(Register8::from_r8_8080(L)),              Ot::NoOperand);
    inst!( 0xA6, o, 0b0000000000000000, 0x000, ANA,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),  Ot::NoOperand);
    inst!( 0xA7, o, 0b0000000000000000, 0x000, ANA,     Ot::FixedRegister8(Register8::from_r8_8080(AC)),             Ot::NoOperand);

    inst!( 0xA8, o, 0b0000000000000000, 0x000, XRA,     Ot::FixedRegister8(Register8::from_r8_8080(B)),              Ot::NoOperand);
    inst!( 0xA9, o, 0b0000000000000000, 0x000, XRA,     Ot::FixedRegister8(Register8::from_r8_8080(C)),              Ot::NoOperand);
    inst!( 0xAA, o, 0b0000000000000000, 0x000, XRA,     Ot::FixedRegister8(Register8::from_r8_8080(D)),              Ot::NoOperand);
    inst!( 0xAB, o, 0b0000000000000000, 0x000, XRA,     Ot::FixedRegister8(Register8::from_r8_8080(E)),              Ot::NoOperand);
    inst!( 0xAC, o, 0b0000000000000000, 0x000, XRA,     Ot::FixedRegister8(Register8::from_r8_8080(H)),              Ot::NoOperand);
    inst!( 0xAD, o, 0b0000000000000000, 0x000, XRA,     Ot::FixedRegister8(Register8::from_r8_8080(L)),              Ot::NoOperand);
    inst!( 0xAE, o, 0b0000000000000000, 0x000, XRA,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),  Ot::NoOperand);
    inst!( 0xAF, o, 0b0000000000000000, 0x000, XRA,     Ot::FixedRegister8(Register8::from_r8_8080(AC)),             Ot::NoOperand);

    inst!( 0xB0, o, 0b0000000000000000, 0x000, ORA,     Ot::FixedRegister8(Register8::from_r8_8080(B)),              Ot::NoOperand);
    inst!( 0xB1, o, 0b0000000000000000, 0x000, ORA,     Ot::FixedRegister8(Register8::from_r8_8080(C)),              Ot::NoOperand);
    inst!( 0xB2, o, 0b0000000000000000, 0x000, ORA,     Ot::FixedRegister8(Register8::from_r8_8080(D)),              Ot::NoOperand);
    inst!( 0xB3, o, 0b0000000000000000, 0x000, ORA,     Ot::FixedRegister8(Register8::from_r8_8080(E)),              Ot::NoOperand);
    inst!( 0xB4, o, 0b0000000000000000, 0x000, ORA,     Ot::FixedRegister8(Register8::from_r8_8080(H)),              Ot::NoOperand);
    inst!( 0xB5, o, 0b0000000000000000, 0x000, ORA,     Ot::FixedRegister8(Register8::from_r8_8080(L)),              Ot::NoOperand);
    inst!( 0xB6, o, 0b0000000000000000, 0x000, ORA,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),  Ot::NoOperand);
    inst!( 0xB7, o, 0b0000000000000000, 0x000, ORA,     Ot::FixedRegister8(Register8::from_r8_8080(AC)),             Ot::NoOperand);

    inst!( 0xB8, o, 0b0000000000000000, 0x000, CMP,     Ot::FixedRegister8(Register8::from_r8_8080(B)),              Ot::NoOperand);
    inst!( 0xB9, o, 0b0000000000000000, 0x000, CMP,     Ot::FixedRegister8(Register8::from_r8_8080(C)),              Ot::NoOperand);
    inst!( 0xBA, o, 0b0000000000000000, 0x000, CMP,     Ot::FixedRegister8(Register8::from_r8_8080(D)),              Ot::NoOperand);
    inst!( 0xBB, o, 0b0000000000000000, 0x000, CMP,     Ot::FixedRegister8(Register8::from_r8_8080(E)),              Ot::NoOperand);
    inst!( 0xBC, o, 0b0000000000000000, 0x000, CMP,     Ot::FixedRegister8(Register8::from_r8_8080(H)),              Ot::NoOperand);
    inst!( 0xBD, o, 0b0000000000000000, 0x000, CMP,     Ot::FixedRegister8(Register8::from_r8_8080(L)),              Ot::NoOperand);
    inst!( 0xBE, o, 0b0000000000000000, 0x000, CMP,     Ot::Register16Indirect(Register16::from_r16_8080(HL)),  Ot::NoOperand);
    inst!( 0xBF, o, 0b0000000000000000, 0x000, CMP,     Ot::FixedRegister8(Register8::from_r8_8080(AC)),             Ot::NoOperand);

    inst!( 0xC0, o, 0b0000000000000000, 0x000, RNZ,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0xC1, o, 0b0000000000000000, 0x000, POP,     Ot::FixedRegister16(Register16::from_r16_8080(BC)),          Ot::NoOperand);
    inst!( 0xC2, o, 0b0000000000000000, 0x000, JNZ,     Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0xC3, o, 0b0000000000000000, 0x000, JMP,     Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0xC4, o, 0b0000000000000000, 0x000, CNZ,     Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0xC5, o, 0b0000000000000000, 0x000, PUSH,    Ot::FixedRegister16(Register16::from_r16_8080(BC)),          Ot::NoOperand);
    inst!( 0xC6, o, 0b0000000000000000, 0x000, ADI,     Ot::Immediate8,                                         Ot::NoOperand);
    inst!( 0xC7, o, 0b0000000000000000, 0x000, RST0,    Ot::NoOperand,                                          Ot::NoOperand);

    inst!( 0xC8, o, 0b0000000000000000, 0x000, RZ,      Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0xC9, o, 0b0000000000000000, 0x000, RET,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0xCA, o, 0b0000000000000000, 0x000, JZ,      Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0xCB, o, 0b0000000000000000, 0x000, Invalid, Ot::Immediate16,                                          Ot::NoOperand);
    inst!( 0xCC, o, 0b0000000000000000, 0x000, CZ,      Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0xCD, o, 0b0000000000000000, 0x000, CALL,    Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0xCE, o, 0b0000000000000000, 0x000, ACI,     Ot::Immediate8,                                         Ot::NoOperand);
    inst!( 0xCF, o, 0b0000000000000000, 0x000, RST1,    Ot::NoOperand,                                          Ot::NoOperand);

    inst!( 0xD0, o, 0b0000000000000000, 0x000, RNC,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0xD1, o, 0b0000000000000000, 0x000, POP,     Ot::FixedRegister16(Register16::from_r16_8080(DE)),          Ot::NoOperand);
    inst!( 0xD2, o, 0b0000000000000000, 0x000, JNC,     Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0xD3, o, 0b0000000000000000, 0x000, OUT,     Ot::Immediate8,                                         Ot::NoOperand);
    inst!( 0xD4, o, 0b0000000000000000, 0x000, CNC,     Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0xD5, o, 0b0000000000000000, 0x000, PUSH,    Ot::FixedRegister16(Register16::from_r16_8080(DE)),          Ot::NoOperand);
    inst!( 0xD6, o, 0b0000000000000000, 0x000, SUI,     Ot::Immediate8,                                         Ot::NoOperand);
    inst!( 0xD7, o, 0b0000000000000000, 0x000, RST2,    Ot::NoOperand,                                          Ot::NoOperand);

    inst!( 0xD8, o, 0b0000000000000000, 0x000, RC,      Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0xD9, o, 0b0000000000000000, 0x000, Invalid, Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0xDA, o, 0b0000000000000000, 0x000, JC,      Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0xDB, o, 0b0000000000000000, 0x000, IN,      Ot::Immediate8,                                         Ot::NoOperand);
    inst!( 0xDC, o, 0b0000000000000000, 0x000, CC,      Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0xDD, o, 0b0000000000000000, 0x000, Invalid, Ot::Immediate8,                                         Ot::NoOperand);
    inst!( 0xDE, o, 0b0000000000000000, 0x000, SBI,     Ot::Immediate8,                                         Ot::NoOperand);
    inst!( 0xDF, o, 0b0000000000000000, 0x000, RST3,    Ot::NoOperand,                                          Ot::NoOperand);

    inst!( 0xE0, o, 0b0000000000000000, 0x000, RPO,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0xE1, o, 0b0000000000000000, 0x000, POP,     Ot::FixedRegister16(Register16::from_r16_8080(HL)),          Ot::NoOperand);
    inst!( 0xE2, o, 0b0000000000000000, 0x000, JPO,     Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0xE3, o, 0b0000000000000000, 0x000, NOP,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0xE4, o, 0b0000000000000000, 0x000, CPO,     Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0xE5, o, 0b0000000000000000, 0x000, PUSH,    Ot::FixedRegister16(Register16::from_r16_8080(HL)),          Ot::NoOperand);
    inst!( 0xE6, o, 0b0000000000000000, 0x000, ANI,     Ot::Immediate8,                                         Ot::NoOperand);
    inst!( 0xE7, o, 0b0000000000000000, 0x000, RST4,    Ot::NoOperand,                                          Ot::NoOperand);

    inst!( 0xE8, o, 0b0000000000000000, 0x000, RPE,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0xE9, o, 0b0000000000000000, 0x000, PCHL,    Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0xEA, o, 0b0000000000000000, 0x000, JPE,     Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0xEB, o, 0b0000000000000000, 0x000, NOP,     Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0xEC, o, 0b0000000000000000, 0x000, CPE,     Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0xED, o, 0b0000000000000000, 0x000, NOP,     Ot::NoOperand,                                          Ot::NoOperand); // ED prefix
    inst!( 0xEE, o, 0b0000000000000000, 0x000, XRI,     Ot::Immediate8,                                         Ot::NoOperand);
    inst!( 0xEF, o, 0b0000000000000000, 0x000, RST5,    Ot::NoOperand,                                          Ot::NoOperand);

    inst!( 0xF0, o, 0b0000000000000000, 0x000, RP,      Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0xF1, o, 0b0000000000000000, 0x000, POPF,    Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0xF2, o, 0b0000000000000000, 0x000, JP,      Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0xF3, o, 0b0000000000000000, 0x000, DI,      Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0xF4, o, 0b0000000000000000, 0x000, CP,      Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0xF5, o, 0b0000000000000000, 0x000, PUSHF,   Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0xF6, o, 0b0000000000000000, 0x000, ORI,     Ot::Immediate8,                                         Ot::NoOperand);
    inst!( 0xF7, o, 0b0000000000000000, 0x000, RST6,    Ot::NoOperand,                                          Ot::NoOperand);

    inst!( 0xF8, o, 0b0000000000000000, 0x000, RM,      Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0xF9, o, 0b0000000000000000, 0x000, SPHL,    Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0xFA, o, 0b0000000000000000, 0x000, JM,      Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0xFB, o, 0b0000000000000000, 0x000, EI,      Ot::NoOperand,                                          Ot::NoOperand);
    inst!( 0xFC, o, 0b0000000000000000, 0x000, CM,      Ot::Immediate16,                                        Ot::NoOperand);
    inst!( 0xFD, o, 0b0000000000000000, 0x000, Invalid, Ot::Immediate8,                                         Ot::NoOperand);
    inst!( 0xFE, o, 0b0000000000000000, 0x000, CPI,     Ot::Immediate8,                                         Ot::NoOperand);
    inst!( 0xFF, o, 0b0000000000000000, 0x000, RST7,    Ot::NoOperand,                                          Ot::NoOperand);


    o.table
};

impl NecVx0 {
    #[rustfmt::skip]
    pub fn decode_8080(bytes: &mut impl ByteQueue, peek: bool) -> Result<Instruction, Box<dyn std::error::Error>> {
        let mut operand1_type: OperandType = OperandType::NoOperand;
        let mut operand2_type: OperandType = OperandType::NoOperand;
        
        let mut opcode = bytes.q_read_u8(QueueType::First, QueueReader::Biu);
        let mut size: u32 = 1;
        let mut op_prefixes: u32 = 0;
        let mut op_prefix_ct = 0;
        // Read in opcode prefixes until exhausted
        let op_lu = if opcode == 0xED {
            // ED prefix is special, it is not a prefix but an opcode
            // for CALLN or RETEM.  One cycle delay after reading ED prefix.
            bytes.wait(1);
            let secondary_opcode = bytes.q_read_u8(QueueType::Subsequent, QueueReader::Biu);
            op_prefix_ct += 1;
            size += 1;

            match secondary_opcode {
                0xED => {
                    &DECODE_8080_ED[0]
                }
                0xFD => {
                    &DECODE_8080_ED[1]
                }
                _ => {
                    &DECODE_8080_ED[2]
                }
            }
        }
        else {
            &DECODE_8080[opcode as usize]
        };

        // Pack number of prefixes decoded into prefix field (maximum of 3)
        op_prefixes |= std::cmp::min(op_prefix_ct, 3) & 0x03;

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
                (OperandTemplate::FixedRegister8(r8), _) => {
                    (OperandType::Register8(r8), OperandSize::Operand8)
                }
                (OperandTemplate::FixedRegister16(r16), _) => {
                    (OperandType::Register16(r16), OperandSize::Operand16)
                }
                (OperandTemplate::Register16Indirect(r16), _) => {
                    (OperandType::AddressingMode(AddressingMode::RegisterIndirect(r16), OperandSize::Operand8), OperandSize::Operand16)
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
            decode_idx: opcode as usize,
            opcode,
            prefixes: op_prefixes,
            address: 0,
            size,
            width: op_lu.gdr.width(opcode),
            mnemonic: op_lu.mnemonic,
            xi: op_lu.xi,
            segment_override: None,
            operand1_type,
            operand2_type,
        })
    }
}

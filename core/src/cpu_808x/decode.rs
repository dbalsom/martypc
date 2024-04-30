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

    cpu_808x::decode.rs

    Opcode fetch and instruction decoding routines.

    This module is implemented as an associated function, decode(), which
    operates on implementors of ByteQueue. This allows instruction decoding
    from either the processor instruction queue emulation, or directly
    from emulator memory for our debug disassembly viewer.

*/

use std::{error::Error, fmt::Display};

use crate::cpu_808x::{addressing::AddressingMode, mnemonic::Mnemonic, modrm::ModRmByte, *};

use crate::{bytequeue::*, cpu_808x::alu::Xi};

#[derive(Copy, Clone, PartialEq)]
pub enum OperandTemplate {
    NoTemplate,
    NoOperand,
    ModRM8,
    ModRM16,
    Register8,
    Register16,
    SegmentRegister,
    Register8Encoded,
    Register16Encoded,
    Immediate8,
    Immediate16,
    Immediate8SignExtended,
    Relative8,
    Relative16,
    Offset8,
    Offset16,
    FixedRegister8(Register8),
    FixedRegister16(Register16),
    //NearAddress,
    FarAddress,
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

pub struct InstTemplate {
    pub grp: u8,
    pub gdr: u16,
    pub mc: u16,
    pub xi: Option<Xi>,
    pub mnemonic: Mnemonic,
    pub operand1: OperandTemplate,
    pub operand2: OperandTemplate,
}

macro_rules! inst {
    ($op:literal, $grp:literal, $gdr:literal, $mc:literal, $xi:ident, $m:ident, $o1:expr, $o2:expr) => {
        InstTemplate {
            grp: $grp,
            gdr: $gdr,
            mc: $mc,
            xi: Some(Xi::$xi),
            mnemonic: Mnemonic::$m,
            operand1: $o1,
            operand2: $o2,
        }
    };
    ($op:literal, $grp:literal, $gdr:literal, $mc:literal, $m:ident, $o1:expr, $o2:expr) => {
        InstTemplate {
            grp: $grp,
            gdr: $gdr,
            mc: $mc,
            xi: None,
            mnemonic: Mnemonic::$m,
            operand1: $o1,
            operand2: $o2,
        }
    };
}

#[rustfmt::skip]
const DECODE: [InstTemplate; 352] = [
    inst!( 0x00,  0, 0b0100101000000000, 0x008, ADD   , ADD,     Ot::ModRM8,                             Ot::Register8),
    inst!( 0x01,  0, 0b0100101000000000, 0x008, ADD   , ADD,     Ot::ModRM16,                            Ot::Register16),
    inst!( 0x02,  0, 0b0100101000000000, 0x008, ADD   , ADD,     Ot::Register8,                          Ot::ModRM8),
    inst!( 0x03,  0, 0b0100101000000000, 0x008, ADD   , ADD,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0x04,  0, 0b0100100010010010, 0x018, ADD   , ADD,     Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0x05,  0, 0b0100100010010010, 0x018, ADD   , ADD,     Ot::FixedRegister16(Register16::AX),    Ot::Immediate16),
    inst!( 0x06,  0, 0b0100000000110010, 0x02c,         PUSH,    Ot::FixedRegister16(Register16::ES),    Ot::NoOperand),
    inst!( 0x07,  0, 0b0100000000110010, 0x038,         POP,     Ot::FixedRegister16(Register16::ES),    Ot::NoOperand),
    inst!( 0x08,  0, 0b0100101000000000, 0x008, OR    , OR,      Ot::ModRM8,                             Ot::Register8),
    inst!( 0x09,  0, 0b0100101000000000, 0x008, OR    , OR,      Ot::ModRM16,                            Ot::Register16),
    inst!( 0x0A,  0, 0b0100101000000000, 0x008, OR    , OR,      Ot::Register8,                          Ot::ModRM8),
    inst!( 0x0B,  0, 0b0100101000000000, 0x008, OR    , OR,      Ot::Register16,                         Ot::ModRM16),
    inst!( 0x0C,  0, 0b0100100010010010, 0x018, OR    , OR,      Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0x0D,  0, 0b0100100010010010, 0x018, OR    , OR,      Ot::FixedRegister16(Register16::AX),    Ot::Immediate16),
    inst!( 0x0E,  0, 0b0100000000110010, 0x02c,         PUSH,    Ot::FixedRegister16(Register16::CS),    Ot::NoOperand),
    inst!( 0x0F,  0, 0b0100000000110010, 0x038,         POP,     Ot::FixedRegister16(Register16::CS),    Ot::NoOperand),
    inst!( 0x10,  0, 0b0100101000000000, 0x008, ADC   , ADC,     Ot::ModRM8,                             Ot::Register8),
    inst!( 0x11,  0, 0b0100101000000000, 0x008, ADC   , ADC,     Ot::ModRM16,                            Ot::Register16),
    inst!( 0x12,  0, 0b0100101000000000, 0x008, ADC   , ADC,     Ot::Register8,                          Ot::ModRM8),
    inst!( 0x13,  0, 0b0100101000000000, 0x008, ADC   , ADC,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0x14,  0, 0b0100100010010010, 0x018, ADC   , ADC,     Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0x15,  0, 0b0100100010010010, 0x018, ADC   , ADC,     Ot::FixedRegister16(Register16::AX),    Ot::Immediate16),
    inst!( 0x16,  0, 0b0100000000110010, 0x02c,         PUSH,    Ot::FixedRegister16(Register16::SS),    Ot::NoOperand),
    inst!( 0x17,  0, 0b0100000000110010, 0x038,         POP,     Ot::FixedRegister16(Register16::SS),    Ot::NoOperand),
    inst!( 0x18,  0, 0b0100101000000000, 0x008, SBB   , SBB,     Ot::ModRM8,                             Ot::Register8),
    inst!( 0x19,  0, 0b0100101000000000, 0x008, SBB   , SBB,     Ot::ModRM16,                            Ot::Register16),
    inst!( 0x1A,  0, 0b0100101000000000, 0x008, SBB   , SBB,     Ot::Register8,                          Ot::ModRM8),
    inst!( 0x1B,  0, 0b0100101000000000, 0x008, SBB   , SBB,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0x1C,  0, 0b0100100010010010, 0x018, SBB   , SBB,     Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0x1D,  0, 0b0100100010010010, 0x018, SBB   , SBB,     Ot::FixedRegister16(Register16::AX),    Ot::Immediate16),
    inst!( 0x1E,  0, 0b0100000000110010, 0x02c,         PUSH,    Ot::FixedRegister16(Register16::DS),    Ot::NoOperand),
    inst!( 0x1F,  0, 0b0100000000110010, 0x038,         POP,     Ot::FixedRegister16(Register16::DS),    Ot::NoOperand),
    inst!( 0x20,  0, 0b0100101000000000, 0x008, AND   , AND,     Ot::ModRM8,                             Ot::Register8),
    inst!( 0x21,  0, 0b0100101000000000, 0x008, AND   , AND,     Ot::ModRM16,                            Ot::Register16),
    inst!( 0x22,  0, 0b0100101000000000, 0x008, AND   , AND,     Ot::Register8,                          Ot::ModRM8),
    inst!( 0x23,  0, 0b0100101000000000, 0x008, AND   , AND,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0x24,  0, 0b0100100010010010, 0x018, AND   , AND,     Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0x25,  0, 0b0100100010010010, 0x018, AND   , AND,     Ot::FixedRegister16(Register16::AX),    Ot::Immediate16),
    inst!( 0x26,  0, 0b0100010000111010, 0x1FF,         Prefix,  Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x27,  0, 0b0101000000110010, 0x144, DAA   , DAA,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x28,  0, 0b0100101000000000, 0x008, SUB   , SUB,     Ot::ModRM8,                             Ot::Register8),
    inst!( 0x29,  0, 0b0100101000000000, 0x008, SUB   , SUB,     Ot::ModRM16,                            Ot::Register16),
    inst!( 0x2A,  0, 0b0100101000000000, 0x008, SUB   , SUB,     Ot::Register8,                          Ot::ModRM8),
    inst!( 0x2B,  0, 0b0100101000000000, 0x008, SUB   , SUB,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0x2C,  0, 0b0100100010010010, 0x018, SUB   , SUB,     Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0x2D,  0, 0b0100100010010010, 0x018, SUB   , SUB,     Ot::FixedRegister16(Register16::AX),    Ot::Immediate16),
    inst!( 0x2E,  0, 0b0100010000111010, 0x1FF,         Prefix,  Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x2F,  0, 0b0101000000110010, 0x144, DAS   , DAS,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x30,  0, 0b0100101000000000, 0x008, XOR   , XOR,     Ot::ModRM8,                             Ot::Register8),
    inst!( 0x31,  0, 0b0100101000000000, 0x008, XOR   , XOR,     Ot::ModRM16,                            Ot::Register16),
    inst!( 0x32,  0, 0b0100101000000000, 0x008, XOR   , XOR,     Ot::Register8,                          Ot::ModRM8),
    inst!( 0x33,  0, 0b0100101000000000, 0x008, XOR   , XOR,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0x34,  0, 0b0100100010010010, 0x018, XOR   , XOR,     Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0x35,  0, 0b0100100010010010, 0x018, XOR   , XOR,     Ot::FixedRegister16(Register16::AX),    Ot::Immediate16),
    inst!( 0x36,  0, 0b0100010000111010, 0x1FF,         Prefix,  Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x37,  0, 0b0101000000110010, 0x148, AAA   , AAA,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x38,  0, 0b0100101000000000, 0x008, CMP   , CMP,     Ot::ModRM8,                             Ot::Register8),
    inst!( 0x39,  0, 0b0100101000000000, 0x008, CMP   , CMP,     Ot::ModRM16,                            Ot::Register16),
    inst!( 0x3A,  0, 0b0100101000000000, 0x008, CMP   , CMP,     Ot::Register8,                          Ot::ModRM8),
    inst!( 0x3B,  0, 0b0100101000000000, 0x008, CMP   , CMP,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0x3C,  0, 0b0100100010010010, 0x018, CMP   , CMP,     Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0x3D,  0, 0b0100100010010010, 0x018, CMP   , CMP,     Ot::FixedRegister16(Register16::AX),    Ot::Immediate16),
    inst!( 0x3E,  0, 0b0100010000111010, 0x1FF,         Prefix,  Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x3F,  0, 0b0101000000110010, 0x148, AAS   , AAS,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x40,  0, 0b0000000000110010, 0x17c, INC   , INC,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x41,  0, 0b0000000000110010, 0x17c, INC   , INC,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x42,  0, 0b0000000000110010, 0x17c, INC   , INC,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x43,  0, 0b0000000000110010, 0x17c, INC   , INC,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x44,  0, 0b0000000000110010, 0x17c, INC   , INC,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x45,  0, 0b0000000000110010, 0x17c, INC   , INC,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x46,  0, 0b0000000000110010, 0x17c, INC   , INC,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x47,  0, 0b0000000000110010, 0x17c, INC   , INC,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x48,  0, 0b0000000000110010, 0x17c, DEC   , DEC,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x49,  0, 0b0000000000110010, 0x17c, DEC   , DEC,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x4A,  0, 0b0000000000110010, 0x17c, DEC   , DEC,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x4B,  0, 0b0000000000110010, 0x17c, DEC   , DEC,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x4C,  0, 0b0000000000110010, 0x17c, DEC   , DEC,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x4D,  0, 0b0000000000110010, 0x17c, DEC   , DEC,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x4E,  0, 0b0000000000110010, 0x17c, DEC   , DEC,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x4F,  0, 0b0000000000110010, 0x17c, DEC   , DEC,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x50,  0, 0b0000000000110010, 0x028,         PUSH,    Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x51,  0, 0b0000000000110010, 0x028,         PUSH,    Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x52,  0, 0b0000000000110010, 0x028,         PUSH,    Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x53,  0, 0b0000000000110010, 0x028,         PUSH,    Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x54,  0, 0b0000000000110010, 0x028,         PUSH,    Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x55,  0, 0b0000000000110010, 0x028,         PUSH,    Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x56,  0, 0b0000000000110010, 0x028,         PUSH,    Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x57,  0, 0b0000000000110010, 0x028,         PUSH,    Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x58,  0, 0b0000000000110010, 0x034,         POP,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x59,  0, 0b0000000000110010, 0x034,         POP,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x5A,  0, 0b0000000000110010, 0x034,         POP,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x5B,  0, 0b0000000000110010, 0x034,         POP,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x5C,  0, 0b0000000000110010, 0x034,         POP,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x5D,  0, 0b0000000000110010, 0x034,         POP,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x5E,  0, 0b0000000000110010, 0x034,         POP,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x5F,  0, 0b0000000000110010, 0x034,         POP,     Ot::Register16Encoded,                  Ot::NoOperand),
    inst!( 0x60,  0, 0b0000000000110010, 0x0e8,         JO,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x61,  0, 0b0000000000110010, 0x0e8,         JNO,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x62,  0, 0b0000000000110010, 0x0e8,         JB,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x63,  0, 0b0000000000110010, 0x0e8,         JNB,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x64,  0, 0b0000000000110010, 0x0e8,         JZ,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x65,  0, 0b0000000000110010, 0x0e8,         JNZ,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x66,  0, 0b0000000000110010, 0x0e8,         JBE,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x67,  0, 0b0000000000110010, 0x0e8,         JNBE,    Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x68,  0, 0b0000000000110010, 0x0e8,         JS,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x69,  0, 0b0000000000110010, 0x0e8,         JNS,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x6A,  0, 0b0000000000110010, 0x0e8,         JP,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x6B,  0, 0b0000000000110010, 0x0e8,         JNP,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x6C,  0, 0b0000000000110010, 0x0e8,         JL,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x6D,  0, 0b0000000000110010, 0x0e8,         JNL,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x6E,  0, 0b0000000000110010, 0x0e8,         JLE,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x6F,  0, 0b0000000000110010, 0x0e8,         JNLE,    Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x70,  0, 0b0000000000110010, 0x0e8,         JO,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x71,  0, 0b0000000000110010, 0x0e8,         JNO,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x72,  0, 0b0000000000110010, 0x0e8,         JB,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x73,  0, 0b0000000000110010, 0x0e8,         JNB,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x74,  0, 0b0000000000110010, 0x0e8,         JZ,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x75,  0, 0b0000000000110010, 0x0e8,         JNZ,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x76,  0, 0b0000000000110010, 0x0e8,         JBE,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x77,  0, 0b0000000000110010, 0x0e8,         JNBE,    Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x78,  0, 0b0000000000110010, 0x0e8,         JS,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x79,  0, 0b0000000000110010, 0x0e8,         JNS,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x7A,  0, 0b0000000000110010, 0x0e8,         JP,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x7B,  0, 0b0000000000110010, 0x0e8,         JNP,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x7C,  0, 0b0000000000110010, 0x0e8,         JL,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x7D,  0, 0b0000000000110010, 0x0e8,         JNL,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x7E,  0, 0b0000000000110010, 0x0e8,         JLE,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x7F,  0, 0b0000000000110010, 0x0e8,         JNLE,    Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x80,  1, 0b0110100000000000, 0x00c, ADD   , Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x81,  2, 0b0110100000000000, 0x00c, CMP   , Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x82,  3, 0b0110100000000000, 0x00c, ADD   , Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x83,  4, 0b0110100000000000, 0x00c, CMP   , Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x84,  0, 0b0110100000000000, 0x094,         TEST,    Ot::ModRM8,                             Ot::Register8),
    inst!( 0x85,  0, 0b0110100000000000, 0x094,         TEST,    Ot::ModRM16,                            Ot::Register16),
    inst!( 0x86,  0, 0b0110100000000000, 0x0a4,         XCHG,    Ot::Register8,                          Ot::ModRM8),
    inst!( 0x87,  0, 0b0110100000000000, 0x0a4,         XCHG,    Ot::Register16,                         Ot::ModRM16),
    inst!( 0x88,  0, 0b0100101000100010, 0x000,         MOV,     Ot::ModRM8,                             Ot::Register8),
    inst!( 0x89,  0, 0b0100101000100010, 0x000,         MOV,     Ot::ModRM16,                            Ot::Register16),
    inst!( 0x8A,  0, 0b0100101000100000, 0x000,         MOV,     Ot::Register8,                          Ot::ModRM8),
    inst!( 0x8B,  0, 0b0100101000100000, 0x000,         MOV,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0x8C,  0, 0b0100001100100010, 0x0ec,         MOV,     Ot::ModRM16,                            Ot::SegmentRegister),
    inst!( 0x8D,  0, 0b0100000000100010, 0x004,         LEA,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0x8E,  0, 0b0100001100100000, 0x0ec,         MOV,     Ot::SegmentRegister,                    Ot::ModRM16),
    inst!( 0x8F,  0, 0b0100000000100010, 0x040,         POP,     Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0x90,  0, 0b0100000000110010, 0x084,         XCHG,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x91,  0, 0b0100000000110010, 0x084,         XCHG,    Ot::Register16Encoded,                  Ot::FixedRegister16(Register16::AX)),
    inst!( 0x92,  0, 0b0100000000110010, 0x084,         XCHG,    Ot::Register16Encoded,                  Ot::FixedRegister16(Register16::AX)),
    inst!( 0x93,  0, 0b0100000000110010, 0x084,         XCHG,    Ot::Register16Encoded,                  Ot::FixedRegister16(Register16::AX)),
    inst!( 0x94,  0, 0b0100000000110010, 0x084,         XCHG,    Ot::Register16Encoded,                  Ot::FixedRegister16(Register16::AX)),
    inst!( 0x95,  0, 0b0100000000110010, 0x084,         XCHG,    Ot::Register16Encoded,                  Ot::FixedRegister16(Register16::AX)),
    inst!( 0x96,  0, 0b0100000000110010, 0x084,         XCHG,    Ot::Register16Encoded,                  Ot::FixedRegister16(Register16::AX)),
    inst!( 0x97,  0, 0b0100000000110010, 0x084,         XCHG,    Ot::Register16Encoded,                  Ot::FixedRegister16(Register16::AX)),
    inst!( 0x98,  0, 0b0100000000110010, 0x054,         CBW,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x99,  0, 0b0100000000110010, 0x058,         CWD,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x9A,  0, 0b0100000000110010, 0x070,         CALLF,   Ot::FarAddress,                         Ot::NoOperand),
    inst!( 0x9B,  0, 0b0100000000110010, 0x0f8,         WAIT,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x9C,  0, 0b0100000000110010, 0x030,         PUSHF,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x9D,  0, 0b0100000000110010, 0x03c,         POPF,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x9E,  0, 0b0100000000110010, 0x100,         SAHF,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x9F,  0, 0b0100000000110010, 0x104,         LAHF,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xA0,  0, 0b0100100010110010, 0x060,         MOV,     Ot::FixedRegister8(Register8::AL),      Ot::Offset8),
    inst!( 0xA1,  0, 0b0100100010110010, 0x060,         MOV,     Ot::FixedRegister16(Register16::AX),    Ot::Offset16),
    inst!( 0xA2,  0, 0b0100100010110010, 0x064,         MOV,     Ot::Offset8,                            Ot::FixedRegister8(Register8::AL)),
    inst!( 0xA3,  0, 0b0100100010110010, 0x064,         MOV,     Ot::Offset16,                           Ot::FixedRegister16(Register16::AX)),
    inst!( 0xA4,  0, 0b0100100010110010, 0x12c,         MOVSB,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xA5,  0, 0b0100100010110010, 0x12c,         MOVSW,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xA6,  0, 0b0100100010110010, 0x120,         CMPSB,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xA7,  0, 0b0100100010110010, 0x120,         CMPSW,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xA8,  0, 0b0100100010110010, 0x09C,         TEST,    Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0xA9,  0, 0b0100100010110010, 0x09C,         TEST,    Ot::FixedRegister16(Register16::AX),    Ot::Immediate16),
    inst!( 0xAA,  0, 0b0100100010110010, 0x11c,         STOSB,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xAB,  0, 0b0100100010110010, 0x11c,         STOSW,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xAC,  0, 0b0100100010110010, 0x12c,         LODSB,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xAD,  0, 0b0100100010110010, 0x12c,         LODSW,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xAE,  0, 0b0100100010110010, 0x120,         SCASB,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xAF,  0, 0b0100100010110010, 0x120,         SCASW,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xB0,  0, 0b0100000000110010, 0x01c,         MOV,     Ot::Register8Encoded,                   Ot::Immediate8),
    inst!( 0xB1,  0, 0b0100000000110010, 0x01c,         MOV,     Ot::Register8Encoded,                   Ot::Immediate8),
    inst!( 0xB2,  0, 0b0100000000110010, 0x01c,         MOV,     Ot::Register8Encoded,                   Ot::Immediate8),
    inst!( 0xB3,  0, 0b0100000000110010, 0x01c,         MOV,     Ot::Register8Encoded,                   Ot::Immediate8),
    inst!( 0xB4,  0, 0b0100000000110010, 0x01c,         MOV,     Ot::Register8Encoded,                   Ot::Immediate8),
    inst!( 0xB5,  0, 0b0100000000110010, 0x01c,         MOV,     Ot::Register8Encoded,                   Ot::Immediate8),
    inst!( 0xB6,  0, 0b0100000000110010, 0x01c,         MOV,     Ot::Register8Encoded,                   Ot::Immediate8),
    inst!( 0xB7,  0, 0b0100000000110010, 0x01c,         MOV,     Ot::Register8Encoded,                   Ot::Immediate8),
    inst!( 0xB8,  0, 0b0100000000110010, 0x01c,         MOV,     Ot::Register16Encoded,                  Ot::Immediate16),
    inst!( 0xB9,  0, 0b0100000000110010, 0x01c,         MOV,     Ot::Register16Encoded,                  Ot::Immediate16),
    inst!( 0xBA,  0, 0b0100000000110010, 0x01c,         MOV,     Ot::Register16Encoded,                  Ot::Immediate16),
    inst!( 0xBB,  0, 0b0100000000110010, 0x01c,         MOV,     Ot::Register16Encoded,                  Ot::Immediate16),
    inst!( 0xBC,  0, 0b0100000000110010, 0x01c,         MOV,     Ot::Register16Encoded,                  Ot::Immediate16),
    inst!( 0xBD,  0, 0b0100000000110010, 0x01c,         MOV,     Ot::Register16Encoded,                  Ot::Immediate16),
    inst!( 0xBE,  0, 0b0100000000110010, 0x01c,         MOV,     Ot::Register16Encoded,                  Ot::Immediate16),
    inst!( 0xBF,  0, 0b0100000000110010, 0x01c,         MOV,     Ot::Register16Encoded,                  Ot::Immediate16),
    inst!( 0xC0,  0, 0b0100000000110000, 0x0cc,         RETN,    Ot::Immediate16,                        Ot::NoOperand),
    inst!( 0xC1,  0, 0b0100000000110000, 0x0bc,         RETN,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xC2,  0, 0b0100000000110000, 0x0cc,         RETN,    Ot::Immediate16,                        Ot::NoOperand),
    inst!( 0xC3,  0, 0b0100000000110000, 0x0bc,         RETN,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xC4,  0, 0b0100000000100000, 0x0f0,         LES,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0xC5,  0, 0b0100000000100000, 0x0f4,         LDS,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0xC6,  0, 0b0100100000100010, 0x014,         MOV,     Ot::ModRM8,                             Ot::Immediate8),
    inst!( 0xC7,  0, 0b0100100000100010, 0x014,         MOV,     Ot::ModRM16,                            Ot::Immediate16),
    inst!( 0xC8,  0, 0b0100000000110000, 0x0cc,         RETF,    Ot::Immediate16,                        Ot::NoOperand),
    inst!( 0xC9,  0, 0b0100000000110000, 0x0c0,         RETF,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xCA,  0, 0b0100000000110000, 0x0cc,         RETF,    Ot::Immediate16,                        Ot::NoOperand),
    inst!( 0xCB,  0, 0b0100000000110000, 0x0c0,         RETF,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xCC,  0, 0b0100000000110000, 0x1b0,         INT3,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xCD,  0, 0b0100000000110000, 0x1a8,         INT,     Ot::Immediate8,                         Ot::NoOperand),
    inst!( 0xCE,  0, 0b0100000000110000, 0x1ac,         INTO,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xCF,  0, 0b0100000000110000, 0x0c8,         IRET,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xD0,  5, 0b0100100000000000, 0x088, ROL   , Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xD1,  6, 0b0100100000000000, 0x088, SAR   , Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xD2,  7, 0b0100100000000000, 0x08c, ROL   , Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xD3,  8, 0b0100100000000000, 0x08c, SAR   , Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xD4,  0, 0b0101000000110000, 0x174,         AAM,     Ot::Immediate8,                         Ot::NoOperand),
    inst!( 0xD5,  0, 0b0101000000110000, 0x170,         AAD,     Ot::Immediate8,                         Ot::NoOperand),
    inst!( 0xD6,  0, 0b0101000000110000, 0x0a0,         SALC,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xD7,  0, 0b0101000000110000, 0x10c,         XLAT,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xD8,  0, 0b0100000000100000, 0x108,         ESC,     Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xD9,  0, 0b0100000000100000, 0x108,         ESC,     Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xDA,  0, 0b0100000000100000, 0x108,         ESC,     Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xDB,  0, 0b0100000000100000, 0x108,         ESC,     Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xDC,  0, 0b0100000000100000, 0x108,         ESC,     Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xDD,  0, 0b0100000000100000, 0x108,         ESC,     Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xDE,  0, 0b0100000000100000, 0x108,         ESC,     Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xDF,  0, 0b0100000000100000, 0x108,         ESC,     Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xE0,  0, 0b0110000000110000, 0x138,         LOOPNE,  Ot::Relative8,                          Ot::NoOperand),
    inst!( 0xE1,  0, 0b0110000000110000, 0x138,         LOOPE,   Ot::Relative8,                          Ot::NoOperand),
    inst!( 0xE2,  0, 0b0110000000110000, 0x140,         LOOP,    Ot::Relative8,                          Ot::NoOperand),
    inst!( 0xE3,  0, 0b0110000000110000, 0x134,         JCXZ,    Ot::Relative8,                          Ot::NoOperand),
    inst!( 0xE4,  0, 0b0100100010110011, 0x0ac,         IN,      Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0xE5,  0, 0b0100100010110011, 0x0ac,         IN,      Ot::FixedRegister16(Register16::AX),    Ot::Immediate8),
    inst!( 0xE6,  0, 0b0100100010110011, 0x0b0,         OUT,     Ot::Immediate8,                         Ot::FixedRegister8(Register8::AL)),
    inst!( 0xE7,  0, 0b0100100010110011, 0x0b0,         OUT,     Ot::Immediate8,                         Ot::FixedRegister16(Register16::AX)),
    inst!( 0xE8,  0, 0b0110000000110000, 0x07c,         CALL,    Ot::Relative16,                         Ot::NoOperand),
    inst!( 0xE9,  0, 0b0110000000110000, 0x0d0,         JMP,     Ot::Relative16,                         Ot::NoOperand),
    inst!( 0xEA,  0, 0b0110000000110000, 0x0e0,         JMPF,    Ot::FarAddress,                         Ot::NoOperand),
    inst!( 0xEB,  0, 0b0110000000110000, 0x0d0,         JMP,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0xEC,  0, 0b0100100010110011, 0x0b4,         IN,      Ot::FixedRegister8(Register8::AL),      Ot::FixedRegister16(Register16::DX)),
    inst!( 0xED,  0, 0b0100100010110011, 0x0b4,         IN,      Ot::FixedRegister16(Register16::AX),    Ot::FixedRegister16(Register16::DX)),
    inst!( 0xEE,  0, 0b0100100010110011, 0x0b8,         OUT,     Ot::FixedRegister16(Register16::DX),    Ot::FixedRegister8(Register8::AL)),
    inst!( 0xEF,  0, 0b0100100010110011, 0x0b8,         OUT,     Ot::FixedRegister16(Register16::DX),    Ot::FixedRegister16(Register16::AX)),
    inst!( 0xF0,  0, 0b0100010000111010, 0x1FF,         LOCK,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xF1,  0, 0b0100010000111010, 0x1FF,         LOCK,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xF2,  0, 0b0100010000111010, 0x1FF,         Prefix,  Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xF3,  0, 0b0100010000111010, 0x1FF,         Prefix,  Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xF4,  0, 0b0100010000110010, 0x1FF,         HLT,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xF5,  0, 0b0100010000110010, 0x1FF,         CMC,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xF6,  9, 0b0100100000100100, 0x098,         Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xF7, 10, 0b0100100000100100, 0x160,         Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xF8,  0, 0b0100010001110010, 0x1FF,         CLC,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xF9,  0, 0b0100010001110010, 0x1FF,         STC,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xFA,  0, 0b0100010001110010, 0x1FF,         CLI,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xFB,  0, 0b0100010001110010, 0x1FF,         STI,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xFC,  0, 0b0100010001110010, 0x1FF,         CLD,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xFD,  0, 0b0100010001110010, 0x1FF,         STD,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xFE, 11, 0b0000100000100100, 0x020,         Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xFF, 12, 0b0000100000100100, 0x026,         Group,   Ot::NoOperand,                          Ot::NoOperand),
    // Group
    inst!( 0x80,  1, 0b0110100000000000, 0x00c, ADD   , ADD  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0x80,  1, 0b0110100000000000, 0x00c, OR    , OR   ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0x80,  1, 0b0110100000000000, 0x00c, ADC   , ADC  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0x80,  1, 0b0110100000000000, 0x00c, SBB   , SBB  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0x80,  1, 0b0110100000000000, 0x00c, AND   , AND  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0x80,  1, 0b0110100000000000, 0x00c, SUB   , SUB  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0x80,  1, 0b0110100000000000, 0x00c, XOR   , XOR  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0x80,  1, 0b0110100000000000, 0x00c, CMP   , CMP  ,   Ot::ModRM8,                             Ot::NoOperand),
    // Group
    inst!( 0x81,  1, 0b0110100000000000, 0x00c, ADD   , ADD  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0x81,  1, 0b0110100000000000, 0x00c, OR    , OR   ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0x81,  1, 0b0110100000000000, 0x00c, ADC   , ADC  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0x81,  1, 0b0110100000000000, 0x00c, SBB   , SBB  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0x81,  1, 0b0110100000000000, 0x00c, AND   , AND  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0x81,  1, 0b0110100000000000, 0x00c, SUB   , SUB  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0x81,  1, 0b0110100000000000, 0x00c, XOR   , XOR  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0x81,  1, 0b0110100000000000, 0x00c, CMP   , CMP  ,   Ot::ModRM16,                            Ot::NoOperand),
    // Group
    inst!( 0x82,  1, 0b0110100000000000, 0x00c, ADD   , ADD  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0x82,  1, 0b0110100000000000, 0x00c, OR    , OR   ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0x82,  1, 0b0110100000000000, 0x00c, ADC   , ADC  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0x82,  1, 0b0110100000000000, 0x00c, SBB   , SBB  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0x82,  1, 0b0110100000000000, 0x00c, AND   , AND  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0x82,  1, 0b0110100000000000, 0x00c, SUB   , SUB  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0x82,  1, 0b0110100000000000, 0x00c, XOR   , XOR  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0x82,  1, 0b0110100000000000, 0x00c, CMP   , CMP  ,   Ot::ModRM8,                             Ot::NoOperand),
    // Group
    inst!( 0x83,  1, 0b0110100000000000, 0x00c, ADD   , ADD  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0x83,  1, 0b0110100000000000, 0x00c, OR    , OR   ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0x83,  1, 0b0110100000000000, 0x00c, ADC   , ADC  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0x83,  1, 0b0110100000000000, 0x00c, SBB   , SBB  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0x83,  1, 0b0110100000000000, 0x00c, AND   , AND  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0x83,  1, 0b0110100000000000, 0x00c, SUB   , SUB  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0x83,  1, 0b0110100000000000, 0x00c, XOR   , XOR  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0x83,  1, 0b0110100000000000, 0x00c, CMP   , CMP  ,   Ot::ModRM16,                            Ot::NoOperand),
    // Group
    inst!( 0xD0,  2, 0b0100100000000000, 0x088, ROL   , ROL  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xD0,  2, 0b0100100000000000, 0x088, ROR   , ROR  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xD0,  2, 0b0100100000000000, 0x088, RCL   , RCL  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xD0,  2, 0b0100100000000000, 0x088, RCR   , RCR  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xD0,  2, 0b0100100000000000, 0x088, SHL   , SHL  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xD0,  2, 0b0100100000000000, 0x088, SHR   , SHR  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xD0,  2, 0b0100100000000000, 0x088, SETMO , SETMO,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xD0,  2, 0b0100100000000000, 0x088, SAR   , SAR  ,   Ot::ModRM8,                             Ot::NoOperand),
    // Group
    inst!( 0xD1,  2, 0b0100100000000000, 0x088, ROL   , ROL  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xD1,  2, 0b0100100000000000, 0x088, ROR   , ROR  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xD1,  2, 0b0100100000000000, 0x088, RCL   , RCL  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xD1,  2, 0b0100100000000000, 0x088, RCR   , RCR  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xD1,  2, 0b0100100000000000, 0x088, SHL   , SHL  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xD1,  2, 0b0100100000000000, 0x088, SHR   , SHR  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xD1,  2, 0b0100100000000000, 0x088, SETMO , SETMO,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xD1,  2, 0b0100100000000000, 0x088, SAR   , SAR  ,   Ot::ModRM16,                            Ot::NoOperand),
    // Group
    inst!( 0xD2,  3, 0b0100100000000000, 0x08c, ROL   , ROL   ,  Ot::ModRM8,                             Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD2,  3, 0b0100100000000000, 0x08c, ROR   , ROR   ,  Ot::ModRM8,                             Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD2,  3, 0b0100100000000000, 0x08c, RCL   , RCL   ,  Ot::ModRM8,                             Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD2,  3, 0b0100100000000000, 0x08c, RCR   , RCR   ,  Ot::ModRM8,                             Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD2,  3, 0b0100100000000000, 0x08c, SHL   , SHL   ,  Ot::ModRM8,                             Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD2,  3, 0b0100100000000000, 0x08c, SHR   , SHR   ,  Ot::ModRM8,                             Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD2,  3, 0b0100100000000000, 0x08c, SETMO , SETMOC,  Ot::ModRM8,                             Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD2,  3, 0b0100100000000000, 0x08c, SAR   , SAR   ,  Ot::ModRM8,                             Ot::FixedRegister8(Register8::CL)),
    // Group
    inst!( 0xD3,  3, 0b0100100000000000, 0x08c, ROL   , ROL   ,  Ot::ModRM16,                            Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD3,  3, 0b0100100000000000, 0x08c, ROR   , ROR   ,  Ot::ModRM16,                            Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD3,  3, 0b0100100000000000, 0x08c, RCL   , RCL   ,  Ot::ModRM16,                            Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD3,  3, 0b0100100000000000, 0x08c, RCR   , RCR   ,  Ot::ModRM16,                            Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD3,  3, 0b0100100000000000, 0x08c, SHL   , SHL   ,  Ot::ModRM16,                            Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD3,  3, 0b0100100000000000, 0x08c, SHR   , SHR   ,  Ot::ModRM16,                            Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD3,  3, 0b0100100000000000, 0x08c, SETMO , SETMOC,  Ot::ModRM16,                            Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD3,  3, 0b0100100000000000, 0x08c, SAR   , SAR   ,  Ot::ModRM16,                            Ot::FixedRegister8(Register8::CL)),
    // Group
    inst!( 0xF6,  4, 0b0100100000100100, 0x098,         TEST  ,  Ot::ModRM8,                             Ot::Immediate16),
    inst!( 0xF6,  4, 0b0100100000100100, 0x098,         TEST  ,  Ot::ModRM8,                             Ot::Immediate16),
    inst!( 0xF6,  4, 0b0100100000100100, 0x098, NOT   , NOT   ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xF6,  4, 0b0100100000100100, 0x098, NEG   , NEG   ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xF6,  4, 0b0100100000100100, 0x098,         MUL   ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xF6,  4, 0b0100100000100100, 0x098,         IMUL  ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xF6,  4, 0b0100100000100100, 0x098,         DIV   ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xF6,  4, 0b0100100000100100, 0x098,         IDIV  ,  Ot::ModRM8,                             Ot::NoOperand),
    // Group
    inst!( 0xF7,  4, 0b0100100000100100, 0x160,         TEST  ,  Ot::ModRM16,                            Ot::Immediate16),
    inst!( 0xF7,  4, 0b0100100000100100, 0x160,         TEST  ,  Ot::ModRM16,                            Ot::Immediate16),
    inst!( 0xF7,  4, 0b0100100000100100, 0x160, NOT   , NOT   ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xF7,  4, 0b0100100000100100, 0x160, NEG   , NEG   ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xF7,  4, 0b0100100000100100, 0x160,         MUL   ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xF7,  4, 0b0100100000100100, 0x160,         IMUL  ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xF7,  4, 0b0100100000100100, 0x160,         DIV   ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xF7,  4, 0b0100100000100100, 0x160,         IDIV  ,  Ot::ModRM16,                            Ot::NoOperand),
    // Group
    inst!( 0xFE,  5, 0b0000100000100100, 0x020, INC   , INC   ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xFE,  5, 0b0000100000100100, 0x020, DEC   , DEC   ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xFE,  5, 0b0000100000100100, 0x020,         CALL  ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xFE,  5, 0b0000100000100100, 0x020,         CALLF ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xFE,  5, 0b0000100000100100, 0x020,         JMP   ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xFE,  5, 0b0000100000100100, 0x020,         JMPF  ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xFE,  5, 0b0000100000100100, 0x020,         PUSH  ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xFE,  5, 0b0000100000100100, 0x020,         PUSH  ,  Ot::ModRM8,                             Ot::NoOperand),
    // Group
    inst!( 0xFF,  5, 0b0000100000100100, 0x026, INC   , INC   ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xFF,  5, 0b0000100000100100, 0x026, DEC   , DEC   ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xFF,  5, 0b0000100000100100, 0x026,         CALL  ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xFF,  5, 0b0000100000100100, 0x026,         CALLF ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xFF,  5, 0b0000100000100100, 0x026,         JMP   ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xFF,  5, 0b0000100000100100, 0x026,         JMPF  ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xFF,  5, 0b0000100000100100, 0x026,         PUSH  ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xFF,  5, 0b0000100000100100, 0x026,         PUSH  ,  Ot::ModRM16,                            Ot::NoOperand),
];

impl Cpu {
    #[rustfmt::skip]
    pub fn decode(bytes: &mut impl ByteQueue) -> Result<Instruction, Box<dyn std::error::Error>> {

        let mut operand1_type: OperandType = OperandType::NoOperand;
        let mut operand2_type: OperandType = OperandType::NoOperand;
        let mut operand1_size: OperandSize = OperandSize::NoOperand;
        let mut operand2_size: OperandSize = OperandSize::NoOperand;

        let mut opcode = bytes.q_read_u8(QueueType::First, QueueReader::Biu);
        let mut size: u32 = 1;

        let mut mnemonic;

        let mut operand1_template;
        let mut operand2_template;

        let mut op_flags: u32;
        let mut op_prefixes: u32 = 0;
        let mut op_segment_override = None;
        let mut loaded_modrm = false;

        // Read in opcode prefixes until exhausted
        loop {
            // Set flags for all prefixes encountered...
            op_prefixes |= match opcode {
                0x26 => OPCODE_PREFIX_ES_OVERRIDE,
                0x2E => OPCODE_PREFIX_CS_OVERRIDE,
                0x36 => OPCODE_PREFIX_SS_OVERRIDE,
                0x3E => OPCODE_PREFIX_DS_OVERRIDE,
                0xF0 => OPCODE_PREFIX_LOCK,
                0xF1 => OPCODE_PREFIX_LOCK,
                0xF2 => OPCODE_PREFIX_REP1,
                0xF3 => OPCODE_PREFIX_REP2,
                _=> {
                    break;
                }
            };
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

        let op_lu = &DECODE[opcode as usize];
        
        // Match templatizeable instructions
        (mnemonic, operand1_template, operand2_template, op_flags) = match opcode {
            0x00 => (Mnemonic::ADD,  OperandTemplate::ModRM8,   OperandTemplate::Register8,     I_LOAD_EA ),
            0x01 => (Mnemonic::ADD,  OperandTemplate::ModRM16,   OperandTemplate::Register16,   I_LOAD_EA ),
            0x02 => (Mnemonic::ADD,  OperandTemplate::Register8,   OperandTemplate::ModRM8,     I_LOAD_EA ),
            0x03 => (Mnemonic::ADD,  OperandTemplate::Register16,   OperandTemplate::ModRM16,   I_LOAD_EA ),
            0x04 => (Mnemonic::ADD,  OperandTemplate::FixedRegister8(Register8::AL),   OperandTemplate::Immediate8,    0),
            0x05 => (Mnemonic::ADD,  OperandTemplate::FixedRegister16(Register16::AX),   OperandTemplate::Immediate16, 0),
            0x06 => (Mnemonic::PUSH, OperandTemplate::FixedRegister16(Register16::ES),   OperandTemplate::NoOperand,   0),
            0x07 => (Mnemonic::POP,  OperandTemplate::FixedRegister16(Register16::ES),   OperandTemplate::NoOperand,   0),
            0x08 => (Mnemonic::OR,   OperandTemplate::ModRM8,    OperandTemplate::Register8,    I_LOAD_EA ),
            0x09 => (Mnemonic::OR,   OperandTemplate::ModRM16,    OperandTemplate::Register16,  I_LOAD_EA ),
            0x0A => (Mnemonic::OR,   OperandTemplate::Register8,    OperandTemplate::ModRM8,    I_LOAD_EA ),
            0x0B => (Mnemonic::OR,   OperandTemplate::Register16,    OperandTemplate::ModRM16,  I_LOAD_EA ),
            0x0C => (Mnemonic::OR,   OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
            0x0D => (Mnemonic::OR,   OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0),
            0x0E => (Mnemonic::PUSH, OperandTemplate::FixedRegister16(Register16::CS),   OperandTemplate::NoOperand,   0),
            0x0F => (Mnemonic::POP,  OperandTemplate::FixedRegister16(Register16::CS),   OperandTemplate::NoOperand,   0),
            0x10 => (Mnemonic::ADC,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    I_LOAD_EA ),
            0x11 => (Mnemonic::ADC,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  I_LOAD_EA ),
            0x12 => (Mnemonic::ADC,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    I_LOAD_EA ),
            0x13 => (Mnemonic::ADC,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  I_LOAD_EA ),
            0x14 => (Mnemonic::ADC,  OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
            0x15 => (Mnemonic::ADC,  OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0),
            0x16 => (Mnemonic::PUSH, OperandTemplate::FixedRegister16(Register16::SS),   OperandTemplate::NoOperand,   0),
            0x17 => (Mnemonic::POP,  OperandTemplate::FixedRegister16(Register16::SS),   OperandTemplate::NoOperand,   0),
            0x18 => (Mnemonic::SBB,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    I_LOAD_EA ),
            0x19 => (Mnemonic::SBB,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  I_LOAD_EA ),
            0x1A => (Mnemonic::SBB,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    I_LOAD_EA ),
            0x1B => (Mnemonic::SBB,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  I_LOAD_EA ),
            0x1C => (Mnemonic::SBB,  OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
            0x1D => (Mnemonic::SBB,  OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0),
            0x1E => (Mnemonic::PUSH, OperandTemplate::FixedRegister16(Register16::DS),   OperandTemplate::NoOperand,   0),
            0x1F => (Mnemonic::POP,  OperandTemplate::FixedRegister16(Register16::DS),   OperandTemplate::NoOperand,   0),
            0x20 => (Mnemonic::AND,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    I_LOAD_EA ),
            0x21 => (Mnemonic::AND,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  I_LOAD_EA ),
            0x22 => (Mnemonic::AND,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    I_LOAD_EA ),
            0x23 => (Mnemonic::AND,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  I_LOAD_EA ),
            0x24 => (Mnemonic::AND,  OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
            0x25 => (Mnemonic::AND,  OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0),
            0x27 => (Mnemonic::DAA,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand, 0),
            0x28 => (Mnemonic::SUB,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    I_LOAD_EA ),
            0x29 => (Mnemonic::SUB,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  I_LOAD_EA ),
            0x2A => (Mnemonic::SUB,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    I_LOAD_EA ),
            0x2B => (Mnemonic::SUB,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  I_LOAD_EA ),
            0x2C => (Mnemonic::SUB,  OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
            0x2D => (Mnemonic::SUB,  OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0),
            0x2F => (Mnemonic::DAS,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,  0),
            0x30 => (Mnemonic::XOR,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    I_LOAD_EA ),
            0x31 => (Mnemonic::XOR,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  I_LOAD_EA ),
            0x32 => (Mnemonic::XOR,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    I_LOAD_EA ),
            0x33 => (Mnemonic::XOR,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  I_LOAD_EA ),
            0x34 => (Mnemonic::XOR,  OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
            0x35 => (Mnemonic::XOR,  OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0),
        //  0x36 Segment override prefix
            0x37 => (Mnemonic::AAA,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,  0),
            0x38 => (Mnemonic::CMP,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    I_LOAD_EA ),
            0x39 => (Mnemonic::CMP,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  I_LOAD_EA ),
            0x3A => (Mnemonic::CMP,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    I_LOAD_EA ),
            0x3B => (Mnemonic::CMP,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  I_LOAD_EA ),
            0x3C => (Mnemonic::CMP,  OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
            0x3D => (Mnemonic::CMP,  OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0),
            0x3F => (Mnemonic::AAS,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,  0),
            0x40..=0x47 => (Mnemonic::INC,  OperandTemplate::Register16Encoded,    OperandTemplate::NoOperand, 0),
            0x48..=0x4F => (Mnemonic::DEC,  OperandTemplate::Register16Encoded,    OperandTemplate::NoOperand, 0),
            0x50..=0x57 => (Mnemonic::PUSH, OperandTemplate::Register16Encoded,    OperandTemplate::NoOperand, 0),
            0x58..=0x5F => (Mnemonic::POP,  OperandTemplate::Register16Encoded,    OperandTemplate::NoOperand, 0),
        //  0x60..=0x6F >= on 8088, these instructions map to 0x70-7F
            0x60 => (Mnemonic::JO,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x61 => (Mnemonic::JNO,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x62 => (Mnemonic::JB,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x63 => (Mnemonic::JNB,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x64 => (Mnemonic::JZ,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x65 => (Mnemonic::JNZ,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x66 => (Mnemonic::JBE,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x67 => (Mnemonic::JNBE, OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x68 => (Mnemonic::JS,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x69 => (Mnemonic::JNS,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x6A => (Mnemonic::JP,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x6B => (Mnemonic::JNP,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x6C => (Mnemonic::JL,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x6D => (Mnemonic::JNL,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x6E => (Mnemonic::JLE,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x6F => (Mnemonic::JNLE, OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x70 => (Mnemonic::JO,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x71 => (Mnemonic::JNO,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x72 => (Mnemonic::JB,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x73 => (Mnemonic::JNB,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x74 => (Mnemonic::JZ,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x75 => (Mnemonic::JNZ,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x76 => (Mnemonic::JBE,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x77 => (Mnemonic::JNBE, OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x78 => (Mnemonic::JS,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x79 => (Mnemonic::JNS,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x7A => (Mnemonic::JP,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x7B => (Mnemonic::JNP,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x7C => (Mnemonic::JL,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x7D => (Mnemonic::JNL,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x7E => (Mnemonic::JLE,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),
            0x7F => (Mnemonic::JNLE, OperandTemplate::Relative8,    OperandTemplate::NoOperand,  I_REL_JUMP),

            0x84 => (Mnemonic::TEST,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    I_LOAD_EA),
            0x85 => (Mnemonic::TEST,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  I_LOAD_EA),
            0x86 => (Mnemonic::XCHG,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    I_LOAD_EA),
            0x87 => (Mnemonic::XCHG,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  I_LOAD_EA),
            0x88 => (Mnemonic::MOV,   OperandTemplate::ModRM8,    OperandTemplate::Register8,    0),
            0x89 => (Mnemonic::MOV,   OperandTemplate::ModRM16,    OperandTemplate::Register16,  0),
            0x8A => (Mnemonic::MOV,   OperandTemplate::Register8,    OperandTemplate::ModRM8,    I_LOAD_EA),
            0x8B => (Mnemonic::MOV,   OperandTemplate::Register16,    OperandTemplate::ModRM16,  I_LOAD_EA),
            0x8C => (Mnemonic::MOV,   OperandTemplate::ModRM16,    OperandTemplate::SegmentRegister,  0),
            0x8D => (Mnemonic::LEA,   OperandTemplate::Register16,   OperandTemplate::ModRM16,   0),
            0x8E => (Mnemonic::MOV,   OperandTemplate::SegmentRegister,    OperandTemplate::ModRM16,  I_LOAD_EA),
            0x8F => (Mnemonic::POP,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,    0),
            0x90 => (Mnemonic::NOP,   OperandTemplate::NoOperand,   OperandTemplate::NoOperand,  0),
            0x91..=0x97 => (Mnemonic::XCHG,  OperandTemplate::Register16Encoded,   OperandTemplate::FixedRegister16(Register16::AX),  0),
            0x98 => (Mnemonic::CBW,   OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
            0x99 => (Mnemonic::CWD,   OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
            0x9A => (Mnemonic::CALLF, OperandTemplate::FarAddress,   OperandTemplate::NoOperand,  0),
            0x9B => (Mnemonic::WAIT,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
            0x9C => (Mnemonic::PUSHF, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
            0x9D => (Mnemonic::POPF,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
            0x9E => (Mnemonic::SAHF,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
            0x9F => (Mnemonic::LAHF,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
            0xA0 => (Mnemonic::MOV,   OperandTemplate::FixedRegister8(Register8::AL),   OperandTemplate::Offset8,      0),
            0xA1 => (Mnemonic::MOV,   OperandTemplate::FixedRegister16(Register16::AX),   OperandTemplate::Offset16,   0),
            0xA2 => (Mnemonic::MOV,   OperandTemplate::Offset8,   OperandTemplate::FixedRegister8(Register8::AL),      0),
            0xA3 => (Mnemonic::MOV,   OperandTemplate::Offset16,   OperandTemplate::FixedRegister16(Register16::AX),   0),
            0xA4 => (Mnemonic::MOVSB, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
            0xA5 => (Mnemonic::MOVSW, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
            0xA6 => (Mnemonic::CMPSB, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
            0xA7 => (Mnemonic::CMPSW, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
            0xA8 => (Mnemonic::TEST,  OperandTemplate::FixedRegister8(Register8::AL),   OperandTemplate::Immediate8,    0),
            0xA9 => (Mnemonic::TEST,  OperandTemplate::FixedRegister16(Register16::AX),   OperandTemplate::Immediate16, 0),
            0xAA => (Mnemonic::STOSB, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
            0xAB => (Mnemonic::STOSW, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
            0xAC => (Mnemonic::LODSB, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
            0xAD => (Mnemonic::LODSW, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
            0xAE => (Mnemonic::SCASB, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
            0xAF => (Mnemonic::SCASW, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
            0xB0..=0xB7 => (Mnemonic::MOV,  OperandTemplate::Register8Encoded,   OperandTemplate::Immediate8,   0),
            0xB8..=0xBF => (Mnemonic::MOV,  OperandTemplate::Register16Encoded,   OperandTemplate::Immediate16, 0),
            0xC0 => (Mnemonic::RETN, OperandTemplate::Immediate16,   OperandTemplate::NoOperand,  0),
            0xC1 => (Mnemonic::RETN, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
            0xC2 => (Mnemonic::RETN, OperandTemplate::Immediate16,   OperandTemplate::NoOperand,  0),
            0xC3 => (Mnemonic::RETN, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
            0xC4 => (Mnemonic::LES,  OperandTemplate::Register16,   OperandTemplate::ModRM16,     I_LOAD_EA),
            0xC5 => (Mnemonic::LDS,  OperandTemplate::Register16,   OperandTemplate::ModRM16,     I_LOAD_EA),
            0xC6 => (Mnemonic::MOV,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,      0),
            0xC7 => (Mnemonic::MOV,  OperandTemplate::ModRM16,    OperandTemplate::Immediate16,   0),
            0xC8 => (Mnemonic::RETF, OperandTemplate::Immediate16,   OperandTemplate::NoOperand,   0),
            0xC9 => (Mnemonic::RETF, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,     0),
            0xCA => (Mnemonic::RETF, OperandTemplate::Immediate16,   OperandTemplate::NoOperand,   0),
            0xCB => (Mnemonic::RETF, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,     0),
            0xCC => (Mnemonic::INT3, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,     0),
            0xCD => (Mnemonic::INT,  OperandTemplate::Immediate8,    OperandTemplate::NoOperand,   0),
            0xCE => (Mnemonic::INTO, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,     0),
            0xCF => (Mnemonic::IRET, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,     0),

            0xD4 => (Mnemonic::AAM,  OperandTemplate::Immediate8,   OperandTemplate::NoOperand,    0),
            0xD5 => (Mnemonic::AAD,  OperandTemplate::Immediate8,   OperandTemplate::NoOperand,    0),
            0xD6 => (Mnemonic::SALC, OperandTemplate::NoOperand,  OperandTemplate::NoOperand,      0),
            0xD7 => (Mnemonic::XLAT, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,     0),
            // FPU instructions
            0xD8..=0xDF => (Mnemonic::ESC, OperandTemplate::ModRM16, OperandTemplate::NoOperand,   I_LOAD_EA),

            0xE0 => (Mnemonic::LOOPNE, OperandTemplate::Relative8,   OperandTemplate::NoOperand,   I_REL_JUMP),
            0xE1 => (Mnemonic::LOOPE,  OperandTemplate::Relative8,   OperandTemplate::NoOperand,   I_REL_JUMP),
            0xE2 => (Mnemonic::LOOP, OperandTemplate::Relative8,   OperandTemplate::NoOperand,     I_REL_JUMP),
            0xE3 => (Mnemonic::JCXZ, OperandTemplate::Relative8,   OperandTemplate::NoOperand,     I_REL_JUMP),
            0xE4 => (Mnemonic::IN,   OperandTemplate::FixedRegister8(Register8::AL),   OperandTemplate::Immediate8,    0),
            0xE5 => (Mnemonic::IN,   OperandTemplate::FixedRegister16(Register16::AX),   OperandTemplate::Immediate8,   0),
            0xE6 => (Mnemonic::OUT,  OperandTemplate::Immediate8,   OperandTemplate::FixedRegister8(Register8::AL),  0),
            0xE7 => (Mnemonic::OUT,  OperandTemplate::Immediate8,   OperandTemplate::FixedRegister16(Register16::AX), 0),
            0xE8 => (Mnemonic::CALL, OperandTemplate::Relative16,   OperandTemplate::NoOperand,    I_REL_JUMP),
            0xE9 => (Mnemonic::JMP,  OperandTemplate::Relative16,   OperandTemplate::NoOperand,    I_REL_JUMP),
            0xEA => (Mnemonic::JMPF, OperandTemplate::FarAddress,  OperandTemplate::NoOperand,    0),
            0xEB => (Mnemonic::JMP,  OperandTemplate::Relative8,   OperandTemplate::NoOperand,     I_REL_JUMP),
            0xEC => (Mnemonic::IN,   OperandTemplate::FixedRegister8(Register8::AL),   OperandTemplate::FixedRegister16(Register16::DX),     0),
            0xED => (Mnemonic::IN,   OperandTemplate::FixedRegister16(Register16::AX),   OperandTemplate::FixedRegister16(Register16::DX),   0),
            0xEE => (Mnemonic::OUT,  OperandTemplate::FixedRegister16(Register16::DX),   OperandTemplate::FixedRegister8(Register8::AL),     0),
            0xEF => (Mnemonic::OUT,  OperandTemplate::FixedRegister16(Register16::DX),   OperandTemplate::FixedRegister16(Register16::AX),   0),

            0xF1 => (Mnemonic::NOP,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
            0xF4 => (Mnemonic::HLT,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
            0xF5 => (Mnemonic::CMC,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
            0xF8 => (Mnemonic::CLC,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
            0xF9 => (Mnemonic::STC,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
            0xFA => (Mnemonic::CLI,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
            0xFB => (Mnemonic::STI,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
            0xFC => (Mnemonic::CLD,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
            0xFD => (Mnemonic::STD,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
            // No match to templatizable instruction, handle in next match statement
            _=> (Mnemonic::NoOpcode, OperandTemplate::NoTemplate, OperandTemplate::NoTemplate,  0)
        };

        let mut modrm = Default::default();

        // If we haven't had a match yet, we are in a group instruction
        if mnemonic == Mnemonic::NoOpcode {
            assert_ne!(op_lu.grp, 0, "Group instruction with no group number");
            
            // All group instructions have a modrm w/ op extension. Load the modrm now.
            let modrm_len;
            (modrm, modrm_len) = ModRmByte::read(bytes);
            size += modrm_len;

            loaded_modrm = true;
            let op_ext = modrm.get_op_extension();

            // FX group opcodes seem to have a one-cycle delay. TODO: Why not all groups?

            (mnemonic, operand1_template, operand2_template, op_flags) = match (opcode, op_ext) {
                (0x80 | 0x82, 0x00) => (Mnemonic::ADD,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,    I_LOAD_EA ),
                (0x80 | 0x82, 0x01) => (Mnemonic::OR,   OperandTemplate::ModRM8,   OperandTemplate::Immediate8,    I_LOAD_EA ),
                (0x80 | 0x82, 0x02) => (Mnemonic::ADC,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,    I_LOAD_EA ),
                (0x80 | 0x82, 0x03) => (Mnemonic::SBB,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,    I_LOAD_EA ),
                (0x80 | 0x82, 0x04) => (Mnemonic::AND,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,    I_LOAD_EA ),
                (0x80 | 0x82, 0x05) => (Mnemonic::SUB,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,    I_LOAD_EA ),
                (0x80 | 0x82, 0x06) => (Mnemonic::XOR,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,    I_LOAD_EA ),
                (0x80 | 0x82, 0x07) => (Mnemonic::CMP,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,    I_LOAD_EA ),

                (0x81, 0x00) => (Mnemonic::ADD,   OperandTemplate::ModRM16,   OperandTemplate::Immediate16,    I_LOAD_EA ),
                (0x81, 0x01) => (Mnemonic::OR,    OperandTemplate::ModRM16,   OperandTemplate::Immediate16,    I_LOAD_EA ),
                (0x81, 0x02) => (Mnemonic::ADC,   OperandTemplate::ModRM16,   OperandTemplate::Immediate16,    I_LOAD_EA ),
                (0x81, 0x03) => (Mnemonic::SBB,   OperandTemplate::ModRM16,   OperandTemplate::Immediate16,    I_LOAD_EA ),
                (0x81, 0x04) => (Mnemonic::AND,   OperandTemplate::ModRM16,   OperandTemplate::Immediate16,    I_LOAD_EA ),
                (0x81, 0x05) => (Mnemonic::SUB,   OperandTemplate::ModRM16,   OperandTemplate::Immediate16,    I_LOAD_EA ),
                (0x81, 0x06) => (Mnemonic::XOR,   OperandTemplate::ModRM16,   OperandTemplate::Immediate16,    I_LOAD_EA ),
                (0x81, 0x07) => (Mnemonic::CMP,   OperandTemplate::ModRM16,   OperandTemplate::Immediate16,    I_LOAD_EA ),

                (0x83, 0x00) => (Mnemonic::ADD,   OperandTemplate::ModRM16,   OperandTemplate::Immediate8SignExtended,    I_LOAD_EA ),
                (0x83, 0x01) => (Mnemonic::OR,    OperandTemplate::ModRM16,   OperandTemplate::Immediate8SignExtended,    I_LOAD_EA ),
                (0x83, 0x02) => (Mnemonic::ADC,   OperandTemplate::ModRM16,   OperandTemplate::Immediate8SignExtended,    I_LOAD_EA ),
                (0x83, 0x03) => (Mnemonic::SBB,   OperandTemplate::ModRM16,   OperandTemplate::Immediate8SignExtended,    I_LOAD_EA ),
                (0x83, 0x04) => (Mnemonic::AND,   OperandTemplate::ModRM16,   OperandTemplate::Immediate8SignExtended,    I_LOAD_EA ),
                (0x83, 0x05) => (Mnemonic::SUB,   OperandTemplate::ModRM16,   OperandTemplate::Immediate8SignExtended,    I_LOAD_EA ),
                (0x83, 0x06) => (Mnemonic::XOR,   OperandTemplate::ModRM16,   OperandTemplate::Immediate8SignExtended,    I_LOAD_EA ),
                (0x83, 0x07) => (Mnemonic::CMP,   OperandTemplate::ModRM16,   OperandTemplate::Immediate8SignExtended,    I_LOAD_EA ),

                (0xD0, 0x00) => (Mnemonic::ROL,   OperandTemplate::ModRM8,    OperandTemplate::NoOperand,    I_LOAD_EA ),
                (0xD0, 0x01) => (Mnemonic::ROR,   OperandTemplate::ModRM8,    OperandTemplate::NoOperand,    I_LOAD_EA ),
                (0xD0, 0x02) => (Mnemonic::RCL,   OperandTemplate::ModRM8,    OperandTemplate::NoOperand,    I_LOAD_EA ),
                (0xD0, 0x03) => (Mnemonic::RCR,   OperandTemplate::ModRM8,    OperandTemplate::NoOperand,    I_LOAD_EA ),
                (0xD0, 0x04) => (Mnemonic::SHL,   OperandTemplate::ModRM8,    OperandTemplate::NoOperand,    I_LOAD_EA ),
                (0xD0, 0x05) => (Mnemonic::SHR,   OperandTemplate::ModRM8,    OperandTemplate::NoOperand,    I_LOAD_EA ),
                (0xD0, 0x06) => (Mnemonic::SETMO, OperandTemplate::ModRM8,    OperandTemplate::NoOperand,    I_LOAD_EA ),
                (0xD0, 0x07) => (Mnemonic::SAR,   OperandTemplate::ModRM8,    OperandTemplate::NoOperand,    I_LOAD_EA ),

                (0xD1, 0x00) => (Mnemonic::ROL,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,    I_LOAD_EA ),
                (0xD1, 0x01) => (Mnemonic::ROR,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,    I_LOAD_EA ),
                (0xD1, 0x02) => (Mnemonic::RCL,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,    I_LOAD_EA ),
                (0xD1, 0x03) => (Mnemonic::RCR,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,    I_LOAD_EA ),
                (0xD1, 0x04) => (Mnemonic::SHL,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,    I_LOAD_EA ),
                (0xD1, 0x05) => (Mnemonic::SHR,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,    I_LOAD_EA ),
                (0xD1, 0x06) => (Mnemonic::SETMO, OperandTemplate::ModRM16,   OperandTemplate::NoOperand,    I_LOAD_EA ),
                (0xD1, 0x07) => (Mnemonic::SAR,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,    I_LOAD_EA ),

                (0xD2, 0x00) => (Mnemonic::ROL,   OperandTemplate::ModRM8,    OperandTemplate::FixedRegister8(Register8::CL),    I_LOAD_EA ),
                (0xD2, 0x01) => (Mnemonic::ROR,   OperandTemplate::ModRM8,    OperandTemplate::FixedRegister8(Register8::CL),    I_LOAD_EA ),
                (0xD2, 0x02) => (Mnemonic::RCL,   OperandTemplate::ModRM8,    OperandTemplate::FixedRegister8(Register8::CL),    I_LOAD_EA ),
                (0xD2, 0x03) => (Mnemonic::RCR,   OperandTemplate::ModRM8,    OperandTemplate::FixedRegister8(Register8::CL),    I_LOAD_EA ),
                (0xD2, 0x04) => (Mnemonic::SHL,   OperandTemplate::ModRM8,    OperandTemplate::FixedRegister8(Register8::CL),    I_LOAD_EA ),
                (0xD2, 0x05) => (Mnemonic::SHR,   OperandTemplate::ModRM8,    OperandTemplate::FixedRegister8(Register8::CL),    I_LOAD_EA ),
                (0xD2, 0x06) => (Mnemonic::SETMOC,OperandTemplate::ModRM8,    OperandTemplate::FixedRegister8(Register8::CL),    I_LOAD_EA ),
                (0xD2, 0x07) => (Mnemonic::SAR,   OperandTemplate::ModRM8,    OperandTemplate::FixedRegister8(Register8::CL),    I_LOAD_EA ),

                (0xD3, 0x00) => (Mnemonic::ROL,   OperandTemplate::ModRM16,   OperandTemplate::FixedRegister8(Register8::CL),    I_LOAD_EA ),
                (0xD3, 0x01) => (Mnemonic::ROR,   OperandTemplate::ModRM16,   OperandTemplate::FixedRegister8(Register8::CL),    I_LOAD_EA ),
                (0xD3, 0x02) => (Mnemonic::RCL,   OperandTemplate::ModRM16,   OperandTemplate::FixedRegister8(Register8::CL),    I_LOAD_EA ),
                (0xD3, 0x03) => (Mnemonic::RCR,   OperandTemplate::ModRM16,   OperandTemplate::FixedRegister8(Register8::CL),    I_LOAD_EA ),
                (0xD3, 0x04) => (Mnemonic::SHL,   OperandTemplate::ModRM16,   OperandTemplate::FixedRegister8(Register8::CL),    I_LOAD_EA ),
                (0xD3, 0x05) => (Mnemonic::SHR,   OperandTemplate::ModRM16,   OperandTemplate::FixedRegister8(Register8::CL),    I_LOAD_EA ),
                (0xD3, 0x06) => (Mnemonic::SETMOC,OperandTemplate::ModRM16,   OperandTemplate::FixedRegister8(Register8::CL),    I_LOAD_EA ),
                (0xD3, 0x07) => (Mnemonic::SAR,   OperandTemplate::ModRM16,   OperandTemplate::FixedRegister8(Register8::CL),    I_LOAD_EA ),

                (0xF6, 0x01) => (Mnemonic::TEST,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,     I_LOAD_EA ),
                (0xF6, 0x00) => (Mnemonic::TEST,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,     I_LOAD_EA ),
                (0xF6, 0x02) => (Mnemonic::NOT,   OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      I_LOAD_EA ),
                (0xF6, 0x03) => (Mnemonic::NEG,   OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      I_LOAD_EA ),
                (0xF6, 0x04) => (Mnemonic::MUL,   OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      I_LOAD_EA ),
                (0xF6, 0x05) => (Mnemonic::IMUL,  OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      I_LOAD_EA ),
                (0xF6, 0x06) => (Mnemonic::DIV,   OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      I_LOAD_EA ),
                (0xF6, 0x07) => (Mnemonic::IDIV,  OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      I_LOAD_EA ),

                (0xF7, 0x00) => (Mnemonic::TEST,  OperandTemplate::ModRM16,   OperandTemplate::Immediate16,   I_LOAD_EA ),
                (0xF7, 0x01) => (Mnemonic::TEST,  OperandTemplate::ModRM16,   OperandTemplate::Immediate16,   I_LOAD_EA ),
                (0xF7, 0x02) => (Mnemonic::NOT,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     I_LOAD_EA ),
                (0xF7, 0x03) => (Mnemonic::NEG,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     I_LOAD_EA ),
                (0xF7, 0x04) => (Mnemonic::MUL,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     I_LOAD_EA ),
                (0xF7, 0x05) => (Mnemonic::IMUL,  OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     I_LOAD_EA ),
                (0xF7, 0x06) => (Mnemonic::DIV,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     I_LOAD_EA ),
                (0xF7, 0x07) => (Mnemonic::IDIV,  OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     I_LOAD_EA ),

                (0xFE, 0x00) => (Mnemonic::INC,   OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      I_LOAD_EA ),
                (0xFE, 0x01) => (Mnemonic::DEC,   OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      I_LOAD_EA ),
                (0xFE, 0x02) => (Mnemonic::CALL,  OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      I_LOAD_EA ),
                (0xFE, 0x03) => (Mnemonic::CALLF, OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      I_LOAD_EA ),
                (0xFE, 0x04) => (Mnemonic::JMP,   OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      I_LOAD_EA ),
                (0xFE, 0x05) => (Mnemonic::JMPF,  OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      I_LOAD_EA ),
                (0xFE, 0x06) => (Mnemonic::PUSH,  OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      I_LOAD_EA ),
                (0xFE, 0x07) => (Mnemonic::PUSH,  OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      I_LOAD_EA ),

                (0xFF, 0x00) => (Mnemonic::INC,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     I_LOAD_EA ),
                (0xFF, 0x01) => (Mnemonic::DEC,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     I_LOAD_EA ),
                (0xFF, 0x02) => (Mnemonic::CALL,  OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     I_LOAD_EA ),
                (0xFF, 0x03) => (Mnemonic::CALLF, OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     I_LOAD_EA ),
                (0xFF, 0x04) => (Mnemonic::JMP,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     I_LOAD_EA ),
                (0xFF, 0x05) => (Mnemonic::JMPF,  OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     I_LOAD_EA ),
                (0xFF, 0x06) => (Mnemonic::PUSH,  OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     I_LOAD_EA ),
                (0xFF, 0x07) => (Mnemonic::PUSH,  OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     I_LOAD_EA ),

                _=> (Mnemonic::NoOpcode, OperandTemplate::NoOperand, OperandTemplate::NoOperand, 0)
            };

            op_flags |= I_HAS_MODRM;
        }

        // Handle templatized operands

        // Set a flag to load the ModRM Byte if either operand requires one
        let load_modrm_op1 = match operand1_template {
            OperandTemplate::ModRM8 => true,
            OperandTemplate::ModRM16 => true,
            OperandTemplate::Register8 => true,
            OperandTemplate::Register16 => true,
            _=> false
        };
        let load_modrm_op2 = match operand2_template {
            OperandTemplate::ModRM8 => true,
            OperandTemplate::ModRM16 => true,
            OperandTemplate::Register8 => true,
            OperandTemplate::Register16 => true,
            _=> false
        };

        // Load the ModRM byte if required
        if !loaded_modrm && (load_modrm_op1 | load_modrm_op2) {
            op_flags |= I_HAS_MODRM;
            let modrm_len;
            (modrm, modrm_len) = ModRmByte::read(bytes);
            size += modrm_len;
            loaded_modrm = true;
        }

        if loaded_modrm && (op_flags & I_LOAD_EA == 0) {
            // The EA calculated by the modrm will not be loaded (ie, we proceed to EADONE instead of EALOAD).

            if opcode == 0x8F {
                if let AddressingMode::RegisterMode = modrm.get_addressing_mode() {
                    // Don't process modrm cycles?
                }
                else {
                    bytes.wait_i(2, &[0x1e3, MC_RTN]);
                }
            }
            else {
                bytes.wait_i(2, &[0x1e3, MC_RTN]);
            }
        }

        // Match templatized operands.
        let mut match_op = |op_template| -> (OperandType, OperandSize) {
            match op_template {

                OperandTemplate::ModRM8 => {
                    let addr_mode = modrm.get_addressing_mode();
                    let operand_type = match addr_mode {
                        AddressingMode::RegisterMode => OperandType::Register8(modrm.get_op1_reg8()),
                        _=> OperandType::AddressingMode(addr_mode),
                    };
                    (operand_type, OperandSize::Operand8)
                }
                OperandTemplate::ModRM16 => {
                    let addr_mode = modrm.get_addressing_mode();
                    let operand_type = match addr_mode {
                        AddressingMode::RegisterMode => OperandType::Register16(modrm.get_op1_reg16()),
                        _=> OperandType::AddressingMode(addr_mode)
                    };
                    (operand_type, OperandSize::Operand16)
                }
                OperandTemplate::Register8 => {
                    let operand_type = OperandType::Register8(modrm.get_op2_reg8());
                    (operand_type, OperandSize::Operand8)
                }
                OperandTemplate::Register16 => {
                    let operand_type = OperandType::Register16(modrm.get_op2_reg16());
                    (operand_type, OperandSize::Operand16)
                }
                OperandTemplate::SegmentRegister => {
                    let operand_type = OperandType::Register16(modrm.get_op2_segmentreg16());
                    (operand_type, OperandSize::Operand16)
                }
                OperandTemplate::Register8Encoded => {
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
                OperandTemplate::Register16Encoded => {
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
                OperandTemplate::Immediate8 => {
                    // Peek at immediate value now, fetch during execute
                    let operand = bytes.q_peek_u8();
                    size += 1;
                    (OperandType::Immediate8(operand), OperandSize::Operand8)
                }
                OperandTemplate::Immediate16 => {
                    // Peek at immediate value now, fetch during execute
                    let operand = bytes.q_peek_u16();
                    size += 2;
                    (OperandType::Immediate16(operand), OperandSize::Operand16)
                }
                OperandTemplate::Immediate8SignExtended => {
                    // Peek at immediate value now, fetch during execute
                    let operand = bytes.q_peek_i8();
                    size += 1;
                    (OperandType::Immediate8s(operand), OperandSize::Operand8)
                }
                OperandTemplate::Relative8 => {
                    // Peek at rel8 value now, fetch during execute
                    let operand = bytes.q_peek_i8();
                    size += 1;
                    (OperandType::Relative8(operand), OperandSize::Operand8)
                }
                OperandTemplate::Relative16 => {
                    // Peek at rel16 value now, fetch during execute
                    let operand = bytes.q_peek_i16();
                    size += 2;
                    (OperandType::Relative16(operand), OperandSize::Operand16)
                }
                OperandTemplate::Offset8 => {
                    // Peek at offset8 value now, fetch during execute
                    let operand = bytes.q_peek_u16();
                    size += 2;
                    (OperandType::Offset8(operand), OperandSize::Operand8)
                }
                OperandTemplate::Offset16 => {
                    // Peek at offset16 value now, fetch during execute
                    let operand = bytes.q_peek_u16();
                    size += 2;
                    (OperandType::Offset16(operand), OperandSize::Operand16)
                }
                OperandTemplate::FixedRegister8(r8) => {
                    (OperandType::Register8(r8), OperandSize::Operand8)
                }
                OperandTemplate::FixedRegister16(r16) => {
                    (OperandType::Register16(r16), OperandSize::Operand16)
                }
                /*
                OperandTemplate::NearAddress => {
                    let offset = bytes.q_read_u16(QueueType::Subsequent, QueueReader::Eu);
                    size += 2;
                    Ok((OperandType::NearAddress(offset), OperandSize::NoSize))
                }
                */
                OperandTemplate::FarAddress => {
                    let (segment, offset) = bytes.q_peek_farptr16();
                    size += 4;
                    (OperandType::FarAddress(segment,offset), OperandSize::NoSize)
                }
                _=>(OperandType::NoOperand,OperandSize::NoOperand)
            }
        };

        match operand1_template {
            OperandTemplate::NoTemplate => {},
            _=> (operand1_type, operand1_size) = match_op(operand1_template)
        }

        match operand2_template {
            OperandTemplate::NoTemplate => {},
            _=> (operand2_type, operand2_size) = match_op(operand2_template)
        }

        // Set a flag if either of the instruction operands is a memory operand.
        if let OperandType::AddressingMode(_) = operand1_type {
            op_flags |= I_USES_MEM;
        }
        if let OperandType::AddressingMode(_) = operand2_type {
            op_flags |= I_USES_MEM;
        }

        //size = bytes.tell() as u32 - op_address;

        if let Mnemonic::InvalidOpcode = mnemonic {
            return Err(Box::new(InstructionDecodeError::UnsupportedOpcode(opcode)));
        }

        Ok(Instruction {
            opcode,
            flags: op_flags,
            prefixes: op_prefixes,
            address: 0,
            size,
            mnemonic,
            segment_override: op_segment_override,
            operand1_type,
            operand1_size,
            operand2_type,
            operand2_size
        })
    }
}

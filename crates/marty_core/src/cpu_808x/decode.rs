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

    cpu_808x::decode.rs

    Opcode fetch and instruction decoding routines.

    This module is implemented as an associated function, decode(), which
    operates on implementors of ByteQueue. This allows instruction decoding
    from either the processor instruction queue emulation, or directly
    from emulator memory for our debug disassembly viewer.

*/

use crate::{
    bytequeue::*,
    cpu_808x::{gdr::GdrEntry, modrm::ModRmByte, Intel808x, *},
    cpu_common::{
        alu::Xi,
        operands::OperandSize,
        AddressingMode,
        Displacement,
        Instruction,
        Mnemonic,
        OperandType,
        Segment,
        OPCODE_PREFIX_CS_OVERRIDE,
        OPCODE_PREFIX_DS_OVERRIDE,
        OPCODE_PREFIX_ES_OVERRIDE,
        OPCODE_PREFIX_LOCK,
        OPCODE_PREFIX_REP1,
        OPCODE_PREFIX_REP2,
        OPCODE_PREFIX_SS_OVERRIDE,
    },
};
use std::{error::Error, fmt::Display};

#[derive(Copy, Clone, PartialEq)]
pub enum OperandTemplate {
    NoOperand,
    ModRM8,
    ModRM16,
    Register8,
    Register16,
    SegmentRegister,
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

impl OperandTemplate {
    #[inline(always)]
    pub fn resolve_operand(
        &self,
        bytes: &mut impl ByteQueue,
        modrm: &ModRmByte,
        displacement: Displacement,
        size: &mut u32,
        peek: bool,
    ) -> OperandType {
        match (self, peek) {
            (OperandTemplate::ModRM8, _) => {
                let addr_mode = modrm.addressing_mode(displacement);
                match addr_mode {
                    AddressingMode::RegisterMode => OperandType::Register8(modrm.op1_reg8()),
                    _ => OperandType::AddressingMode(addr_mode, OperandSize::Operand8),
                }
            }
            (OperandTemplate::ModRM16, _) => {
                let addr_mode = modrm.addressing_mode(displacement);
                match addr_mode {
                    AddressingMode::RegisterMode => OperandType::Register16(modrm.op1_reg16()),
                    _ => OperandType::AddressingMode(addr_mode, OperandSize::Operand16),
                }
            }
            (OperandTemplate::Register8, _) => OperandType::Register8(modrm.op2_reg8()),
            (OperandTemplate::Register16, _) => OperandType::Register16(modrm.op2_reg16()),
            (OperandTemplate::SegmentRegister, _) => OperandType::Register16(modrm.op2_segmentreg16()),
            (OperandTemplate::Immediate8, true) => {
                // Peek at immediate value now, fetch during execute
                let operand = bytes.q_peek_u8();
                *size += 1;
                OperandType::Immediate8(operand)
            }
            (OperandTemplate::Immediate8, false) => {
                *size += 1;
                OperandType::Immediate8(0)
            }
            (OperandTemplate::Immediate16, true) => {
                // Peek at immediate value now, fetch during execute
                let operand = bytes.q_peek_u16();
                *size += 2;
                OperandType::Immediate16(operand)
            }
            (OperandTemplate::Immediate16, false) => {
                *size += 2;
                OperandType::Immediate16(0)
            }
            (OperandTemplate::Immediate8SignExtended, true) => {
                // Peek at immediate value now, fetch during execute
                let operand = bytes.q_peek_i8();
                *size += 1;
                OperandType::Immediate8s(operand)
            }
            (OperandTemplate::Immediate8SignExtended, false) => {
                *size += 1;
                OperandType::Immediate8s(0)
            }
            (OperandTemplate::Relative8, true) => {
                // Peek at rel8 value now, fetch during execute
                let operand = bytes.q_peek_i8();
                *size += 1;
                OperandType::Relative8(operand)
            }
            (OperandTemplate::Relative8, false) => {
                *size += 1;
                OperandType::Relative8(0)
            }
            (OperandTemplate::Relative16, true) => {
                // Peek at rel16 value now, fetch during execute
                let operand = bytes.q_peek_i16();
                *size += 2;
                OperandType::Relative16(operand)
            }
            (OperandTemplate::Relative16, false) => {
                *size += 2;
                OperandType::Relative16(0)
            }
            (OperandTemplate::Offset8, true) => {
                // Peek at offset8 value now, fetch during execute
                let operand = bytes.q_peek_u16();
                *size += 2;
                OperandType::Offset8(operand)
            }
            (OperandTemplate::Offset8, false) => {
                *size += 2;
                OperandType::Offset8(0)
            }
            (OperandTemplate::Offset16, true) => {
                // Peek at offset16 value now, fetch during execute
                let operand = bytes.q_peek_u16();
                *size += 2;
                OperandType::Offset16(operand)
            }
            (OperandTemplate::Offset16, false) => {
                *size += 2;
                OperandType::Offset16(0)
            }
            (OperandTemplate::FixedRegister8(r8), _) => OperandType::Register8(*r8),
            (OperandTemplate::FixedRegister16(r16), _) => OperandType::Register16(*r16),
            (OperandTemplate::FarAddress, true) => {
                let (segment, offset) = bytes.q_peek_farptr16();
                *size += 4;
                OperandType::FarAddress(segment, offset)
            }
            (OperandTemplate::FarAddress, false) => {
                *size += 4;
                OperandType::FarAddress(0, 0)
            }
            _ => OperandType::NoOperand,
        }
    }
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
    pub gdr: GdrEntry,
    pub mc: u16,
    pub mc_fn: fn(&mut Intel808x),
    pub xi: Option<Xi>,
    pub mnemonic: Mnemonic,
    pub operand1: OperandTemplate,
    pub operand2: OperandTemplate,
}

macro_rules! inst {
    ($op:literal, $grp:literal, $gdr:literal, $mc:literal, $mc_fn:ident, $xi:ident, $m:ident, $o1:expr, $o2:expr) => {
        InstTemplate {
            grp: $grp,
            gdr: GdrEntry($gdr),
            mc: $mc,
            mc_fn: Intel808x::$mc_fn,
            xi: Some(Xi::$xi),
            mnemonic: Mnemonic::$m,
            operand1: $o1,
            operand2: $o2,
        }
    };
    ($op:literal, $grp:literal, $gdr:literal, $mc:literal, $mc_fn:ident, $m:ident, $o1:expr, $o2:expr) => {
        InstTemplate {
            grp: $grp,
            gdr: GdrEntry($gdr),
            mc: $mc,
            mc_fn: Intel808x::$mc_fn,
            xi: None,
            mnemonic: Mnemonic::$m,
            operand1: $o1,
            operand2: $o2,
        }
    };
}

#[rustfmt::skip]
pub const DECODE: [InstTemplate; 352] = [
    inst!( 0x00,  0, 0b0100101000000000, 0x008, mc_008,  ADD   , ADD,     Ot::ModRM8,                             Ot::Register8),
    inst!( 0x01,  0, 0b0100101000000000, 0x008, mc_008,  ADD   , ADD,     Ot::ModRM16,                            Ot::Register16),
    inst!( 0x02,  0, 0b0100101000000000, 0x008, mc_008,  ADD   , ADD,     Ot::Register8,                          Ot::ModRM8),
    inst!( 0x03,  0, 0b0100101000000000, 0x008, mc_008,  ADD   , ADD,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0x04,  0, 0b0100100010010010, 0x018, mc_018,  ADD   , ADD,     Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0x05,  0, 0b0100100010010010, 0x018, mc_018,  ADD   , ADD,     Ot::FixedRegister16(Register16::AX),    Ot::Immediate16),
    inst!( 0x06,  0, 0b0100000000110010, 0x02c, mc_02c,  PUSH,    Ot::FixedRegister16(Register16::ES),    Ot::NoOperand),
    inst!( 0x07,  0, 0b0100000000110010, 0x038, mc_038,  POP,     Ot::FixedRegister16(Register16::ES),    Ot::NoOperand),
    inst!( 0x08,  0, 0b0100101000000000, 0x008, mc_008,  OR    , OR,      Ot::ModRM8,                             Ot::Register8),
    inst!( 0x09,  0, 0b0100101000000000, 0x008, mc_008,  OR    , OR,      Ot::ModRM16,                            Ot::Register16),
    inst!( 0x0A,  0, 0b0100101000000000, 0x008, mc_008,  OR    , OR,      Ot::Register8,                          Ot::ModRM8),
    inst!( 0x0B,  0, 0b0100101000000000, 0x008, mc_008,  OR    , OR,      Ot::Register16,                         Ot::ModRM16),
    inst!( 0x0C,  0, 0b0100100010010010, 0x018, mc_018,  OR    , OR,      Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0x0D,  0, 0b0100100010010010, 0x018, mc_018,  OR    , OR,      Ot::FixedRegister16(Register16::AX),    Ot::Immediate16),
    inst!( 0x0E,  0, 0b0100000000110010, 0x02c, mc_02c,          PUSH,    Ot::FixedRegister16(Register16::CS),    Ot::NoOperand),
    inst!( 0x0F,  0, 0b0100000000110010, 0x038, mc_038,          POP,     Ot::FixedRegister16(Register16::CS),    Ot::NoOperand),
    inst!( 0x10,  0, 0b0100101000000000, 0x008, mc_008,  ADC   , ADC,     Ot::ModRM8,                             Ot::Register8),
    inst!( 0x11,  0, 0b0100101000000000, 0x008, mc_008,  ADC   , ADC,     Ot::ModRM16,                            Ot::Register16),
    inst!( 0x12,  0, 0b0100101000000000, 0x008, mc_008,  ADC   , ADC,     Ot::Register8,                          Ot::ModRM8),
    inst!( 0x13,  0, 0b0100101000000000, 0x008, mc_008,  ADC   , ADC,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0x14,  0, 0b0100100010010010, 0x018, mc_018,  ADC   , ADC,     Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0x15,  0, 0b0100100010010010, 0x018, mc_018,  ADC   , ADC,     Ot::FixedRegister16(Register16::AX),    Ot::Immediate16),
    inst!( 0x16,  0, 0b0100000000110010, 0x02c, mc_02c,          PUSH,    Ot::FixedRegister16(Register16::SS),    Ot::NoOperand),
    inst!( 0x17,  0, 0b0100000000110010, 0x038, mc_038,          POP,     Ot::FixedRegister16(Register16::SS),    Ot::NoOperand),
    inst!( 0x18,  0, 0b0100101000000000, 0x008, mc_008,  SBB   , SBB,     Ot::ModRM8,                             Ot::Register8),
    inst!( 0x19,  0, 0b0100101000000000, 0x008, mc_008,  SBB   , SBB,     Ot::ModRM16,                            Ot::Register16),
    inst!( 0x1A,  0, 0b0100101000000000, 0x008, mc_008,  SBB   , SBB,     Ot::Register8,                          Ot::ModRM8),
    inst!( 0x1B,  0, 0b0100101000000000, 0x008, mc_008,  SBB   , SBB,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0x1C,  0, 0b0100100010010010, 0x018, mc_018,  SBB   , SBB,     Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0x1D,  0, 0b0100100010010010, 0x018, mc_018,  SBB   , SBB,     Ot::FixedRegister16(Register16::AX),    Ot::Immediate16),
    inst!( 0x1E,  0, 0b0100000000110010, 0x02c, mc_02c,          PUSH,    Ot::FixedRegister16(Register16::DS),    Ot::NoOperand),
    inst!( 0x1F,  0, 0b0100000000110010, 0x038, mc_038,          POP,     Ot::FixedRegister16(Register16::DS),    Ot::NoOperand),
    inst!( 0x20,  0, 0b0100101000000000, 0x008, mc_008,  AND   , AND,     Ot::ModRM8,                             Ot::Register8),
    inst!( 0x21,  0, 0b0100101000000000, 0x008, mc_008,  AND   , AND,     Ot::ModRM16,                            Ot::Register16),
    inst!( 0x22,  0, 0b0100101000000000, 0x008, mc_008,  AND   , AND,     Ot::Register8,                          Ot::ModRM8),
    inst!( 0x23,  0, 0b0100101000000000, 0x008, mc_008,  AND   , AND,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0x24,  0, 0b0100100010010010, 0x018, mc_018,  AND   , AND,     Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0x25,  0, 0b0100100010010010, 0x018, mc_018,  AND   , AND,     Ot::FixedRegister16(Register16::AX),    Ot::Immediate16),
    inst!( 0x26,  0, 0b0100010000111010, 0x1FF, mc_nop,          Prefix,  Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x27,  0, 0b0101000000110010, 0x144, mc_144,  DAA   , DAA,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x28,  0, 0b0100101000000000, 0x008, mc_008,  SUB   , SUB,     Ot::ModRM8,                             Ot::Register8),
    inst!( 0x29,  0, 0b0100101000000000, 0x008, mc_008,  SUB   , SUB,     Ot::ModRM16,                            Ot::Register16),
    inst!( 0x2A,  0, 0b0100101000000000, 0x008, mc_008,  SUB   , SUB,     Ot::Register8,                          Ot::ModRM8),
    inst!( 0x2B,  0, 0b0100101000000000, 0x008, mc_008,  SUB   , SUB,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0x2C,  0, 0b0100100010010010, 0x018, mc_018,  SUB   , SUB,     Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0x2D,  0, 0b0100100010010010, 0x018, mc_018,  SUB   , SUB,     Ot::FixedRegister16(Register16::AX),    Ot::Immediate16),
    inst!( 0x2E,  0, 0b0100010000111010, 0x1FF, mc_nop,          Prefix,  Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x2F,  0, 0b0101000000110010, 0x144, mc_144,  DAS   , DAS,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x30,  0, 0b0100101000000000, 0x008, mc_008,  XOR   , XOR,     Ot::ModRM8,                             Ot::Register8),
    inst!( 0x31,  0, 0b0100101000000000, 0x008, mc_008,  XOR   , XOR,     Ot::ModRM16,                            Ot::Register16),
    inst!( 0x32,  0, 0b0100101000000000, 0x008, mc_008,  XOR   , XOR,     Ot::Register8,                          Ot::ModRM8),
    inst!( 0x33,  0, 0b0100101000000000, 0x008, mc_008,  XOR   , XOR,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0x34,  0, 0b0100100010010010, 0x018, mc_018,  XOR   , XOR,     Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0x35,  0, 0b0100100010010010, 0x018, mc_018,  XOR   , XOR,     Ot::FixedRegister16(Register16::AX),    Ot::Immediate16),
    inst!( 0x36,  0, 0b0100010000111010, 0x1FF, mc_nop,          Prefix,  Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x37,  0, 0b0101000000110010, 0x148, mc_148,  AAA   , AAA,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x38,  0, 0b0100101000000000, 0x008, mc_008,  CMP   , CMP,     Ot::ModRM8,                             Ot::Register8),
    inst!( 0x39,  0, 0b0100101000000000, 0x008, mc_008,  CMP   , CMP,     Ot::ModRM16,                            Ot::Register16),
    inst!( 0x3A,  0, 0b0100101000000000, 0x008, mc_008,  CMP   , CMP,     Ot::Register8,                          Ot::ModRM8),
    inst!( 0x3B,  0, 0b0100101000000000, 0x008, mc_008,  CMP   , CMP,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0x3C,  0, 0b0100100010010010, 0x018, mc_018,  CMP   , CMP,     Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0x3D,  0, 0b0100100010010010, 0x018, mc_018,  CMP   , CMP,     Ot::FixedRegister16(Register16::AX),    Ot::Immediate16),
    inst!( 0x3E,  0, 0b0100010000111010, 0x1FF, mc_nop,          Prefix,  Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x3F,  0, 0b0101000000110010, 0x148, mc_148,  AAS   , AAS,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x40,  0, 0b0000000000110010, 0x17c, mc_17c,  INC   , INC,     Ot::FixedRegister16(Register16::AX),    Ot::NoOperand),
    inst!( 0x41,  0, 0b0000000000110010, 0x17c, mc_17c,  INC   , INC,     Ot::FixedRegister16(Register16::CX),    Ot::NoOperand),
    inst!( 0x42,  0, 0b0000000000110010, 0x17c, mc_17c,  INC   , INC,     Ot::FixedRegister16(Register16::DX),    Ot::NoOperand),
    inst!( 0x43,  0, 0b0000000000110010, 0x17c, mc_17c,  INC   , INC,     Ot::FixedRegister16(Register16::BX),    Ot::NoOperand),
    inst!( 0x44,  0, 0b0000000000110010, 0x17c, mc_17c,  INC   , INC,     Ot::FixedRegister16(Register16::SP),    Ot::NoOperand),
    inst!( 0x45,  0, 0b0000000000110010, 0x17c, mc_17c,  INC   , INC,     Ot::FixedRegister16(Register16::BP),    Ot::NoOperand),
    inst!( 0x46,  0, 0b0000000000110010, 0x17c, mc_17c,  INC   , INC,     Ot::FixedRegister16(Register16::SI),    Ot::NoOperand),
    inst!( 0x47,  0, 0b0000000000110010, 0x17c, mc_17c,  INC   , INC,     Ot::FixedRegister16(Register16::DI),    Ot::NoOperand),
    inst!( 0x48,  0, 0b0000000000110010, 0x17c, mc_17c,  DEC   , DEC,     Ot::FixedRegister16(Register16::AX),    Ot::NoOperand),
    inst!( 0x49,  0, 0b0000000000110010, 0x17c, mc_17c,  DEC   , DEC,     Ot::FixedRegister16(Register16::CX),    Ot::NoOperand),
    inst!( 0x4A,  0, 0b0000000000110010, 0x17c, mc_17c,  DEC   , DEC,     Ot::FixedRegister16(Register16::DX),    Ot::NoOperand),
    inst!( 0x4B,  0, 0b0000000000110010, 0x17c, mc_17c,  DEC   , DEC,     Ot::FixedRegister16(Register16::BX),    Ot::NoOperand),
    inst!( 0x4C,  0, 0b0000000000110010, 0x17c, mc_17c,  DEC   , DEC,     Ot::FixedRegister16(Register16::SP),    Ot::NoOperand),
    inst!( 0x4D,  0, 0b0000000000110010, 0x17c, mc_17c,  DEC   , DEC,     Ot::FixedRegister16(Register16::BP),    Ot::NoOperand),
    inst!( 0x4E,  0, 0b0000000000110010, 0x17c, mc_17c,  DEC   , DEC,     Ot::FixedRegister16(Register16::SI),    Ot::NoOperand),
    inst!( 0x4F,  0, 0b0000000000110010, 0x17c, mc_17c,  DEC   , DEC,     Ot::FixedRegister16(Register16::DI),    Ot::NoOperand),
    inst!( 0x50,  0, 0b0000000000110010, 0x028, mc_028,          PUSH,    Ot::FixedRegister16(Register16::AX),    Ot::NoOperand),
    inst!( 0x51,  0, 0b0000000000110010, 0x028, mc_028,          PUSH,    Ot::FixedRegister16(Register16::CX),    Ot::NoOperand),
    inst!( 0x52,  0, 0b0000000000110010, 0x028, mc_028,          PUSH,    Ot::FixedRegister16(Register16::DX),    Ot::NoOperand),
    inst!( 0x53,  0, 0b0000000000110010, 0x028, mc_028,          PUSH,    Ot::FixedRegister16(Register16::BX),    Ot::NoOperand),
    inst!( 0x54,  0, 0b0000000000110010, 0x028, mc_028,          PUSH,    Ot::FixedRegister16(Register16::SP),    Ot::NoOperand),
    inst!( 0x55,  0, 0b0000000000110010, 0x028, mc_028,          PUSH,    Ot::FixedRegister16(Register16::BP),    Ot::NoOperand),
    inst!( 0x56,  0, 0b0000000000110010, 0x028, mc_028,          PUSH,    Ot::FixedRegister16(Register16::SI),    Ot::NoOperand),
    inst!( 0x57,  0, 0b0000000000110010, 0x028, mc_028,          PUSH,    Ot::FixedRegister16(Register16::DI),    Ot::NoOperand),
    inst!( 0x58,  0, 0b0000000000110010, 0x034, mc_034,          POP,     Ot::FixedRegister16(Register16::AX),    Ot::NoOperand),
    inst!( 0x59,  0, 0b0000000000110010, 0x034, mc_034,          POP,     Ot::FixedRegister16(Register16::CX),    Ot::NoOperand),
    inst!( 0x5A,  0, 0b0000000000110010, 0x034, mc_034,          POP,     Ot::FixedRegister16(Register16::DX),    Ot::NoOperand),
    inst!( 0x5B,  0, 0b0000000000110010, 0x034, mc_034,          POP,     Ot::FixedRegister16(Register16::BX),    Ot::NoOperand),
    inst!( 0x5C,  0, 0b0000000000110010, 0x034, mc_034,          POP,     Ot::FixedRegister16(Register16::SP),    Ot::NoOperand),
    inst!( 0x5D,  0, 0b0000000000110010, 0x034, mc_034,          POP,     Ot::FixedRegister16(Register16::BP),    Ot::NoOperand),
    inst!( 0x5E,  0, 0b0000000000110010, 0x034, mc_034,          POP,     Ot::FixedRegister16(Register16::SI),    Ot::NoOperand),
    inst!( 0x5F,  0, 0b0000000000110010, 0x034, mc_034,          POP,     Ot::FixedRegister16(Register16::DI),    Ot::NoOperand),
    inst!( 0x60,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JO,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x61,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JNO,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x62,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JB,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x63,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JNB,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x64,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JZ,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x65,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JNZ,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x66,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JBE,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x67,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JNBE,    Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x68,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JS,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x69,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JNS,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x6A,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JP,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x6B,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JNP,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x6C,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JL,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x6D,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JNL,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x6E,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JLE,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x6F,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JNLE,    Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x70,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JO,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x71,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JNO,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x72,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JB,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x73,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JNB,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x74,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JZ,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x75,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JNZ,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x76,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JBE,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x77,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JNBE,    Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x78,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JS,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x79,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JNS,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x7A,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JP,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x7B,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JNP,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x7C,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JL,      Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x7D,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JNL,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x7E,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JLE,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x7F,  0, 0b0000000000110010, 0x0e8, mc_0e8,          JNLE,    Ot::Relative8,                          Ot::NoOperand),
    inst!( 0x80,  1, 0b0110100000000000, 0x00c, mc_00c,  ADD   , Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x81,  2, 0b0110100000000000, 0x00c, mc_00c,  CMP   , Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x82,  3, 0b0110100000000000, 0x00c, mc_00c,  ADD   , Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x83,  4, 0b0110100000000000, 0x00c, mc_00c,  CMP   , Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x84,  0, 0b0110100000000000, 0x094, mc_094,          TEST,    Ot::ModRM8,                             Ot::Register8),
    inst!( 0x85,  0, 0b0110100000000000, 0x094, mc_094,          TEST,    Ot::ModRM16,                            Ot::Register16),
    inst!( 0x86,  0, 0b0110100000000000, 0x0a4, mc_0a4,          XCHG,    Ot::Register8,                          Ot::ModRM8),
    inst!( 0x87,  0, 0b0110100000000000, 0x0a4, mc_0a4,          XCHG,    Ot::Register16,                         Ot::ModRM16),
    inst!( 0x88,  0, 0b0100101000100010, 0x000, mc_000,          MOV,     Ot::ModRM8,                             Ot::Register8),
    inst!( 0x89,  0, 0b0100101000100010, 0x000, mc_000,          MOV,     Ot::ModRM16,                            Ot::Register16),
    inst!( 0x8A,  0, 0b0100101000100000, 0x000, mc_000,          MOV,     Ot::Register8,                          Ot::ModRM8),
    inst!( 0x8B,  0, 0b0100101000100000, 0x000, mc_000,          MOV,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0x8C,  0, 0b0100001100100010, 0x0ec, mc_0ec,          MOV,     Ot::ModRM16,                            Ot::SegmentRegister),
    inst!( 0x8D,  0, 0b0100000000100010, 0x004, mc_004,          LEA,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0x8E,  0, 0b0100001100100000, 0x0ec, mc_0ec,          MOV,     Ot::SegmentRegister,                    Ot::ModRM16),
    inst!( 0x8F,  0, 0b0100000000100010, 0x040, mc_040,          POP,     Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0x90,  0, 0b0100000000110010, 0x084, mc_084,          NOP,     Ot::FixedRegister16(Register16::AX),    Ot::FixedRegister16(Register16::AX)),
    inst!( 0x91,  0, 0b0100000000110010, 0x084, mc_084,          XCHG,    Ot::FixedRegister16(Register16::CX),    Ot::FixedRegister16(Register16::AX)),
    inst!( 0x92,  0, 0b0100000000110010, 0x084, mc_084,          XCHG,    Ot::FixedRegister16(Register16::DX),    Ot::FixedRegister16(Register16::AX)),
    inst!( 0x93,  0, 0b0100000000110010, 0x084, mc_084,          XCHG,    Ot::FixedRegister16(Register16::BX),    Ot::FixedRegister16(Register16::AX)),
    inst!( 0x94,  0, 0b0100000000110010, 0x084, mc_084,          XCHG,    Ot::FixedRegister16(Register16::SP),    Ot::FixedRegister16(Register16::AX)),
    inst!( 0x95,  0, 0b0100000000110010, 0x084, mc_084,          XCHG,    Ot::FixedRegister16(Register16::BP),    Ot::FixedRegister16(Register16::AX)),
    inst!( 0x96,  0, 0b0100000000110010, 0x084, mc_084,          XCHG,    Ot::FixedRegister16(Register16::SI),    Ot::FixedRegister16(Register16::AX)),
    inst!( 0x97,  0, 0b0100000000110010, 0x084, mc_084,          XCHG,    Ot::FixedRegister16(Register16::DI),    Ot::FixedRegister16(Register16::AX)),
    inst!( 0x98,  0, 0b0100000000110010, 0x054, mc_054,          CBW,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x99,  0, 0b0100000000110010, 0x058, mc_058,          CWD,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x9A,  0, 0b0100000000110010, 0x070, mc_070,          CALLF,   Ot::FarAddress,                         Ot::NoOperand),
    inst!( 0x9B,  0, 0b0100000000110010, 0x0f8, mc_0f8,          WAIT,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x9C,  0, 0b0100000000110010, 0x030, mc_030,          PUSHF,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x9D,  0, 0b0100000000110010, 0x03c, mc_03c,          POPF,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x9E,  0, 0b0100000000110010, 0x100, mc_100,          SAHF,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0x9F,  0, 0b0100000000110010, 0x104, mc_104,          LAHF,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xA0,  0, 0b0100100010110010, 0x060, mc_060,          MOV,     Ot::FixedRegister8(Register8::AL),      Ot::Offset8),
    inst!( 0xA1,  0, 0b0100100010110010, 0x060, mc_060,          MOV,     Ot::FixedRegister16(Register16::AX),    Ot::Offset16),
    inst!( 0xA2,  0, 0b0100100010110010, 0x064, mc_064,          MOV,     Ot::Offset8,                            Ot::FixedRegister8(Register8::AL)),
    inst!( 0xA3,  0, 0b0100100010110010, 0x064, mc_064,          MOV,     Ot::Offset16,                           Ot::FixedRegister16(Register16::AX)),
    inst!( 0xA4,  0, 0b0100100010110010, 0x12c, mc_12c,          MOVSB,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xA5,  0, 0b0100100010110010, 0x12c, mc_12c,          MOVSW,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xA6,  0, 0b0100100010110010, 0x120, mc_120,          CMPSB,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xA7,  0, 0b0100100010110010, 0x120, mc_120,          CMPSW,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xA8,  0, 0b0100100010110010, 0x09C, mc_09c,          TEST,    Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0xA9,  0, 0b0100100010110010, 0x09C, mc_09c,          TEST,    Ot::FixedRegister16(Register16::AX),    Ot::Immediate16),
    inst!( 0xAA,  0, 0b0100100010110010, 0x11c, mc_11c,          STOSB,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xAB,  0, 0b0100100010110010, 0x11c, mc_11c,          STOSW,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xAC,  0, 0b0100100010110010, 0x12c, mc_12cb,         LODSB,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xAD,  0, 0b0100100010110010, 0x12c, mc_12cb,         LODSW,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xAE,  0, 0b0100100010110010, 0x120, mc_120,          SCASB,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xAF,  0, 0b0100100010110010, 0x120, mc_120,          SCASW,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xB0,  0, 0b0100000000110010, 0x01c, mc_01c,          MOV,     Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0xB1,  0, 0b0100000000110010, 0x01c, mc_01c,          MOV,     Ot::FixedRegister8(Register8::CL),      Ot::Immediate8),
    inst!( 0xB2,  0, 0b0100000000110010, 0x01c, mc_01c,          MOV,     Ot::FixedRegister8(Register8::DL),      Ot::Immediate8),
    inst!( 0xB3,  0, 0b0100000000110010, 0x01c, mc_01c,          MOV,     Ot::FixedRegister8(Register8::BL),      Ot::Immediate8),
    inst!( 0xB4,  0, 0b0100000000110010, 0x01c, mc_01c,          MOV,     Ot::FixedRegister8(Register8::AH),      Ot::Immediate8),
    inst!( 0xB5,  0, 0b0100000000110010, 0x01c, mc_01c,          MOV,     Ot::FixedRegister8(Register8::CH),      Ot::Immediate8),
    inst!( 0xB6,  0, 0b0100000000110010, 0x01c, mc_01c,          MOV,     Ot::FixedRegister8(Register8::DH),      Ot::Immediate8),
    inst!( 0xB7,  0, 0b0100000000110010, 0x01c, mc_01c,          MOV,     Ot::FixedRegister8(Register8::BH),      Ot::Immediate8),
    inst!( 0xB8,  0, 0b0100000000110010, 0x01c, mc_01c,          MOV,     Ot::FixedRegister16(Register16::AX),    Ot::Immediate16),
    inst!( 0xB9,  0, 0b0100000000110010, 0x01c, mc_01c,          MOV,     Ot::FixedRegister16(Register16::CX),    Ot::Immediate16),
    inst!( 0xBA,  0, 0b0100000000110010, 0x01c, mc_01c,          MOV,     Ot::FixedRegister16(Register16::DX),    Ot::Immediate16),
    inst!( 0xBB,  0, 0b0100000000110010, 0x01c, mc_01c,          MOV,     Ot::FixedRegister16(Register16::BX),    Ot::Immediate16),
    inst!( 0xBC,  0, 0b0100000000110010, 0x01c, mc_01c,          MOV,     Ot::FixedRegister16(Register16::SP),    Ot::Immediate16),
    inst!( 0xBD,  0, 0b0100000000110010, 0x01c, mc_01c,          MOV,     Ot::FixedRegister16(Register16::BP),    Ot::Immediate16),
    inst!( 0xBE,  0, 0b0100000000110010, 0x01c, mc_01c,          MOV,     Ot::FixedRegister16(Register16::SI),    Ot::Immediate16),
    inst!( 0xBF,  0, 0b0100000000110010, 0x01c, mc_01c,          MOV,     Ot::FixedRegister16(Register16::DI),    Ot::Immediate16),
    inst!( 0xC0,  0, 0b0100000000110000, 0x0cc, mc_0cc,          RETN,    Ot::Immediate16,                        Ot::NoOperand),
    inst!( 0xC1,  0, 0b0100000000110000, 0x0bc, mc_0bc,          RETN,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xC2,  0, 0b0100000000110000, 0x0cc, mc_0cc,          RETN,    Ot::Immediate16,                        Ot::NoOperand),
    inst!( 0xC3,  0, 0b0100000000110000, 0x0bc, mc_0bc,          RETN,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xC4,  0, 0b0100000000100000, 0x0f0, mc_0f0,          LES,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0xC5,  0, 0b0100000000100000, 0x0f4, mc_0f4,          LDS,     Ot::Register16,                         Ot::ModRM16),
    inst!( 0xC6,  0, 0b0100100000100010, 0x014, mc_014,          MOV,     Ot::ModRM8,                             Ot::Immediate8),
    inst!( 0xC7,  0, 0b0100100000100010, 0x014, mc_014,          MOV,     Ot::ModRM16,                            Ot::Immediate16),
    inst!( 0xC8,  0, 0b0100000000110000, 0x0cc, mc_0cc,          RETF,    Ot::Immediate16,                        Ot::NoOperand),
    inst!( 0xC9,  0, 0b0100000000110000, 0x0c0, mc_0c0,          RETF,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xCA,  0, 0b0100000000110000, 0x0cc, mc_0cc,          RETF,    Ot::Immediate16,                        Ot::NoOperand),
    inst!( 0xCB,  0, 0b0100000000110000, 0x0c0, mc_0c0,          RETF,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xCC,  0, 0b0100000000110000, 0x1b0, mc_1b0,          INT3,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xCD,  0, 0b0100000000110000, 0x1a8, mc_1a8,          INT,     Ot::Immediate8,                         Ot::NoOperand),
    inst!( 0xCE,  0, 0b0100000000110000, 0x1ac, mc_1ac,          INTO,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xCF,  0, 0b0100000000110000, 0x0c8, mc_0c8,          IRET,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xD0,  5, 0b0100100000000000, 0x088, mc_088,  ROL   , Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xD1,  6, 0b0100100000000000, 0x088, mc_088,  SAR   , Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xD2,  7, 0b0100100000000000, 0x08c, mc_08c,  ROL   , Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xD3,  8, 0b0100100000000000, 0x08c, mc_08c,  SAR   , Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xD4,  0, 0b0101000000110000, 0x174, mc_174,          AAM,     Ot::Immediate8,                         Ot::NoOperand),
    inst!( 0xD5,  0, 0b0101000000110000, 0x170, mc_170,          AAD,     Ot::Immediate8,                         Ot::NoOperand),
    inst!( 0xD6,  0, 0b0101000000110000, 0x0a0, mc_0a0,          SALC,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xD7,  0, 0b0101000000110000, 0x10c, mc_10c,          XLAT,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xD8,  0, 0b0100000000100000, 0x108, mc_108,          ESC,     Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xD9,  0, 0b0100000000100000, 0x108, mc_108,          ESC,     Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xDA,  0, 0b0100000000100000, 0x108, mc_108,          ESC,     Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xDB,  0, 0b0100000000100000, 0x108, mc_108,          ESC,     Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xDC,  0, 0b0100000000100000, 0x108, mc_108,          ESC,     Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xDD,  0, 0b0100000000100000, 0x108, mc_108,          ESC,     Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xDE,  0, 0b0100000000100000, 0x108, mc_108,          ESC,     Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xDF,  0, 0b0100000000100000, 0x108, mc_108,          ESC,     Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xE0,  0, 0b0110000000110000, 0x138, mc_138,          LOOPNE,  Ot::Relative8,                          Ot::NoOperand),
    inst!( 0xE1,  0, 0b0110000000110000, 0x138, mc_138,          LOOPE,   Ot::Relative8,                          Ot::NoOperand),
    inst!( 0xE2,  0, 0b0110000000110000, 0x140, mc_140,          LOOP,    Ot::Relative8,                          Ot::NoOperand),
    inst!( 0xE3,  0, 0b0110000000110000, 0x134, mc_134,          JCXZ,    Ot::Relative8,                          Ot::NoOperand),
    inst!( 0xE4,  0, 0b0100100010110011, 0x0ac, mc_0ac,          IN,      Ot::FixedRegister8(Register8::AL),      Ot::Immediate8),
    inst!( 0xE5,  0, 0b0100100010110011, 0x0ac, mc_0ac,          IN,      Ot::FixedRegister16(Register16::AX),    Ot::Immediate8),
    inst!( 0xE6,  0, 0b0100100010110011, 0x0b0, mc_0b0,          OUT,     Ot::Immediate8,                         Ot::FixedRegister8(Register8::AL)),
    inst!( 0xE7,  0, 0b0100100010110011, 0x0b0, mc_0b0,          OUT,     Ot::Immediate8,                         Ot::FixedRegister16(Register16::AX)),
    inst!( 0xE8,  0, 0b0110000000110000, 0x07c, mc_07c,          CALL,    Ot::Relative16,                         Ot::NoOperand),
    inst!( 0xE9,  0, 0b0110000000110000, 0x0d0, mc_0d0,          JMP,     Ot::Relative16,                         Ot::NoOperand),
    inst!( 0xEA,  0, 0b0110000000110000, 0x0e0, mc_0e0,          JMPF,    Ot::FarAddress,                         Ot::NoOperand),
    inst!( 0xEB,  0, 0b0110000000110000, 0x0d0, mc_0d0,          JMP,     Ot::Relative8,                          Ot::NoOperand),
    inst!( 0xEC,  0, 0b0100100010110011, 0x0b4, mc_0b4,          IN,      Ot::FixedRegister8(Register8::AL),      Ot::FixedRegister16(Register16::DX)),
    inst!( 0xED,  0, 0b0100100010110011, 0x0b4, mc_0b4,          IN,      Ot::FixedRegister16(Register16::AX),    Ot::FixedRegister16(Register16::DX)),
    inst!( 0xEE,  0, 0b0100100010110011, 0x0b8, mc_0b8,          OUT,     Ot::FixedRegister16(Register16::DX),    Ot::FixedRegister8(Register8::AL)),
    inst!( 0xEF,  0, 0b0100100010110011, 0x0b8, mc_0b8,          OUT,     Ot::FixedRegister16(Register16::DX),    Ot::FixedRegister16(Register16::AX)),
    inst!( 0xF0,  0, 0b0100010000111010, 0x1FF, mc_nop,          LOCK,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xF1,  0, 0b0100010000111010, 0x1FF, mc_nop,          LOCK,    Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xF2,  0, 0b0100010000111010, 0x1FF, mc_nop,          Prefix,  Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xF3,  0, 0b0100010000111010, 0x1FF, mc_nop,          Prefix,  Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xF4,  0, 0b0100010000110010, 0x1FF, mc_nop,          HLT,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xF5,  0, 0b0100010000110010, 0x1FF, mc_nop,          CMC,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xF6,  9, 0b0100100000100100, 0x098, mc_098,          Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xF7, 10, 0b0100100000100100, 0x160, mc_160,          Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xF8,  0, 0b0100010001110010, 0x1FF, mc_nop,          CLC,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xF9,  0, 0b0100010001110010, 0x1FF, mc_nop,          STC,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xFA,  0, 0b0100010001110010, 0x1FF, mc_nop,          CLI,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xFB,  0, 0b0100010001110010, 0x1FF, mc_nop,          STI,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xFC,  0, 0b0100010001110010, 0x1FF, mc_nop,          CLD,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xFD,  0, 0b0100010001110010, 0x1FF, mc_nop,          STD,     Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xFE, 11, 0b0000100000100100, 0x020, mc_020,          Group,   Ot::NoOperand,                          Ot::NoOperand),
    inst!( 0xFF, 12, 0b0000100000100100, 0x026, mc_026,          Group,   Ot::NoOperand,                          Ot::NoOperand),
    // Group
    inst!( 0x80,  1, 0b0110100000000000, 0x00c, mc_00c,  ADD   , ADD  ,   Ot::ModRM8,                             Ot::Immediate8),
    inst!( 0x80,  1, 0b0110100000000000, 0x00c, mc_00c,  OR    , OR   ,   Ot::ModRM8,                             Ot::Immediate8),
    inst!( 0x80,  1, 0b0110100000000000, 0x00c, mc_00c,  ADC   , ADC  ,   Ot::ModRM8,                             Ot::Immediate8),
    inst!( 0x80,  1, 0b0110100000000000, 0x00c, mc_00c,  SBB   , SBB  ,   Ot::ModRM8,                             Ot::Immediate8),
    inst!( 0x80,  1, 0b0110100000000000, 0x00c, mc_00c,  AND   , AND  ,   Ot::ModRM8,                             Ot::Immediate8),
    inst!( 0x80,  1, 0b0110100000000000, 0x00c, mc_00c,  SUB   , SUB  ,   Ot::ModRM8,                             Ot::Immediate8),
    inst!( 0x80,  1, 0b0110100000000000, 0x00c, mc_00c,  XOR   , XOR  ,   Ot::ModRM8,                             Ot::Immediate8),
    inst!( 0x80,  1, 0b0110100000000000, 0x00c, mc_00c,  CMP   , CMP  ,   Ot::ModRM8,                             Ot::Immediate8),
    // Group
    inst!( 0x81,  1, 0b0110100000000000, 0x00c, mc_00c,  ADD   , ADD  ,   Ot::ModRM16,                            Ot::Immediate16),
    inst!( 0x81,  1, 0b0110100000000000, 0x00c, mc_00c,  OR    , OR   ,   Ot::ModRM16,                            Ot::Immediate16),
    inst!( 0x81,  1, 0b0110100000000000, 0x00c, mc_00c,  ADC   , ADC  ,   Ot::ModRM16,                            Ot::Immediate16),
    inst!( 0x81,  1, 0b0110100000000000, 0x00c, mc_00c,  SBB   , SBB  ,   Ot::ModRM16,                            Ot::Immediate16),
    inst!( 0x81,  1, 0b0110100000000000, 0x00c, mc_00c,  AND   , AND  ,   Ot::ModRM16,                            Ot::Immediate16),
    inst!( 0x81,  1, 0b0110100000000000, 0x00c, mc_00c,  SUB   , SUB  ,   Ot::ModRM16,                            Ot::Immediate16),
    inst!( 0x81,  1, 0b0110100000000000, 0x00c, mc_00c,  XOR   , XOR  ,   Ot::ModRM16,                            Ot::Immediate16),
    inst!( 0x81,  1, 0b0110100000000000, 0x00c, mc_00c,  CMP   , CMP  ,   Ot::ModRM16,                            Ot::Immediate16),
    // Group
    inst!( 0x82,  1, 0b0110100000000000, 0x00c, mc_00c,  ADD   , ADD  ,   Ot::ModRM8,                             Ot::Immediate8),
    inst!( 0x82,  1, 0b0110100000000000, 0x00c, mc_00c,  OR    , OR   ,   Ot::ModRM8,                             Ot::Immediate8),
    inst!( 0x82,  1, 0b0110100000000000, 0x00c, mc_00c,  ADC   , ADC  ,   Ot::ModRM8,                             Ot::Immediate8),
    inst!( 0x82,  1, 0b0110100000000000, 0x00c, mc_00c,  SBB   , SBB  ,   Ot::ModRM8,                             Ot::Immediate8),
    inst!( 0x82,  1, 0b0110100000000000, 0x00c, mc_00c,  AND   , AND  ,   Ot::ModRM8,                             Ot::Immediate8),
    inst!( 0x82,  1, 0b0110100000000000, 0x00c, mc_00c,  SUB   , SUB  ,   Ot::ModRM8,                             Ot::Immediate8),
    inst!( 0x82,  1, 0b0110100000000000, 0x00c, mc_00c,  XOR   , XOR  ,   Ot::ModRM8,                             Ot::Immediate8),
    inst!( 0x82,  1, 0b0110100000000000, 0x00c, mc_00c,  CMP   , CMP  ,   Ot::ModRM8,                             Ot::Immediate8),
    // Group
    inst!( 0x83,  1, 0b0110100000000000, 0x00c, mc_00c,  ADD   , ADD  ,   Ot::ModRM16,                            Ot::Immediate8SignExtended),
    inst!( 0x83,  1, 0b0110100000000000, 0x00c, mc_00c,  OR    , OR   ,   Ot::ModRM16,                            Ot::Immediate8SignExtended),
    inst!( 0x83,  1, 0b0110100000000000, 0x00c, mc_00c,  ADC   , ADC  ,   Ot::ModRM16,                            Ot::Immediate8SignExtended),
    inst!( 0x83,  1, 0b0110100000000000, 0x00c, mc_00c,  SBB   , SBB  ,   Ot::ModRM16,                            Ot::Immediate8SignExtended),
    inst!( 0x83,  1, 0b0110100000000000, 0x00c, mc_00c,  AND   , AND  ,   Ot::ModRM16,                            Ot::Immediate8SignExtended),
    inst!( 0x83,  1, 0b0110100000000000, 0x00c, mc_00c,  SUB   , SUB  ,   Ot::ModRM16,                            Ot::Immediate8SignExtended),
    inst!( 0x83,  1, 0b0110100000000000, 0x00c, mc_00c,  XOR   , XOR  ,   Ot::ModRM16,                            Ot::Immediate8SignExtended),
    inst!( 0x83,  1, 0b0110100000000000, 0x00c, mc_00c,  CMP   , CMP  ,   Ot::ModRM16,                            Ot::Immediate8SignExtended),
    // Group
    inst!( 0xD0,  2, 0b0100100000000000, 0x088, mc_088,  ROL   , ROL  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xD0,  2, 0b0100100000000000, 0x088, mc_088,  ROR   , ROR  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xD0,  2, 0b0100100000000000, 0x088, mc_088,  RCL   , RCL  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xD0,  2, 0b0100100000000000, 0x088, mc_088,  RCR   , RCR  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xD0,  2, 0b0100100000000000, 0x088, mc_088,  SHL   , SHL  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xD0,  2, 0b0100100000000000, 0x088, mc_088,  SHR   , SHR  ,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xD0,  2, 0b0100100000000000, 0x088, mc_088,  SETMO , SETMO,   Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xD0,  2, 0b0100100000000000, 0x088, mc_088,  SAR   , SAR  ,   Ot::ModRM8,                             Ot::NoOperand),
    // Group
    inst!( 0xD1,  2, 0b0100100000000000, 0x088, mc_088,  ROL   , ROL  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xD1,  2, 0b0100100000000000, 0x088, mc_088,  ROR   , ROR  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xD1,  2, 0b0100100000000000, 0x088, mc_088,  RCL   , RCL  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xD1,  2, 0b0100100000000000, 0x088, mc_088,  RCR   , RCR  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xD1,  2, 0b0100100000000000, 0x088, mc_088,  SHL   , SHL  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xD1,  2, 0b0100100000000000, 0x088, mc_088,  SHR   , SHR  ,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xD1,  2, 0b0100100000000000, 0x088, mc_088,  SETMO , SETMO,   Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xD1,  2, 0b0100100000000000, 0x088, mc_088,  SAR   , SAR  ,   Ot::ModRM16,                            Ot::NoOperand),
    // Group
    inst!( 0xD2,  3, 0b0100100000000000, 0x08c, mc_08c,  ROL   , ROL   ,  Ot::ModRM8,                             Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD2,  3, 0b0100100000000000, 0x08c, mc_08c,  ROR   , ROR   ,  Ot::ModRM8,                             Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD2,  3, 0b0100100000000000, 0x08c, mc_08c,  RCL   , RCL   ,  Ot::ModRM8,                             Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD2,  3, 0b0100100000000000, 0x08c, mc_08c,  RCR   , RCR   ,  Ot::ModRM8,                             Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD2,  3, 0b0100100000000000, 0x08c, mc_08c,  SHL   , SHL   ,  Ot::ModRM8,                             Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD2,  3, 0b0100100000000000, 0x08c, mc_08c,  SHR   , SHR   ,  Ot::ModRM8,                             Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD2,  3, 0b0100100000000000, 0x08c, mc_08c,  SETMOC, SETMOC,  Ot::ModRM8,                             Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD2,  3, 0b0100100000000000, 0x08c, mc_08c,  SAR   , SAR   ,  Ot::ModRM8,                             Ot::FixedRegister8(Register8::CL)),
    // Group
    inst!( 0xD3,  3, 0b0100100000000000, 0x08c, mc_08c,  ROL   , ROL   ,  Ot::ModRM16,                            Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD3,  3, 0b0100100000000000, 0x08c, mc_08c,  ROR   , ROR   ,  Ot::ModRM16,                            Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD3,  3, 0b0100100000000000, 0x08c, mc_08c,  RCL   , RCL   ,  Ot::ModRM16,                            Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD3,  3, 0b0100100000000000, 0x08c, mc_08c,  RCR   , RCR   ,  Ot::ModRM16,                            Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD3,  3, 0b0100100000000000, 0x08c, mc_08c,  SHL   , SHL   ,  Ot::ModRM16,                            Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD3,  3, 0b0100100000000000, 0x08c, mc_08c,  SHR   , SHR   ,  Ot::ModRM16,                            Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD3,  3, 0b0100100000000000, 0x08c, mc_08c,  SETMOC, SETMOC,  Ot::ModRM16,                            Ot::FixedRegister8(Register8::CL)),
    inst!( 0xD3,  3, 0b0100100000000000, 0x08c, mc_08c,  SAR   , SAR   ,  Ot::ModRM16,                            Ot::FixedRegister8(Register8::CL)),
    // Group
    inst!( 0xF6,  4, 0b0100100000100100, 0x098, mc_098,          TEST  ,  Ot::ModRM8,                             Ot::Immediate8),
    inst!( 0xF6,  4, 0b0100100000100100, 0x098, mc_098,          TEST  ,  Ot::ModRM8,                             Ot::Immediate8),
    inst!( 0xF6,  4, 0b0100100000100100, 0x098, mc_04c,  NOT   , NOT   ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xF6,  4, 0b0100100000100100, 0x098, mc_050,  NEG   , NEG   ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xF6,  4, 0b0100100000100100, 0x098, mc_150,          MUL   ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xF6,  4, 0b0100100000100100, 0x098, mc_150,          IMUL  ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xF6,  4, 0b0100100000100100, 0x098, mc_160,          DIV   ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xF6,  4, 0b0100100000100100, 0x098, mc_160,          IDIV  ,  Ot::ModRM8,                             Ot::NoOperand),
    // Group
    inst!( 0xF7,  4, 0b0100100000100100, 0x160, mc_098,          TEST  ,  Ot::ModRM16,                            Ot::Immediate16),
    inst!( 0xF7,  4, 0b0100100000100100, 0x160, mc_098,          TEST  ,  Ot::ModRM16,                            Ot::Immediate16),
    inst!( 0xF7,  4, 0b0100100000100100, 0x160, mc_04c,  NOT   , NOT   ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xF7,  4, 0b0100100000100100, 0x160, mc_050,  NEG   , NEG   ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xF7,  4, 0b0100100000100100, 0x160, mc_158,          MUL   ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xF7,  4, 0b0100100000100100, 0x160, mc_158,          IMUL  ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xF7,  4, 0b0100100000100100, 0x160, mc_168,          DIV   ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xF7,  4, 0b0100100000100100, 0x160, mc_168,          IDIV  ,  Ot::ModRM16,                            Ot::NoOperand),
    // Group
    inst!( 0xFE,  5, 0b0000100000100100, 0x020, mc_020,  INC   , INC   ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xFE,  5, 0b0000100000100100, 0x020, mc_020,  DEC   , DEC   ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xFE,  5, 0b0000100000100100, 0x020, mc_074,          CALL  ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xFE,  5, 0b0000100000100100, 0x020, mc_068,          CALLF ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xFE,  5, 0b0000100000100100, 0x020, mc_0d8,          JMP   ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xFE,  5, 0b0000100000100100, 0x020, mc_0dc,          JMPF  ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xFE,  5, 0b0000100000100100, 0x020, mc_026,          PUSH  ,  Ot::ModRM8,                             Ot::NoOperand),
    inst!( 0xFE,  5, 0b0000100000100100, 0x020, mc_026,          PUSH  ,  Ot::ModRM8,                             Ot::NoOperand),
    // Group
    inst!( 0xFF,  5, 0b0000100000100100, 0x026, mc_020,  INC   , INC   ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xFF,  5, 0b0000100000100100, 0x026, mc_020,  DEC   , DEC   ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xFF,  5, 0b0000100000100100, 0x026, mc_074,          CALL  ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xFF,  5, 0b0000100000100100, 0x026, mc_068,          CALLF ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xFF,  5, 0b0000100000100100, 0x026, mc_0d8,          JMP   ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xFF,  5, 0b0000100000100100, 0x026, mc_0dc,          JMPF  ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xFF,  5, 0b0000100000100100, 0x026, mc_026,          PUSH  ,  Ot::ModRM16,                            Ot::NoOperand),
    inst!( 0xFF,  5, 0b0000100000100100, 0x026, mc_026,          PUSH  ,  Ot::ModRM16,                            Ot::NoOperand),
];

impl Intel808x {
    #[rustfmt::skip]
    pub fn decode(bytes: &mut impl ByteQueue, peek: bool) -> Result<Instruction, Box<dyn std::error::Error>> {
        // Read an initial byte as our opcode or first prefix
        let mut opcode = bytes.q_read_u8(QueueType::First, QueueReader::Biu);
        let mut size: u32 = 1;
        let mut op_prefixes: u32 = 0;
        let mut op_segment_override = None;
        let mut decode_idx: usize;

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

        // Lookup the opcode in the decode table
        decode_idx = opcode as usize;
        let mut op_lu = &DECODE[decode_idx];
    
        // Prepare to read ModRm
        let mut modrm= ModRmByte::default_ref();
        let mut loaded_modrm = false;
        let mut displacement = Displacement::NoDisp;

        // Check if resolved first opcode is a group instruction
        if op_lu.grp != 0 {
            // All group instructions have a ModRm. Load it now.
            let modrm_len;
            (modrm, displacement, modrm_len) = ModRmByte::read(bytes);
            size += modrm_len;
            loaded_modrm = true;

            // Perform secondary lookup of opcode group + extension.
            decode_idx = 256 + ((op_lu.grp as usize - 1) * 8) + modrm.op_extension() as usize;
            op_lu = &DECODE[decode_idx];
        }

        // Load the ModRm byte if required, and we didn't already load it to decode a group.
        if op_lu.gdr.has_modrm() && !loaded_modrm {
            let modrm_len;
            (modrm, displacement, modrm_len) = ModRmByte::read(bytes);
            size += modrm_len;
        }

        // Resolve operand templates into OperandTypes
        let operand1_type: OperandType = op_lu.operand1.resolve_operand(bytes, modrm, displacement, &mut size, peek);
        let operand2_type: OperandType = op_lu.operand2.resolve_operand(bytes, modrm, displacement, &mut size, peek);

        Ok(Instruction {
            decode_idx,
            opcode,
            prefixes: op_prefixes,
            address: 0,
            size,
            width: op_lu.gdr.width(opcode),
            mnemonic: op_lu.mnemonic,
            xi: op_lu.xi,
            segment_override: op_segment_override,
            operand1_type,
            operand2_type,
        })
    }
}

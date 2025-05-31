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

    cpu_common::builder.rs

    Implements the common Instruction type.

*/

use std::{
    fmt,
    fmt::{Display, Formatter, Result as fmtResult},
};

use super::{addressing::WithPlusSign, mnemonic::mnemonic_to_str};
use crate::{
    cpu_common::{
        alu::Xi,
        operands::OperandSize,
        AddressingMode,
        Mnemonic,
        OperandType,
        Register16,
        Register8,
        Segment,
        OPCODE_PREFIX_0F,
        OPCODE_PREFIX_LOCK,
        OPCODE_PREFIX_REP1,
        OPCODE_PREFIX_REP2,
        OPCODE_PREFIX_REP3,
        OPCODE_PREFIX_REP4,
    },
    syntax_token::{SyntaxFormatType, SyntaxToken, SyntaxTokenVec, SyntaxTokenize},
};

#[derive(Copy, Clone)]
pub enum OperandSelect {
    FirstOperand,
    SecondOperand,
}

#[derive(Clone, Debug, PartialEq)]
pub enum InstructionWidth {
    Byte,
    Word,
}

impl InstructionWidth {
    #[inline(always)]
    pub fn sign_mask(&self) -> u16 {
        match self {
            InstructionWidth::Byte => 0x80,
            InstructionWidth::Word => 0x8000,
        }
    }
}

impl From<InstructionWidth> for OperandSize {
    fn from(iw: InstructionWidth) -> Self {
        match iw {
            InstructionWidth::Byte => OperandSize::Operand8,
            InstructionWidth::Word => OperandSize::Operand16,
        }
    }
}
impl From<&InstructionWidth> for OperandSize {
    fn from(iw: &InstructionWidth) -> Self {
        match iw {
            InstructionWidth::Byte => OperandSize::Operand8,
            InstructionWidth::Word => OperandSize::Operand16,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Instruction {
    pub decode_idx: usize,
    pub opcode: u8,
    pub prefixes: u32,
    pub address: u32,
    pub size: u32,
    pub width: InstructionWidth,
    pub mnemonic: Mnemonic,
    pub xi: Option<Xi>,
    pub segment_override: Option<Segment>,
    pub operand1_type: OperandType,
    pub operand2_type: OperandType,
}

impl Default for Instruction {
    fn default() -> Self {
        Self {
            decode_idx: 0,
            opcode: 0,
            prefixes: 0,
            address: 0,
            size: 1,
            width: InstructionWidth::Byte,
            mnemonic: Mnemonic::NOP,
            xi: None,
            segment_override: None,
            operand1_type: OperandType::NoOperand,
            operand2_type: OperandType::NoOperand,
        }
    }
}

impl Display for Instruction {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmtResult {
        let mut instruction_string = String::new();

        // Stick segment override prefix on certain opcodes (string ops)
        let sego_prefix = override_prefix_to_string(self);
        if let Some(so) = sego_prefix {
            instruction_string.push_str(&so);
            instruction_string.push(' ');
        }

        // Add other prefixes (rep(x), lock, etc)
        let prefix = prefix_to_string(self);
        let mnemonic = mnemonic_to_str(self.mnemonic).to_string().to_lowercase();

        if let Some(p) = prefix {
            instruction_string.push_str(&p);
            instruction_string.push(' ');
        }
        instruction_string.push_str(&mnemonic);

        // Size overrides. Certain instructions have byte operands with no apparent indication
        // in the GDR. We override the operand size here to avoid unwanted sign-extension.
        use Mnemonic::*;
        if matches!(self.mnemonic, NOP) {
            // NOP is a special case. It has no operands, so we don't need to do anything.
            return write!(f, "{instruction_string}");
        }
        let op_size = match self.mnemonic {
            IN | OUT | ENTER | INT | AAD => OperandSize::Operand8,
            _ => OperandSize::from(&self.width),
        };

        let op1 = operand_to_string(self, OperandSelect::FirstOperand, op_size);
        if !op1.is_empty() {
            instruction_string.push(' ');
            instruction_string.push_str(&op1);
        }

        let op2: String = operand_to_string(self, OperandSelect::SecondOperand, op_size);
        if !op2.is_empty() {
            instruction_string.push_str(", ");
            instruction_string.push_str(&op2);
        }

        write!(f, "{}", instruction_string)
    }
}

impl SyntaxTokenize for Instruction {
    fn tokenize(&self) -> Vec<SyntaxToken> {
        // Size overrides. Certain instructions have byte operands with no apparent indication
        // in the GDR. We override the operand size here to avoid unwanted sign-extension.
        use Mnemonic::*;
        let op_size = match self.mnemonic {
            IN | OUT | ENTER | INT | AAD => OperandSize::Operand8,
            _ => OperandSize::from(&self.width),
        };

        let mut i_vec = SyntaxTokenVec(Vec::new());

        // Stick segment override prefix on certain opcodes (string ops)
        let sego_prefix = override_prefix_to_string(self);
        if let Some(so) = sego_prefix {
            i_vec.0.push(SyntaxToken::Prefix(so));
            i_vec.0.push(SyntaxToken::Formatter(SyntaxFormatType::Space));
        }

        let prefix = prefix_to_string(self);
        if let Some(p) = prefix {
            i_vec.0.push(SyntaxToken::Prefix(p));
            i_vec.0.push(SyntaxToken::Formatter(SyntaxFormatType::Space));
        }

        let mnemonic = mnemonic_to_str(self.mnemonic).to_string().to_lowercase();
        i_vec.0.push(SyntaxToken::Mnemonic(mnemonic));

        if matches!(self.mnemonic, Mnemonic::NOP) {
            // NOP is a special case. It has no operands, so we don't need to do anything.
            return i_vec.0;
        }

        let op1_vec = tokenize_operand(self, OperandSelect::FirstOperand, op_size);
        i_vec.append(op1_vec, Some(SyntaxToken::Formatter(SyntaxFormatType::Space)), None);

        let op2_vec = tokenize_operand(self, OperandSelect::SecondOperand, op_size);

        if !op2_vec.is_empty() {
            i_vec.0.push(SyntaxToken::Comma);
            i_vec.append(op2_vec, Some(SyntaxToken::Formatter(SyntaxFormatType::Space)), None);
        }

        i_vec.0
    }
}

struct Imm8Extend(u8);
struct Imm8sExtend(i8);
struct Rel8Extend(i8);

impl fmt::UpperHex for Imm8sExtend {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let prefix = if f.alternate() { "0x" } else { "" };
        // Check if the value is negative (the top bit of an i8 is set)
        if self.0 < 0 {
            // If it's negative, sign-extend with FF
            let bare_hex = format!("FF{:2X}", self.0 as u8);
            f.pad_integral(true, prefix, &bare_hex)
        }
        else {
            // If it's positive or zero, simply show the original byte
            let bare_hex = format!("{:X}", self.0 as u8);
            f.pad_integral(true, prefix, &bare_hex)
        }
    }
}

impl fmt::UpperHex for Imm8Extend {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let prefix = if f.alternate() { "0x" } else { "" };
        // Check if the highest bit (bit 7) is set
        if self.0 & 0x80 != 0 {
            // If it's set, sign-extend with FF
            let bare_hex = format!("FF{:02X}", self.0);
            f.pad_integral(true, prefix, &bare_hex)
        }
        else {
            // If it's not set, simply show the original byte
            let bare_hex = format!("{:02X}", self.0);
            f.pad_integral(true, prefix, &bare_hex)
        }
    }
}

impl fmt::UpperHex for Rel8Extend {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let prefix = if f.alternate() { "0x" } else { "" };
        // Check if the value is negative (the top bit of an i8 is set)
        if self.0 < 0 {
            // If it's negative, sign-extend with FF
            let bare_hex = format!("FF{:02X}", self.0 as u8);
            f.pad_integral(true, prefix, &bare_hex)
        }
        else {
            // If it's positive or zero, simply show the original byte
            let bare_hex = format!("00{:02X}", self.0 as u8);
            f.pad_integral(true, prefix, &bare_hex)
        }
    }
}

fn operand_to_string(i: &Instruction, op: OperandSelect, lvalue: OperandSize) -> String {
    let op_type = match op {
        OperandSelect::FirstOperand => i.operand1_type,
        OperandSelect::SecondOperand => i.operand2_type,
    };

    let instruction_string: String = match op_type {
        OperandType::Immediate8(imm8) => {
            if let OperandSize::Operand8 = lvalue {
                format!("{:X}h", imm8)
            }
            else {
                format!("{:X}h", Imm8Extend(imm8))
            }
        }
        OperandType::Immediate8s(imm8) => {
            // imm8 is always sign-extended to 16
            format!("{:X}h", Imm8sExtend(imm8))
        }
        OperandType::Immediate16(imm16) => {
            format!("{:X}h", imm16)
        }
        OperandType::Relative8(rel8) => {
            //format!("short {:04X}h", i.size as i16 + rel8 as i16)
            format!("{:04X}h", i.size as i16 + rel8 as i16)
        }
        OperandType::Relative16(rel16) => {
            //format!("short {:04X}h", i.size as i16 + rel16)
            format!("{:04X}h", i.size as i16 + rel16)
        }
        OperandType::Offset8(offset8) => {
            let segment: String = match i.segment_override {
                Some(Segment::ES) => "es".to_string(),
                Some(Segment::CS) => "cs".to_string(),
                Some(Segment::SS) => "ss".to_string(),
                _ => "ds".to_string(),
            };
            format!("byte [{}:{:X}h]", segment, offset8)
        }
        OperandType::Offset16(offset16) => {
            let segment: String = match i.segment_override {
                Some(Segment::ES) => "es".to_string(),
                Some(Segment::CS) => "cs".to_string(),
                Some(Segment::SS) => "ss".to_string(),
                _ => "ds".to_string(),
            };
            format!("word [{}:{:X}h]", segment, offset16)
        }
        OperandType::Register8(reg8) => match reg8 {
            Register8::AL => "al".to_string(),
            Register8::CL => "cl".to_string(),
            Register8::DL => "dl".to_string(),
            Register8::BL => "bl".to_string(),
            Register8::AH => "ah".to_string(),
            Register8::CH => "ch".to_string(),
            Register8::DH => "dh".to_string(),
            Register8::BH => "bh".to_string(),
        },
        OperandType::Register16(reg16) => match reg16 {
            Register16::AX => "ax".to_string(),
            Register16::CX => "cx".to_string(),
            Register16::DX => "dx".to_string(),
            Register16::BX => "bx".to_string(),
            Register16::SP => "sp".to_string(),
            Register16::BP => "bp".to_string(),
            Register16::SI => "si".to_string(),
            Register16::DI => "di".to_string(),
            Register16::ES => "es".to_string(),
            Register16::CS => "cs".to_string(),
            Register16::SS => "ss".to_string(),
            Register16::DS => "ds".to_string(),
            _ => "".to_string(),
        },
        OperandType::AddressingMode(addr_mode, size) => {
            let mut ptr_prefix: String = match size {
                OperandSize::Operand8 => "byte ".to_string(),
                OperandSize::Operand16 => "word ".to_string(),
                OperandSize::NoOperand => "*invalid ptr* ".to_string(),
                OperandSize::NoSize => "".to_string(),
            };
            // LEA uses addressing calculations but isn't actually a pointer
            if let Mnemonic::LEA = i.mnemonic {
                ptr_prefix = "".to_string()
            }
            // LES and LDS point to a DWORD address
            if let Mnemonic::LES | Mnemonic::LDS = i.mnemonic {
                ptr_prefix = "dword ".to_string()
            }

            let mut segment1 = "ds".to_string();
            let mut segment2 = "ss".to_string();

            // Handle segment override prefixes
            match i.segment_override {
                Some(Segment::ES) => {
                    segment1 = "es".to_string();
                    segment2 = "es".to_string();
                }
                Some(Segment::CS) => {
                    segment1 = "cs".to_string();
                    segment2 = "cs".to_string();
                }
                Some(Segment::SS) => {
                    segment1 = "ss".to_string();
                    segment2 = "ss".to_string();
                }
                Some(Segment::DS) => {
                    segment1 = "ds".to_string();
                    segment2 = "ds".to_string();
                }
                _ => {}
            }

            match addr_mode {
                AddressingMode::BxSi => format!("{}[{}:bx+si]", ptr_prefix, segment1),
                AddressingMode::BxDi => format!("{}[{}:bx+di]", ptr_prefix, segment1),
                AddressingMode::BpSi => format!("{}[{}:bp+si]", ptr_prefix, segment2),
                AddressingMode::BpDi => format!("{}[{}:bp+di]", ptr_prefix, segment2),
                AddressingMode::Si => format!("{}[{}:si]", ptr_prefix, segment1),
                AddressingMode::Di => format!("{}[{}:di]", ptr_prefix, segment1),
                AddressingMode::Disp16(disp) => format!("{}[{}:{}]", ptr_prefix, segment1, disp),
                AddressingMode::Bx => format!("{}[{}:bx]", ptr_prefix, segment1),
                AddressingMode::BxSiDisp8(disp) => {
                    format!("{}[{}:bx+si{}]", ptr_prefix, segment1, WithPlusSign(disp))
                }
                AddressingMode::BxDiDisp8(disp) => {
                    format!("{}[{}:bx+di{}]", ptr_prefix, segment1, WithPlusSign(disp))
                }
                AddressingMode::BpSiDisp8(disp) => {
                    format!("{}[{}:bp+si{}]", ptr_prefix, segment2, WithPlusSign(disp))
                }
                AddressingMode::BpDiDisp8(disp) => {
                    format!("{}[{}:bp+di{}]", ptr_prefix, segment2, WithPlusSign(disp))
                }
                AddressingMode::SiDisp8(disp) => {
                    format!("{}[{}:si{}]", ptr_prefix, segment1, WithPlusSign(disp))
                }
                AddressingMode::DiDisp8(disp) => {
                    format!("{}[{}:di{}]", ptr_prefix, segment1, WithPlusSign(disp))
                }
                AddressingMode::BpDisp8(disp) => {
                    format!("{}[{}:bp{}]", ptr_prefix, segment2, WithPlusSign(disp))
                }
                AddressingMode::BxDisp8(disp) => {
                    format!("{}[{}:bx{}]", ptr_prefix, segment1, WithPlusSign(disp))
                }
                AddressingMode::BxSiDisp16(disp) => {
                    format!("{}[{}:bx+si{}]", ptr_prefix, segment1, WithPlusSign(disp))
                }
                AddressingMode::BxDiDisp16(disp) => {
                    format!("{}[{}:bx+di{}]", ptr_prefix, segment1, WithPlusSign(disp))
                }
                AddressingMode::BpSiDisp16(disp) => {
                    format!("{}[{}:bp+si{}]", ptr_prefix, segment2, WithPlusSign(disp))
                }
                AddressingMode::BpDiDisp16(disp) => {
                    format!("{}[{}:bp+di{}]", ptr_prefix, segment2, WithPlusSign(disp))
                }
                AddressingMode::SiDisp16(disp) => {
                    format!("{}[{}:si{}]", ptr_prefix, segment1, WithPlusSign(disp))
                }
                AddressingMode::DiDisp16(disp) => {
                    format!("{}[{}:di{}]", ptr_prefix, segment1, WithPlusSign(disp))
                }
                AddressingMode::BpDisp16(disp) => {
                    format!("{}[{}:bp{}]", ptr_prefix, segment2, WithPlusSign(disp))
                }
                AddressingMode::BxDisp16(disp) => {
                    format!("{}[{}:bx{}]", ptr_prefix, segment1, WithPlusSign(disp))
                }
                AddressingMode::RegisterIndirect(reg) => match reg {
                    Register16::AX => "[ax]".to_string(),
                    Register16::BX => "[bx]".to_string(),
                    Register16::CX => "[cx]".to_string(),
                    Register16::DX => "[dx]".to_string(),
                    Register16::SI => "[si]".to_string(),
                    Register16::DI => "[di]".to_string(),
                    Register16::SP => "[sp]".to_string(),
                    Register16::BP => "[bp]".to_string(),
                    Register16::ES => "[es]".to_string(),
                    Register16::CS => "[cs]".to_string(),
                    Register16::SS => "[ss]".to_string(),
                    Register16::DS => "[ds]".to_string(),
                    _ => "".to_string(),
                },
                AddressingMode::RegisterMode => "".to_string(),
            }
        }
        /*
        OperandType::NearAddress(offset) => {
            format!("[{:#06X}]", offset)
        }
        */
        OperandType::FarAddress(segment, offset) => {
            format!("{:04X}h:{:04X}h", segment, offset)
        }
        OperandType::NoOperand => "".to_string(),
        _ => "".to_string(),
    };

    instruction_string
}

fn tokenize_operand(i: &Instruction, op: OperandSelect, lvalue: OperandSize) -> Vec<SyntaxToken> {
    let op_type = match op {
        OperandSelect::FirstOperand => i.operand1_type,
        OperandSelect::SecondOperand => i.operand2_type,
    };

    let mut op_vec = Vec::new();

    match op_type {
        OperandType::Immediate8(imm8) => {
            if let OperandSize::Operand8 = lvalue {
                op_vec.push(SyntaxToken::HexValue(format!("{:X}h", imm8)));
            }
            else {
                op_vec.push(SyntaxToken::HexValue(format!("{:X}h", Imm8Extend(imm8))));
            }
        }
        OperandType::Immediate8s(imm8s) => {
            op_vec.push(SyntaxToken::HexValue(format!("{:X}h", Imm8sExtend(imm8s))));
        }
        OperandType::Immediate16(imm16) => {
            op_vec.push(SyntaxToken::HexValue(format!("{:X}h", imm16)));
        }
        OperandType::Relative8(rel8) => {
            //op_vec.push(SyntaxToken::Text("short".to_string()));
            //op_vec.push(SyntaxToken::Formatter(SyntaxFormatType::Space));
            op_vec.push(SyntaxToken::HexValue(format!("{:04X}h", i.size as i16 + rel8 as i16)));
        }
        OperandType::Relative16(rel16) => {
            //op_vec.push(SyntaxToken::Text("short".to_string()));
            //op_vec.push(SyntaxToken::Formatter(SyntaxFormatType::Space));
            op_vec.push(SyntaxToken::HexValue(format!("{:04X}h", i.size as i16 + rel16)));
        }
        OperandType::Offset8(offset8) => {
            let segment: String = match i.segment_override {
                Some(Segment::ES) => "es".to_string(),
                Some(Segment::CS) => "cs".to_string(),
                Some(Segment::SS) => "ss".to_string(),
                _ => "ds".to_string(),
            };
            op_vec.push(SyntaxToken::Text("byte".to_string()));
            op_vec.push(SyntaxToken::Formatter(SyntaxFormatType::Space));
            op_vec.push(SyntaxToken::OpenBracket);
            op_vec.push(SyntaxToken::Segment(segment));
            op_vec.push(SyntaxToken::Colon);
            op_vec.push(SyntaxToken::HexValue(format!("{:X}h", offset8)));
            op_vec.push(SyntaxToken::CloseBracket);
        }
        OperandType::Offset16(offset16) => {
            let segment: String = match i.segment_override {
                Some(Segment::ES) => "es".to_string(),
                Some(Segment::CS) => "cs".to_string(),
                Some(Segment::SS) => "ss".to_string(),
                _ => "ds".to_string(),
            };
            op_vec.push(SyntaxToken::Text("word".to_string()));
            op_vec.push(SyntaxToken::Formatter(SyntaxFormatType::Space));
            op_vec.push(SyntaxToken::OpenBracket);
            op_vec.push(SyntaxToken::Segment(segment));
            op_vec.push(SyntaxToken::Colon);
            op_vec.push(SyntaxToken::HexValue(format!("{:X}h", offset16)));
            op_vec.push(SyntaxToken::CloseBracket);
        }
        OperandType::Register8(reg8) => {
            let reg = match reg8 {
                Register8::AL => "al".to_string(),
                Register8::CL => "cl".to_string(),
                Register8::DL => "dl".to_string(),
                Register8::BL => "bl".to_string(),
                Register8::AH => "ah".to_string(),
                Register8::CH => "ch".to_string(),
                Register8::DH => "dh".to_string(),
                Register8::BH => "bh".to_string(),
            };
            op_vec.push(SyntaxToken::Register(reg));
        }
        OperandType::Register16(reg16) => {
            let reg = match reg16 {
                Register16::AX => "ax".to_string(),
                Register16::CX => "cx".to_string(),
                Register16::DX => "dx".to_string(),
                Register16::BX => "bx".to_string(),
                Register16::SP => "sp".to_string(),
                Register16::BP => "bp".to_string(),
                Register16::SI => "si".to_string(),
                Register16::DI => "di".to_string(),
                Register16::ES => "es".to_string(),
                Register16::CS => "cs".to_string(),
                Register16::SS => "ss".to_string(),
                Register16::DS => "ds".to_string(),
                _ => "".to_string(),
            };
            op_vec.push(SyntaxToken::Register(reg));
        }
        OperandType::AddressingMode(addr_mode, size) => {
            let mut ptr_prefix: Option<String> = match size {
                OperandSize::Operand8 => Some("byte".to_string()),
                OperandSize::Operand16 => Some("word".to_string()),
                OperandSize::NoOperand => Some("*invalid*".to_string()),
                OperandSize::NoSize => None,
            };
            // LEA uses addressing calculations but isn't actually a pointer
            if let Mnemonic::LEA = i.mnemonic {
                ptr_prefix = None
            }
            // LES and LDS point to a DWORD address
            if let Mnemonic::LES | Mnemonic::LDS = i.mnemonic {
                ptr_prefix = Some("dword".to_string())
            }

            // Add pointer prefix, if any
            if let Some(prefix) = ptr_prefix {
                op_vec.push(SyntaxToken::Text(prefix));
                op_vec.push(SyntaxToken::Formatter(SyntaxFormatType::Space))
            }

            let mut segment1 = "ds".to_string();
            let mut segment2 = "ss".to_string();

            // Handle segment override prefixes
            match i.segment_override {
                Some(Segment::ES) => {
                    segment1 = "es".to_string();
                    segment2 = "es".to_string();
                }
                Some(Segment::CS) => {
                    segment1 = "cs".to_string();
                    segment2 = "cs".to_string();
                }
                Some(Segment::SS) => {
                    segment1 = "ss".to_string();
                    segment2 = "ss".to_string();
                }
                Some(Segment::DS) => {
                    segment1 = "ds".to_string();
                    segment2 = "ds".to_string();
                }
                _ => {}
            }

            let segment1_token = SyntaxToken::Segment(segment1);
            let segment2_token = SyntaxToken::Segment(segment2);

            let mut have_addr_mode = true;

            let (seg_token, disp_opt, ea_vec) = match addr_mode {
                AddressingMode::BxSi => (segment1_token, None, ["bx", "si"]),
                AddressingMode::BxDi => (segment1_token, None, ["bx", "di"]),
                AddressingMode::BpSi => (segment2_token, None, ["bp", "si"]),
                AddressingMode::BpDi => (segment2_token, None, ["bp", "di"]),
                AddressingMode::Si => (segment1_token, None, ["si", ""]),
                AddressingMode::Di => (segment1_token, None, ["di", ""]),
                AddressingMode::Disp16(disp) => (segment1_token, Some(disp), ["", ""]),
                AddressingMode::Bx => (segment1_token, None, ["bx", ""]),
                AddressingMode::BxSiDisp8(disp) => (segment1_token, Some(disp), ["bx", "si"]),
                AddressingMode::BxDiDisp8(disp) => (segment1_token, Some(disp), ["bx", "di"]),
                AddressingMode::BpSiDisp8(disp) => (segment2_token, Some(disp), ["bp", "si"]),
                AddressingMode::BpDiDisp8(disp) => (segment2_token, Some(disp), ["bp", "di"]),
                AddressingMode::SiDisp8(disp) => (segment1_token, Some(disp), ["si", ""]),
                AddressingMode::DiDisp8(disp) => (segment1_token, Some(disp), ["di", ""]),
                AddressingMode::BpDisp8(disp) => (segment2_token, Some(disp), ["bp", ""]),
                AddressingMode::BxDisp8(disp) => (segment1_token, Some(disp), ["bx", ""]),
                AddressingMode::BxSiDisp16(disp) => (segment1_token, Some(disp), ["bx", "si"]),
                AddressingMode::BxDiDisp16(disp) => (segment1_token, Some(disp), ["bx", "di"]),
                AddressingMode::BpSiDisp16(disp) => (segment2_token, Some(disp), ["bp", "si"]),
                AddressingMode::BpDiDisp16(disp) => (segment2_token, Some(disp), ["bp", "di"]),
                AddressingMode::SiDisp16(disp) => (segment1_token, Some(disp), ["si", ""]),
                AddressingMode::DiDisp16(disp) => (segment1_token, Some(disp), ["di", ""]),
                AddressingMode::BpDisp16(disp) => (segment2_token, Some(disp), ["bp", ""]),
                AddressingMode::BxDisp16(disp) => (segment1_token, Some(disp), ["bx", ""]),
                AddressingMode::RegisterIndirect(reg) => {
                    have_addr_mode = false;
                    op_vec.push(SyntaxToken::OpenBracket);
                    op_vec.push(SyntaxToken::Register(reg.to_string()));
                    op_vec.push(SyntaxToken::CloseBracket);
                    (segment1_token, None, ["", ""])
                }
                _ => {
                    have_addr_mode = false;
                    (segment1_token, None, ["", ""])
                }
            };

            if have_addr_mode {
                op_vec.push(SyntaxToken::OpenBracket);
                op_vec.push(seg_token);
                op_vec.push(SyntaxToken::Colon);

                if !ea_vec[0].is_empty() {
                    // Have first component of ea
                    op_vec.push(SyntaxToken::Register(ea_vec[0].to_string()));
                }
                else if let Some(disp) = disp_opt {
                    // Displacement by itself
                    op_vec.push(SyntaxToken::Displacement(format!("{}", disp)));
                }

                if !ea_vec[1].is_empty() {
                    // Have second component of ea
                    op_vec.push(SyntaxToken::PlusSign);
                    op_vec.push(SyntaxToken::Register(ea_vec[1].to_string()));
                }

                if !ea_vec[0].is_empty() {
                    // Have at least one ea component. Add +displacement if present.
                    if let Some(disp) = disp_opt {
                        // TODO: Generate +/- as tokens for displacement?
                        op_vec.push(SyntaxToken::Displacement(format!("{}", WithPlusSign(disp))));
                    }
                }

                op_vec.push(SyntaxToken::CloseBracket);
            }
        }
        /*
        OperandType::NearAddress(offset) => {

            op_vec.push(SyntaxToken::OpenBracket);
            op_vec.push(SyntaxToken::HexValue(format!("{:04X}h", offset)));
            op_vec.push(SyntaxToken::CloseBracket);
        }
        */
        OperandType::FarAddress(segment, offset) => {
            op_vec.push(SyntaxToken::HexValue(format!("{:04X}h", segment)));
            op_vec.push(SyntaxToken::Colon);
            op_vec.push(SyntaxToken::HexValue(format!("{:04X}h", offset)));
        }
        _ => {}
    };

    op_vec
}

fn override_prefix_to_string(i: &Instruction) -> Option<String> {
    if let Some(seg_override) = i.segment_override {
        match (i.prefixes & OPCODE_PREFIX_0F != 0, i.opcode) {
            (false, 0xA4 | 0xA5 | 0xAA | 0xAB | 0xAC | 0xAD | 0xA6 | 0xA7 | 0xAE | 0xAF) => {
                let segment = match seg_override {
                    Segment::ES => "es",
                    Segment::CS => "cs",
                    Segment::SS => "ss",
                    Segment::DS => "ds",
                    Segment::None => return None,
                };
                Some(segment.to_string())
            }
            (true, 0x20 | 0x22 | 0x26 | 0x33 | 0x3B) => {
                let segment = match seg_override {
                    Segment::ES => "es",
                    Segment::CS => "cs",
                    Segment::SS => "ss",
                    Segment::DS => "ds",
                    Segment::None => return None,
                };
                Some(segment.to_string())
            }
            _ => None,
        }
    }
    else {
        // No override
        None
    }
}

fn prefix_to_string(i: &Instruction) -> Option<String> {
    // Handle REPx prefixes

    let mut prefix_str = String::new();

    if i.prefixes & OPCODE_PREFIX_LOCK != 0 {
        prefix_str.push_str("lock ");
    }

    if i.prefixes & OPCODE_PREFIX_REP1 != 0 {
        match i.opcode {
            0x6C | 0x6D | 0x6E | 0x6F | 0xA4 | 0xA5 | 0xAA | 0xAB | 0xAC | 0xAD => prefix_str.push_str("rep"),
            0xA6 | 0xA7 | 0xAE | 0xAF => prefix_str.push_str("repne"),
            _ => {}
        }
    }
    else if i.prefixes & OPCODE_PREFIX_REP2 != 0 {
        match i.opcode {
            0x6C | 0x6D | 0x6E | 0x6F | 0xA4 | 0xA5 | 0xAA | 0xAB | 0xAC | 0xAD => prefix_str.push_str("rep"),
            0xA6 | 0xA7 | 0xAE | 0xAF => prefix_str.push_str("repe"),
            _ => {}
        }
    }
    else if i.prefixes & OPCODE_PREFIX_REP3 != 0 {
        match i.opcode {
            0xA4 | 0xA5 | 0xAA | 0xAB | 0xAC | 0xAD => prefix_str.push_str("rep"),
            0x6C | 0x6D | 0x6E | 0x6F | 0xA6 | 0xA7 | 0xAE | 0xAF => prefix_str.push_str("repnc"),
            _ => {}
        }
    }
    else if i.prefixes & OPCODE_PREFIX_REP4 != 0 {
        match i.opcode {
            0xA4 | 0xA5 | 0xAA | 0xAB | 0xAC | 0xAD => prefix_str.push_str("rep"),
            0x6C | 0x6D | 0x6E | 0x6F | 0xA6 | 0xA7 | 0xAE | 0xAF => prefix_str.push_str("repc"),
            _ => {}
        }
    }

    if prefix_str.is_empty() {
        None
    }
    else {
        Some(prefix_str)
    }
}

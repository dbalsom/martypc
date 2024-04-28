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

    cpu_808x::display.rs

    Formatting routines for mnemonics and Instruction type.
    Converts Instructions into string or token representations.

*/

use std::fmt;

use crate::cpu_808x::{addressing::AddressingMode, mnemonic::Mnemonic, *};

use crate::syntax_token::SyntaxToken;

#[derive(Copy, Clone)]
pub enum OperandSelect {
    FirstOperand,
    SecondOperand,
}

fn mnemonic_to_str(op: Mnemonic) -> &'static str {
    match op {
        Mnemonic::NOP => "NOP",
        Mnemonic::AAA => "AAA",
        Mnemonic::AAD => "AAD",
        Mnemonic::AAM => "AAM",
        Mnemonic::AAS => "AAS",
        Mnemonic::ADC => "ADC",
        Mnemonic::ADD => "ADD",
        Mnemonic::AND => "AND",
        Mnemonic::CALL => "CALL",
        Mnemonic::CALLF => "CALLF",
        Mnemonic::CBW => "CBW",
        Mnemonic::CLC => "CLC",
        Mnemonic::CLD => "CLD",
        Mnemonic::CLI => "CLI",
        Mnemonic::CMC => "CMC",
        Mnemonic::CMP => "CMP",
        Mnemonic::CMPSB => "CMPSB",
        Mnemonic::CMPSW => "CMPSW",
        Mnemonic::CWD => "CWD",
        Mnemonic::DAA => "DAA",
        Mnemonic::DAS => "DAS",
        Mnemonic::DEC => "DEC",
        Mnemonic::DIV => "DIV",
        Mnemonic::ESC => "ESC",
        Mnemonic::FWAIT => "FWAIT",
        Mnemonic::HLT => "HLT",
        Mnemonic::IDIV => "IDIV",
        Mnemonic::IMUL => "IMUL",
        Mnemonic::IN => "IN",
        Mnemonic::INC => "INC",
        Mnemonic::INT => "INT",
        Mnemonic::INT3 => "INT3",
        Mnemonic::INTO => "INTO",
        Mnemonic::IRET => "IRET",
        Mnemonic::JB => "JB",
        Mnemonic::JBE => "JBE",
        Mnemonic::JCXZ => "JCXZ",
        Mnemonic::JL => "JL",
        Mnemonic::JLE => "JLE",
        Mnemonic::JMP => "JMP",
        Mnemonic::JMPF => "JMPF",
        Mnemonic::JNB => "JNB",
        Mnemonic::JNBE => "JNBE",
        Mnemonic::JNL => "JNL",
        Mnemonic::JNLE => "JNLE",
        Mnemonic::JNO => "JNO",
        Mnemonic::JNP => "JNP",
        Mnemonic::JNS => "JNS",
        Mnemonic::JNZ => "JNZ",
        Mnemonic::JO => "JO",
        Mnemonic::JP => "JP",
        Mnemonic::JS => "JS",
        Mnemonic::JZ => "JZ",
        Mnemonic::LAHF => "LAHF",
        Mnemonic::LDS => "LDS",
        Mnemonic::LEA => "LEA",
        Mnemonic::LES => "LES",
        Mnemonic::LOCK => "LOCK",
        Mnemonic::LODSB => "LODSB",
        Mnemonic::LODSW => "LODSW",
        Mnemonic::LOOP => "LOOP",
        Mnemonic::LOOPNE => "LOOPNE",
        Mnemonic::LOOPE => "LOOPE",
        Mnemonic::MOV => "MOV",
        Mnemonic::MOVSB => "MOVSB",
        Mnemonic::MOVSW => "MOVSW",
        Mnemonic::MUL => "MUL",
        Mnemonic::NEG => "NEG",
        Mnemonic::NOT => "NOT",
        Mnemonic::OR => "OR",
        Mnemonic::OUT => "OUT",
        Mnemonic::POP => "POP",
        Mnemonic::POPF => "POPF",
        Mnemonic::PUSH => "PUSH",
        Mnemonic::PUSHF => "PUSHF",
        Mnemonic::RCL => "RCL",
        Mnemonic::RCR => "RCR",
        Mnemonic::REP => "REP",
        Mnemonic::REPNE => "REPNE",
        Mnemonic::REPE => "REPE",
        Mnemonic::RETF => "RETF",
        Mnemonic::RETN => "RETN",
        Mnemonic::ROL => "ROL",
        Mnemonic::ROR => "ROR",
        Mnemonic::SAHF => "SAHF",
        Mnemonic::SALC => "SALC",
        Mnemonic::SAR => "SAR",
        Mnemonic::SBB => "SBB",
        Mnemonic::SCASB => "SCASB",
        Mnemonic::SCASW => "SCASW",
        Mnemonic::SETMO => "SETMO",
        Mnemonic::SETMOC => "SETMOC",
        Mnemonic::SHL => "SHL",
        Mnemonic::SHR => "SHR",
        Mnemonic::STC => "STC",
        Mnemonic::STD => "STD",
        Mnemonic::STI => "STI",
        Mnemonic::STOSB => "STOSB",
        Mnemonic::STOSW => "STOSW",
        Mnemonic::SUB => "SUB",
        Mnemonic::TEST => "TEST",
        Mnemonic::XCHG => "XCHG",
        Mnemonic::XLAT => "XLAT",
        Mnemonic::XOR => "XOR",
        _ => "INVALID",
    }
}

impl fmt::Display for Mnemonic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", mnemonic_to_str(*self))
    }
}

struct SignedHex<T>(T);

struct WithPlusSign<T>(T);
struct WithSign<T>(T);

impl fmt::Display for Displacement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Displacement::Pending8 | Displacement::Pending16 | Displacement::NoDisp => {
                write!(f, "Invalid Displacement")
            }
            Displacement::Disp8(i) => {
                write!(f, "{:X}h", i)
            }
            Displacement::Disp16(i) => {
                write!(f, "{:X}h", i)
            }
        }
    }
}

impl fmt::Display for SignedHex<Displacement> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Displacement::Pending8 | Displacement::Pending16 | Displacement::NoDisp => {
                write!(f, "Invalid Displacement")
            }
            Displacement::Disp8(i) => {
                if *i < 0 {
                    write!(f, "{:X}h", !i.wrapping_sub(1))
                }
                else {
                    write!(f, "{:X}h", i)
                }
            }
            Displacement::Disp16(i) => {
                if *i < 0 {
                    write!(f, "{:X}h", !i.wrapping_sub(1))
                }
                else {
                    write!(f, "{:X}h", i)
                }
            }
        }
    }
}

impl Display for WithPlusSign<Displacement> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Displacement::Pending8 | Displacement::Pending16 | Displacement::NoDisp => {
                write!(f, "Invalid Displacement")
            }
            Displacement::Disp8(i) => {
                if *i < 0 {
                    write!(f, "-{}", SignedHex(self.0))
                }
                else {
                    write!(f, "+{}", SignedHex(self.0))
                }
            }
            Displacement::Disp16(i) => {
                if *i < 0 {
                    write!(f, "-{}", SignedHex(self.0))
                }
                else {
                    write!(f, "+{}", SignedHex(self.0))
                }
            }
        }
    }
}

impl Display for WithSign<Displacement> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Displacement::Pending8 | Displacement::Pending16 | Displacement::NoDisp => {
                write!(f, "Invalid Displacement")
            }
            Displacement::Disp8(i) => {
                if *i < 0 {
                    write!(f, "-{}", SignedHex(self.0))
                }
                else {
                    write!(f, "{}", SignedHex(self.0))
                }
            }
            Displacement::Disp16(i) => {
                if *i < 0 {
                    write!(f, "-{}", SignedHex(self.0))
                }
                else {
                    write!(f, "{}", SignedHex(self.0))
                }
            }
        }
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut instruction_string = String::new();

        // Stick segment override prefix on certain opcodes (string ops)
        let sego_prefix = override_prefix_to_string(self);
        if let Some(so) = sego_prefix {
            instruction_string.push_str(&so);
            instruction_string.push_str(" ");
        }

        // Add other prefixes (rep(x), lock, etc)
        let prefix = prefix_to_string(self);
        let mnemonic = mnemonic_to_str(self.mnemonic).to_string().to_lowercase();

        if let Some(p) = prefix {
            instruction_string.push_str(&p);
            instruction_string.push_str(" ");
        }

        instruction_string.push_str(&mnemonic);

        // Dont sign-extend 8-bit port addresses.
        let op_size = match self.mnemonic {
            Mnemonic::IN | Mnemonic::OUT => OperandSize::Operand8,
            _ => self.operand1_size,
        };

        let op1 = operand_to_string(self, OperandSelect::FirstOperand, op_size);
        if op1.len() > 0 {
            instruction_string.push_str(" ");
            instruction_string.push_str(&op1);
        }

        let op2: String = operand_to_string(self, OperandSelect::SecondOperand, op_size);
        if op2.len() > 0 {
            instruction_string.push_str(", ");
            instruction_string.push_str(&op2);
        }

        write!(f, "{}", instruction_string)
    }
}

impl Cpu {
    pub fn tokenize_instruction(i: &Instruction) -> Vec<SyntaxToken> {
        // Dont sign-extend 8-bit port addresses.
        let op_size = match i.mnemonic {
            Mnemonic::IN | Mnemonic::OUT => OperandSize::Operand8,
            _ => i.operand1_size,
        };

        let mut i_vec = SyntaxTokenVec(Vec::new());

        // Stick segment override prefix on certain opcodes (string ops)
        let sego_prefix = override_prefix_to_string(i);
        if let Some(so) = sego_prefix {
            i_vec.0.push(SyntaxToken::Prefix(so));
            i_vec.0.push(SyntaxToken::Formatter(SyntaxFormatType::Space));
        }

        let prefix = prefix_to_string(i);
        if let Some(p) = prefix {
            i_vec.0.push(SyntaxToken::Prefix(p));
            i_vec.0.push(SyntaxToken::Formatter(SyntaxFormatType::Space));
        }

        let mnemonic = mnemonic_to_str(i.mnemonic).to_string().to_lowercase();
        i_vec.0.push(SyntaxToken::Mnemonic(mnemonic));

        let op1_vec = tokenize_operand(i, OperandSelect::FirstOperand, op_size);
        i_vec.append(op1_vec, Some(SyntaxToken::Formatter(SyntaxFormatType::Space)), None);

        let op2_vec = tokenize_operand(i, OperandSelect::SecondOperand, op_size);

        if !op2_vec.is_empty() {
            i_vec.0.push(SyntaxToken::Comma);
            i_vec.append(op2_vec, Some(SyntaxToken::Formatter(SyntaxFormatType::Space)), None);
        }

        i_vec.0
    }
}

impl SyntaxTokenize for Instruction {
    fn tokenize(&self) -> Vec<SyntaxToken> {
        Cpu::tokenize_instruction(self)
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
    let (op_type, op_size) = match op {
        OperandSelect::FirstOperand => (i.operand1_type, i.operand1_size),
        OperandSelect::SecondOperand => (i.operand2_type, i.operand2_size),
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
        OperandType::AddressingMode(addr_mode) => {
            let mut ptr_prefix: String = match op_size {
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
    let (op_type, op_size) = match op {
        OperandSelect::FirstOperand => (i.operand1_type, i.operand1_size),
        OperandSelect::SecondOperand => (i.operand2_type, i.operand2_size),
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
        OperandType::AddressingMode(addr_mode) => {
            let mut ptr_prefix: Option<String> = match op_size {
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
                AddressingMode::RegisterMode => {
                    have_addr_mode = false;
                    (segment1_token, None, ["", ""])
                }
            };

            if have_addr_mode {
                op_vec.push(SyntaxToken::OpenBracket);
                op_vec.push(seg_token);
                op_vec.push(SyntaxToken::Colon);

                if ea_vec[0].len() > 0 {
                    // Have first component of ea
                    op_vec.push(SyntaxToken::Register(ea_vec[0].to_string()));
                }
                else if let Some(disp) = disp_opt {
                    // Displacement by itself
                    op_vec.push(SyntaxToken::Displacement(format!("{}", disp)));
                }

                if ea_vec[1].len() > 0 {
                    // Have second component of ea
                    op_vec.push(SyntaxToken::PlusSign);
                    op_vec.push(SyntaxToken::Register(ea_vec[1].to_string()));
                }

                if ea_vec[0].len() > 0 {
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
    if i.segment_override.is_some() {
        match i.opcode {
            0xA4 | 0xA5 | 0xAA | 0xAB | 0xAC | 0xAD | 0xA6 | 0xA7 | 0xAE | 0xAF => {
                let segment: String = match i.segment_override {
                    Some(Segment::ES) => "es".to_string(),
                    Some(Segment::CS) => "cs".to_string(),
                    Some(Segment::SS) => "ss".to_string(),
                    _ => "ds".to_string(),
                };
                Some(segment)
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
    // TODO: IS F2 valid on 6C, 6D, etc?

    if i.prefixes & OPCODE_PREFIX_LOCK != 0 {
        Some("lock".to_string())
    }
    else if i.prefixes & OPCODE_PREFIX_REP1 != 0 {
        match i.opcode {
            0xF6 | 0xF7 => None, // Don't show REP prefix on div.
            0xA4 | 0xA5 | 0xAA | 0xAB | 0xAC | 0xAD => Some("rep".to_string()),
            0xA6 | 0xA7 | 0xAE | 0xAF => Some("repne".to_string()),
            _ => None,
        }
    }
    else if i.prefixes & OPCODE_PREFIX_REP2 != 0 {
        match i.opcode {
            0xF6 | 0xF7 => None, // Don't show REP prefix on div.
            0xA4 | 0xA5 | 0xAA | 0xAB | 0xAC | 0xAD => Some("rep".to_string()),
            0xA6 | 0xA7 | 0xAE | 0xAF => Some("repe".to_string()),
            _ => None,
        }
    }
    else {
        None
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "cpu_validator")]
    use crate::cpu_validator;

    use crate::{cpu_808x::*, syntax_token::*};

    #[test]
    fn test_display_methods_match() {
        let test_ct = 1_000_000;

        #[cfg(feature = "cpu_validator")]
        use cpu_validator::ValidatorMode;

        let mut cpu = Cpu::new(
            CpuType::Intel8088,
            TraceMode::None,
            TraceLogger::None,
            #[cfg(feature = "cpu_validator")]
            ValidatorType::None,
            #[cfg(feature = "cpu_validator")]
            TraceLogger::None,
            #[cfg(feature = "cpu_validator")]
            ValidatorMode::Instruction,
            #[cfg(feature = "cpu_validator")]
            1_000_000,
        );

        cpu.randomize_seed(1234);
        cpu.randomize_mem();

        for i in 0..test_ct {
            cpu.reset();
            cpu.randomize_regs();

            if cpu.get_register16(Register16::IP) > 0xFFF0 {
                // Avoid IP wrapping issues for now
                continue;
            }
            let opcodes: Vec<u8> = (0u8..=255u8).collect();

            let mut instruction_address =
                Cpu::calc_linear_address(cpu.get_register16(Register16::CS), cpu.get_register16(Register16::IP));

            while (cpu.get_register16(Register16::IP) > 0xFFF0) || ((instruction_address & 0xFFFFF) > 0xFFFF0) {
                // Avoid IP wrapping issues for now
                cpu.randomize_regs();
                instruction_address =
                    Cpu::calc_linear_address(cpu.get_register16(Register16::CS), cpu.get_register16(Register16::IP));
            }

            cpu.random_inst_from_opcodes(&opcodes);

            cpu.bus_mut().seek(instruction_address as usize);
            let (opcode, _cost) = cpu.bus_mut().read_u8(instruction_address as usize, 0).expect("mem err");

            let mut i = match Cpu::decode(cpu.bus_mut()) {
                Ok(i) => i,
                Err(_) => {
                    log::error!("Instruction decode error, skipping...");
                    continue;
                }
            };

            let s1 = i.to_string();
            let s2 = SyntaxTokenVec(i.tokenize()).to_string();

            if s1.to_lowercase() == s2.to_lowercase() {
                //log::debug!("Disassembly matches: {}, {}", s1, s2);
            }
            else {
                println!("Test: {} Disassembly mismatch: {}, {}", i, s1, s2);
                assert_eq!(s1, s2);
            }
        }
    }
}

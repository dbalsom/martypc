/*
    Marty PC Emulator 
    (C)2023 Daniel Balsom
    https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

    ---------------------------------------------------------------------------

    cpu_808x::display.rs

    Formatting routines for mnemonics and Instruction type. 
    Converts Instructions into string or token representations.

*/

use std::fmt;

use crate::cpu_808x::*;
use crate::cpu_808x::mnemonic::Mnemonic;
use crate::cpu_808x::addressing::AddressingMode;

use crate::syntax_token::SyntaxToken;

#[derive(Copy, Clone)]
pub enum OperandSelect {
    FirstOperand,
    SecondOperand
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

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {

        let mut instruction_string = String::new();
        
        let prefix = prefix_to_string(self);
        let mnemonic = mnemonic_to_str(self.mnemonic).to_string().to_lowercase();

        if prefix.len() > 0 {
            instruction_string.push_str(&prefix);
            instruction_string.push_str(" ");
        }
        instruction_string.push_str(&mnemonic);
        instruction_string.push_str(" ");

        let op1 = operand_to_string(self, OperandSelect::FirstOperand);
        if op1.len() > 0 {
            instruction_string.push_str(&op1);
        }

        let op2: String = operand_to_string(self, OperandSelect::SecondOperand);
        if op2.len() > 0 {
            instruction_string.push_str(", ");
            instruction_string.push_str(&op2);
        }

        write!(f, "{}", instruction_string)
     }
}

impl Cpu {
    pub fn tokenize_instruction(i: &Instruction) -> Vec<SyntaxToken> {

        let mut i_vec = Vec::new();

        let mut prefix = prefix_to_string(i);
        if prefix.len() > 0 {
            prefix.push_str(" ");
            i_vec.push(SyntaxToken::Prefix(prefix));
        }

        let mnemonic = mnemonic_to_str(i.mnemonic).to_string().to_lowercase();
        i_vec.push(SyntaxToken::Mnemonic(mnemonic));

        let mut op1_vec = tokenize_operand(i, OperandSelect::FirstOperand);
        i_vec.append(&mut op1_vec);

        let mut op2_vec = tokenize_operand(i, OperandSelect::SecondOperand);

        if op2_vec.len() > 0 {
            i_vec.push(SyntaxToken::Comma);
        }

        i_vec.append(&mut op2_vec);

        i_vec
    }
}

impl SyntaxTokenize for Instruction {
    fn tokenize(&self) -> Vec<SyntaxToken> {
        Cpu::tokenize_instruction(self)
    }
}

fn operand_to_string(i: &Instruction, op: OperandSelect) -> String {

    let (op_type, op_size) = match op {
        OperandSelect::FirstOperand => (i.operand1_type, i.operand1_size),
        OperandSelect::SecondOperand => (i.operand2_type, i.operand2_size)
    };
    
    let instruction_string: String = match op_type {
        OperandType::Immediate8(imm8) => {
            format!("{:#04X}", imm8)
        }
        OperandType::Immediate16(imm16) => {
            format!("{:#04X}",imm16)
        }
        OperandType::Relative8(rel8) => {
            //if i.flags & INSTRUCTION_REL_JUMP != 0 {
            //    // Display relative jmp label as absolute offset
            //    let display_imm = relative_offset_u32(i.address + i.size, rel8 as i32);
            //    format!("{:#06X}", display_imm)
            //}
            //else {
            //    format!("{:#06X}", rel8)
            //}
            format!("{:#04X}", rel8)
        }
        OperandType::Relative16(rel16) => {
            //if i.flags & INSTRUCTION_REL_JUMP != 0 {
            //    // Display relative jmp label as absolute offset
            //    let display_imm = relative_offset_u32(i.address + i.size, rel16 as i32);
            //    format!("{:#06X}", display_imm)
            //}
            //else {
            //    format!("{:#06X}", rel16)
            //}            
            format!("{:#06X}", rel16)
        }
        OperandType::Offset8(offset8) => {
            let segment;
            match i.segment_override {
                SegmentOverride::ES => {
                    segment = "es".to_string();
                }
                SegmentOverride::CS => {
                    segment = "cs".to_string();
                }
                SegmentOverride::SS => {
                    segment = "ss".to_string();
                }
                _ => {
                    segment = "ds".to_string();
                }
            }            
            format!("byte ptr {}:[{:#06X}]", segment, offset8)
        }
        OperandType::Offset16(offset16) => {
            let segment;
            match i.segment_override {
                SegmentOverride::ES => {
                    segment = "es".to_string();
                }
                SegmentOverride::CS => {
                    segment = "cs".to_string();
                }
                SegmentOverride::SS => {
                    segment = "ss".to_string();
                }
                _ => {
                    segment = "ds".to_string();
                }
            }                        
            format!("word ptr {}:[{:#06X}]", segment, offset16)
        }
        OperandType::Register8(reg8) => {
            match reg8 {
                Register8::AL => "al".to_string(),
                Register8::CL => "cl".to_string(),
                Register8::DL => "dl".to_string(),
                Register8::BL => "bl".to_string(),
                Register8::AH => "ah".to_string(),
                Register8::CH => "ch".to_string(),
                Register8::DH => "dh".to_string(),
                Register8::BH => "bh".to_string(),
            }
        }
        OperandType::Register16(reg16) => {
            match reg16 {
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
                _=>"".to_string(),
            }
        },
        OperandType::AddressingMode(addr_mode) => {
            let mut ptr_prefix: String = match op_size {
                OperandSize::Operand8 => "byte ptr ".to_string(),
                OperandSize::Operand16 => "word ptr ".to_string(),
                OperandSize::NoOperand => "*invalid ptr* ".to_string(),
                OperandSize::NoSize => "".to_string()
            };
            // LEA uses addressing calculations but isn't actually a pointer
            if let Mnemonic::LEA = i.mnemonic {
                ptr_prefix = "".to_string()
            }
            // LES and LDS point to a DWORD address 
            if let Mnemonic::LES | Mnemonic::LDS = i.mnemonic {
                ptr_prefix = "dword ptr ".to_string()
            }

            let mut segment1 = "ds".to_string();
            let mut segment2 = "ss".to_string();

            // Handle segment override prefixes 
            match i.segment_override {
                SegmentOverride::ES => {
                    segment1 = "es".to_string();
                    segment2 = "es".to_string();
                }
                SegmentOverride::CS => {
                    segment1 = "cs".to_string();
                    segment2 = "cs".to_string();
                }
                SegmentOverride::SS => {
                    segment1 = "ss".to_string();
                    segment2 = "ss".to_string();
                }
                SegmentOverride::DS => {
                    segment1 = "ds".to_string();
                    segment2 = "ds".to_string();
                }
                _ => {}
            }

            match addr_mode {
                AddressingMode::BxSi             => format!("{}{}:[bx+si]", ptr_prefix, segment1),
                AddressingMode::BxDi             => format!("{}{}:[bx+di]", ptr_prefix, segment1),
                AddressingMode::BpSi             => format!("{}{}:[bp+si]", ptr_prefix, segment2),
                AddressingMode::BpDi             => format!("{}{}:[bp+di]", ptr_prefix, segment2),
                AddressingMode::Si               => format!("{}{}:[si]", ptr_prefix, segment1),
                AddressingMode::Di               => format!("{}{}:[di]", ptr_prefix, segment1),
                AddressingMode::Disp16(disp)     => format!("{}{}:[{}]", ptr_prefix, segment1, disp),
                AddressingMode::Bx               => format!("{}{}:[bx]", ptr_prefix, segment1),
                AddressingMode::BxSiDisp8(disp)  => format!("{}{}:[bx+si+{}]", ptr_prefix, segment1, disp),
                AddressingMode::BxDiDisp8(disp)  => format!("{}{}:[bx+di+{}]", ptr_prefix, segment1, disp),
                AddressingMode::BpSiDisp8(disp)  => format!("{}{}:[bp+si+{}]", ptr_prefix, segment2, disp),
                AddressingMode::BpDiDisp8(disp)  => format!("{}{}:[bp+di+{}]", ptr_prefix, segment2, disp),
                AddressingMode::SiDisp8(disp)    => format!("{}{}:[si+{}]", ptr_prefix, segment1, disp),
                AddressingMode::DiDisp8(disp)    => format!("{}{}:[di+{}]", ptr_prefix, segment1, disp),
                AddressingMode::BpDisp8(disp)    => format!("{}{}:[bp+{}]", ptr_prefix, segment2, disp),
                AddressingMode::BxDisp8(disp)    => format!("{}{}:[bx+{}]", ptr_prefix, segment1, disp),
                AddressingMode::BxSiDisp16(disp) => format!("{}{}:[bx+si+{}]", ptr_prefix, segment1, disp),
                AddressingMode::BxDiDisp16(disp) => format!("{}{}:[bx+di+{}]", ptr_prefix, segment1, disp),
                AddressingMode::BpSiDisp16(disp) => format!("{}{}:[bp+si+{}]", ptr_prefix, segment2, disp),
                AddressingMode::BpDiDisp16(disp) => format!("{}{}:[bp+si+{}]", ptr_prefix, segment2, disp),
                AddressingMode::SiDisp16(disp)   => format!("{}{}:[si+{}]", ptr_prefix, segment1, disp),
                AddressingMode::DiDisp16(disp)   => format!("{}{}:[di+{}]", ptr_prefix, segment1, disp),
                AddressingMode::BpDisp16(disp)   => format!("{}{}:[bp+{}]", ptr_prefix, segment2, disp),
                AddressingMode::BxDisp16(disp)   => format!("{}{}:[bx+{}]", ptr_prefix, segment1, disp),
                AddressingMode::RegisterMode => format!("")
            }
        }
        /*
        OperandType::NearAddress(offset) => {
            format!("[{:#06X}]", offset)
        }
        */
        OperandType::FarAddress(segment, offset) => {
            format!("far {:#06X}:{:#06X}", segment, offset)
        }
        OperandType::NoOperand => "".to_string(),
        _=>"".to_string()
    };

    instruction_string
}


fn tokenize_operand(i: &Instruction, op: OperandSelect) -> Vec<SyntaxToken> {

    let (op_type, op_size) = match op {
        OperandSelect::FirstOperand => (i.operand1_type, i.operand1_size),
        OperandSelect::SecondOperand => (i.operand2_type, i.operand2_size)
    };
    
    let mut op_vec = Vec::new();

    match op_type {
        OperandType::Immediate8(imm8) => {
            op_vec.push(SyntaxToken::HexValue(format!("{:02X}h", imm8)));
        }
        OperandType::Immediate8s(imm8s) => {
            op_vec.push(SyntaxToken::HexValue(format!("{:02X}h", imm8s)));
        }        
        OperandType::Immediate16(imm16) => {
            op_vec.push(SyntaxToken::HexValue(format!("{:04X}h", imm16)));
        }
        OperandType::Relative8(rel8) => {
            //if i.flags & INSTRUCTION_REL_JUMP != 0 {
            //    // Display relative jmp label as absolute offset
            //    let display_imm = relative_offset_u32(i.address + i.size, rel8 as i32);
            //    format!("{:#06X}", display_imm)
            //}
            //else {
            //    format!("{:#06X}", rel8)
            //}
            op_vec.push(SyntaxToken::HexValue(format!("{:02X}h", rel8)));
        }
        OperandType::Relative16(rel16) => {
            //if i.flags & INSTRUCTION_REL_JUMP != 0 {
            //    // Display relative jmp label as absolute offset
            //    let display_imm = relative_offset_u32(i.address + i.size, rel16 as i32);
            //    format!("{:#06X}", display_imm)
            //}
            //else {
            //    format!("{:#06X}", rel16)
            //}            
            op_vec.push(SyntaxToken::HexValue(format!("{:04X}h", rel16)));
        }
        OperandType::Offset8(offset8) => {
            let segment;
            match i.segment_override {
                SegmentOverride::ES => {
                    segment = "es".to_string();
                }
                SegmentOverride::CS => {
                    segment = "cs".to_string();
                }
                SegmentOverride::SS => {
                    segment = "ss".to_string();
                }
                _ => {
                    segment = "ds".to_string();
                }
            }
            op_vec.push(SyntaxToken::Text("byte ".to_string()));
            op_vec.push(SyntaxToken::Segment(segment));
            op_vec.push(SyntaxToken::Colon);
            op_vec.push(SyntaxToken::OpenBracket);
            op_vec.push(SyntaxToken::HexValue(format!("{:04X}h", offset8)));
            op_vec.push(SyntaxToken::CloseBracket);
        }
        OperandType::Offset16(offset16) => {
            let segment;
            match i.segment_override {
                SegmentOverride::ES => {
                    segment = "es".to_string();
                }
                SegmentOverride::CS => {
                    segment = "cs".to_string();
                }
                SegmentOverride::SS => {
                    segment = "ss".to_string();
                }
                _ => {
                    segment = "ds".to_string();
                }
            }                        

            op_vec.push(SyntaxToken::Text("word ".to_string()));
            op_vec.push(SyntaxToken::Segment(segment));
            op_vec.push(SyntaxToken::Colon);
            op_vec.push(SyntaxToken::OpenBracket);
            op_vec.push(SyntaxToken::HexValue(format!("{:04X}h", offset16)));
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
                _=>"".to_string(),
            };

            op_vec.push(SyntaxToken::Register(reg));
        },
        OperandType::AddressingMode(addr_mode) => {
            let mut ptr_prefix: String = match op_size {
                OperandSize::Operand8 => "byte ".to_string(),
                OperandSize::Operand16 => "word ".to_string(),
                OperandSize::NoOperand => "*invalid* ".to_string(),
                OperandSize::NoSize => "".to_string()
            };
            // LEA uses addressing calculations but isn't actually a pointer
            if let Mnemonic::LEA = i.mnemonic {
                ptr_prefix = "".to_string()
            }
            // LES and LDS point to a DWORD address 
            if let Mnemonic::LES | Mnemonic::LDS = i.mnemonic {
                ptr_prefix = "dword".to_string()
            }

            op_vec.push(SyntaxToken::Text(ptr_prefix.clone()));

            let mut segment1 = "ds".to_string();
            let mut segment2 = "ss".to_string();

            // Handle segment override prefixes 
            match i.segment_override {
                SegmentOverride::ES => {
                    segment1 = "es".to_string();
                    segment2 = "es".to_string();
                }
                SegmentOverride::CS => {
                    segment1 = "cs".to_string();
                    segment2 = "cs".to_string();
                }
                SegmentOverride::SS => {
                    segment1 = "ss".to_string();
                    segment2 = "ss".to_string();
                }
                SegmentOverride::DS => {
                    segment1 = "ds".to_string();
                    segment2 = "ds".to_string();
                }
                _ => {}
            }

            let segment1_token = SyntaxToken::Segment(segment1);
            let segment2_token = SyntaxToken::Segment(segment2);

            let mut have_addr_mode = true;

            let (seg_token, disp_opt, ea_vec) = match addr_mode {
                AddressingMode::BxSi             => (segment1_token, None, ["bx","si"]),
                AddressingMode::BxDi             => (segment1_token, None, ["bx", "di"]),
                AddressingMode::BpSi             => (segment2_token, None, ["bp", "si"]),
                AddressingMode::BpDi             => (segment2_token, None, ["bp", "di"]),
                AddressingMode::Si               => (segment1_token, None, ["si", ""]),
                AddressingMode::Di               => (segment1_token, None, ["di", ""]),
                AddressingMode::Disp16(disp)     => (segment1_token, Some(disp), ["", ""]),
                AddressingMode::Bx               => (segment1_token, None, ["bx",""]),
                AddressingMode::BxSiDisp8(disp)  => (segment1_token, Some(disp), ["bx","si"]),
                AddressingMode::BxDiDisp8(disp)  => (segment1_token, Some(disp), ["bx","di"]),
                AddressingMode::BpSiDisp8(disp)  => (segment2_token, Some(disp), ["bp","si"]),
                AddressingMode::BpDiDisp8(disp)  => (segment2_token, Some(disp), ["bp","di"]),
                AddressingMode::SiDisp8(disp)    => (segment1_token, Some(disp), ["si", ""]),
                AddressingMode::DiDisp8(disp)    => (segment1_token, Some(disp), ["di", ""]),
                AddressingMode::BpDisp8(disp)    => (segment2_token, Some(disp), ["bp", ""]),
                AddressingMode::BxDisp8(disp)    => (segment1_token, Some(disp), ["bx", ""]),
                AddressingMode::BxSiDisp16(disp) => (segment1_token, Some(disp), ["bx", "si"]),
                AddressingMode::BxDiDisp16(disp) => (segment1_token, Some(disp), ["bx", "di"]),
                AddressingMode::BpSiDisp16(disp) => (segment2_token, Some(disp), ["bp", "si"]),
                AddressingMode::BpDiDisp16(disp) => (segment2_token, Some(disp), ["bp", "si"]),
                AddressingMode::SiDisp16(disp)   => (segment1_token, Some(disp), ["si", ""]),
                AddressingMode::DiDisp16(disp)   => (segment1_token, Some(disp), ["di", ""]),
                AddressingMode::BpDisp16(disp)   => (segment2_token, Some(disp), ["bp", ""]),
                AddressingMode::BxDisp16(disp)   => (segment1_token, Some(disp), ["bx", ""]),
                AddressingMode::RegisterMode => {
                    have_addr_mode = false;
                    (segment1_token, None, ["", ""])
                }
            };

            if have_addr_mode {
                //op_vec.push(SyntaxToken::Text(ptr_prefix.clone()));
                op_vec.push(seg_token);
                op_vec.push(SyntaxToken::Colon);
                op_vec.push(SyntaxToken::OpenBracket);

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
                        op_vec.push(SyntaxToken::PlusSign);
                        op_vec.push(SyntaxToken::Displacement(format!("{}", disp)));
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
            op_vec.push(SyntaxToken::Text("far".to_string()));
            op_vec.push(SyntaxToken::HexValue(format!("{:04X}h", segment)));
            op_vec.push(SyntaxToken::Colon);
            op_vec.push(SyntaxToken::HexValue(format!("{:04X}h", offset)));
        }
        _ => {}
    };

    op_vec
}

fn prefix_to_string(i: &Instruction ) -> String {

    // Handle REPx prefixes
    if i.prefixes & OPCODE_PREFIX_REP1 != 0 {
        "repne".to_string()
    } 
    else if i.prefixes & OPCODE_PREFIX_LOCK != 0 {
        "lock".to_string()
    }
    else if i.prefixes & OPCODE_PREFIX_REP2 != 0 {
        match i.opcode {
            0xA4 | 0xA5 | 0xAA | 0xAB | 0xAC | 0xAD => "rep".to_string(),
            0xA6 | 0xA7 | 0xAE | 0xAF => "repe".to_string(),
            _ =>" ".to_string(),
        }
    }
    else {
        "".to_string()
    }
}

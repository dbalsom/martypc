use std::fmt::Display;
use std::error::Error;

use crate::cpu::*;
use crate::cpu::cpu_addressing::AddressingMode;
use crate::cpu::cpu_modrm::ModRmByte;
use crate::cpu::cpu_mnemonic::Mnemonic;

use crate::bytequeue::*;

#[derive(Copy, Clone)]
#[derive(PartialEq)]
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
    NearAddress,
    FarAddress
}

#[derive(Debug)]
pub enum InstructionDecodeError {
    UnsupportedOpcode(u8),
    InvalidSegmentRegister,
    ReadOutOfBounds,
    GeneralDecodeError(u8),
    Unimplemented(u8)
}

impl Error for InstructionDecodeError {}
impl Display for InstructionDecodeError{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            InstructionDecodeError::UnsupportedOpcode(o)=> {
                write!(f, "An unsupported opcode was encountered: {:#2x}.", o )
            }
            InstructionDecodeError::InvalidSegmentRegister=> {
                write!(f, "An invalid segment register was specified.")
            }
            InstructionDecodeError::ReadOutOfBounds=> {
                write!(f, "Unexpected buffer exhaustion while decoding instruction.")
            }
            InstructionDecodeError::GeneralDecodeError(o) => {
                write!(f, "General error decoding opcode {:#2x}.", o)
            }
            InstructionDecodeError::Unimplemented(o)=> {
                write!(f, "Decoding of instruction {:#2x} not implemented.", o)
            }
        }
    }
}

impl<'a> Cpu<'a> {
    pub fn decode(bytes: &mut impl ByteQueue) -> Result<Instruction, Box<dyn std::error::Error>> {

        let mut operand1_type: OperandType = OperandType::NoOperand;
        let mut operand2_type: OperandType = OperandType::NoOperand;
        let mut operand1_size: OperandSize = OperandSize::NoOperand;
        let mut operand2_size: OperandSize = OperandSize::NoOperand;

        let op_address = bytes.tell() as u32;
        bytes.clear_delay();
        let mut opcode = bytes.q_read_u8(QueueType::First);

        let mut mnemonic;

        let mut operand1_template = OperandTemplate::NoTemplate;
        let mut operand2_template = OperandTemplate::NoTemplate;
        let mut op_size: u32 = 1;
        let mut op_flags: u32 = 0;
        let mut op_prefixes: u32 = 0;
        let mut op_segment_override = SegmentOverride::None;
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
                0xF2 => OPCODE_PREFIX_REP1,
                0xF3 => OPCODE_PREFIX_REP2,
                _=> {
                    break;
                }
            };
            // ... but only store the last segment override prefix seen
            op_segment_override = match opcode {
                0x26 => SegmentOverride::ES,
                0x2E => SegmentOverride::CS,
                0x36 => SegmentOverride::SS,
                0x3E => SegmentOverride::DS,
                _=> op_segment_override
            };

            // Reset first-fetch flag on each prefix read
            opcode = bytes.q_read_u8(QueueType::First);
        }

        // Match templatizeable instructions
        (mnemonic, operand1_template, operand2_template, op_flags) = match opcode {
            0x00 => (Mnemonic::ADD,  OperandTemplate::ModRM8,   OperandTemplate::Register8,     0),
            0x01 => (Mnemonic::ADD,  OperandTemplate::ModRM16,   OperandTemplate::Register16,   0),
            0x02 => (Mnemonic::ADD,  OperandTemplate::Register8,   OperandTemplate::ModRM8,     0),
            0x03 => (Mnemonic::ADD,  OperandTemplate::Register16,   OperandTemplate::ModRM16,   0),
            0x04 => (Mnemonic::ADD,  OperandTemplate::FixedRegister8(Register8::AL),   OperandTemplate::Immediate8,    0),
            0x05 => (Mnemonic::ADD,  OperandTemplate::FixedRegister16(Register16::AX),   OperandTemplate::Immediate16, 0),
            0x06 => (Mnemonic::PUSH, OperandTemplate::FixedRegister16(Register16::ES),   OperandTemplate::NoOperand,   0),
            0x07 => (Mnemonic::POP,  OperandTemplate::FixedRegister16(Register16::ES),   OperandTemplate::NoOperand,   0),
            0x08 => (Mnemonic::OR,   OperandTemplate::ModRM8,    OperandTemplate::Register8,    0),
            0x09 => (Mnemonic::OR,   OperandTemplate::ModRM16,    OperandTemplate::Register16,  0),
            0x0A => (Mnemonic::OR,   OperandTemplate::Register8,    OperandTemplate::ModRM8,    0),
            0x0B => (Mnemonic::OR,   OperandTemplate::Register16,    OperandTemplate::ModRM16,  0),
            0x0C => (Mnemonic::OR,   OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
            0x0D => (Mnemonic::OR,   OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0),
            0x0E => (Mnemonic::PUSH, OperandTemplate::FixedRegister16(Register16::CS),   OperandTemplate::NoOperand,   0),
            0x0F => (Mnemonic::POP,  OperandTemplate::FixedRegister16(Register16::CS),   OperandTemplate::NoOperand,   0),    
            0x10 => (Mnemonic::ADC,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    0),
            0x11 => (Mnemonic::ADC,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  0),
            0x12 => (Mnemonic::ADC,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    0),
            0x13 => (Mnemonic::ADC,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  0),
            0x14 => (Mnemonic::ADC,  OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
            0x15 => (Mnemonic::ADC,  OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0), 
            0x16 => (Mnemonic::PUSH, OperandTemplate::FixedRegister16(Register16::SS),   OperandTemplate::NoOperand,   0),
            0x17 => (Mnemonic::POP,  OperandTemplate::FixedRegister16(Register16::SS),   OperandTemplate::NoOperand,   0), 
            0x18 => (Mnemonic::SBB,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    0),
            0x19 => (Mnemonic::SBB,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  0),
            0x1A => (Mnemonic::SBB,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    0),
            0x1B => (Mnemonic::SBB,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  0),
            0x1C => (Mnemonic::SBB,  OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
            0x1D => (Mnemonic::SBB,  OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0), 
            0x1E => (Mnemonic::PUSH, OperandTemplate::FixedRegister16(Register16::DS),   OperandTemplate::NoOperand,   0),
            0x1F => (Mnemonic::POP,  OperandTemplate::FixedRegister16(Register16::DS),   OperandTemplate::NoOperand,   0),   
            0x20 => (Mnemonic::AND,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    0),
            0x21 => (Mnemonic::AND,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  0),
            0x22 => (Mnemonic::AND,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    0),
            0x23 => (Mnemonic::AND,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  0),
            0x24 => (Mnemonic::AND,  OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
            0x25 => (Mnemonic::AND,  OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0), 
            0x27 => (Mnemonic::DAA,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand, 0),
            0x28 => (Mnemonic::SUB,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    0),
            0x29 => (Mnemonic::SUB,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  0),
            0x2A => (Mnemonic::SUB,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    0),
            0x2B => (Mnemonic::SUB,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  0),
            0x2C => (Mnemonic::SUB,  OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
            0x2D => (Mnemonic::SUB,  OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0), 
            0x2F => (Mnemonic::DAS,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,  0),
            0x30 => (Mnemonic::XOR,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    0),
            0x31 => (Mnemonic::XOR,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  0),
            0x32 => (Mnemonic::XOR,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    0),
            0x33 => (Mnemonic::XOR,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  0),
            0x34 => (Mnemonic::XOR,  OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
            0x35 => (Mnemonic::XOR,  OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0),
        //  0x36 Segment override prefix
            0x37 => (Mnemonic::AAA,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,  0),
            0x38 => (Mnemonic::CMP,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    0),
            0x39 => (Mnemonic::CMP,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  0),
            0x3A => (Mnemonic::CMP,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    0),
            0x3B => (Mnemonic::CMP,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  0),
            0x3C => (Mnemonic::CMP,  OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
            0x3D => (Mnemonic::CMP,  OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0),
            0x3F => (Mnemonic::AAS,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,  0),
            0x40..=0x47 => (Mnemonic::INC,  OperandTemplate::Register16Encoded,    OperandTemplate::NoOperand, 0),
            0x48..=0x4F => (Mnemonic::DEC,  OperandTemplate::Register16Encoded,    OperandTemplate::NoOperand, 0),
            0x50..=0x57 => (Mnemonic::PUSH, OperandTemplate::Register16Encoded,    OperandTemplate::NoOperand, 0),
            0x58..=0x5F => (Mnemonic::POP,  OperandTemplate::Register16Encoded,    OperandTemplate::NoOperand, 0),
        //  0x60..=0x6F >= on 8088, these instructions map to 0x70-7F
            0x60 => (Mnemonic::JO,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x61 => (Mnemonic::JNO,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x62 => (Mnemonic::JB,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x63 => (Mnemonic::JNB,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x64 => (Mnemonic::JZ,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x65 => (Mnemonic::JNZ,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x66 => (Mnemonic::JBE,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x67 => (Mnemonic::JNBE, OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x68 => (Mnemonic::JS,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x69 => (Mnemonic::JNS,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x6A => (Mnemonic::JP,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x6B => (Mnemonic::JNP,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x6C => (Mnemonic::JL,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x6D => (Mnemonic::JNL,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x6E => (Mnemonic::JLE,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x6F => (Mnemonic::JNLE, OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),        
            0x70 => (Mnemonic::JO,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x71 => (Mnemonic::JNO,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x72 => (Mnemonic::JB,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x73 => (Mnemonic::JNB,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x74 => (Mnemonic::JZ,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x75 => (Mnemonic::JNZ,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x76 => (Mnemonic::JBE,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x77 => (Mnemonic::JNBE, OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x78 => (Mnemonic::JS,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x79 => (Mnemonic::JNS,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x7A => (Mnemonic::JP,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x7B => (Mnemonic::JNP,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x7C => (Mnemonic::JL,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x7D => (Mnemonic::JNL,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x7E => (Mnemonic::JLE,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
            0x7F => (Mnemonic::JNLE, OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),

            0x84 => (Mnemonic::TEST,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    0),
            0x85 => (Mnemonic::TEST,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  0),
            0x86 => (Mnemonic::XCHG,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    0),
            0x87 => (Mnemonic::XCHG,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  0),
            0x88 => (Mnemonic::MOV,   OperandTemplate::ModRM8,    OperandTemplate::Register8,    0),
            0x89 => (Mnemonic::MOV,   OperandTemplate::ModRM16,    OperandTemplate::Register16,  0),
            0x8A => (Mnemonic::MOV,   OperandTemplate::Register8,    OperandTemplate::ModRM8,    0),
            0x8B => (Mnemonic::MOV,   OperandTemplate::Register16,    OperandTemplate::ModRM16,  0),
            0x8C => (Mnemonic::MOV,   OperandTemplate::ModRM16,    OperandTemplate::SegmentRegister,  0),
            0x8D => (Mnemonic::LEA,   OperandTemplate::Register16,   OperandTemplate::ModRM16,   0),
            0x8E => (Mnemonic::MOV,   OperandTemplate::SegmentRegister,    OperandTemplate::ModRM16,  0),
            0x8F => (Mnemonic::POP,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,    0),
            0x90 => (Mnemonic::NOP,   OperandTemplate::NoOperand,   OperandTemplate::NoOperand,  0),
            0x91..=0x97 => (Mnemonic::XCHG,  OperandTemplate::Register16Encoded,   OperandTemplate::FixedRegister16(Register16::AX),  0),
            0x98 => (Mnemonic::CBW,   OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
            0x99 => (Mnemonic::CWD,   OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
            0x9A => (Mnemonic::CALLF, OperandTemplate::FarAddress,   OperandTemplate::NoOperand,  0), 
            0x9B => (Mnemonic::FWAIT, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0), 
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
            0xC4 => (Mnemonic::LES,  OperandTemplate::Register16,   OperandTemplate::ModRM16,     0),
            0xC5 => (Mnemonic::LDS,  OperandTemplate::Register16,   OperandTemplate::ModRM16,     0),
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
            0xD8..=0xDF => (Mnemonic::ESC, OperandTemplate::ModRM16, OperandTemplate::NoOperand, 0),

            0xE0 => (Mnemonic::LOOPNE, OperandTemplate::Relative8,   OperandTemplate::NoOperand,   INSTRUCTION_REL_JUMP),
            0xE1 => (Mnemonic::LOOPE,  OperandTemplate::Relative8,   OperandTemplate::NoOperand,   INSTRUCTION_REL_JUMP),
            0xE2 => (Mnemonic::LOOP, OperandTemplate::Relative8,   OperandTemplate::NoOperand,     INSTRUCTION_REL_JUMP),
            0xE3 => (Mnemonic::JCXZ, OperandTemplate::Relative8,   OperandTemplate::NoOperand,     INSTRUCTION_REL_JUMP),
            0xE4 => (Mnemonic::IN,   OperandTemplate::FixedRegister8(Register8::AL),   OperandTemplate::Immediate8,    0),
            0xE5 => (Mnemonic::IN,   OperandTemplate::FixedRegister16(Register16::AX),   OperandTemplate::Immediate8,   0),
            0xE6 => (Mnemonic::OUT,  OperandTemplate::Immediate8,   OperandTemplate::FixedRegister8(Register8::AL),  0),
            0xE7 => (Mnemonic::OUT,  OperandTemplate::Immediate8,   OperandTemplate::FixedRegister16(Register16::AX), 0),
            0xE8 => (Mnemonic::CALL, OperandTemplate::Relative16,   OperandTemplate::NoOperand,    INSTRUCTION_REL_JUMP),
            0xE9 => (Mnemonic::JMP,  OperandTemplate::Relative16,   OperandTemplate::NoOperand,    INSTRUCTION_REL_JUMP),
            0xEA => (Mnemonic::JMPF, OperandTemplate::FarAddress,  OperandTemplate::NoOperand,    0),
            0xEB => (Mnemonic::JMP,  OperandTemplate::Relative8,   OperandTemplate::NoOperand,     INSTRUCTION_REL_JUMP),
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
            // All group instructions have a modrm w/ op extension.

            modrm = ModRmByte::read_from(bytes)?;
            loaded_modrm = true;
            let op_ext = modrm.get_op_extension();
            
            (mnemonic, operand1_template, operand2_template, op_flags) = match (opcode, op_ext) {
                (0x80 | 0x82, 0x00) => (Mnemonic::ADD,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,    0),
                (0x80 | 0x82, 0x01) => (Mnemonic::OR,   OperandTemplate::ModRM8,   OperandTemplate::Immediate8,    0),
                (0x80 | 0x82, 0x02) => (Mnemonic::ADC,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,    0),
                (0x80 | 0x82, 0x03) => (Mnemonic::SBB,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,    0),
                (0x80 | 0x82, 0x04) => (Mnemonic::AND,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,    0),
                (0x80 | 0x82, 0x05) => (Mnemonic::SUB,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,    0),
                (0x80 | 0x82, 0x06) => (Mnemonic::XOR,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,    0),
                (0x80 | 0x82, 0x07) => (Mnemonic::CMP,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,    0),
                
                (0x81, 0x00) => (Mnemonic::ADD,   OperandTemplate::ModRM16,   OperandTemplate::Immediate16,    0),
                (0x81, 0x01) => (Mnemonic::OR,    OperandTemplate::ModRM16,   OperandTemplate::Immediate16,    0),
                (0x81, 0x02) => (Mnemonic::ADC,   OperandTemplate::ModRM16,   OperandTemplate::Immediate16,    0),
                (0x81, 0x03) => (Mnemonic::SBB,   OperandTemplate::ModRM16,   OperandTemplate::Immediate16,    0),
                (0x81, 0x04) => (Mnemonic::AND,   OperandTemplate::ModRM16,   OperandTemplate::Immediate16,    0),
                (0x81, 0x05) => (Mnemonic::SUB,   OperandTemplate::ModRM16,   OperandTemplate::Immediate16,    0),
                (0x81, 0x06) => (Mnemonic::XOR,   OperandTemplate::ModRM16,   OperandTemplate::Immediate16,    0),
                (0x81, 0x07) => (Mnemonic::CMP,   OperandTemplate::ModRM16,   OperandTemplate::Immediate16,    0),
                
                (0x83, 0x00) => (Mnemonic::ADD,   OperandTemplate::ModRM16,   OperandTemplate::Immediate8SignExtended,    0),
                (0x83, 0x01) => (Mnemonic::OR,    OperandTemplate::ModRM16,   OperandTemplate::Immediate8SignExtended,    0),
                (0x83, 0x02) => (Mnemonic::ADC,   OperandTemplate::ModRM16,   OperandTemplate::Immediate8SignExtended,    0),
                (0x83, 0x03) => (Mnemonic::SBB,   OperandTemplate::ModRM16,   OperandTemplate::Immediate8SignExtended,    0),
                (0x83, 0x04) => (Mnemonic::AND,   OperandTemplate::ModRM16,   OperandTemplate::Immediate8SignExtended,    0),
                (0x83, 0x05) => (Mnemonic::SUB,   OperandTemplate::ModRM16,   OperandTemplate::Immediate8SignExtended,    0),
                (0x83, 0x06) => (Mnemonic::XOR,   OperandTemplate::ModRM16,   OperandTemplate::Immediate8SignExtended,    0),
                (0x83, 0x07) => (Mnemonic::CMP,   OperandTemplate::ModRM16,   OperandTemplate::Immediate8SignExtended,    0),   
                
                (0xD0, 0x00) => (Mnemonic::ROL,   OperandTemplate::ModRM8,    OperandTemplate::NoOperand,    0),
                (0xD0, 0x01) => (Mnemonic::ROR,   OperandTemplate::ModRM8,    OperandTemplate::NoOperand,    0),
                (0xD0, 0x02) => (Mnemonic::RCL,   OperandTemplate::ModRM8,    OperandTemplate::NoOperand,    0),
                (0xD0, 0x03) => (Mnemonic::RCR,   OperandTemplate::ModRM8,    OperandTemplate::NoOperand,    0),
                (0xD0, 0x04) => (Mnemonic::SHL,   OperandTemplate::ModRM8,    OperandTemplate::NoOperand,    0),
                (0xD0, 0x05) => (Mnemonic::SHR,   OperandTemplate::ModRM8,    OperandTemplate::NoOperand,    0),
                (0xD0, 0x06) => (Mnemonic::SETMO, OperandTemplate::ModRM8,    OperandTemplate::NoOperand,    0),
                (0xD0, 0x07) => (Mnemonic::SAR,   OperandTemplate::ModRM8,    OperandTemplate::NoOperand,    0),
                
                (0xD1, 0x00) => (Mnemonic::ROL,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,    0),
                (0xD1, 0x01) => (Mnemonic::ROR,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,    0),
                (0xD1, 0x02) => (Mnemonic::RCL,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,    0),
                (0xD1, 0x03) => (Mnemonic::RCR,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,    0),
                (0xD1, 0x04) => (Mnemonic::SHL,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,    0),
                (0xD1, 0x05) => (Mnemonic::SHR,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,    0),
                (0xD1, 0x06) => (Mnemonic::SETMO, OperandTemplate::ModRM16,   OperandTemplate::NoOperand,    0),
                (0xD1, 0x07) => (Mnemonic::SAR,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,    0),

                (0xD2, 0x00) => (Mnemonic::ROL,   OperandTemplate::ModRM8,    OperandTemplate::FixedRegister8(Register8::CL),    0),
                (0xD2, 0x01) => (Mnemonic::ROR,   OperandTemplate::ModRM8,    OperandTemplate::FixedRegister8(Register8::CL),    0),
                (0xD2, 0x02) => (Mnemonic::RCL,   OperandTemplate::ModRM8,    OperandTemplate::FixedRegister8(Register8::CL),    0),
                (0xD2, 0x03) => (Mnemonic::RCR,   OperandTemplate::ModRM8,    OperandTemplate::FixedRegister8(Register8::CL),    0),
                (0xD2, 0x04) => (Mnemonic::SHL,   OperandTemplate::ModRM8,    OperandTemplate::FixedRegister8(Register8::CL),    0),
                (0xD2, 0x05) => (Mnemonic::SHR,   OperandTemplate::ModRM8,    OperandTemplate::FixedRegister8(Register8::CL),    0),
                (0xD2, 0x06) => (Mnemonic::SETMOC,OperandTemplate::ModRM8,    OperandTemplate::FixedRegister8(Register8::CL),    0),
                (0xD2, 0x07) => (Mnemonic::SAR,   OperandTemplate::ModRM8,    OperandTemplate::FixedRegister8(Register8::CL),    0),

                (0xD3, 0x00) => (Mnemonic::ROL,   OperandTemplate::ModRM16,   OperandTemplate::FixedRegister8(Register8::CL),    0),
                (0xD3, 0x01) => (Mnemonic::ROR,   OperandTemplate::ModRM16,   OperandTemplate::FixedRegister8(Register8::CL),    0),
                (0xD3, 0x02) => (Mnemonic::RCL,   OperandTemplate::ModRM16,   OperandTemplate::FixedRegister8(Register8::CL),    0),
                (0xD3, 0x03) => (Mnemonic::RCR,   OperandTemplate::ModRM16,   OperandTemplate::FixedRegister8(Register8::CL),    0),
                (0xD3, 0x04) => (Mnemonic::SHL,   OperandTemplate::ModRM16,   OperandTemplate::FixedRegister8(Register8::CL),    0),
                (0xD3, 0x05) => (Mnemonic::SHR,   OperandTemplate::ModRM16,   OperandTemplate::FixedRegister8(Register8::CL),    0),
                (0xD3, 0x06) => (Mnemonic::SETMOC,OperandTemplate::ModRM16,   OperandTemplate::FixedRegister8(Register8::CL),    0),
                (0xD3, 0x07) => (Mnemonic::SAR,   OperandTemplate::ModRM16,   OperandTemplate::FixedRegister8(Register8::CL),    0),

                (0xF6, 0x00) => (Mnemonic::TEST,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,     0),
                (0xF6, 0x01) => (Mnemonic::TEST,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,     0),
                (0xF6, 0x02) => (Mnemonic::NOT,   OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      0),
                (0xF6, 0x03) => (Mnemonic::NEG,   OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      0),
                (0xF6, 0x04) => (Mnemonic::MUL,   OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      0),
                (0xF6, 0x05) => (Mnemonic::IMUL,  OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      0),
                (0xF6, 0x06) => (Mnemonic::DIV,   OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      0),
                (0xF6, 0x07) => (Mnemonic::IDIV,  OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      0),

                (0xF7, 0x00) => (Mnemonic::TEST,  OperandTemplate::ModRM16,   OperandTemplate::Immediate16,   0),
                (0xF7, 0x01) => (Mnemonic::TEST,  OperandTemplate::ModRM16,   OperandTemplate::Immediate16,   0),
                (0xF7, 0x02) => (Mnemonic::NOT,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     0),
                (0xF7, 0x03) => (Mnemonic::NEG,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     0),
                (0xF7, 0x04) => (Mnemonic::MUL,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     0),
                (0xF7, 0x05) => (Mnemonic::IMUL,  OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     0),
                (0xF7, 0x06) => (Mnemonic::DIV,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     0),
                (0xF7, 0x07) => (Mnemonic::IDIV,  OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     0),                

                (0xFE, 0x00) => (Mnemonic::INC,   OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      0),
                (0xFE, 0x01) => (Mnemonic::DEC,   OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      0),
                (0xFE, 0x02) => (Mnemonic::CALL,  OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      0),
                (0xFE, 0x03) => (Mnemonic::CALLF, OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      0),
                (0xFE, 0x04) => (Mnemonic::JMP,   OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      0),
                (0xFE, 0x05) => (Mnemonic::JMPF,  OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      0),
                (0xFE, 0x06) => (Mnemonic::PUSH,  OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      0),
                (0xFE, 0x07) => (Mnemonic::PUSH,  OperandTemplate::ModRM8,   OperandTemplate::NoOperand,      0),                    
                    
                (0xFF, 0x00) => (Mnemonic::INC,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     0),
                (0xFF, 0x01) => (Mnemonic::DEC,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     0),
                (0xFF, 0x02) => (Mnemonic::CALL,  OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     0),
                (0xFF, 0x03) => (Mnemonic::CALLF, OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     0),
                (0xFF, 0x04) => (Mnemonic::JMP,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     0),
                (0xFF, 0x05) => (Mnemonic::JMPF,  OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     0),
                (0xFF, 0x06) => (Mnemonic::PUSH,  OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     0),
                (0xFF, 0x07) => (Mnemonic::PUSH,  OperandTemplate::ModRM16,   OperandTemplate::NoOperand,     0), 
                
                _=> (Mnemonic::NoOpcode, OperandTemplate::NoOperand, OperandTemplate::NoOperand, 0)
            };

            op_flags |= INSTRUCTION_HAS_MODRM;
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
            op_flags |= INSTRUCTION_HAS_MODRM;
            modrm = ModRmByte::read_from(bytes)?;
        }
        else if !loaded_modrm {
            // No modrm. Set a one cycle fetch delay. This has no effect when reading from memory.
            // When fetching from the processor instruction queue, the 2nd byte must be a modrm or 
            // the fetch is skipped for that cycle.
            bytes.delay(1);
        }

        // Handle fetch delays for 0xF0, 0xF1, 0xF2, 0xF3
        // These instructions decrement and compare CX before fetching their rel8 operand, taking two
        // additional cycles. This is hacky but necessary to have seperate decode/execute phases.
        if opcode & 0xFC == 0xF0 {
            bytes.delay(2);
        }

        // Match templatized operands.
        let mut match_op = |op_template| -> Result<(OperandType, OperandSize), Box<dyn std::error::Error>> {
            match op_template {

                OperandTemplate::ModRM8 => {
                    let addr_mode = modrm.get_addressing_mode();
                    let operand_type = match addr_mode {
                        AddressingMode::RegisterMode => OperandType::Register8(modrm.get_op1_reg8()),
                        _=> OperandType::AddressingMode(addr_mode),
                    };
                    Ok((operand_type, OperandSize::Operand8))
                }
                OperandTemplate::ModRM16 => {
                    let addr_mode = modrm.get_addressing_mode();
                    let operand_type = match addr_mode {
                        AddressingMode::RegisterMode => OperandType::Register16(modrm.get_op1_reg16()),
                        _=> OperandType::AddressingMode(addr_mode)
                    };
                    Ok((operand_type, OperandSize::Operand16))
                }
                OperandTemplate::Register8 => {
                    let operand_type = OperandType::Register8(modrm.get_op2_reg8());
                    Ok((operand_type, OperandSize::Operand8))
                }
                OperandTemplate::Register16 => {              
                    let operand_type = OperandType::Register16(modrm.get_op2_reg16());
                    Ok((operand_type, OperandSize::Operand16))       
                }
                OperandTemplate::SegmentRegister => {
                    let operand_type = OperandType::Register16(modrm.get_op2_segmentreg16());
                    Ok((operand_type, OperandSize::Operand16))
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
                    Ok((operand_type, OperandSize::Operand8))
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
                    Ok((operand_type, OperandSize::Operand16))
                }
                OperandTemplate::Immediate8 => {
                    let operand = bytes.q_read_u8(QueueType::Subsequent);
                    Ok((OperandType::Immediate8(operand), OperandSize::Operand8))
                }
                OperandTemplate::Immediate16 => {
                    let operand = bytes.q_read_u16(QueueType::Subsequent);
                    Ok((OperandType::Immediate16(operand), OperandSize::Operand16))
                }
                OperandTemplate::Immediate8SignExtended => {
                    let operand = bytes.q_read_u8(QueueType::Subsequent);
                    let op_extended = operand as i8 as i16 as u16;
                    Ok((OperandType::Immediate16(op_extended), OperandSize::Operand16))
                }
                OperandTemplate::Relative8 => {
                    let operand = bytes.q_read_i8(QueueType::Subsequent);
                    Ok((OperandType::Relative8(operand), OperandSize::Operand8))
                }
                OperandTemplate::Relative16 => {
                    let operand = bytes.q_read_i16(QueueType::Subsequent);
                    Ok((OperandType::Relative16(operand), OperandSize::Operand16))                
                }
                OperandTemplate::Offset8 => {
                    let operand = bytes.q_read_u16(QueueType::Subsequent);
                    Ok((OperandType::Offset8(operand), OperandSize::Operand8))
                }
                OperandTemplate::Offset16 => {
                    let operand = bytes.q_read_u16(QueueType::Subsequent);
                    Ok((OperandType::Offset16(operand), OperandSize::Operand16))
                }
                OperandTemplate::FixedRegister8(r8) => {
                    Ok((OperandType::Register8(r8), OperandSize::Operand8))
                }
                OperandTemplate::FixedRegister16(r16) => {
                    Ok((OperandType::Register16(r16), OperandSize::Operand16))
                }
                OperandTemplate::NearAddress => {
                    let offset = bytes.q_read_u16(QueueType::Subsequent);
                    Ok((OperandType::NearAddress(offset), OperandSize::NoSize))
                }
                OperandTemplate::FarAddress => {
                    let offset = bytes.q_read_u16(QueueType::Subsequent);
                    let segment = bytes.q_read_u16(QueueType::Subsequent);
                    Ok((OperandType::FarAddress(segment,offset), OperandSize::NoSize))
                }
                _=>Ok((OperandType::NoOperand,OperandSize::NoOperand))
            }
        };

        match operand1_template {
            OperandTemplate::NoTemplate => {},
            _=> (operand1_type, operand1_size) = match_op(operand1_template)?
        }
    
        match operand2_template {
            OperandTemplate::NoTemplate => {},
            _=> (operand2_type, operand2_size) = match_op(operand2_template)?
        }

        // Set a flag if either of the instruction operands is a memory operand.
        if let OperandType::AddressingMode(_) = operand1_type {
            op_flags |= INSTRUCTION_USES_MEM;
        }
        if let OperandType::AddressingMode(_) = operand2_type {
            op_flags |= INSTRUCTION_USES_MEM;
        }

        op_size = bytes.tell() as u32 - op_address;

        if let Mnemonic::InvalidOpcode = mnemonic {
            return Err(Box::new(InstructionDecodeError::UnsupportedOpcode(opcode)));
        }

        Ok(Instruction { 
            opcode: opcode,
            flags: op_flags,
            prefixes: op_prefixes,
            address: 0,
            size: op_size, 
            mnemonic: mnemonic,
            segment_override: op_segment_override,
            operand1_type: operand1_type,
            operand1_size: operand1_size,
            //operand1: 0,
            operand2_type: operand2_type,
            operand2_size: operand2_size,
            //operand2: 0,
            is_location: false 
        })
    }
}

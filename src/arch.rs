#![allow(dead_code)]
use std::fmt;
use std::fmt::Display;
use std::error::Error;

use crate::byteinterface::{ByteInterface};

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

pub const MAX_INSTRUCTION_SIZE: usize = 15;

const OPCODE_REGISTER_SELECT_MASK: u8 = 0b0000_0111;

const MODRM_REG_MASK:          u8 = 0b00_111_000;
const MODRM_ADDR_MASK:         u8 = 0b11_000_111;
const MODRM_MOD_MASK:          u8 = 0b11_000_000;

const MODRM_ADDR_BX_SI:        u8 = 0b00_000_000;
const MODRM_ADDR_BX_DI:        u8 = 0b00_000_001;
const MODRM_ADDR_BP_SI:        u8 = 0b00_000_010;
const MODRM_ADDR_BP_DI:        u8 = 0b00_000_011;
const MODRM_ADDR_SI:           u8 = 0b00_000_100;
const MODRM_ADDR_DI:           u8 = 0b00_000_101;
const MODRM_ADDR_DISP16:       u8 = 0b00_000_110;
const MODRM_ADDR_BX:           u8 = 0b00_000_111;

const MODRM_ADDR_BX_SI_DISP8:  u8 = 0b01_000_000;
const MODRM_ADDR_BX_DI_DISP8:  u8 = 0b01_000_001;
const MODRM_ADDR_BP_SI_DISP8:  u8 = 0b01_000_010;
const MODRM_ADDR_BP_DI_DISP8:  u8 = 0b01_000_011;
const MODRM_ADDR_SI_DI_DISP8:  u8 = 0b01_000_100;
const MODRM_ADDR_DI_DISP8:     u8 = 0b01_000_101;
const MODRM_ADDR_BP_DISP8:     u8 = 0b01_000_110;
const MODRM_ADDR_BX_DISP8:     u8 = 0b01_000_111;

const MODRM_ADDR_BX_SI_DISP16: u8 = 0b10_000_000;
const MODRM_ADDR_BX_DI_DISP16: u8 = 0b10_000_001;
const MODRM_ADDR_BP_SI_DISP16: u8 = 0b10_000_010;
const MODRM_ADDR_BP_DI_DISP16: u8 = 0b10_000_011;
const MODRM_ADDR_SI_DI_DISP16: u8 = 0b10_000_100;
const MODRM_ADDR_DI_DISP16:    u8 = 0b10_000_101;
const MODRM_ADDR_BP_DISP16:    u8 = 0b10_000_110;
const MODRM_ADDR_BX_DISP16:    u8 = 0b10_000_111;

const MODRM_EG_AX_OR_AL:       u8 = 0b00_000_000;
const MODRM_REG_CX_OR_CL:      u8 = 0b00_000_001;
const MODRM_REG_DX_OR_DL:      u8 = 0b00_000_010;
const MODRM_REG_BX_OR_BL:      u8 = 0b00_000_011;
const MODRM_REG_SP_OR_AH:      u8 = 0b00_000_100;
const MODRM_REG_BP_OR_CH:      u8 = 0b00_000_101;
const MODRM_REG_SI_OR_DH:      u8 = 0b00_000_110;
const MODRM_RED_DI_OR_BH:      u8 = 0b00_000_111;

// Instruction flags
const INSTRUCTION_USES_MEM:    u32 = 0b0000_0001;
const INSTRUCTION_HAS_MODRM:   u32 = 0b0000_0010;
const INSTRUCTION_LOCKABLE:    u32 = 0b0000_0100;
const INSTRUCTION_REL_JUMP:    u32 = 0b0000_1000;

// Instruction prefixes
pub const OPCODE_PREFIX_ES_OVERRIDE: u32     = 0b_0000_0000_0001;
pub const OPCODE_PREFIX_CS_OVERRIDE: u32     = 0b_0000_0000_0010;
pub const OPCODE_PREFIX_SS_OVERRIDE: u32     = 0b_0000_0000_0100;
pub const OPCODE_PREFIX_DS_OVERRIDE: u32     = 0b_0000_0000_1000;
pub const OPCODE_PREFIX_OPERAND_OVERIDE: u32 = 0b_0000_0001_0000;
pub const OPCODE_PREFIX_ADDRESS_OVERIDE: u32 = 0b_0000_0010_0000;
pub const OPCODE_PREFIX_WAIT: u32            = 0b_0000_0100_0000;
pub const OPCODE_PREFIX_LOCK: u32            = 0b_0000_1000_0000;
pub const OPCODE_PREFIX_REP1: u32            = 0b_0001_0000_0000;
pub const OPCODE_PREFIX_REP2: u32            = 0b_0010_0000_0000;

#[derive(Debug)]
pub enum RepType {
    NoRep,
    Rep,
    Repne,
    Repe
}
impl Default for RepType {
    fn default() -> Self { RepType::NoRep }
}
pub enum RepDirection {
    RepForward,
    RepReverse
}

#[derive(Copy, Clone, Debug)]
pub enum Opcode {
    InvalidOpcode,
    NOP,
    AAA,
    AAD,
    AAM,
    AAS,
    ADC,
    ADD,
    AND,
    CALL,
    CALLF,
    CBW,
    CLC,
    CLD,
    CLI,
    CMC,
    CMP,
    CMPSB,
    CMPSW,
    CWD,
    DAA,
    DAS,
    DEC,
    DIV,
    ESC,
    FWAIT,
    HLT,
    IDIV,
    IMUL,
    IN,
    INC,
    INT,
    INT3,
    INTO,
    IRET,
    JB,
    JBE,
    JCXZ,
    JL,
    JLE,
    JMP,
    JMPF,
    JNB,
    JNBE,
    JNL,
    JNLE,
    JNO,
    JNP,
    JNS,
    JNZ,
    JO,
    JP,
    JS,
    JZ,
    LAHF,
    LDS,
    LEA,
    LES,
    LOCK,
    LODSB,
    LODSW,
    LOOP,
    LOOPNE,
    LOOPE,
    MOV,
    MOVSB,
    MOVSW,
    MUL,
    NEG,
    NOT,
    OR,
    OUT,
    POP,
    POPF,
    PUSH,
    PUSHF,
    RCL,
    RCR,
    REP,
    REPNE,
    REPE,
    RETF,
    RETN,
    ROL,
    ROR,
    SAHF,
    SALC,
    SAR,
    SBB,
    SCASB,
    SCASW,
    SHL,
    SHR,
    STC,
    STD,
    STI,
    STOSB,
    STOSW,
    SUB,
    TEST,
    XCHG,
    XLAT,
    XOR,
}

impl Default for Opcode {
    fn default() -> Self { Opcode::InvalidOpcode }
}

fn opcode_to_str(op: Opcode) -> &'static str {
    match op {
        Opcode::NOP => "NOP",
        Opcode::AAA => "AAA",
        Opcode::AAD => "AAD",
        Opcode::AAM => "AAM",
        Opcode::AAS => "AAS",
        Opcode::ADC => "ADC",
        Opcode::ADD => "ADD",
        Opcode::AND => "AND",
        Opcode::CALL => "CALL",
        Opcode::CALLF => "CALLF",
        Opcode::CBW => "CBW",
        Opcode::CLC => "CLC",
        Opcode::CLD => "CLD",
        Opcode::CLI => "CLI",
        Opcode::CMC => "CMC",
        Opcode::CMP => "CMP",
        Opcode::CMPSB => "CMPSB",
        Opcode::CMPSW => "CMPSW",
        Opcode::CWD => "CWD",
        Opcode::DAA => "DAA",
        Opcode::DAS => "DAS",
        Opcode::DEC => "DEC",
        Opcode::DIV => "DIV",
        Opcode::ESC => "ESC",
        Opcode::FWAIT => "FWAIT",
        Opcode::HLT => "HLT",
        Opcode::IDIV => "IDIV",
        Opcode::IMUL => "IMUL",
        Opcode::IN => "IN",
        Opcode::INC => "INC",
        Opcode::INT => "INT",
        Opcode::INT3 => "INT3",
        Opcode::INTO => "INTO",
        Opcode::IRET => "IRET",
        Opcode::JB => "JB",
        Opcode::JBE => "JBE",
        Opcode::JCXZ => "JCXZ",
        Opcode::JL => "JL",
        Opcode::JLE => "JLE",
        Opcode::JMP => "JMP",
        Opcode::JMPF => "JMPF",
        Opcode::JNB => "JNB",
        Opcode::JNBE => "JNBE",
        Opcode::JNL => "JNL",
        Opcode::JNLE => "JNLE",
        Opcode::JNO => "JNO",
        Opcode::JNP => "JNP",
        Opcode::JNS => "JNS",
        Opcode::JNZ => "JNZ",
        Opcode::JO => "JO",
        Opcode::JP => "JP",
        Opcode::JS => "JS",
        Opcode::JZ => "JZ",
        Opcode::LAHF => "LAHF",
        Opcode::LDS => "LDS",
        Opcode::LEA => "LEA",
        Opcode::LES => "LES",
        Opcode::LOCK => "LOCK",
        Opcode::LODSB => "LODSB",
        Opcode::LODSW => "LODSW",
        Opcode::LOOP => "LOOP",
        Opcode::LOOPNE => "LOOPNE",
        Opcode::LOOPE => "LOOPE",
        Opcode::MOV => "MOV",
        Opcode::MOVSB => "MOVSB",
        Opcode::MOVSW => "MOVSW",
        Opcode::MUL => "MUL",
        Opcode::NEG => "NEG",
        Opcode::NOT => "NOT",
        Opcode::OR => "OR",
        Opcode::OUT => "OUT",
        Opcode::POP => "POP",
        Opcode::POPF => "POPF",
        Opcode::PUSH => "PUSH",
        Opcode::PUSHF => "PUSHF",
        Opcode::RCL => "RCL",
        Opcode::RCR => "RCR",
        Opcode::REP => "REP",
        Opcode::REPNE => "REPNE",
        Opcode::REPE => "REPE",
        Opcode::RETF => "RETF",
        Opcode::RETN => "RETN",
        Opcode::ROL => "ROL",
        Opcode::ROR => "ROR",
        Opcode::SAHF => "SAHF",
        Opcode::SALC => "SALC",
        Opcode::SAR => "SAR",
        Opcode::SBB => "SBB",
        Opcode::SCASB => "SCASB",
        Opcode::SCASW => "SCASW",
        Opcode::SHL => "SHL",
        Opcode::SHR => "SHR",
        Opcode::STC => "STC",
        Opcode::STD => "STD",
        Opcode::STI => "STI",
        Opcode::STOSB => "STOSB",
        Opcode::STOSW => "STOSW",
        Opcode::SUB => "SUB",
        Opcode::TEST => "TEST",
        Opcode::XCHG => "XCHG",
        Opcode::XLAT => "XLAT",
        Opcode::XOR => "XOR",
        _ => "INVALID",
    }
}

#[derive(Copy, Clone)]
#[derive(PartialEq)]
pub enum Register8 {
    AL,
    CL,
    DL,
    BL,
    AH,
    CH,
    DH,
    BH
}

#[derive(Copy, Clone)]
#[derive(PartialEq)]
pub enum Register16 {
    AX, 
    CX,
    DX,
    BX,
    SP,
    BP,
    SI,
    DI,
    ES,
    CS,
    SS,
    DS,
    IP,
    InvalidRegister
}

#[derive(Copy, Clone)]
pub enum AddressingMode {
    BxSi,
    BxDi,
    BpSi,
    BpDi,
    Si,
    Di,
    Disp16(Displacement),
    Bx,
    BxSiDisp8(Displacement),
    BxDiDisp8(Displacement),
    BpSiDisp8(Displacement),
    BpDiDisp8(Displacement),
    SiDisp8(Displacement),
    DiDisp8(Displacement),
    BpDisp8(Displacement),
    BxDisp8(Displacement),
    BxSiDisp16(Displacement),
    BxDiDisp16(Displacement),
    BpSiDisp16(Displacement),
    BpDiDisp16(Displacement),
    SiDisp16(Displacement),
    DiDisp16(Displacement),
    BpDisp16(Displacement),
    BxDisp16(Displacement),
    RegisterMode
}

#[derive(Copy, Clone)]
pub enum OperandType {
    Immediate8(u8),
    Immediate16(u16),
    Relative8(i8),
    Relative16(i16),
    Offset8(u16),
    Offset16(u16),
    Register8(Register8),
    Register16(Register16),
    AddressingMode(AddressingMode),
    NearAddress(u16),
    FarAddress(u16,u16),
    NoOperand,
    InvalidOperand
}

//#[derive(Copy, Clone)]
//pub enum OperandTemplate {
//    NoTemplate,
//    ModRM8andImmediate8,
//    ModRM16andImmediate16,
//    ModRM8andRegister8,
//    ModRM16andRegister16,
//    Register8andModRM8,
//    Register16andModRM16,
//    ALandImmediate8,
//    AXandImmediate16,
//    Register16EncodedNoOps,
//    Register8EncodedImm8,
//    Register16EncodedImm16,
//}

#[derive(Copy, Clone)]
#[derive(PartialEq)]
pub enum OperandTemplate {
    NoTemplate,
    NoOperand,
    ModRM8,
    ModRM16,
    Register8,
    Register16,
    Register8Encoded,
    Register16Encoded,
    Immediate8,
    Immediate16,
    Relative8,
    Relative16,
    Offset8,
    Offset16,
    FixedRegister8(Register8),
    FixedRegister16(Register16),
    NearAddress,
    FarAddress
}

#[derive(Copy, Clone)]
pub enum SegmentOverride {
    NoOverride,
    SegmentES,
    SegmentCS,
    SegmentSS,
    SegmentDS
}

#[derive(Copy, Clone)]
pub enum OperandSize {
    NoOperand,
    NoSize,
    Operand8,
    Operand16
}

#[derive(Copy, Clone)]
pub enum OperandSelect {
    FirstOperand,
    SecondOperand
}

#[derive(Copy, Clone)]
pub enum DispType {
    NoDisp,
    Disp8,
    Disp16,
}

#[derive(Copy, Clone)]
pub enum Displacement {
    NoDisp,
    Disp8(i8),
    Disp16(i16),
}

impl Displacement {
    pub fn get_i16(&self) -> i16 {
        match self {
            Displacement::NoDisp => 0,
            Displacement::Disp8(disp) => *disp as i16,
            Displacement::Disp16(disp) => *disp
        }
    }
    pub fn get_u16(&self) -> u16 {
        match self {
            Displacement::NoDisp => 0,
            Displacement::Disp8(disp) => (*disp as i16) as u16,
            Displacement::Disp16(disp) => *disp as u16
        }        
    }
}

impl fmt::Display for Displacement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Displacement::NoDisp => write!(f,"Invalid Displacement"),
            Displacement::Disp8(i) => write!(f,"{:#04x}", i),
            Displacement::Disp16(i) => write!(f,"{:#06x}", i),
        }
    }
}

pub struct ModRmByte {
    byte: u8,
    b_mod: u8,
    b_reg: u8,
    b_rm:  u8,
    addressing_mode: AddressingMode
}

impl Default for ModRmByte {
    fn default() -> Self {
        Self {
            byte: 0,
            b_mod: 0,
            b_reg: 0,
            b_rm: 0,
            addressing_mode: AddressingMode::BxSi
        }
    }
}

impl ModRmByte {
    fn read_from(bytes: &mut impl ByteInterface, cost: &mut u32) -> Result<ModRmByte, Box<dyn std::error::Error>> {
        let mut cycle_cost = 0;
        let byte = bytes.read_u8(&mut cycle_cost);
        let mut displacement = Displacement::NoDisp;

        // The 'mod' field is two bits and along with the r/m field, specifies the general addressing mode,
        // including the size of any displacement. First we determine the size of the displacement, if any,
        // and read the displacement value. 
        let b_mod = (byte >> 6) & 0x03;

        match b_mod {
            0b00 => {
                // Addressing mode [disp16] is a single mode of 0b00
                if byte & MODRM_ADDR_MASK == MODRM_ADDR_DISP16 {
                    let tdisp = bytes.read_i16(&mut cycle_cost);
                    displacement = Displacement::Disp16(tdisp);
                }
            },
            0b01 => {
                // 0b01 signifies an 8 bit displacement (sign-extended to 16)
                let tdisp = bytes.read_i8(&mut cycle_cost);
                displacement = Displacement::Disp8(tdisp);
            } 
            0b10 => {
                // 0b10 signifies a 16 bit displacement
                let tdisp = bytes.read_i16(&mut cycle_cost);
                displacement = Displacement::Disp16(tdisp);
            }
            _ => displacement = Displacement::NoDisp,            
        }

        // Set the addressing mode based on the cominbation of Mod and R/M bitfields + Displacement
        let addressing_mode = match byte & MODRM_ADDR_MASK {
            MODRM_ADDR_BX_SI=>       AddressingMode::BxSi,
            MODRM_ADDR_BX_DI=>       AddressingMode::BxDi,
            MODRM_ADDR_BP_SI=>       AddressingMode::BpSi,
            MODRM_ADDR_BP_DI=>       AddressingMode::BpDi,
            MODRM_ADDR_SI=>          AddressingMode::Si,
            MODRM_ADDR_DI =>         AddressingMode::Di,
            MODRM_ADDR_DISP16=>      AddressingMode::Disp16(displacement),
            MODRM_ADDR_BX =>         AddressingMode::Bx,
            MODRM_ADDR_BX_SI_DISP8=> AddressingMode::BxSiDisp8(displacement),
            MODRM_ADDR_BX_DI_DISP8=> AddressingMode::BxDiDisp8(displacement),
            MODRM_ADDR_BP_SI_DISP8=> AddressingMode::BpSiDisp8(displacement),
            MODRM_ADDR_BP_DI_DISP8=> AddressingMode::BpDiDisp8(displacement),
            MODRM_ADDR_SI_DI_DISP8=> AddressingMode::SiDisp8(displacement),
            MODRM_ADDR_DI_DISP8=>    AddressingMode::DiDisp8(displacement),
            MODRM_ADDR_BP_DISP8=>    AddressingMode::BpDisp8(displacement),
            MODRM_ADDR_BX_DISP8=>    AddressingMode::BxDisp8(displacement),
            MODRM_ADDR_BX_SI_DISP16=>AddressingMode::BxSiDisp16(displacement),
            MODRM_ADDR_BX_DI_DISP16=>AddressingMode::BxDiDisp16(displacement),
            MODRM_ADDR_BP_SI_DISP16=>AddressingMode::BpSiDisp16(displacement),
            MODRM_ADDR_BP_DI_DISP16=>AddressingMode::BpDiDisp16(displacement),
            MODRM_ADDR_SI_DI_DISP16=>AddressingMode::SiDisp16(displacement),
            MODRM_ADDR_DI_DISP16=>   AddressingMode::DiDisp16(displacement),
            MODRM_ADDR_BP_DISP16=>   AddressingMode::BpDisp16(displacement),
            MODRM_ADDR_BX_DISP16=>   AddressingMode::BxDisp16(displacement),
            _=> AddressingMode::RegisterMode,
        };        

        // 'REG' field specifies either register operand or opcode extension. There's no way 
        // to know without knowing the opcode, which we don't
        let b_reg: u8 = (byte >> 3) & 0x07;
        // 'R/M' field is last three bits
        let b_rm: u8 = byte & 0x07;

        Ok(ModRmByte {
            byte,
            b_mod,
            b_reg,
            b_rm,
            addressing_mode
        })        
    }

    // Interpret the 'R/M' field as an 8 bit register selector
    fn get_op1_reg8(&self) -> Register8 {
        match self.b_rm {
            0x00 => Register8::AL,
            0x01 => Register8::CL,
            0x02 => Register8::DL,
            0x03 => Register8::BL,
            0x04 => Register8::AH,
            0x05 => Register8::CH,
            0x06 => Register8::DH,
            0x07 => Register8::BH,
            _=> unreachable!("impossible Register8")
        }   
    }
    // Interpret the 'R/M' field as a 16 bit register selector
    fn get_op1_reg16(&self) -> Register16 {
        match self.b_rm {
            0x00 => Register16::AX,
            0x01 => Register16::CX,
            0x02 => Register16::DX,
            0x03 => Register16::BX,
            0x04 => Register16::SP,
            0x05 => Register16::BP,
            0x06 => Register16::SI,
            0x07 => Register16::DI,
            _=> unreachable!("impossible Register16")
        }
    }
    // Interpret the 'REG' field as an 8 bit register selector
    fn get_op2_reg8(&self) -> Register8 {
        match self.b_reg {
            0x00 => Register8::AL,
            0x01 => Register8::CL,
            0x02 => Register8::DL,
            0x03 => Register8::BL,
            0x04 => Register8::AH,
            0x05 => Register8::CH,
            0x06 => Register8::DH,
            0x07 => Register8::BH,
            _=> unreachable!("impossible Register8")
        }
    }
    // Interpret the 'REG' field as a 16 bit register selector
    fn get_op2_reg16(&self) -> Register16 {
        match self.b_reg {
            0x00 => Register16::AX,
            0x01 => Register16::CX,
            0x02 => Register16::DX,
            0x03 => Register16::BX,
            0x04 => Register16::SP,
            0x05 => Register16::BP,
            0x06 => Register16::SI,
            0x07 => Register16::DI,
            _=> unreachable!("impossible Register16")
        }
    }
    // Intepret the 'REG' field as a 16 bit segment register selector
    fn get_op2_segmentreg16(&self) -> Register16 {
        match self.b_reg {
            0x00 => Register16::ES,
            0x01 => Register16::CS,
            0x02 => Register16::SS,
            0x03 => Register16::DS,
            _=> Register16::InvalidRegister
        }
    }
    // Intepret the 'REG' field as a 3 bit opcode extension
    fn get_op_extension(&self) -> u8 {
        self.b_reg
    }
    fn get_addressing_mode(&self) -> AddressingMode {
        self.addressing_mode
    }
}

#[derive (Copy, Clone)]
pub struct Instruction {
    pub(crate) opcode: u8,
    pub(crate) flags: u32,
    pub(crate) prefixes: u32,
    pub(crate) address: u32,
    pub(crate) size: u32,
    pub(crate) mnemonic: Opcode,
    pub(crate) segment_override: SegmentOverride,
    pub(crate) operand1_type: OperandType,
    pub(crate) operand1_size: OperandSize,
    //pub(crate) operand1: u16,
    pub(crate) operand2_type: OperandType,
    pub(crate) operand2_size: OperandSize,
    //pub(crate) operand2: u16,
    pub(crate) is_location: bool
}

impl Default for Instruction {
    fn default() -> Self {
        Self {
            opcode:   0,
            flags:    0,
            prefixes: 0,
            address:  0,
            size:     1,
            mnemonic: Opcode::NOP,
            segment_override: SegmentOverride::NoOverride,
            operand1_type: OperandType::NoOperand,
            operand1_size: OperandSize::NoOperand,
            //operand1: 0,
            operand2_type: OperandType::NoOperand,
            operand2_size: OperandSize::NoOperand,
            //operand2: 0,
            is_location: false,
        }
    }
}

fn prefix_to_string(i: &Instruction ) -> String {

    // Handle REPx prefixes
    if i.prefixes & OPCODE_PREFIX_REP1 != 0 {
        "REPNE".to_string()
    } 
    else if i.prefixes & OPCODE_PREFIX_LOCK != 0 {
        "LOCK".to_string()
    }
    else if i.prefixes & OPCODE_PREFIX_REP2 != 0 {
        match i.opcode {
            0xA4 | 0xA5 | 0xAA | 0xAB | 0xAC | 0xAD => "REP".to_string(),
            0xA6 | 0xA7 | 0xAE | 0xAF => "REPE".to_string(),
            _=>"".to_string(),
        }
    }
    else {
        "".to_string()
    }
}

fn operand_to_string(i: &Instruction, op: OperandSelect) -> String {

    let (op_type, op_size) = match op {
        OperandSelect::FirstOperand=> (i.operand1_type, i.operand1_size),
        OperandSelect::SecondOperand=> (i.operand2_type, i.operand2_size)
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
                SegmentOverride::SegmentES => {
                    segment = "es".to_string();
                }
                SegmentOverride::SegmentCS => {
                    segment = "cs".to_string();
                }
                SegmentOverride::SegmentSS => {
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
                SegmentOverride::SegmentES => {
                    segment = "es".to_string();
                }
                SegmentOverride::SegmentCS => {
                    segment = "cs".to_string();
                }
                SegmentOverride::SegmentSS => {
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
            if let Opcode::LEA = i.mnemonic {
                ptr_prefix = "".to_string()
            }
            // LES and LDS point to a DWORD address 
            if let Opcode::LES | Opcode::LDS = i.mnemonic {
                ptr_prefix = "dword ptr ".to_string()
            }

            let mut segment1 = "ds".to_string();
            let mut segment2 = "ss".to_string();

            // Handle segment override prefixes 
            match i.segment_override {
                SegmentOverride::SegmentES => {
                    segment1 = "es".to_string();
                    segment2 = "es".to_string();
                }
                SegmentOverride::SegmentCS => {
                    segment1 = "cs".to_string();
                    segment2 = "cs".to_string();
                }
                SegmentOverride::SegmentSS => {
                    segment1 = "ss".to_string();
                    segment2 = "ss".to_string();
                }
                SegmentOverride::SegmentDS => {
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
        OperandType::NearAddress(offset) => {
            format!("[{:#06X}]", offset)
        }
        OperandType::FarAddress(segment, offset) => {
            format!("far {:#06X}:{:#06X}", segment, offset)
        }
        OperandType::NoOperand => "".to_string(),
        _=>"".to_string()
    };

    instruction_string
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {

        let mut instruction_string = String::new();
        
        let prefix = prefix_to_string(self);
        let mnemonic = opcode_to_str(self.mnemonic).to_string().to_lowercase();

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

pub fn decode(bytes: &mut impl ByteInterface) -> Result<Instruction, Box<dyn std::error::Error>> {

    let mut operand1_type: OperandType = OperandType::NoOperand;
    let mut operand2_type: OperandType = OperandType::NoOperand;
    let mut operand1_size: OperandSize = OperandSize::NoOperand;
    let mut operand2_size: OperandSize = OperandSize::NoOperand;

    let op_address = bytes.tell() as u32;
    let mut cycle_cost = 0;
    let mut opcode = bytes.read_u8(&mut cycle_cost);

    let mut mnemonic = Opcode::InvalidOpcode;

    let mut operand1_template = OperandTemplate::NoTemplate;
    let mut operand2_template = OperandTemplate::NoTemplate;
    let mut op_size: u32 = 1;
    let mut op_flags: u32 = 0;
    let mut op_prefixes: u32 = 0;
    let mut op_segment_override = SegmentOverride::NoOverride;

    // Read in opcode prefixes until exhausted
    loop {
        // Set flags for all prefixes encountered...
        op_prefixes |= match opcode {
            0x26 => OPCODE_PREFIX_ES_OVERRIDE,
            0x2E => OPCODE_PREFIX_CS_OVERRIDE,
            0x36 => OPCODE_PREFIX_SS_OVERRIDE,
            0x3E => OPCODE_PREFIX_DS_OVERRIDE,
            0x66 => OPCODE_PREFIX_OPERAND_OVERIDE,
            0x67 => OPCODE_PREFIX_ADDRESS_OVERIDE,
            //0x9B => OPCODE_PREFIX_WAIT,
            0xF0 => OPCODE_PREFIX_LOCK,
            0xF2 => OPCODE_PREFIX_REP1,
            0xF3 => OPCODE_PREFIX_REP2,
            _=> {
                break;
            }
        };
        // ... but only store the last segment override prefix seen
        op_segment_override = match opcode {
            0x26 => SegmentOverride::SegmentES,
            0x2E => SegmentOverride::SegmentCS,
            0x36 => SegmentOverride::SegmentSS,
            0x3E => SegmentOverride::SegmentDS,
            _=> op_segment_override
        };

        opcode = bytes.read_u8(&mut cycle_cost);
    }

    // Match templatizeable instructions
    (mnemonic, operand1_template, operand2_template, op_flags) = match opcode {
        0x00 => (Opcode::ADD,  OperandTemplate::ModRM8,   OperandTemplate::Register8,     0),
        0x01 => (Opcode::ADD,  OperandTemplate::ModRM16,   OperandTemplate::Register16,   0),
        0x02 => (Opcode::ADD,  OperandTemplate::Register8,   OperandTemplate::ModRM8,     0),
        0x03 => (Opcode::ADD,  OperandTemplate::Register16,   OperandTemplate::ModRM16,   0),
        0x04 => (Opcode::ADD,  OperandTemplate::FixedRegister8(Register8::AL),   OperandTemplate::Immediate8,    0),
        0x05 => (Opcode::ADD,  OperandTemplate::FixedRegister16(Register16::AX),   OperandTemplate::Immediate16, 0),
        0x06 => (Opcode::PUSH, OperandTemplate::FixedRegister16(Register16::ES),   OperandTemplate::NoOperand,   0),
        0x07 => (Opcode::POP,  OperandTemplate::FixedRegister16(Register16::ES),   OperandTemplate::NoOperand,   0),
        0x08 => (Opcode::OR,   OperandTemplate::ModRM8,    OperandTemplate::Register8,    0),
        0x09 => (Opcode::OR,   OperandTemplate::ModRM16,    OperandTemplate::Register16,  0),
        0x0A => (Opcode::OR,   OperandTemplate::Register8,    OperandTemplate::ModRM8,    0),
        0x0B => (Opcode::OR,   OperandTemplate::Register16,    OperandTemplate::ModRM16,  0),
        0x0C => (Opcode::OR,   OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
        0x0D => (Opcode::OR,   OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0),
        0x0E => (Opcode::PUSH, OperandTemplate::FixedRegister16(Register16::CS),   OperandTemplate::NoOperand,   0),
        0x0F => (Opcode::POP,  OperandTemplate::FixedRegister16(Register16::CS),   OperandTemplate::NoOperand,   0),    
        0x10 => (Opcode::ADC,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    0),
        0x11 => (Opcode::ADC,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  0),
        0x12 => (Opcode::ADC,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    0),
        0x13 => (Opcode::ADC,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  0),
        0x14 => (Opcode::ADC,  OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
        0x15 => (Opcode::ADC,  OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0), 
        0x16 => (Opcode::PUSH, OperandTemplate::FixedRegister16(Register16::SS),   OperandTemplate::NoOperand,   0),
        0x17 => (Opcode::POP,  OperandTemplate::FixedRegister16(Register16::SS),   OperandTemplate::NoOperand,   0), 
        0x18 => (Opcode::SBB,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    0),
        0x19 => (Opcode::SBB,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  0),
        0x1A => (Opcode::SBB,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    0),
        0x1B => (Opcode::SBB,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  0),
        0x1C => (Opcode::SBB,  OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
        0x1D => (Opcode::SBB,  OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0), 
        0x1E => (Opcode::PUSH, OperandTemplate::FixedRegister16(Register16::DS),   OperandTemplate::NoOperand,   0),
        0x1F => (Opcode::POP,  OperandTemplate::FixedRegister16(Register16::DS),   OperandTemplate::NoOperand,   0),   
        0x20 => (Opcode::AND,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    0),
        0x21 => (Opcode::AND,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  0),
        0x22 => (Opcode::AND,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    0),
        0x23 => (Opcode::AND,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  0),
        0x24 => (Opcode::AND,  OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
        0x25 => (Opcode::AND,  OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0), 
        0x27 => (Opcode::DAA,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand, 0),
        0x28 => (Opcode::SUB,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    0),
        0x29 => (Opcode::SUB,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  0),
        0x2A => (Opcode::SUB,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    0),
        0x2B => (Opcode::SUB,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  0),
        0x2C => (Opcode::SUB,  OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
        0x2D => (Opcode::SUB,  OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0), 
        0x2F => (Opcode::DAS,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,  0),
        0x30 => (Opcode::XOR,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    0),
        0x31 => (Opcode::XOR,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  0),
        0x32 => (Opcode::XOR,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    0),
        0x33 => (Opcode::XOR,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  0),
        0x34 => (Opcode::XOR,  OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
        0x35 => (Opcode::XOR,  OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0),
    //  0x36 Segment override prefix
        0x37 => (Opcode::AAA,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,  0),
        0x38 => (Opcode::CMP,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    0),
        0x39 => (Opcode::CMP,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  0),
        0x3A => (Opcode::CMP,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    0),
        0x3B => (Opcode::CMP,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  0),
        0x3C => (Opcode::CMP,  OperandTemplate::FixedRegister8(Register8::AL),    OperandTemplate::Immediate8,    0),
        0x3D => (Opcode::CMP,  OperandTemplate::FixedRegister16(Register16::AX),    OperandTemplate::Immediate16, 0),
        0x3F => (Opcode::AAS,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,  0),
        0x40..=0x47 => (Opcode::INC,  OperandTemplate::Register16Encoded,    OperandTemplate::NoOperand, 0),
        0x48..=0x4F => (Opcode::DEC,  OperandTemplate::Register16Encoded,    OperandTemplate::NoOperand, 0),
        0x50..=0x57 => (Opcode::PUSH, OperandTemplate::Register16Encoded,    OperandTemplate::NoOperand, 0),
        0x58..=0x5F => (Opcode::POP,  OperandTemplate::Register16Encoded,    OperandTemplate::NoOperand, 0),
    //  0x60..=0x6F >= on 8088, these instructions map to 0x70-7F
        0x60 => (Opcode::JO,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x61 => (Opcode::JNO,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x62 => (Opcode::JB,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x63 => (Opcode::JNB,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x64 => (Opcode::JZ,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x65 => (Opcode::JNZ,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x66 => (Opcode::JBE,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x67 => (Opcode::JNBE, OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x68 => (Opcode::JS,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x69 => (Opcode::JNS,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x6A => (Opcode::JP,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x6B => (Opcode::JNP,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x6C => (Opcode::JL,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x6D => (Opcode::JNL,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x6E => (Opcode::JLE,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x6F => (Opcode::JNLE, OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),        
        0x70 => (Opcode::JO,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x71 => (Opcode::JNO,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x72 => (Opcode::JB,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x73 => (Opcode::JNB,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x74 => (Opcode::JZ,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x75 => (Opcode::JNZ,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x76 => (Opcode::JBE,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x77 => (Opcode::JNBE, OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x78 => (Opcode::JS,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x79 => (Opcode::JNS,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x7A => (Opcode::JP,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x7B => (Opcode::JNP,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x7C => (Opcode::JL,   OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x7D => (Opcode::JNL,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x7E => (Opcode::JLE,  OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),
        0x7F => (Opcode::JNLE, OperandTemplate::Relative8,    OperandTemplate::NoOperand,  INSTRUCTION_REL_JUMP),

        0x84 => (Opcode::TEST,  OperandTemplate::ModRM8,    OperandTemplate::Register8,    0),
        0x85 => (Opcode::TEST,  OperandTemplate::ModRM16,    OperandTemplate::Register16,  0),
        0x86 => (Opcode::XCHG,  OperandTemplate::Register8,    OperandTemplate::ModRM8,    0),
        0x87 => (Opcode::XCHG,  OperandTemplate::Register16,    OperandTemplate::ModRM16,  0),
        0x88 => (Opcode::MOV,   OperandTemplate::ModRM8,    OperandTemplate::Register8,    0),
        0x89 => (Opcode::MOV,   OperandTemplate::ModRM16,    OperandTemplate::Register16,  0),
        0x8A => (Opcode::MOV,   OperandTemplate::Register8,    OperandTemplate::ModRM8,    0),
        0x8B => (Opcode::MOV,   OperandTemplate::Register16,    OperandTemplate::ModRM16,  0),

        0x8D => (Opcode::LEA,   OperandTemplate::Register16,   OperandTemplate::ModRM16,   0),
        0x8F => (Opcode::POP,   OperandTemplate::ModRM16,   OperandTemplate::NoOperand,    0),
        0x90 => (Opcode::NOP,   OperandTemplate::NoOperand,   OperandTemplate::NoOperand,  0),
        0x91..=0x97 => (Opcode::XCHG,  OperandTemplate::Register16Encoded,   OperandTemplate::FixedRegister16(Register16::AX),  0),
        0x98 => (Opcode::CBW,   OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
        0x99 => (Opcode::CWD,   OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),
        0x9A => (Opcode::CALLF, OperandTemplate::FarAddress,   OperandTemplate::NoOperand,  0), 
        0x9B => (Opcode::FWAIT, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0), 
        0x9C => (Opcode::PUSHF, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0), 
        0x9D => (Opcode::POPF,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0), 
        0x9E => (Opcode::SAHF,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0), 
        0x9F => (Opcode::LAHF,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0), 
        0xA0 => (Opcode::MOV,   OperandTemplate::FixedRegister8(Register8::AL),   OperandTemplate::Offset8,      0),
        0xA1 => (Opcode::MOV,   OperandTemplate::FixedRegister16(Register16::AX),   OperandTemplate::Offset16,   0),
        0xA2 => (Opcode::MOV,   OperandTemplate::Offset8,   OperandTemplate::FixedRegister8(Register8::AL),      0),
        0xA3 => (Opcode::MOV,   OperandTemplate::Offset16,   OperandTemplate::FixedRegister16(Register16::AX),   0),
        0xA4 => (Opcode::MOVSB, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0), 
        0xA5 => (Opcode::MOVSW, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0), 
        0xA6 => (Opcode::CMPSB, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0), 
        0xA7 => (Opcode::CMPSW, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0),         
        0xA8 => (Opcode::TEST,  OperandTemplate::FixedRegister8(Register8::AL),   OperandTemplate::Immediate8,    0),
        0xA9 => (Opcode::TEST,  OperandTemplate::FixedRegister16(Register16::AX),   OperandTemplate::Immediate16, 0),
        0xAA => (Opcode::STOSB, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0), 
        0xAB => (Opcode::STOSW, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0), 
        0xAC => (Opcode::LODSB, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0), 
        0xAD => (Opcode::LODSW, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0), 
        0xAE => (Opcode::SCASB, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0), 
        0xAF => (Opcode::SCASW, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,   0), 
        0xB0..=0xB7 => (Opcode::MOV,  OperandTemplate::Register8Encoded,   OperandTemplate::Immediate8,   0),
        0xB8..=0xBF => (Opcode::MOV,  OperandTemplate::Register16Encoded,   OperandTemplate::Immediate16, 0),

        0xC2 => (Opcode::RETN, OperandTemplate::Immediate16,   OperandTemplate::NoOperand,  0),
        0xC3 => (Opcode::RETN, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
        0xC4 => (Opcode::LES,  OperandTemplate::Register16,   OperandTemplate::ModRM16,     0),
        0xC5 => (Opcode::LDS,  OperandTemplate::Register16,   OperandTemplate::ModRM16,     0),
        0xC6 => (Opcode::MOV,  OperandTemplate::ModRM8,   OperandTemplate::Immediate8,      0),
        0xC7 => (Opcode::MOV,  OperandTemplate::ModRM16,    OperandTemplate::Immediate16,   0),

        0xCA => (Opcode::RETF, OperandTemplate::Immediate16,   OperandTemplate::NoOperand,   0),
        0xCB => (Opcode::RETF, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,     0),
        0xCC => (Opcode::INT3, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,     0),
        0xCD => (Opcode::INT,  OperandTemplate::Immediate8,    OperandTemplate::NoOperand,   0),
        0xCE => (Opcode::INTO, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,     0),
        0xCF => (Opcode::IRET, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,     0),

        0xD4 => (Opcode::AAM,  OperandTemplate::Immediate8,   OperandTemplate::NoOperand,    0),
        0xD5 => (Opcode::AAD,  OperandTemplate::Immediate8,   OperandTemplate::NoOperand,    0),
        0xD6 => (Opcode::SALC, OperandTemplate::NoOperand,  OperandTemplate::NoOperand,      0),
        0xD7 => (Opcode::XLAT, OperandTemplate::NoOperand,   OperandTemplate::NoOperand,     0),
        // FPU instructions
        0xD8..=0xDF => (Opcode::ESC, OperandTemplate::ModRM16, OperandTemplate::NoOperand, 0),

        0xE0 => (Opcode::LOOPNE, OperandTemplate::Relative8,   OperandTemplate::NoOperand,   INSTRUCTION_REL_JUMP),
        0xE1 => (Opcode::LOOPE,  OperandTemplate::Relative8,   OperandTemplate::NoOperand,   INSTRUCTION_REL_JUMP),
        0xE2 => (Opcode::LOOP, OperandTemplate::Relative8,   OperandTemplate::NoOperand,     INSTRUCTION_REL_JUMP),
        0xE3 => (Opcode::JCXZ, OperandTemplate::Relative8,   OperandTemplate::NoOperand,     INSTRUCTION_REL_JUMP),
        0xE4 => (Opcode::IN,   OperandTemplate::FixedRegister8(Register8::AL),   OperandTemplate::Immediate8,    0),
        0xE5 => (Opcode::IN,   OperandTemplate::FixedRegister16(Register16::AX),   OperandTemplate::Immediate8,   0),
        0xE6 => (Opcode::OUT,  OperandTemplate::Immediate8,   OperandTemplate::FixedRegister8(Register8::AL),  0),
        0xE7 => (Opcode::OUT,  OperandTemplate::Immediate8,   OperandTemplate::FixedRegister16(Register16::AX), 0),
        0xE8 => (Opcode::CALL, OperandTemplate::Relative16,   OperandTemplate::NoOperand,    INSTRUCTION_REL_JUMP),
        0xE9 => (Opcode::JMP,  OperandTemplate::Relative16,   OperandTemplate::NoOperand,    INSTRUCTION_REL_JUMP),
        0xEA => (Opcode::JMPF, OperandTemplate::FarAddress,  OperandTemplate::NoOperand,    0),
        0xEB => (Opcode::JMP,  OperandTemplate::Relative8,   OperandTemplate::NoOperand,     INSTRUCTION_REL_JUMP),
        0xEC => (Opcode::IN,   OperandTemplate::FixedRegister8(Register8::AL),   OperandTemplate::FixedRegister16(Register16::DX),     0),
        0xED => (Opcode::IN,   OperandTemplate::FixedRegister16(Register16::AX),   OperandTemplate::FixedRegister16(Register16::DX),   0),
        0xEE => (Opcode::OUT,  OperandTemplate::FixedRegister16(Register16::DX),   OperandTemplate::FixedRegister8(Register8::AL),     0),
        0xEF => (Opcode::OUT,  OperandTemplate::FixedRegister16(Register16::DX),   OperandTemplate::FixedRegister16(Register16::AX),   0),
        
        0xF4 => (Opcode::HLT,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
        0xF5 => (Opcode::CMC,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
        0xF8 => (Opcode::CLC,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
        0xF9 => (Opcode::STC,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
        0xFA => (Opcode::CLI,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
        0xFB => (Opcode::STI,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
        0xFC => (Opcode::CLD,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
        0xFD => (Opcode::STD,  OperandTemplate::NoOperand,   OperandTemplate::NoOperand,    0),
        // No match to templatizable instruction, handle in next match statement
        _=> (Opcode::InvalidOpcode, OperandTemplate::NoTemplate, OperandTemplate::NoTemplate,  0)
    };
    
    // Handle instructions with opcode extensions
    match opcode {
        0x80 | 0x82 => {
            // MATH Opcode Extensions (0x82 is alias for 0x80):  r/m8, imm8
            op_flags |= INSTRUCTION_HAS_MODRM;
            let modrm = ModRmByte::read_from(bytes, &mut cycle_cost)?;

            let addr_mode = modrm.get_addressing_mode();
            operand1_type = match addr_mode {
                AddressingMode::RegisterMode=> OperandType::Register8(modrm.get_op1_reg8()),
                _=> OperandType::AddressingMode(addr_mode)
            };
            let op_ext = modrm.get_op_extension();
            mnemonic = match op_ext {
                0x00 => Opcode::ADD,
                0x01 => Opcode::OR,
                0x02 => Opcode::ADC,
                0x03 => Opcode::SBB,
                0x04 => Opcode::AND,
                0x05 => Opcode::SUB,
                0x06 => Opcode::XOR,
                0x07 => Opcode::CMP,
                _=>Opcode::InvalidOpcode
            };
            operand1_size = OperandSize::Operand8;
            let operand2 = bytes.read_u8(&mut cycle_cost);
            operand2_type = OperandType::Immediate8(operand2);
        }
        0x81 => {
            // MATH Opcode Extensions: r/m16, imm16
            op_flags |= INSTRUCTION_HAS_MODRM;
            let modrm = ModRmByte::read_from(bytes, &mut cycle_cost)?;
            let addr_mode = modrm.get_addressing_mode();
            operand1_type = match addr_mode {
                AddressingMode::RegisterMode=> OperandType::Register16(modrm.get_op1_reg16()),
                _=> OperandType::AddressingMode(addr_mode)
            };
            let op_ext = modrm.get_op_extension();
            mnemonic = match op_ext {
                0x00 => Opcode::ADD,
                0x01 => Opcode::OR,
                0x02 => Opcode::ADC,
                0x03 => Opcode::SBB,
                0x04 => Opcode::AND,
                0x05 => Opcode::SUB,
                0x06 => Opcode::XOR,
                0x07 => Opcode::CMP,
                _=>Opcode::InvalidOpcode
            };
            operand1_size = OperandSize::Operand16;
            let operand2 = bytes.read_u16(&mut cycle_cost);
            operand2_type = OperandType::Immediate16(operand2);
        }
        0x83 => {
            // MATH Opcode Extensions: r/m16, imm8 (sign-extended)
            // Strictly speaking, OR, AND and XOR are not supported on 8088 in this form, but whatever
            op_flags |= INSTRUCTION_HAS_MODRM;
            let modrm = ModRmByte::read_from(bytes, &mut cycle_cost)?;
            let addr_mode = modrm.get_addressing_mode();
            operand1_type = match addr_mode {
                AddressingMode::RegisterMode=> OperandType::Register16(modrm.get_op1_reg16()),
                _=> OperandType::AddressingMode(addr_mode)
            };
            let op_ext = modrm.get_op_extension();
            mnemonic = match op_ext {
                0x00 => Opcode::ADD,
                0x01 => Opcode::OR,
                0x02 => Opcode::ADC,
                0x03 => Opcode::SBB,
                0x04 => Opcode::AND,
                0x05 => Opcode::SUB,
                0x06 => Opcode::XOR,
                0x07 => Opcode::CMP,
                _=>Opcode::InvalidOpcode
            };
            operand1_size = OperandSize::Operand16;
            let operand2 = bytes.read_u8(&mut cycle_cost);
            operand2_type = OperandType::Immediate8(operand2);
        }
        0x8C => {
            // MOV r/m16*, Sreg
            // *This MOV modrm can only refer to a general purpose register OR memory, and REG field may only refer to segment register
            mnemonic = Opcode::MOV;
            op_flags |= INSTRUCTION_HAS_MODRM;
            let modrm = ModRmByte::read_from(bytes, &mut cycle_cost)?;
            let addr_mode = modrm.get_addressing_mode();
            operand1_type = match addr_mode {
                AddressingMode::RegisterMode=> OperandType::Register16(modrm.get_op1_reg16()),
                _=> OperandType::AddressingMode(modrm.get_addressing_mode())
            };
            operand1_size = OperandSize::Operand16;
            operand2_type = OperandType::Register16(modrm.get_op2_segmentreg16());
        }
        0x8E => {
            // MOV Sreg, r/m16*
            // *This MOV modrm can only refer to a general purpose register OR memory, and REG field may only refer to segment register
            mnemonic = Opcode::MOV;
            op_flags |= INSTRUCTION_HAS_MODRM;
            let modrm = ModRmByte::read_from(bytes, &mut cycle_cost)?;
            let addr_mode = modrm.get_addressing_mode();
            operand2_type = match addr_mode {
                AddressingMode::RegisterMode=> OperandType::Register16(modrm.get_op1_reg16()),
                _=> OperandType::AddressingMode(modrm.get_addressing_mode())
            };
            operand2_size = OperandSize::Operand16;
            operand1_type = OperandType::Register16(modrm.get_op2_segmentreg16());
        }
        0xC0 => {
            // Bitwise opcode extensions - r/m8, imm8
            // This opcode was only supported on 80186 and above
            operand1_size = OperandSize::Operand8;

            op_flags |= INSTRUCTION_HAS_MODRM;
            let modrm = ModRmByte::read_from(bytes, &mut cycle_cost)?;
            let addr_mode = modrm.get_addressing_mode();
            operand1_type = match addr_mode {
                AddressingMode::RegisterMode => OperandType::Register8(modrm.get_op1_reg8()),
                _=> OperandType::AddressingMode(addr_mode)
            };
            let op_ext = modrm.get_op_extension();
            mnemonic = match op_ext {
                0x00 => Opcode::ROL,
                0x01 => Opcode::ROR,
                0x02 => Opcode::RCL,
                0x03 => Opcode::RCR,
                0x04 => Opcode::SHL,
                0x05 => Opcode::SHR,
                0x06 => Opcode::SHL,
                0x07 => Opcode::SAR,
                _=>Opcode::InvalidOpcode
            };

            let operand2 = bytes.read_u8(&mut cycle_cost);
            operand2_type = OperandType::Immediate8(operand2);
        }
        0xC1 => {
            // Bitwise opcode extensions - r/m16, imm8
            // This opcode was only supported on 80186 and above
            operand1_size = OperandSize::Operand16;

            op_flags |= INSTRUCTION_HAS_MODRM;
            let modrm = ModRmByte::read_from(bytes, &mut cycle_cost)?;
            let addr_mode = modrm.get_addressing_mode();
            operand1_type = match addr_mode {
                AddressingMode::RegisterMode => OperandType::Register16(modrm.get_op1_reg16()),
                _=> OperandType::AddressingMode(addr_mode)
            };
            let op_ext = modrm.get_op_extension();
            mnemonic = match op_ext {
                0x00 => Opcode::ROL,
                0x01 => Opcode::ROR,
                0x02 => Opcode::RCL,
                0x03 => Opcode::RCR,
                0x04 => Opcode::SHL,
                0x05 => Opcode::SHR,
                0x06 => Opcode::SHL,
                0x07 => Opcode::SAR,
                _=> Opcode::InvalidOpcode,
            };
            let operand2 = bytes.read_u8(&mut cycle_cost);
            operand2_type = OperandType::Immediate8(operand2);
        }        
        0xD0 => {
            // Bitwise opcode extensions - r/m8, 0x01
            operand1_size = OperandSize::Operand8;

            op_flags |= INSTRUCTION_HAS_MODRM;
            let modrm = ModRmByte::read_from(bytes, &mut cycle_cost)?;
            let addr_mode = modrm.get_addressing_mode();
            operand1_type = match addr_mode {
                AddressingMode::RegisterMode => OperandType::Register8(modrm.get_op1_reg8()),
                _=> OperandType::AddressingMode(addr_mode)
            };
            let op_ext = modrm.get_op_extension();
            mnemonic = match op_ext {
                0x00 => Opcode::ROL,
                0x01 => Opcode::ROR,
                0x02 => Opcode::RCL,
                0x03 => Opcode::RCR,
                0x04 => Opcode::SHL,
                0x05 => Opcode::SHR,
                0x06 => Opcode::SHL,
                0x07 => Opcode::SAR,
                _=>Opcode::InvalidOpcode
            };
            operand2_type = OperandType::Immediate8(0x01);
        }
        0xD1 => {
            // Bitwise opcode extensions
            // Bitwise opcode extensions - r/m16, 0x01
            operand1_size = OperandSize::Operand16;

            op_flags |= INSTRUCTION_HAS_MODRM;
            let modrm = ModRmByte::read_from(bytes, &mut cycle_cost)?;
            let addr_mode = modrm.get_addressing_mode();
            operand1_type = match addr_mode {
                AddressingMode::RegisterMode => OperandType::Register16(modrm.get_op1_reg16()),
                _=> OperandType::AddressingMode(addr_mode)
            };
            let op_ext = modrm.get_op_extension();
            mnemonic = match op_ext {
                0x00 => Opcode::ROL,
                0x01 => Opcode::ROR,
                0x02 => Opcode::RCL,
                0x03 => Opcode::RCR,
                0x04 => Opcode::SHL,
                0x05 => Opcode::SHR,
                0x06 => Opcode::SHL,
                0x07 => Opcode::SAR,
                _=> Opcode::InvalidOpcode,
            };
            operand2_type = OperandType::Immediate8(0x01);
        }
        0xD2 => {
            // Bitwise opcode extensions - r/m8, CL
            operand1_size = OperandSize::Operand8;
            op_flags |= INSTRUCTION_HAS_MODRM;
            let modrm = ModRmByte::read_from(bytes, &mut cycle_cost)?;
            let addr_mode = modrm.get_addressing_mode();
            operand1_type = match addr_mode {
                AddressingMode::RegisterMode => OperandType::Register8(modrm.get_op1_reg8()),
                _=> OperandType::AddressingMode(addr_mode)
            };
            let op_ext = modrm.get_op_extension();
            mnemonic = match op_ext {
                0x00 => Opcode::ROL,
                0x01 => Opcode::ROR,
                0x02 => Opcode::RCL,
                0x03 => Opcode::RCR,
                0x04 => Opcode::SHL,
                0x05 => Opcode::SHR,
                0x06 => Opcode::SHL,
                0x07 => Opcode::SAR,
                _=> Opcode::InvalidOpcode
            };
            operand2_type = OperandType::Register8(Register8::CL);
        }
        0xD3 => {
            // Bitwise opcode extensions - r/m16, CL
            operand1_size = OperandSize::Operand16;
            op_flags |= INSTRUCTION_HAS_MODRM;
            let modrm = ModRmByte::read_from(bytes, &mut cycle_cost)?;
            let addr_mode = modrm.get_addressing_mode();
            operand1_type = match addr_mode {
                AddressingMode::RegisterMode => OperandType::Register16(modrm.get_op1_reg16()),
                _=> OperandType::AddressingMode(addr_mode)
            };
            let op_ext = modrm.get_op_extension();
            mnemonic = match op_ext {
                0x00 => Opcode::ROL,
                0x01 => Opcode::ROR,
                0x02 => Opcode::RCL,
                0x03 => Opcode::RCR,
                0x04 => Opcode::SHL,
                0x05 => Opcode::SHR,
                0x06 => Opcode::SHL,
                0x07 => Opcode::SAR,
               _=> Opcode::InvalidOpcode
            };
            operand2_type = OperandType::Register8(Register8::CL);
        }        
        0xF6 => {
            // Math opcode extensions
            op_flags |= INSTRUCTION_HAS_MODRM;
            let modrm = ModRmByte::read_from(bytes, &mut cycle_cost)?;
            let addr_mode = modrm.get_addressing_mode();
            match addr_mode {
                AddressingMode::RegisterMode => operand1_type = OperandType::Register8(modrm.get_op1_reg8()),
                _=>operand1_type = OperandType::AddressingMode(modrm.get_addressing_mode())
            }
            operand1_size = OperandSize::Operand8;
            let op_ext = modrm.get_op_extension();
            mnemonic = match op_ext {
                0x00 | 0x01 => {
                    // TEST is the only opcode extension that has an immediate value
                    let operand2 = bytes.read_u8(&mut cycle_cost);
                    operand2_type = OperandType::Immediate8(operand2);
                    Opcode::TEST
                }
                0x02 => Opcode::NOT,
                0x03 => Opcode::NEG,
                0x04 => Opcode::MUL,
                0x05 => Opcode::IMUL,
                0x06 => Opcode::DIV,
                0x07 => Opcode::IDIV,
                _=> Opcode::InvalidOpcode
            };              
        }
        0xF7 => {
            // Math opcode extensions
            // Math opcode extensions
            op_flags |= INSTRUCTION_HAS_MODRM;
            let modrm = ModRmByte::read_from(bytes, &mut cycle_cost)?;
            let addr_mode = modrm.get_addressing_mode();
            match addr_mode {
                AddressingMode::RegisterMode => operand1_type = OperandType::Register16(modrm.get_op1_reg16()),
                _=>operand1_type = OperandType::AddressingMode(modrm.get_addressing_mode())
            }
            operand1_size = OperandSize::Operand16;

            let op_ext = modrm.get_op_extension();
            mnemonic = match op_ext {
                0x00 | 0x01 => {
                    // TEST is the only opcode extension that has an immediate value
                    let operand2 = bytes.read_u16(&mut cycle_cost);
                    operand2_type = OperandType::Immediate16(operand2);
                    Opcode::TEST
                }
                0x02 => Opcode::NOT,
                0x03 => Opcode::NEG,
                0x04 => Opcode::MUL,
                0x05 => Opcode::IMUL,
                0x06 => Opcode::DIV,
                0x07 => Opcode::IDIV,
                _=> Opcode::InvalidOpcode
            };               
        }
        0xFE => {
            // INC/DEC opcode extensions - r/m8
            operand1_size = OperandSize::Operand8;
            operand2_type = OperandType::NoOperand;
            op_flags |= INSTRUCTION_HAS_MODRM;
            let modrm = ModRmByte::read_from(bytes, &mut cycle_cost)?;
            let addr_mode = modrm.get_addressing_mode();
            match addr_mode {
                AddressingMode::RegisterMode => operand1_type = OperandType::Register8(modrm.get_op1_reg8()),
                _=>operand1_type = OperandType::AddressingMode(modrm.get_addressing_mode())
            }
            let op_ext = modrm.get_op_extension();
            mnemonic = match op_ext {
                0x00 => Opcode::INC,
                0x01 => Opcode::DEC,
                _=> Opcode::InvalidOpcode
            };            
        }
        0xFF => {
            // INC/DEC/CALL/JMP opcode extensions - r/m16
            operand1_size = OperandSize::Operand16;
            operand2_type = OperandType::NoOperand;
            op_flags |= INSTRUCTION_HAS_MODRM;
            let modrm = ModRmByte::read_from(bytes, &mut cycle_cost)?;
            let addr_mode = modrm.get_addressing_mode();
            match addr_mode {
                AddressingMode::RegisterMode => operand1_type = OperandType::Register16(modrm.get_op1_reg16()),
                _=>operand1_type = OperandType::AddressingMode(modrm.get_addressing_mode())
            }
            let op_ext = modrm.get_op_extension();
            mnemonic = match op_ext {
                0x00 => Opcode::INC,
                0x01 => Opcode::DEC,
                0x02 => Opcode::CALL,
                0x03 => Opcode::CALLF,  // CALLF
                0x04 => Opcode::JMP,
                0x05 => Opcode::JMPF,
                0x06 => Opcode::PUSH,
                _=> Opcode::InvalidOpcode
            }; 
        }
        _ => {
            if let Opcode::InvalidOpcode = mnemonic {
                return Err(Box::new(InstructionDecodeError::UnsupportedOpcode(opcode)));
            }
        }
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
    let mut modrm = Default::default();
    if load_modrm_op1 | load_modrm_op2 {
        op_flags |= INSTRUCTION_HAS_MODRM;
        modrm = ModRmByte::read_from(bytes, &mut cycle_cost)?;
    }
    
    // Match templatized operands. We use a closure to avoid duplicating code for each operand
    let mut match_op = |op_template| -> Result<(OperandType, OperandSize), Box<dyn std::error::Error>> {
        match op_template {

            OperandTemplate::ModRM8 => {
                let addr_mode = modrm.get_addressing_mode();
                let operand_type = match addr_mode {
                    AddressingMode::RegisterMode=> OperandType::Register8(modrm.get_op1_reg8()),
                    _=> OperandType::AddressingMode(addr_mode),
                };
                Ok((operand_type, OperandSize::Operand8))
            }
            OperandTemplate::ModRM16 => {
                let addr_mode = modrm.get_addressing_mode();
                let operand_type = match addr_mode {
                    AddressingMode::RegisterMode=> OperandType::Register16(modrm.get_op1_reg16()),
                    _=> OperandType::AddressingMode(addr_mode)
                };
                Ok((operand_type, OperandSize::Operand16))
            }
            OperandTemplate::Register8 => {
                if op_flags & INSTRUCTION_HAS_MODRM != 0 {
                    let operand_type = OperandType::Register8(modrm.get_op2_reg8());
                    Ok((operand_type, OperandSize::Operand8))
                }
                else {
                    Err(Box::new(InstructionDecodeError::GeneralDecodeError(opcode)))
                }
            }
            OperandTemplate::Register16 => {
                if op_flags & INSTRUCTION_HAS_MODRM != 0 {                
                    let operand_type = OperandType::Register16(modrm.get_op2_reg16());
                    Ok((operand_type, OperandSize::Operand16))
                }
                else {
                    Err(Box::new(InstructionDecodeError::GeneralDecodeError(opcode)))
                }             
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
                let operand = bytes.read_u8(&mut cycle_cost);
                Ok((OperandType::Immediate8(operand), OperandSize::Operand8))
            }
            OperandTemplate::Immediate16 => {
                let operand = bytes.read_u16(&mut cycle_cost);
                Ok((OperandType::Immediate16(operand), OperandSize::Operand16))
            }
            OperandTemplate::Relative8 => {
                let operand = bytes.read_i8(&mut cycle_cost);
                Ok((OperandType::Relative8(operand), OperandSize::Operand8))
            }
            OperandTemplate::Relative16 => {
                let operand = bytes.read_i16(&mut cycle_cost);
                Ok((OperandType::Relative16(operand), OperandSize::Operand16))                
            }
            OperandTemplate::Offset8 => {
                let operand = bytes.read_u16(&mut cycle_cost);
                Ok((OperandType::Offset8(operand), OperandSize::Operand8))
            }
            OperandTemplate::Offset16 => {
                let operand = bytes.read_u16(&mut cycle_cost);
                Ok((OperandType::Offset16(operand), OperandSize::Operand16))
            }
            OperandTemplate::FixedRegister8(r8) => {
                Ok((OperandType::Register8(r8), OperandSize::Operand8))
            }
            OperandTemplate::FixedRegister16(r16) => {
                Ok((OperandType::Register16(r16), OperandSize::Operand16))
            }
            OperandTemplate::NearAddress => {
                let offset = bytes.read_u16(&mut cycle_cost);
                Ok((OperandType::NearAddress(offset), OperandSize::NoSize))
            }
            OperandTemplate::FarAddress => {
                let offset = bytes.read_u16(&mut cycle_cost);
                let segment = bytes.read_u16(&mut cycle_cost);
                Ok((OperandType::FarAddress(segment,offset), OperandSize::NoSize))
            }
            _=>Ok((OperandType::NoOperand,OperandSize::NoOperand))
        }
    };

    match operand1_template {
        OperandTemplate::NoTemplate => (),
        _=> (operand1_type, operand1_size) = match_op(operand1_template)?
    }
  
    match operand2_template {
        OperandTemplate::NoTemplate => (),
        _=> (operand2_type, operand2_size) = match_op(operand2_template)?
    }

    // Set a flag if either of the instruction operands is a memory operand.
    if let OperandType::AddressingMode(_) = operand1_type {
        op_flags |= INSTRUCTION_USES_MEM;
    }
    if let OperandType::AddressingMode(_) = operand2_type {
        op_flags |= INSTRUCTION_USES_MEM;
    }

    // Cheating here by seeing how many bytes we read, should we be specific about what each opcode size is?
    op_size = bytes.tell() as u32 - op_address;

    if let Opcode::InvalidOpcode = mnemonic {
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



use crate::cpu::*;
use crate::cpu::cpu_addressing::AddressingMode;
use crate::bytequeue::ByteQueue;

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
    pub fn read_from(bytes: &mut impl ByteQueue) -> Result<ModRmByte, Box<dyn std::error::Error>> {
        let mut cycle_cost = 0;
        let byte = bytes.q_read_u8();
        let mut displacement = Displacement::NoDisp;

        // The 'mod' field is two bits and along with the r/m field, specifies the general addressing mode,
        // including the size of any displacement. First we determine the size of the displacement, if any,
        // and read the displacement value. 
        let b_mod = (byte >> 6) & 0x03;

        match b_mod {
            0b00 => {
                // Addressing mode [disp16] is a single mode of 0b00
                if byte & MODRM_ADDR_MASK == MODRM_ADDR_DISP16 {
                    let tdisp = bytes.q_read_i16();
                    displacement = Displacement::Disp16(tdisp);
                }
            },
            0b01 => {
                // 0b01 signifies an 8 bit displacement (sign-extended to 16)
                let tdisp = bytes.q_read_i8();
                displacement = Displacement::Disp8(tdisp);
            } 
            0b10 => {
                // 0b10 signifies a 16 bit displacement
                let tdisp = bytes.q_read_i16();
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
    pub fn get_op1_reg8(&self) -> Register8 {
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
    pub fn get_op1_reg16(&self) -> Register16 {
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
    pub fn get_op2_reg8(&self) -> Register8 {
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
    pub fn get_op2_reg16(&self) -> Register16 {
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
    pub fn get_op2_segmentreg16(&self) -> Register16 {
        match self.b_reg {
            0x00 => Register16::ES,
            0x01 => Register16::CS,
            0x02 => Register16::SS,
            0x03 => Register16::DS,
            _=> Register16::InvalidRegister
        }
    }
    // Intepret the 'REG' field as a 3 bit opcode extension
    pub fn get_op_extension(&self) -> u8 {
        self.b_reg
    }
    pub fn get_addressing_mode(&self) -> AddressingMode {
        self.addressing_mode
    }
} 
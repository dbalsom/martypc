
use crate::cpu::*;
use crate::cpu::cpu_addressing::AddressingMode;
use crate::bytequeue::*;

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
    pub fn read_from(bytes: &mut impl ByteQueue, seg_override: SegmentOverride ) -> Result<ModRmByte, Box<dyn std::error::Error>> {
        let mut cycle_cost = 0;
        let byte = bytes.q_read_u8(QueueType::Subsequent);
        let mut displacement = Displacement::NoDisp;

        // Set the addressing mode based on the cominbation of Mod and R/M bitfields + Displacement
        let (mut pre_disp_cost, post_disp_cost) = match byte & MODRM_ADDR_MASK {
            MODRM_ADDR_BX_SI =>        (4,0),
            MODRM_ADDR_BX_DI =>        (5,0),
            MODRM_ADDR_BP_SI =>        (5,0),
            MODRM_ADDR_BP_DI =>        (4,0),
            MODRM_ADDR_SI =>           (3,0),
            MODRM_ADDR_DI =>           (3,0),
            MODRM_ADDR_DISP16 =>       (4,0),
            MODRM_ADDR_BX =>           (3,0),
            MODRM_ADDR_BX_SI_DISP8 =>  (4,4), // Oddly, fetching an 8-bit displacement takes longer than 16-bit!
            MODRM_ADDR_BX_DI_DISP8 =>  (5,4), // This is due to an extra jump at microcode line 1de.
            MODRM_ADDR_BP_SI_DISP8 =>  (5,4),
            MODRM_ADDR_BP_DI_DISP8 =>  (4,4),
            MODRM_ADDR_DI_DISP8 =>     (2,4),
            MODRM_ADDR_SI_DISP8 =>     (2,4),
            MODRM_ADDR_BP_DISP8=>      (2,4),
            MODRM_ADDR_BX_DISP8 =>     (2,4),
            MODRM_ADDR_BX_SI_DISP16 => (4,2),
            MODRM_ADDR_BX_DI_DISP16 => (5,2),
            MODRM_ADDR_BP_SI_DISP16 => (5,2),
            MODRM_ADDR_BP_DI_DISP16 => (4,2),
            MODRM_ADDR_SI_DISP16 =>    (2,2),
            MODRM_ADDR_DI_DISP16 =>    (2,2),
            MODRM_ADDR_BP_DISP16 =>    (2,2),
            MODRM_ADDR_BX_DISP16 =>    (2,2),
            _=> (0,0)
        };   

        // Segment override costs 2 cycles during EA calculation
        match seg_override {
            SegmentOverride::None => {}
            _ => {
                pre_disp_cost += 2;
            }
        }

        // Spend cycles calculating EA
        bytes.wait(pre_disp_cost);

        // The 'mod' field is two bits and along with the r/m field, specifies the general addressing mode,
        // including the size of any displacement. First we determine the size of the displacement, if any,
        // and read the displacement value. 
        let b_mod = (byte >> 6) & 0x03;

        match b_mod {
            0b00 => {
                // Addressing mode [disp16] is a single mode of 0b00
                if byte & MODRM_ADDR_MASK == MODRM_ADDR_DISP16 {
                    let tdisp = bytes.q_read_i16(QueueType::Subsequent);
                    displacement = Displacement::Disp16(tdisp);
                    bytes.wait(post_disp_cost);
                }
            },
            0b01 => {
                // 0b01 signifies an 8 bit displacement (sign-extended to 16)
                let tdisp = bytes.q_read_i8(QueueType::Subsequent);
                displacement = Displacement::Disp8(tdisp);
                bytes.wait(post_disp_cost);
            } 
            0b10 => {
                // 0b10 signifies a 16 bit displacement
                let tdisp = bytes.q_read_i16(QueueType::Subsequent);
                displacement = Displacement::Disp16(tdisp);
                bytes.wait(post_disp_cost);
            }
            _ => displacement = Displacement::NoDisp,            
        }

        // Set the addressing mode based on the cominbation of Mod and R/M bitfields + Displacement
        let (addressing_mode, _ ) = match byte & MODRM_ADDR_MASK {
            MODRM_ADDR_BX_SI =>        (AddressingMode::BxSi, 5),
            MODRM_ADDR_BX_DI =>        (AddressingMode::BxDi, 6),
            MODRM_ADDR_BP_SI =>        (AddressingMode::BpSi, 6),
            MODRM_ADDR_BP_DI =>        (AddressingMode::BpDi, 5),
            MODRM_ADDR_SI =>           (AddressingMode::Si, 3),
            MODRM_ADDR_DI =>           (AddressingMode::Di, 3),
            MODRM_ADDR_DISP16 =>       (AddressingMode::Disp16(displacement), 4),
            MODRM_ADDR_BX =>           (AddressingMode::Bx, 3),
            MODRM_ADDR_BX_SI_DISP8 =>  (AddressingMode::BxSiDisp8(displacement), 9),
            MODRM_ADDR_BX_DI_DISP8 =>  (AddressingMode::BxDiDisp8(displacement), 10),
            MODRM_ADDR_BP_SI_DISP8 =>  (AddressingMode::BpSiDisp8(displacement), 10),
            MODRM_ADDR_BP_DI_DISP8 =>  (AddressingMode::BpDiDisp8(displacement), 9),
            MODRM_ADDR_SI_DISP8 =>     (AddressingMode::SiDisp8(displacement), 7),
            MODRM_ADDR_DI_DISP8 =>     (AddressingMode::DiDisp8(displacement), 7),
            MODRM_ADDR_BP_DISP8=>      (AddressingMode::BpDisp8(displacement), 7),
            MODRM_ADDR_BX_DISP8 =>     (AddressingMode::BxDisp8(displacement), 7),
            MODRM_ADDR_BX_SI_DISP16 => (AddressingMode::BxSiDisp16(displacement), 9),
            MODRM_ADDR_BX_DI_DISP16 => (AddressingMode::BxDiDisp16(displacement), 10),
            MODRM_ADDR_BP_SI_DISP16 => (AddressingMode::BpSiDisp16(displacement), 10),
            MODRM_ADDR_BP_DI_DISP16 => (AddressingMode::BpDiDisp16(displacement), 9),
            MODRM_ADDR_SI_DISP16 =>    (AddressingMode::SiDisp16(displacement), 7),
            MODRM_ADDR_DI_DISP16 =>    (AddressingMode::DiDisp16(displacement), 7),
            MODRM_ADDR_BP_DISP16 =>    (AddressingMode::BpDisp16(displacement), 7),
            MODRM_ADDR_BX_DISP16 =>    (AddressingMode::BxDisp16(displacement), 7),
            _=> (AddressingMode::RegisterMode, 0)
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
            0x04 => Register16::ES,
            0x05 => Register16::CS,
            0x06 => Register16::SS,
            0x07 => Register16::DS,            
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
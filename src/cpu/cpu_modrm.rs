
use crate::cpu::*;
use crate::cpu::cpu_addressing::AddressingMode;
use crate::bytequeue::*;

#[derive (Copy, Clone)]
pub struct ModRmByte {
    byte: u8,
    b_mod: u8,
    b_reg: u8,
    b_rm:  u8,
    pre_disp_cost: u8,
    post_disp_cost: u8,
    disp: Displacement,
    addressing_mode: AddressingMode
}

impl Default for ModRmByte {
    fn default() -> Self {
        Self {
            byte: 0,
            b_mod: 0,
            b_reg: 0,
            b_rm: 0,
            pre_disp_cost: 0,
            post_disp_cost: 0,            
            disp: Displacement::NoDisp,
            addressing_mode: AddressingMode::BxSi
        }
    }
}

// Microcode addresses for EA procedures, pre-displacement
const EA_INSTR_TABLE_PRE: [[u16; 5]; 24] = [
    [0x1d4, 0x1d5, 0x1d6, MC_JUMP, MC_NONE], // MODRM_ADDR_BX_SI  
    [0x1da, MC_JUMP, 0x1d8, 0x1d9, MC_JUMP], // MODRM_ADDR_BX_DI  
    [0x1db, MC_JUMP, 0x1d5, 0x1f6, MC_JUMP], // MODRM_ADDR_BP_SI   
    [0x1d7, 0x1d8, 0x1d9, MC_JUMP, MC_NONE], // MODRM_ADDR_BP_DI  
    [0x003, MC_JUMP, MC_NONE, MC_NONE, MC_NONE],   // MODRM_ADDR_SI    
    [0x01f, MC_JUMP, MC_NONE, MC_NONE, MC_NONE],   // MODRM_ADDR_DI     
    [MC_NONE, MC_NONE, MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_DISP16
    [0x037, MC_JUMP, MC_NONE, MC_NONE, MC_NONE],   // MODRM_ADDR_BX
    [0x1d4, 0x1d5, 0x1d6, MC_JUMP, MC_NONE], // MODRM_ADDR_BX_SI_DISP8
    [0x1da, MC_JUMP, 0x1d8, 0x1d9, MC_JUMP], // MODRM_ADDR_BX_DI_DISP8
    [0x1db, MC_JUMP, 0x1d5, 0x1d6, MC_JUMP], // MODRM_ADDR_BP_SI_DISP8
    [0x1d7, 0x1d8, 0x1d9, MC_JUMP, MC_NONE], // MODRM_ADDR_BP_DI_DISP8
    [0x023, MC_JUMP, MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_DI_DISP8
    [0x003, MC_JUMP, MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_SI_DISP8
    [0x023, MC_JUMP, MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_BP_DISP8
    [0x037, MC_JUMP, MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_BX_DISP8
    [0x1d4, 0x1d5, 0x1d6, MC_JUMP, MC_NONE], // MODRM_ADDR_BX_SI_DISP1
    [0x1da, MC_JUMP, 0x1d8, 0x1d9, MC_JUMP], // MODRM_ADDR_BX_DI_DISP1
    [0x1db, MC_JUMP, 0x1d5, 0x1d6, MC_JUMP], // MODRM_ADDR_BP_SI_DISP1
    [0x1d7, 0x1d8, 0x1d9, MC_JUMP, MC_NONE], // MODRM_ADDR_BP_DI_DISP1
    [0x003, MC_JUMP, MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_SI_DISP16
    [0x01f, MC_JUMP, MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_DI_DISP16
    [0x023, MC_JUMP, MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_BP_DISP16
    [0x037, MC_JUMP, MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_BX_DISP16
];

// Microcode addresses for EA procedures, post-displacement
const EA_INSTR_TABLE_POST: [[u16; 3]; 24] = [
    [MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_BX_SI
    [MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_BX_DI  
    [MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_BP_SI  
    [MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_BP_DI  
    [MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_SI     
    [MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_DI      
    [0x1e0, MC_JUMP, MC_NONE], // MODRM_ADDR_DISP16 
    [MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_BX      
    [MC_JUMP, 0x1e0, MC_JUMP], // MODRM_ADDR_BX_SI_DISP8 
    [MC_JUMP, 0x1e0, MC_JUMP], // MODRM_ADDR_BX_DI_DISP8 
    [MC_JUMP, 0x1e0, MC_JUMP], // MODRM_ADDR_BP_SI_DISP8 
    [MC_JUMP, 0x1e0, MC_JUMP], // MODRM_ADDR_BP_DI_DISP8 
    [MC_JUMP, 0x1e0, MC_JUMP], // MODRM_ADDR_DI_DISP8
    [MC_JUMP, 0x1e0, MC_JUMP], // MODRM_ADDR_SI_DISP8
    [MC_JUMP, 0x1e0, MC_JUMP], // MODRM_ADDR_BP_DISP8
    [MC_JUMP, 0x1e0, MC_JUMP], // MODRM_ADDR_BX_DISP8
    [0x1e0, MC_JUMP, MC_NONE], // MODRM_ADDR_BX_DI_DISP16
    [0x1e0, MC_JUMP, MC_NONE], // MODRM_ADDR_BP_SI_DISP16
    [0x1e0, MC_JUMP, MC_NONE], // MODRM_ADDR_BX_SI_DISP16
    [0x1e0, MC_JUMP, MC_NONE], // MODRM_ADDR_BP_DI_DISP16
    [0x1e0, MC_JUMP, MC_NONE], // MODRM_ADDR_SI_DISP16
    [0x1e0, MC_JUMP, MC_NONE], // MODRM_ADDR_DI_DISP16
    [0x1e0, MC_JUMP, MC_NONE], // MODRM_ADDR_BP_DISP16
    [0x1e0, MC_JUMP, MC_NONE], // MODRM_ADDR_BX_DISP16
];

const MODRM_TABLE: [ModRmByte; 256] = {
    let mut table: [ModRmByte; 256] = [ 
        ModRmByte {
            byte: 0,
            b_mod: 0,
            b_reg: 0,
            b_rm: 0,
            pre_disp_cost: 0,
            post_disp_cost: 0,
            disp: Displacement::NoDisp,
            addressing_mode: AddressingMode::BxSi
        }; 256
    ];
    let mut byte = 0;

    loop {
        
        let mut displacement = Displacement::NoDisp;

        let b_mod = (byte >> 6) & 0x03;

        match b_mod {
            0b00 => {
                // Addressing mode [disp16] is a single mode of 0b00
                if byte & MODRM_ADDR_MASK == MODRM_ADDR_DISP16 {
                    displacement = Displacement::Pending16;
                }
            },
            0b01 => {
                // 0b01 signifies an 8 bit displacement (sign-extended to 16)
                displacement = Displacement::Pending8;
            } 
            0b10 => {
                // 0b10 signifies a 16 bit displacement
                displacement = Displacement::Pending16;
            }
            _ => displacement = Displacement::NoDisp,            
        }

        // Set the EA calculation costs for each addressing mode. 
        // We divide these into two values, representing microcode instructions before and after
        // loading the displacement. Time spent loading the displacement itself is dependent on the 
        // state of the prefetch queue, so can't be known ahead of time.
        let (pre_disp_cost, post_disp_cost) = match byte & MODRM_ADDR_MASK {
            MODRM_ADDR_BX_SI =>        (4,0),
            MODRM_ADDR_BX_DI =>        (5,0),
            MODRM_ADDR_BP_SI =>        (5,0),
            MODRM_ADDR_BP_DI =>        (4,0),
            MODRM_ADDR_SI =>           (2,0),
            MODRM_ADDR_DI =>           (2,0),
            MODRM_ADDR_DISP16 =>       (0,2),
            MODRM_ADDR_BX =>           (2,0),
            MODRM_ADDR_BX_SI_DISP8 =>  (4,3), // Oddly, fetching an 8-bit displacement takes longer than 16-bit!
            MODRM_ADDR_BX_DI_DISP8 =>  (5,3), // This is due to an extra jump at microcode line 1de.
            MODRM_ADDR_BP_SI_DISP8 =>  (5,3),
            MODRM_ADDR_BP_DI_DISP8 =>  (4,3),
            MODRM_ADDR_DI_DISP8 =>     (2,3),
            MODRM_ADDR_SI_DISP8 =>     (2,3),
            MODRM_ADDR_BP_DISP8 =>     (2,3),
            MODRM_ADDR_BX_DISP8 =>     (2,3),
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

        // Set the addressing mode based on the cominbation of Mod and R/M bitfields + Displacement.
        let (addressing_mode, displacement) = match byte & MODRM_ADDR_MASK {
            MODRM_ADDR_BX_SI =>        (AddressingMode::BxSi, Displacement::NoDisp),
            MODRM_ADDR_BX_DI =>        (AddressingMode::BxDi, Displacement::NoDisp),
            MODRM_ADDR_BP_SI =>        (AddressingMode::BpSi, Displacement::NoDisp),
            MODRM_ADDR_BP_DI =>        (AddressingMode::BpDi, Displacement::NoDisp),
            MODRM_ADDR_SI =>           (AddressingMode::Si, Displacement::NoDisp),
            MODRM_ADDR_DI =>           (AddressingMode::Di, Displacement::NoDisp),
            MODRM_ADDR_DISP16 =>       (AddressingMode::Disp16(displacement), displacement),
            MODRM_ADDR_BX =>           (AddressingMode::Bx, Displacement::NoDisp),
            MODRM_ADDR_BX_SI_DISP8 =>  (AddressingMode::BxSiDisp8(displacement), displacement),
            MODRM_ADDR_BX_DI_DISP8 =>  (AddressingMode::BxDiDisp8(displacement), displacement),
            MODRM_ADDR_BP_SI_DISP8 =>  (AddressingMode::BpSiDisp8(displacement), displacement),
            MODRM_ADDR_BP_DI_DISP8 =>  (AddressingMode::BpDiDisp8(displacement), displacement),
            MODRM_ADDR_SI_DISP8 =>     (AddressingMode::SiDisp8(displacement), displacement),
            MODRM_ADDR_DI_DISP8 =>     (AddressingMode::DiDisp8(displacement), displacement),
            MODRM_ADDR_BP_DISP8 =>     (AddressingMode::BpDisp8(displacement), displacement),
            MODRM_ADDR_BX_DISP8 =>     (AddressingMode::BxDisp8(displacement), displacement),
            MODRM_ADDR_BX_SI_DISP16 => (AddressingMode::BxSiDisp16(displacement), displacement),
            MODRM_ADDR_BX_DI_DISP16 => (AddressingMode::BxDiDisp16(displacement), displacement),
            MODRM_ADDR_BP_SI_DISP16 => (AddressingMode::BpSiDisp16(displacement), displacement),
            MODRM_ADDR_BP_DI_DISP16 => (AddressingMode::BpDiDisp16(displacement), displacement),
            MODRM_ADDR_SI_DISP16 =>    (AddressingMode::SiDisp16(displacement), displacement),
            MODRM_ADDR_DI_DISP16 =>    (AddressingMode::DiDisp16(displacement), displacement),
            MODRM_ADDR_BP_DISP16 =>    (AddressingMode::BpDisp16(displacement), displacement),
            MODRM_ADDR_BX_DISP16 =>    (AddressingMode::BxDisp16(displacement), displacement),
            _ => (AddressingMode::RegisterMode, Displacement::NoDisp)
        };

        // 'REG' field specifies either register operand or opcode extension. There's no way 
        // to know without knowing the opcode, which we don't
        let b_reg: u8 = (byte >> 3) & 0x07;
        
        // 'R/M' field is last three bits
        let b_rm: u8 = byte & 0x07;

        table[byte as usize] = ModRmByte {
            byte,
            b_mod,
            b_reg,
            b_rm,
            pre_disp_cost,
            post_disp_cost,
            disp: displacement,
            addressing_mode
        };

        if byte < 255 {
            byte += 1;
        }
        else {
            break;
        }
    }

    table
};



impl ModRmByte {

    /// Read the modrm byte and look up the appropriate value from the modrm table.
    /// Load any displacement, then return modrm struct and size of modrm + displacement.
    pub fn read(bytes: &mut impl ByteQueue, seg_override: SegmentOverride ) -> (ModRmByte, u32) {

        let byte = bytes.q_read_u8(QueueType::Subsequent);
        let mut modrm = MODRM_TABLE[byte as usize];
        let mut disp_size = 0;
        /*
        if modrm.post_disp_cost > 0 {
            log::trace!("modrm: {:02X} rm: {} pre disp cost: {:02} table: {:x?}", byte, modrm.b_rm, modrm.pre_disp_cost, &EA_INSTR_TABLE_PRE[(modrm.b_mod << 3 | modrm.b_rm) as usize]);
        }
        */

        // If modrm is an addressing mode, spend cycles in EA calculation 
        if modrm.b_mod != 0b11 {
            bytes.wait_i(1, &[MC_JUMP]);
            bytes.wait_i(modrm.pre_disp_cost as u32, &EA_INSTR_TABLE_PRE[(modrm.b_mod << 3 | modrm.b_rm) as usize]);

            // Load any displacement
            disp_size = ModRmByte::load_displacement(&mut modrm, bytes);

            /*
            if modrm.post_disp_cost > 0 {
                log::trace!("rm: {} post disp cost: {:02} table: {:x?}", modrm.b_rm, modrm.post_disp_cost, &EA_INSTR_TABLE_POST[(modrm.b_mod << 3 | modrm.b_rm) as usize]);
            }
            */
            bytes.wait_i(modrm.post_disp_cost as u32, &EA_INSTR_TABLE_POST[(modrm.b_mod << 3 | modrm.b_rm) as usize]);
        }
        else {
            bytes.wait(1);
        }
        
        (modrm, disp_size + 1)
    }

    /// Load any displacement the modrm might have. The modrm table only has 'pending' displacement values,
    /// which must be resolved.
    pub fn load_displacement(&mut self, bytes: &mut impl ByteQueue) -> u32 {

        let (displacement, size) = match self.disp {

            Displacement::Pending8 => {
                let tdisp = bytes.q_read_i8(QueueType::Subsequent);
                (Displacement::Disp8(tdisp), 1)
            }
            Displacement::Pending16 => {
                let tdisp = bytes.q_read_i16(QueueType::Subsequent);
                (Displacement::Disp16(tdisp), 2)
            }
            _ => (Displacement::NoDisp, 0)
        };

        match &mut self.addressing_mode {
            AddressingMode::Disp16(d) => *d = displacement,
            AddressingMode::BxSiDisp8(d) => *d = displacement,
            AddressingMode::BxDiDisp8(d) => *d = displacement,
            AddressingMode::BpSiDisp8(d) => *d = displacement,
            AddressingMode::BpDiDisp8(d) => *d = displacement,
            AddressingMode::SiDisp8(d) => *d = displacement,
            AddressingMode::DiDisp8(d) => *d = displacement,
            AddressingMode::BpDisp8(d) => *d = displacement,
            AddressingMode::BxDisp8(d) => *d = displacement,
            AddressingMode::BxSiDisp16(d) => *d = displacement,
            AddressingMode::BxDiDisp16(d) => *d = displacement,
            AddressingMode::BpSiDisp16(d) => *d = displacement,
            AddressingMode::BpDiDisp16(d) => *d = displacement,
            AddressingMode::SiDisp16(d) => *d = displacement,
            AddressingMode::DiDisp16(d) => *d = displacement,
            AddressingMode::BpDisp16(d) => *d = displacement,
            AddressingMode::BxDisp16(d) => *d = displacement,
            _ => {}
        }

        size
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
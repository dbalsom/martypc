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
*/

//! Module for handling ModRM bytes in x86 instructions.
//! [ModRmByte] is designed to be `'static`, so that reading a ModRM byte resolves to
//! a static reference in a const table of [ModRmByte], plus a displacement which can
//! later be passed to `ModRmByte::addressing_mode()` for resolution.

use crate::{
    bytequeue::*,
    cpu_808x::*,
    cpu_common::{AddressingMode, Displacement},
};

pub const MODRM_REG_MASK: u8 = 0b00_111_000;
pub const MODRM_ADDR_MASK: u8 = 0b11_000_111;
//pub const MODRM_MOD_MASK:          u8 = 0b11_000_000;

const MODRM_ADDR_BX_SI: u8 = 0b00_000_000;
const MODRM_ADDR_BX_DI: u8 = 0b00_000_001;
const MODRM_ADDR_BP_SI: u8 = 0b00_000_010;
const MODRM_ADDR_BP_DI: u8 = 0b00_000_011;
const MODRM_ADDR_SI: u8 = 0b00_000_100;
const MODRM_ADDR_DI: u8 = 0b00_000_101;
const MODRM_ADDR_DISP16: u8 = 0b00_000_110;
const MODRM_ADDR_BX: u8 = 0b00_000_111;

const MODRM_ADDR_BX_SI_DISP8: u8 = 0b01_000_000;
const MODRM_ADDR_BX_DI_DISP8: u8 = 0b01_000_001;
const MODRM_ADDR_BP_SI_DISP8: u8 = 0b01_000_010;
const MODRM_ADDR_BP_DI_DISP8: u8 = 0b01_000_011;
const MODRM_ADDR_SI_DISP8: u8 = 0b01_000_100;
const MODRM_ADDR_DI_DISP8: u8 = 0b01_000_101;
const MODRM_ADDR_BP_DISP8: u8 = 0b01_000_110;
const MODRM_ADDR_BX_DISP8: u8 = 0b01_000_111;

const MODRM_ADDR_BX_SI_DISP16: u8 = 0b10_000_000;
const MODRM_ADDR_BX_DI_DISP16: u8 = 0b10_000_001;
const MODRM_ADDR_BP_SI_DISP16: u8 = 0b10_000_010;
const MODRM_ADDR_BP_DI_DISP16: u8 = 0b10_000_011;
const MODRM_ADDR_SI_DISP16: u8 = 0b10_000_100;
const MODRM_ADDR_DI_DISP16: u8 = 0b10_000_101;
const MODRM_ADDR_BP_DISP16: u8 = 0b10_000_110;
const MODRM_ADDR_BX_DISP16: u8 = 0b10_000_111;

/*
const MODRM_REG_AX_OR_AL:      u8 = 0b00_000_000;
const MODRM_REG_CX_OR_CL:      u8 = 0b00_000_001;
const MODRM_REG_DX_OR_DL:      u8 = 0b00_000_010;
const MODRM_REG_BX_OR_BL:      u8 = 0b00_000_011;
const MODRM_REG_SP_OR_AH:      u8 = 0b00_000_100;
const MODRM_REG_BP_OR_CH:      u8 = 0b00_000_101;
const MODRM_REG_SI_OR_DH:      u8 = 0b00_000_110;
const MODRM_RED_DI_OR_BH:      u8 = 0b00_000_111;
*/

#[derive(Copy, Clone)]
pub struct ModRmByte {
    _byte: u8,
    b_mod: u8,
    b_reg: u8,
    b_rm: u8,
    pre_disp_cost: u8,
    post_disp_cost: u8,
    disp_mc: u16,
    disp: Displacement,
    addressing_mode: AddressingMode,
}

// Microcode addresses for EA procedures, pre-displacement
const EA_INSTR_TABLE_PRE: [[u16; 5]; 24] = [
    [0x1d4, 0x1d5, 0x1d6, MC_JUMP, MC_NONE],       // MODRM_ADDR_BX_SI
    [0x1da, MC_JUMP, 0x1d8, 0x1d9, MC_JUMP],       // MODRM_ADDR_BX_DI
    [0x1db, MC_JUMP, 0x1d5, 0x1d6, MC_JUMP],       // MODRM_ADDR_BP_SI
    [0x1d7, 0x1d8, 0x1d9, MC_JUMP, MC_NONE],       // MODRM_ADDR_BP_DI
    [0x003, MC_JUMP, MC_NONE, MC_NONE, MC_NONE],   // MODRM_ADDR_SI
    [0x01f, MC_JUMP, MC_NONE, MC_NONE, MC_NONE],   // MODRM_ADDR_DI
    [MC_NONE, MC_NONE, MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_DISP16
    [0x037, MC_JUMP, MC_NONE, MC_NONE, MC_NONE],   // MODRM_ADDR_BX
    [0x1d4, 0x1d5, 0x1d6, MC_JUMP, MC_NONE],       // MODRM_ADDR_BX_SI_DISP8
    [0x1da, MC_JUMP, 0x1d8, 0x1d9, MC_JUMP],       // MODRM_ADDR_BX_DI_DISP8
    [0x1db, MC_JUMP, 0x1d5, 0x1d6, MC_JUMP],       // MODRM_ADDR_BP_SI_DISP8
    [0x1d7, 0x1d8, 0x1d9, MC_JUMP, MC_NONE],       // MODRM_ADDR_BP_DI_DISP8
    [0x003, MC_JUMP, MC_NONE, MC_NONE, MC_NONE],   // MODRM_ADDR_SI_DISP8
    [0x01f, MC_JUMP, MC_NONE, MC_NONE, MC_NONE],   // MODRM_ADDR_DI_DISP8
    [0x023, MC_JUMP, MC_NONE, MC_NONE, MC_NONE],   // MODRM_ADDR_BP_DISP8
    [0x037, MC_JUMP, MC_NONE, MC_NONE, MC_NONE],   // MODRM_ADDR_BX_DISP8
    [0x1d4, 0x1d5, 0x1d6, MC_JUMP, MC_NONE],       // MODRM_ADDR_BX_SI_DISP16
    [0x1da, MC_JUMP, 0x1d8, 0x1d9, MC_JUMP],       // MODRM_ADDR_BX_DI_DISP16
    [0x1db, MC_JUMP, 0x1d5, 0x1d6, MC_JUMP],       // MODRM_ADDR_BP_SI_DISP16
    [0x1d7, 0x1d8, 0x1d9, MC_JUMP, MC_NONE],       // MODRM_ADDR_BP_DI_DISP16
    [0x003, MC_JUMP, MC_NONE, MC_NONE, MC_NONE],   // MODRM_ADDR_SI_DISP16
    [0x01f, MC_JUMP, MC_NONE, MC_NONE, MC_NONE],   // MODRM_ADDR_DI_DISP16
    [0x023, MC_JUMP, MC_NONE, MC_NONE, MC_NONE],   // MODRM_ADDR_BP_DISP16
    [0x037, MC_JUMP, MC_NONE, MC_NONE, MC_NONE],   // MODRM_ADDR_BX_DISP16
];

// Microcode addresses for EA procedures, post-displacement, EA loaded
const EA_INSTR_TABLE_POST: [[u16; 3]; 24] = [
    [MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_BX_SI
    [MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_BX_DI
    [MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_BP_SI
    [MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_BP_DI
    [MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_SI
    [MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_DI
    [MC_JUMP, MC_NONE, MC_NONE], // MODRM_ADDR_DISP16
    [MC_NONE, MC_NONE, MC_NONE], // MODRM_ADDR_BX
    [MC_JUMP, 0x1e0, MC_JUMP],   // MODRM_ADDR_BX_SI_DISP8
    [MC_JUMP, 0x1e0, MC_JUMP],   // MODRM_ADDR_BX_DI_DISP8
    [MC_JUMP, 0x1e0, MC_JUMP],   // MODRM_ADDR_BP_SI_DISP8
    [MC_JUMP, 0x1e0, MC_JUMP],   // MODRM_ADDR_BP_DI_DISP8
    [MC_JUMP, 0x1e0, MC_JUMP],   // MODRM_ADDR_DI_DISP8
    [MC_JUMP, 0x1e0, MC_JUMP],   // MODRM_ADDR_SI_DISP8
    [MC_JUMP, 0x1e0, MC_JUMP],   // MODRM_ADDR_BP_DISP8
    [MC_JUMP, 0x1e0, MC_JUMP],   // MODRM_ADDR_BX_DISP8
    [0x1e0, MC_JUMP, MC_NONE],   // MODRM_ADDR_BX_SI_DISP16
    [0x1e0, MC_JUMP, MC_NONE],   // MODRM_ADDR_BP_DI_DISP16
    [0x1e0, MC_JUMP, MC_NONE],   // MODRM_ADDR_BX_SI_DISP16
    [0x1e0, MC_JUMP, MC_NONE],   // MODRM_ADDR_BP_DI_DISP16
    [0x1e0, MC_JUMP, MC_NONE],   // MODRM_ADDR_SI_DISP16
    [0x1e0, MC_JUMP, MC_NONE],   // MODRM_ADDR_DI_DISP16
    [0x1e0, MC_JUMP, MC_NONE],   // MODRM_ADDR_BP_DISP16
    [0x1e0, MC_JUMP, MC_NONE],   // MODRM_ADDR_BX_DISP16
];

const MODRM_TABLE: [ModRmByte; 256] = {
    let mut table: [ModRmByte; 256] = [ModRmByte {
        _byte: 0,
        b_mod: 0,
        b_reg: 0,
        b_rm: 0,
        pre_disp_cost: 0,
        post_disp_cost: 0,
        disp_mc: 0,
        disp: Displacement::NoDisp,
        addressing_mode: AddressingMode::BxSi,
    }; 256];
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
            }
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
        //
        // Oddly, fetching an 8-bit displacement takes longer than 16-bit!
        // This is due to an extra jump at microcode line 1de.
        let (pre_disp_cost, post_disp_cost, disp_mc) = match byte & MODRM_ADDR_MASK {
            MODRM_ADDR_BX_SI => (4, 0, 0),
            MODRM_ADDR_BX_DI => (5, 0, 0),
            MODRM_ADDR_BP_SI => (5, 0, 0),
            MODRM_ADDR_BP_DI => (4, 0, 0),
            MODRM_ADDR_SI => (2, 0, 0),
            MODRM_ADDR_DI => (2, 0, 0),
            MODRM_ADDR_DISP16 => (0, 1, 0x1DC),
            MODRM_ADDR_BX => (2, 0, 0),
            MODRM_ADDR_BX_SI_DISP8 => (4, 3, 0x1DE),
            MODRM_ADDR_BX_DI_DISP8 => (5, 3, 0x1DE),
            MODRM_ADDR_BP_SI_DISP8 => (5, 3, 0x1DE),
            MODRM_ADDR_BP_DI_DISP8 => (4, 3, 0x1DE),
            MODRM_ADDR_SI_DISP8 => (2, 3, 0x1DE),
            MODRM_ADDR_DI_DISP8 => (2, 3, 0x1DE),
            MODRM_ADDR_BP_DISP8 => (2, 3, 0x1DE),
            MODRM_ADDR_BX_DISP8 => (2, 3, 0x1DE),
            MODRM_ADDR_BX_SI_DISP16 => (4, 2, 0x1DE),
            MODRM_ADDR_BX_DI_DISP16 => (5, 2, 0x1DE),
            MODRM_ADDR_BP_SI_DISP16 => (5, 2, 0x1DE),
            MODRM_ADDR_BP_DI_DISP16 => (4, 2, 0x1DE),
            MODRM_ADDR_SI_DISP16 => (2, 2, 0x1DE),
            MODRM_ADDR_DI_DISP16 => (2, 2, 0x1DE),
            MODRM_ADDR_BP_DISP16 => (2, 2, 0x1DE),
            MODRM_ADDR_BX_DISP16 => (2, 2, 0x1DE),
            _ => (0, 0, 0),
        };

        // Set the addressing mode based on the combination of Mod and R/M bitfields + Displacement.
        let (addressing_mode, displacement) = match byte & MODRM_ADDR_MASK {
            MODRM_ADDR_BX_SI => (AddressingMode::BxSi, Displacement::NoDisp),
            MODRM_ADDR_BX_DI => (AddressingMode::BxDi, Displacement::NoDisp),
            MODRM_ADDR_BP_SI => (AddressingMode::BpSi, Displacement::NoDisp),
            MODRM_ADDR_BP_DI => (AddressingMode::BpDi, Displacement::NoDisp),
            MODRM_ADDR_SI => (AddressingMode::Si, Displacement::NoDisp),
            MODRM_ADDR_DI => (AddressingMode::Di, Displacement::NoDisp),
            MODRM_ADDR_DISP16 => (AddressingMode::Disp16(displacement), displacement),
            MODRM_ADDR_BX => (AddressingMode::Bx, Displacement::NoDisp),
            MODRM_ADDR_BX_SI_DISP8 => (AddressingMode::BxSiDisp8(displacement), displacement),
            MODRM_ADDR_BX_DI_DISP8 => (AddressingMode::BxDiDisp8(displacement), displacement),
            MODRM_ADDR_BP_SI_DISP8 => (AddressingMode::BpSiDisp8(displacement), displacement),
            MODRM_ADDR_BP_DI_DISP8 => (AddressingMode::BpDiDisp8(displacement), displacement),
            MODRM_ADDR_SI_DISP8 => (AddressingMode::SiDisp8(displacement), displacement),
            MODRM_ADDR_DI_DISP8 => (AddressingMode::DiDisp8(displacement), displacement),
            MODRM_ADDR_BP_DISP8 => (AddressingMode::BpDisp8(displacement), displacement),
            MODRM_ADDR_BX_DISP8 => (AddressingMode::BxDisp8(displacement), displacement),
            MODRM_ADDR_BX_SI_DISP16 => (AddressingMode::BxSiDisp16(displacement), displacement),
            MODRM_ADDR_BX_DI_DISP16 => (AddressingMode::BxDiDisp16(displacement), displacement),
            MODRM_ADDR_BP_SI_DISP16 => (AddressingMode::BpSiDisp16(displacement), displacement),
            MODRM_ADDR_BP_DI_DISP16 => (AddressingMode::BpDiDisp16(displacement), displacement),
            MODRM_ADDR_SI_DISP16 => (AddressingMode::SiDisp16(displacement), displacement),
            MODRM_ADDR_DI_DISP16 => (AddressingMode::DiDisp16(displacement), displacement),
            MODRM_ADDR_BP_DISP16 => (AddressingMode::BpDisp16(displacement), displacement),
            MODRM_ADDR_BX_DISP16 => (AddressingMode::BxDisp16(displacement), displacement),
            _ => (AddressingMode::RegisterMode, Displacement::NoDisp),
        };

        // 'REG' field specifies either register operand or opcode extension. There's no way
        // to know without knowing the opcode, which we don't
        let b_reg: u8 = (byte >> 3) & 0x07;

        // 'R/M' field is last three bits
        let b_rm: u8 = byte & 0x07;

        table[byte as usize] = ModRmByte {
            _byte: byte,
            b_mod,
            b_reg,
            b_rm,
            pre_disp_cost,
            post_disp_cost,
            disp_mc,
            disp: displacement,
            addressing_mode,
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
    pub fn default_ref() -> &'static ModRmByte {
        &MODRM_TABLE[0]
    }

    /// Read the modrm byte and look up the appropriate value from the modrm table.
    /// Load any displacement, then return modrm struct and size of modrm + displacement.
    pub fn read(bytes: &mut impl ByteQueue) -> (&'static ModRmByte, Displacement, u32) {
        let byte = bytes.q_read_u8(QueueType::Subsequent, QueueReader::Biu);
        let modrm = &MODRM_TABLE[byte as usize];
        let disp_size;

        // If modrm is an addressing mode, spend cycles in EA calculation
        if modrm.b_mod != 0b11 {
            let disp: Displacement;
            bytes.wait_i(1, &[MC_JUMP]);
            bytes.wait_i(
                modrm.pre_disp_cost as u32,
                &EA_INSTR_TABLE_PRE[(modrm.b_mod << 3 | modrm.b_rm) as usize],
            );

            // Load any displacement
            (disp, disp_size) = ModRmByte::read_displacement(&modrm, bytes);

            bytes.wait_i(
                modrm.post_disp_cost as u32,
                &EA_INSTR_TABLE_POST[(modrm.b_mod << 3 | modrm.b_rm) as usize],
            );
            (modrm, disp, disp_size + 1)
        }
        else {
            (modrm, Displacement::NoDisp, 1)
        }
    }

    /// Read any displacement the modrm might have. Return the Displacement and size of displacement.
    pub fn read_displacement(&self, bytes: &mut impl ByteQueue) -> (Displacement, u32) {
        match self.disp {
            Displacement::Pending8 => {
                bytes.set_pc(self.disp_mc);
                let tdisp = bytes.q_read_i8(QueueType::Subsequent, QueueReader::Biu);
                (Displacement::Disp8(tdisp), 1)
            }
            Displacement::Pending16 => {
                bytes.set_pc(self.disp_mc);
                let tdisp = bytes.q_read_i16(QueueType::Subsequent, QueueReader::Biu);
                (Displacement::Disp16(tdisp), 2)
            }
            _ => (Displacement::NoDisp, 0),
        }
    }

    // Interpret the 'R/M' field as an 8 bit register selector
    #[inline(always)]
    pub fn op1_reg8(&self) -> Register8 {
        REGISTER8_LUT[self.b_rm as usize]
    }
    // Interpret the 'R/M' field as a 16 bit register selector
    #[inline(always)]
    pub fn op1_reg16(&self) -> Register16 {
        REGISTER16_LUT[self.b_rm as usize]
    }
    // Interpret the 'REG' field as an 8 bit register selector
    #[inline(always)]
    pub fn op2_reg8(&self) -> Register8 {
        REGISTER8_LUT[self.b_reg as usize]
    }
    // Interpret the 'REG' field as a 16 bit register selector
    #[inline(always)]
    pub fn op2_reg16(&self) -> Register16 {
        REGISTER16_LUT[self.b_reg as usize]
    }
    // Interpret the 'REG' field as a 16 bit segment register selector
    #[inline(always)]
    pub fn op2_segmentreg16(&self) -> Register16 {
        SREGISTER_LUT[self.b_reg as usize]
    }
    // Interpret the 'REG' field as a 3 bit opcode extension
    #[inline(always)]
    pub fn op_extension(&self) -> u8 {
        self.b_reg
    }
    // Return whether the modrm byte specifies a memory addressing mode
    #[inline(always)]
    pub fn is_addressing_mode(&self) -> bool {
        self.b_mod != 0b11
    }
    /// Produce an [AddressingMode] enum with the provided [Displacement] inserted.
    #[inline(always)]
    pub fn addressing_mode(&self, displacement: Displacement) -> AddressingMode {
        match self.addressing_mode {
            AddressingMode::Disp16(_) => AddressingMode::Disp16(displacement),
            AddressingMode::BxSiDisp8(_) => AddressingMode::BxSiDisp8(displacement),
            AddressingMode::BxDiDisp8(_) => AddressingMode::BxDiDisp8(displacement),
            AddressingMode::BpSiDisp8(_) => AddressingMode::BpSiDisp8(displacement),
            AddressingMode::BpDiDisp8(_) => AddressingMode::BpDiDisp8(displacement),
            AddressingMode::SiDisp8(_) => AddressingMode::SiDisp8(displacement),
            AddressingMode::DiDisp8(_) => AddressingMode::DiDisp8(displacement),
            AddressingMode::BpDisp8(_) => AddressingMode::BpDisp8(displacement),
            AddressingMode::BxDisp8(_) => AddressingMode::BxDisp8(displacement),
            AddressingMode::BxSiDisp16(_) => AddressingMode::BxSiDisp16(displacement),
            AddressingMode::BxDiDisp16(_) => AddressingMode::BxDiDisp16(displacement),
            AddressingMode::BpSiDisp16(_) => AddressingMode::BpSiDisp16(displacement),
            AddressingMode::BpDiDisp16(_) => AddressingMode::BpDiDisp16(displacement),
            AddressingMode::SiDisp16(_) => AddressingMode::SiDisp16(displacement),
            AddressingMode::DiDisp16(_) => AddressingMode::DiDisp16(displacement),
            AddressingMode::BpDisp16(_) => AddressingMode::BpDisp16(displacement),
            AddressingMode::BxDisp16(_) => AddressingMode::BxDisp16(displacement),
            _ => self.addressing_mode,
        }
    }
}

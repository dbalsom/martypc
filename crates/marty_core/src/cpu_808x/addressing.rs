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

    cpu_808x::addressing.rs

    Implements addressing mode and operand loading routines.

*/

use crate::{
    cpu_808x::{biu::*, decode::DECODE, *},
    cpu_common::{operands::OperandSize, AddressingMode, OperandType, Segment},
    cycles_mc,
};

#[derive(Copy, Clone, Debug)]
pub enum FarPtr {
    Offset,
    Segment,
}

#[rustfmt::skip]
impl Intel808x {
    #[allow(dead_code)]
    #[inline]
    fn is_register_mode(mode: AddressingMode) -> bool {
        matches!(mode, AddressingMode::RegisterMode)
    }
    
    #[inline]
    pub fn calc_linear_address(segment: u16, offset: u16) -> u32 {
        (((segment as u32) << 4) + offset as u32) & 0xFFFFFu32
    }

    #[inline]
    pub fn relative_offset_u16(base: u16, offset: i16) -> u16 {
        base.wrapping_add(offset as u16)
    }

    #[inline]
    pub fn calc_linear_address_seg(&self, segment: Segment, offset: u16) -> u32 {
        let segment_val: u16 = match segment {
            Segment::None => 0,
            Segment::ES => self.es,
            Segment::CS => self.cs,
            Segment::DS => self.ds,
            Segment::SS => self.ss,
        };
        (((segment_val as u32) << 4) + offset as u32) & 0xFFFFFu32
    }

    /// Calculate the Effective Address for the given AddressingMode enum
    pub fn calc_effective_address(
        &mut self,
        mode: AddressingMode,
        segment_override: Option<Segment>,
    ) -> (Segment, u16) {
        // Addressing modes that reference BP use the stack segment instead of data segment
        // unless a segment override is present.

        // Override default segments based on prefix
        let segment_base_ds = segment_override.unwrap_or(Segment::DS);
        let segment_base_ss = segment_override.unwrap_or(Segment::SS);

        let (seg, offset) = match mode {
            // All of this relies on 2's compliment arithmetic for signed displacements
            AddressingMode::BxSi                => (segment_base_ds, self.b.x().wrapping_add(self.si)),
            AddressingMode::BxDi                => (segment_base_ds, self.b.x().wrapping_add(self.di)),
            AddressingMode::BpSi                => (segment_base_ss, self.bp.wrapping_add(self.si)),  // BP -> SS default seg
            AddressingMode::BpDi                => (segment_base_ss, self.bp.wrapping_add(self.di)),  // BP -> SS default seg
            AddressingMode::Si                  => (segment_base_ds, self.si),
            AddressingMode::Di                  => (segment_base_ds, self.di),
            AddressingMode::Disp16(disp16)      => (segment_base_ds, disp16.get_u16()),
            AddressingMode::Bx                  => (segment_base_ds, self.b.x()),
            
            AddressingMode::BxSiDisp8(disp8)    => (segment_base_ds, self.b.x().wrapping_add(self.si.wrapping_add(disp8.get_u16()))),
            AddressingMode::BxDiDisp8(disp8)    => (segment_base_ds, self.b.x().wrapping_add(self.di.wrapping_add(disp8.get_u16()))),
            AddressingMode::BpSiDisp8(disp8)    => (segment_base_ss, self.bp.wrapping_add(self.si.wrapping_add(disp8.get_u16()))),  // BP -> SS default seg
            AddressingMode::BpDiDisp8(disp8)    => (segment_base_ss, self.bp.wrapping_add(self.di.wrapping_add(disp8.get_u16()))),  // BP -> SS default seg
            AddressingMode::SiDisp8(disp8)      => (segment_base_ds, self.si.wrapping_add(disp8.get_u16())),
            AddressingMode::DiDisp8(disp8)      => (segment_base_ds, self.di.wrapping_add(disp8.get_u16())),
            AddressingMode::BpDisp8(disp8)      => (segment_base_ss, self.bp.wrapping_add(disp8.get_u16())),    // BP -> SS default seg
            AddressingMode::BxDisp8(disp8)      => (segment_base_ds, self.b.x().wrapping_add(disp8.get_u16())),
            
            AddressingMode::BxSiDisp16(disp16)  => (segment_base_ds, self.b.x().wrapping_add(self.si.wrapping_add(disp16.get_u16()))),
            AddressingMode::BxDiDisp16(disp16)  => (segment_base_ds, self.b.x().wrapping_add(self.di.wrapping_add(disp16.get_u16()))),
            AddressingMode::BpSiDisp16(disp16)  => (segment_base_ss, self.bp.wrapping_add(self.si.wrapping_add(disp16.get_u16()))), // BP -> SS default reg
            AddressingMode::BpDiDisp16(disp16)  => (segment_base_ss, self.bp.wrapping_add(self.di.wrapping_add(disp16.get_u16()))), // BP -> SS default reg
            AddressingMode::SiDisp16(disp16)    => (segment_base_ds, self.si.wrapping_add(disp16.get_u16())),
            AddressingMode::DiDisp16(disp16)    => (segment_base_ds, self.di.wrapping_add(disp16.get_u16())),
            AddressingMode::BpDisp16(disp16)    => (segment_base_ss, self.bp.wrapping_add(disp16.get_u16())),   // BP -> SS default reg
            AddressingMode::BxDisp16(disp16)    => (segment_base_ds, self.b.x().wrapping_add(disp16.get_u16())),

            // The instruction decoder should convert ModRM operands that specify Registers to Register type operands, so
            // in theory this shouldn't happen
            AddressingMode::RegisterMode => panic!("Can't calculate EA for register")
        };

        self.last_ea = offset; // Save last EA to do voodoo when LEA is called with reg, reg operands
        (seg, offset)
    }

    pub fn load_effective_address(&mut self, operand: OperandType) -> Option<u16> {
        if let OperandType::AddressingMode(mode, _) = operand {
            let (_segment, offset) = self.calc_effective_address(mode, None);
            return Some(offset);
        }
        None
    }

    /// Load the EA operand for the current instruction, if applicable
    /// (not all instructions with a mod r/m will load, ie, write-only instructions)
    pub fn load_operand(&mut self) {
        if DECODE[self.i.decode_idx].gdr.loads_ea() {
            // This instruction loads its EA operand. Load and save into OPR.
            let ea_mode: AddressingMode;
            let ea_size;

            match (self.i.operand1_type, self.i.operand2_type) {
                (OperandType::AddressingMode(mode, size), _) => {
                    ea_mode = mode;
                    ea_size = size;
                }
                (_, OperandType::AddressingMode(mode, size)) => {
                    ea_mode = mode;
                    ea_size = size;
                }
                _ => return,
            }

            self.mc_pc = 0x1e0; // EALOAD - 1
            let (segment, offset) = self.calc_effective_address(ea_mode, self.i.segment_override);
            self.trace_comment("EALOAD");

            if ea_size == OperandSize::Operand16 {
                self.ea_opr = self.biu_read_u16(segment, offset, ReadWriteFlag::Normal);
            }
            else {
                self.ea_opr = self.biu_read_u8(segment, offset, ReadWriteFlag::Normal) as u16;
            }
            cycles_mc!(self, 0x1e2, MC_RTN); // Return delay cycle from EALOAD
        }
        
        else if DECODE[self.i.decode_idx].gdr.has_modrm() {
            // The EA calculated by the modrm will not be loaded (ie, we proceed to EADONE instead of EALOAD).
            if self.i.operand1_type.is_address() || self.i.operand2_type.is_address() {
                cycles_mc!(self, 0x1e3, MC_RTN);
            } 
        }
    }


    /// Return the value of any operand as 16-bit value.
    pub fn read_operand(
        &mut self,
        operand: OperandType,
        seg_override: Option<Segment>,
    ) -> u16 {
        match operand {
            OperandType::Immediate8(_imm8) => {
                self.q_read_u8(QueueType::Subsequent, QueueReader::Eu) as u16
            }
            OperandType::Immediate8s(_imm8s) => {
                // Immediate8s operands are sign extended to 16 bits. 
                // This is normally done automatically by the 8088 when reading into the tmpbL 
                // register, which has a special property that sign extends into tmpbH, overwriting 
                // the previous value.
                self.q_read_i8(QueueType::Subsequent, QueueReader::Eu) as i16 as u16
            }
            OperandType::Relative8(_rel8) => {
                self.q_read_i8(QueueType::Subsequent, QueueReader::Eu) as u16
            }
            OperandType::Offset8(_offset8) => {
                let offset = self.q_read_u16(QueueType::Subsequent, QueueReader::Eu);
                let segment = seg_override.unwrap_or(Segment::DS);
                self.biu_read_u8(segment, offset, ReadWriteFlag::Normal) as u16
            }
            OperandType::Register8(reg8) => match reg8 {
                Register8::AH => self.a.h() as u16,
                Register8::AL => self.a.l() as u16,
                Register8::BH => self.b.h() as u16,
                Register8::BL => self.b.l() as u16,
                Register8::CH => self.c.h() as u16,
                Register8::CL => self.c.l() as u16,
                Register8::DH => self.d.h() as u16,
                Register8::DL => self.d.l() as u16,
            }
            OperandType::Immediate16(_imm16) => {
                self.q_read_u16(QueueType::Subsequent, QueueReader::Eu)
            }
            OperandType::Relative16(_rel16) => {
                self.q_read_i16(QueueType::Subsequent, QueueReader::Eu) as u16
            }
            OperandType::Offset16(_offset16) => {
                let offset = self.q_read_u16(QueueType::Subsequent, QueueReader::Eu);
                let segment = seg_override.unwrap_or(Segment::DS);
                self.biu_read_u16(segment, offset, ReadWriteFlag::Normal)
            }
            OperandType::Register16(reg16) => match reg16 {
                Register16::AX => self.a.x(),
                Register16::CX => self.c.x(),
                Register16::DX => self.d.x(),
                Register16::BX => self.b.x(),
                Register16::SP => self.sp,
                Register16::BP => self.bp,
                Register16::SI => self.si,
                Register16::DI => self.di,
                Register16::ES => self.es,
                Register16::CS => self.cs,
                Register16::SS => self.ss,
                Register16::DS => self.ds,
                _ => panic!("read_operand16(): Invalid Register16 operand: {:?}", reg16),
            },
            OperandType::AddressingMode(_mode, _) => {
                // EA has already been loaded before microcode execution, so just return it.
                self.ea_opr
            }
            _ => panic!("read_operand(): Invalid OperandType: {:?}", operand),
        }
    }

    /// Return the value of an 8-bit Operand
    pub fn read_operand8(
        &mut self,
        operand: OperandType,
        seg_override: Option<Segment>,
    ) -> Option<u8> {
        // The operand enums may contain values peeked from instruction fetch. However, for accurate cycle
        // timing, we have to fetch them again now.

        // Originally we would assert that the peeked operand values equal the fetched values, but this can
        // fail with self-modifying code, such as the end credits of 8088MPH.
        match operand {
            OperandType::Immediate8(_imm8) => {
                let byte = self.q_read_u8(QueueType::Subsequent, QueueReader::Eu);
                Some(byte)
            }
            OperandType::Immediate8s(_imm8s) => {
                let byte = self.q_read_i8(QueueType::Subsequent, QueueReader::Eu);
                Some(byte as u8)
            }
            OperandType::Relative8(_rel8) => {
                let byte = self.q_read_i8(QueueType::Subsequent, QueueReader::Eu);
                Some(byte as u8)
            }
            OperandType::Offset8(_offset8) => {
                let offset = self.q_read_u16(QueueType::Subsequent, QueueReader::Eu);
                let segment = seg_override.unwrap_or(Segment::DS);
                let byte = self.biu_read_u8(segment, offset, ReadWriteFlag::Normal);
                Some(byte)
            }
            OperandType::Register8(reg8) => match reg8 {
                Register8::AH => Some(self.a.h()),
                Register8::AL => Some(self.a.l()),
                Register8::BH => Some(self.b.h()),
                Register8::BL => Some(self.b.l()),
                Register8::CH => Some(self.c.h()),
                Register8::CL => Some(self.c.l()),
                Register8::DH => Some(self.d.h()),
                Register8::DL => Some(self.d.l()),
            }
            OperandType::AddressingMode(_mode, _) => {
                // EA operand was already fetched into ea_opr. Return masked byte.
                if self.i.opcode & 0x01 != 0 {
                    panic!("Reading byte operand for word size instruction");
                }
                Some((self.ea_opr & 0xFF) as u8)
            }
            _ => None,
        }
    }

    /// Return the value of a 16-bit Operand
    pub fn read_operand16(
        &mut self,
        operand: OperandType,
        seg_override: Option<Segment>,
    ) -> Option<u16> {
        // The operand enums may contain values peeked from instruction fetch. However, for accurate cycle
        // timing, we have to fetch them again now.

        // Originally we would assert that the peeked operand values equal the fetched values, but this can
        // fail with self-modifying code, such as the end credits of 8088MPH.
        match operand {
            OperandType::Immediate16(_imm16) => {
                let word = self.q_read_u16(QueueType::Subsequent, QueueReader::Eu);
                Some(word)
            }
            OperandType::Relative16(_rel16) => {
                let word = self.q_read_i16(QueueType::Subsequent, QueueReader::Eu);
                Some(word as u16)
            }
            OperandType::Offset16(_offset16) => {
                let offset = self.q_read_u16(QueueType::Subsequent, QueueReader::Eu);
                let segment = seg_override.unwrap_or(Segment::DS);
                let word = self.biu_read_u16(segment, offset, ReadWriteFlag::Normal);

                Some(word)
            }
            // Normally this operand type would be invalid, but we see it with the invalid forms
            // of FE. When reading an 8-bit register with W set, the entire 16-bit register will be
            // returned.
            OperandType::Register8(reg8) => match reg8 {
                Register8::AL => Some(self.a.x()),
                Register8::CL => Some(self.c.x()),
                Register8::DL => Some(self.d.x()),
                Register8::BL => Some(self.b.x()),
                Register8::AH => Some(self.a.x().swap_bytes()),
                Register8::CH => Some(self.c.x().swap_bytes()),
                Register8::DH => Some(self.d.x().swap_bytes()),
                Register8::BH => Some(self.b.x().swap_bytes()),
            }
            OperandType::Register16(reg16) => match reg16 {
                Register16::AX => Some(self.a.x()),
                Register16::CX => Some(self.c.x()),
                Register16::DX => Some(self.d.x()),
                Register16::BX => Some(self.b.x()),
                Register16::SP => Some(self.sp),
                Register16::BP => Some(self.bp),
                Register16::SI => Some(self.si),
                Register16::DI => Some(self.di),
                Register16::ES => Some(self.es),
                Register16::CS => Some(self.cs),
                Register16::SS => Some(self.ss),
                Register16::DS => Some(self.ds),
                _ => panic!("read_operand16(): Invalid Register16 operand: {:?}", reg16),
            },
            // Normally this operand type would be invalid, but we see it with the invalid forms
            // of FE. When reading an 8-bit memory location with W set, the upper byte will be 
            // set to FF.
            OperandType::AddressingMode(_, OperandSize::Operand8) => {
                // EA operand was already fetched into ea_opr. Return it.
                Some(self.ea_opr | 0xFF00)
            }
            OperandType::AddressingMode(_, OperandSize::Operand16) => {
                // EA operand was already fetched into ea_opr. Return it.
                Some(self.ea_opr)
            }
            _ => None,
        }
    }

    /// Load a far address operand from instruction queue and return the segment, offset tuple.
    pub fn read_operand_faraddr(&mut self) -> (u16, u16) {
        let o1 = self.biu_queue_read(QueueType::Subsequent, QueueReader::Eu);
        let o2 = self.biu_queue_read(QueueType::Subsequent, QueueReader::Eu);
        let s1 = self.biu_queue_read(QueueType::Subsequent, QueueReader::Eu);
        let s2 = self.biu_queue_read(QueueType::Subsequent, QueueReader::Eu);

        (
            (s1 as u16) | (s2 as u16) << 8,
            (o1 as u16) | (o2 as u16) << 8,
        )
    }

    pub fn read_operand_farptr(
        &mut self,
        operand: OperandType,
        seg_override: Option<Segment>,
        flag: ReadWriteFlag,
    ) -> Option<(u16, u16)> {
        match operand {
            OperandType::AddressingMode(mode, _) => {
                let offset = self.ea_opr;
                let (segment, ea_offset) = self.calc_effective_address(mode, seg_override);
                let segment = self.biu_read_u16(segment, ea_offset.wrapping_add(2), flag);
                Some((segment, offset))
            }
            OperandType::Register16(_) => {
                // Illegal form of LES/LDS reg/reg uses the last calculated EA.
                let segment_base_ds = self.i.segment_override.unwrap_or(Segment::DS);
                let offset =
                    self.biu_read_u16(segment_base_ds, self.last_ea, ReadWriteFlag::Normal);
                let segment = self.biu_read_u16(
                    segment_base_ds,
                    self.last_ea.wrapping_add(2),
                    ReadWriteFlag::Normal,
                );
                Some((segment, offset))
            }
            _ => None,
        }
    }

    pub fn read_operand_farptr2(
        &mut self,
        operand: OperandType,
        seg_override: Option<Segment>,
        ptr: FarPtr,
        flag: ReadWriteFlag,
    ) -> Option<u16> {
        match operand {
            OperandType::AddressingMode(mode, _) => {
                let (segment, offset) = self.calc_effective_address(mode, seg_override);

                match ptr {
                    FarPtr::Offset => Some(self.biu_read_u16(segment, offset, flag)),
                    FarPtr::Segment => {
                        Some(self.biu_read_u16(segment, offset.wrapping_add(2), flag))
                    }
                }
            }
            OperandType::Register16(_) => {
                // Illegal form of LES/LDS reg/reg uses the last calculated EA.
                let segment_base_ds = self.i.segment_override.unwrap_or(Segment::DS);
                match ptr {
                    FarPtr::Offset => Some(0),
                    FarPtr::Segment => {
                        Some(self.biu_read_u16(segment_base_ds, self.last_ea.wrapping_add(2), flag))
                    }
                }
            }
            _ => None,
        }
    }

    pub fn write_operand(
        &mut self,
        operand: OperandType,
        seg_override: Option<Segment>,
        value: u16,
        flag: ReadWriteFlag,
    ) {
        match operand {
            OperandType::Offset8(_offset8) => {
                let offset = self.q_read_u16(QueueType::Subsequent, QueueReader::Eu);
                self.cycle();
                let segment = seg_override.unwrap_or(Segment::DS);
                self.biu_write_u8(segment, offset, value as u8, flag);
            }
            OperandType::Register8(reg8) => match reg8 {
                Register8::AH => self.set_register8(Register8::AH, value as u8),
                Register8::AL => self.set_register8(Register8::AL, value as u8),
                Register8::BH => self.set_register8(Register8::BH, value as u8),
                Register8::BL => self.set_register8(Register8::BL, value as u8),
                Register8::CH => self.set_register8(Register8::CH, value as u8),
                Register8::CL => self.set_register8(Register8::CL, value as u8),
                Register8::DH => self.set_register8(Register8::DH, value as u8),
                Register8::DL => self.set_register8(Register8::DL, value as u8),
            },
                        OperandType::Offset16(_offset16) => {
                let offset = self.q_read_u16(QueueType::Subsequent, QueueReader::Eu);
                self.cycle();
                let segment = seg_override.unwrap_or(Segment::DS);
                self.biu_write_u16(segment, offset, value, flag);
            }
            OperandType::Register16(reg16) => {
                match reg16 {
                    Register16::AX => self.set_register16(Register16::AX, value),
                    Register16::CX => self.set_register16(Register16::CX, value),
                    Register16::DX => self.set_register16(Register16::DX, value),
                    Register16::BX => self.set_register16(Register16::BX, value),
                    Register16::SP => self.set_register16(Register16::SP, value),
                    Register16::BP => self.set_register16(Register16::BP, value),
                    Register16::SI => self.set_register16(Register16::SI, value),
                    Register16::DI => self.set_register16(Register16::DI, value),
                    Register16::ES => {
                        self.set_register16(Register16::ES, value);
                        //self.interrupt_inhibit = true;
                    },
                    Register16::CS => {
                        self.set_register16(Register16::CS, value);
                        //self.interrupt_inhibit = true;
                    },
                    Register16::SS => {
                        self.set_register16(Register16::SS, value);
                        //self.interrupt_inhibit = true;
                    }
                    Register16::DS => {
                        self.set_register16(Register16::DS, value);
                        //self.interrupt_inhibit = true;
                    },
                    _ => panic!("read_operand16(): Invalid Register16 operand"),
                }
            }
            OperandType::AddressingMode(mode, size) => {
                let (segment, offset) = self.calc_effective_address(mode, seg_override);
                match size {
                    OperandSize::Operand8 => {
                        self.biu_write_u8(segment, offset, value as u8, flag);
                    }
                    OperandSize::Operand16 => {
                        self.biu_write_u16(segment, offset, value, flag);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Write an 8-bit value to the specified destination operand
    pub fn write_operand8(
        &mut self,
        operand: OperandType,
        seg_override: Option<Segment>,
        value: u8,
        flag: ReadWriteFlag,
    ) {
        match operand {
            OperandType::Offset8(_offset8) => {
                let offset = self.q_read_u16(QueueType::Subsequent, QueueReader::Eu);
                self.cycle();
                let segment = seg_override.unwrap_or(Segment::DS);
                self.biu_write_u8(segment, offset, value, flag);
            }
            OperandType::Register8(reg8) => match reg8 {
                Register8::AH => self.set_register8(Register8::AH, value),
                Register8::AL => self.set_register8(Register8::AL, value),
                Register8::BH => self.set_register8(Register8::BH, value),
                Register8::BL => self.set_register8(Register8::BL, value),
                Register8::CH => self.set_register8(Register8::CH, value),
                Register8::CL => self.set_register8(Register8::CL, value),
                Register8::DH => self.set_register8(Register8::DH, value),
                Register8::DL => self.set_register8(Register8::DL, value),
            },
            OperandType::AddressingMode(mode, _) => {
                let (segment, offset) = self.calc_effective_address(mode, seg_override);
                self.biu_write_u8(segment, offset, value, flag);
            }
            _ => {}
        }
    }
    
    pub fn write_operand16(
        &mut self,
        operand: OperandType,
        seg_override: Option<Segment>,
        value: u16,
        flag: ReadWriteFlag,
    ) {
        match operand {
            OperandType::Offset16(_offset16) => {
                let offset = self.q_read_u16(QueueType::Subsequent, QueueReader::Eu);
                self.cycle();
                let segment = seg_override.unwrap_or(Segment::DS);
                self.biu_write_u16(segment, offset, value, flag);
            }
            OperandType::Register16(reg16) => {
                match reg16 {
                    Register16::AX => self.set_register16(Register16::AX, value),
                    Register16::CX => self.set_register16(Register16::CX, value),
                    Register16::DX => self.set_register16(Register16::DX, value),
                    Register16::BX => self.set_register16(Register16::BX, value),
                    Register16::SP => self.set_register16(Register16::SP, value),
                    Register16::BP => self.set_register16(Register16::BP, value),
                    Register16::SI => self.set_register16(Register16::SI, value),
                    Register16::DI => self.set_register16(Register16::DI, value),
                    Register16::ES => {
                        self.set_register16(Register16::ES, value);
                        //self.interrupt_inhibit = true;
                    },
                    Register16::CS => {
                        self.set_register16(Register16::CS, value);
                        //self.interrupt_inhibit = true;
                    },
                    Register16::SS => {
                        self.set_register16(Register16::SS, value);
                        //self.interrupt_inhibit = true;
                    }
                    Register16::DS => {
                        self.set_register16(Register16::DS, value);
                        //self.interrupt_inhibit = true;
                    },
                    _ => panic!("read_operand16(): Invalid Register16 operand"),
                }
            }
            OperandType::AddressingMode(mode, _) => {
                let (segment, offset) = self.calc_effective_address(mode, seg_override);
                self.biu_write_u16(segment, offset, value, flag);
            }
            _ => {}
        }
    }
}

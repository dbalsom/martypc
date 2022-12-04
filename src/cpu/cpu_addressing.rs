use crate::cpu::*;
use crate::bus::BusInterface;

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

impl Cpu {

    fn is_register_mode(mode: AddressingMode) {
        match mode {
            AddressingMode::RegisterMode => true,
            _ => false
        };
    }

    pub fn calc_linear_address(segment: u16, offset: u16) -> u32 {
        ((segment as u32) << 4) + offset as u32 & 0xFFFFFu32
    }
    
    /// Calculate the Effective Address for the given AddressingMode enum
    fn calc_effective_address(&self, mode: AddressingMode, segment: SegmentOverride) -> (u16, u16) {
        // Addressing modes that reference BP use the stack segment instead of data segment 
        // unless a segment override is present.
        // ------------- Mod 0x00
        // ds:[bx+si]
        // ds:[bx+di]
        // ss:[bp+si]
        // ss:[bp+di]
        // ds:[si]f
        // ds:[di]
        // ds:[{}]
        // ds:[bx]
        // -------------- Mod 0x01
        // ds:[bx+si+{}]
        // ds:[bx+di+{}]
        // ss:[bp+si+{}]
        // ss:[bp+di+{}]
        // ds:[si+{}]
        // ds:[di+{}]
        // ss:[bp+{}]
        // ds:[bx+{}]
        // -------------- Mod 0x10
        // ds:[bx+si+{}]
        // ds:[bx+di+{}]
        // ss:[bp+si+{}]
        // ss:[bp+si+{}]
        // ds:[si+{}]
        // ds:[di+{}]
        // ss:[bp+{}]
        // ds:[bx+{}]

        // Override default segments based on prefix
        let segment_base_default_ds: u16 = match segment {
            SegmentOverride::NoOverride => self.ds,
            SegmentOverride::SegmentES => self.es,
            SegmentOverride::SegmentCS => self.cs,
            SegmentOverride::SegmentSS => self.ss,
            SegmentOverride::SegmentDS => self.ds
        };

        let segment_base_default_ss: u16 = match segment {
            SegmentOverride::NoOverride => self.ss,
            SegmentOverride::SegmentES => self.es,
            SegmentOverride::SegmentCS => self.cs,
            SegmentOverride::SegmentSS => self.ss,
            SegmentOverride::SegmentDS => self.ds
        };      

        match mode {
            // All of this relies on 2's compliment arithmetic for signed displacements
            AddressingMode::BxSi                => (segment_base_default_ds, self.bx.wrapping_add(self.si)),
            AddressingMode::BxDi                => (segment_base_default_ds, self.bx.wrapping_add(self.di)),
            AddressingMode::BpSi                => (segment_base_default_ss, self.bp.wrapping_add(self.si)),  // BP -> SS default seg
            AddressingMode::BpDi                => (segment_base_default_ss, self.bp.wrapping_add(self.di)),  // BP -> SS default seg
            AddressingMode::Si                  => (segment_base_default_ds, self.si),
            AddressingMode::Di                  => (segment_base_default_ds, self.di),
            AddressingMode::Disp16(disp16)      => (segment_base_default_ds, disp16.get_u16()),
            AddressingMode::Bx                  => (segment_base_default_ds, self.bx),
            
            AddressingMode::BxSiDisp8(disp8)    => (segment_base_default_ds, self.bx.wrapping_add(self.si.wrapping_add(disp8.get_u16()))),
            AddressingMode::BxDiDisp8(disp8)    => (segment_base_default_ds, self.bx.wrapping_add(self.di.wrapping_add(disp8.get_u16()))),
            AddressingMode::BpSiDisp8(disp8)    => (segment_base_default_ss, self.bp.wrapping_add(self.si.wrapping_add(disp8.get_u16()))),  // BP -> SS default seg
            AddressingMode::BpDiDisp8(disp8)    => (segment_base_default_ss, self.bp.wrapping_add(self.di.wrapping_add(disp8.get_u16()))),  // BP -> SS default seg
            AddressingMode::SiDisp8(disp8)      => (segment_base_default_ds, self.si.wrapping_add(disp8.get_u16())),
            AddressingMode::DiDisp8(disp8)      => (segment_base_default_ds, self.di.wrapping_add(disp8.get_u16())),
            AddressingMode::BpDisp8(disp8)      => (segment_base_default_ss, self.bp.wrapping_add(disp8.get_u16())),    // BP -> SS default seg
            AddressingMode::BxDisp8(disp8)      => (segment_base_default_ds, self.bx.wrapping_add(disp8.get_u16())),
            
            AddressingMode::BxSiDisp16(disp16)  => (segment_base_default_ds, self.bx.wrapping_add(self.si.wrapping_add(disp16.get_u16()))),
            AddressingMode::BxDiDisp16(disp16)  => (segment_base_default_ds, self.bx.wrapping_add(self.di.wrapping_add(disp16.get_u16()))),
            AddressingMode::BpSiDisp16(disp16)  => (segment_base_default_ss, self.bp.wrapping_add(self.si.wrapping_add(disp16.get_u16()))), // BP -> SS default reg
            AddressingMode::BpDiDisp16(disp16)  => (segment_base_default_ss, self.bp.wrapping_add(self.di.wrapping_add(disp16.get_u16()))), // BP -> SS default reg
            AddressingMode::SiDisp16(disp16)    => (segment_base_default_ds, self.si.wrapping_add(disp16.get_u16())),
            AddressingMode::DiDisp16(disp16)    => (segment_base_default_ds, self.di.wrapping_add(disp16.get_u16())),
            AddressingMode::BpDisp16(disp16)    => (segment_base_default_ss, self.bp.wrapping_add(disp16.get_u16())),   // BP -> SS default reg
            AddressingMode::BxDisp16(disp16)    => (segment_base_default_ds, self.bx.wrapping_add(disp16.get_u16())),

            // The instruction decoder should convert ModRM operands that specify Registers to Register type operands, so
            // in theory this shouldn't happen
            AddressingMode::RegisterMode => panic!("Can't calculate EA for register")
        }
    }

    pub fn load_effective_address(&self, operand: OperandType) -> Option<u16> {
        if let OperandType::AddressingMode(mode) = operand {
            let (_segment, offset) = self.calc_effective_address(mode, SegmentOverride::NoOverride);
            return Some(offset);
        }
        None
    }

    /// Return the value of an 8-bit Operand
    // TODO: implement cycle cost
    pub fn read_operand8(&mut self, operand: OperandType, seg_override: SegmentOverride) -> Option<u8> {

        match operand {
            OperandType::Immediate8(imm8) => Some(imm8),
            OperandType::Relative8(rel8) => Some(rel8 as u8),
            OperandType::Offset8(offset8) => {
                let segment_base: u16 = match seg_override {
                    SegmentOverride::SegmentES => self.es,
                    SegmentOverride::SegmentCS => self.cs,
                    SegmentOverride::SegmentSS => self.ss,
                    _ => self.ds
                };
                let flat_addr = Cpu::calc_linear_address(segment_base, offset8);

                //let (byte, _read_cost) = self.bus.read_u8(flat_addr as usize).unwrap();
                let byte = self.biu_read_u8(flat_addr);
                Some(byte)
            },
            OperandType::Register8(reg8) => {
                match reg8 {
                    Register8::AH => Some(self.ah),
                    Register8::AL => Some(self.al),
                    Register8::BH => Some(self.bh),
                    Register8::BL => Some(self.bl),
                    Register8::CH => Some(self.ch),
                    Register8::CL => Some(self.cl),
                    Register8::DH => Some(self.dh),
                    Register8::DL => Some(self.dl)
                }
            },
            OperandType::AddressingMode(mode) => {
                let (segment, offset) = self.calc_effective_address(mode, seg_override);
                let flat_addr = Cpu::calc_linear_address(segment, offset);

                //let (byte, _read_cost) = self.bus.read_u8(flat_addr as usize).unwrap();
                let byte = self.biu_read_u8(flat_addr);
                Some(byte)
            }
            OperandType::NearAddress(_u16) => None,
            OperandType::FarAddress(_segment, _offset) => None,
            OperandType::NoOperand => None,
            OperandType::InvalidOperand => None,
            _=> None
        }
    }

    /// Return the value of a 16-bit Operand
    // TODO: implement cycle cost
    pub fn read_operand16(&mut self, operand: OperandType, seg_override: SegmentOverride) -> Option<u16> {

        match operand {
            OperandType::Immediate16(imm16) => Some(imm16),
            OperandType::Relative16(rel16) => Some(rel16 as u16),
            OperandType::Offset16(offset16) => {
                let segment_base: u16 = match seg_override {
                    SegmentOverride::SegmentES => self.es,
                    SegmentOverride::SegmentCS => self.cs,
                    SegmentOverride::SegmentSS => self.ss,
                    _ => self.ds
                };
                let flat_addr = Cpu::calc_linear_address(segment_base, offset16);

                //let (word, _read_cost) = self.bus.read_u16(flat_addr as usize).unwrap();
                let word = self.biu_read_u16(flat_addr);

                Some(word)
            }
            OperandType::Register16(reg16) => {
                match reg16 {
                    Register16::AX => Some(self.ax),
                    Register16::CX => Some(self.cx),
                    Register16::DX => Some(self.dx),
                    Register16::BX => Some(self.bx),
                    Register16::SP => Some(self.sp),
                    Register16::BP => Some(self.bp),
                    Register16::SI => Some(self.si),
                    Register16::DI => Some(self.di),
                    Register16::ES => Some(self.es),
                    Register16::CS => Some(self.cs),
                    Register16::SS => Some(self.ss),
                    Register16::DS => Some(self.ds),
                    _=> panic!("read_operand16(): Invalid Register16 operand")
                }
            },
            OperandType::AddressingMode(mode) => {
                let (segment, offset) = self.calc_effective_address(mode, seg_override);
                let flat_addr = Cpu::calc_linear_address(segment, offset);

                //let (word, _read_cost) = self.bus.read_u16(flat_addr as usize).unwrap();
                let word = self.biu_read_u16(flat_addr);
                
                Some(word)
            }
            OperandType::NearAddress(_u16) => None,
            OperandType::FarAddress(_segment, _offset) => None,
            OperandType::NoOperand => None,
            OperandType::InvalidOperand => None,
            _ => None
        }
    }    

    pub fn read_operand_farptr(&mut self, operand: OperandType, seg_override: SegmentOverride) -> Option<(u16, u16)> {

        match operand {
            OperandType::AddressingMode(mode) => {
                let (segment, offset) = self.calc_effective_address(mode, seg_override);
                let flat_addr = Cpu::calc_linear_address(segment, offset);
                let (offset, _read_cost) = self.bus.read_u16(flat_addr as usize).unwrap();
                let (segment, _read_cost) = self.bus.read_u16( (flat_addr + 2) as usize ).unwrap();
                Some((segment, offset))
            }
            _ => None
        }
    }    

    /// Write an 8-bit value to the specified destination operand
    pub fn write_operand8(&mut self, operand: OperandType, seg_override: SegmentOverride, value: u8) {

        match operand {
            OperandType::Immediate8(imm8) => {}
            OperandType::Immediate16(imm16) => {}
            OperandType::Relative8(rel8) => {}
            OperandType::Relative16(rel16) => {}
            OperandType::Offset8(offset8) => {
                let segment_base: u16 = match seg_override {
                    SegmentOverride::SegmentES => self.es,
                    SegmentOverride::SegmentCS => self.cs,
                    SegmentOverride::SegmentSS => self.ss,
                    _ => self.ds
                };
                let flat_addr = Cpu::calc_linear_address(segment_base, offset8);

                //let write_cost = self.bus.write_u8(flat_addr as usize, value);
                self.biu_write_u8(flat_addr, value);
            }
            OperandType::Offset16(offset16) => {}
            OperandType::Register8(reg8) => {
                match reg8 {
                    Register8::AH => self.set_register8(Register8::AH, value),
                    Register8::AL => self.set_register8(Register8::AL , value),
                    Register8::BH => self.set_register8(Register8::BH, value),
                    Register8::BL => self.set_register8(Register8::BL, value),
                    Register8::CH => self.set_register8(Register8::CH, value),
                    Register8::CL => self.set_register8(Register8::CL , value),
                    Register8::DH => self.set_register8(Register8::DH , value),
                    Register8::DL => self.set_register8(Register8::DL , value),
                }
            },
            OperandType::Register16(r16) => {}
            OperandType::AddressingMode(mode) => {
                let (segment, offset) = self.calc_effective_address(mode, seg_override);
                let flat_addr = Cpu::calc_linear_address(segment, offset);

                //let write_cost = self.bus.write_u8(flat_addr as usize, value).unwrap();
                self.biu_write_u8(flat_addr, value);
            }
            OperandType::NearAddress(offset) => {}
            OperandType::FarAddress(segment,offset) => {}
            OperandType::NoOperand => {}
            OperandType::InvalidOperand => {}
        }
    }

    // TODO: implement cycle cost
    pub fn write_operand16(&mut self, operand: OperandType, seg_override: SegmentOverride, value: u16) {

        match operand {
            OperandType::Immediate8(imm8) => {}
            OperandType::Immediate16(imm16) => {}
            OperandType::Relative8(rel8) => {}
            OperandType::Relative16(rel16) => {}
            OperandType::Offset8(offset8) => {}
            OperandType::Offset16(offset16) => {
                let segment_base: u16 = match seg_override {
                    SegmentOverride::SegmentES => self.es,
                    SegmentOverride::SegmentCS => self.cs,
                    SegmentOverride::SegmentSS => self.ss,
                    _ => self.ds
                };
                let flat_addr = Cpu::calc_linear_address(segment_base, offset16);
                
                //let write_cost = self.bus.write_u16(flat_addr as usize, value);
                self.biu_write_u16(flat_addr, value);
            }
            OperandType::Register8(reg8) => {}
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
                    Register16::ES => self.set_register16(Register16::ES, value),
                    Register16::CS => self.set_register16(Register16::CS, value),
                    Register16::SS => {
                        self.set_register16(Register16::SS, value);
                        // Technically only MOV ss, nn instructions will inhibit interrupts for one instruction
                        // Other writes may not. 
                        self.interrupt_inhibit = true;
                    },
                    Register16::DS => self.set_register16(Register16::DS, value),
                    _=> panic!("read_operand16(): Invalid Register16 operand")
                }
            }
            OperandType::AddressingMode(mode) => {
                let (segment, offset) = self.calc_effective_address(mode, seg_override);
                let flat_addr = Cpu::calc_linear_address(segment, offset);
                
                //let write_cost = self.bus.write_u16(flat_addr as usize, value).unwrap();
                self.biu_write_u16(flat_addr, value);
            }
            OperandType::NearAddress(offset) => {}
            OperandType::FarAddress(segment,offset) => {}
            OperandType::NoOperand => {}
            OperandType::InvalidOperand => {}
        }
    }    
}
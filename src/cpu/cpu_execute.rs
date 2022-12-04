
use crate::cpu::*;

use crate::bus::BusInterface;
use crate::io::IoBusInterface;

use crate::util;

use super::CPU_CALL_STACK_LEN;

impl Cpu {

    pub fn execute_instruction(&mut self, io_bus: &mut IoBusInterface) -> ExecutionResult {

        let mut unhandled: bool = false;
        let mut jump: bool = false;
        let mut exception: CpuException = CpuException::NoException;
        let mut cycles = 0;

        let mut handled_override = match self.i.segment_override {
            SegmentOverride::NoOverride => true,
            _ => false,
        };
        
        // Check for REPx prefixes
        if (self.i.prefixes & OPCODE_PREFIX_REP1 != 0) || (self.i.prefixes & OPCODE_PREFIX_REP2 != 0) {
            // A REPx prefix was set
            
            let mut invalid_rep = false;

            match self.i.mnemonic {
                Mnemonic::STOSB | Mnemonic::STOSW | Mnemonic::LODSB | Mnemonic::LODSW | Mnemonic::MOVSB | Mnemonic::MOVSW => {
                    self.rep_type = RepType::Rep;
                }
                Mnemonic::SCASB | Mnemonic::SCASW | Mnemonic::CMPSB | Mnemonic::CMPSW => {
                    // Valid string ops with REP prefix
                    if self.i.prefixes & OPCODE_PREFIX_REP1 != 0 {
                        self.rep_type = RepType::Repne;
                    }
                    else {
                        self.rep_type = RepType::Repe;
                    }
                }
                _=> {
                    invalid_rep = true;
                    //return ExecutionResult::ExecutionError(
                    //    format!("REP prefix on invalid opcode: {:?} at [{:04X}:{:04X}].", self.i.mnemonic, self.cs, self.ip)
                    //);
                    log::warn!("REP prefix on invalid opcode: {:?} at [{:04X}:{:04X}].", self.i.mnemonic, self.cs, self.ip);
                }
            }

            // Check if we have saved state from returning from an interrupt
            if let Some(rep_state) = self.rep_state.last() {
                // Is this state for the current instruction?
                if rep_state.0 == self.cs && rep_state.1 == self.ip {
                    // Restore the current state
                    //log::trace!("Restoring state for REP string instruction.");
                    match rep_state.2 {                        
                        RepState::StosbState(es, di, cx) => { // dst: [es:di], cx
                            self.es = es;
                            self.di = di;
                            self.set_register16(Register16::CX, cx);
                        }, 
                        RepState::StoswState(es, di, cx) => { // dst: [es:di], cx
                            self.es = es;
                            self.di = di;
                            self.set_register16(Register16::CX, cx);
                        }, 
                        RepState::LodsbState(seg, seg_val, si, cx) => { // src: [ds*:si], cx
                            self.set_register16(seg, seg_val);
                            self.si = si;
                            self.set_register16(Register16::CX, cx);
                        }, 
                        RepState::LodswState(seg, seg_val, si, cx) => {  // src: [ds*:si], cx
                            self.set_register16(seg, seg_val);
                            self.si = si;
                            self.set_register16(Register16::CX, cx);                            
                        },
                        RepState::MovsbState(seg, seg_val, si, es, di, cx) => {  // src: [ds*:si], dst: [es:di], cx
                            self.set_register16(seg, seg_val);
                            self.si = si;
                            self.es = es;
                            self.di = di;
                            self.set_register16(Register16::CX, cx);                            
                        },
                        RepState::MovswState(seg, seg_val, si, es, di, cx) => { // src: [ds*:si]. dst: [es:di], cx
                            self.set_register16(seg, seg_val);
                            self.si = si;
                            self.es = es;
                            self.di = di;
                            self.set_register16(Register16::CX, cx); 
                        }, 
                        RepState::ScasbState(es,di,cx) => { // src: [es:di], cx
                            self.es = es;
                            self.di = di;
                            self.set_register16(Register16::CX, cx);
                        }, 
                        RepState::ScaswState(es,di,cx) => { // src: [es:di], cx
                            self.es = es;
                            self.di = di;
                            self.set_register16(Register16::CX, cx);
                        }, 
                        RepState::CmpsbState(seg, seg_val, si, es, di, cx) => { // src: [ds*:si], dst: [es:di], cx
                            self.set_register16(seg, seg_val);
                            self.si = si;
                            self.es = es;
                            self.di = di;
                            self.set_register16(Register16::CX, cx);   
                        }, 
                        RepState::CmpswState(seg, seg_val, si, es, di, cx) => {  // src: [ds*:si], dst: [es:di], cx
                            self.set_register16(seg, seg_val);
                            self.si = si;
                            self.es = es;
                            self.di = di;
                            self.set_register16(Register16::CX, cx);   
                        },        
                        RepState::NoState => {
                            // Rep prefix on invalid opcode, restore nothing
                        }
                    }
                    self.rep_state.pop();
                }
            }
            if !invalid_rep {
                self.in_rep = true;
                self.rep_mnemonic = self.i.mnemonic;
            }
        }

        // Reset the wait cycle after STI
        self.interrupt_inhibit = false;

        // Keep a tally of how many Opcode 0's we've executed in a row. Too many likely means we've run 
        // off the rails, whereupon we halt so we can check things out.
        if self.i.opcode == 0x00 {
            self.opcode0_counter = self.opcode0_counter.wrapping_add(1);

            if self.opcode0_counter > 5 {
                // Halt permanently by clearing interrupt flag
                self.clear_flag(Flag::Interrupt);
                self.halted = true;
            }
        }
        else {
            self.opcode0_counter = 0;
        }

        match self.i.opcode {
            0x00 | 0x02 | 0x04 => {
                // ADD r/m8, r8 | r8, r/m8 | al, imm8
                // 8 bit ADD variants
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();            
                
                let result = self.math_op8(Mnemonic::ADD, op1_value, op2_value);
                self.write_operand8(self.i.operand1_type, self.i.segment_override, result);
                handled_override = true;
            }
            0x01 | 0x03 | 0x05 => {
                // ADD r/m16, r16 | r16, r/m16 | ax, imm16
                // 16 bit ADD variants
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                
                let result = self.math_op16(Mnemonic::ADD, op1_value, op2_value);
                self.write_operand16(self.i.operand1_type, self.i.segment_override, result);       
                handled_override = true;         
            }
            0x06 => {
                // PUSH es
                // Flags: None
                self.push_register16(Register16::ES);
            }
            0x07 => {
                // POP es
                // Flags: None
                self.pop_register16(Register16::ES);
            }
            0x08 | 0x0A | 0x0C => {
                // OR r/m8, r8 | r8, r/m8 | al, imm8
                // 8 bit OR variants
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();
                
                let result = op1_value | op2_value;
                self.write_operand8(self.i.operand1_type, self.i.segment_override, result);

                // Clear carry & overflow flags
                // AoA 6.6.1
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u8(result);
                handled_override = true;
            }
            0x09 | 0x0B | 0x0D => {
                // OR r/m16, r16 | r16, r/m16 | ax, imm16
                // 16 bit OR variants
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                
                let result = op1_value | op2_value;
                self.write_operand16(self.i.operand1_type, self.i.segment_override, result);

                // Clear carry & overflow flags
                // AoA 6.6.1
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u16(result);
                handled_override = true;
            }
            0x0E => {
                // PUSH cs
                // Flags: None
                self.push_register16( Register16::CS);
            }
            0x0F => {
                // POP cs
                // Flags: None
                self.pop_register16(Register16::CS);
            }
            0x10 | 0x12 | 0x14 => {
                // ADC r/m8,r8 | r8, r/m8 | al,imm8 
                // ADC 8-bit variants
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();
                
                let result = self.math_op8(Mnemonic::ADC,  op1_value,  op2_value);
                self.write_operand8(self.i.operand1_type, self.i.segment_override, result);
                handled_override = true;
            }
            0x11 | 0x13 | 0x15 => {
                // ADC r/m16,r16 | r16, r/m16 | ax,imm16 
                // ADC 16-bit variants
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();

                let result = self.math_op16(Mnemonic::ADC,  op1_value,  op2_value);
                self.write_operand16(self.i.operand1_type, self.i.segment_override, result);
                handled_override = true;
            }
            0x16 => {
                // PUSH ss
                // Flags: None
                self.push_register16(Register16::SS);
            }
            0x17 => {
                // POP ss
                // Flags: None
                self.pop_register16(Register16::SS);
            }
            0x18 | 0x1A | 0x1C => {
                // SBB r/m8,r8 | r8, r/m8 | al,imm8 
                // SBB 8-bit variants
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();

                
                let result = self.math_op8(Mnemonic::SBB,  op1_value,  op2_value);
                self.write_operand8(self.i.operand1_type, self.i.segment_override, result);
                handled_override = true;
            }
            0x19 | 0x1B | 0x1D => {
                // SBB r/m16,r16 | r16, r/m16 | ax,imm16 
                // SBB 16-bit variants
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();

                
                let result = self.math_op16(Mnemonic::SBB,  op1_value,  op2_value);
                self.write_operand16(self.i.operand1_type, self.i.segment_override, result);
                handled_override = true;
            }
            0x1E => {
                // PUSH ds
                // Flags: None
                self.push_register16(Register16::DS);
            }
            0x1F => {
                // POP ds
                // Flags: None
                self.pop_register16(Register16::DS);
            }
            0x20 | 0x22 | 0x24 => {
                // AND r/m8,r8 | r8, r/m8 | al,imm8 
                // AND 8-bit variants
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();
                
                let result = self.math_op8(Mnemonic::AND,  op1_value,  op2_value);
                self.write_operand8(self.i.operand1_type, self.i.segment_override, result);
                handled_override = true;
            }
            0x21 | 0x23 | 0x25 => {
                // AND r/m16,r16 | r16, r/m16 | ax,imm16 
                // AND 16-bit variants
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();

                let result = self.math_op16(Mnemonic::AND,  op1_value,  op2_value);
                self.write_operand16(self.i.operand1_type, self.i.segment_override, result);
                handled_override = true;
            }
            0x26 => {
                // ES Segment Override Prefix
            }
            0x27 => {
                // DAA — Decimal Adjust AL after Addition
                self.daa();
            }
            0x28 | 0x2A | 0x2C => {
                // SUB r/m8,r8 | r8, r/m8 | al,imm8 
                // SUB 8-bit variants
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();

                let result = self.math_op8(Mnemonic::SUB,  op1_value,  op2_value);
                self.write_operand8(self.i.operand1_type, self.i.segment_override, result);
                handled_override = true;
            }
            0x29 | 0x2B | 0x2D => {
                // SUB r/m16,r16 | r16, r/m16 | ax,imm16 
                // SUB 16-bit variants
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();

                let result = self.math_op16(Mnemonic::SUB,  op1_value,  op2_value);
                self.write_operand16(self.i.operand1_type, self.i.segment_override, result);
                handled_override = true;
            }
            0x2E => {
                // CS Override Prefix
            }
            0x2F => {
                // DAS
                self.das();
            }
            0x30 | 0x32 | 0x34 => {
                // XOR r/m8, r8  |  XOR r8, r/m8
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();
                
                let result = op1_value ^ op2_value;
                self.write_operand8(self.i.operand1_type, self.i.segment_override, result);

                // Clear carry & overflow flags
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u8(result);
                handled_override = true;
            }
            0x31 | 0x33 | 0x35 => {
                // XOR r/m16, r16 |  XOR r16, r/m16
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                
                let result = op1_value ^ op2_value;
                self.write_operand16(self.i.operand1_type, self.i.segment_override, result);

                // Clear carry & overflow flags
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u16(result);
                handled_override = true;  
            }
            0x36 => {
                // SS Segment Override Prefix
            }
            0x37 => {
                // AAA
                self.aaa();
            }
            0x38 | 0x3A | 0x3C => {
                // CMP r/m8,r8 | r8, r/m8 | al,imm8 
                // CMP 8-bit variants
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();

                
                let _result = self.math_op8(Mnemonic::CMP,  op1_value,  op2_value);
                //self.write_operand8(self.i.operand1_type, self.i.segment_override, result);
                handled_override = true;
            }
            0x39 | 0x3B | 0x3D => {
                // CMP r/m16,r16 | r16, r/m16 | ax,imm16 
                // CMP 16-bit variants
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();

                
                let _result = self.math_op16(Mnemonic::CMP,  op1_value,  op2_value);
                //self.write_operand16(self.i.operand1_type, self.i.segment_override, result);
                handled_override = true;
            }
            0x3E => {
                // DS Segment Override Prefix
            }
            0x3F => {
                // AAS
                self.aas();
            }
            0x40..=0x47 => {
                // INC r16 register-encoded operands
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                // math_op16 handles flags
                let result = self.math_op16(Mnemonic::INC, op1_value, 0);
                self.write_operand16(self.i.operand1_type, self.i.segment_override, result);
                handled_override = true;
            }
            0x48..=0x4F => {
                // DEC r16 register-encoded operands
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                // math_op16 handles flags
                let result = self.math_op16(Mnemonic::DEC, op1_value, 0);
                self.write_operand16(self.i.operand1_type, self.i.segment_override, result);
                handled_override = true;           
            }
            0x50 => {
                // PUSH ax
                // Flags: None                
                self.push_register16(Register16::AX);
            }
            0x51 => {
                // PUSH cx
                // Flags: None                
                self.push_register16(Register16::CX);
            }
            0x52 => {
                // PUSH dx
                // Flags: None                
                self.push_register16(Register16::DX);
            }
            0x53 => {
                // PUSH bx
                // Flags: None
                self.push_register16(Register16::BX);
            }
            0x54 => {
                // PUSH sp
                // Flags: None
                self.push_register16(Register16::SP);
            }
            0x55 => {
                // PUSH bp
                // Flags: None             
                self.push_register16(Register16::BP);
            }
            0x56 => {
                // PUSH si
                // Flags: None                
                self.push_register16(Register16::SI);
            }
            0x57 => {
                // PUSH di
                // Flags: None
                self.push_register16(Register16::DI);                 
            }
            0x58 => {
                // POP ax
                self.pop_register16(Register16::AX);
            }
            0x59 => {
                // POP cx
                self.pop_register16(Register16::CX);
            }
            0x5A => {
                // POP dx
                self.pop_register16(Register16::DX);
            }
            0x5B => {
                // POP bx
                self.pop_register16(Register16::BX);
            }
            0x5C => {
                // POP sp
                self.pop_register16(Register16::SP);
            }
            0x5D => {
                // POP bp
                self.pop_register16(Register16::BP);
            }
            0x5E => {
                // POP si
                self.pop_register16(Register16::SI);
            }
            0x5F => {
                // POP di
                self.pop_register16(Register16::DI);
            }
            0x60..=0x7F => {
                // JMP rel8 variants
                // Note that 0x60-6F maps to 0x70-7F on 8088
                jump = match self.i.opcode & 0x0F {
                    0x00 => self.get_flag(Flag::Overflow),  // JO - Jump if overflow set
                    0x01 => !self.get_flag(Flag::Overflow), // JNO - Jump it overflow not set
                    0x02 => self.get_flag(Flag::Carry), // JB -> Jump if carry set
                    0x03 => !self.get_flag(Flag::Carry), // JNB -> Jump if carry not set
                    0x04 => self.get_flag(Flag::Zero), // JZ -> Jump if Zero set
                    0x05 => !self.get_flag(Flag::Zero), // JNZ -> Jump if Zero not set
                    0x06 => self.get_flag(Flag::Carry) || self.get_flag(Flag::Zero), // JBE -> Jump if Carry OR Zero
                    0x07 => !self.get_flag(Flag::Carry) && !self.get_flag(Flag::Zero), // JNBE -> Jump if Carry not set AND Zero not set
                    0x08 => self.get_flag(Flag::Sign), // JS -> Jump if Sign set
                    0x09 => !self.get_flag(Flag::Sign), // JNS -> Jump if Sign not set
                    0x0A => self.get_flag(Flag::Parity), // JP -> Jump if Parity set
                    0x0B => !self.get_flag(Flag::Parity), // JNP -> Jump if Parity not set
                    0x0C => self.get_flag(Flag::Sign) != self.get_flag(Flag::Overflow), // JL -> Jump if Sign flag != Overflow flag
                    0x0D => self.get_flag(Flag::Sign) == self.get_flag(Flag::Overflow), // JNL -> Jump if Sign flag == Overflow flag
                    0x0E => self.get_flag(Flag::Zero) || (self.get_flag(Flag::Sign) != self.get_flag(Flag::Overflow)),  // JLE ((ZF=1) OR (SF!=OF))
                    0x0F => !self.get_flag(Flag::Zero) && (self.get_flag(Flag::Sign) == self.get_flag(Flag::Overflow)), // JNLE ((ZF=0) AND (SF=OF))
                    _ => false
                };
                if jump {
                    if let OperandType::Relative8(rel8) = self.i.operand1_type {
                        self.ip = util::relative_offset_u16(self.ip, rel8 as i16 + self.i.size as i16 );
                    }
                }
            }
            0x80 | 0x82 => {
                // ADD/OR/ADC/SBB/AND/SUB/XOR/CMP r/m8, imm8
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();
                
                let result = self.math_op8(self.i.mnemonic, op1_value, op2_value);

                if self.i.mnemonic != Mnemonic::CMP {
                    self.write_operand8(self.i.operand1_type, self.i.segment_override, result);
                }
                handled_override = true;
            }
            0x81 => {
                // ADD/OR/ADC/SBB/AND/SUB/XOR/CMP r/m16, imm16
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                // math_op16 handles flags
                let result = self.math_op16(self.i.mnemonic, op1_value, op2_value);

                if self.i.mnemonic != Mnemonic::CMP {
                    self.write_operand16(self.i.operand1_type, self.i.segment_override, result);
                }
                handled_override = true;
            }
            0x83 => {
                // ADD/ADC/SBB/SUB/CMP r/m16, imm_i8
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();

                // imm_i8 gets sign-extended
                let op2_extended = util::sign_extend_u8_to_u16(op2_value);

                // math_op16 handles flags
                let result = self.math_op16(self.i.mnemonic, op1_value, op2_extended);   
                
                if self.i.mnemonic != Mnemonic::CMP {
                    self.write_operand16(self.i.operand1_type, self.i.segment_override, result);
                }
                handled_override = true;
            }
            0x84 => {
                // TEST r/m8, r8
                // Flags: o..sz.pc
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();
                
                self.math_op8(Mnemonic::TEST, op1_value, op2_value);
                handled_override = true;
            }
            0x85 => {
                // TEST r/m16, r16
                // Flags: o..sz.pc                
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                // math_op16 handles flags
                self.math_op16(Mnemonic::TEST, op1_value, op2_value);
                handled_override = true;
            }
            0x86 => {
                // XCHG r8, r/m8
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();

                // Exchange values
                self.write_operand8(self.i.operand1_type, self.i.segment_override, op2_value);
                self.write_operand8(self.i.operand2_type, self.i.segment_override, op1_value);
                handled_override = true;
            }
            0x87 => {
                // XCHG r16, r/m16
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();

                // Exchange values
                self.write_operand16(self.i.operand1_type, self.i.segment_override, op2_value);
                self.write_operand16(self.i.operand2_type, self.i.segment_override, op1_value);
                handled_override = true;
            }
            0x88 | 0x8A => {
                // MOV r/m8, r8  |  MOV r8, r/m8
                let op_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();
                self.write_operand8(self.i.operand1_type, self.i.segment_override, op_value);
                handled_override = true;
            }
            0x89 | 0x8B => {
                // MOV r/m16, r16  |  MOV r16, r/m16
                let op_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                self.write_operand16(self.i.operand1_type, self.i.segment_override, op_value);
                handled_override = true;
            }
            0x8C | 0x8E => {
                // MOV r/m16, SReg | MOV SReg, r/m16

                if let OperandType::Register16(foo) = self.i.operand2_type {
                    if let Register16::InvalidRegister = foo {
                        println!("Whoops!")
                    }
                }                
                let op_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                self.write_operand16(self.i.operand1_type, self.i.segment_override, op_value);
                handled_override = true;
            }
            0x8D => {
                // LEA - Load Effective Address
                let value = self.load_effective_address(self.i.operand2_type).unwrap();
                self.write_operand16(self.i.operand1_type, SegmentOverride::NoOverride, value);
            }
            0x8F => {
                // POP r/m16
                let value = self.pop_u16();
                self.write_operand16(self.i.operand1_type, self.i.segment_override, value);
                handled_override = true;
            }
            0x90 => {
                // NOP
                // Do nothing
            }
            0x91 => {
                // XCHG AX, CX
                // Flags: None
                let ax_value = self.ax;
                let cx_value = self.cx;
                self.set_register16(Register16::AX, cx_value);
                self.set_register16(Register16::CX, ax_value);
            }
            0x92 => {
                // XCHG AX, DX
                // Flags: None
                let ax_value = self.ax;
                let dx_value = self.dx;
                self.set_register16(Register16::AX, dx_value);
                self.set_register16(Register16::DX, ax_value);
            }
            0x93 => {
                // XCHG AX, BX
                // Flags: None
                let ax_value = self.ax;
                let bx_value = self.bx;
                self.set_register16(Register16::AX, bx_value);
                self.set_register16(Register16::BX, ax_value);
            }
            0x94 => {
                // XCHG AX, SP
                // Flags: None
                let ax_value = self.ax;
                let sp_value = self.sp;
                self.set_register16(Register16::AX, sp_value);
                self.set_register16(Register16::SP, ax_value);
            }
            0x95 => {
                // XCHG AX, BP
                // Flags: None
                let ax_value = self.ax;
                let bp_value = self.bp;
                self.set_register16(Register16::AX, bp_value);
                self.set_register16(Register16::BP, ax_value);
            }
            0x96 => {
                // XCHG AX, SI
                // Flags: None
                let ax_value = self.ax;
                let si_value = self.si;
                self.set_register16(Register16::AX, si_value);
                self.set_register16(Register16::SI, ax_value);
            }
            0x97 => {
                // XCHG AX, DI
                // Flags: None
                let ax_value = self.ax;
                let di_value = self.di;
                self.set_register16(Register16::AX, di_value);
                self.set_register16(Register16::DI, ax_value);
            }
            0x98 => {
                // CBW - Convert Byte to Word
                // Flags: None
                self.sign_extend_al();
            }
            0x99 => {
                // CWD - Convert Word to Doubleword
                // Flags: None
                self.sign_extend_ax();
            }
            0x9A => {
                // CALLF - Call Far

                // Push return address of next instruction
                self.push_register16(Register16::CS);
                let next_i = self.ip + (self.i.size as u16);
                self.push_u16(next_i);

                if let OperandType::FarAddress(segment, offset) = self.i.operand1_type {        
                    
                    // Add to call stack
                    if self.call_stack.len() == CPU_CALL_STACK_LEN {
                        self.call_stack.pop_front();
                    }                    
                    self.call_stack.push_back(CallStackEntry::CallF(self.cs, self.ip, segment, offset));
                    
                    self.cs = segment;
                    self.ip = offset;
                }
                jump = true;
            }
            0x9B => {
                unhandled = true;
            }
            0x9C => {
                // PUSHF - Push Flags
                self.push_flags();
            }
            0x9D => {
                // POPF - Pop Flags
                self.pop_flags();
            }
            0x9E => {
                // SAHF - Store AH into Flags
                self.store_flags(self.ah as u16);
                cycles = 4;
            }
            0x9F => {
                // LAHF - Load Status Flags into AH Register
                let flags = self.load_flags() as u8;
                self.set_register8(Register8::AH, flags);
                cycles = 4;
            }
            0xA0 => {
                // MOV al, offset8
                // These MOV variants are unique in that they take a direct offset with no modr/m byte
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();
                self.set_register8(Register8::AL, op2_value);
                handled_override = true;
            }
            0xA1 => {
                // MOV AX, offset16
                // These MOV variants are unique in that they take a direct offset with no modr/m byte
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                self.set_register16(Register16::AX, op2_value);                
                handled_override = true;
            }
            0xA2 => {
                // MOV offset8, Al
                // These MOV variants are unique in that they take a direct offset with no modr/m byte
                let op2_value = self.al;
                self.write_operand8(self.i.operand1_type, self.i.segment_override, op2_value);
                handled_override = true;
            }
            0xA3 => {
                // MOV offset16, AX
                // These MOV variants are unique in that they take a direct offset with no modr/m byte
                let op2_value = self.ax;
                self.write_operand16(self.i.operand1_type, self.i.segment_override, op2_value);
                handled_override = true;        
            }
            0xA4 => {
                // MOVSB
                if !self.in_rep || (self.in_rep && self.cx > 0) {
                    self.string_op(Mnemonic::MOVSB, self.i.segment_override);
                }

                // Check for end condition (CX==0)
                if self.in_rep {
                    if self.cx > 0 {
                        self.decrement_register16(Register16::CX);
                    }
                    if self.cx == 0 {
                        self.in_rep = false;
                        self.rep_type = RepType::NoRep;
                    }
                }
                handled_override = true;
            }
            0xA5 => {
                // MOVSW
                if !self.in_rep || (self.in_rep && self.cx > 0) {
                    self.string_op(Mnemonic::MOVSW, self.i.segment_override);
                }

                // Check for end condition (CX==0)
                if self.in_rep {
                    if self.cx > 0 {
                        self.decrement_register16(Register16::CX);
                    }
                    if self.cx == 0 {
                        self.in_rep = false;
                        self.rep_type = RepType::NoRep;
                    }
                }
                handled_override = true;
            }
            0xA6 | 0xA7 => {
                // CMPSB & CMPSw
                // Segment override: DS overridable
                // Flags: All
                if !self.in_rep || (self.in_rep && self.cx > 0) {
                    self.string_op(self.i.mnemonic, self.i.segment_override);       
                }

                // Check for REP end condition #1 (CX==0)
                if self.in_rep {
                    if self.cx > 0 {
                        self.decrement_register16(Register16::CX);
                    }
                    if self.cx == 0 {
                        self.in_rep = false;
                        self.rep_type = RepType::NoRep;
                    }
                }
                // Check for REP end condition #2 (Z/NZ)
                match self.rep_type {
                    RepType::Repne => {
                        // Repeat while NOT zero. If Zero flag is set, end REP.
                        if self.get_flag(Flag::Zero) {
                            self.in_rep = false;
                            self.rep_type = RepType::NoRep;
                        }
                    }
                    RepType::Repe => {
                        // Repeat while zero. If zero flag is NOT set, end REP.
                        if !self.get_flag(Flag::Zero) {
                            self.in_rep = false;
                            self.rep_type = RepType::NoRep;
                        }
                    }
                    _=> {}
                };
                handled_override = true;
            }
            0xA8 => {
                // TEST al, imm8
                // Flags: o..sz.pc
                let op1_value = self.al;
                let op2_value = self.read_operand8(self.i.operand2_type, SegmentOverride::NoOverride).unwrap();
                
                self.math_op8(Mnemonic::TEST,  op1_value, op2_value);
            }
            0xA9 => {
                // TEST ax, imm16
                // Flags: o..sz.pc
                let op1_value = self.ax;
                let op2_value = self.read_operand16(self.i.operand2_type, SegmentOverride::NoOverride).unwrap();
                
                self.math_op16(Mnemonic::TEST,  op1_value, op2_value);
            }
            0xAA | 0xAB => {
                // STOSB & STOSW
                if !self.in_rep || (self.in_rep && self.cx > 0) {
                    self.string_op(self.i.mnemonic, SegmentOverride::NoOverride);
                }

                // Check for end condition (CX==0)
                if self.in_rep {
                    if self.cx > 0 {
                        self.decrement_register16(Register16::CX);
                    }
                    if self.cx == 0 {
                        self.in_rep = false;
                        self.rep_type = RepType::NoRep;
                    }
                }
            }
            0xAC | 0xAD => {
                // LODSB & LODSW
                // Flags: None

                // Although LODSx is not typically used with a REP prefix, it can be
                if !self.in_rep || (self.in_rep && self.cx > 0) {
                    self.string_op(self.i.mnemonic, self.i.segment_override);
                }
                
                // Check for REP end condition #1 (CX==0)
                if self.in_rep {
                    if self.cx > 0 {
                        self.decrement_register16(Register16::CX);
                    }
                    if self.cx == 0 {
                        self.in_rep = false;
                        self.rep_type = RepType::NoRep;
                    }
                }
                handled_override = true;
            }
            0xAE | 0xAF => {
                // SCASB & SCASW
                // Flags: ALL
                if !self.in_rep || (self.in_rep && self.cx > 0) {
                    self.string_op(self.i.mnemonic, SegmentOverride::NoOverride);
                }

                // Check for REP end condition #1 (CX==0)
                if self.in_rep {
                    if self.cx > 0 {
                        self.decrement_register16(Register16::CX);
                    }
                    if self.cx == 0 {
                        self.in_rep = false;
                        self.rep_type = RepType::NoRep;
                    }
                }
                // Check for REP end condition #2 (Z/NZ)
                match self.rep_type {
                    RepType::Repne => {
                        // Repeat while NOT zero. If Zero flag is set, end REP.
                        if self.get_flag(Flag::Zero) {
                            self.in_rep = false;
                            self.rep_type = RepType::NoRep;
                        }
                    }
                    RepType::Repe => {
                        // Repeat while zero. If zero flag is NOT set, end REP.
                        if !self.get_flag(Flag::Zero) {
                            self.in_rep = false;
                            self.rep_type = RepType::NoRep;
                        }
                    }
                    _=> {}
                };
            }
            0xB0..=0xB7 => {
                // MOV r8, imm8
                if let OperandType::Immediate8(imm8) = self.i.operand2_type {
                    if let OperandType::Register8(reg) = self.i.operand1_type { 
                        self.set_register8(reg, imm8);
                    }
                }
                cycles = 4;
            }
            0xB8..=0xBF => {
                // MOV r16, imm16
                if let OperandType::Immediate16(imm16) = self.i.operand2_type {
                    if let OperandType::Register16(reg) = self.i.operand1_type {
                        self.set_register16(reg, imm16);
                    }
                }
            }
            0xC0 | 0xC2 => {
                // RETN imm16 - Return from call w/ release
                // 0xC0 undocumented alias for 0xC2
                // Flags: None
                let new_ip = self.pop_u16();
                self.ip = new_ip;
                
                let stack_disp = self.read_operand16(self.i.operand1_type, SegmentOverride::NoOverride).unwrap();
                self.release(stack_disp);                

                // Pop call stack
                self.call_stack.pop_back();

                jump = true
            }
            0xC1 | 0xC3 => {
                // RETN - Return from call
                // 0xC1 undocumented alias for 0xC3
                // Flags: None
                // Effectively, this instruction is pop ip
                let new_ip = self.pop_u16();
                self.ip = new_ip;
                
                // Pop call stack
                self.call_stack.pop_back();

                jump = true
            }
            0xC4 => {
                // LES - Load ES from Pointer
                // Operand 2 is far pointer
                let (les_segment, les_offset) = self.read_operand_farptr(self.i.operand2_type, self.i.segment_override).unwrap();

                //log::trace!("LES instruction: Loaded {:04X}:{:04X}", les_segment, les_offset);
                self.write_operand16(self.i.operand1_type, self.i.segment_override, les_offset);
                self.es = les_segment;
                handled_override = true;
            }
            0xC5 => {
                // LDS - Load DS from Pointer
                // Operand 2 is far pointer
                let (lds_segment, lds_offset) = self.read_operand_farptr(self.i.operand2_type, self.i.segment_override).unwrap();

                //log::trace!("LDS instruction: Loaded {:04X}:{:04X}", lds_segment, lds_offset);
                self.write_operand16(self.i.operand1_type, self.i.segment_override, lds_offset);
                self.ds = lds_segment;
                handled_override = true;
            }
            0xC6 => {
                // MOV r/m8, imm8
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();
                self.write_operand8(self.i.operand1_type, self.i.segment_override, op2_value);
                handled_override = true;
            }
            0xC7 => {
                // MOV r/m16, imm16
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                self.write_operand16(self.i.operand1_type, self.i.segment_override, op2_value);
                handled_override = true;
            }
            0xC8 | 0xCA => {
                // RETF imm16 - Far Return w/ release 
                // 0xC8 undocumented alias for 0xCA
                self.pop_register16(Register16::IP);
                self.pop_register16(Register16::CS);
                let stack_disp = self.read_operand16(self.i.operand1_type, SegmentOverride::NoOverride).unwrap();
                self.release(stack_disp);

                // Pop call stack
                self.call_stack.pop_back();
                jump = true;
            }
            0xC9 | 0xCB => {
                // RETF - Far Return
                // 0xC9 undocumented alias for 0xCB
                self.pop_register16(Register16::IP);
                self.pop_register16(Register16::CS);

                // Pop call stack
                self.call_stack.pop_back();                
                jump = true;
            }
            0xCC => {
                // INT 3 - Software Interrupt 3
                // This is a special form of INT which assumes IRQ 3 always. Most assemblers will not generate this form
                self.ip = self.ip.wrapping_add(1);
                self.sw_interrupt(3);

                jump = true;    
            }
            0xCD => {
                // INT - Software Interrupt
                // The Interrupt flag does not affect the handling of non-maskable interrupts (NMIs) or software interrupts
                // generated by the INT instruction. 

                // Get IRQ number
                let irq = self.read_operand8(self.i.operand1_type, SegmentOverride::NoOverride).unwrap();
                self.ip = self.ip.wrapping_add(2);
                self.sw_interrupt(irq );
                jump = true;
            }
            0xCE => {
                // INTO - Call Overflow Interrupt Handler
                self.ip = self.ip.wrapping_add(1);
                self.sw_interrupt(4);
            
                jump = true;
            }
            0xCF => {
                // IRET instruction
                self.end_interrupt();
                jump = true;
            }
            0xD0 => {
                // ROL, ROR, RCL, RCR, SHL, SHR, SAR:  r/m8, 0x01
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let result = self.bitshift_op8(self.i.mnemonic, op1_value, 1);
                self.write_operand8(self.i.operand1_type, self.i.segment_override, result);
                // TODO: Cost
                handled_override = true;
            }
            0xD1 => {
                // ROL, ROR, RCL, RCR, SHL, SHR, SAR:  r/m16, 0x01
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let result = self.bitshift_op16(self.i.mnemonic, op1_value, 1);
                self.write_operand16(self.i.operand1_type, self.i.segment_override, result);
                handled_override = true;
            }
            0xD2 => {
                // ROL, ROR, RCL, RCR, SHL, SHR, SAR:  r/m8, cl
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();
                let result = self.bitshift_op8(self.i.mnemonic, op1_value, op2_value);
                self.write_operand8(self.i.operand1_type, self.i.segment_override, result);
                // TODO: Cost
                handled_override = true;
            }
            0xD3 => {
                // ROL, ROR, RCL, RCR, SHL, SHR, SAR:  r/m16, cl
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();
                let result = self.bitshift_op16(self.i.mnemonic, op1_value, op2_value);
                self.write_operand16(self.i.operand1_type, self.i.segment_override, result);
                // TODO: Cost
                handled_override = true;
            }
            0xD4 => {
                // AAM - Ascii adjust AX after Multiply
                // Get imm8 value
                let op1_value = self.read_operand8(self.i.operand1_type, SegmentOverride::NoOverride).unwrap();
                
                if op1_value == 0 {
                    exception = CpuException::DivideError;
                }
                else {
                    self.aam(op1_value);
                }
            }
            0xD5 => {
                // AAD - Ascii Adjust before Division
                let op1_value = self.read_operand8(self.i.operand1_type, SegmentOverride::NoOverride).unwrap();
                self.aad(op1_value);
            }
            0xD6 => {
                // SALC - Undocumented Opcode - Set Carry flag in AL
                // http://www.rcollins.org/secrets/opcodes/SALC.html

                self.set_register8(Register8::AL,
                    match self.get_flag(Flag::Carry) {
                        true => 0xFF,
                        false => 0
                    }
                );
            }
            0xD7 => {
                // XLAT
                
                // Handle segment override
                let segment_base_default_ds: u16 = match self.i.segment_override {
                    SegmentOverride::NoOverride => self.ds,
                    SegmentOverride::SegmentES => self.es,
                    SegmentOverride::SegmentCS => self.cs,
                    SegmentOverride::SegmentSS => self.ss,
                    SegmentOverride::SegmentDS => self.ds
                };

                let disp16: u16 = self.bx.wrapping_add(self.al as u16);

                let addr = Cpu::calc_linear_address(segment_base_default_ds, disp16);
                
                //let (value, _cost) = self.bus.read_u8(addr as usize).unwrap();
                let value = self.biu_read_u8(addr);
                
                self.set_register8(Register8::AL, value as u8);
                handled_override = true;
            }
            0xD8..=0xDF => {
                // ESC - FPU instructions. 
                
                // Perform dummy read if memory operand
                let _op1_value = self.read_operand16(self.i.operand1_type, SegmentOverride::NoOverride);
            }
            0xE0 => {
                // LOOPNE - Decrement CX, Jump short if count!=0 and ZF=0
                // loop does not modify flags
                self.decrement_register16(Register16::CX);
                if self.cx != 0 {
                    if !self.get_flag(Flag::Zero) {
                        if let OperandType::Relative8(rel8) = self.i.operand1_type {
                            self.ip = util::relative_offset_u16(self.ip, rel8 as i16 + self.i.size as i16 );
                            jump = true;
                        }
                    }
                }
            }
            0xE1 => {
                // LOOPE - Jump short if count!=0 and ZF=1
                // loop does not modify flags
                self.decrement_register16(Register16::CX);
                if self.cx != 0 {
                    if self.get_flag(Flag::Zero) {
                        if let OperandType::Relative8(rel8) = self.i.operand1_type {
                            self.ip = util::relative_offset_u16(self.ip, rel8 as i16 + self.i.size as i16 );
                            jump = true;
                        }
                    }
                }
            }
            0xE2 => {
                // LOOP - Jump short if count!=0 
                // loop does not modify flags
                let dec_cx = self.cx.wrapping_sub(1);
                self.set_register16(Register16::CX, dec_cx);
                if dec_cx != 0 {
                    if let OperandType::Relative8(rel8) = self.i.operand1_type {
                        self.ip = util::relative_offset_u16(self.ip, rel8 as i16 + self.i.size as i16 );
                        jump = true;
                    }
                }
            }
            0xE3 => {
                // JCXZ - Jump if CX == 0
                // Flags: None
                if self.cx == 0 {
                    if let OperandType::Relative8(rel8) = self.i.operand1_type {
                        self.ip = util::relative_offset_u16(self.ip, rel8 as i16 + self.i.size as i16 );
                        jump = true;
                    }
                }
            }
            0xE4 => {
                // IN al, imm8
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap(); 
                
                let in_byte = io_bus.read_u8(op2_value as u16);
                self.set_register8(Register8::AL, in_byte);
                //println!("IN: Would input value from port {:#02X}", op2_value);                
            }
            0xE5 => {
                // IN ax, imm8
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap(); 
                let in_byte = io_bus.read_u8(op2_value as u16);
                self.set_register16(Register16::AX, in_byte as u16);
            }
            0xE6 => {
                // OUT imm8, al
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();                

                // Write to port
                io_bus.write_u8(op1_value as u16, op2_value);
                //println!("OUT: Would output {:02X} to Port {:#02X}", op2_value, op1_value);

            }
            0xE7 => {
                // OUT imm16
                unhandled = true;
            }
            0xE8 => {
                // CALL rel16
                // Push offset of next instruction (CALL rel16 is 3 bytes)
                let cs = self.get_register16(Register16::CS);
                let ip = self.get_register16(Register16::IP);
                let next_i = ip + 3;
                self.push_u16(next_i);

                // Add rel16 to ip
                let rel16 = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                self.ip = util::relative_offset_u16(self.ip, rel16 as i16 + self.i.size as i16 );
                jump = true;

                // Add to call stack
                if self.call_stack.len() == CPU_CALL_STACK_LEN {
                    self.call_stack.pop_front();
                }                
                self.call_stack.push_back(CallStackEntry::Call(cs, ip, rel16));
            }
            0xE9 => {
                // JMP rel16
                if let OperandType::Relative16(rel16) = self.i.operand1_type {
                    self.ip = util::relative_offset_u16(self.ip, rel16 as i16 + self.i.size as i16 );
                }
                jump = true;
                // cycles? 
            }
            0xEA => {
                // JMP FAR
                if let OperandType::FarAddress(segment, offset) = self.i.operand1_type {                
                    self.cs = segment;
                    self.ip = offset;
                }
                jump = true;
                cycles = 24;
            }
            0xEB => {
                // JMP rel8
                if let OperandType::Relative8(rel8) = self.i.operand1_type {
                    self.ip = util::relative_offset_u16(self.ip, rel8 as i16 + self.i.size as i16 );
                }
                jump = true
            }
            0xEC => {
                // IN al, dx
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap(); 
                let in_byte = io_bus.read_u8(op2_value);
                self.set_register8(Register8::AL, in_byte);
            }
            0xED => {
                // IN ax, dx
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap(); 
                let in_byte = io_bus.read_u8(op2_value);
                self.set_register16(Register16::AX, in_byte as u16);
            }
            0xEE => {
                // OUT dx, al
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();                

                io_bus.write_u8(op1_value as u16, op2_value);
                //println!("OUT: Would output {:02X} to Port {:#04X}", op2_value, op1_value);                
            }
            0xEF => {
                // OUT dx, ax
                // On the 8088, this does two writes to successive port #'s 
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();

                // Write first 8 bits to first port
                io_bus.write_u8(op1_value, (op2_value & 0xFF) as u8);
                // Write next 8 bits to port + 1
                io_bus.write_u8(op1_value + 1, (op2_value >> 8 & 0xFF) as u8);
            }
            0xF0 => {
                unhandled = true;
            }
            0xF1 => {
                unhandled = true;
            }
            0xF2 => {
                unhandled = true;
            }
            0xF3 => {
                unhandled = true;
            }
            0xF4 => {
                // HLT - Halt
                self.halted = true;
                cycles = 2;
            }
            0xF5 => {
                // CMC - Complement (invert) Carry Flag
                let carry_flag = self.get_flag(Flag::Carry);
                self.set_flag_state(Flag::Carry, !carry_flag);
            }
            0xF6 => {
                // Miscellaneous Opcode Extensions, r/m8, imm8
                match self.i.mnemonic {

                    Mnemonic::TEST => {
                        let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                        let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();
                        // Don't use result, just set flags
                        let _result = self.math_op8(self.i.mnemonic, op1_value, op2_value);
                    }
                    Mnemonic::NOT => {
                        let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                        let result = self.math_op8(self.i.mnemonic, op1_value, 0);
                        self.write_operand8(self.i.operand1_type, self.i.segment_override, result);
                    }
                    Mnemonic::NEG => {
                        let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                        let result = self.math_op8(self.i.mnemonic, op1_value, 0);
                        self.write_operand8(self.i.operand1_type, self.i.segment_override, result);
                    }
                    Mnemonic::MUL => {
                        let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                        // Multiply handles writing to ax
                        self.multiply_u8(op1_value);
                    }
                    Mnemonic::IMUL => {
                        let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                        // Multiply handles writing to ax
                        self.multiply_i8(op1_value as i8);
                    }                    
                    Mnemonic::DIV => {
                        let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                        // Divide handles writing to dx:ax
                        let success = self.divide_u8(op1_value);
                        if !success {
                            exception = CpuException::DivideError;
                        }
                        // TODO: Handle DIV exceptions
                    }          
                    Mnemonic::IDIV => {
                        let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                        // Divide handles writing to dx:ax
                        let success = self.divide_i8(op1_value);
                        if !success {
                            exception = CpuException::DivideError;
                        }
                        // TODO: Handle DIV exceptions
                    }                                 
                    _=> unhandled = true
                }
                handled_override = true;
            }
            0xF7 => {
                // Miscellaneous Opcode Extensions, r/m16, imm16
                match self.i.mnemonic {

                    Mnemonic::TEST => {
                        let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                        let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                        // Don't use result, just set flags
                        let _result = self.math_op16(self.i.mnemonic, op1_value, op2_value);
                    }
                    Mnemonic::NOT => {
                        let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                        let result = self.math_op16(self.i.mnemonic, op1_value, 0);
                        self.write_operand16(self.i.operand1_type, self.i.segment_override, result);
                    }
                    Mnemonic::NEG => {
                        let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                        let result = self.math_op16(self.i.mnemonic, op1_value, 0);
                        self.write_operand16(self.i.operand1_type, self.i.segment_override, result);
                    }
                    Mnemonic::MUL => {
                        let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                        // Multiply handles writing to ax
                        self.multiply_u16(op1_value);
                    }
                    Mnemonic::IMUL => {
                        let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                        // Multiply handles writing to dx:ax
                        self.multiply_i16(op1_value as i16);
                    }
                    Mnemonic::DIV => {
                        let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                        // Divide handles writing to dx:ax
                        let success = self.divide_u16(op1_value);
                        if !success {
                            exception = CpuException::DivideError;
                        }
                        // TODO: Handle DIV exceptions
                    }
                    Mnemonic::IDIV => {
                        let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                        // Divide handles writing to dx:ax
                        let success = self.divide_i16(op1_value);
                        if !success {
                            exception = CpuException::DivideError;
                        }
                    }
                    _=> unhandled = true
                }
                handled_override = true;
            }
            0xF8 => {
                // CLC - Clear Carry Flag
                self.clear_flag(Flag::Carry);
                cycles = 2;
            }
            0xF9 => {
                // STC - Set Carry Flag
                self.set_flag(Flag::Carry);
            }
            0xFA => {
                // CLI - Clear Interrupt Flag
                self.clear_flag(Flag::Interrupt);
                cycles = 2;
            }
            0xFB => {
                // STI - Set Interrupt Flag
                self.set_flag(Flag::Interrupt);
                cycles = 2;
            }
            0xFC => {
                // CLD - Clear Direction Flag
                self.clear_flag(Flag::Direction);
                cycles = 2;
            }
            0xFD => {
                // STD = Set Direction Flag
                self.set_flag(Flag::Direction);
                cycles = 2;
            }
            0xFE => {
                // INC/DEC r/m8
                let op_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let result = self.math_op8(self.i.mnemonic, op_value, 0);
                self.write_operand8(self.i.operand1_type, self.i.segment_override, result);
                // cycles ?
                handled_override = true;
            }
            0xFF => {
                // Several opcode extensions here
                match self.i.mnemonic {
                    // INC/DEC r/m16
                    Mnemonic::INC | Mnemonic::DEC => {
                        let op_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                        let result = self.math_op16(self.i.mnemonic, op_value, 0);
                        self.write_operand16(self.i.operand1_type, self.i.segment_override, result);
                    },
                    // Push Word onto stack
                    Mnemonic::PUSH => {
                        let op_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                        self.push_u16(op_value);
                    }
                    // Jump to memory r/m16
                    Mnemonic::JMP => {
                        let ptr16 = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();

                        self.ip = ptr16;
                        jump = true;
                    }
                    // Jump Far
                    Mnemonic::JMPF => {

                        let (segment, offset) = self.read_operand_farptr(self.i.operand1_type, self.i.segment_override).unwrap();

                        self.cs = segment;
                        self.ip = offset;
                        jump = true;

                        //log::trace!("JMPF: Destination [{:04X}:{:04X}]", segment, offset);
                    }
                    // Call Near
                    Mnemonic::CALL => {
                        let ptr16 = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                        //log::trace!("CALL: Destination [{:04X}]", ptr16);
                        
                        // Push return address (next instruction offset) onto stack
                        let next_i = self.ip + (self.i.size as u16);
                        self.push_u16(next_i);

                        // Add to call stack
                        if self.call_stack.len() == CPU_CALL_STACK_LEN {
                            self.call_stack.pop_front();
                        }                
                        self.call_stack.push_back(CallStackEntry::Call(self.cs, self.ip, ptr16));
                        
                        self.ip = ptr16;
                        jump = true;
                    }
                    // Call Far
                    Mnemonic::CALLF => {
                        let (segment, offset) = self.read_operand_farptr(self.i.operand1_type, self.i.segment_override).unwrap();

                        // Push return address of next instruction
                        self.push_register16(Register16::CS);
                        let next_i = self.ip + (self.i.size as u16);
                        self.push_u16(next_i);

                        // Add to call stack
                        if self.call_stack.len() == CPU_CALL_STACK_LEN {
                            self.call_stack.pop_front();
                        }                
                        self.call_stack.push_back(CallStackEntry::CallF(self.cs, self.ip, segment, offset));

                        self.cs = segment;
                        self.ip = offset;

                        //log::trace!("CALLF: Destination [{:04X}:{:04X}]", segment, offset);
                        jump = true;
                    }
                    _=> {
                        unhandled = true;
                    }
                }
                handled_override = true;
                // cycles ?
            }
            _ => {
                return ExecutionResult::UnsupportedOpcode(self.i.opcode);
            }

        }

        match self.i.segment_override {
            SegmentOverride::NoOverride => {},
            _ => {
                //Check that we properly handled override. No longer panics as IBM DOS 1.0 has a stray 'cs' override
                if !handled_override {
                    log::warn!("Unhandled segment override at [{:04X}:{:04X}]: {:02X}", self.cs, self.ip, self.i.opcode);
                }
            }

        }

        if unhandled {
            ExecutionResult::UnsupportedOpcode(self.i.opcode)
        }
        else {
            if self.halted && !self.get_flag(Flag::Interrupt) {
                // CPU was halted with interrupts disabled - will not continue
                ExecutionResult::Halt
            }
            else if jump {
                ExecutionResult::OkayJump
            }
            else if self.in_rep {
                ExecutionResult::OkayRep
            }
            else {
                match exception {
                    CpuException::DivideError => ExecutionResult::ExceptionError(exception),
                    CpuException::NoException => ExecutionResult::Okay
                }                
            }
        }
    }
}
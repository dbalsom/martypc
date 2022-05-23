use crate::arch;
use crate::arch::{OperandType, Opcode, Instruction, Register8, Register16, RepType};
use crate::cpu::{Cpu, ExecutionResult, Flag};
use crate::bus::{BusInterface};
use crate::io::IoBusInterface;

use crate::util;

impl Cpu {

    pub fn execute_instruction(&mut self, i: &Instruction, bus: &mut BusInterface, io_bus: &mut IoBusInterface) -> ExecutionResult {

        let mut unhandled: bool = false;
        let mut jump: bool = false;
        let mut cycles = 0;

        // Check for REPx prefixes
        if (i.prefixes & arch::OPCODE_PREFIX_REP1 != 0) || (i.prefixes & arch::OPCODE_PREFIX_REP2 != 0) {
            // A REPx prefix was set
            self.in_rep = true;
            // do we need the rep count?
        }

        match i.opcode {
            0x00 | 0x02 | 0x04 => {
                // ADD r/m8, r8 | r8, r/m8 | al, imm8
                // 8 bit ADD variants
                let op1_value = self.read_operand8(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand8(bus, i.operand2_type, i.segment_override).unwrap();
                // math_op8 handles flags
                let result = self.math_op8(Opcode::ADD, op1_value, op2_value);
                self.write_operand8(bus, i.operand1_type, i.segment_override, result);
            }
            0x01 | 0x03 | 0x05 => {
                // OR r/m16, r16 | r16, r/m16 | ax, imm16
                // 16 bit OR variants
                let op1_value = self.read_operand16(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand16(bus, i.operand2_type, i.segment_override).unwrap();
                // math_op8 handles flags
                let result = self.math_op16(Opcode::ADD, op1_value, op2_value);
                self.write_operand16(bus, i.operand1_type, i.segment_override, result);                
            }
            0x06 => {
                // PUSH es
                // Flags: None
                self.push_register16(bus, Register16::ES);
            }
            0x07 => {
                // POP es
                // Flags: None
                self.pop_register16(bus, Register16::ES);
            }
            0x08 | 0x0A | 0x0C => {
                // OR r/m8, r8 | r8, r/m8 | al, imm8
                // 8 bit OR variants
                let op1_value = self.read_operand8(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand8(bus, i.operand2_type, i.segment_override).unwrap();
                
                let result = op1_value | op2_value;
                self.write_operand8(bus, i.operand1_type, i.segment_override, result);

                // Clear carry & overflow flags
                // AoA 6.6.1
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u8(result);
            }
            0x09 | 0x0B | 0x0D => {
                // OR r/m16, r16 | r16, r/m16 | ax, imm16
                // 16 bit OR variants
                let op1_value = self.read_operand16(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand16(bus, i.operand2_type, i.segment_override).unwrap();
                
                let result = op1_value | op2_value;
                self.write_operand16(bus, i.operand1_type, i.segment_override, result);

                // Clear carry & overflow flags
                // AoA 6.6.1
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u16(result);
            }
            0x0E => {
                // PUSH cs
                // Flags: None
                self.push_register16(bus, Register16::CS);
            }
            0x0F => {
                // POP cs
                // Flags: None
                self.pop_register16(bus, Register16::CS);
            }
            0x10 | 0x12 | 0x14 => {
                // ADC r/m8,r8 | r8, r/m8 | al,imm8 
                // ADC 8-bit variants
                let op1_value = self.read_operand8(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand8(bus, i.operand2_type, i.segment_override).unwrap();

                // math_op8 handles flags
                let result = self.math_op8(Opcode::ADC,  op1_value,  op2_value);
                self.write_operand8(bus, i.operand1_type, i.segment_override, result);
                unhandled = true;
            }
            0x11 | 0x13 | 0x15 => {
                // ADC r/m16,r16 | r16, r/m16 | ax,imm16 
                // ADC 16-bit variants
                let op1_value = self.read_operand16(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand16(bus, i.operand2_type, i.segment_override).unwrap();

                // math_op8 handles flags
                let result = self.math_op16(Opcode::ADC,  op1_value,  op2_value);
                self.write_operand16(bus, i.operand1_type, i.segment_override, result);
            }
            0x16 => {
                // PUSH ss
                // Flags: None
                self.push_register16(bus, Register16::SS);
            }
            0x17 => {
                // POP ss
                // Flags: None
                self.pop_register16(bus, Register16::SS);
            }
            0x18 | 0x1A | 0x1C => {
                // SBB r/m8,r8 | r8, r/m8 | al,imm8 
                // SBB 8-bit variants
                let op1_value = self.read_operand8(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand8(bus, i.operand2_type, i.segment_override).unwrap();

                // math_op8 handles flags
                let result = self.math_op8(Opcode::SBB,  op1_value,  op2_value);
                self.write_operand8(bus, i.operand1_type, i.segment_override, result);
                unhandled = true;
            }
            0x19 | 0x1B | 0x1D => {
                // SBB r/m16,r16 | r16, r/m16 | ax,imm16 
                // SBB 16-bit variants
                let op1_value = self.read_operand16(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand16(bus, i.operand2_type, i.segment_override).unwrap();

                // math_op8 handles flags
                let result = self.math_op16(Opcode::SBB,  op1_value,  op2_value);
                self.write_operand16(bus, i.operand1_type, i.segment_override, result);
            }
            0x1E => {
                // PUSH ds
                // Flags: None
                self.push_register16(bus, Register16::DS);
            }
            0x1F => {
                // POP ds
                // Flags: None
                self.pop_register16(bus, Register16::DS);
            }
            0x20 | 0x22 | 0x24 => {
                // AND r/m8,r8 | r8, r/m8 | al,imm8 
                // AND 8-bit variants
                let op1_value = self.read_operand8(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand8(bus, i.operand2_type, i.segment_override).unwrap();

                // math_op8 handles flags
                let result = self.math_op8(Opcode::AND,  op1_value,  op2_value);
                self.write_operand8(bus, i.operand1_type, i.segment_override, result);
            }
            0x21 | 0x23 | 0x25 => {
                // AND r/m16,r16 | r16, r/m16 | ax,imm16 
                // AND 16-bit variants
                let op1_value = self.read_operand16(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand16(bus, i.operand2_type, i.segment_override).unwrap();

                // math_op8 handles flags
                let result = self.math_op16(Opcode::AND,  op1_value,  op2_value);
                self.write_operand16(bus, i.operand1_type, i.segment_override, result);
            }
            0x26 => {
                // ES Segment Override Prefix
            }
            0x27 => {
                // DAA
                unhandled = true;
            }
            0x28 | 0x2A | 0x2C => {
                // SUB r/m8,r8 | r8, r/m8 | al,imm8 
                // SUB 8-bit variants
                let op1_value = self.read_operand8(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand8(bus, i.operand2_type, i.segment_override).unwrap();

                // math_op8 handles flags
                let result = self.math_op8(Opcode::SUB,  op1_value,  op2_value);
                self.write_operand8(bus, i.operand1_type, i.segment_override, result);
            }
            0x29 | 0x2B | 0x2D => {
                // SUB r/m16,r16 | r16, r/m16 | ax,imm16 
                // SUB 16-bit variants
                let op1_value = self.read_operand16(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand16(bus, i.operand2_type, i.segment_override).unwrap();

                // math_op8 handles flags
                let result = self.math_op16(Opcode::SUB,  op1_value,  op2_value);
                self.write_operand16(bus, i.operand1_type, i.segment_override, result);
            }
            0x2E => {
                // CS Override Prefix
            }
            0x2F => {
                // DAS
                unhandled = true;
            }
            0x30 | 0x32 | 0x34 => {
                // XOR r/m8, r8  |  XOR r8, r/m8
                let op1_value = self.read_operand8(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand8(bus, i.operand2_type, i.segment_override).unwrap();
                
                let result = op1_value ^ op2_value;
                self.write_operand8(bus, i.operand1_type, i.segment_override, result);

                // Clear carry & overflow flags
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u8(result);
            }
            0x31 | 0x33 | 0x35 => {
                // XOR r/m16, r16 |  XOR r16, r/m16
                let op1_value = self.read_operand16(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand16(bus, i.operand2_type, i.segment_override).unwrap();
                
                let result = op1_value ^ op2_value;
                self.write_operand16(bus, i.operand1_type, i.segment_override, result);

                // Clear carry & overflow flags
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                self.set_flags_from_result_u16(result);                
            }
            0x36 => {
                // SS Segment Override Prefix
            }
            0x37 => {
                // AAA
                unhandled = true;
            }
            0x38 | 0x3A | 0x3C => {
                // CMP r/m8,r8 | r8, r/m8 | al,imm8 
                // CMP 8-bit variants
                let op1_value = self.read_operand8(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand8(bus, i.operand2_type, i.segment_override).unwrap();

                // math_op8 handles flags
                let result = self.math_op8(Opcode::CMP,  op1_value,  op2_value);
                self.write_operand8(bus, i.operand1_type, i.segment_override, result);
                unhandled = true;
            }
            0x39 | 0x3B | 0x3D => {
                // CMP r/m16,r16 | r16, r/m16 | ax,imm16 
                // CMP 16-bit variants
                let op1_value = self.read_operand16(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand16(bus, i.operand2_type, i.segment_override).unwrap();

                // math_op8 handles flags
                let result = self.math_op16(Opcode::CMP,  op1_value,  op2_value);
                self.write_operand16(bus, i.operand1_type, i.segment_override, result);
            }
            0x3E => {
                // DS Segment Override Prefix
            }
            0x3F => {
                // AAS
                unhandled = true;
            }
            0x40..=0x47 => {
                // INC r16 register-encoded operands
                let op1_value = self.read_operand16(bus, i.operand1_type, i.segment_override).unwrap();
                // math_op16 handles flags
                let result = self.math_op16(Opcode::INC, op1_value, 0);
                self.write_operand16(bus, i.operand1_type, i.segment_override, result);
            }
            0x48..=0x4F => {
                // DEC r16 register-encoded operands
                let op1_value = self.read_operand16(bus, i.operand1_type, i.segment_override).unwrap();
                // math_op16 handles flags
                let result = self.math_op16(Opcode::DEC, op1_value, 0);
                self.write_operand16(bus, i.operand1_type, i.segment_override, result);                
            }
            0x50 => {
                // PUSH ax
                // Flags: None                
                self.push_register16(bus, Register16::AX);
            }
            0x51 => {
                // PUSH cx
                // Flags: None                
                self.push_register16(bus, Register16::CX);
            }
            0x52 => {
                // PUSH dx
                // Flags: None                
                self.push_register16(bus, Register16::DX);
            }
            0x53 => {
                // PUSH bx
                // Flags: None
                self.push_register16(bus, Register16::BX);
            }
            0x54 => {
                // PUSH sp
                // Flags: None
                self.push_register16(bus, Register16::SP);
            }
            0x55 => {
                // PUSH bp
                // Flags: None             
                self.push_register16(bus, Register16::BP);
            }
            0x56 => {
                // PUSH si
                // Flags: None                
                self.push_register16(bus, Register16::SI);
            }
            0x57 => {
                // PUSH di
                // Flags: None
                self.push_register16(bus, Register16::DI);                 
            }
            0x58 => {
                // POP ax
                self.pop_register16(bus, Register16::AX);
            }
            0x59 => {
                unhandled = true;
            }
            0x5A => {
                unhandled = true;
            }
            0x5B => {
                unhandled = true;
            }
            0x5C => {
                unhandled = true;
            }
            0x5D => {
                unhandled = true;
            }
            0x5E => {
                unhandled = true;
            }
            0x5F => {
                unhandled = true;
            }
            0x60 => {
                unhandled = true;
            }
            0x61 => {
                unhandled = true;
            }
            0x62 => {
                unhandled = true;
            }
            0x63 => {
                unhandled = true;
            }
            0x64 => {
                unhandled = true;
            }
            0x65 => {
                unhandled = true;
            }
            0x66 => {
                unhandled = true;
            }
            0x67 => {
                unhandled = true;
            }
            0x68 => {
                unhandled = true;
            }
            0x69 => {
                unhandled = true;
            }
            0x6A => {
                unhandled = true;
            }
            0x6B => {
                unhandled = true;
            }
            0x6C => {
                unhandled = true;
            }
            0x6D => {
                unhandled = true;
            }
            0x6E => {
                unhandled = true;
            }
            0x6F => {
                unhandled = true;
            }
            0x70..=0x7F => {
                // JMP rel8 variants
                jump = match i.opcode {
                    0x70 => self.get_flag(Flag::Overflow),  // JO - Jump if overflow set
                    0x71 => !self.get_flag(Flag::Overflow), // JNO - Jump it overflow not set
                    0x72 => self.get_flag(Flag::Carry), // JB -> Jump if carry set
                    0x73 => !self.get_flag(Flag::Carry), // JNB -> Jump if carry not set
                    0x74 => self.get_flag(Flag::Zero), // JZ -> Jump if Zero set
                    0x75 => !self.get_flag(Flag::Zero), // JNZ -> Jump if Zero not set
                    0x76 => self.get_flag(Flag::Carry) | self.get_flag(Flag::Zero), // JBE -> Jump if Carry OR Zero
                    0x77 => !self.get_flag(Flag::Carry) & !self.get_flag(Flag::Zero), // JNBE -> Jump if Carry not set AND Zero not set
                    0x78 => self.get_flag(Flag::Sign), // JS -> Jump if Sign set
                    0x79 => !self.get_flag(Flag::Sign), // JNS -> Jump if Sign not set
                    0x7A => self.get_flag(Flag::Parity), // JP -> Jump if Parity set
                    0x7B => !self.get_flag(Flag::Parity), // JNP -> Jump if Parity not set
                    0x7C => self.get_flag(Flag::Sign) != self.get_flag(Flag::Overflow), // JL -> Jump if Sign flag != Overflow flag
                    0x7D => self.get_flag(Flag::Sign) == self.get_flag(Flag::Overflow), // JNL -> Jump if Sign flag == Overflow flag
                    0x7E => self.get_flag(Flag::Zero) | (self.get_flag(Flag::Sign) != self.get_flag(Flag::Overflow)),  // JLE ((ZF=1) OR (SF!=OF))
                    0x7F => !self.get_flag(Flag::Zero) & (self.get_flag(Flag::Sign) == self.get_flag(Flag::Overflow)), // JNLE ((ZF=0) AND (SF=OF))
                    _ => false
                };
                if jump {
                    if let OperandType::Relative8(rel8) = i.operand1_type {
                        self.ip = util::relative_offset_u16(self.ip, rel8 as i16 + i.size as i16 );
                    }
                }
            }
            0x80 | 0x82 => {
                // ADD/OR/ADC/SBB/AND/SUB/XOR/CMP r/m8, imm8
                let op1_value = self.read_operand8(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand8(bus, i.operand2_type, i.segment_override).unwrap();
                // math_op8 handles flags
                let result = self.math_op8(i.mnemonic, op1_value, op2_value);

                self.write_operand8(bus, i.operand1_type, i.segment_override, result);
            }
            0x81 => {
                // ADD/OR/ADC/SBB/AND/SUB/XOR/CMP r/m16, imm16
                let op1_value = self.read_operand16(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand16(bus, i.operand2_type, i.segment_override).unwrap();
                // math_op16 handles flags
                let result = self.math_op16(i.mnemonic, op1_value, op2_value);

                self.write_operand16(bus, i.operand1_type, i.segment_override, result);
            }
            0x83 => {
                // ADD/ADC/SBB/SUB/CMP r/m16, imm_i8
                let op1_value = self.read_operand16(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand8(bus, i.operand2_type, i.segment_override).unwrap();

                // imm_i8 gets sign-extended
                let op2_extended = util::sign_extend_u8_to_u16(op2_value);

                // math_op16 handles flags
                let result = self.math_op16(i.mnemonic, op1_value, op2_extended);      
                self.write_operand16(bus, i.operand1_type, i.segment_override, result);
            }
            0x84 => {
                unhandled = true;
            }
            0x85 => {
                unhandled = true;
            }
            0x86 => {
                // XCHG r8, r/m8
                let op1_value = self.read_operand8(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand8(bus, i.operand2_type, i.segment_override).unwrap();

                // Exchange values
                self.write_operand8(bus, i.operand1_type, i.segment_override, op2_value);
                self.write_operand8(bus, i.operand2_type, i.segment_override, op1_value);
            }
            0x87 => {
                // XCHG r16, r/m16
                let op1_value = self.read_operand16(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand16(bus, i.operand2_type, i.segment_override).unwrap();

                // Exchange values
                self.write_operand16(bus, i.operand1_type, i.segment_override, op2_value);
                self.write_operand16(bus, i.operand2_type, i.segment_override, op1_value);
            }
            0x88 | 0x8A => {
                // MOV r/m8, r8  |  MOV r8, r/m8
                let op_value = self.read_operand8(bus, i.operand2_type, i.segment_override).unwrap();
                self.write_operand8(bus, i.operand1_type, i.segment_override, op_value);
            }
            0x89 | 0x8B => {
                // MOV r/m16, r16  |  MOV r16, r/m16
                let op_value = self.read_operand16(bus, i.operand2_type, i.segment_override).unwrap();
                self.write_operand16(bus, i.operand1_type, i.segment_override, op_value);
            }
            0x8C | 0x8E => {
                // MOV SReg, r/m16  | MOV SReg, r/m16
                let op_value = self.read_operand16(bus, i.operand2_type, i.segment_override).unwrap();
                self.write_operand16(bus, i.operand1_type, i.segment_override, op_value);
            }
            0x8D => {
                unhandled = true;
            }
            0x8F => {
                unhandled = true;
            }
            0x90 => {
                unhandled = true;
            }
            0x91 => {
                unhandled = true;
            }
            0x92 => {
                unhandled = true;
            }
            0x93 => {
                unhandled = true;
            }
            0x94 => {
                unhandled = true;
            }
            0x95 => {
                unhandled = true;
            }
            0x96 => {
                unhandled = true;
            }
            0x97 => {
                unhandled = true;
            }
            0x98 => {
                unhandled = true;
            }
            0x99 => {
                unhandled = true;
            }
            0x9A => {
                unhandled = true;
            }
            0x9B => {
                unhandled = true;
            }
            0x9C => {
                // PUSHF - Push Flags
                self.push_flags(bus);
            }
            0x9D => {
                // POPF - Pop Flags
                self.pop_flags(bus);
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
                unhandled = true;
            }
            0xA1 => {
                unhandled = true;
            }
            0xA2 => {
                unhandled = true;
            }
            0xA3 => {
                unhandled = true;
            }
            0xA4 => {
                unhandled = true;
            }
            0xA5 => {
                unhandled = true;
            }
            0xA6 => {
                unhandled = true;
            }
            0xA7 => {
                unhandled = true;
            }
            0xA8 => {
                unhandled = true;
            }
            0xA9 => {
                unhandled = true;
            }
            0xAA => {
                // STOSB
                self.string_op(bus, Opcode::STOSB);

                // Check for end condition (CX==0)
                if self.in_rep {
                    self.decrement_register16(Register16::CX);
                    if self.get_register16(Register16::CX) == 0 {
                        self.in_rep = false;
                        self.rep_type = RepType::NoRep;
                    }
                }
            }
            0xAB => {
                // STOSW
                self.string_op(bus, Opcode::STOSW);
                // Check for end condition (CX==0)
                if self.in_rep {
                    self.decrement_register16(Register16::CX);
                    if self.get_register16(Register16::CX) == 0 {
                        self.in_rep = false;
                        self.rep_type = RepType::NoRep;
                    }
                }
            }
            0xAC => {
                // LODSB
                // Flags: None
                self.string_op(bus, i.mnemonic);
                
            }
            0xAD => {
                unhandled = true;
            }
            0xAE => {
                unhandled = true;
            }
            0xAF => {
                unhandled = true;
            }
            0xB0..=0xB7 => {
                // MOV r8, imm8
                if let OperandType::Immediate8(imm8) = i.operand2_type {
                    if let OperandType::Register8(reg) = i.operand1_type { 
                        self.set_register8(reg, imm8);
                    }
                }
                cycles = 4;
            }
            0xB8..=0xBF => {
                // MOV r16, imm16
                if let OperandType::Immediate16(imm16) = i.operand2_type {
                    if let OperandType::Register16(reg) = i.operand1_type {
                        self.set_register16(reg, imm16);
                    }
                }
            }
            0xC0 => {
                unhandled = true;
            }
            0xC1 => {
                unhandled = true;
            }
            0xC2 => {
                unhandled = true;
            }
            0xC3 => {
                // RETN - Return from call
                // Effectively, this instruction is pop ip
                let new_ip = self.pop_u16(bus);
                self.ip = new_ip;
                // RETN alters no flags
                jump = true
            }
            0xC4 => {
                unhandled = true;
            }
            0xC5 => {
                unhandled = true;
            }
            0xC6 => {
                // MOV r/m8, imm8
                let op2_value = self.read_operand8(bus, i.operand2_type, i.segment_override).unwrap();
                self.write_operand8(bus, i.operand1_type, i.segment_override, op2_value);
            }
            0xC7 => {
                // MOV r/m16, imm16
                let op2_value = self.read_operand16(bus, i.operand2_type, i.segment_override).unwrap();
                self.write_operand16(bus, i.operand1_type, i.segment_override, op2_value);
            }
            0xC8 => {
                unhandled = true;
            }
            0xC9 => {
                unhandled = true;
            }
            0xCA => {
                unhandled = true;
            }
            0xCB => {
                unhandled = true;
            }
            0xCC => {
                unhandled = true;
            }
            0xCD => {
                unhandled = true;
            }
            0xCE => {
                unhandled = true;
            }
            0xCF => {
                // IRET instruction
                self.end_interrupt(bus);
                jump = true;
            }
            0xD0 => {
                // ROL, ROR, RCL, RCR, SHL, SHR, SAR:  r/m8, 0x01
                let op1_value = self.read_operand8(bus, i.operand1_type, i.segment_override).unwrap();
                let result = self.bitshift_op8(i.mnemonic, op1_value, 1);
                self.write_operand8(bus, i.operand1_type, i.segment_override, result);
                // TODO: Cost
            }
            0xD1 => {
                unhandled = true;
            }
            0xD2 => {
                // ROL, ROR, RCL, RCR, SHL, SHR, SAR:  r/m8, cl
                let op1_value = self.read_operand8(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand8(bus, i.operand2_type, i.segment_override).unwrap();
                let result = self.bitshift_op8(i.mnemonic, op1_value, op2_value);
                self.write_operand8(bus, i.operand1_type, i.segment_override, result);
                // TODO: Cost
            }
            0xD3 => {
                // ROL, ROR, RCL, RCR, SHL, SHR, SAR:  r/m16, cl
                let op1_value = self.read_operand16(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand8(bus, i.operand2_type, i.segment_override).unwrap();
                let result = self.bitshift_op16(i.mnemonic, op1_value, op2_value);
                self.write_operand16(bus, i.operand1_type, i.segment_override, result);
                // TODO: Cost
            }
            0xD4 => {
                unhandled = true;
            }
            0xD5 => {
                unhandled = true;
            }
            0xD6 => {
                unhandled = true;
            }
            0xD7 => {
                unhandled = true;
            }
            0xD8 => {
                unhandled = true;
            }
            0xD9 => {
                unhandled = true;
            }
            0xDA => {
                unhandled = true;
            }
            0xDB => {
                unhandled = true;
            }
            0xDC => {
                unhandled = true;
            }
            0xDD => {
                unhandled = true;
            }
            0xDE => {
                unhandled = true;
            }
            0xDF => {
                unhandled = true;
            }
            0xE0 => {
                // LOOPNE - Decrement CX, Jump short if count!=0 and ZF=0
                // loop does not modify flags
                self.decrement_register16(Register16::CX);
                if self.cx != 0 {
                    if !self.get_flag(Flag::Zero) {
                        if let OperandType::Relative8(rel8) = i.operand1_type {
                            self.ip = util::relative_offset_u16(self.ip, rel8 as i16 + i.size as i16 );
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
                        if let OperandType::Relative8(rel8) = i.operand1_type {
                            self.ip = util::relative_offset_u16(self.ip, rel8 as i16 + i.size as i16 );
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
                    if let OperandType::Relative8(rel8) = i.operand1_type {
                        self.ip = util::relative_offset_u16(self.ip, rel8 as i16 + i.size as i16 );
                        jump = true;
                    }
                }
            }
            0xE3 => {
                unhandled = true;
            }
            0xE4 => {
                // IN al, imm8
                let op2_value = self.read_operand8(bus, i.operand2_type, i.segment_override).unwrap(); 
                
                let in_byte = io_bus.read_u8(op2_value as u16);
                self.set_register8(Register8::AL, in_byte);
                //println!("IN: Would input value from port {:#02X}", op2_value);                
            }
            0xE5 => {
                unhandled = true;
            }
            0xE6 => {
                // OUT imm8, al
                let op1_value = self.read_operand8(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand8(bus, i.operand2_type, i.segment_override).unwrap();                

                // Write to port
                io_bus.write_u8(op1_value as u16, op2_value);
                //println!("OUT: Would output {:02X} to Port {:#02X}", op2_value, op1_value);

            }
            0xE7 => {
                unhandled = true;
            }
            0xE8 => {
                // CALL rel16
                // Push offset of next instruction (CALL rel16 is 3 bytes)
                let ip = self.get_register16(Register16::IP);
                let next_i = ip + 3;
                self.push_u16(bus, next_i);

                // Add rel16 to ip
                let rel = self.read_operand16(bus, i.operand1_type, i.segment_override).unwrap();
                self.ip = self.ip.wrapping_add(rel);
            }
            0xE9 => {
                // JMP rel16
                if let OperandType::Relative16(rel16) = i.operand1_type {
                    self.ip = util::relative_offset_u16(self.ip, rel16 as i16 + i.size as i16 );
                }
                jump = true;
                // cycles? 
            }
            0xEA => {
                // JMP FAR
                if let OperandType::FarAddress(segment, offset) = i.operand1_type {                
                    self.cs = segment;
                    self.ip = offset;
                }
                jump = true;
                cycles = 24;
            }
            0xEB => {
                // JMP rel8
                if let OperandType::Relative8(rel8) = i.operand1_type {
                    self.ip = util::relative_offset_u16(self.ip, rel8 as i16 + i.size as i16 );
                }
                jump = true
            }
            0xEC => {
                // IN al, dx
                let op2_value = self.read_operand16(bus, i.operand2_type, i.segment_override).unwrap(); 
                let in_byte = io_bus.read_u8(op2_value);
                self.set_register8(Register8::AL, in_byte);
            }
            0xED => {
                unhandled = true;
            }
            0xEE => {
                // OUT dx, al
                let op1_value = self.read_operand16(bus, i.operand1_type, i.segment_override).unwrap();
                let op2_value = self.read_operand8(bus, i.operand2_type, i.segment_override).unwrap();                

                io_bus.write_u8(op1_value as u16, op2_value);
                //println!("OUT: Would output {:02X} to Port {:#04X}", op2_value, op1_value);                
            }
            0xEF => {
                unhandled = true;
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
                unhandled = true;
            }
            0xF6 => {
                // Miscellaneous Opcode Extensions
                match i.mnemonic {

                    Opcode::TEST => {
                        let op1_value = self.read_operand8(bus, i.operand1_type, i.segment_override).unwrap();
                        let op2_value = self.read_operand8(bus, i.operand1_type, i.segment_override).unwrap();
                        let result = self.math_op8(i.mnemonic, op1_value, op2_value);
                    }
                    Opcode::NOT => {
                        let op1_value = self.read_operand8(bus, i.operand1_type, i.segment_override).unwrap();
                        let result = self.math_op8(i.mnemonic, op1_value, 0);
                        self.write_operand8(bus, i.operand1_type, i.segment_override, result);
                    }
                    Opcode::NEG => {
                        let op1_value = self.read_operand8(bus, i.operand1_type, i.segment_override).unwrap();
                        let result = self.math_op8(i.mnemonic, op1_value, 0);
                        self.write_operand8(bus, i.operand1_type, i.segment_override, result);
                    }
                    _=> unhandled = true
                }
            }
            0xF7 => {
                unhandled = true;
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
                let op_value = self.read_operand8(bus, i.operand1_type, i.segment_override).unwrap();
                let result = self.math_op8(i.mnemonic, op_value, 0);
                self.write_operand8(bus, i.operand1_type, i.segment_override, result);
                // cycles ?
            }
            0xFF => {
                // Several opcode extensions here
                match i.mnemonic {
                    // INC/DEC r/m16
                    Opcode::INC | Opcode::DEC => {
                        let op_value = self.read_operand16(bus, i.operand1_type, i.segment_override).unwrap();
                        let result = self.math_op16(i.mnemonic, op_value, 0);
                        self.write_operand16(bus, i.operand1_type, i.segment_override, result);
                    },
                    _=> {
                        unhandled = true;
                    }
                }

                // cycles ?
            }
            _ => {
                return ExecutionResult::UnsupportedOpcode(i.opcode);
            }

        }

        if unhandled {
            ExecutionResult::UnsupportedOpcode(i.opcode)
        }
        else {
            if self.halted{
                ExecutionResult::Halt
            }
            else if jump {
                ExecutionResult::OkayJump
            }
            else if self.in_rep {
                ExecutionResult::OkayRep
            }
            else {
                ExecutionResult::Okay
            }
        }
    }
}
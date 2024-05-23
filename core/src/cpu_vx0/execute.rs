/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2024 Daniel Balsom

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

    cpu_vx0::execute.rs

    Executes an instruction after it has been fetched.
    Includes all main opcode implementations.
*/

use crate::{
    cpu_common::{
        CpuException,
        ExecutionResult,
        Mnemonic,
        OperandType,
        QueueOp,
        Segment,
        OPCODE_PREFIX_REP1,
        OPCODE_PREFIX_REP2,
        OPCODE_PREFIX_REP3,
        OPCODE_PREFIX_REP4,
        OPCODE_PREFIX_REPMASK,
    },
    cpu_vx0::{biu::*, *},
    util,
};
use ExecutionResult::Okay;

/*
macro_rules! read_operand {
    ($self:ident, $op: expr) => {
        {
            if $self.i.opcode & 0x01 == 0 {
                $self.op1_8 = $self.read_operand8($op, $self.i.segment_override).unwrap()
            }
            else {
                $self.op1_16 = $self.read_operand16($op, $self.i.segment_override).unwrap()
            }
        }
    };
}

macro_rules! write_operand {
    ($self:ident, $op: expr, $value: expr, $flag: expr ) => {
        {
            if $self.i.opcode & 0x01 == 0 {
                $self.write_operand8($op, $self.i.segment_override, $self.result_8, $flag)
            }
            else {
                $self.write_operand16($op, $self.i.segment_override, $self.result_16, $flag)
            }
        }
    };
}

macro_rules! alu_op {
    ($self:ident) => {
        if $self.i.opcode & 0x01 == 0 {
            $self.result_8 = $self.math_op8($self.i.mnemonic, $self.op1_8, $self.op2_8)
        }
        else {
            $self.result_16 = $self.math_op16($self.i.mnemonic, $self.op1_16, $self.op2_16)
        }
    }
}
*/

// rustfmt chokes on large match statements.
#[rustfmt::skip]
impl NecVx0 {
    /// Execute the current instruction. At the phase this function is called we have
    /// fetched and decoded any prefixes, the opcode byte, modrm and any displacement
    /// and populated an Instruction struct.
    ///
    /// Additionally, if an EA was to be loaded, the load has already been performed.
    ///
    /// For each opcode, we execute cycles equivalent to the microcode routine for
    /// that function. Microcode line numbers are usually provided for cycle tracing.
    ///
    /// The microcode instruction with a terminating RNI should not be executed, as this
    /// requires the next instruction byte to be fetched and is handled by finalize().
    #[rustfmt::skip]
    pub fn execute_instruction(&mut self) -> ExecutionResult {
        let mut unhandled: bool = false;
        let mut jump: bool = false;
        let mut exception: CpuException = CpuException::NoException;

        self.step_over_target = None;

        self.trace_comment("EXECUTE");
        
        // TODO: Check optimization here. We could reset several flags at once if they were in a
        //       bitfield.
        // Reset instruction reentrancy flag
        self.instruction_reentrant = false;
        
        // Reset jumped flag.
        self.jumped = false;
        
            // Reset trap suppression flag
        self.trap_suppressed = false;

        // Decrement trap counters.
        self.trap_enable_delay = self.trap_enable_delay.saturating_sub(1);
        self.trap_disable_delay = self.trap_disable_delay.saturating_sub(1);

        // If we have an NX loaded RNI cycle from the previous instruction, execute it.
        // Otherwise, wait one cycle before beginning instruction if there was no modrm.
        if self.nx {
            self.trace_comment("RNI");
            self.cycle();
            self.nx = false;
        }
        else if self.last_queue_op == QueueOp::First {
            self.cycle();
        }

        // Check for REPx prefixes
        if (self.i.prefixes & OPCODE_PREFIX_REPMASK != 0) {
            // A REPx prefix was set

            let mut invalid_rep = false;

            match self.i.mnemonic {
                Mnemonic::INSB | Mnemonic::INSW | Mnemonic::OUTSB | Mnemonic::OUTSW => {
                    self.rep_type = RepType::Rep;
                }
                Mnemonic::STOSB | Mnemonic::STOSW | Mnemonic::LODSB | Mnemonic::LODSW | Mnemonic::MOVSB | Mnemonic::MOVSW => {
                    self.rep_type = RepType::Rep;
                }
                Mnemonic::SCASB | Mnemonic::SCASW | Mnemonic::CMPSB | Mnemonic::CMPSW => {
                    // Valid string ops with REP prefix
                    if self.i.prefixes & OPCODE_PREFIX_REP1 != 0 {
                        self.rep_type = RepType::Repne;
                    }
                    else if self.i.prefixes & OPCODE_PREFIX_REP2 != 0 {
                        self.rep_type = RepType::Repe;
                    }
                    else if self.i.prefixes & OPCODE_PREFIX_REP3 != 0 {
                        self.rep_type = RepType::Repnc;
                    }
                    else if self.i.prefixes & OPCODE_PREFIX_REP4 != 0 {
                        self.rep_type = RepType::Repc;
                    }
                    else {
                        invalid_rep = true;
                    }
                }
                Mnemonic::MUL | Mnemonic::IMUL | Mnemonic::DIV | Mnemonic::IDIV => {
                    // REP prefix on MUL/DIV negates the product/quotient.
                    self.rep_type = RepType::MulDiv;
                }
                _ => {
                    invalid_rep = true;
                    //return ExecutionResult::ExecutionError(
                    //    format!("REP prefix on invalid opcode: {:?} at [{:04X}:{:04X}].", self.i.mnemonic, self.cs, self.ip)
                    //);
                    
                    /*
                    log::warn!(
                       "REP prefix on invalid opcode: {:?} at [{:04X}:{:04X}].",
                       self.i.mnemonic,
                       self.cs,
                       self.ip(),
                    );
                    */
                }
            }

            if !invalid_rep {
                self.in_rep = true;
                self.rep_mnemonic = self.i.mnemonic;
            }
        }

        // Reset the wait cycle after STI
        self.interrupt_inhibit = false;

        // Most instructions will issue an RNI. We can set RNI to false for those that don't.
        //self.rni = true;

        // Keep a tally of how many Opcode 0x00's we've executed in a row. Too many likely means we've run
        // off the rails into uninitialized memory, whereupon we halt so that we can check things out.

        // This is now optional in the configuration file, as some test applications like acid88 won't work
        // otherwise.
        if self.i.opcode == 0x00 {
            self.opcode0_counter = self.opcode0_counter.wrapping_add(1);

            if self.off_rails_detection && (self.opcode0_counter > 5) {
                // Halt permanently by clearing interrupt flag
                self.clear_flag(Flag::Interrupt);
                self.halted = true;
                self.instruction_reentrant = true;
            }
        }
        else {
            self.opcode0_counter = 0;
        }

        // Main opcode dispatch
        match self.i.opcode {
            0x00 | 0x02 | 0x04 |  // ADD r/m8, r8 | r8, r/m8 | al, imm8
            0x08 | 0x0A | 0x0C |  // OR  r/m8, r8 | r8, r/m8 | al, imm8
            0x10 | 0x12 | 0x14 |  // ADC r/m8, r8 | r8, r/m8 | al, imm8 
            0x18 | 0x1A | 0x1C |  // SBB r/m8, r8 | r8, r/m8 | al, imm8 
            0x20 | 0x22 | 0x24 |  // AND r/m8, r8 | r8, r/m8 | al, imm8 
            0x28 | 0x2A | 0x2C |  // SUB r/m8, r8 | r8, r/m8 | al, imm8 
            0x30 | 0x32 | 0x34 => // XOR r/m8, r8 | r8, r/m8 | al, imm8
            { 
                // 16 bit ADD variants
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();
                
                self.cycles_nx_i(2, &[0x008, 0x009]);

                if let OperandType::AddressingMode(_) = self.i.operand1_type {
                    self.cycles_i(2, &[0x009, 0x00a]);
                }

                let result = self.math_op8(self.i.mnemonic, op1_value, op2_value);
                self.write_operand8(self.i.operand1_type, self.i.segment_override, result, ReadWriteFlag::RNI);
            }
            0x01 | 0x03 | 0x05 |  // ADD r/m16, r16 | r16, r/m16 | ax, imm16
            0x09 | 0x0B | 0x0D |  // OR  r/m16, r16 | r16, r/m16 | ax, imm16
            0x11 | 0x13 | 0x15 |  // ADC r/m16, r16 | r16, r/m16 | ax, imm16 
            0x19 | 0x1B | 0x1D |  // SBB r/m16, r16 | r16, r/m16 | ax, imm16 
            0x21 | 0x23 | 0x25 |  // AND r/m16, r16 | r16, r/m16 | ax, imm16 
            0x29 | 0x2B | 0x2D |  // SUB r/m16, r16 | r16, r/m16 | ax, imm16 
            0x31 | 0x33 | 0x35 => // XOR r/m16, r16 | r16, r/m16 | ax, imm16
            {
                // 16 bit ADD variants
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                
                self.cycles_nx_i(2, &[0x008, 0x009]);

                if let OperandType::AddressingMode(_) = self.i.operand1_type {
                    self.cycles_i(2, &[0x009, 0x00a]);
                }

                let result = self.math_op16(self.i.mnemonic, op1_value, op2_value);
                self.write_operand16(self.i.operand1_type, self.i.segment_override, result, ReadWriteFlag::RNI);           
            }
            0x06 => {
                // PUSH es
                // Flags: None
                self.cycles_i(3, &[0x02c, 0x02d, 0x023]);
                self.push_register16(Register16::ES, ReadWriteFlag::RNI);
            }
            0x07 => {
                // POP es
                // Flags: None
                self.pop_register16(Register16::ES, ReadWriteFlag::RNI);
                //self.cycle();
            }
            0x0E => {
                // PUSH cs
                // Flags: None
                self.cycles_i(3, &[0x02c, 0x02d, 0x023]);
                self.push_register16(Register16::CS, ReadWriteFlag::RNI);
            }
            0x0F => {
                // POP cs
                // Flags: None
                self.pop_register16(Register16::CS, ReadWriteFlag::RNI);
                //self.cycle();
            }
            0x16 => {
                // PUSH ss
                // Flags: None
                self.cycles_i(3, &[0x02c, 0x02d, 0x023]);
                self.push_register16(Register16::SS, ReadWriteFlag::RNI);
            }
            0x17 => {
                // POP ss
                // Flags: None
                self.pop_register16(Register16::SS, ReadWriteFlag::RNI);
                //self.cycle();
            }
            0x1E => {
                // PUSH ds
                // Flags: None
                self.cycles_i(3, &[0x02c, 0x02d, 0x023]);
                self.push_register16(Register16::DS, ReadWriteFlag::RNI);
            }
            0x1F => {
                // POP ds
                // Flags: None
                self.pop_register16(Register16::DS, ReadWriteFlag::RNI);
                //self.cycle();
            }
            // ES Segment Override Prefix
            0x26 => {}
            0x27 => {
                // DAA — Decimal Adjust AL after Addition
                self.cycles_nx_i(3, &[0x144, 0x145, 0x146]);
                self.daa();
            }
            // CS Override Prefix
            0x2E => {}
            0x2F => {
                // DAS
                self.cycles_nx_i(3, &[0x144, 0x145, 0x146]);                
                self.das();
            }
            // SS Segment Override Prefix
            0x36 => {}
            0x37 => {
                // AAA
                self.aaa();
            }
            0x38 | 0x3A | 0x3C => {
                // CMP r/m8,r8 | r8, r/m8 | al,imm8 
                // CMP 8-bit variants
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();
                
                if self.i.opcode == 0x3C {
                    // 0x018
                    self.cycles_nx_i(2, &[MC_JUMP, 0x01a]);
                }
                else {
                    // 0x008
                    self.cycles_nx_i(2, &[0x008, 0x009]);
                }
                
                let _result = self.math_op8(Mnemonic::CMP,  op1_value,  op2_value);
                //self.write_operand8(self.i.operand1_type, self.i.segment_override, result);
            }
            0x39 | 0x3B | 0x3D => {
                // CMP r/m16,r16 | r16, r/m16 | ax,imm16 
                // CMP 16-bit variants
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();

                if self.i.opcode == 0x3D {
                    // 0x018
                    self.cycle_nx_i(0x01a);
                }
                else {
                    // 0x008
                    self.cycles_nx_i(2, &[0x008, 0x009]);
                }

                let _result = self.math_op16(Mnemonic::CMP,  op1_value,  op2_value);
                //self.write_operand16(self.i.operand1_type, self.i.segment_override, result);
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
                self.write_operand16(self.i.operand1_type, self.i.segment_override, result, ReadWriteFlag::RNI);

                self.cycles_nx(1);
            }
            0x48..=0x4F => {
                // DEC r16 register-encoded operands
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                // math_op16 handles flags
                let result = self.math_op16(Mnemonic::DEC, op1_value, 0);
                self.write_operand16(self.i.operand1_type, self.i.segment_override, result, ReadWriteFlag::RNI);

                self.cycles_nx(1);         
            }
            0x50..=0x57 => {
                // PUSH reg16
                // Flags: None
                let reg = REGISTER16_LUT[(self.i.opcode & 0x07) as usize];
                self.cycles_i(3, &[0x028, 0x029, 0x02a]);

                self.push_register16(reg, ReadWriteFlag::RNI);
            }
            0x58..=0x5F => {
                // POP reg16
                // Flags: None
                let reg = REGISTER16_LUT[(self.i.opcode & 0x07) as usize];

                self.pop_register16(reg, ReadWriteFlag::RNI);
                self.cycle_nx_i(0x035);
            }
            0x60 => {
                // PUSHA
                let saved_sp = self.get_register16(Register16::SP);
                self.cycles(2);
                self.push_register16(Register16::AX, ReadWriteFlag::Normal);
                self.cycles(1);
                self.push_register16(Register16::CX, ReadWriteFlag::Normal);
                self.push_register16(Register16::DX, ReadWriteFlag::Normal);
                self.push_register16(Register16::BX, ReadWriteFlag::Normal);
                self.push_u16(saved_sp, ReadWriteFlag::Normal);
                self.push_register16(Register16::BP, ReadWriteFlag::Normal);
                self.push_register16(Register16::SI, ReadWriteFlag::Normal);
                self.push_register16(Register16::DI, ReadWriteFlag::RNI);
            }
            0x61 => {
                // POPA
                self.pop_register16(Register16::DI, ReadWriteFlag::Normal);
                self.pop_register16(Register16::SI, ReadWriteFlag::Normal);
                self.pop_register16(Register16::BP, ReadWriteFlag::Normal);
                self.set_register16(Register16::SP, self.get_register16(Register16::SP).wrapping_add(2));
                self.pop_register16(Register16::BX, ReadWriteFlag::Normal);
                self.pop_register16(Register16::DX, ReadWriteFlag::Normal);
                self.pop_register16(Register16::CX, ReadWriteFlag::Normal);
                self.pop_register16(Register16::AX, ReadWriteFlag::RNI);
            }
            0x62 => {
                // BOUND
                
                let idx = self.read_operand16(self.i.operand1_type, None).unwrap() as i16;
                let bounds_opt = self.read_operand_m16m16(self.i.operand2_type, self.i.segment_override, ReadWriteFlag::Normal);
                
                // 63 always throws exception.
                if self.i.opcode & 0x01 != 0 {
                    self.sw_interrupt(5);
                    exception = CpuException::BoundsException;
                    jump = true;
                }
                else if let Some((start_u, end_u)) = bounds_opt {
                    let start_i = start_u as i16;
                    let end_i = end_u as i16;
                    
                    if (idx >= start_i) && (idx <= end_i) {
                        //log::warn!("BOUND: In bounds: {} <= {} <= {}", start_i, idx, end_i);
                        // In bounds!
                    }
                    else {
                        //log::warn!("BOUND: Out of bounds: {} <= {} <= {}", start_i, idx, end_i);
                        // Bounds range exception
                        self.sw_interrupt(5);
                        exception = CpuException::BoundsException;
                        jump = true;
                    }
                }
                else {
                    // Illegal form of BOUND. A real V20 will halt now.
                    log::warn!("BOUND: Illegal form of BOUND!");
                    self.halted = true;
                }
            }
            0x63 => {
                // UNDEFINED (?)
                let _ = self.read_operand16(self.i.operand1_type, self.i.segment_override);
            }
            0x64 | 0x65 => {},
            0x68 => {
                // PUSH imm16
                let imm16 = self.read_operand16(self.i.operand1_type, None).unwrap();
                self.push_u16(imm16, ReadWriteFlag::RNI);
            }
            0x69 => {
                // IMUL r16, r/m16, imm16
                let op3_value = self.q_read_u16(QueueType::Subsequent, QueueReader::Eu);
                let mul_op = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                // Truncate product.
                let (_, product) = self.mul16(mul_op, op3_value, true, false);

                self.write_operand16(self.i.operand1_type, None, product, ReadWriteFlag::RNI);

                if let OperandType::Register8(_) = self.i.operand1_type {
                    self.cycle();
                }

                self.clear_flag(Flag::AuxCarry);
                self.set_szp_flags_from_result_u16(product);
            }
            0x6A => {
                // PUSH imm8
                let imm8 = self.read_operand8(self.i.operand1_type, None).unwrap() as i8 as i16 as u16;
                self.push_u16(imm8 as u16, ReadWriteFlag::RNI);
            }
            0x6B => {
                // IMUL r16, r/m16, imm8
                // Sign extend immediate byte operand.
                let op3_value = self.q_read_u8(QueueType::Subsequent, QueueReader::Eu) as i8 as i16;
                let mul_op = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                // Truncate product.
                let (_, product) = self.mul16(mul_op, op3_value as u16, true, false);
                
                self.write_operand16(self.i.operand1_type, None, product, ReadWriteFlag::RNI);

                if let OperandType::Register8(_) = self.i.operand1_type {
                    self.cycle();
                }

                self.set_szp_flags_from_result_u8(product as u8);
            }
            0x6C | 0x6D | 0x6E | 0x6F => {
                // INSB | INSW | OUTSB | OUTSW
                // rep_start() will terminate early if CX==0
                if self.rep_start() {
                    self.string_op(self.i.mnemonic, None);
                    self.cycle_i(0x130);

                    // Check for end condition (CX==0)
                    if self.in_rep {
                        self.decrement_register16(Register16::CX); // 131
                        // Check for interrupt
                        if self.intr_pending {
                            self.cycles_i(2, &[0x131, MC_JUMP]); // Jump to RPTI
                            self.rep_interrupt();
                        }
                        else {
                            self.cycles_i(2, &[0x131, 0x132]);
                            if self.c.x() == 0 {
                                // Fall through to 133, RNI
                                self.rep_end();
                            }
                            else {
                                self.cycle_i(MC_JUMP); // jump to 1
                            }
                        }
                    }
                    else {
                        // End non-rep prefixed MOVSB
                        self.cycle_i(MC_JUMP); // jump to 133, RNI
                    }
                }
            }
            0x6C..=0x6F => {
                unhandled = true;
            }
            0x70..=0x7F => {
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

                let rel8 = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                self.cycle_i(0x0e9);

                if jump {
                    self.reljmp2(rel8 as i8 as i16, true);
                }
                /*
                else {
                    self.cycle_i(0x0ea);
                }
                */
            }
            0x80 | 0x82 => {
                // ADD/OR/ADC/SBB/AND/SUB/XOR/CMP r/m8, imm8
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();
                
                self.cycle_nx();
                let result = self.math_op8(self.i.mnemonic, op1_value, op2_value);

                if let OperandType::AddressingMode(_) = self.i.operand1_type {
                    self.cycles_i(2, &[0x00e, 0x00f]);
                }

                if self.i.mnemonic != Mnemonic::CMP {
                    self.write_operand8(self.i.operand1_type, self.i.segment_override, result, ReadWriteFlag::RNI);
                }
            }
            0x81 => {
                // ADD/OR/ADC/SBB/AND/SUB/XOR/CMP r/m16, imm16
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                
                self.cycle_nx();
                let result = self.math_op16(self.i.mnemonic, op1_value, op2_value);

                if let OperandType::AddressingMode(_) = self.i.operand1_type {
                    if self.i.mnemonic != Mnemonic::CMP {
                        self.cycles_i(2, &[0x00e, 0x00f]);
                    }
                    else {
                        self.cycles_nx_i(2, &[0x00e, 0x00f]);
                    }
                }

                if self.i.mnemonic != Mnemonic::CMP {
                    self.write_operand16(self.i.operand1_type, self.i.segment_override, result, ReadWriteFlag::RNI);
                }
            }
            0x83 => {
                // ADD/ADC/SBB/SUB/CMP r/m16, imm8 (sign-extended)
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();

                let sign_extended = op2_value as i8 as i16 as u16;

                // math_op16 handles flags
                let result = self.math_op16(self.i.mnemonic, op1_value, sign_extended);

                if let OperandType::AddressingMode(_) = self.i.operand1_type {
                    self.cycles_i(2, &[0x00e, 0x00f]);
                }

                if self.i.mnemonic != Mnemonic::CMP {
                    self.write_operand16(self.i.operand1_type, self.i.segment_override, result, ReadWriteFlag::RNI);
                }
            }            
            0x84 => {
                // TEST r/m8, r8
                // Flags: o..sz.pc
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();
                
                self.math_op8(Mnemonic::TEST, op1_value, op2_value);
                self.cycles_nx_i(2, &[0x94]);
                
                /*
                if let OperandType::AddressingMode(_) = self.i.operand1_type {
                    self.cycles_i(2, &[0x95, 0x96]);
                }
                */
            }
            0x85 => {
                // TEST r/m16, r16
                // Flags: o..sz.pc                
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                // math_op16 handles flags
                self.math_op16(Mnemonic::TEST, op1_value, op2_value);
                self.cycles_nx_i(2, &[0x94]);
                /*
                if let OperandType::AddressingMode(_) = self.i.operand1_type {
                    self.cycle();
                } 
                */               
            }
            0x86 => {
                // XCHG r8, r/m8
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();

                self.cycles_nx(3);
                
                if let OperandType::AddressingMode(_) = self.i.operand2_type {
                    // Memory operand takes 2 more cycles
                    self.cycles(2);
                }

                // Exchange values. Write operand2 first so we don't affect EA calculation if EA includes register being swapped.
                self.write_operand8(self.i.operand2_type, self.i.segment_override, op1_value, ReadWriteFlag::RNI);
                self.write_operand8(self.i.operand1_type, self.i.segment_override, op2_value, ReadWriteFlag::Normal);
            }
            0x87 => {
                // XCHG r16, r/m16
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();

                self.cycles_nx(3);

                if let OperandType::AddressingMode(_) = self.i.operand2_type {
                    // Memory operand takes 2 more cycles
                    self.cycles(2);
                }

                // Exchange values. Write operand2 first so we don't affect EA calculation if EA includes register being swapped.
                self.write_operand16(self.i.operand2_type, self.i.segment_override, op1_value, ReadWriteFlag::RNI);
                self.write_operand16(self.i.operand1_type, self.i.segment_override, op2_value, ReadWriteFlag::Normal);
            }
            0x88 | 0x8A => {
                // MOV r/m8, r8  |  MOV r8, r/m8
                self.cycle_nx();
                let op_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();

                if let OperandType::AddressingMode(_) = self.i.operand1_type {
                    self.cycles_i(2, &[0x000, 0x001]);
                }
                self.write_operand8(self.i.operand1_type, self.i.segment_override, op_value, ReadWriteFlag::RNI);
            }
            0x89 | 0x8B => {
                // MOV r/m16, r16  |  MOV r16, r/m16
                self.cycle_nx();
                let op_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();

                if let OperandType::AddressingMode(_) = self.i.operand1_type {
                    self.cycles_i(2, &[0x000, 0x001]);
                }
                self.write_operand16(self.i.operand1_type, self.i.segment_override, op_value, ReadWriteFlag::RNI);
            }
            0x8C | 0x8E => {
                // MOV r/m16, SReg | MOV SReg, r/m16

                if let OperandType::AddressingMode(_) = self.i.operand1_type {
                    self.cycle_i(0x0ec);
                }
                let op_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                self.write_operand16(self.i.operand1_type, self.i.segment_override, op_value, ReadWriteFlag::RNI);
            }
            0x8D => {
                // LEA - Load Effective Address
                let ea = self.load_effective_address(self.i.operand2_type);
                match ea {
                    Some(value) => {
                        self.write_operand16(self.i.operand1_type, None, value, ReadWriteFlag::RNI);
                    }
                    None => {
                        // In the event of an invalid (Register) operand2, operand1 is set to the last EA calculated by an instruction.
                        self.write_operand16(self.i.operand1_type, None, self.last_ea, ReadWriteFlag::RNI);
                        //self.cycles(1);
                    }
                }
            }
            0x8F => {
                // POP r/m16
                self.cycle_i(0x040);
                let value = self.pop_u16();
                self.cycle_i(0x042);
                if let OperandType::AddressingMode(_) = self.i.operand1_type {
                    self.cycles_i(2, &[0x043, 0x044]);
                }                   
                self.write_operand16(self.i.operand1_type, self.i.segment_override, value, ReadWriteFlag::RNI);
            }
            0x90..=0x97 => {
                // XCHG AX, r
                // Cycles: 3 (1 Fetch + 2 EU)
                // Flags: None
                let op_reg = REGISTER16_LUT[(self.i.opcode & 0x07) as usize];
                let ax_value = self.a.x();
                let op_reg_value = self.get_register16(op_reg);

                self.cycles_nx_i(2, &[0x084, 0x085]);

                self.set_register16(Register16::AX, op_reg_value);
                self.set_register16(op_reg, ax_value);
            }
            0x98 => {
                // CBW - Convert Byte to Word
                // Flags: None
                if self.a.l() & 0x80 != 0 {
                    self.a.set_h(0xFF);
                }
                else {
                    self.a.set_h(0);
                }
            }
            0x99 => {
                // CWD - Convert Word to Doubleword
                // Flags: None
                self.cycles(3);
                if self.a.x() & 0x8000 == 0 {
                    self.d.set_x(0x0000);
                }
                else {
                    self.cycle(); // Microcode jump @ 05a
                    self.d.set_x(0xFFFF);
                }
            }
            0x9A => {
                // CALLF - Call Far addr16:16
                // This instruction reads a direct FAR address from the instruction queue. (See 0xEA for its twin JMPF)
                let (segment, offset) = self.read_operand_faraddr();
                self.farcall(segment, offset, true);

                // Save next address if we step over this CALL.
                self.step_over_target = Some(CpuAddress::Segmented(self.cs, self.ip()));

                self.push_call_stack(
                    CallStackEntry::CallF {
                        ret_cs: self.cs,
                        ret_ip: self.ip(),
                        call_cs: segment,
                        call_ip: offset
                    },
                    self.cs,
                    self.ip(),
                );

                /*
                self.cs = segment;
                self.ip = offset;

                // NEARCALL
                self.biu_queue_flush();
                self.cycles_i(3, &[0x077, 0x078, 0x079]); 
                self.push_u16(next_i, ReadWriteFlag::RNI);
                */
                jump = true;
            }
            0x9B => {
                // WAIT
                self.cycles(3);
            }
            0x9C => {
                // PUSHF - Push Flags
                self.cycles(3);
                self.push_flags(ReadWriteFlag::RNI);
            }
            0x9D => {
                // POPF - Pop Flags
                self.pop_flags();
            }
            0x9E => {
                // SAHF - Store AH into Flags
                self.store_flags(self.a.h() as u16);
            }
            0x9F => {
                // LAHF - Load Status Flags into AH Register
                let flags = self.load_flags() as u8;
                self.set_register8(Register8::AH, flags);
            }
            0xA0 => {
                // MOV al, offset8
                // These MOV variants are unique in that they take a direct offset with no modr/m byte
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();
                //self.cycle_i(0x063);
                self.set_register8(Register8::AL, op2_value);
            }
            0xA1 => {
                // MOV AX, offset16
                // These MOV variants are unique in that they take a direct offset with no modr/m byte
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                //self.cycle_i(0x063);
                self.set_register16(Register16::AX, op2_value);                
            }
            0xA2 => {
                // MOV offset8, Al
                // These MOV variants are unique in that they take a direct offset with no modr/m byte
                let op2_value = self.a.l();
                self.write_operand8(self.i.operand1_type, self.i.segment_override, op2_value, ReadWriteFlag::RNI);
            }
            0xA3 => {
                // MOV offset16, AX
                // These MOV variants are unique in that they take a direct offset with no modr/m byte
                let op2_value = self.a.x();
                self.write_operand16(self.i.operand1_type, self.i.segment_override, op2_value, ReadWriteFlag::RNI);   
            }
            0xA4 | 0xA5 => {
                // MOVSB & MOVSW

                // rep_start() will terminate early if CX==0
                if self.rep_start() {

                    self.string_op(self.i.mnemonic, self.i.segment_override);
                    self.cycle_i(0x130);

                    // Check for end condition (CX==0)
                    if self.in_rep {
                        match self.rep_type {
                            RepType::Repc => {
                                // Repeat while Carry flag is set. If Carry flag is NOT set, end REP.
                                if !self.get_flag(Flag::Carry) {
                                    self.rep_end();
                                    self.cycle_i(MC_JUMP); // Jump to 1f4, RNI
                                }
                            }
                            RepType::Repnc => {
                                // Repeat while NOT Carry. If Carry flag is set, end REP.
                                if self.get_flag(Flag::Carry) {
                                    self.rep_end();
                                    self.cycle_i(MC_JUMP); // Jump to 1f4, RNI
                                }
                            }
                            _ => {}
                        }
                        
                        self.decrement_register16(Register16::CX); // 131

                        // Check for interrupt
                        if self.intr_pending {
                            self.cycles_i(2, &[0x131, MC_JUMP]); // Jump to RPTI
                            self.rep_interrupt();
                            
                        }
                        else {
                            self.cycles_i(2, &[0x131, 0x132]);

                            if self.c.x() == 0 {
                                // Fall through to 133, RNI
                                self.rep_end();
                            }
                            else {
                                self.cycle_i(MC_JUMP); // jump to 1
                            }
                        }
                    }
                    else {
                        // End non-rep prefixed MOVSB
                        self.cycle_i(MC_JUMP); // jump to 133, RNI
                    }                
                }
            }
            0xA6 | 0xA7 | 0xAE | 0xAF => {
                // CMPSB, CMPSW, SCASB, SCASW
                // Segment override: DS overridable
                // Flags: All

                if self.rep_start() {
                    
                    self.string_op(self.i.mnemonic, self.i.segment_override);
    
                    if self.in_rep {
                        let mut end = false;
                        // Check for REP end condition #1 (Z/NZ)
                        self.cycle_i(0x129);
                        self.decrement_register16(Register16::CX); // 129

                        match self.rep_type {
                            RepType::Repc => {
                                // Repeat while Carry flag is set. If Carry flag is NOT set, end REP.
                                if !self.get_flag(Flag::Carry) {
                                    self.rep_end();
                                    self.cycle_i(MC_JUMP); // Jump to 1f4, RNI
                                    end = true;
                                }
                            }
                            RepType::Repnc => {
                                // Repeat while NOT Carry. If Carry flag is set, end REP.
                                if self.get_flag(Flag::Carry) {
                                    self.rep_end();
                                    self.cycle_i(MC_JUMP); // Jump to 1f4, RNI
                                    end = true;
                                }
                            }
                            RepType::Repne => {
                                // Repeat while NOT zero. If Zero flag is set, end REP.
                                if self.get_flag(Flag::Zero) {
                                    self.rep_end();
                                    self.cycle_i(MC_JUMP); // Jump to 1f4, RNI
                                    end = true;
                                }
                            }
                            RepType::Repe => {
                                // Repeat while zero. If zero flag is NOT set, end REP.
                                if !self.get_flag(Flag::Zero) {
                                    self.rep_end();
                                    self.cycle_i(MC_JUMP); // Jump to 1f4, RNI
                                    end = true;
                                }
                            }
                            _=> {}
                        };

                        if !end {
                            
                            self.cycle_i(0x12a);
    
                            if self.intr_pending {
                                self.cycle_i(MC_JUMP); // Jump to RPTI
                                self.rep_interrupt();
                            }   
    
                            // Check for REP end condition #2 (CX==0)
                            self.cycle_i(0x12b);
                            if self.c.x() == 0 {
                                self.rep_end();
                                // Next instruction is 1f4: RNI, so don't spend cycle
                            }                 
                            else {
                                self.cycle_i(MC_JUMP); // Jump to line 1: 121
                            }
                        }
                    }
                    else {
                        // End non-rep prefixed CMPS
                        self.cycle_i(MC_JUMP); // Jump to 1f4, RNI
                    }
                }
            }
            0xA8 => {
                // TEST al, imm8
                // Flags: o..sz.pc
                let op1_value = self.a.l();
                let op2_value = self.read_operand8(self.i.operand2_type, None).unwrap();
                
                self.math_op8(Mnemonic::TEST,  op1_value, op2_value);
            }
            0xA9 => {
                // TEST ax, imm16
                // Flags: o..sz.pc
                let op1_value = self.a.x();
                let op2_value = self.read_operand16(self.i.operand2_type, None).unwrap();
                
                self.math_op16(Mnemonic::TEST,  op1_value, op2_value);
            }
            0xAA | 0xAB | 0xAC | 0xAD=> {
                // STOSB,STOSW,LODSB,LODSW

                if self.rep_start() {
                    
                    self.string_op(self.i.mnemonic, None);
                    self.cycle_i(0x11e);
    
                    // Check for end condition (CX==0)
                    if self.in_rep {
                        match self.rep_type {
                            RepType::Repc => {
                                // Repeat while Carry flag is set. If Carry flag is NOT set, end REP.
                                if !self.get_flag(Flag::Carry) {
                                    self.rep_end();
                                    self.cycle_i(MC_JUMP); // Jump to 1f4, RNI
                                }
                            }
                            RepType::Repnc => {
                                // Repeat while NOT Carry. If Carry flag is set, end REP.
                                if self.get_flag(Flag::Carry) {
                                    self.rep_end();
                                    self.cycle_i(MC_JUMP); // Jump to 1f4, RNI
                                }
                            }
                            _ => {}
                        }
                        
                        // Check for interrupt
                        self.cycle_i(0x11f);
                        if self.intr_pending {
                            self.cycle_i(MC_JUMP); // Jump to RPTI
                            self.rep_interrupt();
                        }
                        
                        self.cycle_i(0x1f0);
                        self.decrement_register16(Register16::CX); //1f0
                        if self.c.x() == 0 {
                            self.rep_end();
                        }
                        else {
                            // Jump to 1
                            self.cycle_i(MC_JUMP);
                        }
                    }
                    else {
                        // Jump to 1f1
                        self.cycle_i(MC_JUMP);
                    }
                }
            }
            0xB0..=0xB7 => {
                // MOV r8, imm8
                let op2_value = self.read_operand8(self.i.operand2_type, None).unwrap();
                if let OperandType::Register8(reg) = self.i.operand1_type { 
                    self.set_register8(reg, op2_value);
                }
                self.cycle_i(MC_JUMP);
            }
            0xB8..=0xBF => {
                // MOV r16, imm16
                let op2_value = self.read_operand16(self.i.operand2_type, None).unwrap();
                if let OperandType::Register16(reg) = self.i.operand1_type { 
                    self.set_register16(reg, op2_value);
                }
                //self.cycle_i(0x01e);
            }
            0xC0 => {
                // ROL, ROR, RCL, RCR, SHL, SHR, SAR:  r/m8, imm8
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();

                self.cycles_i(6, &[0x08c, 0x08d, 0x08e, MC_JUMP, 0x090, 0x091]);

                if op2_value > 0 {
                    for _ in 0..op2_value {
                        self.cycles_i(4, &[MC_JUMP, 0x08f, 0x090, 0x091]);
                    }

                    // If there is a terminal write to M, don't process RNI on line 0x92
                    if let OperandType::AddressingMode(_) = self.i.operand1_type {
                        //if self.c.l() != 0 {
                        self.cycle_i(0x092);
                        //}
                    }

                    let result = self.bitshift_op8(self.i.mnemonic, op1_value, op2_value);
                    self.write_operand8(self.i.operand1_type, self.i.segment_override, result, ReadWriteFlag::RNI);
                }
                
            }
            0xC1 => {
                // ROL, ROR, RCL, RCR, SHL, SHR, SAR:  r/m16, imm8
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();

                self.cycles_i(6, &[0x08c, 0x08d, 0x08e, MC_JUMP, 0x090, 0x091]);

                if op2_value > 0 {
                    for _ in 0..op2_value {
                        self.cycles_i(4, &[MC_JUMP, 0x08f, 0x090, 0x091]);
                    }

                    // If there is a terminal write to M, don't process RNI on line 0x92
                    if let OperandType::AddressingMode(_) = self.i.operand1_type {
                        self.cycle_i(0x092);
                    }

                    let result = self.bitshift_op16(self.i.mnemonic, op1_value, op2_value);
                    self.write_operand16(self.i.operand1_type, self.i.segment_override, result, ReadWriteFlag::RNI);
                }
                
            }
            0xC2 => {
                // RETN imm16 - Return from call w/ release
                // 0xC0 undocumented alias for 0xC2
                // Flags: None

                let stack_disp = self.read_operand16(self.i.operand1_type, None).unwrap();
                self.cycle_i(MC_JUMP); // JMP to FARRET
                let new_pc = self.pop_u16();
                self.pc = new_pc;
                
                self.biu_fetch_suspend();
                self.cycles_i(2, &[0x0c3, 0x0c4]);
                self.biu_queue_flush();
                self.cycles_i(3, &[0x0c5, MC_JUMP, 0x0ce]);    
                
                self.release(stack_disp);

                // Pop call stack
                //self.call_stack.pop_back();

                jump = true
            }
            0xC3 => {
                // RETN - Return from call
                // 0xC1 undocumented alias for 0xC3
                // Flags: None
                // Effectively, this instruction is pop ip
                let new_pc = self.pop_u16();
                self.pc = new_pc;
                self.biu_fetch_suspend();
                self.cycle_i(0x0bd);
                self.biu_queue_flush();
                self.cycles_i(2, &[0x0be, 0x0bf]);                
                
                // Pop call stack
                // self.call_stack.pop_back();

                jump = true
            }
            0xC4 => {
                // LES - Load ES from Pointer
                self.cycles_i(2, &[0x0F0, 0x0F1]);

                // Operand 2 is far pointer
                let (les_segment, les_offset) = 
                    self.read_operand_farptr(
                        self.i.operand2_type, 
                        self.i.segment_override,
                        ReadWriteFlag::Normal
                    ).unwrap();

                //log::trace!("LES instruction: Loaded {:04X}:{:04X}", les_segment, les_offset);
                self.write_operand16(
                    self.i.operand1_type, 
                    self.i.segment_override, 
                    les_offset, 
                    ReadWriteFlag::Normal);
                self.es = les_segment;
            }
            0xC5 => {
                // LDS - Load DS from Pointer
                self.cycles_i(2, &[0x0F4, 0x0F5]);

                // Operand 2 is far pointer
                let (lds_segment, lds_offset) = 
                    self.read_operand_farptr(
                        self.i.operand2_type, 
                        self.i.segment_override,
                        ReadWriteFlag::RNI
                    ).unwrap();

                //log::trace!("LDS instruction: Loaded {:04X}:{:04X}", lds_segment, lds_offset);
                self.write_operand16(
                    self.i.operand1_type, 
                    self.i.segment_override, 
                    lds_offset, 
                    ReadWriteFlag::Normal);
                self.ds = lds_segment;
                //self.cycle_i(0x0f7);
            }
            0xC6 => {
                // MOV r/m8, imm8
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();
                self.cycles(2);
                self.write_operand8(self.i.operand1_type, self.i.segment_override, op2_value, ReadWriteFlag::RNI);
            }
            0xC7 => {
                // MOV r/m16, imm16
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                self.cycle_i(0x01e);
                self.write_operand16(self.i.operand1_type, self.i.segment_override, op2_value, ReadWriteFlag::RNI);
            }
            0xC8 => {
                // ENTER imm16, imm8
                let alloc_size = self.read_operand16(self.i.operand1_type, None).unwrap();
                let nesting_level = self.read_operand8(self.i.operand2_type, None).unwrap();

                // First, the old BP value is saved to the stack so that the BP of the calling procedure
                // can be restored
                self.push_register16(Register16::BP, ReadWriteFlag::Normal);
                let frame_temp = self.get_register16(Register16::SP);
                
                if nesting_level > 1 {
                    for i in 0..nesting_level {
                        self.set_register16(Register16::BP, self.get_register16(Register16::BP).wrapping_sub(2));
                        let ptr = self.biu_read_u16(Segment::SS, self.get_register16(Register16::BP), ReadWriteFlag::Normal);
                        self.push_u16(ptr, ReadWriteFlag::Normal);
                    }
                }
                else if nesting_level == 1 {
                    self.push_u16(frame_temp, ReadWriteFlag::Normal);
                };
                self.set_register16(Register16::BP, frame_temp);
                self.set_register16(Register16::SP, self.get_register16(Register16::SP).wrapping_sub(alloc_size));
            }
            0xC9 => {
                // LEAVE
                self.set_register16(Register16::SP, self.get_register16(Register16::BP));
                let new_bp = self.pop_u16();
                self.set_register16(Register16::BP, new_bp);
            }
            0xCA => {
                // RETF imm16 - Far Return w/ release 
                // 0xC8 undocumented alias for 0xCA
                let stack_disp = self.read_operand16(self.i.operand1_type, None).unwrap();
                self.farret(true);
                self.release(stack_disp);
                self.cycle_i(0x0ce);
                jump = true;
            }
            0xCB => {
                // RETF - Far Return
                // 0xC9 undocumented alias for 0xCB
                self.cycle_i(0x0c0);
                self.farret(true);
                jump = true;
            }
            0xCC => {
                // INT 3 - Software Interrupt 3
                // This is a special form of INT which assumes IRQ 3 always. Most assemblers will not generate this form

                // Save next address if we step over this INT.
                self.step_over_target = Some(CpuAddress::Segmented(self.cs, self.ip()));

                self.int3();
                jump = true;    
            }
            0xCD => {
                // INT imm8 - Software Interrupt
                // The Interrupt flag does not affect the handling of non-maskable interrupts (NMIs) or software interrupts
                // generated by the INT instruction.
                
                // Get interrupt number (immediate operand)
                let irq = self.read_operand8(self.i.operand1_type, None).unwrap();

                // Save next address if we step over this INT.
                self.step_over_target = Some(CpuAddress::Segmented(self.cs, self.ip()));
                
                self.cycle_i(MC_JUMP); // Jump to INTR
                self.sw_interrupt(irq);
                jump = true;
            }
            0xCE => {
                // INTO - Call Overflow Interrupt Handler
                if self.get_flag(Flag::Overflow) {
                    self.cycles_i(4, &[0x1ac, 0x1ad, MC_JUMP, 0x1af]);
                    
                    // Save next address if we step over this INT.
                    self.step_over_target = Some(CpuAddress::Segmented(self.cs, self.ip()));
                    self.sw_interrupt(4);
                    jump = true;
                }
                else {
                    // Overflow not set. 
                    self.cycles_i(2, &[0x1ac, 0x1ad]);
                }
            }
            0xCF => {
                // IRET instruction
                self.iret_routine();
                jump = true;
            }
            0xD0 => {
                // ROL, ROR, RCL, RCR, SHL, SHR, SAR:  r/m8, 0x01
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let result = self.bitshift_op8(self.i.mnemonic, op1_value, 1);
                if let OperandType::AddressingMode(_) = self.i.operand1_type {
                    self.cycle_i(0x088);
                }
                self.write_operand8(self.i.operand1_type, self.i.segment_override, result, ReadWriteFlag::RNI);
            }
            0xD1 => {
                // ROL, ROR, RCL, RCR, SHL, SHR, SAR:  r/m16, 0x01

                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let result = self.bitshift_op16(self.i.mnemonic, op1_value, 1);
                if let OperandType::AddressingMode(_) = self.i.operand1_type {
                    self.cycle_i(0x088); 
                }                
                self.write_operand16(self.i.operand1_type, self.i.segment_override, result, ReadWriteFlag::RNI);
            }
            0xD2 => {
                // ROL, ROR, RCL, RCR, SHL, SHR, SAR:  r/m8, cl
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();

                self.cycles_i(6, &[0x08c, 0x08d, 0x08e, MC_JUMP, 0x090, 0x091]);
                //self.cycles_i(5, &[0x08d, 0x08e, MC_JUMP, 0x090, 0x091]);

                if self.c.l() > 0 {
                    for _ in 0..(self.c.l() ) {
                        self.cycles_i(4, &[MC_JUMP, 0x08f, 0x090, 0x091]);
                    }

                    // If there is a terminal write to M, don't process RNI on line 0x92
                    if let OperandType::AddressingMode(_) = self.i.operand1_type {
                        //if self.c.l() != 0 {
                        self.cycle_i(0x092);
                        //}
                    }

                    let result = self.bitshift_op8(self.i.mnemonic, op1_value, op2_value);
                    self.write_operand8(self.i.operand1_type, self.i.segment_override, result, ReadWriteFlag::RNI);
                }
            }
            0xD3 => {
                // ROL, ROR, RCL, RCR, SHL, SHR, SAR:  r/m16, cl
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();

                self.cycles_i(6, &[0x08c, 0x08d, 0x08e, MC_JUMP, 0x090, 0x091]);
                //self.cycles_i(5, &[0x08d, 0x08e, MC_JUMP, 0x090, 0x091]);

                if self.c.l() > 0 {
                    for _ in 0..(self.c.l() ) {
                        self.cycles_i(4, &[MC_JUMP, 0x08f, 0x090, 0x091]);
                    }

                    // If there is a terminal write to M, don't process RNI on line 0x92
                    if let OperandType::AddressingMode(_) = self.i.operand1_type {
                        self.cycle_i(0x092);
                    }

                    let result = self.bitshift_op16(self.i.mnemonic, op1_value, op2_value);
                    self.write_operand16(self.i.operand1_type, self.i.segment_override, result, ReadWriteFlag::RNI);
                }
            }
            0xD4 => {
                // AAM - Ascii adjust AX after Multiply
                // Get imm8 value
                let op1_value = self.read_operand8(self.i.operand1_type, None).unwrap();
                
                if op1_value != 0 {
                    if !self.aam(op1_value) {
                        //self.set_szp_flags_from_result_u8(0);
                        self.clear_flag(Flag::AuxCarry);
                        self.clear_flag(Flag::Carry);
                        self.clear_flag(Flag::Overflow);
                    }
                }
                else {
                    self.set_szp_flags_from_result_u8(self.a.l());
                    self.set_register8(Register8::AH, 0xFF);
                    self.clear_flag(Flag::AuxCarry);
                    self.clear_flag(Flag::Carry);
                    self.clear_flag(Flag::Overflow);
                }

            }
            0xD5 => {
                // AAD - Ascii Adjust before Division
                
                // We read the immediate value, but it is not used in the AAD operation on V20
                let op1_value = self.read_operand8(self.i.operand1_type, None).unwrap();
                self.aad(0x0A);
            }
            0xD6 | 0xD7 => {
                // XLAT
                
                // Handle segment override, default DS
                let segment = self.i.segment_override.unwrap_or(Segment::DS);
                let disp16: u16 = self.b.x().wrapping_add(self.a.l() as u16);
                
                self.cycles_i(3, &[0x10c, 0x10d, 0x10e]);

                let value = self.biu_read_u8(segment, disp16);
                
                self.set_register8(Register8::AL, value);
            }
            0x66 | 0x67 | 0xD8..=0xDF => {
                // ESC - FPU instructions. 
                
                // Perform dummy read if memory operand
                let _op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override);
            }
            0xE0 | 0xE1 => {
                // LOOPNE & LOOPE
                // LOOPNE - Decrement CX, Jump short if count!=0 and ZF=0
                // LOOPE - Jump short if count!=0 and ZF=1
                // loop does not modify flags

                self.decrement_register16(Register16::CX);
                self.cycles_i(2, &[0x138, 0x139]);

                let mut zero_condition = !self.get_flag(Flag::Zero);
                if self.i.opcode == 0xE1 { 
                    zero_condition = !zero_condition;
                }

                let rel8 = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();

                if self.c.x() != 0 && zero_condition {
                    self.reljmp2(rel8 as i8 as i16, true);
                    jump = true;
                }
                else {
                    self.cycle_i(0x13c);
                }            
            }
            0xE2 => {
                // LOOP - Jump short if count != 0 
                // loop does not modify flags
                
                self.decrement_register16(Register16::CX);
                self.cycles_i(2, &[0x140, 0x141]);

                let rel8 = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();

                if self.c.x() != 0 {
                    self.reljmp2(rel8 as i8 as i16, true);
                    jump = true;
                }
                if !jump {
                    self.cycle();
                }
            }
            0xE3 => {
                // JCXZ - Jump if CX == 0
                // Flags: None
            
                self.cycles_i(2, &[0x138, 0x139]);
                let rel8 = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();

                self.cycle_i(0x13b);

                if self.c.x() == 0 {
                    self.reljmp2(rel8 as i8 as i16, true);
                    jump = true;
                }
                else {
                    self.cycle_i(0x13c);
                }
            }
            0xE4 => {
                // IN al, imm8
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap(); 
                self.cycles_i(2, &[0x0ad, 0x0ae]);

                let in_byte = self.biu_io_read_u8(op2_value as u16);
                self.set_register8(Register8::AL, in_byte);
                //println!("IN: Would input value from port {:#02X}", op2_value);  
            }
            0xE5 => {
                // IN ax, imm8
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap(); 
                self.cycles_i(2, &[0x0ad, 0x0ae]);

                let in_word = self.biu_io_read_u16(op2_value as u16);
                self.set_register16(Register16::AX, in_word);
            }
            0xE6 => {
                // OUT imm8, al
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();                
                self.cycles_i(2, &[0x0b1, 0x0b2]);

                // Write to port
                self.biu_io_write_u8(op1_value as u16, op2_value, ReadWriteFlag::RNI);
            }
            0xE7 => {
                // OUT imm8, ax
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();                
                self.cycles_i(2, &[0x0b1, 0x0b2]);

                // Write to consecutive ports
                self.biu_io_write_u16(op1_value as u16, op2_value, ReadWriteFlag::RNI);

                /*
                // Write first 8 bits to first port
                self.biu_io_write_u8(op1_value as u16, (op2_value & 0xFF) as u8, ReadWriteFlag::Normal);
                // Write next 8 bits to port + 1
                self.biu_io_write_u8((op1_value + 1) as u16, (op2_value >> 8 & 0xFF) as u8, ReadWriteFlag::RNI);
                */
            }
            0xE8 => {
                // CALL rel16
                // Unique microcode routine. Does not call NEARCALL.

                // Fetch rel16 operand
                let rel16 = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();

                self.biu_fetch_suspend(); // 0x07E
                self.cycles_i(2, &[0x07e, 0x07f]);
                self.corr();
                self.cycle_i(0x080);
                
                // Save next address if we step over this CALL.
                self.step_over_target = Some(CpuAddress::Segmented(self.cs, self.pc));

                let ret_addr = self.pc;
                // Add rel16 to pc
                let new_pc = util::relative_offset_u16(self.pc, rel16 as i16);

                // Add to call stack
                self.push_call_stack(
                    CallStackEntry::Call {
                        ret_cs: self.cs,
                        ret_ip: self.pc,
                        call_ip: new_pc
                    },
                    self.cs,
                    self.pc
                );

                // Set new IP
                self.pc = new_pc;
                self.biu_queue_flush();
                self.cycles_i(3, &[0x081, 0x082, MC_JUMP]); 

                // Push return address
                self.push_u16(ret_addr, ReadWriteFlag::RNI);
                jump = true;
            }
            0xE9 => {
                // JMP rel16
                let rel16 = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();

                // We fall through to reljmp, so no jump
                self.reljmp2(rel16 as i16, false);
                jump = true;
            }
            0xEA => {
                // JMP FAR [addr16:16]
                // This instruction reads a direct FAR address from the instruction queue. (See 0x9A for its twin CALLF)
                let (segment, offset) = self.read_operand_faraddr();
                self.biu_fetch_suspend();
                self.cycles_i(2, &[0x0e4, 0x0e5]);
                self.cs = segment;
                self.pc = offset;
                self.biu_queue_flush();
                self.cycle_i(0x0e6); // Doesn't hurt to run this RNI as we have to re-fill queue
                jump = true;
            }
            0xEB => {
                // JMP rel8
                let rel8 = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                self.reljmp2(rel8 as i8 as i16, true); // We jump directly into reljmp
                jump = true
            }
            0xEC => {
                // IN al, dx
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap(); 
                let in_byte = self.biu_io_read_u8(op2_value);
                self.set_register8(Register8::AL, in_byte);
            }
            0xED => {
                // IN ax, dx
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap(); 
                let in_word = self.biu_io_read_u16(op2_value);
                self.set_register16(Register16::AX, in_word);
            }
            0xEE => {
                // OUT dx, al
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();                
                self.cycle_i(0x0b8);

                self.biu_io_write_u8(op1_value, op2_value, ReadWriteFlag::RNI);
            }
            0xEF => {
                // OUT dx, ax
                // On the 8088, this does two writes to successive port #'s 
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                self.cycle_i(0x0b8);

                if op1_value == 0x06 {
                    log::debug!("OUT of {:02X}", op2_value);
                }

                // Write to consecutive ports
                self.biu_io_write_u16(op1_value, op2_value, ReadWriteFlag::RNI);

                /*
                // Write first 8 bits to first port
                self.biu_io_write_u8(op1_value, (op2_value & 0xFF) as u8, ReadWriteFlag::Normal);
                // Write next 8 bits to port + 1
                self.biu_io_write_u8(op1_value + 1, (op2_value >> 8 & 0xFF) as u8, ReadWriteFlag::RNI);
                */
            }
            0xF0 => {
                unhandled = true;
            }
            0xF1 => {
                // Does nothing?
                self.cycle();
            }
            0xF2 => {
                unhandled = true;
            }
            0xF3 => {
                unhandled = true;
            }
            0xF4 => {
                // HLT - Halt
                // HLT is non-microcoded, so cycles spent here aren't logged by mc.

                self.biu_bus_wait_halt();       // wait until at least t2 of m-cycle
                self.halt_not_hold = true;      // set internal halt signal
                self.biu_fetch_halt();          // halt prefetcher
                self.biu_bus_wait_finish();     // wait until end of m-cycle

                if self.intr {
                    // If an intr is pending now, execute it without actually halting.
                    log::trace!("Halt overriden at [{:05X}]", NecVx0::calc_linear_address(self.cs, self.ip()));
                    self.cycles(2); // Cycle to load interrupt routine
                    self.halt_not_hold = false;
                }
                else {
                    // Actually halt
                    log::trace!("Halt at [{:05X}]", NecVx0::calc_linear_address(self.cs, self.ip()));
                    self.halted = true;
                    self.biu_halt();
                    // HLT is reentrant as step will remain in halt state until interrupt, even
                    // when stepped.
                    self.instruction_reentrant = true;
                }

            }
            0xF5 => {
                // CMC - Complement (invert) Carry Flag
                let carry_flag = self.get_flag(Flag::Carry);
                self.set_flag_state(Flag::Carry, !carry_flag);
            }
            0xF6 => {
                // Miscellaneous Opcode Extensions, r/m8, imm8

                // REP negates product/quotient of MUL/DIV
                let negate = (self.i.prefixes & (OPCODE_PREFIX_REP1 | OPCODE_PREFIX_REP2)) != 0;

                match self.i.mnemonic {

                    Mnemonic::TEST => {
                        let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                        let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();

                        // 8 bit TEST takes a jump
                        self.cycles_i(2, &[MC_JUMP, 0x09a]);

                        // Don't use result, just set flags
                        let _result = self.math_op8(self.i.mnemonic, op1_value, op2_value);
                    }
                    Mnemonic::NOT => {
                        let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                        let result = self.math_op8(self.i.mnemonic, op1_value, 0);

                        if let OperandType::AddressingMode(_) = self.i.operand1_type {
                            self.cycles_i(2,&[0x04c, 0x04d]);
                        }                        
                        self.write_operand8(self.i.operand1_type, self.i.segment_override, result, ReadWriteFlag::RNI);
                    }
                    Mnemonic::NEG => {
                        let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                        let result = self.math_op8(self.i.mnemonic, op1_value, 0);

                        if let OperandType::AddressingMode(_) = self.i.operand1_type {
                            self.cycles_i(2,&[0x050, 0x051]);
                        }                          
                        self.write_operand8(self.i.operand1_type, self.i.segment_override, result, ReadWriteFlag::RNI);
                    }
                    Mnemonic::MUL => {
                        let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                        
                        //self.multiply_u8(op1_value);
                        let product = self.mul8(self.a.l(), op1_value, false, negate);
                        self.set_register16(Register16::AX, product);

                        if let OperandType::Register8(_) = self.i.operand1_type {
                            self.cycle();
                        }

                        self.set_szp_flags_from_result_u8(self.a.h());
                    }
                    Mnemonic::IMUL => {
                        let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                        
                        //self.multiply_i8(op1_value as i8);
                        let product = self.mul8(self.a.l(), op1_value, true, negate);
                        self.set_register16(Register16::AX, product);

                        if let OperandType::Register8(_) = self.i.operand1_type {
                            self.cycle();
                        }

                        self.set_szp_flags_from_result_u8(self.a.h());
                    }                    
                    Mnemonic::DIV => {
                        let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                        if let OperandType::Register8(_) = self.i.operand1_type {
                            self.cycle();
                        }
                        
                        match self.div8(self.a.x(), op1_value, false, negate) {
                            Ok((al, ah)) => {
                                self.set_register8(Register8::AL, al); // Quotient in AL
                                self.set_register8(Register8::AH, ah); // Remainder in AH
                            }
                            Err(_) => {

                                self.set_szp_flags_from_result_u8(self.a.h());
                                //self.set_flag(Flag::Zero);
                                //self.clear_flag(Flag::Sign);
                                self.clear_flag(Flag::AuxCarry);
                                self.clear_flag(Flag::Carry);
                                self.clear_flag(Flag::Overflow);
                                self.int0();
                                exception = CpuException::DivideError;
                            }
                        }
                    }          
                    Mnemonic::IDIV => {
                        let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                        if let OperandType::Register8(_) = self.i.operand1_type {
                            self.cycle();
                        }

                        match self.div8(self.a.x(), op1_value, true, negate) {
                            Ok((al, ah)) => {
                                self.set_register8(Register8::AL, al); // Quotient in AL
                                self.set_register8(Register8::AH, ah); // Remainder in AH
                            }
                            Err(_) => {

                                self.set_szp_flags_from_result_u8(self.a.h());
                                //self.set_flag(Flag::Zero);
                                //self.clear_flag(Flag::Sign);                                
                                self.clear_flag(Flag::AuxCarry);
                                self.clear_flag(Flag::Carry);
                                self.clear_flag(Flag::Overflow);

                                // Don't include REP prefix as part of instruction size
                                //let size_adj = if self.i.prefixes & (OPCODE_PREFIX_REP1 | OPCODE_PREFIX_REP2) != 0 { 1 } else { 0 };
                                self.int0();
                                exception = CpuException::DivideError;
                            }
                        }
                    }                                 
                    _=> unhandled = true
                }
            }
            0xF7 => {
                // Miscellaneous Opcode Extensions, r/m16, imm16

                // REP negates product/quotient of MUL/DIV
                let negate = (self.i.prefixes & (OPCODE_PREFIX_REP1 | OPCODE_PREFIX_REP2)) != 0;

                match self.i.mnemonic {

                    Mnemonic::TEST => {
                        let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                        let op2_value = self.read_operand16(self.i.operand2_type, self.i.segment_override).unwrap();
                        
                        self.cycle_i(0x09a);
                        // Don't use result, just set flags
                        let _result = self.math_op16(self.i.mnemonic, op1_value, op2_value);
                    }
                    Mnemonic::NOT => {
                        let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                        let result = self.math_op16(self.i.mnemonic, op1_value, 0);
                        if let OperandType::AddressingMode(_) = self.i.operand1_type {
                            self.cycles_i(2,&[0x04c, 0x04d]);
                        }                            
                        self.write_operand16(self.i.operand1_type, self.i.segment_override, result, ReadWriteFlag::RNI);
                    }
                    Mnemonic::NEG => {
                        let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                        let result = self.math_op16(self.i.mnemonic, op1_value, 0);

                        if let OperandType::AddressingMode(_) = self.i.operand1_type {
                            self.cycles_i(2,&[0x050, 0x051]);
                        }        
                        self.write_operand16(self.i.operand1_type, self.i.segment_override, result, ReadWriteFlag::RNI);
                    }
                    Mnemonic::MUL => {
                        let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                        // Multiply handles writing to ax
                        //self.multiply_u16(op1_value);

                        let (dx, ax) = self.mul16(self.a.x(), op1_value, false, negate);

                        if let OperandType::Register16(_) = self.i.operand1_type {
                            self.cycle();
                        }

                        //self.cycle();
                        self.set_register16(Register16::DX, dx);
                        self.set_register16(Register16::AX, ax);

                        self.set_szp_flags_from_result_u16(self.d.x());
                    }
                    Mnemonic::IMUL => {
                        let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                        // Multiply handles writing to dx:ax
                        //self.multiply_i16(op1_value as i16);
                         
                        let (dx, ax) = self.mul16(self.a.x(), op1_value, true, negate);

                        if let OperandType::Register16(_) = self.i.operand1_type {
                            self.cycle();
                        }

                        self.set_register16(Register16::DX, dx);
                        self.set_register16(Register16::AX, ax);    

                        self.set_szp_flags_from_result_u16(self.d.x());
                    }
                    Mnemonic::DIV => {
                        let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                        if let OperandType::Register16(_) = self.i.operand1_type {
                            self.cycle();
                        }

                        match self.div16(((self.d.x() as u32) << 16 ) | (self.a.x() as u32), op1_value, false, negate) {
                            Ok((quotient, remainder)) => {
                                self.set_register16(Register16::AX, quotient); // Quotient in AX
                                self.set_register16(Register16::DX, remainder); // Remainder in DX
                            }
                            Err(_) => {

                                self.set_szp_flags_from_result_u8(self.a.h());
                                //self.set_flag(Flag::Zero);
                                //self.clear_flag(Flag::Sign);
                                self.clear_flag(Flag::AuxCarry);
                                self.clear_flag(Flag::Carry);
                                self.clear_flag(Flag::Overflow);
                                self.int0();

                                exception = CpuException::DivideError;
                            }
                        }
                    }
                    Mnemonic::IDIV => {
                        let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                        if let OperandType::Register16(_) = self.i.operand1_type {
                            self.cycle();
                        }

                        match self.div16(((self.d.x() as u32) << 16 ) | (self.a.x() as u32), op1_value, true, negate) {
                            Ok((quotient, remainder)) => {
                                self.set_register16(Register16::AX, quotient); // Quotient in AX
                                self.set_register16(Register16::DX, remainder); // Remainder in DX
                            }
                            Err(_) => {

                                self.set_szp_flags_from_result_u8(self.a.h());
                                //self.set_flag(Flag::Zero);
                                //self.clear_flag(Flag::Sign);
                                self.clear_flag(Flag::AuxCarry);
                                self.clear_flag(Flag::Carry);
                                self.clear_flag(Flag::Overflow);
                                self.int0();
                                exception = CpuException::DivideError;
                            }
                        }                        
                    }
                    _=> unhandled = true
                }
            }
            0xF8 => {
                // CLC - Clear Carry Flag
                self.clear_flag(Flag::Carry);
                //self.cycle()
            }
            0xF9 => {
                // STC - Set Carry Flag
                self.set_flag(Flag::Carry);
                //self.cycle()
            }
            0xFA => {
                // CLI - Clear Interrupt Flag
                self.clear_flag(Flag::Interrupt);
                //self.cycle()
            }
            0xFB => {
                // STI - Set Interrupt Flag
                self.set_flag(Flag::Interrupt);
                //self.cycle()
            }
            0xFC => {
                // CLD - Clear Direction Flag
                self.clear_flag(Flag::Direction);
                //self.cycle()
            }
            0xFD => {
                // STD = Set Direction Flag
                self.set_flag(Flag::Direction);
                //self.cycle()
            }
            0xFE => {
                // INC/DEC r/m8
                // Technically only the INC and DEC forms of this group are valid. However, the other operands do 8 bit 
                // sorta-broken versions of CALL, JMP and PUSH. The behavior implemented here was derived from 
                // experimentation with a real 8088 CPU.
                match self.i.mnemonic {
                    // INC/DEC r/m16
                    Mnemonic::INC | Mnemonic::DEC => {
                        let op_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                        let result = self.math_op8(self.i.mnemonic, op_value, 0);

                        if let OperandType::AddressingMode(_) = self.i.operand1_type {
                            self.cycles_i(2,&[0x020, 0x021]);
                        }                           
                        self.write_operand8(self.i.operand1_type, self.i.segment_override, result, ReadWriteFlag::RNI);
                    },
                    // Call Near
                    Mnemonic::CALL => {

                        if let OperandType::AddressingMode(_) = self.i.operand1_type {
                            // Reads only 8 bit operand from modrm.
                            let ptr8 = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                            
                            // Push only 8 bits of next IP onto stack
                            let next_i = self.ip();

                            // We do not allow stepping over 0xFE call here as it is unlikely to lead to a valid location or return.

                            self.push_u8((next_i & 0xFF) as u8, ReadWriteFlag::Normal);

                            // temporary timings
                            self.biu_fetch_suspend();
                            self.cycles(4);
                            self.biu_queue_flush();

                            // Set only lower 8 bits of IP, upper bits FF
                            self.pc = 0xFF00 | ptr8 as u16;
                        }
                        else if let OperandType::Register8(reg) = self.i.operand1_type {
                            
                            // Push only 8 bits of next IP onto stack
                            let next_i = self.ip() + (self.i.size as u16);
                            self.push_u8((next_i & 0xFF) as u8, ReadWriteFlag::Normal);

                            // temporary timings
                            self.biu_fetch_suspend();
                            self.cycles(4);
                            self.biu_queue_flush();
                            
                            // If this form uses a register operand, the full 16 bits are copied to IP.
                            self.pc = self.get_register16(NecVx0::reg8to16(reg));
                        }
                        jump = true;
                    }
                    // Call Far
                    Mnemonic::CALLF => {
                        if let OperandType::AddressingMode(mode) = self.i.operand1_type {
                            let (ea_segment, ea_offset) = self.calc_effective_address(mode, None);

                            // Read one byte of offset and one byte of segment
                            let offset = self.biu_read_u8(ea_segment, ea_offset);

                            self.cycles_i(3, &[0x1e2, MC_RTN, 0x068]); // RTN delay
                            
                            let segment = self.biu_read_u8(ea_segment, ea_offset.wrapping_add(2));

                            self.cycle_i(0x06a);
                            self.biu_fetch_suspend();
                            self.cycles_i(3, &[0x06b, 0x06c, MC_NONE]);
    
                            // Push low byte of CS
                            self.push_u8((self.cs & 0x00FF) as u8, ReadWriteFlag::Normal);
                            
                            let next_i = self.ip();
                            // We do not handle stepping over 0xFE call here as it is unlikely to lead to a valid location or return.
                            self.cs = 0xFF00 | segment as u16;
                            self.pc = 0xFF00 | offset as u16;
                            
                            self.cycles_i(3, &[0x06e, 0x06f, MC_JUMP]); // UNC NEARCALL
                            self.biu_queue_flush();
                            self.cycles_i(3, &[0x077, 0x078, 0x079]);

                            // Push low byte of next IP
                            self.push_u8((next_i & 0x00FF) as u8, ReadWriteFlag::RNI);
                            jump = true;
                        }
                        else if let OperandType::Register8(reg) = self.i.operand1_type {

                            // Read one byte from DS:0004 (weird?) and don't do anything with it.
                            let _ = self.biu_read_u8(Segment::DS, 0x0004);

                            // Push low byte of CS
                            self.push_u8((self.cs & 0x00FF) as u8, ReadWriteFlag::Normal);
                            // Push low byte of next IP
                            self.push_u8((self.ip() & 0x00FF) as u8, ReadWriteFlag::Normal);

                            // temporary timings
                            self.biu_fetch_suspend();
                            self.cycles(4);
                            self.biu_queue_flush();
                            
                            // If this form uses a register operand, the full 16 bits are copied to PC.
                            self.pc = self.get_register16(NecVx0::reg8to16(reg));
                        }
                    }
                    // Jump to memory r/m16
                    Mnemonic::JMP => {
                        // Reads only 8 bit operand from modrm.
                        let ptr8 = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();

                        // Set only lower 8 bits of PC, upper bits FF
                        self.pc = 0xFF00 | ptr8 as u16;

                        self.biu_fetch_suspend();
                        self.cycles(4);
                        self.biu_queue_flush();
                        jump = true;
                    }
                    // Jump Far
                    Mnemonic::JMPF => {
                        if let OperandType::AddressingMode(mode) = self.i.operand1_type {
                            let (ea_segment, ea_offset) = self.calc_effective_address(mode, None);

                            // Read one byte of offset and one byte of segment
                            let offset = self.biu_read_u8(ea_segment, ea_offset);
                            let segment = self.biu_read_u8(ea_segment, ea_offset.wrapping_add(2));

                            self.biu_fetch_suspend();
                            self.cycles(4);
                            self.biu_queue_flush();

                            self.cs = 0xFF00 | segment as u16;
                            self.pc = 0xFF00 | offset as u16;
                            jump = true;                     
                        }
                        else if let OperandType::Register8(reg) = self.i.operand1_type {

                            // Read one byte from DS:0004 (weird?) and don't do anything with it.
                            let _ = self.biu_read_u8(Segment::DS, 0x0004);

                            // temporary timings
                            self.biu_fetch_suspend();
                            self.cycles(4);
                            self.biu_queue_flush();
                            
                            // If this form uses a register operand, the full 16 bits are copied to PC.
                            self.pc = self.get_register16(NecVx0::reg8to16(reg));
                        }
                    }
                    // Push Byte onto stack
                    Mnemonic::PUSH => {
                        // Read one byte from rm
                        let op_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                        self.cycles_i(3, &[0x024, 0x025, 0x026]);

                        // Write one byte to stack
                        self.push_u8(op_value, ReadWriteFlag::RNI);
                    }                                                           
                    _ => {
                        unhandled = true;
                    }
                }
            }
            0xFF => {
                // Several opcode extensions here
                match self.i.mnemonic {
                    Mnemonic::INC | Mnemonic::DEC => {
                        // INC/DEC r/m16
                        let op_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                        let result = self.math_op16(self.i.mnemonic, op_value, 0);

                        if let OperandType::AddressingMode(_) = self.i.operand1_type {
                            self.cycles_i(2,&[0x020, 0x021]);
                        }                         
                        self.write_operand16(self.i.operand1_type, self.i.segment_override, result, ReadWriteFlag::RNI);
                    },
                    Mnemonic::CALL => {

                        if let OperandType::AddressingMode(_) = self.i.operand1_type {

                            let ptr16 = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();

                            self.biu_fetch_suspend();
                            self.cycles_i(4, &[0x074, 0x075, MC_CORR, 0x076]);

                            // Save next address if we step over this CALL.
                            self.step_over_target = Some(CpuAddress::Segmented(self.cs, self.ip()));

                            let return_ip = self.ip();
                            
                            // Add to call stack
                            self.push_call_stack(
                                CallStackEntry::Call {
                                    ret_cs: self.cs,
                                    ret_ip: return_ip,
                                    call_ip: ptr16
                                },
                                self.cs,
                                return_ip
                            );

                            
                            self.pc = ptr16;
                            self.biu_queue_flush();
                            self.cycles_i(3, &[0x077, 0x078, 0x079]);

                            // Push return address (next instruction offset) onto stack
                            self.push_u16(return_ip, ReadWriteFlag::RNI);
                            
                        }
                        else if let OperandType::Register16(reg) = self.i.operand1_type {
                            // Register form is invalid (can't use arbitrary modrm register as a pointer)
                            // We model the odd behavior of this invalid form here.
                            self.biu_fetch_suspend();
                            self.cycles_i(2, &[0x074, 0x075]);
                            self.corr();self.cycle_i(0x076);
                            
                            let next_i = self.pc; // PC already corrected above
                            self.pc = self.get_register16(reg); // Value of IP becomes value of register operand
                            self.biu_queue_flush();
                            self.cycles_i(3, &[0x077, 0x078, 0x079]);

                            // Push return address (next instruction offset) onto stack
                            self.push_u16(next_i, ReadWriteFlag::RNI);                            
                        }

                        jump = true;
                    }
                    Mnemonic::CALLF => {
                        // CALL FAR r/mFarPtr
                        if let OperandType::AddressingMode(_mode) = self.i.operand1_type {
                            self.cycle_i(0x068);
                            let (segment, offset) = self.read_operand_farptr(self.i.operand1_type, self.i.segment_override, ReadWriteFlag::Normal).unwrap();
                            let next_i = self.ip();
            
                            self.farcall(segment, offset, true);

                            // Save next address if we step over this CALL.
                            self.step_over_target = Some(CpuAddress::Segmented(self.cs, next_i));

                            // Add to call stack
                            self.push_call_stack(
                                CallStackEntry::CallF {
                                    ret_cs: self.cs,
                                    ret_ip: next_i,
                                    call_cs: segment,
                                    call_ip: offset
                                },
                                self.cs,
                                next_i
                            );
                        }
                        else if let OperandType::Register16(_) = self.i.operand1_type {
                            // Register form is invalid (can't use arbitrary modrm register as a pointer)
                            // We model the odd behavior of this invalid form here.

                            let seg = self.i.segment_override.unwrap_or(Segment::DS);
                            
                            // Read the segment from Seg:0004 
                            let offset = 0x0004;    
                            let segment = self.biu_read_u16(seg, offset, ReadWriteFlag::Normal);

                            self.cycle_i(0x06a);
                            self.biu_fetch_suspend();
                            self.cycles_i(3, &[0x06b, 0x06c]);
                            self.corr();

                            // Push CS
                            self.push_register16(Register16::CS, ReadWriteFlag::Normal);
                            let next_i = self.pc; // PC already corrected above
                            self.cs = segment;
                            //self.ip = self.last_ea; // I am not sure where IP gets its value.

                            self.cycles_i(3, &[0x06e, 0x06f, MC_JUMP]);
                            self.biu_queue_flush();
                            self.cycles_i(3, &[0x077, 0x078, 0x079]);

                            // Push next IP
                            self.push_u16(next_i, ReadWriteFlag::RNI);
                        }
                        jump = true;
                    }
                    // Jump to memory r/m16
                    Mnemonic::JMP => {
                        let ptr16 = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();

                        self.biu_fetch_suspend();
                        self.cycle_i(0x0d8);
                        self.pc = ptr16;
                        self.biu_queue_flush();
                        jump = true;
                    }
                    // Jump Far
                    Mnemonic::JMPF => {
                        let offset;

                        if let OperandType::AddressingMode(_mode) = self.i.operand1_type {
                            
                            self.cycle_i(0x0dc);
                            self.biu_fetch_suspend();
                            self.cycle_i(0x0dd);

                            let (segment, offset) = self.read_operand_farptr(self.i.operand1_type, self.i.segment_override, ReadWriteFlag::Normal).unwrap();

                            self.cs = segment;
                            self.pc = offset;
                            self.biu_queue_flush();                                
                        }
                        else {
                            // Register form is invalid (can't use arbitrary modrm register as a pointer)
                            // We model the odd behavior of this invalid form here.
                            let seg = self.i.segment_override.unwrap_or(Segment::DS);

                            self.cycle();
                            self.biu_fetch_suspend();
                            self.cycle();
                            
                            // Read the segment from Seg:0004 
                            offset = 0x0004;    
                            let segment = self.biu_read_u16(seg, offset, ReadWriteFlag::Normal);

                            self.cs = segment;
                            self.biu_queue_flush();
                        }
                        jump = true;
                        //log::trace!("JMPF: Destination [{:04X}:{:04X}]", segment, offset);
                    }                    
                    // Push Word onto stack
                    Mnemonic::PUSH => {
                        let mut op_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                        self.cycles_i(3, &[0x024, 0x025, 0x026]);
                        
                        // If SP, push the new value of SP instead of the old value
                        if let OperandType::Register16(Register16::SP) = self.i.operand1_type {
                            op_value = op_value.wrapping_sub(2);
                        }
                        self.push_u16(op_value, ReadWriteFlag::RNI);
                    }                    
                    _=> {
                        unhandled = true;
                    }
                }
            }
        }

        // Reset REP init flag. This flag is set after a rep-prefixed instruction is executed for the first time. It
        // should be preserved between executions of a rep-prefixed instruction unless an interrupt occurs, in which
        // case the rep-prefix instruction terminates normally after RPTI. This flag determines whether RPTS is
        // run when executing the instruction.
        if !self.in_rep {
            self.rep_init = false;
        }
        else {
            self.instruction_reentrant = true;
        }

        if unhandled {
            unreachable!("Invalid opcode!");
            //ExecutionResult::UnsupportedOpcode(self.i.opcode)
        }
        else if self.halted && !self.reported_halt && !self.get_flag(Flag::Interrupt) && !self.get_flag(Flag::Trap) {
            // CPU was halted with interrupts disabled - will not continue
            self.reported_halt = true;
            ExecutionResult::Halt
        }
        else if jump {
            ExecutionResult::OkayJump
        }
        else if self.in_rep {
            if let RepType::MulDiv = self.rep_type {
                // Rep prefix on MUL/DIV just sets flags, do not rep
                self.in_rep = false;
                Okay
            }
            else {
                self.rep_init = true;
                // Set step-over target so that we can skip long REP instructions.
                // Normally the step behavior during REP is to perform a single iteration.
                self.step_over_target = Some(CpuAddress::Segmented(self.cs, self.ip()));
                ExecutionResult::OkayRep
            }
        }
        else {
            match exception {
                CpuException::DivideError => ExecutionResult::ExceptionError(exception),
                CpuException::BoundsException => ExecutionResult::ExceptionError(exception),
                CpuException::NoException => Okay,
            }
        }
    }

    /// Execute an extended opcode (Prefixed with 0F).
    /// We can make some optimizations here as no instructions here take a REP prefix or perform
    /// flow control.
    #[rustfmt::skip]
    pub fn execute_extended_instruction(&mut self) -> ExecutionResult {
        let mut unhandled: bool = false;
        let mut jump: bool = false;
        let mut exception: CpuException = CpuException::NoException;

        self.step_over_target = None;

        self.trace_comment("EXECUTE_EXT");

        // TODO: Check optimization here. We could reset several flags at once if they were in a
        //       bitfield.
        // Reset instruction reentrancy flag
        self.instruction_reentrant = false;

        // Reset jumped flag.
        self.jumped = false;

        // Reset trap suppression flag
        self.trap_suppressed = false;

        // Decrement trap counters.
        self.trap_enable_delay = self.trap_enable_delay.saturating_sub(1);
        self.trap_disable_delay = self.trap_disable_delay.saturating_sub(1);

        // If we have an NX loaded RNI cycle from the previous instruction, execute it.
        // Otherwise, wait one cycle before beginning instruction if there was no modrm.
        if self.nx {
            self.trace_comment("RNI");
            self.cycle();
            self.nx = false;
        } else if self.last_queue_op == QueueOp::First {
            self.cycle();
        }
        
        match self.i.opcode {
            0x10 => {
                // TEST1, r/m8, CL
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let bit_n = self.get_register8(Register8::CL) & 0x03; // Mask CL to 3 bits.
                self.cycles(2 );
                self.set_flag_state(Flag::Zero, (1<<bit_n) & op1_value == 0);
            }
            0x11 => {
                // TEST1, r/m16, CL
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let bit_n = self.get_register8(Register8::CL) & 0x0F; // Mask CL to 4 bits.
                self.cycles(2 );
                self.set_flag_state(Flag::Zero, (1<<bit_n) & op1_value == 0);
            }
            _ => {
                unhandled = true;
            }
        }
        
        // Reset REP init flag. This flag is set after a rep-prefixed instruction is executed for the first time. It
        // should be preserved between executions of a rep-prefixed instruction unless an interrupt occurs, in which
        // case the rep-prefix instruction terminates normally after RPTI. This flag determines whether RPTS is
        // run when executing the instruction.
        if !self.in_rep {
            self.rep_init = false;
        }
        else {
            self.instruction_reentrant = true;
        }

        if unhandled {
            unreachable!("Invalid opcode!");
            //ExecutionResult::UnsupportedOpcode(self.i.opcode)
        }
        else if self.halted && !self.reported_halt && !self.get_flag(Flag::Interrupt) && !self.get_flag(Flag::Trap) {
            // CPU was halted with interrupts disabled - will not continue
            self.reported_halt = true;
            ExecutionResult::Halt
        }
        else if jump {
            ExecutionResult::OkayJump
        }
        else if self.in_rep {
            if let RepType::MulDiv = self.rep_type {
                // Rep prefix on MUL/DIV just sets flags, do not rep
                self.in_rep = false;
                Okay
            }
            else {
                self.rep_init = true;
                // Set step-over target so that we can skip long REP instructions.
                // Normally the step behavior during REP is to perform a single iteration.
                self.step_over_target = Some(CpuAddress::Segmented(self.cs, self.ip()));
                ExecutionResult::OkayRep
            }
        }
        else {
            match exception {
                CpuException::DivideError => ExecutionResult::ExceptionError(exception),
                CpuException::BoundsException => ExecutionResult::ExceptionError(exception),
                CpuException::NoException => Okay,
            }
        }        
    }
}

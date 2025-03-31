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

    cpu_vx0::execute_extended.rs

    Executes an instruction after it has been fetched.
    Includes extended (0x0F-prefixed) opcode implementations.
*/

use crate::{
    cpu_common::{
        alu::{AluAdc, AluSbb},
        CpuAddress,
        CpuException,
        ExecutionResult,
        Mnemonic,
        QueueOp,
        Register16,
        Register8,
        Segment,
    },
    cpu_vx0::{Flag, NecVx0, ReadWriteFlag, RepType},
};

// Bitfield width for BINS/BEXT instructions
pub enum BitfieldWidth {
    Word,
    DWord,
}

// rustfmt chokes on large match statements.
#[rustfmt::skip]
impl NecVx0 {
    /// Execute an extended opcode (Prefixed with 0F).
    /// We can make some optimizations here as no instructions here take a REP prefix or perform
    /// flow control.
    #[rustfmt::skip]
    pub fn execute_extended_instruction(&mut self) -> ExecutionResult {
        let mut unhandled: bool = false;
        let jump: bool = false;
        let exception: CpuException = CpuException::NoException;

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
            0x10 | 0x18 => {
                // TEST1, r/m8, CL | r/m8, imm8
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let bit_n = self.read_operand8(self.i.operand2_type, None).unwrap() & 0x07; // Mask bit_n to 3 bits.
                self.cycles(2 );
                let temp = op1_value & (1 << bit_n);
                self.set_szp_flags_from_result_u8(temp);
                self.set_flag_state(Flag::Zero, temp == 0);
                // Clear overflow and carry

                self.clear_flag(Flag::AuxCarry);
                self.clear_flag(Flag::Overflow);
                self.clear_flag(Flag::Carry);
            }
            0x11 | 0x19 => {
                // TEST1, r/m16, CL | r/m16, imm8
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let bit_n = self.read_operand8(self.i.operand2_type, None).unwrap() & 0x0F; // Mask bit_n to 4 bits.
                self.cycles(2 );
                let temp = op1_value & (1u16 << bit_n);
                self.set_szp_flags_from_result_u16(temp);
                self.set_flag_state(Flag::Zero, temp == 0);
                // Clear overflow and carry
                self.clear_flag(Flag::AuxCarry);
                self.clear_flag(Flag::Overflow);
                self.clear_flag(Flag::Carry);
            }
            0x12 | 0x1A => {
                // CLR1, r/m8, CL | r/m8, imm8
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let bit_n = self.read_operand8(self.i.operand2_type, None).unwrap() & 0x07; // Mask bit_n to 3 bits.
                self.cycles(2 );
                let temp = op1_value & !(1 << bit_n);
                //self.set_szp_flags_from_result_u8(temp);
                //self.set_flag_state(Flag::Zero, temp == 0);
                self.write_operand8(self.i.operand1_type, self.i.segment_override, temp, ReadWriteFlag::Normal);

                // Clear aux, overflow and carry
                //self.clear_flag(Flag::AuxCarry);
                //self.clear_flag(Flag::Overflow);
                //self.clear_flag(Flag::Carry);
            }
            0x13 | 0x1B => {
                // CLR1, r/m16, CL | r/m16, imm8
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let bit_n = self.read_operand8(self.i.operand2_type, None).unwrap() & 0x0F; // Mask bit_n to 4 bits.
                self.cycles(2 );
                let temp = op1_value & !(1u16 << bit_n);
                //self.set_szp_flags_from_result_u16(temp);
                //self.set_flag_state(Flag::Zero, temp == 0);
                self.write_operand16(self.i.operand1_type, self.i.segment_override, temp, ReadWriteFlag::Normal);

                // Clear aux, overflow and carry
                //self.clear_flag(Flag::AuxCarry);
                //self.clear_flag(Flag::Overflow);
                //self.clear_flag(Flag::Carry);
            }
            0x14 | 0x1C => {
                // SET1, r/m8, CL | r/m8, imm8
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let bit_n = self.read_operand8(self.i.operand2_type, None).unwrap() & 0x07; // Mask bit_n to 3 bits.
                self.cycles(2 );
                let temp = op1_value | (1 << bit_n);
                //self.set_szp_flags_from_result_u8(temp);
                //self.set_flag_state(Flag::Zero, temp == 0);
                self.write_operand8(self.i.operand1_type, self.i.segment_override, temp, ReadWriteFlag::Normal);

                // Clear aux, overflow and carry
                //self.clear_flag(Flag::AuxCarry);
                //self.clear_flag(Flag::Overflow);
                //self.clear_flag(Flag::Carry);
            }
            0x15 | 0x1D => {
                // SET1, r/m16, CL | r/m16, imm8
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let bit_n = self.read_operand8(self.i.operand2_type, None).unwrap() & 0x0F; // Mask bit_n to 4 bits.
                self.cycles(2 );
                let temp = op1_value | (1u16 << bit_n);
                //self.set_szp_flags_from_result_u16(temp);
                //self.set_flag_state(Flag::Zero, temp == 0);
                self.write_operand16(self.i.operand1_type, self.i.segment_override, temp, ReadWriteFlag::Normal);

                // Clear aux, overflow and carry
                //self.clear_flag(Flag::AuxCarry);
                //self.clear_flag(Flag::Overflow);
                //self.clear_flag(Flag::Carry);
            }
            0x16 | 0x1E => {
                // NOT1, r/m8, CL | r/m8, imm8
                // Flags: NOT1 does not modify flags
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                let bit_n = self.read_operand8(self.i.operand2_type, None).unwrap() & 0x07; // Mask bit_n to 3 bits.
                self.cycles(2 );
                let temp = op1_value ^ (1 << bit_n);
                //self.set_szp_flags_from_result_u8(temp);
                //self.set_flag_state(Flag::Zero, temp == 0);
                self.write_operand8(self.i.operand1_type, self.i.segment_override, temp, ReadWriteFlag::Normal);

                // Clear aux, overflow and carry
                //self.clear_flag(Flag::AuxCarry);
                //self.clear_flag(Flag::Overflow);
                //self.clear_flag(Flag::Carry);
            }
            0x17 | 0x1F => {
                // NOT1, r/m16, CL | r/m16, imm8
                // NOT1 does not modify flags
                let op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
                let bit_n = self.read_operand8(self.i.operand2_type, None).unwrap() & 0x0F; // Mask bit_n to 4 bits.
                self.cycles(2 );
                let temp = op1_value ^ (1u16 << bit_n);
                //self.set_szp_flags_from_result_u16(temp);
                //self.set_flag_state(Flag::Zero, temp == 0);
                self.write_operand16(self.i.operand1_type, self.i.segment_override, temp, ReadWriteFlag::Normal);

                // Clear aux, overflow and carry
                //self.clear_flag(Flag::AuxCarry);
                //self.clear_flag(Flag::Overflow);
                //self.clear_flag(Flag::Carry);
            }
            0x20 | 0x22 | 0x26 => {
                // ADD4S, SUB4s, CMP4S
                let segment_base_ds = self.i.segment_override.unwrap_or(Segment::DS);
                let mut bcd_carry = false;
                let mut bcd_overflow;
                let mut bcd_aux_carry = false;
                let mut result;
                // Valid count for BCD string operations is 1-254. A count of 0 or 255 will wrap
                // the loop counter to 65535.
                let terminating_count = self.c.l();
                let mut loop_counter: u16 = (terminating_count.wrapping_add(1) >> 1) as u16;
                let mut terminate = false;

                // SI and DI are not actually modified, so take copies
                let mut src_idx = self.si;
                let mut dst_idx = self.di;
                // Handle nibble pairs. The check for 0 emulates the infinite loop behavior.
                while !terminate {
                    let src = self.biu_read_u8(segment_base_ds, src_idx);
                    let dst = self.biu_read_u8(Segment::ES, dst_idx);
                    
                    
                    match self.i.mnemonic {
                        Mnemonic::ADD4S => {
                            (result, bcd_carry, bcd_overflow, bcd_aux_carry) = src.alu_adc(dst, bcd_carry);
                            self.set_flag_state(Flag::Zero, result == 0 && !bcd_carry);
                            // I have no idea how the sign flag is actually set...
                            self.set_flag_state(Flag::Sign, result & 0x80 != 0);
                            (result, bcd_carry, _, bcd_aux_carry) = self.daa_indirect(result, bcd_carry, bcd_overflow, bcd_aux_carry);

                            self.biu_write_u8(Segment::ES, dst_idx, result, ReadWriteFlag::Normal);
                        }
                        Mnemonic::SUB4S => {
                            (result, bcd_carry, bcd_overflow, bcd_aux_carry) = dst.alu_sbb(src, bcd_carry);
                            self.set_flag_state(Flag::Zero, false);
                            (result, bcd_carry, _, bcd_aux_carry) = self.das_indirect(result, bcd_carry, bcd_overflow, bcd_aux_carry);
                            self.biu_write_u8(Segment::ES, dst_idx, result, ReadWriteFlag::Normal);
                        }
                        Mnemonic::CMP4S => {
                            (result, bcd_carry, bcd_overflow, bcd_aux_carry) = dst.alu_sbb(src, bcd_carry);
                            self.set_flag_state(Flag::Zero, false);
                            (_, bcd_carry, _, bcd_aux_carry) = self.das_indirect(result, bcd_carry, bcd_overflow, bcd_aux_carry);
                        }
                        _ => {
                            unreachable!("bad decode");
                        }
                    }

                    src_idx = src_idx.wrapping_add(1);
                    dst_idx = dst_idx.wrapping_add(1);
                    // Deliberate underflow possibility if CL == 0 or 255
                    loop_counter = loop_counter.wrapping_sub(1);
                    if loop_counter == 0 {
                        terminate = true;
                    }
                    
                    
                }

                // Parity flag appears to always be cleared
                self.set_flag_state(Flag::Parity, false);
                self.set_flag_state(Flag::Carry, bcd_carry);
                self.set_flag_state(Flag::AuxCarry, bcd_aux_carry);
                self.set_flag_state(Flag::Overflow, false);
            }
            0x28 => {
                // ROL4
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                self.write_operand8(self.i.operand1_type, self.i.segment_override, (op1_value << 4) | (self.a.l() & 0x0F), ReadWriteFlag::Normal);
                self.set_register8(Register8::AL, (self.a.l() << 4) | (op1_value >> 4));
            }
            0x2A => {
                // ROR4
                let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
                self.write_operand8(self.i.operand1_type, self.i.segment_override, (self.a.l() << 4) | (op1_value >> 4), ReadWriteFlag::Normal);
                self.set_register8(Register8::AL, op1_value);
            }
            0x31 | 0x39 => {
                // INS/BINS/NECINS Bitfield insert
                // Operation can either operate on a word or dword of data. It performs a variable number of reads depending on
                // bit_idx and bit_len. 
                let mut r1: Option<u16> = None; // Read #1 preserves any bits to left of bit_idx in first word.
                let mut r2: Option<u16> = None; // Read #2 preserves any bits to the right of bit_end in first or second word.
                let bit_len = self.read_operand8(self.i.operand2_type, None).unwrap() & 0x0F; // Mask bit_len to 4 bits.
                let bit_idx = self.read_operand8(self.i.operand1_type, None).unwrap() & 0x0F; // Mask bit_idx to 4 bits.

                let bit_end = bit_idx + bit_len;
                // Determine if this is a word or dword operation based on whether bit_end spans word boundaries.
                let op_width = if bit_end < 16 { BitfieldWidth::Word } else { BitfieldWidth::DWord };
                
                // Read in r1 if bit_idx > 0
                if bit_idx > 0 {
                    r1 = Some(self.biu_read_u16(Segment::ES, self.di, ReadWriteFlag::Normal));
                }
                
                match op_width {
                    BitfieldWidth::Word => {
                        // Read in r2 if bit_end < 15
                        if bit_end < 15 {
                            r2 = Some(self.biu_read_u16(Segment::ES, self.di, ReadWriteFlag::Normal));
                        }
                        let trailing_bits = 16 - (bit_end + 1);
                        let word_mask = !((0xFFFF << bit_idx) & (0xFFFF >> trailing_bits));
                        
                        // Use the data read from either r1 or r2, as long as one happened. If neither happened, the data 
                        // doesn't matter anyway since the whole word will be replaced by AX.
                        let word_data: u16 = r2.unwrap_or(r1.unwrap_or(0)) & word_mask;

                        // Update bit_idx register to bit_end
                        // This must be done before AX is read in case someone was using AL or AH as operands
                        self.write_operand8(self.i.operand1_type, None, (16 - trailing_bits) & 0x0F, ReadWriteFlag::Normal);
                        
                        let ax_bits = self.a.x() & (0xFFFF >> (16-(bit_len + 1)));
                        let final_value = word_data | (ax_bits << bit_idx);
                        
                        self.biu_write_u16(Segment::ES, self.di, final_value, ReadWriteFlag::Normal);

                        if trailing_bits == 0 {
                            // We reached the end of the word boundary, so we need to increment di.
                            self.set_register16(Register16::DI, self.di.wrapping_add(2));
                        }

                    }
                    BitfieldWidth::DWord => {
                        let trailing_bits = 32 - (bit_end + 1);
                        let mut word_mask: u16 = !(0xFFFF << bit_idx);

                        // We will always have a r1 value since bit_idx has to be > 0 to have a dword size operation
                        let mut word_data: u16 = r1.unwrap() & word_mask;

                        // Update bit_idx register to bit_end
                        // This must be done before AX is read in case someone was using AL or AH as operands
                        self.write_operand8(self.i.operand1_type, None, (16 - trailing_bits) & 0x0F, ReadWriteFlag::Normal);

                        let mut ax_bits = self.a.x() & (0xFFFF >> (16-(bit_len + 1)));
                        let write1 = word_data | (ax_bits << bit_idx);

                        // Write out the first word and increment DI to next word
                        self.biu_write_u16(Segment::ES, self.di, write1, ReadWriteFlag::Normal);
                        self.set_register16(Register16::DI, self.di.wrapping_add(2));
                        
                        // Always read in r2 since there will always be at least one bit preserved in the second word.
                        word_mask = !(0xFFFF >> trailing_bits);
                        word_data = self.biu_read_u16(Segment::ES, self.di.wrapping_add(2), ReadWriteFlag::Normal) & word_mask;
                        ax_bits >>= 16 - bit_idx;
                        let write2 = word_data | ax_bits;
                        
                        // Write second word.
                        self.biu_write_u16(Segment::ES, self.di, write2, ReadWriteFlag::Normal);
                        if trailing_bits == 0 {
                            // We reached the end of the dword boundary, so we need to increment di again to point to the next dword.
                            self.set_register16(Register16::DI, self.di.wrapping_add(2));
                        }

                    }
                }
            }
            0x33 | 0x3B => {
                // BEXT
                let base_segment_ds = self.i.segment_override.unwrap_or(Segment::DS);
                let bit_len = self.read_operand8(self.i.operand2_type, None).unwrap() & 0x0F; // Mask bit_len to 4 bits.
                let bit_idx = self.read_operand8(self.i.operand1_type, None).unwrap() & 0x0F; // Mask bit_idx to 4 bits.

                let bit_end = bit_idx + bit_len;
                // Determine if this is a word or dword operation based on whether bit_end spans word boundaries.
                let op_width = if bit_end < 16 { BitfieldWidth::Word } else { BitfieldWidth::DWord };

                match op_width {
                    BitfieldWidth::Word => {
                        let trailing_bits = 16 - (bit_end + 1);
                        let word_mask = (0xFFFF << bit_idx) & (0xFFFF >> trailing_bits);
                        let word_data = self.biu_read_u16(base_segment_ds, self.si, ReadWriteFlag::Normal) & word_mask;
                        
                        log::warn!("word data: {:04X} mask: {:016b}", word_data, word_mask);
                        self.set_register16(Register16::AX, word_data >> bit_idx);

                        // Update bit_idx register to bit_end
                        // This must be done before AX is read in case someone was using AL or AH as operands
                        self.write_operand8(self.i.operand1_type, None, (16 - trailing_bits) & 0x0F, ReadWriteFlag::Normal);
                        
                        if trailing_bits == 0 {
                            // We reached the end of the word boundary, so we need to increment di.
                            self.set_register16(Register16::SI, self.si.wrapping_add(2));
                        }
                    }
                    BitfieldWidth::DWord => {
                        let trailing_bits = 32 - (bit_end + 1);
                        let mut word_mask: u16 = 0xFFFF << bit_idx;
                        let word1_data = self.biu_read_u16(base_segment_ds, self.si, ReadWriteFlag::Normal) & word_mask;
                        self.set_register16(Register16::SI, self.si.wrapping_add(2));
                        
                        word_mask = 0xFFFF >> trailing_bits;
                        let word2_data = self.biu_read_u16(base_segment_ds, self.si, ReadWriteFlag::Normal) & word_mask;
                        self.set_register16(Register16::AX, (word1_data >> bit_idx) | (word2_data << (16-bit_idx)));

                        // Update bit_idx register to bit_end
                        // This must be done before AX is read in case someone was using AL or AH as operands
                        self.write_operand8(self.i.operand1_type, None, (16 - trailing_bits) & 0x0F, ReadWriteFlag::Normal);
                        
                        if trailing_bits == 0 {
                            // We reached the end of the dword boundary, so we need to increment di again to point to the next dword.
                            self.set_register16(Register16::DI, self.di.wrapping_add(2));
                        }

                    }
                }                
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
            unreachable!("Invalid opcode: {:02X}", self.i.opcode);
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
                ExecutionResult::Okay
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
                CpuException::NoException => ExecutionResult::Okay,
            }
        }
    }
}

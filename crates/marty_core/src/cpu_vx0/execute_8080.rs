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
        alu::{AluAdc, AluAdd, AluRotateCarryLeft, AluRotateCarryRight, AluRotateLeft, AluRotateRight, AluSbb},
        mnemonic::Mnemonic8080,
        CpuAddress,
        CpuArch,
        CpuException,
        CpuType,
        ExecutionResult,
        Mnemonic,
        QueueOp,
        Register16,
        Register16_8080,
        Register8,
        Segment,
    },
    cpu_vx0::{microcode::MC_JUMP, Flag, NecVx0, ReadWriteFlag, RepType, REGISTER16_8080_LUT, REGISTER8_8080_LUT},
};

// Bitfield width for BINS/BEXT instructions
pub enum BitfieldWidth {
    Word,
    DWord,
}

// rustfmt chokes on large match statements.
#[rustfmt::skip]
impl NecVx0 {
    /// Execute an 8080 opcode (When in 8080 emulation mode).
    #[rustfmt::skip]
    pub fn execute_8080_instruction(&mut self) -> ExecutionResult {
        let mut unhandled: bool = false;
        let mut jump: bool = false;
        let exception: CpuException = CpuException::NoException;

        self.step_over_target = None;

        self.trace_comment("EXECUTE_8080");

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

        let param0 = self.i.opcode & 0x07;
        let param1 = (self.i.opcode >> 3) & 0x07;

        // Decode by octal organization. The top match will be on the 2 MSB bits of the opcode.
        match self.i.opcode & 0xC0 {
            // Quadrant 0
            0b0000_0000 => {
                match param0 {
                    0 => {
                        match param1 {
                            0 => {
                                // 00 NOP
                                self.cycles(2);
                            }
                            1 => {
                                // 08 Invalid 1-byte
                                self.cycles(2);
                            }
                            2 => {
                                //10 Invalid 3-byte
                                let _dummy = self.read_operand16(self.i.operand1_type, None).unwrap();
                            }
                            3 => {
                                // 18 Invalid 1-byte
                                self.cycles(2);
                            }
                            4 => {
                                // 20 Invalid 3-byte
                                let _dummy = self.read_operand16(self.i.operand1_type, None).unwrap();
                            }
                            5 => {
                                // 28 Invalid 1-byte
                                self.cycles(2);
                            }
                            6 => {
                                // 30 Invalid 3-byte
                                let _dummy = self.read_operand16(self.i.operand1_type, None).unwrap();
                            }
                            7 => {
                                // 38 Invalid 1-byte
                                self.cycles(2);
                            }
                            _ => {

                            }
                        }
                    }
                    1 => {
                        // LXI / DAD
                        match param1 {
                            0 => {
                                // 01 LXI B
                                let value = self.read_operand16(self.i.operand2_type, None).unwrap();
                                self.set_register16(Register16::CX, value);
                            }
                            2 => {
                                // 11 LXI D
                                let value = self.read_operand16(self.i.operand2_type, None).unwrap();
                                self.set_register16(Register16::DX, value);
                            }
                            4 => {
                                // 21 LXI H
                                let value = self.read_operand16(self.i.operand2_type, None).unwrap();
                                self.set_register16(Register16::BX, value);
                            }
                            6 => {
                                // 31 LXI SP
                                let value = self.read_operand16(self.i.operand2_type, None).unwrap();
                                self.set_register16(Register16::BP, value);
                            }
                            1 => {
                                // 09 DAD B (HL = HL + BC)
                                let (result, carry, _, _) = self.hl_80().alu_adc(self.bc_80(), false);
                                self.set_register16(Register16::BX, result);
                                self.set_flag_state(Flag::Carry, carry);
                            }
                            3 => {
                                // 19 DAD D (HL = HL + DE)
                                let (result, carry, _, _) = self.hl_80().alu_adc(self.de_80(), false);
                                self.set_register16(Register16::BX, result);
                                self.set_flag_state(Flag::Carry, carry);
                            }
                            5 => {
                                // 29 DAD H (HL = HL + HL)
                                let (result, carry, _, _) = self.hl_80().alu_adc(self.hl_80(), false);
                                self.set_register16(Register16::BX, result);
                                self.set_flag_state(Flag::Carry, carry);                                
                            }
                            _ => {
                                // 39 DAD SP (HL = HL + SP)
                                let (result, carry, _, _) = self.hl_80().alu_adc(self.sp_80(), false);
                                self.set_register16(Register16::BX, result);
                                self.set_flag_state(Flag::Carry, carry);                                
                            }
                        }
                    }
                    2 => {
                        // STAX / LDAX / SHLD / LHLD / STA / LDA
                        match param1 {
                            0 => {
                                // 02 STAX BC ((BC) <- A)
                                let value = self.acc_80();
                                self.write_operand8(self.i.operand1_type, None, value, ReadWriteFlag::RNI);
                            }
                            1 => {
                                // 0A LDAX BC (A <- (BC))
                                let value = self.read_operand8(self.i.operand1_type, None).unwrap();
                                self.set_register8(Register8::AL, value);
                            }
                            2 => {
                                // 12 STAX DE ((DE) <- A)
                                let value = self.acc_80();
                                self.write_operand8(self.i.operand1_type, None, value, ReadWriteFlag::RNI);
                            }
                            3 => {
                                // 1A LDAX D (A <- (DE))
                                let value = self.read_operand8(self.i.operand1_type, None).unwrap();
                                self.set_register8(Register8::AL, value);
                            }
                            4 => {
                                // 22 SHLD addr (addr <- HL)
                                let addr = self.read_operand16(self.i.operand1_type, None).unwrap();
                                self.biu_write_u16(Segment::DS, addr, self.hl_80(), ReadWriteFlag::RNI);
                            }
                            5 => {
                                // 2A LHLD addr (HL <- addr)
                                let addr = self.read_operand16(self.i.operand1_type, None).unwrap();
                                let value = self.biu_read_u16(Segment::DS, addr, ReadWriteFlag::RNI);
                                self.set_register16(Register16::from_r16_8080(Register16_8080::HL), value);
                            }
                            6 => {
                                // STA addr (addr <- A)
                                let addr = self.read_operand16(self.i.operand1_type, None).unwrap();
                                self.biu_write_u8(Segment::DS, addr, self.acc_80(), ReadWriteFlag::RNI);
                            }
                            _ => {
                                // 32 LDA addr (A <- addr)
                                let addr = self.read_operand16(self.i.operand1_type, None).unwrap();
                                let value = self.biu_read_u8(Segment::DS, addr);
                                self.set_register8(Register8::AL, value);
                            }
                        }
                    }
                    3 if param1 & 1 == 0 => {
                        // 03, 13, 23, 33  INX (Increment register pair) Flags: None
                        let reg = REGISTER16_8080_LUT[(param1 >> 1) as usize];
                        let value = self.get_register16(reg);
                        self.set_register16(reg, value.wrapping_add(1));
                    }
                    3 if param1 & 1 != 0 => {
                        // 0B, 1B, 2B, 3B DCX (Decrement register pair) Flags: None
                        let reg = REGISTER16_8080_LUT[(param1 >> 1) as usize];
                        let value = self.get_register16(reg);
                        self.set_register16(reg, value.wrapping_sub(1));
                    }
                    4 => {
                        // INR (Increment Register) Flags: Z,S,P,AC
                        let value = self.read_operand8(self.i.operand1_type, None).unwrap();
                        let (result, _carry, _overflow, aux_carry) = value.alu_add(1);
                        self.write_operand8(self.i.operand1_type, None, result, ReadWriteFlag::RNI);
                        self.set_szp_flags_from_result_u8(result);
                        self.set_flag_state(Flag::AuxCarry, aux_carry);
                    }
                    5 => {
                        // 05, 0D, 15, 1D, 25, 2D, 35, 3D DCR (Decrement Register/Memory) Flags: Z,S,P,AC
                        let value = self.read_operand8(self.i.operand1_type, None).unwrap();
                        let (result, _carry, _overflow, aux_carry) = value.alu_sbb(1, false);
                        self.write_operand8(self.i.operand1_type, None, result, ReadWriteFlag::RNI);
                        self.set_szp_flags_from_result_u8(result);
                        self.set_flag_state(Flag::AuxCarry, aux_carry);
                    }
                    6 => {
                        // MVI (Move)
                        let value = self.read_operand8(self.i.operand2_type, None).unwrap();
                        self.write_operand8(self.i.operand1_type, None, value, ReadWriteFlag::RNI);
                    }
                    _ => {
                        // RLC / RRC / RAL / RAR / DAA / CMA / STC / CMC
                        match param1 {
                            0 => {
                                // RLC (Rotate ACC left - set carry)
                                let value = self.acc_80();
                                let (result, carry, _overflow) = value.alu_rol(1);
                                self.set_flag_state(Flag::Carry, carry);
                                self.set_register8(Register8::AL, result);
                            }
                            1 => {
                                // RRC (Rotate ACC right - set carry)
                                let value = self.acc_80();
                                 let (result, carry, _overflow) = value.alu_ror(1);
                                self.set_flag_state(Flag::Carry, carry);
                                self.set_register8(Register8::AL, result);
                            }
                            2 => {
                                // RAL (Rotate ACC left through carry)
                                let value = self.acc_80();
                                let (result, carry, _overflow) = value.alu_rcl(1, self.get_flag(Flag::Carry));
                                self.set_flag_state(Flag::Carry, carry);
                                self.set_register8(Register8::AL, result);
                            }
                            3 => {
                                // RAR (Rotate ACC right through carry)
                                let value = self.acc_80();
                                let (result, carry, _overflow) = value.alu_rcr(1, self.get_flag(Flag::Carry));
                                self.set_flag_state(Flag::Carry, carry);
                                self.set_register8(Register8::AL, result);
                            }
                            4 => {
                                // DAA (Decimal Adjust ACC after addition)
                                let value = self.acc_80();
                                // TODO: 8080 mode might need a unique DAA - need to test
                                self.daa();
                            }
                            5 => {
                                // CMA (Complement ACC)
                                self.set_register8(Register8::AL, !self.acc_80());
                            }
                            6 => {
                                // STC (Set Carry Flag)
                                self.set_flag(Flag::Carry);
                            }
                            _ => {
                                // CMC (Complement Carry)
                                self.set_flag_state(Flag::Carry, !self.get_flag(Flag::Carry));
                            }
                        }
                    }
                }
            }
            // Quadrant 1 - MOV and HLT
            0b0100_0000 => {
                if self.i.opcode == 0x76 {
                    // HLT
                    self.halted = true;
                    self.reported_halt = false;
                    self.trace_comment("HLT");
                    return ExecutionResult::Halt;
                }
                else {
                    let dst = self.read_operand8(self.i.operand2_type, None).unwrap();
                    self.write_operand8(self.i.operand1_type, None, dst, ReadWriteFlag::RNI);
                }
            }
            // Quadrant 2 - ALU operations
            0b1000_0000 => {
                match param1 {
                    0 => {
                        // ADD
                        let rhs = self.read_operand8(self.i.operand1_type, None).unwrap();
                        let (result, carry, _overflow, aux_carry) = self.acc_80().alu_adc(rhs, false);
                        self.set_flag_state(Flag::Carry, carry);
                        self.set_flag_state(Flag::AuxCarry, aux_carry);
                        self.set_szp_flags_from_result_u8(result);
                        self.set_register8(Register8::AL, result);
                    }
                    1 => {
                        // ADC
                        let rhs = self.read_operand8(self.i.operand1_type, None).unwrap();
                        let (result, carry, _overflow, aux_carry) = self.acc_80().alu_adc(rhs, self.get_flag(Flag::Carry));
                        self.set_flag_state(Flag::Carry, carry);
                        self.set_flag_state(Flag::AuxCarry, aux_carry);
                        self.set_szp_flags_from_result_u8(result);
                        self.set_register8(Register8::AL, result);
                    }
                    2 => {
                        // SUB
                        let rhs = self.read_operand8(self.i.operand1_type, None).unwrap();
                        let (result, carry, _overflow, aux_carry) = self.acc_80().alu_sbb(rhs, false);
                        self.set_flag_state(Flag::Carry, carry);
                        self.set_flag_state(Flag::AuxCarry, aux_carry);
                        self.set_szp_flags_from_result_u8(result);
                        self.set_register8(Register8::AL, result);
                    }
                    3 => {
                        // SBB
                        let rhs = self.read_operand8(self.i.operand1_type, None).unwrap();
                        let (result, carry, _overflow, aux_carry) = self.acc_80().alu_sbb(rhs, self.get_flag(Flag::Carry));
                        self.set_flag_state(Flag::Carry, carry);
                        self.set_flag_state(Flag::AuxCarry, aux_carry);
                        self.set_szp_flags_from_result_u8(result);
                        self.set_register8(Register8::AL, result);
                    }
                    4 => {
                        // ANA
                        let rhs = self.read_operand8(self.i.operand1_type, None).unwrap();
                        let result = self.acc_80() & rhs;
                        self.set_szp_flags_from_result_u8(result);
                        self.set_register8(Register8::AL, result);
                        self.clear_flag(Flag::Carry);
                        self.clear_flag(Flag::AuxCarry);
                    }
                    5 => {
                        // XRA
                        let rhs = self.read_operand8(self.i.operand1_type, None).unwrap();
                        let result = self.acc_80() ^ rhs;
                        self.set_szp_flags_from_result_u8(result);
                        self.set_register8(Register8::AL, result);
                        self.clear_flag(Flag::Carry);
                        self.clear_flag(Flag::AuxCarry);
                    }
                    6 => {
                        // ORA
                        let value = self.read_operand8(self.i.operand1_type, None).unwrap();
                        let result = self.acc_80() | value;
                        self.set_szp_flags_from_result_u8(result);
                        self.set_register8(Register8::AL, result);
                        self.clear_flag(Flag::Carry);
                        self.clear_flag(Flag::AuxCarry);
                    }
                    _ => {
                        // CMP
                        let rhs = self.read_operand8(self.i.operand1_type, None).unwrap();
                        let (result, carry, _overflow, aux_carry) = self.acc_80().alu_sbb(rhs, false);
                        self.set_flag_state(Flag::Carry, carry);
                        self.set_flag_state(Flag::AuxCarry, aux_carry);
                        self.set_szp_flags_from_result_u8(result);
                    }
                }
            }
            // Quadrant 3
            0b1100_0000 => {
                match param0 {
                    0 => {
                        match param1 {
                            0 => {
                                // RNZ - Return if not zero
                                if !self.get_flag(Flag::Zero) {
                                    self.ret_8080();
                                    jump = true;
                                }
                            }
                            1 => {
                                // RZ - Return if zero
                                if self.get_flag(Flag::Zero) {
                                    self.ret_8080();
                                    jump = true;
                                }
                            }
                            2 => {
                                // RNC - Return if not carry
                                if !self.get_flag(Flag::Carry) {
                                    self.ret_8080();
                                    jump = true;
                                }
                            }
                            3 => {
                                // RC - Return if carry
                                if self.get_flag(Flag::Carry) {
                                    self.ret_8080();
                                    jump = true;
                                }
                            }
                            4 => {
                                // RPO - Return if parity odd
                                if !self.get_flag(Flag::Parity) {
                                    self.ret_8080();
                                    jump = true;
                                }
                            }
                            5 => {
                                // RPE - Return if parity even
                                if self.get_flag(Flag::Parity) {
                                    self.ret_8080();
                                    jump = true;
                                }
                            },
                            6 => {
                                // RP - Return if plus
                                if !self.get_flag(Flag::Sign) {
                                    self.ret_8080();
                                    jump = true;
                                }
                            },
                            _ => {
                                // RM - Return if minus
                                if self.get_flag(Flag::Sign) {
                                    self.ret_8080();
                                    jump = true;
                                }
                            }
                        }
                    },
                    1 => {
                        match param1 {
                            0 => {
                                // C1 POP BC
                                self.pop_register16_8080(Register16_8080::BC);
                            }
                            1 => {
                                // C9 RET
                                self.ret_8080();
                            }
                            2 => {
                                // D1 POP DE
                                self.pop_register16_8080(Register16_8080::DE);
                            }
                            3 => {
                                // D9 Invalid 3-byte opcode
                                let _dummy = self.read_operand16(self.i.operand1_type, None).unwrap();
                                unhandled = true;
                            }
                            4 => {
                                // E1 POP HL
                                self.pop_register16_8080(Register16_8080::HL);
                            }
                            5 => {
                                // E9 PCHL
                                self.pchl_8080();
                            }
                            6 => {
                                // F1 POP PSW
                                self.pop_psw_8080();
                            }
                            _ => {
                                // F9 SPHL (SP <- HL)
                                self.set_register16(Register16::BP, self.hl_80());
                            }
                        }
                    },
                    2 => {
                        // Conditional jumps.
                        let do_jump = match param1 {
                            0 => !self.get_flag(Flag::Zero),
                            1 => self.get_flag(Flag::Zero),
                            2 => !self.get_flag(Flag::Carry),
                            3 => self.get_flag(Flag::Carry),
                            4 => !self.get_flag(Flag::Parity),
                            5 => self.get_flag(Flag::Parity),
                            6 => !self.get_flag(Flag::Sign),
                            _ => self.get_flag(Flag::Sign),
                        };
                        
                        let new_pc = self.read_operand16(self.i.operand1_type, None).unwrap();
                        
                        if do_jump {
                            
                            self.cycle_i(MC_JUMP);
                            self.biu_fetch_suspend();
                            self.cycles(2);
                            self.corr();
                            self.pc = new_pc;
                            self.biu_queue_flush();
                            self.cycles(2);
                            jump = true;
                        }
                    }
                    3 => {
                        match param1 {
                            0 => {
                                let new_pc = self.read_operand16(self.i.operand1_type, None).unwrap();
                                // C3 JMP addr
                                self.cycle_i(MC_JUMP);
                                self.biu_fetch_suspend();
                                self.cycles(2);
                                self.corr();
                                self.pc = new_pc;
                                self.biu_queue_flush();
                                self.cycles(2);
                                jump = true;
                            }
                            1 => {
                                // CB Invalid, 3-byte opcode
                                let _dummy = self.read_operand16(self.i.operand1_type, None).unwrap();
                                unhandled = true;
                            }
                            2 => {
                                // D3 OUT D8
                                let io_addr = self.read_operand8(self.i.operand1_type, None).unwrap();
                                self.biu_io_write_u8(io_addr as u16, self.acc_80(), ReadWriteFlag::RNI);
                            }
                            3 => {
                                // FB IN D8
                                let io_addr = self.read_operand8(self.i.operand1_type, None).unwrap();
                                let byte = self.biu_io_read_u8(io_addr as u16);
                                self.a.set_l(byte);
                            }
                            4 => {
                                // E3 XTHL (Exchange HL with top of stack)
                                let temp = self.hl_80();
                                self.pop_register16_8080(Register16_8080::HL);
                                self.push_u16_8080(temp);
                            }
                            5 => {
                                // EB XCHG (Exchange HL with DE)
                                let temp = self.hl_80();
                                self.set_register16(Register16::from_r16_8080(Register16_8080::HL), self.de_80());
                                self.set_register16(Register16::from_r16_8080(Register16_8080::DE), temp);
                            }
                            6 => {
                                // F3 DI (Disable interrupts)
                                self.clear_flag(Flag::Interrupt);
                            }
                            _ => {
                                // FB EI (Enable Interrupts)
                                self.set_flag(Flag::Interrupt);
                            }
                        }
                    }
                    4 => {
                        // Conditional calls
                        let do_call = match param1 {
                            0 => !self.get_flag(Flag::Zero),
                            1 => self.get_flag(Flag::Zero),
                            2 => !self.get_flag(Flag::Carry),
                            3 => self.get_flag(Flag::Carry),
                            4 => !self.get_flag(Flag::Parity),
                            5 => self.get_flag(Flag::Parity),
                            6 => !self.get_flag(Flag::Sign),
                            _ => self.get_flag(Flag::Sign),
                        };

                        let addr = self.read_operand16(self.i.operand1_type, None).unwrap();
                        if do_call {
                            self.call_8080(addr);
                            jump = true;
                        }
                    }
                    5 => {
                        match param1 {
                            0 => {
                                // C5 PUSH BC
                                self.push_register16_8080(Register16_8080::BC);
                            }
                            1 => {
                                // CD CALL addr
                                let addr = self.read_operand16(self.i.operand1_type, None).unwrap();
                                self.call_8080(addr);
                                jump = true;                                
                            }
                            2 => {
                                // D5 PUSH DE
                                self.push_register16_8080(Register16_8080::DE);
                            }
                            3 => {
                                // DD Invalid 2-byte opcode
                                let _dummy = self.read_operand8(self.i.operand1_type, None).unwrap();
                                unhandled = true;
                            }
                            4 => {
                                // E5 PUSH H
                                self.push_register16_8080(Register16_8080::HL);
                            }
                            5 => {
                                // ED CALLN & RETEM
                                log::warn!("ED V20 escape opcode!");
                                match self.i.mnemonic {
                                    Mnemonic::I8080(mnem) if matches!(mnem, Mnemonic8080::CALLN) => {
                                        let irq = self.read_operand8(self.i.operand1_type, None).unwrap();
                                        self.calln_8080(irq);
                                    }
                                    Mnemonic::I8080(mnem) if matches!(mnem, Mnemonic8080::RETEM) => {
                                        log::warn!("RETEM!");
                                        self.ret(true);
                                        self.pop_flags();
                                        // TODO: Does RETEM itself control leaving emulation mode or is it simply the 
                                        //       status of the MD bit? If the latter, we should probably move enter/exit
                                        //       emulation mode to pop_flags().
                                        self.exit_emulation_mode();
                                        jump = true;
                                    }
                                    _ => {
                                        unhandled = true;
                                    }
                                }
                            }
                            6 => {
                                // F5 PUSH PSW
                                self.push_psw_8080();
                            }
                            _ => {
                                // FD Invalid 2-byte opcode
                                let _dummy = self.read_operand8(self.i.operand1_type, None).unwrap();
                                unhandled = true;
                            }
                        }
                    }
                    6 => {
                        // ALU ops with immediate
                        match param1 {
                            0 => {
                                // ADI (Add immediate to A)
                                let value = self.read_operand8(self.i.operand1_type, None).unwrap();
                                let (result, carry, _overflow, aux_carry) = self.acc_80().alu_adc(value, false);
                                self.set_flag_state(Flag::Carry, carry);
                                self.set_flag_state(Flag::AuxCarry, aux_carry);
                                self.set_szp_flags_from_result_u8(result);
                                self.set_register8(Register8::AL, result);
                            }
                            1 => {
                                // ACI (Add immediate to A with carry)
                                let value = self.read_operand8(self.i.operand1_type, None).unwrap();
                                let (result, carry, _overflow, aux_carry) = self.acc_80().alu_adc(value, self.get_flag(Flag::Carry));
                                self.set_flag_state(Flag::Carry, carry);
                                self.set_flag_state(Flag::AuxCarry, aux_carry);
                                self.set_szp_flags_from_result_u8(result);
                                self.set_register8(Register8::AL, result);
                            }
                            2 => {
                                // SUI (Subtract immediate from A)
                                let value = self.read_operand8(self.i.operand1_type, None).unwrap();
                                let (result, carry, _overflow, aux_carry) = self.acc_80().alu_sbb(value, false);
                                self.set_flag_state(Flag::Carry, carry);
                                self.set_flag_state(Flag::AuxCarry, aux_carry);
                                self.set_szp_flags_from_result_u8(result);
                                self.set_register8(Register8::AL, result);
                            }
                            3 => {
                                // SBI (Subtract immediate from A with borrow)
                                let value = self.read_operand8(self.i.operand1_type, None).unwrap();
                                let (result, carry, _overflow, aux_carry) = self.acc_80().alu_sbb(value, self.get_flag(Flag::Carry));
                                self.set_flag_state(Flag::Carry, carry);
                                self.set_flag_state(Flag::AuxCarry, aux_carry);
                                self.set_szp_flags_from_result_u8(result);
                                self.set_register8(Register8::AL, result);
                            }
                            4 => {
                                // ANI (AND immediate with A)
                                let value = self.read_operand8(self.i.operand1_type, None).unwrap();
                                let result = self.acc_80() & value;
                                self.set_szp_flags_from_result_u8(result);
                                self.set_register8(Register8::AL, result);
                                self.clear_flag(Flag::Carry);
                                self.clear_flag(Flag::AuxCarry);
                            }
                            5 => {
                                // XRI (XOR immediate with A)
                                let value = self.read_operand8(self.i.operand1_type, None).unwrap();
                                let result = self.acc_80() ^ value;
                                self.set_szp_flags_from_result_u8(result);
                                self.set_register8(Register8::AL, result);
                                self.clear_flag(Flag::Carry);
                                self.clear_flag(Flag::AuxCarry);
                            }
                            6 => {
                                // ORI (OR immediate with A)
                                let value = self.read_operand8(self.i.operand1_type, None).unwrap();
                                let result = self.acc_80() | value;
                                self.set_szp_flags_from_result_u8(result);
                                self.set_register8(Register8::AL, result);
                                self.clear_flag(Flag::Carry);
                                self.clear_flag(Flag::AuxCarry);                                
                            }
                            _ => {
                                // CPI (Compare immediate with A)
                                let value = self.read_operand8(self.i.operand1_type, None).unwrap();
                                let (result, carry, _overflow, aux_carry) = self.acc_80().alu_sbb(value, false);
                                self.set_flag_state(Flag::Carry, carry);
                                self.set_flag_state(Flag::AuxCarry, aux_carry);
                                self.set_szp_flags_from_result_u8(result);
                            }
                        }
                    }
                    _ => {
                        // C7,CF,D7,DF,E7,EF,F7,FF RST.
                        let new_pc = (param1 as u16) * 8; // RST 0-7
                        self.call_8080(new_pc);
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
            log::warn!("Invalid opcode: {:02X}", self.i.opcode);
            //ExecutionResult::UnsupportedOpcode(self.i.opcode)
            ExecutionResult::Okay
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

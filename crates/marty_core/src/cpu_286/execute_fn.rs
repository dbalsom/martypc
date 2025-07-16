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

//! Microcode execution functions for the 808x CPU.

use crate::{
    cpu_286::{
        microcode::MC_JUMP,
        muldiv::{Cord, Corx},
        CallStackEntry,
        Flag,
        Intel286,
        RepType,
        IO_READ_BREAKPOINT,
        IO_WRITE_BREAKPOINT,
        REGISTER16_LUT,
        SREGISTER_LUT,
    },
    cpu_common::{
        alu::Xi,
        CpuAddress,
        CpuException,
        InstructionWidth,
        Mnemonic,
        OperandType,
        Register16,
        Register8,
        Segment,
        OPCODE_PREFIX_REP1,
        OPCODE_PREFIX_REP2,
    },
    cycles,
    cycles_mc,
    util,
};

#[rustfmt::skip]
impl Intel286 {
    pub fn mc_nop(&mut self) {
        // Handle non-microcoded instructions.
        match self.i.opcode {
            0x9B => {
                // WAIT
                cycles!(self, 3);
            }
            0xF4 => {
                // HLT - Halt
                self.biu_bus_wait_halt(); // wait until at least t2 of m-cycle
                self.halt_not_hold = true; // set internal halt signal
                self.biu_fetch_halt(); // halt prefetcher
                self.biu_bus_wait_finish(); // wait until end of m-cycle

                if self.intr {
                    // If an intr is pending now, execute it without actually halting.

                    // log::trace!("Halt overridden at [{:05X}]", Intel286::calc_linear_address(self.cs, self.ip()));
                    self.cycles(2); // Cycle to load interrupt routine
                    self.halt_not_hold = false;
                }
                else {
                    // Actually halt
                    log::trace!("Halt at [{:05X}]", Intel286::calc_linear_address(self.cs, self.ip()));
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
            _ => {
                log::error!("Unhandled non-microcoded instruction: {:02X}", self.i.opcode);
            }
        }
    }

    /// MOV rm<->reg
    /// Opcodes 88-8B
    pub fn mc_000(&mut self) {
        let op_value = self.read_operand(self.i.operand2_type, self.i.segment_override);
        if self.i.operand1_type.is_address() {
            cycles_mc!(self, 0x000, 0x001);
        }
        self.write_operand(self.i.operand1_type, self.i.segment_override, op_value);
    }

    /// LEA
    /// Opcode 8D
    pub fn mc_004(&mut self) {
        let ea = self.load_effective_address(self.i.operand2_type);
        match ea {
            Some(value) => {
                self.write_operand16(self.i.operand1_type, None, value);
            }
            None => {
                // In the event of an invalid (Register) operand2, operand1 is set to the last EA calculated by an instruction.
                self.write_operand16(self.i.operand1_type, None, self.last_ea);
            }
        }
    }

    /// ALU rm<->reg
    pub fn mc_008(&mut self) {
        let op1_value = self.read_operand(self.i.operand1_type, self.i.segment_override);
        let op2_value = self.read_operand(self.i.operand2_type, self.i.segment_override);
        self.cycle_i(0x008);
        let xi = self.i.xi.unwrap();
        let result = self.alu_op(xi, op1_value, op2_value);
        match xi {
            Xi::CMP => {}
            _ => {
                if self.i.operand1_type.is_address() {
                    cycles_mc!(self, 0x009, 0x00a);
                }
                self.write_operand(self.i.operand1_type, self.i.segment_override, result);
            }
        }
    }

    /// ALU reg, imm
    /// Opcodes 80,81,82,83
    pub fn mc_00c(&mut self) {
        let op1_value = self.read_operand(self.i.operand1_type, self.i.segment_override);
        let op2_value = self.read_operand(self.i.operand2_type, self.i.segment_override);

        // The microcode here uses L8 to skip reading the second byte of an immediate when the
        // instruction is byte-sized (W bit 0). However, 83 breaks this pattern. It is a word-sized
        // instruction with a byte-size immediate. What signals the CPU to override the operand
        // size for 83 isn't clear. So we have this ugly opcode check.
        if InstructionWidth::Byte == self.i.width || self.i.opcode == 0x83 {
            self.cycle_i(MC_JUMP); // Jump over 2nd queue read when byte width
        }

        let result = self.alu_op(self.i.xi.unwrap(), op1_value, op2_value);

        if self.i.operand1_type.is_address() {
            cycles_mc!(self, 0x00e);
        }

        if self.i.mnemonic != Mnemonic::CMP {
            self.write_operand(self.i.operand1_type, self.i.segment_override, result);
        }
    }

    /// MOV rm, imm
    /// Opcodes C6, C7
    pub fn mc_014(&mut self) {
        let op2_value = self.read_operand(self.i.operand2_type, self.i.segment_override);
        if self.i.width == InstructionWidth::Byte {
            self.cycle_i(MC_JUMP); // 0x014 jumps over 2nd queue read
        }
        if self.i.operand1_type.is_address() {
            // This cycle is an RNI for register operands
            cycles_mc!(self, 0x016);
        }
        self.write_operand(self.i.operand1_type, self.i.segment_override, op2_value);
    }

    /// ALU A, imm
    pub fn mc_018(&mut self) {
        let op1_value = self.read_operand(self.i.operand1_type, self.i.segment_override);
        let op2_value = self.read_operand(self.i.operand2_type, self.i.segment_override);
        if let InstructionWidth::Byte = self.i.width {
            self.cycle_i(MC_JUMP); // 0x018 jumps over 2nd queue read
        }
        let result = self.alu_op(self.i.xi.unwrap(), op1_value, op2_value); // 0x01A flagged NXT
        self.set_register_a(result)
    }

    /// MOV r, imm
    /// Opcodes B0-B7
    pub fn mc_01c(&mut self) {
        let op2_value = self.read_operand(self.i.operand2_type, None);
        // W bit is not valid for B0-B7, so we hard-code the operand size here.
        // The microcode uses L8 to skip reading the second byte of an immediate, but its logic
        // is not fully understood.
        if self.i.opcode & 0x08 == 0 {
            self.cycle_i(MC_JUMP); // 0x01c jumps over 2nd queue read
        }
        self.write_operand(self.i.operand1_type, None, op2_value);
    }

    /// INC/DEC rm
    /// Opcodes FE.0, FE.1, FF.0, FF.1
    pub fn mc_020(&mut self) {
        let op_value = self.read_operand(self.i.operand1_type, self.i.segment_override);
        let result = self.alu_op(self.i.xi.unwrap(), op_value, 0);

        self.cycle_i(0x020);
        if self.i.operand1_type.is_address() {
            self.cycle_i(0x021);
        }                           
        self.write_operand(self.i.operand1_type, self.i.segment_override, result);
    }

    /// PUSH rm
    /// Opcodes FF.6, FF.7, *FE.6, *FE.7 (*invalid 8-bit forms)
    pub fn mc_026(&mut self) {
        let mut op_value = self.read_operand(self.i.operand1_type, self.i.segment_override);
        cycles_mc!(self, 0x024, 0x025, 0x026);

        // If SP, push the new value of SP instead of the old value
        if let OperandType::Register16(Register16::SP) = self.i.operand1_type {
            op_value = op_value.wrapping_sub(2);
        }

        // Write operand to stack, as either 8-bit or 16-bit value depending on instruction width.
        self.push_sized(op_value);
    }

    /// PUSH reg
    pub fn mc_028(&mut self) {
        cycles_mc!(self, 0x028, 0x029, 0x02a);
        let reg = REGISTER16_LUT[(self.i.opcode & 0x07) as usize];
        self.push_register16(reg);
    }

    /// PUSH sreg
    pub fn mc_02c(&mut self) {
        cycles_mc!(self, 0x02c, 0x02d, 0x02e);
        let reg = SREGISTER_LUT[((self.i.opcode >> 3) & 0x03) as usize];
        self.push_register16(reg);
    }

    /// PUSHF - Push Flags
    /// Opcode: 9C
    pub fn mc_030(&mut self) {
        cycles!(self, 3);
        self.push_flags();
    }

    /// POP reg
    pub fn mc_034(&mut self) {
        let reg = REGISTER16_LUT[(self.i.opcode & 0x07) as usize];
        self.pop_register16(reg);
    }

    /// POP sreg
    pub fn mc_038(&mut self) {
        let value = self.pop_u16();
        self.write_operand16(self.i.operand1_type, None, value);
    }

    /// POPF - Pop Flags
    /// Opcode: 9D
    pub fn mc_03c(&mut self) {
        self.pop_flags();
    }

    /// POP r/m16
    /// Opcode: 8F
    pub fn mc_040(&mut self) {
        self.cycle_i(0x040);
        let value = self.pop_u16();
        self.cycle_i(0x042);
        if self.i.operand1_type.is_address() {
            cycles_mc!(self, 0x043, 0x044);
        }
        self.write_operand16(self.i.operand1_type, self.i.segment_override, value);
    }

    /// NOT rm
    /// Opcodes: F6.2, F7.2
    pub fn mc_04c(&mut self) {
        let op1_value = self.read_operand(self.i.operand1_type, self.i.segment_override);
        let result = self.alu_op(self.i.xi.unwrap(), op1_value, 0);
        
        if self.i.operand1_type.is_address() {
            cycles_mc!(self, 0x04c, 0x04d);
        }                        
        else {
            // 0x04c is flagged with NXT in published microcode. Test timings indicate maybe this was changed.
            self.cycle_i(0x04c);
        }
        self.write_operand(self.i.operand1_type, self.i.segment_override, result);        
    }
    
    /// NEG rm, imm
    /// Opcodes: F6.3, F7.3
    pub fn mc_050(&mut self) {
        let op1_value = self.read_operand(self.i.operand1_type, self.i.segment_override);
        let result = self.alu_op(self.i.xi.unwrap(), op1_value, 0);

        if self.i.operand1_type.is_address() {
            cycles_mc!(self, 0x050, 0x051);
        }                          
        else {
            // 0x050 is flagged with NXT in published microcode. Test timings indicate maybe this was changed.
            self.cycle_i(0x050);
        }
        self.write_operand(self.i.operand1_type, self.i.segment_override, result);        
    }

    /// CBW - Convert Byte to Word
    /// Opcode: 98
    pub fn mc_054(&mut self) {
        if self.a.l() & 0x80 != 0 {
            self.a.set_h(0xFF);
        }
        else {
            self.a.set_h(0);
        }
    }
    
    /// CWD - Convert Word to Doubleword
    /// Opcode: 99
    pub fn mc_058(&mut self) {
        cycles!(self, 3);
        if self.a.x() & 0x8000 == 0 {
            self.d.set_x(0x0000);
        }
        else {
            self.cycle(); // Microcode jump @ 05a
            self.d.set_x(0xFFFF);
        }
    }
    
    /// MOV A, offset
    /// Opcodes: A0, A1
    pub fn mc_060(&mut self) {         
        // These MOV variants are unique in that they take a direct offset with no modr/m byte
        let op2_value = self.read_operand(self.i.operand2_type, self.i.segment_override);
        self.set_register_a(op2_value);
    }
    
    /// MOV offset, A
    /// Opcodes: A2, A3
    pub fn mc_064(&mut self) {
        let op2_value = self.a.x();
        self.write_operand(self.i.operand1_type, self.i.segment_override, op2_value);
    }

    /// CALL FAR rm FarPtr
    /// Opcodes: FF.3, *FE.3 (*invalid 8-bit form)
    pub fn mc_068(&mut self) {

        if let OperandType::AddressingMode(_mode, _) = self.i.operand1_type {
            self.cycle_i(0x068);
            let (segment, offset) = self.read_operand_farptr(self.i.operand1_type, self.i.segment_override).unwrap();
            let next_i = self.ip();
            let next_cs = self.cs;

            self.farcall(segment, offset, true);

            // Save next address if we step over this CALL.
            self.step_over_target = Some(CpuAddress::Segmented(self.cs, next_i));

            // Add to call stack
            self.push_call_stack(
                CallStackEntry::CallF {
                    ret_cs: next_cs,
                    ip: self.instruction_ip,
                    ret_ip: next_i,
                    call_cs: segment,
                    call_ip: offset
                },
                next_cs,
                next_i
            );
        }
        else if let OperandType::Register16(_) = self.i.operand1_type {
            // Register form is invalid (can't use arbitrary modrm register as a pointer)
            let seg = self.i.segment_override.unwrap_or(Segment::DS);

            // Spend a cycle if reading register operand
            if self.i.operand1_type.is_register() {
                self.cycle_i(0x069);
            }

            // EALOAD sets tmpa into IND, but on a register operand, IND is never set.
            // Even worse, the microcode goes on to reference tmpb, which is uninitialized.
            // There is no way to properly emulate this, unless you basically track every place where
            // tmpb is changed.
            // At that point, you might as well make a microcode emulator.

            // Some random value seen in tests. Don't copy this.
            let offset = 0x0004;
            let segment = self.biu_read_u16(seg, offset);

            self.cycle_i(0x06a);
            self.biu_fetch_suspend();
            cycles_mc!(self, 0x06b, 0x06c);
            self.corr();

            // Push CS
            self.cycle_i(0x06d);
            self.push_register16(Register16::CS);
            let next_i = self.pc; // PC actually gets value of tmpb, which can't be determined. :(
            self.cs = segment;

            cycles_mc!(self, 0x06e, 0x06f, MC_JUMP);
            self.biu_queue_flush();
            cycles_mc!(self, 0x077, 0x078, 0x079);

            // Push next IP
            self.push_u16(next_i);
        }
        self.jumped = true;
    }
    
    /// CALLF - Call Far addr16:16
    /// Opcode: 9A
    pub fn mc_070(&mut self) {
        // This instruction reads a direct FAR address from the instruction queue. (See 0xEA for its twin JMPF)
        let (segment, offset) = self.read_operand_faraddr();
        self.farcall(segment, offset, true);

        // Save next address if we step over this CALL.
        self.step_over_target = Some(CpuAddress::Segmented(self.cs, self.ip()));

        self.push_call_stack(
            CallStackEntry::CallF {
                ret_cs: self.cs,
                ip: self.instruction_ip,
                ret_ip: self.ip(),
                call_cs: segment,
                call_ip: offset
            },
            self.cs,
            self.ip(),
        );
        self.jumped = true;
    }

    /// CALL rm16*
    /// Opcodes: FF.2 (*and invalid FE.2)
    pub fn mc_074(&mut self) {
        // Note this will read either a memory or register operand.
        let ptr16 = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
        // Spend a cycle if reading register operand
        if self.i.operand1_type.is_register() {
            self.cycle_i(0x074);
        }
        self.biu_fetch_suspend();
        cycles_mc!(self, 0x074, 0x075);
        self.corr();
        self.cycle_i(0x076);

        // Save next address if we step over this CALL.
        self.step_over_target = Some(CpuAddress::Segmented(self.cs, self.pc));

        let return_ip = self.pc;

        // Add to call stack
        self.push_call_stack(
            CallStackEntry::Call {
                cs: self.cs,
                ip: self.instruction_ip,
                ret_ip: self.pc,
                call_ip: ptr16
            },
            self.cs,
            return_ip
        );

        self.pc = ptr16;
        self.biu_queue_flush();
        cycles_mc!(self, 0x077, 0x078, 0x079);

        // Push return address (next instruction offset) onto stack, size-aware
        self.push_sized(return_ip);
    }
    
    /// CALL rel16
    /// Unique microcode routine. Does not call NEARCALL.
    /// Opcode: E8
    pub fn mc_07c(&mut self) {
        let rel16 = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();

        self.biu_fetch_suspend(); // 0x07E
        cycles_mc!(self, 0x07e, 0x07f);
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
                cs: self.cs,
                ip: self.instruction_ip,
                ret_ip: self.pc,
                call_ip: new_pc
            },
            self.cs,
            self.pc
        );

        // Set new IP
        self.pc = new_pc;
        self.biu_queue_flush();
        cycles_mc!(self, 0x081, 0x082, MC_JUMP); 

        // Push return address
        self.push_u16(ret_addr);
        self.jumped = true;        
    }

    // XCHG r, AX
    // Opcodes 90-97
    pub fn mc_084(&mut self) {
        let op_reg = REGISTER16_LUT[(self.i.opcode & 0x07) as usize];
        let ax_value = self.a.x();
        let op_reg_value = self.get_register16(op_reg);

        self.cycle_i(0x084);

        self.set_register16(Register16::AX, op_reg_value);
        self.set_register16(op_reg, ax_value);
    }
    
    // ROL, ROR, RCL, RCR, SHL, SHR, SAR:  rm, 0x01
    // Opcodes: D0, D1
    pub fn mc_088(&mut self) {
        let op1_value = self.read_operand(self.i.operand1_type, self.i.segment_override);
        let result = self.alu_bitshift_op(self.i.xi.unwrap(), op1_value, 1);
        if self.i.operand1_type.is_address() {
            cycles_mc!(self, 0x088, 0x089);
        }
        self.write_operand(self.i.operand1_type, self.i.segment_override, result);        
    }
    
    /// ROL, ROR, RCL, RCR, SHL, SHR, SAR: rm, cl
    /// Opcodes: D2, D3
    pub fn mc_08c(&mut self) {
        let op1_value = self.read_operand(self.i.operand1_type, self.i.segment_override);
        let op2_value = self.c.l();

        cycles_mc!(self, 0x08c, 0x08d, 0x08e, MC_JUMP, 0x090, 0x091);
        if self.c.l() > 0 {
            for _ in 0..self.c.l() {
                cycles_mc!(self, MC_JUMP, 0x08f, 0x090, 0x091);
            }
        }
        
        // If there is a terminal write to M, don't process RNI on line 0x92
        if self.i.operand1_type.is_address() {
            self.cycle_i(0x092);
        }

        let result = self.alu_bitshift_op(self.i.xi.unwrap(), op1_value, op2_value);
        self.write_operand(self.i.operand1_type, self.i.segment_override, result);        
    }

    /// TEST rm, r
    /// Opcodes 84-85
    /// Flags: o..sz.pc
    pub fn mc_094(&mut self) {
        let op1_value = self.read_operand(self.i.operand1_type, self.i.segment_override);
        let op2_value = self.read_operand(self.i.operand2_type, self.i.segment_override);
        // TEST is not a true ALU operation, so it doesn't use Xi here. It's just a bitwise AND.
        _ = self.alu_op(Xi::AND, op1_value, op2_value);
        self.cycle_i(0x094);
    }

    /// TEST rm, imm
    /// Opcodes 0xF6.0, 0xF7.0
    pub fn mc_098(&mut self) {
        let op1_value = self.read_operand(self.i.operand1_type, self.i.segment_override);
        let op2_value = self.read_operand(self.i.operand2_type, self.i.segment_override);

        if self.i.width == InstructionWidth::Byte {
            self.cycle_i(MC_JUMP); // Skip 2nd byte read from queue
        }
        self.cycle_i(0x09a);

        // Don't use result, just set flags
        let _result = self.alu_op(Xi::AND, op1_value, op2_value);        
    }

    /// TEST al, imm8
    /// Flags: o..sz.pc
    /// Opcodes: A8, A9
    pub fn mc_09c(&mut self) {
        let op1_value = self.a.x();
        let op2_value = self.read_operand(self.i.operand2_type, None);
        if self.i.width == InstructionWidth::Byte {
            self.cycle_i(MC_JUMP); // Skip 2nd byte read from queue
        }
        self.alu_op(Xi::AND, op1_value, op2_value);
    }

    /// SALC - Undocumented Opcode - Set Carry flag in AL
    /// http://www.rcollins.org/secrets/opcodes/SALC.html
    /// Opcode: D6
    pub fn mc_0a0(&mut self) {
        self.cycle_i(0x0a0);
        match self.get_flag(Flag::Carry) {
            true => {
                self.cycle_i(MC_JUMP);
                self.set_register8(Register8::AL, 0xFF);
            },
            false => {
                self.set_register8(Register8::AL, 0);
            }
        }
    }

    /// XCHG rm, r
    /// Opcodes 86-87
    pub fn mc_0a4(&mut self) {
        let op1_value = self.read_operand(self.i.operand1_type, self.i.segment_override);
        let op2_value = self.read_operand(self.i.operand2_type, self.i.segment_override);
        cycles_mc!(self, 0x0a4, 0x0a5);
        if self.i.operand2_type.is_address() {
            // Memory operand takes 2 more cycles
            cycles_mc!(self, 0x0a6, 0x0a7);
        }
        // Exchange values. Write operand2 first so we don't affect EA calculation if EA includes register being swapped.
        self.write_operand(self.i.operand2_type, self.i.segment_override, op1_value);
        self.write_operand(self.i.operand1_type, self.i.segment_override, op2_value);
    }

    // IN A, imm8
    pub fn mc_0ac(&mut self) {
        let op2_value = self.read_operand8(self.i.operand2_type, self.i.segment_override).unwrap();
        self.cycle_i(0x0ad);

        let in_value = match self.i.width {
            InstructionWidth::Byte => {
                self.biu_io_read_u8(op2_value as u16) as u16
            }
            InstructionWidth::Word => {
                self.biu_io_read_u16(op2_value as u16)
            }
        };
        
        if self.io_flags[op2_value as usize] & IO_READ_BREAKPOINT != 0 {
            self.set_breakpoint_flag();
        }
        self.set_register_a(in_value);        
    }

    // OUT imm8, A
    pub fn mc_0b0(&mut self) {
        let op1_value = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
        let op2_value = self.read_operand(self.i.operand2_type, self.i.segment_override);
        cycles_mc!(self, 0x0b1, 0x0b2);

        // Write to port
        match self.i.width {
            InstructionWidth::Byte => {
                self.biu_io_write_u8(op1_value as u16, op2_value as u8);
            }
            InstructionWidth::Word => {
                // Write to consecutive ports
                self.biu_io_write_u16(op1_value as u16, op2_value);
            }
        };

        if self.io_flags[op1_value as usize] & IO_WRITE_BREAKPOINT != 0 {
            self.set_breakpoint_flag();
        }
    }
    
    // IN A, dx
    pub fn mc_0b4(&mut self) {
        let address = self.d.x();
        let in_value = match self.i.width {
            InstructionWidth::Byte => {
                self.biu_io_read_u8(address) as u16
            }
            InstructionWidth::Word => {
                self.biu_io_read_u16(address)
            }
        };

        if self.io_flags[address as usize] & IO_READ_BREAKPOINT != 0 {
            self.set_breakpoint_flag();
        }
        
        self.set_register_a(in_value);        
    }
    
    // OUT dx, A
    pub fn mc_0b8(&mut self) {
        let address = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();
        let op2_value = self.read_operand(self.i.operand2_type, self.i.segment_override);                
        self.cycle_i(0x0b8);

        match self.i.width {
            InstructionWidth::Byte => {
                self.biu_io_write_u8(address, op2_value as u8);
            }
            InstructionWidth::Word => {
                self.biu_io_write_u16(address, op2_value);
            }
        };

        if self.io_flags[address as usize] & IO_WRITE_BREAKPOINT != 0 {
            self.set_breakpoint_flag();
        }          
    }

    /// RETN - Return from call
    /// 0xC1 undocumented alias for 0xC3
    /// Opcodes C1, C3
    pub fn mc_0bc(&mut self) {
        let new_pc = self.pop_u16();
        self.pc = new_pc;
        self.biu_fetch_suspend();
        self.cycle_i(0x0bd);
        self.biu_queue_flush();
        cycles_mc!(self, 0x0be, 0x0bf);
        self.jumped = true
    }

    /// RETF - Far Return
    /// Opcodes: C9, CB
    pub fn mc_0c0(&mut self) {
        self.cycle_i(0x0c0);
        self.farret(true);
        self.jumped = true;
    }

    /// IRET
    /// Return from interrupt
    /// Opcode: CF
    pub fn mc_0c8(&mut self) {
        self.cycle_i(0x0c8);
        self.farret(true);
        self.pop_flags();
        self.cycle_i(0x0ca);
        self.jumped = true;
    }

    /// RETN/RETF imm16 - Return from call w/ release
    /// 0xC0 undocumented alias for 0xC2
    /// Opcodes C0, C2, C8, CA
    pub fn mc_0cc(&mut self) {
        let stack_disp = self.read_operand16(self.i.operand1_type, None).unwrap();
        // Far jump based on opcode
        self.farret(self.i.opcode & 0x08 != 0);
        self.cycle_i(0x0ce);
        self.release(stack_disp);
        self.jumped = true
    }

    /// JMP rel
    /// Opcodes E9, EB
    pub fn mc_0d0(&mut self) {
        let rel = self.read_operand(self.i.operand1_type, self.i.segment_override);
        if self.i.width == InstructionWidth::Byte {
            self.reljmp2(rel as i8 as i16, true); // We jump directly into reljmp
        }
        else {
            self.reljmp2(rel as i16, false); // Fall through to reljmp after 2nd queue read
        }
        self.jumped = true;
    }

    /// JUMP rm
    /// Opcodes FF.4, *FE.4 (*invalid 8-bit form)
    pub fn mc_0d8(&mut self) {
        // Explicitly use 16-bit read here to handle 8-bit operands as wide
        let ptr16 = self.read_operand16(self.i.operand1_type, self.i.segment_override).unwrap();

        if self.i.operand1_type.is_register() {
            self.cycle();
        }

        self.biu_fetch_suspend();
        self.cycle_i(0x0d8);
        self.pc = ptr16;
        self.biu_queue_flush();
        self.jumped = true;
    }

    /// JMP FAR rm FarPtr
    /// Opcodes FF.5, *FE.5 (*invalid 8-bit form)
    pub fn mc_0dc(&mut self) {
        let offset;

        if let OperandType::AddressingMode(_mode, _) = self.i.operand1_type {
            
            self.cycle_i(0x0dc);
            self.biu_fetch_suspend();
            self.cycle_i(0x0dd);

            let (segment, offset) = self.read_operand_farptr(self.i.operand1_type, self.i.segment_override).unwrap();

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
            let segment = self.biu_read_u16(seg, offset);

            self.cs = segment;
            self.biu_queue_flush();
        }
        self.jumped = true;        
    }


    /// JMP FAR [addr16:16]
    /// This instruction reads a direct FAR address from the instruction queue. (See 0x9A for its twin CALLF)
    /// Opcode: EA
    pub fn mc_0e0(&mut self) {
        let (segment, offset) = self.read_operand_faraddr();
        self.biu_fetch_suspend();
        cycles_mc!(self, 0x0e4, 0x0e5);
        self.cs = segment;
        self.pc = offset;
        self.biu_queue_flush();
        self.cycle_i(0x0e6); // Doesn't hurt to run this RNI as we have to re-fill queue
        self.jumped = true;
    }

    /// JMP rel8
    /// Opcodes 60-7F
    pub fn mc_0e8(&mut self) {
        self.jumped = match self.i.opcode & 0x0F {
            0x00 =>  self.get_flag(Flag::Overflow),  // JO - Jump if overflow set
            0x01 => !self.get_flag(Flag::Overflow),  // JNO - Jump it overflow not set
            0x02 =>  self.get_flag(Flag::Carry),     // JB -> Jump if carry set
            0x03 => !self.get_flag(Flag::Carry),     // JNB -> Jump if carry not set
            0x04 =>  self.get_flag(Flag::Zero),      // JZ -> Jump if Zero set
            0x05 => !self.get_flag(Flag::Zero),      // JNZ -> Jump if Zero not set
            0x06 =>  self.get_flag(Flag::Carry) || self.get_flag(Flag::Zero),    // JBE -> Jump if Carry OR Zero
            0x07 => !self.get_flag(Flag::Carry) && !self.get_flag(Flag::Zero),   // JNBE -> Jump if Carry not set AND Zero not set
            0x08 =>  self.get_flag(Flag::Sign),                                  // JS -> Jump if Sign set
            0x09 => !self.get_flag(Flag::Sign),                                  // JNS -> Jump if Sign not set
            0x0A =>  self.get_flag(Flag::Parity),                                // JP -> Jump if Parity set
            0x0B => !self.get_flag(Flag::Parity),                                // JNP -> Jump if Parity not set
            0x0C =>  self.get_flag(Flag::Sign) != self.get_flag(Flag::Overflow), // JL -> Jump if Sign flag != Overflow flag
            0x0D =>  self.get_flag(Flag::Sign) == self.get_flag(Flag::Overflow), // JNL -> Jump if Sign flag == Overflow flag
            0x0E =>  self.get_flag(Flag::Zero) || (self.get_flag(Flag::Sign) != self.get_flag(Flag::Overflow)), // JLE ((ZF=1) OR (SF!=OF))
            0x0F => !self.get_flag(Flag::Zero) && (self.get_flag(Flag::Sign) == self.get_flag(Flag::Overflow)), // JNLE ((ZF=0) AND (SF=OF))
            _ => false,
        };

        let rel8 = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
        self.cycle_i(0x0e9);
        if self.jumped {
            self.reljmp2(rel8 as i8 as i16, true);
        }
    }

    /// MOV sreg<->rm
    /// Opcodes 8C, 8E
    pub fn mc_0ec(&mut self) {
        if self.i.operand1_type.is_address() {
            self.cycle_i(0x0ec);
        }
        let op_value = self.read_operand(self.i.operand2_type, self.i.segment_override);
        self.write_operand(self.i.operand1_type, self.i.segment_override, op_value);
    }

    /// LES - Load ES from Pointer
    /// Opcode: C4
    pub fn mc_0f0(&mut self) {
        cycles_mc!(self, 0x0F0, 0x0F1);
        // Operand 2 is far pointer
        let (les_segment, les_offset) =
            self.read_operand_farptr(self.i.operand2_type, self.i.segment_override).unwrap();
        //log::trace!("LES instruction: Loaded {:04X}:{:04X}", les_segment, les_offset);
        self.write_operand16(self.i.operand1_type, self.i.segment_override, les_offset);
        self.es = les_segment;
    }

    /// LDS - Load DS from Pointer
    /// Opcode: C5
    pub fn mc_0f4(&mut self) {
        cycles_mc!(self, 0x0F4, 0x0F5);

        // Operand 2 is far pointer
        let (lds_segment, lds_offset) =
            self.read_operand_farptr(self.i.operand2_type, self.i.segment_override).unwrap();
        //log::trace!("LDS instruction: Loaded {:04X}:{:04X}", lds_segment, lds_offset);
        self.write_operand16(self.i.operand1_type, self.i.segment_override, lds_offset);
        self.ds = lds_segment;
    }
    
    /// WAIT
    /// Opcode: 9B
    pub fn mc_0f8(&mut self) {}
    
    /// SAHF - Store AH into Flags
    /// Opcode: 9E
    pub fn mc_100(&mut self) {
        cycles_mc!(self, 0x100, 0x101);
        self.store_flags(self.a.h() as u16);
    }
    
    /// LAHF - Load Status Flags into AH Register
    /// Opcode: 9F
    pub fn mc_104(&mut self) {
        let flags = self.load_flags() as u8;
        self.set_register8(Register8::AH, flags);
    }
    
    /// ESC - FPU instructions.
    /// Opcodes: D8-DF 
    pub fn mc_108(&mut self) {
        // Perform dummy read if memory operand
        let _op1_value = self.read_operand16(self.i.operand1_type, self.i.segment_override);        
    }
    
    /// XLAT
    /// Opcode: D7
    pub fn mc_10c(&mut self) {
        let segment = self.i.segment_override.unwrap_or(Segment::DS);
        let disp16: u16 = self.b.x().wrapping_add(self.a.l() as u16);
        
        cycles_mc!(self, 0x10c, 0x10d, 0x10e);

        let value = self.biu_read_u8(segment, disp16);
        
        self.set_register8(Register8::AL, value);        
    }

    // STOSB & STOSW
    // Segment override: DS overridable
    // Flags: None
    // Opcodes AA, AB
    pub fn mc_11c(&mut self) {
        if self.rep_start() {
            self.string_op(self.i.mnemonic, None);
            cycles_mc!(self, 0x11d, 0x11e);

            // Check for end condition (CX==0)
            if self.in_rep {
                // Check for interrupt
                self.cycle_i(0x11f);
                if self.intr_pending || self.get_flag(Flag::Trap) {
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
    
    /// CMPSB, CMPSW, SCASB, SCASW
    /// Segment override: DS overridable
    /// Flags: All
    /// Opcodes A6, A7, AE, AF
    pub fn mc_120(&mut self) {
        if self.rep_start() {
            self.string_op(self.i.mnemonic, self.i.segment_override);
            if self.in_rep {
                let mut end = false;
                // Check for REP end condition #1 (Z/NZ)
                self.cycle_i(0x129);
                self.decrement_register16(Register16::CX); // 129

                match self.rep_type {
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
                    if self.intr_pending || self.get_flag(Flag::Trap) {
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
    
    /// MOVSB & MOVSW
    /// Segment override: DS overridable
    /// Opcodes A4, A5
    pub fn mc_12c(&mut self) {
        if self.rep_start() {
            self.string_op(self.i.mnemonic, self.i.segment_override);
            cycles_mc!(self, 0x12f, 0x130);

            // Check for end condition (CX==0)
            if self.in_rep {
                self.decrement_register16(Register16::CX); // 131
                // Check for interrupt
                if self.intr_pending || self.get_flag(Flag::Trap) {
                    cycles_mc!(self, 0x131, MC_JUMP); // Jump to RPTI
                    self.rep_interrupt();
                }
                else {
                    cycles_mc!(self, 0x131, 0x132);
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

    /// LODSB & LODSW
    /// Segment override: DS overridable
    /// Flags: None
    /// Although LODSx is not typically used with a REP prefix, it can be
    /// Opcodes AC, AD
    pub fn mc_12cb(&mut self) {
        if self.rep_start() {
            self.string_op(self.i.mnemonic, self.i.segment_override);
            cycles_mc!(self, 0x12e, MC_JUMP, 0x1f8);
            // Check for REP end condition #1 (CX==0)
            if self.in_rep {
                cycles_mc!(self, MC_JUMP, 0x131); // Jump to 131
                self.decrement_register16(Register16::CX); // 131
                // Check for interrupt
                if self.intr_pending || self.get_flag(Flag::Trap) {
                    self.cycle_i(MC_JUMP); // Jump to RPTI
                    self.rep_interrupt();
                }
                else {
                    self.cycle_i(0x132);
                    if self.c.x() == 0 {
                        // Fall through to 133/1f9, RNI
                        self.rep_end();
                    }
                    else {
                        self.cycle_i(MC_JUMP); // jump to 1
                    }
                }
            }
            // Non-prefixed LODSx ends with RNI
        }
    }
    
    /// JCXZ - Jump if CX == 0
    /// Opcode: E3
    pub fn mc_134(&mut self) {
        cycles_mc!(self, 0x134, 0x135);
        let rel8 = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();
        if self.c.x() != 0 {
            self.cycle_i(MC_JUMP);
        }
        else {
            self.cycle_i(0x137);
            self.reljmp2(rel8 as i8 as i16, true);
            self.jumped = true;
        }        
    }
    
    /// LOOPNE & LOOPE
    /// LOOPNE - Decrement CX, Jump short if count !=0 and ZF=0
    /// LOOPE  - Jump short if count !=0 and ZF=1    
    /// Opcodes: E0, E1
    pub fn mc_138(&mut self) {
        self.decrement_register16(Register16::CX);
        cycles_mc!(self, 0x138, 0x139);

        let ne_flag = self.i.opcode & 0x01 == 1;
        let rel8 = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();

        if ne_flag != self.get_flag(Flag::Zero) {
            self.cycle_i(MC_JUMP);
        }
        else if self.c.x() != 0 {
            self.cycle_i(0x13b);
            self.reljmp2(rel8 as i8 as i16, true);
            self.jumped = true;
        }        
    }
    
    /// LOOP - Jump short if count != 0
    /// Opcode: E2 
    pub fn mc_140(&mut self) {
        self.decrement_register16(Register16::CX);
        cycles_mc!(self, 0x140, 0x141);

        let rel8 = self.read_operand8(self.i.operand1_type, self.i.segment_override).unwrap();

        if self.c.x() != 0 {
            self.reljmp2(rel8 as i8 as i16, true);
            self.jumped = true;
        }
        if !self.jumped {
            self.cycle();
        }        
    }

    /// DAA — Decimal Adjust AL after Addition
    /// DAS - Decimal Adjust AL after Subtraction
    pub fn mc_144(&mut self) {
        cycles_mc!(self, 0x144, 0x145);
        match self.i.xi {
            Some(Xi::DAA) => self.daa(),
            Some(Xi::DAS) => self.das(),
            _ => {}
        }
    }

    /// AAA — ASCII Adjust AL after Addition
    /// AAS — ASCII Adjust AL after Subtraction
    pub fn mc_148(&mut self) {
        match self.i.xi {
            Some(Xi::AAA) => self.aaa(),
            Some(Xi::AAS) => self.aas(),
            _ => {}
        }
    }
    
    /// MUL rm8, imm
    /// Opcodes F6.4, F6.5
    pub fn mc_150(&mut self) {
        let op1_value = self.read_operand(self.i.operand1_type, self.i.segment_override);

        // Negate product if a REP prefix is present
        let negate = (self.i.prefixes & (OPCODE_PREFIX_REP1 | OPCODE_PREFIX_REP2)) != 0;

        // Control of signed/unsigned multiplication is determined by the X0 operator, but I'm
        // not quite sure how it works.
        let signed = self.i.mnemonic == Mnemonic::IMUL;
        let ax = self.mul8(self.a.l(), op1_value as u8, signed, negate);
        self.set_register16(Register16::AX, ax);
        if !signed {
            self.set_szp_flags_from_result_u8(self.a.h());
            self.clear_flag(Flag::AuxCarry);
        }

        if self.i.operand1_type.is_register() {
            self.cycle();
        }
    }
    
    /// MUL rm16 imm
    /// Opcodes F7.4, F7.5
    pub fn mc_158(&mut self) {
        let op1_value = self.read_operand(self.i.operand1_type, self.i.segment_override);

        // Negate product if a REP prefix is present
        let negate = (self.i.prefixes & (OPCODE_PREFIX_REP1 | OPCODE_PREFIX_REP2)) != 0;

        // Control of signed/unsigned multiplication is determined by the X0 operator, but I'm
        // not quite sure how it works.
        let signed = self.i.mnemonic == Mnemonic::IMUL;
        let (dx, ax) = self.mul16(self.a.x(), op1_value, signed, negate);
        self.set_register16(Register16::AX, ax);
        self.set_register16(Register16::DX, dx);
        if !signed {
            self.set_szp_flags_from_result_u16(self.d.x());
            self.clear_flag(Flag::AuxCarry);
        }

        if self.i.operand1_type.is_register() {
            self.cycle();
        }
    }

    /// DIV & IDIV rm8
    /// Opcodes F6.6, F6.7
    pub fn mc_160(&mut self) {
        let op1_value = self.read_operand(self.i.operand1_type, self.i.segment_override);

        // Negate quotient if a REP prefix is present
        let negate = (self.i.prefixes & (OPCODE_PREFIX_REP1 | OPCODE_PREFIX_REP2)) != 0;
        // Control of signed/unsigned division is determined by the X0 operator, but I'm
        // not quite sure how it works.
        let signed = self.i.mnemonic == Mnemonic::IDIV;

        // Extra cycle for register operand for some reason
        if self.i.operand1_type.is_register() {
            self.cycle();
        }

        match self.div8(self.a.x(), op1_value as u8, signed, negate) {
            Ok((al, ah)) => {
                self.set_register8(Register8::AL, al); // Quotient in AL
                self.set_register8(Register8::AH, ah); // Remainder in AH
            }
            Err(_) => {
                self.int0();
                self.exception = CpuException::DivideError;
            }
        }
    }
    
    /// DIV & IDIV rm16
    /// Opcodes F7.6, F7.7
    pub fn mc_168(&mut self) {
        let op1_value = self.read_operand(self.i.operand1_type, self.i.segment_override);

        // Negate quotient if a REP prefix is present
        let negate = (self.i.prefixes & (OPCODE_PREFIX_REP1 | OPCODE_PREFIX_REP2)) != 0;
        // Control of signed/unsigned division is determined by the X0 operator, but I'm
        // not quite sure how it works.
        let signed = self.i.mnemonic == Mnemonic::IDIV;

        // Extra cycle for register operand for some reason
        if self.i.operand1_type.is_register() {
            self.cycle();
        }

        match self.div16(((self.d.x() as u32) << 16 ) | (self.a.x() as u32), op1_value, signed, negate) {
            Ok((quotient, remainder)) => {
                self.set_register16(Register16::AX, quotient); // Quotient in AX
                self.set_register16(Register16::DX, remainder); // Remainder in DX
            }
            Err(_) => {
                self.int0();
                self.exception = CpuException::DivideError;
            }
        }
    }

    /// AAD - Ascii Adjust before Division
    /// Opcode: D5
    pub fn mc_170(&mut self) {
        self.i.width = InstructionWidth::Byte;
        let imm8 = self.read_operand8(self.i.operand1_type, None).unwrap();
        cycles_mc!(self, 0x170, 0x171, MC_JUMP);
        let (_, product) = 0u8.corx(self, self.a.h() as u16, imm8 as u16, false);
        let tmpc = self.alu_op(Xi::ADD, self.a.l() as u16, product);
        self.set_register8(Register8::AL, tmpc as u8);
        self.set_register8(Register8::AH, 0);

        cycles_mc!(self, 0x172, 0x173);

        // Other sources set flags from AX register. Intel's documentation specifies AL
        self.set_szp_flags_from_result_u8(self.a.l());
    }

    /// AAM - Ascii adjust AX after Multiply
    /// Opcode: D4
    pub fn mc_174(&mut self) {
        let imm = self.read_operand(self.i.operand1_type, None);
        cycles_mc!(self, 0x175, 0x176, MC_JUMP);
        match 0u8.cord(self, 0, imm, self.a.l() as u16) {
            Ok((quotient, remainder, _)) => {
                // 177:          | COM1 tmpc
                self.set_register8(Register8::AH, !(quotient as u8));
                self.set_register8(Register8::AL, remainder as u8);
                self.cycle_i(0x177);
                // Other sources set flags from AX register. Intel's documentation specifies AL
                self.set_szp_flags_from_result_u8(self.a.l());
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::AuxCarry);
                self.clear_flag(Flag::Overflow);
            }
            Err(_) => {
                self.set_szp_flags_from_result_u8(0);
                self.clear_flag(Flag::AuxCarry);
                self.clear_flag(Flag::Carry);
                self.clear_flag(Flag::Overflow);
                // Divide exception
                self.int0();
                self.jumped = true;
                self.exception = CpuException::DivideError;
            }
        }
    }

    /// INC & DEC reg
    pub fn mc_17c(&mut self) {
        let op1_value = self
            .read_operand16(self.i.operand1_type, self.i.segment_override)
            .unwrap();
        let result = self.alu_op(self.i.xi.unwrap(), op1_value, 0);
        self.write_operand16(self.i.operand1_type, self.i.segment_override, result);
    }

    /// INT imm8 - Software Interrupt
    /// The Interrupt flag does not affect the handling of non-maskable interrupts (NMIs) or software interrupts
    /// generated by the INT instruction.
    /// Opcode: CD
    pub fn mc_1a8(&mut self) {
        // Get interrupt number (immediate operand)
        let irq = self.read_operand8(self.i.operand1_type, None).unwrap();
        // Save next address if we step over this INT.
        self.step_over_target = Some(CpuAddress::Segmented(self.cs, self.ip()));

        // Another cycle deviance here between observed timings and microcode. Skipping this jump to align with
        // tests.
        //self.cycle_i(MC_JUMP); // Jump to INTR
        self.sw_interrupt(irq);
        self.jumped = true;
    }

    /// INTO - Call Overflow Interrupt Handler
    /// Opcode: CE
    pub fn mc_1ac(&mut self) {
        if self.get_flag(Flag::Overflow) {
            cycles_mc!(self, 0x1ac, 0x1ad, MC_JUMP, 0x1af);

            // Save next address if we step over this INT.
            self.step_over_target = Some(CpuAddress::Segmented(self.cs, self.ip()));
            self.sw_interrupt(4);
            self.jumped = true;
        }
        else {
            // Overflow not set.
            cycles_mc!(self, 0x1ac, 0x1ad);
        }
    }

    /// INT 3 - Software Interrupt 3
    /// This is a special form of INT which assumes IRQ 3 always. Most assemblers will not generate this form
    /// Opcode: CC
    pub fn mc_1b0(&mut self) {
        self.step_over_target = Some(CpuAddress::Segmented(self.cs, self.ip()));
        self.int3();
        self.jumped = true;
    }
}

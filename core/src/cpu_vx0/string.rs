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

    cpu_vx0::string.rs

    Implements string operations.

*/

use crate::{
    cpu_common::{alu::AluSub, Mnemonic, Segment},
    cpu_vx0::*,
};

impl NecVx0 {
    pub fn string_op(&mut self, opcode: Mnemonic, segment_override: Option<Segment>) {
        let segment_base_ds = segment_override.unwrap_or(Segment::DS);

        match opcode {
            Mnemonic::INSB => {
                let io_value = self.biu_io_read_u8(self.get_register16(Register16::DX));
                self.biu_write_u8(Segment::ES, self.di, io_value, ReadWriteFlag::Normal);

                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.di = self.di.wrapping_add(1);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.di = self.di.wrapping_sub(1);
                    }
                }
            }
            Mnemonic::INSW => {
                let io_value = self.biu_io_read_u16(self.get_register16(Register16::DX));
                self.biu_write_u16(Segment::ES, self.di, io_value, ReadWriteFlag::Normal);

                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.di = self.di.wrapping_add(2);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.di = self.di.wrapping_sub(2);
                    }
                }
            }
            Mnemonic::OUTSW => {
                let mem_value = self.biu_read_u16(Segment::ES, self.di, ReadWriteFlag::Normal);
                self.biu_io_write_u16(self.get_register16(Register16::DX), mem_value, ReadWriteFlag::Normal);

                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.di = self.di.wrapping_add(2);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.di = self.di.wrapping_sub(2);
                    }
                }
            }
            Mnemonic::OUTSB => {
                let mem_value = self.biu_read_u8(Segment::ES, self.di);
                self.biu_io_write_u8(self.get_register16(Register16::DX), mem_value, ReadWriteFlag::Normal);

                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.di = self.di.wrapping_add(1);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.di = self.di.wrapping_sub(1);
                    }
                }
            }
            Mnemonic::STOSB => {
                // STOSB - Write AL to [es:di]  (ES prefix cannot be overridden)
                // No flags affected

                // Write AL to [es:di]
                self.biu_write_u8(Segment::ES, self.di, self.a.l(), ReadWriteFlag::Normal);

                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.di = self.di.wrapping_add(1);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.di = self.di.wrapping_sub(1);
                    }
                }
            }
            Mnemonic::STOSW => {
                // STOSW - Write AX to [es:di] (ES prefix cannot be overridden)
                // No flags affected

                // Write AX to [es:di]
                self.biu_write_u16(Segment::ES, self.di, self.a.x(), ReadWriteFlag::Normal);

                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.di = self.di.wrapping_add(2);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.di = self.di.wrapping_sub(2);
                    }
                }
            }
            Mnemonic::LODSB => {
                // LODSB affects no flags
                // Store byte [ds:si] in AL   (Segment overrideable)

                let data = self.biu_read_u8(segment_base_ds, self.si);

                self.set_register8(Register8::AL, data);

                // Increment or Decrement SI according to Direction flag
                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.si = self.si.wrapping_add(1);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.si = self.si.wrapping_sub(1);
                    }
                }
            }
            Mnemonic::LODSW => {
                // LODSW affects no flags
                // Store word [ds:si] in AX   (Segment overrideable)
                let data = self.biu_read_u16(segment_base_ds, self.si, ReadWriteFlag::Normal);

                self.set_register16(Register16::AX, data);

                // Increment or Decrement SI according to Direction flag
                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.si = self.si.wrapping_add(2);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.si = self.si.wrapping_sub(2);
                    }
                }
            }
            Mnemonic::MOVSB => {
                // Store byte from [ds:si] in [es:di]  (DS Segment overrideable)

                let data = self.biu_read_u8(segment_base_ds, self.si);
                self.cycle_i(0x12e);
                self.biu_write_u8(Segment::ES, self.di, data, ReadWriteFlag::Normal);

                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.si = self.si.wrapping_add(1);
                        self.di = self.di.wrapping_add(1);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.si = self.si.wrapping_sub(1);
                        self.di = self.di.wrapping_sub(1);
                    }
                }
            }
            Mnemonic::MOVSW => {
                // Store word from [ds:si] in [es:di] (DS Segment overrideable)

                let data = self.biu_read_u16(segment_base_ds, self.si, ReadWriteFlag::Normal);
                self.cycle_i(0x12e);
                self.biu_write_u16(Segment::ES, self.di, data, ReadWriteFlag::Normal);

                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.si = self.si.wrapping_add(2);
                        self.di = self.di.wrapping_add(2);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.si = self.si.wrapping_sub(2);
                        self.di = self.di.wrapping_sub(2);
                    }
                }
            }
            Mnemonic::SCASB => {
                // SCASB: Compare byte from [es:di] with value in AL.
                // Flags: o..szapc
                // Override: ES cannot be overridden

                self.cycles_i(2, &[0x121, MC_JUMP]);
                let data = self.biu_read_u8(Segment::ES, self.di);
                self.cycles_i(3, &[0x126, 0x127, 0x128]);

                let (result, carry, overflow, aux_carry) = self.a.l().alu_sub(data);
                // Test operation behaves like CMP
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u8(result);

                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.di = self.di.wrapping_add(1);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.di = self.di.wrapping_sub(1);
                    }
                }
            }
            Mnemonic::SCASW => {
                // SCASW: Compare word from [es:di] with value in AX.
                // Flags: o..szapc
                // Override: ES cannot be overridden

                self.cycles_i(2, &[0x121, MC_JUMP]);
                let data = self.biu_read_u16(Segment::ES, self.di, ReadWriteFlag::Normal);
                self.cycles_i(3, &[0x126, 0x127, 0x128]);

                let (result, carry, overflow, aux_carry) = self.a.x().alu_sub(data);
                // Test operation behaves like CMP
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u16(result);

                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.di = self.di.wrapping_add(2);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.di = self.di.wrapping_sub(2);
                    }
                }
            }
            Mnemonic::CMPSB => {
                // CMPSB: Compare bytes from [es:di] to [ds:si]
                // Flags: The CF, OF, SF, ZF, AF, and PF flags are set according to the temporary result of the comparison.
                // Override: DS can be overridden

                self.cycle_i(0x121);
                let dssi_op = self.biu_read_u8(segment_base_ds, self.si);
                self.cycles_i(2, &[0x123, 0x124]);
                let esdi_op = self.biu_read_u8(Segment::ES, self.di);
                self.cycles_i(3, &[0x126, 0x127, 0x128]);

                let (result, carry, overflow, aux_carry) = dssi_op.alu_sub(esdi_op);

                // Test operation behaves like CMP
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u8(result);

                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.si = self.si.wrapping_add(1);
                        self.di = self.di.wrapping_add(1);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.si = self.si.wrapping_sub(1);
                        self.di = self.di.wrapping_sub(1);
                    }
                }
            }
            Mnemonic::CMPSW => {
                // CMPSW: Compare words from [es:di] to [ds:si]
                // Flags: The CF, OF, SF, ZF, AF, and PF flags are set according to the temporary result of the comparison.
                // Override: DS can be overridden

                self.cycle_i(0x121);
                let dssi_op = self.biu_read_u16(segment_base_ds, self.si, ReadWriteFlag::Normal);
                self.cycles_i(2, &[0x123, 0x124]);
                let esdi_op = self.biu_read_u16(Segment::ES, self.di, ReadWriteFlag::Normal);
                self.cycles_i(3, &[0x126, 0x127, 0x128]);

                let (result, carry, overflow, aux_carry) = dssi_op.alu_sub(esdi_op);

                // Test operation behaves like CMP
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_szp_flags_from_result_u16(result);

                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.si = self.si.wrapping_add(2);
                        self.di = self.di.wrapping_add(2);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.si = self.si.wrapping_sub(2);
                        self.di = self.di.wrapping_sub(2);
                    }
                }
            }
            _ => {
                panic!("CPU: Unhandled opcode to string_op(): {:?}", opcode);
            }
        }
    }

    /// Implement the RPTS microcode co-routine for string operation repetition.
    pub fn rep_start(&mut self) -> bool {
        if !self.rep_init {
            // First entry into REP-prefixed instruction, run the first line where we
            // decide whether to call RPTS
            match self.i.mnemonic {
                Mnemonic::MOVSB | Mnemonic::MOVSW => self.cycle_i(0x12c),
                Mnemonic::CMPSB | Mnemonic::CMPSW => self.cycle_i(0x120),
                Mnemonic::STOSB | Mnemonic::STOSW => self.cycle_i(0x11c),
                Mnemonic::LODSB | Mnemonic::LODSW => self.cycle_i(0x12c),
                Mnemonic::SCASB | Mnemonic::SCASW => self.cycle_i(0x120),
                _ => {}
            }

            if self.in_rep {
                // Rep-prefixed instruction is starting for the first time. Run the RPTS procedure.
                if self.c.x() == 0 {
                    self.cycles_i(4, &[MC_JUMP, 0x112, 0x113, 0x114]);
                    self.rep_end();
                    return false;
                }
                else {
                    // CX > 0. Load ALU for decrementing CX
                    self.cycles_i(7, &[MC_JUMP, 0x112, 0x113, 0x114, MC_JUMP, 0x116, MC_RTN]);
                }

                // Mark this instruction as reentrant - step will execute a single iteration
                self.instruction_reentrant = true;
            }
        }

        self.rep_init = true;
        true
    }

    pub fn rep_end(&mut self) {
        self.rep_init = false;
        self.in_rep = false;
        self.rep_type = RepType::NoRep;
    }

    /// Implement the RPTI microcode co-routine for string interrupt handling.
    pub fn rep_interrupt(&mut self) {
        self.biu_fetch_suspend();
        self.cycles_i(2, &[0x118, 0x119]);
        self.corr();
        self.cycle_i(0x11a);
        self.biu_queue_flush();

        // Rewind IP so that it points to REP instruction again afterward.
        // This behavior will emulate the 8088's bug with string operations and segment overrides,
        // as the next time the instruction is fetched it will be with only a single prefix.
        self.pc = self.pc.wrapping_sub(2);

        self.rep_end();
        // Flush was on RNI so no extra cycle here
    }
}

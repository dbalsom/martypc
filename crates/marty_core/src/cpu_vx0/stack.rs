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

    cpu_vx0::stack.rs

    Implements stack-oriented routines such as push and pop.

*/

use crate::{
    cpu_common::{Register16_8080, Segment},
    cpu_vx0::{biu::*, *},
};

impl NecVx0 {
    #[inline]
    pub fn push_u8(&mut self, data: u8, flag: ReadWriteFlag) {
        // Stack pointer grows downwards
        self.sp = self.sp.wrapping_sub(2);
        self.biu_write_u8(Segment::SS, self.sp, data, flag);
    }

    #[inline]
    pub fn push_u16(&mut self, data: u16, flag: ReadWriteFlag) {
        // Stack pointer grows downwards
        self.sp = self.sp.wrapping_sub(2);
        self.biu_write_u16(Segment::SS, self.sp, data, flag);
    }

    #[inline]
    pub fn pop_u16(&mut self) -> u16 {
        let result = self.biu_read_u16(Segment::SS, self.sp, ReadWriteFlag::Normal);

        // Stack pointer shrinks upwards
        self.sp = self.sp.wrapping_add(2);
        result
    }

    pub fn push_register16(&mut self, reg: Register16, flag: ReadWriteFlag) {
        // Stack pointer grows downwards
        self.sp = self.sp.wrapping_sub(2);

        let data = match reg {
            Register16::AX => self.a.x(),
            Register16::BX => self.b.x(),
            Register16::CX => self.c.x(),
            Register16::DX => self.d.x(),
            Register16::SP => self.sp,
            Register16::BP => self.bp,
            Register16::SI => self.si,
            Register16::DI => self.di,
            Register16::CS => {
                self.interrupt_inhibit = true;
                self.cs
            }
            Register16::DS => {
                self.interrupt_inhibit = true;
                self.ds
            }
            Register16::SS => {
                self.interrupt_inhibit = true;
                self.ss
            }
            Register16::ES => {
                self.interrupt_inhibit = true;
                self.es
            }
            Register16::PC => self.pc,
            _ => panic!("Invalid register"),
        };

        self.biu_write_u16(Segment::SS, self.sp, data, flag);
    }

    pub fn pop_register16(&mut self, reg: Register16, flag: ReadWriteFlag) {
        let data = self.biu_read_u16(Segment::SS, self.sp, flag);

        let mut update_sp = true;
        match reg {
            Register16::AX => self.set_register16(reg, data),
            Register16::BX => self.set_register16(reg, data),
            Register16::CX => self.set_register16(reg, data),
            Register16::DX => self.set_register16(reg, data),
            Register16::SP => {
                self.sp = data;
                update_sp = false;
            }
            Register16::BP => self.bp = data,
            Register16::SI => self.si = data,
            Register16::DI => self.di = data,
            Register16::CS => {
                self.cs = data;
                self.interrupt_inhibit = true;
            }
            Register16::DS => {
                self.ds = data;
                self.interrupt_inhibit = true;
            }
            Register16::SS => {
                self.ss = data;
                self.interrupt_inhibit = true
            }
            Register16::ES => {
                self.es = data;
                self.interrupt_inhibit = true;
            }
            Register16::PC => self.pc = data,
            _ => panic!("Invalid register"),
        };
        // Stack pointer grows downwards
        if update_sp {
            self.sp = self.sp.wrapping_add(2);
        }
    }

    /// Push a 16-bit register pair to the stack in 8080 emulation mode.
    pub fn push_register16_8080(&mut self, reg: Register16_8080) {
        // Stack pointer grows downwards
        self.bp = self.bp.wrapping_sub(2);

        let data = match reg {
            Register16_8080::HL => self.b.x(),
            Register16_8080::DE => self.d.x(),
            Register16_8080::BC => self.c.x(),
            _ => panic!("Invalid register"),
        };

        self.biu_write_u16(Segment::DS, self.bp, data, ReadWriteFlag::RNI);
    }

    /// Push a 16-bit value to the stack in 8080 emulation mode.
    #[inline]
    pub fn push_u16_8080(&mut self, data: u16) {
        // Stack pointer grows downwards
        self.bp = self.bp.wrapping_sub(2);
        self.biu_write_u16(Segment::DS, self.bp, data, ReadWriteFlag::RNI);
    }

    /// Pop a 16-bit value from the stack in 8080 emulation mode.
    #[inline]
    pub fn pop_u16_8080(&mut self) -> u16 {
        let result = self.biu_read_u16(Segment::DS, self.bp, ReadWriteFlag::Normal);

        // Virtual stack pointer shrinks upwards
        self.bp = self.bp.wrapping_add(2);
        result
    }

    // Pop a 16-bit register pair from the stack in 8080 emulation mode.
    // The 8080 virtual stack pointer is BP, preserving SP for native return.
    #[inline]
    pub fn pop_register16_8080(&mut self, reg: Register16_8080) {
        let data = self.biu_read_u16(Segment::DS, self.bp, ReadWriteFlag::RNI);

        match reg {
            Register16_8080::HL => self.b.set_x(data),
            Register16_8080::DE => self.d.set_x(data),
            Register16_8080::BC => self.c.set_x(data),
            _ => panic!("Invalid register"),
        };
        // Virtual stack pointer grows downwards
        self.bp = self.bp.wrapping_add(2);
    }

    // Pop the 8080 PSW from the stack in 8080 emulation mode.
    // This also pops the accumulator.
    // The 8080 virtual stack pointer is BP, preserving SP for native return.
    pub fn pop_psw_8080(&mut self) {
        let psw = self.biu_read_u8(Segment::DS, self.bp.wrapping_add(1)) as u16;
        let acc = self.biu_read_u8(Segment::DS, self.bp);

        self.set_flag_state(Flag::Carry, psw & CPU_FLAG_CARRY != 0);
        self.set_flag_state(Flag::Parity, psw & CPU_FLAG_PARITY != 0);
        self.set_flag_state(Flag::AuxCarry, psw & CPU_FLAG_AUX_CARRY != 0);
        self.set_flag_state(Flag::Zero, psw & CPU_FLAG_ZERO != 0);
        self.set_flag_state(Flag::Sign, psw & CPU_FLAG_SIGN != 0);
        self.a.set_l(acc);
        // Virtual stack pointer grows downwards
        self.bp = self.bp.wrapping_add(2);
    }

    // Push the 8080 PSW from the stack in 8080 emulation mode.
    // This also pushes the accumulator.
    // The 8080 virtual stack pointer is BP, preserving SP for native return.
    #[inline]
    pub fn push_psw_8080(&mut self) {
        // 8080 PSW is lower byte of flags.
        let word = (self.flags & 0xFF) << 8 | self.a.l() as u16;
        self.bp = self.bp.wrapping_sub(2);
        self.biu_write_u16(Segment::DS, self.bp, word, ReadWriteFlag::RNI);
    }

    #[inline]
    pub fn push_flags(&mut self, wflag: ReadWriteFlag) {
        // Stack pointer grows downwards
        self.sp = self.sp.wrapping_sub(2);
        self.biu_write_u16(Segment::SS, self.sp, self.flags, wflag);
    }

    pub fn pop_flags(&mut self) {
        let result = self.biu_read_u16(Segment::SS, self.sp, ReadWriteFlag::Normal);

        let trap_was_set = self.get_flag(Flag::Trap);
        let int_was_set = self.get_flag(Flag::Interrupt);

        // Ensure state of reserved flag bits
        self.flags = result & FLAGS_POP_MASK;
        self.flags |= CPU_FLAGS_RESERVED_ON;

        // Was interrupt flag just set? Set interrupt inhibit.
        let int_is_set = self.get_flag(Flag::Interrupt);
        if !int_was_set && int_is_set {
            self.interrupt_inhibit = true;
        }

        // Was trap flag just set? Set trap enable delay.
        let trap_is_set = self.get_flag(Flag::Trap);
        if !trap_was_set && trap_is_set {
            self.trap_enable_delay = 1;
        }

        // Was trap flag just disabled? Set trap disable delay.
        if trap_was_set && !trap_is_set {
            self.trap_disable_delay = 1;
        }

        // Stack pointer grows downwards
        self.sp = self.sp.wrapping_add(2);
    }

    pub fn release(&mut self, disp: u16) {
        self.sp = self.sp.wrapping_add(disp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_pop_psw_8080() {}
}

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

    cpu_vx0::jump.rs

    Implements microcode routines for jumps and calls.
*/

use crate::{
    cpu_vx0::{biu::*, *},
    util,
};

impl NecVx0 {
    /*
    /// Execute the RELJMP microcode routine, optionally including the jump into the procedure.
    #[inline]
    pub fn reljmp(&mut self, new_pc: u16, jump: bool) {
        if jump {
            self.cycle_i(MC_JUMP);
        }
        //self.biu_fetch_suspend_i(0x0d2);
        self.biu_fetch_suspend();
        self.cycles_i(4, &[0x0d2, 0x0d3, MC_CORR, 0x0d4]);
        self.pc = new_pc;
        self.biu_queue_flush(); // 0d5
        self.cycle_i(0x0d5);
    }*/

    /// Execute the RELJMP microcode routine, optionally including the jump into the procedure.
    #[inline]
    pub fn reljmp2(&mut self, rel: i16, jump: bool) {
        //TODO: avoid branching. separate functions? make caller handle?
        if jump {
            self.cycle_i(MC_JUMP);
        }
        //self.biu_fetch_suspend_i(0x0d2);
        self.biu_fetch_suspend();
        self.cycles_i(2, &[0x0d2, 0x0d3]);
        self.corr();
        self.pc = util::relative_offset_u16(self.pc, rel);
        self.cycle_i(0x0d4);

        self.biu_queue_flush(); // 0d5
        self.cycle_i(0x0d5);
    }

    /// Execute the FARCALL microcode routine.
    #[inline]
    pub fn farcall(&mut self, new_cs: u16, new_ip: u16, jump: bool) {
        if jump {
            self.cycle_i(MC_JUMP);
        }
        self.biu_fetch_suspend(); // 0x06B
        self.cycles_i(2, &[0x06b, 0x06c]);
        self.corr();
        // Push return segment to stack
        self.push_u16(self.cs, ReadWriteFlag::Normal);
        self.cs = new_cs;
        self.cycles_i(2, &[0x06e, 0x06f]);
        self.nearcall(new_ip);
    }

    /// Execute the FARCALL2 microcode routine. Called by interrupt procedures.
    #[inline]
    pub fn farcall2(&mut self, new_cs: u16, new_ip: u16) {
        self.cycles_i(2, &[MC_JUMP, 0x06c]);
        self.corr();
        // Push return segment to stack
        self.push_u16(self.cs, ReadWriteFlag::Normal);
        self.cs = new_cs;
        self.cycles_i(2, &[0x06e, 0x06f]);
        self.nearcall(new_ip);
    }

    /// Execute the NEARCALL microcode routine.
    #[inline]
    pub fn nearcall(&mut self, new_ip: u16) {
        let ret_ip = self.pc; // NEARCALL assumes that CORR was called prior
        self.cycle_i(MC_JUMP);
        self.pc = new_ip;
        self.biu_queue_flush();
        self.cycles_i(3, &[0x077, 0x078, 0x079]);
        self.push_u16(ret_ip, ReadWriteFlag::RNI);
    }

    /// Execute the FARRET microcode routine, including the jump into the procedure.
    /// FARRET can also perform a near return. Control which type of return is performed
    /// by the far parameter.
    pub fn ret(&mut self, far: bool) {
        self.cycle_i(MC_JUMP);
        //self.pop_register16(Register16::IP, ReadWriteFlag::RNI);
        self.pc = self.pop_u16();
        self.biu_fetch_suspend();
        //self.cycle_i(MC_NONE);
        self.cycles_i(2, &[0x0c3, 0x0c4]);

        let far2 = self.i.opcode & 0x08 != 0;
        assert_eq!(far, far2);

        if far {
            self.cycle_i(MC_JUMP);
            self.pop_register16(Register16::CS, ReadWriteFlag::Normal);

            self.biu_queue_flush();
            self.cycles_i(2, &[0x0c7, MC_RTN]);
        }
        else {
            self.biu_queue_flush();
            self.cycles_i(2, &[0x0c5, MC_RTN]);
        }
    }

    /// Execute a CALL in 8080 emulation mode.
    #[inline]
    pub fn call_8080(&mut self, new_ip: u16) {
        self.biu_fetch_suspend();
        self.cycles(2);
        self.corr();
        let ret_ip = self.pc;
        self.cycle_i(MC_JUMP);
        self.pc = new_ip;
        self.biu_queue_flush();
        self.cycles(3);
        self.push_u16_8080(ret_ip);
    }

    /// Execute a return in 8080 emulation mode.
    #[inline]
    pub fn ret_8080(&mut self) {
        self.cycle_i(MC_JUMP);
        let ret_ip = self.pop_u16_8080();
        self.biu_fetch_suspend();
        self.cycles(2);
        self.biu_queue_flush();
        self.pc = ret_ip;
        self.cycles(2);
    }

    /// Execute PCHL (jump to HL) in 8080 emulation mode.
    #[inline]
    pub fn pchl_8080(&mut self) {
        self.cycle_i(MC_JUMP);
        self.biu_fetch_suspend();
        self.cycles(2);
        self.biu_queue_flush();
        self.pc = self.hl_80();
        self.cycles(2);
    }
}

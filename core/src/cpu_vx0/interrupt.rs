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

    cpu_vx0::interrupt.rs

    Routines to handle interrupts.

*/

use crate::{
    cpu_common::{Segment, ServiceEvent},
    cpu_vx0::*,
};

impl NecVx0 {
    /// Execute the IRET microcode routine.
    pub fn iret_routine(&mut self) {
        self.cycle_i(0x0c8);
        self.farret(true);
        self.pop_flags();
        self.cycle_i(0x0ca);
    }

    /// Perform a software interrupt
    pub fn sw_interrupt(&mut self, interrupt: u8) {
        // Interrupt FC, emulator internal services.
        if self.enable_service_interrupt && interrupt == 0xFC {
            match self.a.h() {
                0x01 => {
                    // TODO: Make triggering pit logging a separate service number. Just re-using this one
                    // out of laziness.
                    self.service_events.push_back(ServiceEvent::TriggerPITLogging);

                    log::debug!(
                        "Received emulator trap interrupt: CS: {:04X} IP: {:04X}",
                        self.b.x(),
                        self.c.x()
                    );
                    self.biu_fetch_suspend();
                    self.cycles(4);

                    self.cs = self.b.x();
                    self.pc = self.c.x();

                    // Set execution segments
                    self.ds = self.cs;
                    self.es = self.cs;
                    self.ss = self.cs;
                    // Create stack
                    self.sp = 0xFFFE;

                    self.biu_queue_flush();
                    self.cycles(4);
                    self.set_breakpoint_flag();
                }
                _ => {}
            }
            return;
        }

        self.cycles_i(3, &[0x19d, 0x19e, 0x19f]);

        // Read the IVT
        let vec_addr = (interrupt as usize * INTERRUPT_VEC_LEN) as u16;

        let new_ip = self.biu_read_u16(Segment::None, vec_addr, ReadWriteFlag::Normal);
        self.cycle_i(0x1a1);
        let new_cs = self.biu_read_u16(Segment::None, vec_addr.wrapping_add(2), ReadWriteFlag::Normal);

        // Add interrupt to call stack
        self.push_call_stack(
            CallStackEntry::Interrupt {
                ret_cs: self.cs,
                ret_ip: self.ip(),
                call_cs: new_cs,
                call_ip: new_ip,
                itype: InterruptType::Software,
                number: interrupt,
                ah: self.a.h(),
            },
            self.cs,
            self.ip(),
        );

        self.biu_fetch_suspend(); // 1a3 SUSP
        self.cycles_i(2, &[0x1a3, 0x1a4]);
        self.push_flags(ReadWriteFlag::Normal);
        self.clear_flag(Flag::Interrupt);
        self.clear_flag(Flag::Trap);
        self.cycle_i(0x1a6);
        self.farcall2(new_cs, new_ip);
        self.int_count += 1;
    }

    /*
        /// Handle a CPU exception
        pub fn handle_exception(&mut self, exception: u8) {
            self.push_flags(ReadWriteFlag::Normal);

            // Push return address of next instruction onto stack
            self.push_register16(Register16::CS, ReadWriteFlag::Normal);

            // Don't push address of next instruction
            self.push_u16(self.ip, ReadWriteFlag::Normal);

            if exception == 0x0 {
                log::trace!(
                    "CPU Exception: {:02X} Saving return: {:04X}:{:04X}",
                    exception,
                    self.cs,
                    self.ip
                );
            }
            // Read the IVT
            let ivt_addr = Cpu::calc_linear_address(0x0000, (exception as usize * INTERRUPT_VEC_LEN) as u16);
            let (new_ip, _cost) = self.bus.read_u16(ivt_addr as usize, 0).unwrap();
            let (new_cs, _cost) = self.bus.read_u16((ivt_addr + 2) as usize, 0).unwrap();

            // Add interrupt to call stack
            self.push_call_stack(
                CallStackEntry::Interrupt {
                    ret_cs: self.cs,
                    ret_ip: self.ip,
                    call_cs: new_cs,
                    call_ip: new_ip,
                    itype: InterruptType::Exception,
                    number: exception,
                    ah: self.ah,
                },
                self.cs,
                self.ip,
            );

            self.ip = new_ip;
            self.cs = new_cs;

            // Flush queue
            self.biu_queue_flush();
            self.biu_update_pc();
        }
    */
    #[allow(dead_code)]
    pub fn log_interrupt(&self, interrupt: u8) {
        match interrupt {
            0x10 => {
                // Video Services
                match self.a.h() {
                    0x00 => {
                        log::trace!(
                            "CPU: Video Interrupt: {:02X} (AH:{:02X} Set video mode) Video Mode: {:02X}",
                            interrupt,
                            self.a.h(),
                            self.a.l()
                        );
                    }
                    0x01 => {
                        log::trace!(
                            "CPU: Video Interrupt: {:02X} (AH:{:02X} Set text-mode cursor shape: CH:{:02X}, CL:{:02X})",
                            interrupt,
                            self.a.h(),
                            self.c.h(),
                            self.c.l()
                        );
                    }
                    0x02 => {
                        log::trace!("CPU: Video Interrupt: {:02X} (AH:{:02X} Set cursor position): Page:{:02X} Row:{:02X} Col:{:02X}",
                            interrupt, self.a.h(), self.b.h(), self.d.h(), self.d.l());
                    }
                    0x09 => {
                        log::trace!("CPU: Video Interrupt: {:02X} (AH:{:02X} Write character and attribute): Char:'{}' Page:{:02X} Color:{:02x} Ct:{:02}", 
                            interrupt, self.a.h(), self.a.l() as char, self.b.h(), self.b.l(), self.c.x());
                    }
                    0x10 => {
                        log::trace!(
                            "CPU: Video Interrupt: {:02X} (AH:{:02X} Write character): Char:'{}' Page:{:02X} Ct:{:02}",
                            interrupt,
                            self.a.h(),
                            self.a.l() as char,
                            self.b.h(),
                            self.c.x()
                        );
                    }
                    _ => {}
                }
            }
            _ => {}
        };
    }

    /// Execute the INTR microcode routine.
    /// skip_first is used to skip the first microcode instruction, such as when entering from
    /// INT1 or INT2.
    pub fn intr_routine(&mut self, vector: u8, itype: InterruptType, skip_first: bool) {
        // Check for interrupt breakpoint.
        if self.int_flags[vector as usize] & INTERRUPT_BREAKPOINT != 0 {
            self.set_breakpoint_flag();
        }

        if !skip_first {
            self.cycle_i(0x019d);
        }
        self.cycles_i(2, &[0x19e, 0x19f]);

        // Read the IVT
        let vec_addr = (vector as usize * INTERRUPT_VEC_LEN) as u16;

        let new_ip = self.biu_read_u16(Segment::None, vec_addr, ReadWriteFlag::Normal);
        self.cycle_i(0x1a1);
        let new_cs = self.biu_read_u16(Segment::None, vec_addr.wrapping_add(2), ReadWriteFlag::Normal);

        // Add interrupt to call stack
        self.push_call_stack(
            CallStackEntry::Interrupt {
                ret_cs: self.cs,
                ret_ip: self.ip(),
                call_cs: new_cs,
                call_ip: new_ip,
                itype,
                number: vector,
                ah: self.a.h(),
            },
            self.cs,
            self.ip(),
        );

        self.biu_fetch_suspend(); // 1a3 SUSP
        self.cycles_i(2, &[0x1a3, 0x1a4]);
        self.push_flags(ReadWriteFlag::Normal);
        self.clear_flag(Flag::Interrupt);
        self.clear_flag(Flag::Trap);
        self.cycle_i(0x1a6);

        self.farcall2(new_cs, new_ip);
    }

    /// Perform a hardware interrupt
    pub fn hw_interrupt(&mut self, vector: u8) {
        self.in_int = true;
        // Begin IRQ routine
        self.biu_inta(vector);
        self.biu_fetch_suspend();
        self.cycles_i(2, &[0x19b, 0x19c]);

        // Begin INTR routine
        self.intr_routine(vector, InterruptType::Hardware, false);
        self.int_count += 1;
        self.in_int = false;
    }

    /// Perform INT0 (Divide By 0)
    pub fn int0(&mut self) {
        self.cycles_i(2, &[0x1a7, MC_JUMP]);
        self.intr_routine(0, InterruptType::Exception, true);
        self.int_count += 1;
    }

    /// Perform INT1 (Trap)
    pub fn int1(&mut self) {
        self.cycles_i(2, &[0x198, MC_JUMP]);
        self.intr_routine(1, InterruptType::Exception, true);
        self.int_count += 1;
    }

    /// Perform INT2 (NMI)
    pub fn int2(&mut self) {
        self.cycles_i(2, &[0x199, MC_JUMP]);
        self.intr_routine(2, InterruptType::Exception, true);
        self.int_count += 1;
    }

    /// Perform INT3
    pub fn int3(&mut self) {
        self.cycles_i(4, &[0x1b0, MC_JUMP, 0x1b2, MC_JUMP]);
        self.intr_routine(3, InterruptType::Software, false);
        self.int_count += 1;
    }

    /// Perform INTO
    pub fn int_o(&mut self) {
        self.cycles_i(4, &[0x1ac, 0x1ad]);

        if self.get_flag(Flag::Overflow) {
            self.cycles_i(2, &[0x1af, MC_JUMP]);
            self.intr_routine(4, InterruptType::Hardware, false);
            self.int_count += 1;
        }
    }

    /// Return true if an interrupt can occur under current execution state
    #[inline]
    pub fn interrupts_enabled(&self) -> bool {
        self.get_flag(Flag::Interrupt) && !self.interrupt_inhibit
    }

    /// Returns true if a trap can occur under current execution state.
    #[inline]
    pub fn trap_enabled(&self) -> bool {
        // Trap if trap flag is set, OR trap flag has been cleared but disable delay in effect (to trap POPF that clears trap)
        // but only if trap is not suppressed and enable delay is 0.
        (self.get_flag(Flag::Trap) || self.trap_disable_delay != 0)
            && !self.trap_suppressed
            && self.trap_enable_delay == 0
    }
}

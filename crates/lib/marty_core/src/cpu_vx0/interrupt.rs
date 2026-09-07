/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

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
    service_interrupt::{is_martypc_probe, is_service_control, ServiceFunction},
};

impl NecVx0 {
    /// Execute the IRET microcode routine.
    pub fn iret_routine(&mut self) {
        self.cycles(1);
        self.ret(true);
        self.pop_flags();
        self.cycles(1);
    }

    /// Enter 8080 emulation mode through BRKEM after its native interrupt frame has been built.
    pub fn brkem_routine(&mut self, interrupt: u8) {
        self.sw_interrupt(interrupt);
        self.mode_flag_write_enabled = true;
        self.enter_emulation_mode();
    }

    /// Return from 8080 emulation mode through RETEM.
    pub fn retem_routine(&mut self) {
        self.ret(true);
        self.pop_flags();

        // RETEM always returns to native mode and inhibits subsequent writes to MD,
        // regardless of the value in the saved flags image.
        if self.in_emulation_mode() {
            self.exit_emulation_mode();
        }
        self.mode_flag_write_enabled = false;
    }

    /// Perform the 8080's CALLN.
    /// This is essentially a software interrupt that changes the mode flag after pushing the old
    /// flag to the stack.
    pub fn calln_8080(&mut self, interrupt: u8) {
        self.cycles(3);

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
        self.exit_emulation_mode();

        self.clear_flag(Flag::Interrupt);
        self.clear_flag(Flag::Trap);
        self.cycle_i(0x1a6);
        self.farcall2(new_cs, new_ip);
        self.int_count += 1;
    }

    /// Perform a software interrupt
    pub fn sw_interrupt(&mut self, interrupt: u8) {
        if is_martypc_probe(interrupt, self) {
            self.service_events.push_back(ServiceEvent::ServiceInterruptProbe);
            return;
        }

        // Configured emulator internal service interrupt.
        if self.service_interrupt_vector == Some(interrupt) {
            let function = self.a.h();

            if function == ServiceFunction::ServiceControl.into() {
                if is_service_control(self) {
                    self.service_events.push_back(ServiceEvent::ServiceInterrupt(function));
                    return;
                }
            }
            else if self.service_interrupt_enabled {
                match ServiceFunction::try_from(function) {
                    Ok(ServiceFunction::Debugger) => {
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
                    _ => self.service_events.push_back(ServiceEvent::ServiceInterrupt(function)),
                }
                return;
            }
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
        if interrupt == 0x10 {
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

        // Interrupts taken while executing 8080 code are serviced in native mode. Push
        // first so the emulation-mode value of MD can be restored by IRET.
        if self.in_emulation_mode() {
            self.exit_emulation_mode();
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cpu_common::{CpuArch, CpuType, TraceMode},
        tracelogger::TraceLogger,
    };

    fn test_cpu() -> NecVx0 {
        NecVx0::new(CpuType::NecV20(CpuArch::I86), TraceMode::None, TraceLogger::None)
    }

    fn write_vector(cpu: &mut NecVx0, vector: u8, cs: u16, ip: u16) {
        let address = vector as usize * INTERRUPT_VEC_LEN;
        cpu.bus.write_u16(address, ip, 0).unwrap();
        cpu.bus.write_u16(address + 2, cs, 0).unwrap();
    }

    fn read_stack_word(cpu: &mut NecVx0, offset: u16) -> u16 {
        let address = NecVx0::calc_linear_address(cpu.ss, offset) as usize;
        cpu.bus.read_u16(address, 0).unwrap().0
    }

    #[test]
    fn native_iret_cannot_restore_mode_when_writes_are_disabled() {
        let mut cpu = test_cpu();
        cpu.ss = 0;
        cpu.sp = 0x1000;
        cpu.bus.write_u16(0x1000, 0x1234, 0).unwrap();
        cpu.bus.write_u16(0x1002, 0x5678, 0).unwrap();
        cpu.bus.write_u16(0x1004, CPU_FLAGS_RESERVED_ON, 0).unwrap();
        cpu.i.opcode = 0xCF;

        cpu.iret_routine();

        assert_eq!(cpu.cs, 0x5678);
        assert_eq!(cpu.pc, 0x1234);
        assert!(cpu.get_flag(Flag::Mode));
        assert!(!cpu.in_emulation_mode());
        assert!(!cpu.mode_flag_write_enabled);
    }

    #[test]
    fn interrupt_from_emulation_mode_returns_to_emulation_mode() {
        const BRKEM_VECTOR: u8 = 0x20;
        const IRQ_VECTOR: u8 = 0x21;

        let mut cpu = test_cpu();
        cpu.ss = 0;
        cpu.sp = 0x2000;
        cpu.cs = 0x1000;
        cpu.pc = 0x0100;
        cpu.queue.flush();
        write_vector(&mut cpu, BRKEM_VECTOR, 0x2000, 0x0200);
        write_vector(&mut cpu, IRQ_VECTOR, 0x3000, 0x0300);

        cpu.brkem_routine(BRKEM_VECTOR);
        let emulation_sp = cpu.sp;
        assert!(cpu.mode_flag_write_enabled);
        assert!(cpu.in_emulation_mode());

        cpu.intr_routine(IRQ_VECTOR, InterruptType::Hardware, false);

        assert!(!cpu.in_emulation_mode());
        assert!(cpu.get_flag(Flag::Mode));
        assert!(cpu.mode_flag_write_enabled);
        assert_eq!(
            read_stack_word(&mut cpu, emulation_sp.wrapping_sub(2)) & CPU_FLAG_MODE,
            0
        );

        cpu.i.opcode = 0xCF;
        cpu.iret_routine();

        assert_eq!(cpu.sp, emulation_sp);
        assert!(cpu.in_emulation_mode());
        assert!(!cpu.get_flag(Flag::Mode));
        assert!(cpu.mode_flag_write_enabled);
        assert_eq!(cpu.cpu_type, CpuType::NecV20(CpuArch::I8080));
    }

    #[test]
    fn retem_returns_native_and_disables_mode_flag_writes() {
        const BRKEM_VECTOR: u8 = 0x20;

        let mut cpu = test_cpu();
        cpu.ss = 0;
        cpu.sp = 0x2000;
        cpu.cs = 0x1000;
        cpu.pc = 0x0100;
        cpu.queue.flush();
        write_vector(&mut cpu, BRKEM_VECTOR, 0x2000, 0x0200);

        cpu.brkem_routine(BRKEM_VECTOR);
        assert!(cpu.mode_flag_write_enabled);
        assert!(cpu.in_emulation_mode());

        // RETEM must force native mode even if the saved MD bit has been altered.
        let flags_offset = cpu.sp.wrapping_add(4);
        let flags_address = NecVx0::calc_linear_address(cpu.ss, flags_offset) as usize;
        cpu.bus.write_u16(flags_address, CPU_FLAGS_RESERVED_ON, 0).unwrap();
        cpu.i.opcode = 0xFD;
        cpu.retem_routine();

        assert_eq!(cpu.sp, 0x2000);
        assert_eq!(cpu.cs, 0x1000);
        assert_eq!(cpu.pc, 0x0100);
        assert!(!cpu.mode_flag_write_enabled);
        assert!(!cpu.in_emulation_mode());
        assert!(cpu.get_flag(Flag::Mode));
        assert_eq!(cpu.cpu_type, CpuType::NecV20(CpuArch::I86));
    }
}

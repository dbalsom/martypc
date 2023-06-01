use crate::cpu_808x::*;
use crate::cpu_808x::jump::*;

impl<'a> Cpu<'a> {

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
        if interrupt == 0xFC {
            match self.ah {
                0x01 => {

                    // TODO: Make triggering pit logging a separate service number. Just re-using this one
                    // out of laziness.
                    self.service_events.push_back(ServiceEvent::TriggerPITLogging);

                    log::debug!("Received emulator trap interrupt: CS: {:04X} IP: {:04X}", self.bx, self.cx);
                    self.biu_suspend_fetch();
                    self.cycles(4);

                    self.cs = self.bx;
                    self.ip = self.cx;

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

            return
        }

        self.cycles_i(3, &[0x19d, 0x19e, 0x19f]);
        // Read the IVT
        let ivt_addr = Cpu::calc_linear_address(0x0000, (interrupt as usize * INTERRUPT_VEC_LEN) as u16);
        let new_ip = self.biu_read_u16(Segment::None, ivt_addr, ReadWriteFlag::Normal);
        self.cycle_i(0x1a1);
        let new_cs = self.biu_read_u16(Segment::None, ivt_addr + 2, ReadWriteFlag::Normal);

        // Add interrupt to call stack
        self.push_call_stack(
            CallStackEntry::Interrupt {
                ret_cs: self.cs,
                ret_ip: self.ip,
                call_cs: new_cs,
                call_ip: new_ip,
                itype: InterruptType::Software,
                number: interrupt,
                ah: self.ah
            },
            self.cs,
            self.ip
        );

        self.biu_suspend_fetch(); // 1a3 SUSP
        self.cycles_i(2, &[0x1a3, 0x1a4]);
        self.push_flags(ReadWriteFlag::Normal);
        self.clear_flag(Flag::Interrupt);
        self.clear_flag(Flag::Trap);

        // FARCALL2
        self.cycles_i(4, &[0x1a6, MC_JUMP, 0x06c, MC_CORR]);
        // Push return segment
        self.push_register16(Register16::CS, ReadWriteFlag::Normal);
        self.cs = new_cs;        
        self.cycle_i(0x06e);

        // NEARCALL
        let old_ip = self.ip;
        self.cycles_i(2, &[0x06f, MC_JUMP]);
        self.ip = new_ip;    
        self.biu_queue_flush();  
        self.cycles_i(3, &[0x077, 0x078, 0x079]);
        // Finally, push return address
        self.push_u16(old_ip, ReadWriteFlag::RNI);

        if interrupt == 0x13 {
            // Disk interrupts
            if self.dl & 0x80 != 0 {
                // Hard disk request
                match self.ah {
                    0x03 => {
                        log::trace!("Hard disk int13h: Write Sectors: Num: {} Drive: {:02X} C: {} H: {} S: {}",
                            self.al,
                            self.dl,
                            self.ch,
                            self.dh,
                            self.cl)
                    }
                    _=> log::trace!("Hard disk requested in int13h. AH: {:02X}", self.ah)
                }
                
            }
        }

        if interrupt == 0x10 && self.ah==0x00 {
            log::trace!("CPU: int10h: Set Mode {:02X} Return [{:04X}:{:04X}]", interrupt, self.cs, self.ip);
        }        

        if interrupt == 0x21 {
            //log::trace!("CPU: int21h: AH: {:02X} [{:04X}:{:04X}]", self.ah, self.cs, self.ip);
            if self.ah == 0x4B {
                log::trace!("int21,4B: EXEC/Load and Execute Program @ [{:04X}:{:04X}] es:bx: [{:04X}:{:04X}]", self.cs, self.ip, self.es, self.bx);
            }
            if self.ah == 0x55 {
                log::trace!("int21,55:  @ [{:04X}]:[{:04X}]", self.cs, self.ip);
            }            
        }         

        if interrupt == 0x16 {
            if self.ah == 0x01 {
                //log::trace!("int16,01: Poll keyboard @ [{:04X}]:[{:04X}]", self.cs, self.ip);
            }
        }

        self.int_count += 1;
    }

    /// Handle a CPU exception
    pub fn handle_exception(&mut self, exception: u8) {

        self.push_flags(ReadWriteFlag::Normal);

        // Push return address of next instruction onto stack
        self.push_register16(Register16::CS, ReadWriteFlag::Normal);

        // Don't push address of next instruction
        self.push_u16(self.ip, ReadWriteFlag::Normal);
        
        if exception == 0x0 {
            log::trace!("CPU Exception: {:02X} Saving return: {:04X}:{:04X}", exception, self.cs, self.ip);
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
                ah: self.ah
            },
            self.cs,
            self.ip
        );

        self.ip = new_ip;
        self.cs = new_cs;

        // Flush queue
        self.biu_queue_flush();
        self.biu_update_pc();        
    }    

    pub fn log_interrupt(&self, interrupt: u8) {

        match interrupt {
            0x10 => {
                // Video Services
                match self.ah {
                    0x00 => {
                        log::trace!("CPU: Video Interrupt: {:02X} (AH:{:02X} Set video mode) Video Mode: {:02X}", 
                            interrupt, self.ah, self.al);
                    }
                    0x01 => {
                        log::trace!("CPU: Video Interrupt: {:02X} (AH:{:02X} Set text-mode cursor shape: CH:{:02X}, CL:{:02X})", 
                            interrupt, self.ah, self.ch, self.cl);
                    }
                    0x02 => {
                        log::trace!("CPU: Video Interrupt: {:02X} (AH:{:02X} Set cursor position): Page:{:02X} Row:{:02X} Col:{:02X}",
                            interrupt, self.ah, self.bh, self.dh, self.dl);
                    }
                    0x09 => {
                        log::trace!("CPU: Video Interrupt: {:02X} (AH:{:02X} Write character and attribute): Char:'{}' Page:{:02X} Color:{:02x} Ct:{:02}", 
                            interrupt, self.ah, self.al as char, self.bh, self.bl, self.cx);
                    }
                    0x10 => {
                        log::trace!("CPU: Video Interrupt: {:02X} (AH:{:02X} Write character): Char:'{}' Page:{:02X} Ct:{:02}", 
                            interrupt, self.ah, self.al as char, self.bh, self.cx);
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

        //log::debug!("in INTR routine!");
        if !skip_first {
            self.cycle_i(0x019d);
        }
        self.cycles_i(2, &[0x19e, 0x19f]);
        // Read the IVT
        let ivt_addr = Cpu::calc_linear_address(0x0000, (vector as usize * INTERRUPT_VEC_LEN) as u16);
        let new_ip = self.biu_read_u16(Segment::None, ivt_addr, ReadWriteFlag::Normal);
        self.cycle_i(0x1a1);
        let new_cs = self.biu_read_u16(Segment::None, ivt_addr + 2, ReadWriteFlag::Normal);

        // Add interrupt to call stack
        self.push_call_stack(
            CallStackEntry::Interrupt {
                ret_cs: self.cs,
                ret_ip: self.ip,
                call_cs: new_cs,
                call_ip: new_ip,
                itype,
                number: vector,
                ah: self.ah
            },
            self.cs,
            self.ip
        );

        self.biu_suspend_fetch(); // 1a3 SUSP
        self.cycles_i(2, &[0x1a3, 0x1a4]);
        self.push_flags(ReadWriteFlag::Normal);
        self.clear_flag(Flag::Interrupt);
        self.clear_flag(Flag::Trap);        
        self.cycle_i(0x1a6);

        self.farcall2(new_cs, new_ip);
    }

    /// Perform a hardware interrupt
    pub fn hw_interrupt(&mut self, vector: u8) {

        // Begin IRQ routine
        self.set_mc_pc(0x19a);
        self.biu_inta(vector);
        self.biu_suspend_fetch();
        self.cycles_i(2, &[0x19b, 0x19c]);

        // Begin INTR routine
        self.intr_routine(vector, InterruptType::Hardware, false);
        self.int_count += 1;
    }

    /// Perform INT1 (Trap)
    pub fn int1(&mut self) {
        self.cycles_i(2, &[0x198, MC_JUMP]);
        self.intr_routine(1, InterruptType::Hardware, true);
        self.int_count += 1;        
    }

    /// Perform INT2 (NMI)
    pub fn int2(&mut self) {
        self.cycles_i(2, &[0x199, MC_JUMP]);
        self.intr_routine(2, InterruptType::Hardware, true);
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
        (self.get_flag(Flag::Trap) || self.trap_disable_delay != 0) && !self.trap_suppressed && self.trap_enable_delay == 0
    }

}
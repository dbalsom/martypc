/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

    cpu_808x::cycle.rs

    Contains functions for cycling the cpu through its various states.
    Cycle functions are called by instructions and bus logic whenever
    the CPU should be ticked.

*/

use crate::cpu_808x::*;

#[cfg(feature = "cpu_validator")]
use crate::cpu_validator::{BusType, ReadType};

macro_rules! validate_read_u8 {
    ($myself: expr, $addr: expr, $data: expr, $btype: expr, $rtype: expr) => {{
        #[cfg(feature = "cpu_validator")]
        if let Some(ref mut validator) = &mut $myself.validator {
            validator.emu_read_byte($addr, $data, $btype, $rtype)
        }
    }};
}

macro_rules! validate_write_u8 {
    ($myself: expr, $addr: expr, $data: expr, $btype: expr) => {{
        #[cfg(feature = "cpu_validator")]
        if let Some(ref mut validator) = &mut $myself.validator {
            validator.emu_write_byte($addr, $data, $btype)
        }
    }};
}

impl Cpu {
    #[inline]
    pub fn set_mc_pc(&mut self, instr: u16) {
        self.mc_pc = instr;
        //self.next_instr = instr;
    }

    #[inline]
    pub fn next_mc(&mut self) {
        if self.mc_pc < MC_NONE {
            self.mc_pc += 1;
        }
    }

    #[inline]
    pub fn cycle(&mut self) {
        self.cycle_i(MC_NONE);
    }

    /// Execute a CPU cycle.
    /// 'instr' is the microcode line reference of the cycle being executed, if applicable
    /// (otherwise it should be passed MC_NONE).
    /// The CPU will transition between T-states, execute bus transfers on T3 or TW-last,
    /// and otherwise do all necessary actions to advance the cpu state.
    pub fn cycle_i(&mut self, instr: u16) {
        if instr == MC_NONE {
            self.trace_instr = self.mc_pc;
        }
        else {
            self.mc_pc = instr;
            self.trace_instr = instr;
        }

        if self.t_cycle == TCycle::Tinit {
            self.t_cycle = TCycle::T1;
        }

        if self.in_int {
            self.int_elapsed += 1;
        }
        else {
            self.instr_elapsed += 1;
        }

        // Operate current t-state
        match self.bus_status_latch {
            BusStatus::Passive => {
                self.transfer_n = 0;
            }
            BusStatus::MemRead
            | BusStatus::MemWrite
            | BusStatus::IoRead
            | BusStatus::IoWrite
            | BusStatus::CodeFetch
            | BusStatus::InterruptAck => {
                match self.t_cycle {
                    TCycle::Tinit => {
                        panic!("Can't execute TInit state");
                    }
                    TCycle::Ti | TCycle::T1 => {}
                    TCycle::T2 => {
                        // Turn off ale signal on T2
                        self.i8288.ale = false;

                        // Read/write signals go high on T2.
                        match self.bus_status_latch {
                            BusStatus::CodeFetch | BusStatus::MemRead => {
                                self.i8288.mrdc = true;
                            }
                            BusStatus::MemWrite => {
                                // Only AMWC goes high on T2. MWTC delayed to T3.
                                self.i8288.amwc = true;
                            }
                            BusStatus::IoRead => {
                                self.i8288.iorc = true;
                            }
                            BusStatus::IoWrite => {
                                // Only AIOWC goes high on T2. IOWC delayed to T3.
                                self.i8288.aiowc = true;
                            }
                            BusStatus::InterruptAck => {
                                self.i8288.inta = true;
                                if self.transfer_n == 0 {
                                    self.lock = true;
                                }
                            }
                            _ => {}
                        }

                        match self.bus_status_latch {
                            BusStatus::CodeFetch | BusStatus::MemRead => {
                                self.bus_wait_states = self
                                    .bus
                                    .get_read_wait(self.address_latch as usize, self.instr_elapsed)
                                    .unwrap();
                            }
                            BusStatus::MemWrite => {
                                self.bus_wait_states = self
                                    .bus
                                    .get_write_wait(self.address_latch as usize, self.instr_elapsed)
                                    .unwrap();
                            }
                            BusStatus::IoRead => {
                                self.bus_wait_states = 1;
                            }
                            BusStatus::IoWrite => {
                                self.bus_wait_states = 1;
                            }
                            _ => {}
                        }

                        if !self.enable_wait_states {
                            //trace_print!(self, "Suppressing wait states!");
                            self.bus_wait_states = 0;
                            self.wait_states = 0;
                        }
                    }
                    TCycle::T3 => {
                        if self.wait_states == 0 {
                            // Do bus transfer on T3 if no wait states.
                            self.do_bus_transfer();
                        }
                        else {
                            self.ready = false;
                        }

                        // A prefetch decision is always made on T3 of the last bus cycle of an atomic
                        // bus operation, regardless of wait states.
                        if self.final_transfer {
                            self.biu_make_biu_decision();
                        }
                    }
                    TCycle::Tw => {
                        if self.is_last_wait() {
                            // Reading/writing occurs on the last Tw state.
                            self.do_bus_transfer();
                            self.ready = true;
                        }
                    }
                    TCycle::T4 => {
                        // If we just completed a code fetch, make the byte available in the queue.
                        if let BusStatus::CodeFetch = self.bus_status_latch {
                            self.queue.push8(self.data_bus as u8);

                            //self.pc = (self.pc + 1) & 0xFFFFFu32;
                            self.inc_pc();
                        }
                    }
                }
            }
            BusStatus::Halt => {
                self.trace_comment("HALT");
            }
        };

        // Perform cycle tracing, if enabled
        if self.trace_enabled {
            match self.trace_mode {
                TraceMode::Cycle => {
                    // Get value of timer channel #1 for DMA printout
                    let mut dma_count = 0;

                    if let Some(pit) = self.bus.pit_mut().as_mut() {
                        (_, dma_count) = pit.get_channel_count(1);
                    }

                    let state_str = self.cycle_state_string(dma_count, false);
                    self.trace_print(&state_str);
                    self.trace_str_vec.push(state_str);

                    self.trace_comment.clear();
                    self.trace_instr = MC_NONE;
                }
                TraceMode::Sigrok => {
                    self.trace_csv_line();
                }
                _ => {}
            }
        }

        #[cfg(feature = "cpu_validator")]
        {
            let cycle_state = self.get_cycle_state();
            self.cycle_states.push(cycle_state);
        }

        // Do DRAM refresh (DMA channel 0) simulation
        if self.enable_wait_states && self.dram_refresh_simulation {
            self.dram_refresh_cycle_num = self.dram_refresh_cycle_num.saturating_sub(1);

            match &mut self.dma_state {
                DmaState::Idle => {
                    if self.dram_refresh_cycle_num == 0 && self.dram_refresh_cycle_period > 0 {
                        // DRAM refresh cycle counter has hit terminal count.
                        // Begin DMA transfer simulation by issuing a DREQ.
                        self.dma_state = DmaState::Dreq;

                        // Reset counter.
                        self.dram_refresh_cycle_num = self.dram_refresh_cycle_period;
                    }
                }
                DmaState::TimerTrigger => {
                    // Timer channel #1 begins rising immediately, but with a slow enough
                    // rise time that DREQ is functionally delayed one cycle
                    self.dma_state = DmaState::Dreq;
                }
                DmaState::Dreq => {
                    // DMA request triggered on DMA controller. Next cycle, DMA controller
                    // will assert HRQ (Hold Request)
                    self.dma_state = DmaState::Hrq;
                }
                DmaState::Hrq => {
                    // DMA Hold Request.
                    // DMA Hold request waits for issuance of HOLDA (Hold Acknowledge)
                    // This signal is generated by miscellaneous TTL logic when S0 & S1 state
                    // indicates PASV or HALT and !LOCK. (1,1) (Note, bus goes PASV after T2)

                    if self.bus_status == BusStatus::Passive
                        || match self.t_cycle {
                            TCycle::T3 | TCycle::Tw | TCycle::T4 => true,
                            _ => false,
                        }
                    {
                        // S0 & S1 are idle. Issue hold acknowledge if LOCK not asserted.
                        if !self.lock {
                            self.dma_state = DmaState::HoldA;
                        }
                    }
                }
                DmaState::HoldA => {
                    // DMA Hold Acknowledge has been issued. DMA controller will enter S1
                    // on next cycle.
                    self.dma_state = DmaState::Operating(4);
                    self.dma_aen = true;
                }
                DmaState::Operating(cycles) => {
                    // the DMA controller has control of the bus now.
                    // Run DMA transfer cycles.
                    *cycles = cycles.saturating_sub(1);
                    if *cycles == 3 {
                        // DMAWAIT asserted on S2
                        self.dma_wait_states = 7; // Effectively 6 as this is decremented this cycle
                        self.ready = false;
                    }
                    if *cycles == 0 {
                        // Transfer cycles have elapsed, so move to next state.
                        self.dma_aen = false;
                        self.dma_state = DmaState::Idle
                    }
                }
            }
        }

        // Transition to next T state
        self.t_cycle = match self.t_cycle {
            TCycle::Tinit => {
                // A new bus cycle has been initiated, begin it in T1.
                TCycle::T1
            }
            TCycle::Ti => {
                // If bus status is PASV, stay in Ti (no bus transfer occurring)
                match self.bus_status_latch {
                    BusStatus::Passive => TCycle::Ti,
                    BusStatus::Halt => {
                        // Halt only lasts for one cycle. Reset status and ALE.
                        self.bus_status = BusStatus::Passive;
                        self.bus_status_latch = BusStatus::Passive;
                        self.i8288.ale = false;
                        TCycle::Ti
                    }
                    _ => TCycle::T1,
                }
            }
            TCycle::T1 => {
                // If there is a valid bus status on T1, transition to T2, unless
                // status is HALT, which only lasts one cycle.
                match self.bus_status_latch {
                    BusStatus::Passive => {
                        //panic!("T1 with passive bus"),
                        TCycle::T1
                    }
                    BusStatus::Halt => {
                        // Halt only lasts for one cycle. Reset status and ALE.
                        self.bus_status = BusStatus::Passive;
                        self.bus_status_latch = BusStatus::Passive;
                        self.i8288.ale = false;
                        TCycle::Ti
                    }
                    _ => TCycle::T2,
                }
            }
            TCycle::T2 => {
                self.wait_states += self.bus_wait_states;
                TCycle::T3
            }
            TCycle::T3 => {
                // If no wait states have been reported, advance to T3, otherwise go to Tw
                if self.wait_states > 0 || self.dma_wait_states > 0 {
                    self.wait_states = self.wait_states.saturating_sub(1);
                    TCycle::Tw
                }
                else {
                    self.biu_bus_end();
                    TCycle::T4
                }
            }
            TCycle::Tw => {
                // If we are handling wait states, continue in Tw (decrement at end of cycle)
                // If we have handled all wait states, transition to T4
                if self.wait_states > 0 || self.dma_wait_states > 0 {
                    self.wait_states = self.wait_states.saturating_sub(1);
                    //log::debug!("wait states: {}", self.wait_states);
                    TCycle::Tw
                }
                else {
                    self.biu_bus_end();
                    TCycle::T4
                }
            }
            TCycle::T4 => {
                // We reached the end of a bus transfer, to transition back to Ti and PASV.
                self.bus_status_latch = BusStatus::Passive;
                TCycle::Ti
            }
        };

        // Handle prefetching
        self.biu_tick_prefetcher();

        match self.fetch_state {
            FetchState::ScheduleNext | FetchState::Scheduled(0) => {
                // A fetch is scheduled for this cycle; however we may have additional delays to process.

                // If bus_pending_eu is true, then we arrived here during biu_bus_begin.
                // That means that we biu_bus_begin will process a fetch abort.
                // In that case, we should do nothing instead of transitioning to a new fetch state.
                if !self.bus_pending_eu {
                    if let BusStatus::Passive = self.bus_status_latch {
                        // Begin a fetch if we are not transitioning into any delay state, otherwise transition
                        // into said state.
                        if self.next_fetch_state == FetchState::InProgress {
                            self.begin_fetch();
                        }
                        else {
                            self.fetch_state = self.next_fetch_state;
                        }
                    }
                }
            }
            FetchState::DelayDone => {
                if self.next_fetch_state == FetchState::InProgress {
                    if self.biu_state_new == BiuStateNew::Prefetch {
                        self.begin_fetch();
                    }
                }
                else {
                    self.fetch_state = self.next_fetch_state;
                }
            }
            FetchState::Idle if !self.fetch_suspended => {
                if self.queue_op == QueueOp::Flush {
                    //trace_print!(self, "Flush scheduled fetch!");
                    self.biu_schedule_fetch(2);
                }

                if (self.bus_status_latch == BusStatus::Passive) && (self.t_cycle == TCycle::T1) {
                    // Nothing is scheduled, suspended, aborted, and bus is idle. Make a prefetch decision.
                    //trace_print!(self, "schedule fetch due to bus idle");
                    //self.biu_make_fetch_decision();
                }
            }
            _ => {}
        }

        self.biu_tick_state();

        // Reset queue operation
        self.last_queue_op = self.queue_op;
        self.last_queue_byte = self.queue_byte;
        self.queue_op = QueueOp::Idle;

        self.instr_cycle += 1;
        self.device_cycles += 1;

        self.cycle_num += 1;
        self.dma_wait_states = self.dma_wait_states.saturating_sub(1);

        if self.wait_states == 0 && self.dma_wait_states == 0 {
            self.ready = true;
        }

        // Advance timestamp 210ns.
        self.t_stamp += self.t_step;

        /*
        // Try to catch a runaway instruction?
        if !self.halted && !self.in_rep && self.instr_cycle > 200 {
            log::error!("Exceeded max cycles for instruction.");
            self.trace_flush();
            panic!("Exceeded max cycles for instruction.");
        }
        */

        self.last_queue_len = self.queue.len();
    }

    /// Temporary function to increment pc. Needed to handle wraparound
    /// of code segment.  This should be unnecessary once pc is converted to u16.
    pub fn inc_pc(&mut self) {
        // pc shouldn't be less than cs:00
        if self.pc < ((self.cs as u32) << 4) {
            // Bad pc, fall back to old behavior.
            self.pc = (self.pc + 1) & 0xFFFFF;
            return;
        }

        assert!(self.pc >= ((self.cs as u32) << 4));

        // Subtract cs from pc to get the 'real' value of pc
        let mut real_pc: u16 = (self.pc - ((self.cs as u32) << 4)) as u16;

        // Increment real pc with wraparound
        real_pc = real_pc.wrapping_add(1);

        // Calculate new 'linear' pc
        self.pc = Cpu::calc_linear_address(self.cs, real_pc);
    }

    pub fn do_bus_transfer(&mut self) {
        let byte;

        match (self.bus_status_latch, self.transfer_size) {
            (BusStatus::CodeFetch, TransferSize::Byte) => {
                (byte, _) = self
                    .bus
                    .read_u8(self.address_latch as usize, self.instr_elapsed)
                    .unwrap();
                self.data_bus = byte as u16;

                validate_read_u8!(
                    self,
                    self.address_latch,
                    (self.data_bus & 0x00FF) as u8,
                    BusType::Mem,
                    ReadType::Code
                );
            }
            (BusStatus::CodeFetch, TransferSize::Word) => {
                (self.data_bus, _) = self
                    .bus
                    .read_u16(self.address_latch as usize, self.instr_elapsed)
                    .unwrap();
            }
            (BusStatus::MemRead, TransferSize::Byte) => {
                (byte, _) = self
                    .bus
                    .read_u8(self.address_latch as usize, self.instr_elapsed)
                    .unwrap();
                self.instr_elapsed = 0;
                self.data_bus = byte as u16;

                validate_read_u8!(
                    self,
                    self.address_latch,
                    (self.data_bus & 0x00FF) as u8,
                    BusType::Mem,
                    ReadType::Data
                );
            }
            (BusStatus::MemRead, TransferSize::Word) => {
                (self.data_bus, _) = self
                    .bus
                    .read_u16(self.address_latch as usize, self.instr_elapsed)
                    .unwrap();
                self.instr_elapsed = 0;
            }
            (BusStatus::MemWrite, TransferSize::Byte) => {
                self.i8288.mwtc = true;
                _ = self
                    .bus
                    .write_u8(
                        self.address_latch as usize,
                        (self.data_bus & 0x00FF) as u8,
                        self.instr_elapsed,
                    )
                    .unwrap();
                self.instr_elapsed = 0;

                validate_write_u8!(self, self.address_latch, (self.data_bus & 0x00FF) as u8, BusType::Mem);
            }
            (BusStatus::MemWrite, TransferSize::Word) => {
                self.i8288.mwtc = true;
                _ = self
                    .bus
                    .write_u16(self.address_latch as usize, self.data_bus, self.instr_elapsed)
                    .unwrap();
                self.instr_elapsed = 0;
            }
            (BusStatus::IoRead, TransferSize::Byte) => {
                self.i8288.iorc = true;
                byte = self
                    .bus
                    .io_read_u8((self.address_latch & 0xFFFF) as u16, self.instr_elapsed);
                self.data_bus = byte as u16;
                self.instr_elapsed = 0;

                validate_read_u8!(
                    self,
                    self.address_latch,
                    (self.data_bus & 0x00FF) as u8,
                    BusType::Io,
                    ReadType::Data
                );
            }
            (BusStatus::IoWrite, TransferSize::Byte) => {
                self.i8288.iowc = true;
                self.bus.io_write_u8(
                    (self.address_latch & 0xFFFF) as u16,
                    (self.data_bus & 0x00FF) as u8,
                    self.instr_elapsed,
                );
                self.instr_elapsed = 0;

                validate_write_u8!(self, self.address_latch, (self.data_bus & 0x00FF) as u8, BusType::Io);
            }
            (BusStatus::InterruptAck, TransferSize::Byte) => {
                // The vector is read from the PIC directly before we even enter an INTA bus state, so there's
                // nothing to do.

                //log::debug!("in INTA transfer_n: {}", self.transfer_n);
                // Deassert lock
                if self.transfer_n == 2 {
                    //log::debug!("deasserting lock! transfer_n: {}", self.transfer_n);
                    self.lock = false;
                    self.intr = false;
                }
                //self.transfer_n += 1;
            }
            _ => {
                trace_print!(self, "Unhandled bus state!");
                log::warn!("Unhandled bus status: {:?}!", self.bus_status_latch);
            }
        }

        self.bus_status = BusStatus::Passive;
        self.address_bus = (self.address_bus & !0xFF) | (self.data_bus as u32);
    }

    pub fn begin_fetch(&mut self) {
        if let BiuStateNew::Prefetch | BiuStateNew::ToPrefetch(_) = self.biu_state_new {
            //trace_print!(self, "scheduling fetch: {}", self.queue.len());

            if self.biu_queue_has_room() {
                //trace_print!(self, "Setting address bus to PC: {:05X}", self.pc);
                self.fetch_state = FetchState::InProgress;
                self.bus_status = BusStatus::CodeFetch;
                self.bus_status_latch = BusStatus::CodeFetch;
                self.bus_segment = Segment::CS;
                self.t_cycle = TCycle::T1;
                self.address_bus = self.pc;
                self.address_latch = self.pc;
                self.i8288.ale = true;
                self.data_bus = 0;
                self.transfer_size = self.fetch_size;
                self.operand_size = match self.fetch_size {
                    TransferSize::Byte => OperandSize::Operand8,
                    TransferSize::Word => OperandSize::Operand16,
                };
                self.transfer_n = 1;
                self.final_transfer = true;
            }
            else if !self.bus_pending_eu {
                // Cancel fetch if queue is full and no pending bus request from EU that
                // would otherwise trigger an abort.
                self.biu_abort_fetch_full();
            }
        }
        else {
            log::error!("Tried to fetch in invalid BIU state: {:?}", self.biu_state_new);
            self.trace_flush();
            panic!(
                "{}",
                format!("Tried to fetch in invalid BIU state: {:?}", self.biu_state_new)
            );
        }
    }

    #[inline]
    pub fn cycle_nx(&mut self) {
        self.nx = true;
    }

    #[inline]
    pub fn cycle_nx_i(&mut self, _instr: u16) {
        self.nx = true;
    }

    #[inline]
    pub fn cycles(&mut self, ct: u32) {
        for _ in 0..ct {
            self.cycle();
        }
    }

    #[inline]
    pub fn cycles_i(&mut self, ct: u32, instrs: &[u16]) {
        for i in 0..ct as usize {
            self.cycle_i(instrs[i]);
        }
    }

    #[inline]
    pub fn cycles_nx(&mut self, ct: u32) {
        self.cycles(ct - 1);
        self.nx = true;
        //self.cycles(ct);
    }

    #[inline]
    pub fn cycles_nx_i(&mut self, ct: u32, instrs: &[u16]) {
        self.cycles_i(ct - 1, instrs);
        self.nx = true;
        //self.cycles_i(ct, instrs);
    }

    #[cfg(feature = "cpu_validator")]
    pub fn get_cycle_states(&self) -> &Vec<CycleState> {
        &self.cycle_states
    }
}

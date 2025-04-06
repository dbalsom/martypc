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

    cpu_808x::cycle.rs

    Contains functions for cycling the cpu through its various states.
    Cycle functions are called by instructions and bus logic whenever
    the CPU should be ticked.

*/

use crate::{cpu_808x::*, cpu_common::QueueOp};

impl Intel808x {
    #[inline(always)]
    pub fn set_mc_pc(&mut self, instr: u16) {
        self.mc_pc = instr;
        //self.next_instr = instr;
    }

    #[inline(always)]
    pub fn next_mc(&mut self) {
        if self.mc_pc < MC_NONE {
            self.mc_pc += 1;
        }
    }

    #[inline(always)]
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

        // TODO: Can we refactor this so this isn't necessary?
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
                match self.fetch_state {
                    FetchState::Delayed(0) => {
                        self.fetch_state = FetchState::Normal;
                        self.trace_comment("END_DELAY");
                        self.biu_make_fetch_decision();
                    }
                    FetchState::PausedFull => {
                        if self.queue.has_room_for_fetch() {
                            self.biu_make_fetch_decision();
                        }
                    }
                    _ => {}
                }
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
                    TCycle::Ti => {
                        self.biu_make_fetch_decision();
                    }
                    TCycle::T1 => {}
                    TCycle::T2 => {
                        // Turn off ale signal on T2
                        self.i8288.ale = false;
                        // Default to no IO wait states. IO bus states can override.
                        self.io_wait_states = 0;

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
                                // TODO: IO wait states are not determined by the CPU, but by motherboard logic.
                                //       We should look up IO wait states from the motherboard (bus).
                                self.io_wait_states = 1;
                            }
                            BusStatus::IoWrite => {
                                //self.io_wait_states = 1;
                                self.io_wait_states = self
                                    .bus
                                    .io_write_wait((self.address_latch & 0xFFFF) as u16, self.instr_elapsed);
                            }
                            _ => {}
                        }

                        if !self.enable_wait_states {
                            //trace_print!(self, "Suppressing wait states!");
                            self.io_wait_states = 0;
                            self.bus_wait_states = 0;
                        }

                        // A prefetch decision is made at the end of T2 of the last bus cycle of an atomic
                        // bus operation, regardless of wait states.
                        if self.final_transfer {
                            self.biu_make_fetch_decision();
                        }
                    }
                    TCycle::T3 => {
                        if self.is_last_wait_t3tw() {
                            // Do bus transfer on T3 if no wait states.
                            self.biu_do_bus_transfer();
                            self.ready = true;
                        }
                    }
                    TCycle::Tw => {
                        if self.is_last_wait_t3tw() {
                            self.biu_do_bus_transfer();
                            self.ready = true;
                        }
                    }
                    TCycle::T4 => {
                        // If we just completed a code fetch, make the byte available in the queue.
                        if let BusStatus::CodeFetch = self.bus_status_latch {
                            match self.bus_width {
                                BusWidth::Byte => {
                                    self.pc = self.pc.wrapping_add(self.queue.push8(self.data_bus as u8));
                                }
                                BusWidth::Word => self.pc = self.pc.wrapping_add(self.queue.push16(self.data_bus)),
                            }
                        }

                        if self.final_transfer {
                            self.biu_make_fetch_decision();
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
            self.do_cycle_trace();
        }

        #[cfg(any(feature = "cpu_validator", feature = "cpu_collect_cycle_states"))]
        {
            let cycle_state = self.get_cycle_state();
            self.cycle_states.push(cycle_state);
        }

        // Do DRAM refresh (DMA channel 0) simulation
        if self.enable_wait_states && self.dram_refresh_simulation {
            self.tick_dma();
        }

        // Advance fetch delay counter
        if let FetchState::Delayed(delay) = &mut self.fetch_state {
            if self.t_cycle != TCycle::Tw {
                *delay = delay.saturating_sub(1);
            }
        }

        // Transition to next Ta state
        self.ta_cycle = match self.ta_cycle {
            TaCycle::Tr => {
                // We can always proceed from Tr to Ts.
                TaCycle::Ts
            }
            TaCycle::Ts => {
                // Always proceed from Ts to T0(?).
                TaCycle::T0
            }
            TaCycle::T0 => {
                // We can proceed from T0 to Td on T4 if the pending bus cycle is not a code fetch.
                match (self.pl_status, self.bus_pending) {
                    (BusStatus::CodeFetch, BusPendingType::None) => {
                        // We can immediately end the address cycle on Ti or T4 if there is no pending eu request.
                        if matches!(self.t_cycle, TCycle::Ti | TCycle::T4)
                            && !matches!(self.fetch_state, FetchState::Suspended | FetchState::Halted)
                        {
                            self.biu_bus_begin_fetch();
                            TaCycle::Td
                        }
                        else {
                            TaCycle::T0
                        }
                    }
                    (BusStatus::CodeFetch, BusPendingType::EuLate) => {
                        // We have a late EU bus request. We will abort the code fetch, but only
                        // on T4. We will do nothing on T3; this implements the prefetch abort "delay".
                        if matches!(self.t_cycle, TCycle::Ti | TCycle::T4) {
                            self.biu_fetch_abort();
                            self.ta_cycle
                        }
                        else {
                            TaCycle::T0
                        }
                    }
                    _ => {
                        // Not a code fetch - no abort handling required. Begin the next bus cycle at Ti or T4.
                        if matches!(self.t_cycle, TCycle::Ti | TCycle::T4) {
                            self.t_cycle = TCycle::Tinit;
                            TaCycle::Td
                        }
                        else {
                            TaCycle::T0
                        }
                    }
                }
            }
            TaCycle::Td => TaCycle::Td,
            TaCycle::Ta => TaCycle::Ta,
        };

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
                if self.have_wait_states() {
                    self.ready = false;
                }
                TCycle::T3
            }
            TCycle::Tw | TCycle::T3 => {
                // If no wait states have been reported, advance to T3, otherwise go to Tw
                if self.have_wait_states() {
                    // First drain bus wait states
                    self.bus_wait_states = self.bus_wait_states.saturating_sub(1);

                    // Only drain IO wait states when DMA is not active. When DMA is on, the 8288
                    // outputs are suppressed. This means that IO and DMA wait states cannot overlap -
                    // an IO device would gain no benefit from the wait state it can't see or detect
                    if self.dma_wait_states == 0 {
                        self.io_wait_states = self.io_wait_states.saturating_sub(1);
                    }
                    //self.io_wait_states = self.io_wait_states.saturating_sub(1);

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

        // Reset queue operation
        self.last_queue_op = self.queue_op;
        self.last_queue_byte = self.queue_byte;
        self.queue_op = QueueOp::Idle;

        self.instr_cycle += 1;
        self.device_cycles += 1;

        self.cycle_num += 1;
        if self.cycle_num & 1 == 0 {
            self.clk0 = !self.clk0;
        }
        self.dma_wait_states = self.dma_wait_states.saturating_sub(1);

        if !self.have_wait_states() {
            self.ready = true;
        }

        self.last_queue_len = self.queue.len();
    }

    /// Advance the DMA scheduler by one tick. This function is called every CPU tick. Since it is
    /// only called from within cycle_i() it can be inlined.
    #[inline(always)]
    pub fn tick_dma(&mut self) {
        self.dram_refresh_cycle_num = self.dram_refresh_cycle_num.saturating_sub(1);

        // Reset scheduler at terminal count
        if self.dram_refresh_cycle_num == 0 && !self.dram_refresh_tc {
            // The DACK0 signal suppresses generation of DREQ0, but we advance DMA state after this.
            // So we will use HOLDA as the suppression signal to give us one cycle advance notice of !DACK0.
            self.dram_refresh_tc = true;
            self.dma_req = !self.dma_holda;

            if self.dram_refresh_retrigger {
                self.dram_refresh_tc = false;
                self.dram_refresh_cycle_num = self.dram_refresh_cycle_period;
            }
        }

        match &mut self.dma_state {
            DmaState::Idle => {
                if self.dma_req {
                    // DRAM refresh cycle counter has hit terminal count.
                    // Begin DMA transfer simulation by entering DREQ state.
                    self.dma_state = DmaState::Dreq;
                }
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

                if (self.bus_status == BusStatus::Passive)
                    || matches!(self.t_cycle, TCycle::T3 | TCycle::Tw | TCycle::T4)
                {
                    // S0 & S1 are idle. Issue hold acknowledge if LOCK not asserted.
                    if !self.lock {
                        self.dma_state = DmaState::HoldA;
                        self.dma_holda = true;
                    }
                }
            }
            DmaState::HoldA => {
                // DMA Hold Acknowledge has been issued. DMA controller will enter S1
                // on next cycle, if no bus wait states are present.
                if self.bus_wait_states < 2 && self.io_wait_states < 2 {
                    //if self.bus_wait_states < 2 {
                    self.dma_state = DmaState::Operating(0);
                    self.dma_aen = true;
                }
            }
            DmaState::Operating(cycles) => {
                // the DMA controller has control of the bus now.
                // Run DMA transfer cycles.
                *cycles += 1;
                match *cycles {
                    1 => {
                        // DMAWAIT asserted after S1
                        self.dma_wait_states = 7; // Effectively 6 as this is decremented this cycle
                        self.ready = false;
                    }
                    2 => {
                        // DACK asserted after S2
                        self.dma_req = false;
                        self.dma_ack = true;
                    }
                    4 => {
                        // Transfer cycles have elapsed, so move to end state.
                        self.dma_holda = false;
                    }
                    5 => {
                        self.dma_aen = false;
                        self.dma_ack = false;
                        self.dma_state = DmaState::Idle;
                    }
                    _ => {}
                }
            }
            DmaState::End => {
                // DMA transfer has completed. Deassert DACK and reset state to idle.
                self.dma_state = DmaState::Idle;
            }
        }
    }

    #[inline]
    pub fn cycles(&mut self, ct: u32) {
        for _ in 0..ct {
            self.cycle();
        }
    }

    #[inline]
    pub fn cycles_i(&mut self, ct: u32, instrs: &[u16]) {
        assert!(ct as usize <= instrs.len());
        for mc_i in instrs.iter().take(ct as usize) {
            self.cycle_i(*mc_i);
        }
    }

    #[cfg(any(feature = "cpu_validator", feature = "cpu_collect_cycle_states"))]
    pub fn get_cycle_states_internal(&self) -> &Vec<CycleState> {
        &self.cycle_states
    }
}

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

    cpu_vx0::cycle.rs

    Contains functions for cycling the cpu through its various states.
    Cycle functions are called by instructions and bus logic whenever
    the CPU should be ticked.

*/

use crate::{cpu_common::QueueOp, cpu_vx0::*};

#[cfg(feature = "cpu_validator")]
use crate::cpu_validator::{BusType, ReadType};

#[cfg(any(feature = "cpu_validator", feature = "cpu_collect_cycle_states"))]
use crate::cpu_validator::CycleState;

macro_rules! validate_read_u8 {
    ($myself: expr, $addr: expr, $data: expr, $btype: expr, $rtype: expr) => {{
        #[cfg(feature = "cpu_validator")]
        if let Some(ref mut validator) = &mut $myself.validator {
            validator.emu_read_byte($addr, $data, $btype, $rtype)
        }
    }};
}

macro_rules! validate_read_u16 {
    ($myself: expr, $addr: expr, $data: expr, $btype: expr, $rtype: expr) => {{
        #[cfg(feature = "cpu_validator")]
        if let Some(ref mut validator) = &mut $myself.validator {
            validator.emu_read_word($addr, $data, $btype, $rtype)
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

#[allow(unused_macros)]
macro_rules! validate_write_u16 {
    ($myself: expr, $addr: expr, $data: expr, $btype: expr) => {{
        #[cfg(feature = "cpu_validator")]
        if let Some(ref mut validator) = &mut $myself.validator {
            validator.emu_write_word($addr, $data, $btype)
        }
    }};
}

impl NecVx0 {
    #[inline(always)]
    pub fn cycle(&mut self) {
        self.cycle_i(MC_NONE);
    }

    /// Execute a CPU cycle.
    /// 'instr' is the microcode line reference of the cycle being executed, if applicable
    /// (otherwise it should be passed MC_NONE).
    /// The CPU will transition between T-states, execute bus transfers on T3 or TW-last,
    /// and otherwise do all necessary actions to advance the cpu state.
    pub fn cycle_i(&mut self, _instr: u16) {
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
                if let FetchState::Delayed(0) = self.fetch_state {
                    self.biu_make_fetch_decision();
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
                        else if self.bus_wait_states > 0 {
                            self.ready = false;
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
                            self.do_bus_transfer();
                            self.ready = true;
                        }
                    }
                    TCycle::Tw => {
                        if self.is_last_wait_t3tw() {
                            self.do_bus_transfer();
                            self.ready = true;
                        }
                    }
                    TCycle::T4 => {
                        // If we just completed a code fetch, make the byte available in the queue.
                        if let BusStatus::CodeFetch = self.bus_status_latch {
                            self.queue.push8(self.data_bus as u8);
                            self.pc = self.pc.wrapping_add(1);
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

        #[cfg(feature = "cpu_validator")]
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
                // We can proceed from Ts to T0 if there is not a late EU bus request.
                /*                match (self.pl_status, self.bus_pending) {
                    (BusStatus::CodeFetch, BusPendingType::EuLate) => TaCycle::Tr,
                    _ => TaCycle::T0,
                }*/

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
                            self.biu_fetch_bus_begin();
                            TaCycle::Td
                        }
                        else {
                            TaCycle::T0
                        }
                    }
                    (BusStatus::CodeFetch, BusPendingType::EuLate) => {
                        // We have a late EU bus request. We will abort the code fetch, but only
                        // on T4. We will do nothing on T3; this implements the prefetch abort delay.
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

        if self.wait_states == 0 && self.dma_wait_states == 0 {
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
                    || match self.t_cycle {
                        TCycle::T3 | TCycle::Tw | TCycle::T4 => true,
                        _ => false,
                    }
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
                // on next cycle.
                self.dma_state = DmaState::Operating(0);
                self.dma_aen = true;
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

                validate_read_u16!(self, self.address_latch, self.data_bus, BusType::Mem, ReadType::Code);
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
                    None,
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

    #[cfg(any(feature = "cpu_validator", feature = "cpu_collect_cycle_states"))]
    pub fn get_cycle_states_internal(&self) -> &Vec<CycleState> {
        &self.cycle_states
    }
}

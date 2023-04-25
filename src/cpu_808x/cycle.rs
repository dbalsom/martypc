/*
    MartyPC Emulator 
    (C)2023 Daniel Balsom
    https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.


    cpu_808x::cycle.rs

    Contains functions for cycling the cpu through its various states.
    Cycle functions are called by instructions and bus logic whenever
    the CPU should be ticked.

*/

use crate::cpu_808x::*;
use crate::cpu_808x::biu::*;
use crate::cpu_808x::addressing::*;


impl<'a> Cpu<'a> {
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

        let byte;

        self.trace_instr = instr;

        if self.t_cycle == TCycle::TInit {
            self.t_cycle = TCycle::T1;
        }

        // Operate current t-state
        match self.bus_status {
            BusStatus::Passive => {
                self.transfer_n = 0;
            }
            BusStatus::MemRead | BusStatus::MemWrite | BusStatus::IORead | BusStatus::IOWrite | BusStatus::CodeFetch => {
                match self.t_cycle {
                    TCycle::TInit => {
                        panic!("Can't execute TInit state");
                    },
                    TCycle::T1 => {
                    },
                    TCycle::T2 => {
                        // Turn off ale signal on T2
                        self.i8288.ale = false;

                        // Read/write signals go high on T2.
                        match self.bus_status {
                            BusStatus::CodeFetch | BusStatus::MemRead => {
                                self.i8288.mrdc = true;
                            }
                            BusStatus::MemWrite => {
                                // Only AMWC goes high on T2. MWTC delayed to T3.
                                self.i8288.amwc = true;
                            }
                            BusStatus::IORead => {
                                self.i8288.iorc = true;
                            }
                            BusStatus::IOWrite => {
                                // Only AIOWC goes high on T2. IOWC delayed to T3.
                                self.i8288.aiowc = true;
                            }
                            _ => {}
                        }

                        match self.bus_status {
                            BusStatus::CodeFetch | BusStatus::MemRead => {
                                self.bus_wait_states = self.bus.get_read_wait(self.address_bus as usize, self.instr_elapsed).unwrap();
                                self.instr_elapsed = 0;
                            }
                            BusStatus::MemWrite => {
                                self.bus_wait_states = self.bus.get_write_wait(self.address_bus as usize, self.instr_elapsed).unwrap();
                                self.instr_elapsed = 0;
                            }
                            BusStatus::IORead => {
                                self.bus_wait_states = 1;
                            }
                            BusStatus::IOWrite => {
                                self.bus_wait_states = 1;
                            }                                                                                                                     
                            _=> {
                                log::warn!("Unhandled bus status: {:?}!", self.bus_status);
                            }
                        }
                    }
                    TCycle::T3 => {
                        // Reading/writing occurs on T3. The READY handshake is not simulated, instead the BusInterface
                        // methods will return the number of wait states appropriate for each read/write.

                        match (self.bus_status, self.transfer_size) {
                            (BusStatus::CodeFetch, TransferSize::Byte) => {
                                (byte, _) = self.bus.read_u8(self.address_bus as usize, self.instr_elapsed).unwrap();
                                self.instr_elapsed = 0;
                                self.wait_states += self.bus_wait_states;
                                
                                self.data_bus = byte as u16;
                                self.transfer_n += 1;
                            }
                            (BusStatus::CodeFetch, TransferSize::Word) => {
                                (self.data_bus, _) = self.bus.read_u16(self.address_bus as usize, self.instr_elapsed).unwrap();
                                self.instr_elapsed = 0;
                                self.wait_states += self.bus_wait_states;
                                
                                self.transfer_n += 1;
                            }
                            (BusStatus::MemRead, TransferSize::Byte) => {
                                (byte, _) = self.bus.read_u8(self.address_bus as usize, self.instr_elapsed).unwrap();
                                self.instr_elapsed = 0;
                                self.wait_states += self.bus_wait_states;
                                
                                self.data_bus = byte as u16;
                                self.transfer_n += 1;
                            }                            
                            (BusStatus::MemRead, TransferSize::Word) => {
                                (self.data_bus, _) = self.bus.read_u16(self.address_bus as usize, self.instr_elapsed).unwrap();
                                self.instr_elapsed = 0;
                                self.wait_states += self.bus_wait_states;
                                
                                self.transfer_n += 1;
                            }                         
                            (BusStatus::MemWrite, TransferSize::Byte) => {
                                self.i8288.mwtc = true;
                                _ = 
                                    self.bus.write_u8(
                                        self.address_bus as usize, 
                                        (self.data_bus & 0x00FF) as u8, 
                                        self.instr_elapsed
                                    ).unwrap();
                                self.instr_elapsed = 0;
                                self.wait_states += self.bus_wait_states;
                                
                                self.transfer_n += 1;
                            }
                            (BusStatus::MemWrite, TransferSize::Word) => {
                                self.i8288.mwtc = true;
                                _ = self.bus.write_u16(self.address_bus as usize, self.data_bus, self.instr_elapsed).unwrap();
                                self.instr_elapsed = 0;
                                self.wait_states += self.bus_wait_states;
                                
                                self.transfer_n += 1;
                            }
                            (BusStatus::IORead, TransferSize::Byte) => {
                                byte = self.bus.io_read_u8((self.address_bus & 0xFFFF) as u16);
                                self.wait_states += 1;
                                self.data_bus = byte as u16;
                                self.transfer_n += 1;
                            }
                            (BusStatus::IOWrite, TransferSize::Byte) => {
                                self.i8288.iowc = true;
                                self.bus.io_write_u8((self.address_bus & 0xFFFF) as u16, (self.data_bus & 0x00FF) as u8);
                                self.wait_states += 1;
                                self.transfer_n += 1;
                            }                                                                                                                     
                            _=> {
                                log::warn!("Unhandled bus status: {:?}!", self.bus_status);
                            }
                        }

                        if !self.enable_wait_states {
                            // No wait states for you!
                            self.wait_states = 0;
                        }

                        if self.is_last_wait() && self.is_operand_complete() {
                            self.biu_make_fetch_decision();
                        }
                    }
                    TCycle::Tw => {
                        if self.is_last_wait() && self.is_operand_complete() {
                            self.biu_make_fetch_decision();
                        }                        
                    }
                    TCycle::T4 => {
                        match (self.bus_status, self.transfer_size) {
                            (BusStatus::CodeFetch, TransferSize::Byte) => {
                                //log::debug!("Code fetch completed!");
                                //log::debug!("Pushed byte {:02X} to queue!", self.data_bus as u8);
                                self.queue.push8(self.data_bus as u8);
                                self.pc = (self.pc + 1) & 0xFFFFFu32;
                            }
                            (BusStatus::CodeFetch, TransferSize::Word) => {
                                self.queue.push16(self.data_bus);
                                self.pc = (self.pc + 2) & 0xFFFFFu32;
                            }
                            _=> {}                        
                        }
                    }
                }
            }
            _ => {
                // Handle other states
            }
        };

        // Perform cycle tracing, if enabled
        if self.trace_enabled && self.trace_mode == TraceMode::Cycle {
            self.trace_print(&self.cycle_state_string(false));   
            self.trace_str_vec.push(self.cycle_state_string(true));
        }

        #[cfg(feature = "cpu_validator")]
        {
            let cycle_state = self.get_cycle_state();
            self.cycle_states.push(cycle_state);
        }

        // Transition to next T state
        self.t_cycle = match self.t_cycle {
            TCycle::TInit => {
                // A new bus cycle has been initiated, begin it in T1.
                TCycle::T1
            }
            TCycle::T1 => {
                // If bus status is PASV, stay in T1 (no bus transfer occurring)
                // Otherwise if there is a valid bus status on T1, transition to T2.
                match self.bus_status {
                    BusStatus::Passive => TCycle::T1,
                    BusStatus::MemRead | BusStatus::MemWrite | BusStatus::IORead | BusStatus::IOWrite | BusStatus::CodeFetch => {
                        TCycle::T2
                    },
                    _=> self.t_cycle
                }
            }
            TCycle::T2 => TCycle::T3,
            TCycle::T3 => {
                // If no wait states have been reported, advance to T3, otherwise go to Tw
                if self.wait_states == 0 {
                    self.biu_bus_end();
                    TCycle::T4
                }
                else {
                    TCycle::Tw
                }
            }
            TCycle::Tw => {
                // If we are handling wait states, continue in Tw (decrement at end of cycle)
                // If we have handled all wait states, transition to T4
                if self.wait_states > 0 {
                    //log::debug!("wait states: {}", self.wait_states);
                    TCycle::Tw
                }
                else {
                    self.biu_bus_end();
                    TCycle::T4
                }                
            }
            TCycle::T4 => {
                // We reached the end of a bus transfer, to transition back to T1 and PASV.
                self.bus_status = BusStatus::Passive;
                TCycle::T1
            }            
        };

        // Handle prefetching
        self.biu_tick_prefetcher();

        match self.fetch_state {
            FetchState::Scheduled(n) if n > 1 => {
                //self.trace_print("Scheduled fetch!");
                if !self.fetch_suspended {
                    if self.biu_queue_has_room() {

                        //trace_print!(self, "Fetch started");
                        self.fetch_state = FetchState::InProgress;
                        self.bus_status = BusStatus::CodeFetch;
                        self.bus_segment = Segment::CS;
                        self.t_cycle = TCycle::T1;
                        self.address_bus = self.pc;
                        self.i8288.ale = true;
                        self.data_bus = 0;
                        self.transfer_size = self.fetch_size;
                        self.operand_size = match self.fetch_size {
                            TransferSize::Byte => OperandSize::Operand8,
                            TransferSize::Word => OperandSize::Operand16
                        };
                        self.transfer_n = 0;
                    }
                    else if !self.bus_pending_eu {
                        /*
                        // Cancel fetch if queue is full and no pending bus request from EU that 
                        // would otherwise trigger an abort.
                        self.fetch_state = FetchState::Idle;
                        trace_print!(self, "Fetch cancelled. bus_pending_eu: {}", self.bus_pending_eu);
                        */
                    }
                }
            }
            FetchState::Idle => {
                if self.queue_op == QueueOp::Flush {
                    //trace_print!(self, "Flush scheduled fetch!");
                    self.biu_schedule_fetch();
                }
                if (self.bus_status == BusStatus::Passive) && (self.t_cycle == TCycle::T1) {
                    // Nothing is scheduled, suspended, aborted, and bus is idle. Make a prefetch decision.
                    //self.biu_make_fetch_decision();
                }
            }
            _ => {}
        } 

        // Reset queue operation
        self.last_queue_op = self.queue_op;
        self.queue_op = QueueOp::Idle;

        self.instr_cycle += 1;
        self.instr_elapsed += 1;
        self.cycle_num += 1;
        
        self.wait_states = self.wait_states.saturating_sub(1);

        // Do DRAM refresh (DMA channel 0) simulation
        if self.dram_refresh_simulation {
            self.dram_refresh_cycles += 1;

            match &mut self.dma_state {
                DmaState::Idle => {
                    if self.dram_refresh_cycles == self.dram_refresh_cycle_target {
                        // DRAM refresh cycle counter has hit target. 
                        // Begin DMA transfer simulation by issuing a DREQ.
                        self.dma_state = DmaState::Dreq;

                        // Reset counter.
                        self.dram_refresh_cycles = 0;
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
                    // indicates PASV or HALT. (1,1) (Note, bus goes PASV after T2)

                    if self.bus_status == BusStatus::Passive || 
                        match self.t_cycle {
                            TCycle::T3 | TCycle::Tw | TCycle::T4 => true,
                            _ => false
                        }
                    {
                        // S0 & S1 are idle. Issue hold acknowledge.
                        self.dma_state = DmaState::HoldA;
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
                        self.wait_states += 6;
                        //self.wait_states += 6_u32.saturating_sub(self.wait_states);
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

        self.trace_comment = ""; 
        self.trace_instr = MC_NONE;
    }

    #[inline]
    pub fn cycle_nx(&self) {
        // Do nothing
    }

    #[inline]
    pub fn cycle_nx_i(&self, _instr: u16) {
        // Do nothing
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
    }

    #[inline]
    pub fn cycles_nx_i(&mut self, ct: u32, instrs: &[u16]) {
        self.cycles_i(ct - 1, instrs);
    }
}
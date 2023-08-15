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

    cpu_808x::biu.rs

    Implement CPU behavior specific to the BIU (Bus Interface Unit)

*/
use crate::cpu_808x::*;
use crate::bytequeue::*;

pub enum ReadWriteFlag {
    Normal,
    RNI
}

impl ByteQueue for Cpu {
    fn seek(&mut self, _pos: usize) {
        // Instruction queue does not support seeking
    }

    fn tell(&self) -> usize {
        //log::trace!("pc: {:05X} qlen: {}", self.pc, self.queue.len());
        self.pc as usize - (self.queue.len() + (self.queue.has_preload() as usize))
    }

    fn delay(&mut self, delay: u32) {
        self.fetch_delay += delay;
    }

    fn wait(&mut self, cycles: u32) {
        self.cycles(cycles);
    }

    fn wait_i(&mut self, cycles: u32, instr: &[u16]) {
        self.cycles_i(cycles, instr);
    }

    fn wait_comment(&mut self, comment: &'static str) {
        self.trace_comment(comment);
    }

    fn set_pc(&mut self, pc: u16) {
        self.mc_pc = pc;
    }

    fn clear_delay(&mut self) {
        self.fetch_delay = 0;
    }

    fn q_read_u8(&mut self, dtype: QueueType, reader: QueueReader) -> u8 {
        self.biu_queue_read(dtype, reader)
    }

    fn q_read_i8(&mut self, dtype: QueueType, reader: QueueReader) -> i8 {
        self.biu_queue_read(dtype, reader) as i8
    }

    fn q_read_u16(&mut self, dtype: QueueType, reader: QueueReader) -> u16 {
        let lo = self.biu_queue_read(dtype, reader);
        let ho = self.biu_queue_read(QueueType::Subsequent, reader);
        
        (ho as u16) << 8 | (lo as u16)
    }

    fn q_read_i16(&mut self, dtype: QueueType, reader: QueueReader) -> i16 {
        let lo = self.biu_queue_read(dtype, reader);
        let ho = self.biu_queue_read(QueueType::Subsequent, reader);
        
        ((ho as u16) << 8 | (lo as u16)) as i16
    }

    fn q_peek_u8(&mut self) -> u8 {
        let (byte, _cost) = self.bus.read_u8(self.pc as usize - self.queue.len(), 0).unwrap();
        byte
    }

    fn q_peek_i8(&mut self) -> i8 {
        let (byte, _cost) = self.bus.read_u8(self.pc as usize - self.queue.len(), 0).unwrap();
        byte as i8
    }

    fn q_peek_u16(&mut self) -> u16 {
        let (word, _cost) = self.bus.read_u16(self.pc as usize - self.queue.len(), 0).unwrap();
        word
    }    

    fn q_peek_i16(&mut self) -> i16 {
        let (word, _cost) = self.bus.read_u16(self.pc as usize - self.queue.len(), 0).unwrap();
        word as i16
    }

    fn q_peek_farptr16(&mut self) -> (u16, u16) {
        let read_offset = self.pc as usize - self.queue.len();

        let (offset, _cost) = self.bus.read_u16(read_offset, 0).unwrap();
        let (segment, _cost) = self.bus.read_u16(read_offset + 2, 0).unwrap();
        (segment, offset)
    }
}


impl Cpu {

    /// Read a byte from the instruction queue.
    /// Either return a byte currently in the queue, or fetch a byte into the queue and 
    /// then return it.
    /// 
    /// Regardless of 8088 or 8086, the queue is read from one byte at a time.
    /// 
    /// QueueType is used to set the QS status lines for first/subsequent byte fetches.
    /// QueueReader is used to advance the microcode instruction if the queue read is
    /// from the EU executing an instruction. The BIU reading the queue to fetch an
    /// instruction will not advance the microcode PC.
    pub fn biu_queue_read(&mut self, dtype: QueueType, reader: QueueReader) -> u8 {

        let byte;
        //trace_print!(self, "biu_queue_read()");

        if let Some(preload_byte) = self.queue.get_preload() {
            // We have a pre-loaded byte from finalizing the last instruction.
            self.last_queue_op = QueueOp::First;
            self.last_queue_byte = preload_byte;

            // Since we have a pre-loaded fetch, the next instruction will always begin 
            // execution on the next cycle. If NX bit is set, advance the MC PC to 
            // execute the RNI from the previous instruction.
            self.next_mc();
            if self.nx {
                self.nx = false;
            }   

            return preload_byte
        }

        if self.queue.len() > 0 {
            // The queue has an available byte. Return it.

            // Handle fetch delays.
            // Delays are set during decode from instructions with no modrm
            while self.fetch_delay > 0 {
                //log::trace!("Fetch delay skip: {}", self.fetch_delay);
                self.fetch_delay -= 1;
                self.trace_comment("fetch delay");
                self.cycle();
            }

            //self.trace_print("biu_queue_read: pop()");
            //self.trace_comment("Q_READ");
            byte = self.queue.pop();
            self.last_queue_direction = QueueDirection::Read;
            self.biu_fetch_on_queue_read();
        }
        else {
            // Queue is empty, wait for a byte to be fetched into the queue then return it.
            // Fetching is automatic, therefore, just cycle the cpu until a byte appears...
            while self.queue.len() == 0 {
                self.cycle();
            }

            // ...and pop it out.
            byte = self.queue.pop();
            self.last_queue_direction = QueueDirection::Read;         
        }

        self.queue_byte = byte;

        let mut advance_pc = false;

        // TODO: These enums duplicate functionality
        self.queue_op = match dtype {
            QueueType::First => {
                QueueOp::First
            },
            QueueType::Subsequent => {
                match reader {
                    QueueReader::Biu => QueueOp::Subsequent,
                    QueueReader::Eu => {
                        // Advance the microcode PC.
                        advance_pc = true;
                        QueueOp::Subsequent
                    }
                }
            }
        };

        self.cycle();
        if advance_pc {
            if self.nx {
                self.nx = false;
            }
            self.mc_pc += 1;
        }
        byte
    }

    pub fn biu_fetch_on_queue_read(&mut self) {
        // TODO: What if queue is read during transitional state?
        if matches!(self.biu_state_new, BiuStateNew::Idle) && self.queue.len() == 3 {
            self.biu_change_state(BiuStateNew::Prefetch);
            //trace_print!(self, "Transitioning BIU from idle to prefetch due to queue read.");                        
            self.biu_schedule_fetch(3);
        }
    }

    /// This function will cycle the CPU until a byte is available in the instruction queue,
    /// then read it out, to prepare for execution of the next instruction. 
    /// 
    /// We consider this byte 'preloaded' - this does not correspond to a real CPU state
    pub fn biu_fetch_next(&mut self) {

        // Don't fetch if we are in a string instruction that is still repeating.
        if !self.in_rep {
            self.trace_comment("FETCH");
            let mut finalize_timeout = 0;
            
            /*
            if MICROCODE_FLAGS_8088[self.mc_pc as usize] == RNI {
                trace_print!(self, "Executed terminating RNI!");
            }
            */

            if self.queue.len() == 0 {
                while { 
                    if self.nx {
                        self.trace_comment("NX");
                        self.next_mc();
                        self.nx = false;
                        self.rni = false;
                    }
                    self.cycle();
                    self.mc_pc = MC_NONE;
                    finalize_timeout += 1;
                    if finalize_timeout == 20 {
                        self.trace_flush();
                        panic!("FETCH timeout! wait states: {}", self.wait_states);
                    }
                    self.queue.len() == 0
                } {}
                // Should be a byte in the queue now. Preload it
                self.queue.set_preload();
                self.queue_op = QueueOp::First;
                self.last_queue_direction = QueueDirection::Read;
                self.trace_comment("FETCH_END");
                self.cycle();
            }
            else {
                self.queue.set_preload();
                self.queue_op = QueueOp::First;
                self.last_queue_direction = QueueDirection::Read;

                // Check if reading the queue will resume the BIU if stalled.
                self.biu_fetch_on_queue_read();

                if self.nx {
                    self.trace_comment("NX");
                    self.next_mc();
                }

                if self.rni {
                    self.trace_comment("RNI");
                    self.rni = false;
                }
                
                self.trace_comment("FETCH_END");
                self.cycle();
            }
        }
    }

    pub fn biu_suspend_fetch(&mut self) {
        self.trace_comment("SUSP");
        self.fetch_suspended = true;
        self.fetch_state = FetchState::Idle;

        // SUSP waits for any current fetch to complete.
        if self.bus_status == BusStatus::CodeFetch {
            self.biu_bus_wait_finish();
            self.biu_change_state(BiuStateNew::Idle);
            self.cycle();
        }
        else {
            // new state logic: transition to B_idle
            self.biu_change_state(BiuStateNew::Idle);
        }
    }

    /*
    pub fn biu_suspend_fetch_i(&mut self, mc: u16) {
        self.trace_comment("SUSP");
        self.fetch_suspended = true;
        self.fetch_state = FetchState::Idle;
        self.biu_state = BiuState::Suspended;
        // new state logic: transition to B_idle
        self.biu_change_state(BiuStateNew::Idle);

        // SUSP waits for any current fetch to complete.
        if self.bus_status == BusStatus::CodeFetch {
            self.biu_bus_wait_finish();
            
            //self.cycle_i(mc);
        }
    }
    */    

    /// Schedule a prefetch. Depending on queue state, there may be Delay cycles scheduled
    /// that begin after the inital two Scheduled cycles are complete.
    pub fn biu_schedule_fetch(&mut self, ct: u8) {
        if let FetchState::Scheduled(_) = self.fetch_state {
            // Fetch already scheduled, do nothing
            return
        }
        
        // The 8088 applies a 3-cycle fetch delay if:
        //      - We are scheduling a prefetch during a CODE fetch
        //      - The queue length was 3 at the beginning of T3

        /*
        // If we are in some kind of bus transfer (not passive) then add any wait states that 
        // might apply.
        let schedule_adjust = if self.bus_status != BusStatus::Passive {
            self.wait_states as u8
        }
        else {
            0
        };
        */

        if ct == 0 {
            // Schedule count of 0 indicates fetch after bus transfer is complete, ie, ScheduleNext
            if self.bus_status == BusStatus::CodeFetch 
                && (self.queue.len() == 3 || (self.queue.len() == 2 && self.queue_op != QueueOp::Idle)) 
            {
                self.fetch_state = FetchState::ScheduleNext;
                self.next_fetch_state = FetchState::Delayed(3);
            }
            else {
                self.fetch_state = FetchState::ScheduleNext;
                self.next_fetch_state = FetchState::InProgress;
            };
        }
        else {
            
            if self.bus_status == BusStatus::CodeFetch 
                && (self.queue.len() == 3 || (self.queue.len() == 2 && self.queue_op != QueueOp::Idle)) 
            {
                self.fetch_state = FetchState::Scheduled(ct);
                self.next_fetch_state = FetchState::Delayed(3);
            }
            else {
                self.fetch_state = FetchState::Scheduled(ct);
                self.next_fetch_state = FetchState::InProgress;
            };
        }

        // new bus logic: transition to PF state
        self.biu_change_state(BiuStateNew::Prefetch);
    }

    /// Abort a fetch that has just started (on T1) due to an EU bus request on previous
    /// T3 or later. This incurs two penalty cycles.
    pub fn biu_abort_fetch(&mut self) {

        self.fetch_state = FetchState::Aborting(2);
        self.t_cycle = TCycle::T1;
        self.bus_status = BusStatus::Passive;
        self.i8288.ale = false;
        self.trace_comment("ABORT");

        // new bus logic: transition to EU state
        self.biu_change_state(BiuStateNew::Eu);
        
        self.cycles(2);
    }

    /// Abort a scheduled fetch when it cannot be completed because the queue is full.
    pub fn biu_abort_fetch_full(&mut self) {

        // new bus logic: Enter idle state when we can't fetch.
        self.biu_change_state(BiuStateNew::Idle);
        self.fetch_state = FetchState::Idle;
        self.bus_status = BusStatus::Passive;
        self.trace_comment("BIU_IDLE");
    }

    /*
    pub fn biu_try_cancel_fetch(&mut self) {

        match self.fetch_state {
            FetchState::Scheduled(3) => {
                // Fetch was scheduled this cycle, cancel it
                self.trace_comment("CANCEL");

                self.fetch_state = FetchState::BlockedByEU;
            }
            _=> {
                // Can't cancel.
            }
        }
    }
    */

    pub fn biu_queue_flush(&mut self) {
        self.pc -= self.queue.len() as u32;
        self.queue.flush();
        self.queue_op = QueueOp::Flush;
        self.trace_comment("FLUSH");
        self.biu_update_pc();
        
        //trace_print!("Fetch state to idle");
        self.fetch_state = FetchState::Idle;
        self.fetch_suspended = false;

        // new state logic: enter prefetch state
        self.biu_change_state(BiuStateNew::Prefetch);
    }

    pub fn biu_update_pc(&mut self) {
        //log::debug!("Resetting PC to CS:IP: {:04X}:{:04X}", self.cs, self.ip);
        self.pc = Cpu::calc_linear_address(self.cs, self.ip);
    }

    /// Don't adjust the relative PC position, but update the pc for a new value of cs.  
    /// This is used to support worthless instructions like pop cs and mov cs, r/m16.
    pub fn biu_update_cs(&mut self, new_cs: u16) {

        let pc_offset = (self.pc.wrapping_sub((self.cs as u32) << 4)) as u16;

        self.pc = Cpu::calc_linear_address(new_cs, pc_offset);
        self.cs = new_cs;
    }    

    pub fn biu_queue_has_room(&mut self) -> bool {
        match self.cpu_type {
            CpuType::Intel8088 => {
                self.queue.len() < 4
            }
            CpuType::Intel8086 => {
                // 8086 fetches two bytes at a time, so must be two free bytes in queue
                self.queue.len() < 5
            }
        }
    }

    /// This function handles the logic performed by the BIU on T3 of a bus transfer to 
    /// potentially change BIU states.
    pub fn biu_make_biu_decision(&mut self) {

        if (self.queue.len() == 3 && self.queue_op == QueueOp::Idle) || (self.queue.len() == 2 && self.queue_op != QueueOp::Idle) {
            self.trace_comment("THREE");
        }

        if self.fetch_state == FetchState::BlockedByEU {
            // EU has claimed the bus this m-cycle, so transition to EU state.
            self.biu_change_state(BiuStateNew::Eu);
        }
        else {
            // EU has not claimed the bus, attempt to prefetch...
            if self.biu_queue_has_room() {
                if !self.fetch_suspended {
                    self.biu_schedule_fetch(0);
                }
            }
            else {
                // No room in queue for fetch. Transition to idle state!
                self.biu_change_state(BiuStateNew::Idle);
            }
        }
    }

    /// Transition the BIU state machine to a new state. 
    /// We must enter a transitional state to get to the requested state.
    pub fn biu_change_state(&mut self, new_state: BiuStateNew) {

        self.biu_state_new = match (self.biu_state_new, new_state) {
            (BiuStateNew::Idle, BiuStateNew::Eu) => BiuStateNew::ToEu(3),
            (BiuStateNew::Idle, BiuStateNew::Prefetch) => BiuStateNew::ToPrefetch(3),
            (BiuStateNew::Prefetch, BiuStateNew::Idle) => BiuStateNew::ToIdle(1),
            (BiuStateNew::Prefetch, BiuStateNew::Eu) => BiuStateNew::ToEu(2),
            (BiuStateNew::Eu, BiuStateNew::Idle) => BiuStateNew::ToIdle(1),
            (BiuStateNew::Eu, BiuStateNew::Prefetch) => BiuStateNew::ToPrefetch(2),
            _ => self.biu_state_new
        }
    }

    /// Tick the current BIU state machine state and resolve transitional states
    /// when associated timer has expired.
    #[inline]
    pub fn biu_tick_state(&mut self) { 

        self.biu_state_new = match self.biu_state_new {
            BiuStateNew::Idle | BiuStateNew::Prefetch | BiuStateNew::Eu => self.biu_state_new,
            BiuStateNew::ToIdle(_) => BiuStateNew::Idle,
            BiuStateNew::ToPrefetch(ref mut c) => {
                *c = c.saturating_sub(1);
                if *c == 0 {
                    BiuStateNew::Prefetch
                }
                else {
                    BiuStateNew::ToPrefetch(*c)
                }
            }
            BiuStateNew::ToEu(ref mut c) => {
                *c = c.saturating_sub(1);
                if *c == 0 {
                    BiuStateNew::Eu
                }
                else {
                    BiuStateNew::ToEu(*c)
                }
            }
        }
    }

    #[inline]
    pub fn biu_tick_prefetcher(&mut self) {
        match &mut self.fetch_state {
            FetchState::Delayed(c) => {
                *c = c.saturating_sub(1);

                if *c == 0 {
                    // Trigger fetch on expiry of Delayed state.
                    // We reset the next_fetch_state so we don't loop back to Delayed again.
                    self.fetch_state = FetchState::DelayDone;
                    self.next_fetch_state = FetchState::InProgress;
                }                   
            }
            FetchState::Scheduled(c) => {
                *c = c.saturating_sub(1);
            }
            FetchState::Aborting(c) => {
                *c = c.saturating_sub(1);

                if *c == 0 {
                    self.fetch_state = FetchState::Idle;
                }                
            }
            _=> {}
        }
    }

    /// Issue a HALT.  HALT is a unique bus state, and must wait for a free bus cycle to 
    /// begin.
    pub fn biu_halt(&mut self) {

        self.biu_bus_begin(
            BusStatus::Halt,
            Segment::None,
            0,
            0,
            TransferSize::Byte,
            OperandSize::Operand8,
            true
        );

        self.cycle();
    }

    /// Issue an interrupt acknowledge, consisting of two consecutive INTA bus cycles.
    pub fn biu_inta(&mut self, vector: u8) {

        self.biu_bus_begin(
            BusStatus::InterruptAck,
            Segment::None,
            0,
            0,
            TransferSize::Byte,
            OperandSize::Operand16,
            true
        );

        self.biu_bus_wait_finish();

        self.biu_bus_begin(
            BusStatus::InterruptAck,
            Segment::None,
            0,
            vector as u16,
            TransferSize::Byte,
            OperandSize::Operand16,
            false
        );

        self.biu_bus_wait_finish();
    }

    pub fn biu_read_u8(&mut self, seg: Segment, addr: u32) -> u8 {

        self.biu_bus_begin(
            BusStatus::MemRead, 
            seg, 
            addr, 
            0, 
            TransferSize::Byte,
            OperandSize::Operand8,
            true
        );
        let _cycles_waited = self.biu_bus_wait_finish();
        
        //validate_read_u8!(self, addr, (self.data_bus & 0x00FF) as u8, ReadType::Data);

        (self.data_bus & 0x00FF) as u8
    }

    pub fn biu_write_u8(&mut self, seg: Segment, addr: u32, byte: u8, flag: ReadWriteFlag) {

        self.biu_bus_begin(
            BusStatus::MemWrite, 
            seg, 
            addr, 
            byte as u16, 
            TransferSize::Byte,
            OperandSize::Operand8,
            true
        );
        match flag {
            ReadWriteFlag::Normal => self.biu_bus_wait_finish(),
            ReadWriteFlag::RNI => self.biu_bus_wait_until(TCycle::Tw)
        };
        
        //validate_write_u8!(self, addr, (self.data_bus & 0x00FF) as u8);
    }

    pub fn biu_io_read_u8(&mut self, addr: u16) -> u8 {

        self.biu_bus_begin(
            BusStatus::IoRead, 
            Segment::None, 
            addr as u32, 
            0, 
            TransferSize::Byte,
            OperandSize::Operand8,
            true
        );
        let _cycles_waited = self.biu_bus_wait_finish();
        
        //validate_read_u8!(self, addr, (self.data_bus & 0x00FF) as u8, ReadType::Data);

        (self.data_bus & 0x00FF) as u8
    }

    pub fn biu_io_write_u8(&mut self, addr: u16, byte: u8, flag: ReadWriteFlag) {
        
        self.biu_bus_begin(
            BusStatus::IoWrite, 
            Segment::None, 
            addr as u32, 
            byte as u16, 
            TransferSize::Byte,
            OperandSize::Operand8,
            true
        );
        match flag {
            ReadWriteFlag::Normal => self.biu_bus_wait_finish(),
            ReadWriteFlag::RNI => self.biu_bus_wait_until(TCycle::Tw)
        };
        
        //validate_write_u8!(self, addr, (self.data_bus & 0x00FF) as u8);
    }

    pub fn biu_io_read_u16(&mut self, addr: u16, flag: ReadWriteFlag) {
        
        self.biu_bus_begin(
            BusStatus::IoRead, 
            Segment::None, 
            addr as u32, 
            0, 
            TransferSize::Byte,
            OperandSize::Operand16,
            true
        );
        self.biu_bus_wait_finish();

        self.biu_bus_begin(
            BusStatus::IoRead, 
            Segment::None, 
            addr as u32, 
            0, 
            TransferSize::Byte,
            OperandSize::Operand16,
            false
        );

        match flag {
            ReadWriteFlag::Normal => self.biu_bus_wait_finish(),
            ReadWriteFlag::RNI => self.biu_bus_wait_until(TCycle::Tw)
        };
    }        

    pub fn biu_io_write_u16(&mut self, addr: u16, word: u16, flag: ReadWriteFlag) {
        
        self.biu_bus_begin(
            BusStatus::IoWrite, 
            Segment::None, 
            addr as u32, 
            word & 0x00FF,
            TransferSize::Byte,
            OperandSize::Operand16,
            true
        );
        self.biu_bus_wait_finish();

        self.biu_bus_begin(
            BusStatus::IoWrite, 
            Segment::None, 
            addr.wrapping_add(1) as u32, 
            (word >> 8) & 0x00FF, 
            TransferSize::Byte,
            OperandSize::Operand16,
            false
        );

        match flag {
            ReadWriteFlag::Normal => self.biu_bus_wait_finish(),
            ReadWriteFlag::RNI => self.biu_bus_wait_until(TCycle::Tw)
        };
    }    

    pub fn biu_read_u16(&mut self, seg: Segment, addr: u32, flag: ReadWriteFlag) -> u16 {

        let mut word;

        match self.cpu_type {
            CpuType::Intel8088 => {
                // 8088 performs two consecutive byte transfers
                self.biu_bus_begin(
                    BusStatus::MemRead, 
                    seg, 
                    addr, 
                    0, 
                    TransferSize::Byte,
                    OperandSize::Operand16,
                    true
                );
                self.biu_bus_wait_finish();
                word = self.data_bus & 0x00FF;

                //validate_read_u8!(self, addr, (self.data_bus & 0x00FF) as u8, ReadType::Data);

                self.biu_bus_begin(
                    BusStatus::MemRead, 
                    seg, 
                    addr.wrapping_add(1), 
                    0, 
                    TransferSize::Byte,
                    OperandSize::Operand16,
                    false
                );
                match flag {
                    ReadWriteFlag::Normal => self.biu_bus_wait_finish(),
                    ReadWriteFlag::RNI => {
                        // self.bus_wait_until(TCycle::T3)
                        self.biu_bus_wait_finish()
                    }
                };
                word |= (self.data_bus & 0x00FF) << 8;

                //validate_read_u8!(self, addr + 1, (self.data_bus & 0x00FF) as u8, ReadType::Data);
                word
            }
            CpuType::Intel8086 => {
                self.biu_bus_begin(
                    BusStatus::MemRead, 
                    seg, 
                    addr, 
                    0, 
                    TransferSize::Word,
                    OperandSize::Operand16,
                    true
                );
                match flag {
                    ReadWriteFlag::Normal => self.biu_bus_wait_finish(),
                    ReadWriteFlag::RNI => self.biu_bus_wait_until(TCycle::Tw)
                };

                self.data_bus
            }
        }
    }

    pub fn biu_write_u16(&mut self, seg: Segment, addr: u32, word: u16, flag: ReadWriteFlag) {

        match self.cpu_type {
            CpuType::Intel8088 => {
                // 8088 performs two consecutive byte transfers
                self.biu_bus_begin(
                    BusStatus::MemWrite, 
                    seg, 
                    addr, 
                    word & 0x00FF, 
                    TransferSize::Byte,
                    OperandSize::Operand16,
                    true);

                //validate_write_u8!(self, addr, (word & 0x00FF) as u8);

                self.biu_bus_wait_finish();

                self.biu_bus_begin(
                    BusStatus::MemWrite, 
                    seg, 
                    addr.wrapping_add(1), 
                    (word >> 8) & 0x00FF, 
                    TransferSize::Byte,
                    OperandSize::Operand16,
                    false);

                //validate_write_u8!(self, addr + 1, ((word >> 8) & 0x00FF) as u8);

                match flag {
                    ReadWriteFlag::Normal => self.biu_bus_wait_finish(),
                    ReadWriteFlag::RNI => self.biu_bus_wait_until(TCycle::Tw)
                };
            }
            CpuType::Intel8086 => {
                self.biu_bus_begin(
                    BusStatus::MemWrite, 
                    seg, 
                    addr, 
                    word, 
                    TransferSize::Word,
                    OperandSize::Operand16,
                    true);
                match flag {
                    ReadWriteFlag::Normal => self.biu_bus_wait_finish(),
                    ReadWriteFlag::RNI => self.biu_bus_wait_until(TCycle::Tw)
                };
            }
        }

    }    

    /// If in an active bus cycle, cycle the cpu until the bus cycle has reached T4.
    pub fn biu_bus_wait_finish(&mut self) -> u32 {
        let mut bus_cycles_elapsed = 0;
        match self.bus_status {
            BusStatus::Passive => {
                // No active bus transfer
                return 0
            }
            _ => {
                while self.t_cycle != TCycle::T4 {
                    self.cycle();
                    bus_cycles_elapsed += 1;
                }
                return bus_cycles_elapsed
            }
        }
    }

    /// If the fetch state is Delayed(_), wait until it is not.
    pub fn biu_bus_wait_on_delay(&mut self) {
        loop {
            if let FetchState::Delayed(_) = self.fetch_state {
                self.trace_comment("BUS_WAIT_ON_DELAY");
                self.cycle();
            }
            else {
                break
            }
        }
    }

    /// If the BIU state is a transitional state, wait until it is not.
    pub fn biu_wait_for_transition(&mut self) {
        loop {
            match self.biu_state_new {
                BiuStateNew::ToEu(_) | BiuStateNew::ToPrefetch(_) | BiuStateNew::ToIdle(_) => self.cycle(),
                _ => break
            }
        }
    }

    /// If in an active bus cycle, cycle the CPU until the target T-state is reached.
    /// 
    /// This function is usually used on a terminal write to wait for T3-TwLast to 
    /// handle RNI in microcode. The next instruction byte will be fetched on this 
    /// terminating cycle and the beginning of execution will overlap with T4.
    pub fn biu_bus_wait_until(&mut self, target_state: TCycle) -> u32 {
        let mut bus_cycles_elapsed = 0;
        match self.bus_status {
            BusStatus::Passive => {
                // No active bus transfer
                return 0
            }
            BusStatus::MemRead | BusStatus::MemWrite | BusStatus::IoRead | BusStatus::IoWrite | BusStatus::CodeFetch => {
        
                
                if target_state == TCycle::Tw {
                    // Interpret waiting for Tw as waiting for T3 or Last Tw
                    loop {
                        match (self.t_cycle, self.wait_states) {
                            (TCycle::T3, 0) => {
                                self.trace_comment(" >> wait match!");
                                if self.bus_wait_states == 0 {
                                    self.trace_comment(">> no bus_wait_states");
                                    return bus_cycles_elapsed
                                }
                                else {
                                    self.trace_comment(">> wait state!");
                                    self.cycle();
                                }
                            }
                            (TCycle::T3, n) | (TCycle::Tw, n) => {
                                for _ in 0..n {
                                    self.cycle();
                                    bus_cycles_elapsed += 1;
                                }
                                return bus_cycles_elapsed
                            }
                            _ => {
                                self.cycle();
                                bus_cycles_elapsed += 1;
                            }
                        }
                    }
                }
                else {
                    while self.t_cycle != target_state {
                        self.cycle();
                        bus_cycles_elapsed += 1;
                    }
                }
                //self.cycle();
                return bus_cycles_elapsed
            }
            _ => {
                // Handle other statuses
                return 0
            }
        }
    }   

    /// Begins a new bus cycle of the specified type.
    /// 
    /// This is a complex operation; we must wait for the current bus transfer, if any, to complete.
    /// We must process any delays and penalties as appropriate. For example, if we initiate a 
    /// request for a bus cycle when a prefetch is scheduled, we will incur a 2 cycle abort penalty.
    /// 
    /// Note: this function is for EU bus requests only. It cannot start a CODE fetch.
    pub fn biu_bus_begin(
        &mut self, 
        new_bus_status: BusStatus, 
        bus_segment: Segment, 
        address: u32, 
        data: u16, 
        size: TransferSize,
        op_size: OperandSize,
        first: bool,
    ) {

        /*
        trace_print!(
            self,
            "Bus begin! {:?}:[{:05X}] in {:?}", 
            new_bus_status, 
            address, 
            self.t_cycle
        );
        */
        self.trace_comment("BUS_BEGIN");

        // Check this address for a memory access breakpoint
        if self.bus.get_flags(address as usize) & MEM_BPA_BIT != 0 {
            // Breakpoint hit
            self.state = CpuState::BreakpointHit;
        }

        // Save current fetch state
        let _old_fetch_state = self.fetch_state;

        if new_bus_status != BusStatus::CodeFetch {
            // The EU has requested a Read/Write cycle, if we haven't scheduled a prefetch, block 
            // prefetching until the bus transfer is complete.
            
            self.bus_pending_eu = true; 
            match self.fetch_state {
                FetchState::Scheduled(_) | FetchState::Delayed(_) => {
                    // Can't block prefetching if already scheduled.
                }
                _ => {
                    if self.is_before_t3() { //&& !matches!(self.biu_state_new, BiuStateNew::Idle) {
                        // We can prevent any prefetch from being scheduled this cycle by 
                        // if the request comes in before T3/TwLast. This 'claims' the bus
                        // for the EU.

                        //trace_print!(self, "Blocking fetch: T:{:?}", self.t_cycle);
                        self.fetch_state = FetchState::BlockedByEU;
                    }
                }
            }
        }

        // Wait for any current bus cycle to terminate.
        let mut _waited_cycles = self.biu_bus_wait_finish();

        // If there was an active bus cycle, we're now on T4 - tick over to T1 to get 
        // ready to start the new bus cycle. This will trigger the prefetcher, which
        // will use the 'bus_pending_eu' flag to suppress certain behavior
        if self.t_cycle == TCycle::T4 {
            self.cycle();
            _waited_cycles += 1;
        }
        
        // Wait until we have left Resuming biu state (biu_state was BiuState::Resuming)
        //_waited_cycles += self.biu_bus_wait_for_resume();

        // Wait for any transitional BIU state to complete.
        self.biu_wait_for_transition();

        // Wait until we have left Delayed fetch state (fetch_state was FetchState::Delayed)
        self.biu_bus_wait_on_delay();

        // Release our lock on the bus.
        if self.fetch_state == FetchState::BlockedByEU {
            self.fetch_state = FetchState::Idle;
        }
        
        self.bus_pending_eu = false;

        // Set the final_transfer flag if this is the last bus cycle of an atomic bus transfer
        // (word read/writes cannot be interrupted by prefetches)
        if let TransferSize::Word = size {
            self.transfer_n = 1;
            self.final_transfer = true;
        }
        else if first {
            self.final_transfer = match op_size {
                OperandSize::Operand8 => {
                    self.transfer_n = 1;
                    true
                },
                OperandSize::Operand16 => {
                    self.transfer_n = 1;
                    false
                },
                _ => panic!("invalid OperandSize")
            }
        }    
        else {
            // first == false is only possible if doing word transfer on 8088
            self.transfer_n = 2;
            self.final_transfer = true;
        }

        // When we waited for any bus transfer to complete and then ticked to T1, the bus status is now either:
        //      - Passive, in the case no prefetch was scheduled
        //      - CodeFetch, in the case a prefetch was scheduled (and we will need to abort it)
        if self.bus_status == BusStatus::Passive || self.bus_status == BusStatus::CodeFetch || self.t_cycle == TCycle::T4 {

            /* 
            // Consider a prefetch to be scheduled in either the scheduled or delayed fetch states.
            // Fetch can be delayed in circumstances like specific queue length (3 for 8088)
            let fetch_scheduled = match self.fetch_state {
                FetchState::Scheduled(_) => true,
                FetchState::Delayed(3) => true,
                _ => false,
            };
            */

            // Handle the three main BIU states. (Transitional states are invalid at this point)
            match self.biu_state_new {
                BiuStateNew::Eu => {
                    // Nothing to do; we are in the right state
                }
                BiuStateNew::Prefetch => {
                    // We transitioned to the Prefetch state on T3. We need to perform a prefetch abort
                    // (transition back to the EU state)
                    self.biu_abort_fetch();
                }
                BiuStateNew::Idle => {
                    // We transitioned to the Idle state on T3, because prefetching was suspended or the queue was full.
                    // Transition to EU state to begin bus transfer.
                    if new_bus_status == BusStatus::Halt {
                        // There is a one-cycle delay before the Halt status begins.
                        self.cycle();
                    }
                    else if self.transfer_n == 1 {
                        // Only change state on first byte of any bus operation
                        self.biu_change_state(BiuStateNew::Eu);

                        // Execute the transition states.
                        self.cycles(3);
                    }                                    
                }
                _ => {
                    self.trace_flush();
                    panic!("Beginning bus transfer in invalid state: {:?}", self.biu_state_new);
                }
            }
            /*
            match self.biu_state {
                BiuState::Operating => {
                    if new_bus_status != BusStatus::CodeFetch && (fetch_scheduled || self.fetch_state == FetchState::InProgress) {
                        // A fetch was scheduled already, so we have to abort and incur a two cycle penalty.
                        self.biu_abort_fetch(); 
                    }
                }
                BiuState::Suspended => {
                    // The BIU is suspended. Delay the first operation of any bus transfer by 3 cycles.

                    if new_bus_status == BusStatus::Halt {
                        // There is a one-cycle delay before the Halt status begins.
                        self.cycle();
                    }
                    else if self.transfer_n == 1 {
                        // Suspend delays for the BIU only apply to the first byte of a transfer.

                        self.biu_state = BiuState::SDelayed(3);
                        // Claim the bus for the EU.
                        self.fetch_state = FetchState::BlockedByEU;
                        // Execute the SDelayed states.
                        self.cycles(3);
                        // Return to Suspended state after bus op.
                        self.biu_state = BiuState::Suspended; 
                    }
                }
                BiuState::Resuming(_) => {
                    self.trace_flush();
                    unreachable!("Shouldn't be in resuming state on bus request");
                }
                BiuState::SDelayed(_) => {
                    self.trace_flush();
                    unreachable!("Shouldn't be in SDelayed state on bus request");
                }
            }
            */

            // Finally, begin the new bus state.
            self.bus_status = new_bus_status;
            self.bus_segment = bus_segment;
            self.t_cycle = TCycle::Tinit;
            self.address_bus = address;
            self.i8288.ale = true;
            self.data_bus = data as u16;
            self.transfer_size = size;
            self.operand_size = op_size;
        }
        else {
            self.trace_flush();
            panic!("biu_bus_begin: Attempted to start bus cycle in unhandled state: {:?}:{:?}", self.bus_status, self.t_cycle);
        }

    }

    pub fn biu_bus_end(&mut self) {

        // Reset i8288 signals
        self.i8288.mrdc = false;
        self.i8288.amwc = false;
        self.i8288.mwtc = false;
        self.i8288.iorc = false;
        self.i8288.aiowc = false;
        self.i8288.iowc = false;

        //self.bus_pending_eu = false;
    }

}
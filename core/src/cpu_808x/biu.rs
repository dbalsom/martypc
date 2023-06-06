/*
    Marty PC Emulator 
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

    ---------------------------------------------------------------------------

    cpu_808x::biu.rs

    Implement CPU behavior specific to the BIU (Bus Interface Unit)

*/
use crate::cpu_808x::*;
use crate::bytequeue::*;

#[derive (Debug, PartialEq)]
pub enum BiuState {
    Operating,
    Suspended,
    SDelayed(u8),
    Resuming(u8)
}

impl Default for BiuState {
    fn default() -> Self {
        BiuState::Operating
    }
}

pub enum ReadWriteFlag {
    Normal,
    RNI
}

impl ByteQueue for Cpu<'_> {
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


impl<'a> Cpu<'a> {

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
            self.biu_resume_on_queue_read();
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

    pub fn biu_resume_on_queue_read(&mut self) {

        if matches!(self.biu_state, BiuState::Suspended) {
            match self.cpu_type {
                // 8088 will have room in queue at 3 bytes,
                // 8086 will have room in queue at 4 bytes
                CpuType::Intel8088 => {
                    if self.queue.len() == 3 {
                        self.biu_state = BiuState::Resuming(3);
                        trace_print!(self, "Resuming from suspend due to queue read.");
                        self.biu_schedule_fetch();
                    }
                }
                CpuType::Intel8086 => {
                    if self.queue.len() == 4 {
                        // We can fetch again after this read
                        self.biu_state = BiuState::Resuming(3);
                        self.biu_schedule_fetch();
                    }
                }
            }
        }
    }

    /*
    pub fn biu_queue_fetch(&mut self, bus: &mut BusInterface) {

        self.biu_bus_begin(BusStatus::CodeFetch, self.pc, 0, TransferSize::Byte);
        self.bus_wait_finish();
        /*
        let byte;
        let _cost;
        if self.cycle_state == CycleState::T3 {
            (byte, _cost) = bus.read_u8(self.pc as usize).unwrap();
            // TODO: Handle wait states here

            #[cfg(feature = "cpu_validator")]
            {
                // Validator code fetch
                self.validator.as_mut().unwrap().emu_read_byte(self.pc, byte, ReadType::Code);
            }
            // Proceed to T4 and store byte
            self.cycle();
            //self.queue.push();
            self.pc += 1;
        }
        self.bus_status = BusStatus::Passive;
        */
    }
    */

    pub fn biu_suspend_fetch(&mut self) {
        self.trace_comment("SUSP");
        self.fetch_suspended = true;

        // SUSP waits for any current fetch to complete.
        if self.bus_status == BusStatus::CodeFetch {
            self.biu_bus_wait_finish();
            //self.cycle();
        }

        self.biu_state = BiuState::Suspended;
    }

    pub fn biu_suspend_fetch_i(&mut self, mc: u16) {
        self.trace_comment("SUSP");
        self.fetch_suspended = true;

        // SUSP waits for any current fetch to complete.
        if self.bus_status == BusStatus::CodeFetch {
            self.biu_bus_wait_finish();
            self.biu_state = BiuState::Suspended;
            self.cycle_i(mc);
        }
        else {
            self.biu_state = BiuState::Suspended;
        }
        //trace_print!(self, "Suspending BIU");
    }    

    /// Schedule a prefetch to occur after either 2 or 4 cycles, depending on queue
    /// length. If the queue is full, nothing happens.
    pub fn biu_schedule_fetch(&mut self) {
        if let FetchState::Scheduled(_) = self.fetch_state {
            // Fetch already scheduled, do nothing
            return
        }
        
        // The 8088 introduces a 3-cycle scheduling delay when there are 3
        // bytes in the queue.
        // The 8086 introduces a 3-cycle scheduling delay when there are either
        // 3 or 4 bytes in the queue (guessing)

        if self.bus_status == BusStatus::CodeFetch && 
            (
                self.queue.len() == 3 || (self.queue.len() == 2 && self.queue_op != QueueOp::Idle)
            ) 
        {
            self.fetch_state = FetchState::Scheduled(2);
            self.next_fetch_state = FetchState::Delayed(3);
        }
        else {
            self.fetch_state = FetchState::Scheduled(2);
            self.next_fetch_state = FetchState::InProgress;
        };

        /*
        match self.cpu_type {
            CpuType::Intel8088 => {
                match self.queue.len() {
                    0..=2 => self.fetch_state = FetchState::Scheduled(2),
                    3 => self.fetch_state = FetchState::Scheduled(fetch_delay),
                    _ => {}
                }
            }
            CpuType::Intel8086 => {
                match self.queue.len() {
                    0..=2 => self.fetch_state = FetchState::Scheduled(2),
                    3..=4 => self.fetch_state = FetchState::Scheduled(fetch_delay),
                    _ => {}
                }
            }
        }
        */
    }

    /// Abort a scheduled fetch when an EU bus request has been received.
    pub fn biu_abort_fetch(&mut self) {

        self.fetch_state = FetchState::Aborted(2);
        self.t_cycle = TCycle::T1;
        self.bus_status = BusStatus::Passive;
        self.i8288.ale = false;
        self.trace_comment("ABORT");
        self.cycles(2);
    }

    /// Abort a scheduled fetch when it cannot be completed because the queue is full.
    pub fn biu_abort_fetch_full(&mut self) {
        self.biu_state = BiuState::Suspended;
        self.fetch_state = FetchState::Idle;
        self.bus_status = BusStatus::Passive;
        self.trace_comment("BIU_STALL");
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

        // BIU can be resumed after a SUSP by a request from the EU.
        // So don't enter resuming state unless we are still suspended.
        if self.biu_state == BiuState::Suspended {
            self.biu_state = BiuState::Resuming(3);
        }
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

    pub fn biu_make_fetch_decision(&mut self) {
        /*
        if self.biu_queue_has_room() && !self.bus_pending_eu && !self.fetch_suspended {
            self.biu_schedule_fetch();
        }
        */

        if (self.queue.len() == 3 && self.queue_op == QueueOp::Idle) || (self.queue.len() == 2 && self.queue_op != QueueOp::Idle) {
            self.trace_comment("THREE");
        }

        // If the BIU is operating, we can schedule a fetch if the EU does not own the bus
        let can_fetch = match (&self.biu_state, &self.fetch_state) {
            (_, FetchState::BlockedByEU) => false,  // 
            (BiuState::Operating, _) => true,
            _=> false
        };

        /*
        //if self.biu_queue_has_room() && can_fetch && self.queue_op != QueueOp::Flush {
        if can_fetch && self.queue_op != QueueOp::Flush { 
            // 8088 schedules fetch even when queue is full
            self.biu_schedule_fetch();
        }
        */

        if can_fetch {
            self.biu_schedule_fetch();
        }
    }

    #[inline]
    pub fn biu_tick_state(&mut self) {

        if let BiuState::Resuming(ref mut c) = self.biu_state {
            *c = c.saturating_sub(1);

            // Resume BIU if resume operation completed
            if *c == 0 {
                self.biu_state = BiuState::Operating;
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
            FetchState::Aborted(c) => {
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

    /// If the BIU state is not Operating, wait until it is.
    pub fn biu_bus_wait_for_resume(&mut self) -> u32 {
        let mut bus_cycles_elapsed = 0;

        loop {
            match self.biu_state {
                BiuState::Operating => {
                    return bus_cycles_elapsed;
                }
                BiuState::Resuming(_) => {
                    self.cycle();
                    bus_cycles_elapsed += 1;
                    continue;
                }
                BiuState::Suspended => {
                    return 0
                }
                BiuState::SDelayed(_) => {
                    return 0
                }
            }
        }
    }

    /// If the fetch state is Delayed(_), wait until it is not.
    pub fn biu_bus_wait_on_delay(&mut self) -> u32 {
        let mut delay_cycles_elapsed = 0;
        loop {
            if let FetchState::Delayed(_) = self.fetch_state {
                self.trace_comment("BUS_WAIT_ON_DELAY");
                self.cycle();
                delay_cycles_elapsed += 1;
            }
            else {
                break
            }
        }
        delay_cycles_elapsed
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

    /// Begin a new bus cycle of the specified type. Set the address latch and set the data bus appropriately.
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

            if self.transfer_n == 0 {
                // First transfer, advance MC PC to next instruction.
                self.next_mc();
            }
            
            self.bus_pending_eu = true; 
            match self.fetch_state {
                FetchState::Scheduled(_) | FetchState::Delayed(_) => {
                    // Don't block prefetching if already scheduled.
                }
                _ => {
                    if self.is_before_last_wait() && !self.fetch_suspended {
                        //trace_print!(self, "Blocking fetch: T:{:?}", self.t_cycle);
                        self.fetch_state = FetchState::BlockedByEU;
                    }
                }
            }
        }

        // Wait for the current bus cycle to terminate.
        let mut _waited_cycles = self.biu_bus_wait_finish();
        if self.t_cycle == TCycle::T4 {
            self.cycle();
            _waited_cycles += 1;
        }

        // Wait until we have left Resuming bus state.
        _waited_cycles += self.biu_bus_wait_for_resume();
        // Wait until we have left Delayed fetch state.
        _waited_cycles += self.biu_bus_wait_on_delay();

        if self.fetch_state == FetchState::BlockedByEU {
            self.fetch_state = FetchState::Idle;
        }
        
        self.bus_pending_eu = false;

        // Reset the transfer number if this is the first transfer of a word
        if first {
            self.transfer_n = 0;
        }        

        if new_bus_status == BusStatus::CodeFetch {
            // Prefetch is starting so reset prefetch scheculed flag
            self.fetch_state = FetchState::InProgress;
            self.transfer_n = 0;
        }

        if self.bus_status == BusStatus::Passive || self.bus_status == BusStatus::CodeFetch || self.t_cycle == TCycle::T4 {

            let fetch_scheduled = match self.fetch_state {
                FetchState::Scheduled(_) => true,
                FetchState::Delayed(3) => true,
                _ => false,
            };

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
                        self.cycle();
                    }
                    else if self.transfer_n == 0 {
                        self.biu_state = BiuState::SDelayed(3);
                        // Claim the bus for the EU.
                        self.fetch_state = FetchState::BlockedByEU;
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

            self.bus_status = new_bus_status;
            self.bus_segment = bus_segment;
            self.t_cycle = TCycle::TInit;
            self.address_bus = address;
            self.i8288.ale = true;
            self.data_bus = data as u16;
            self.transfer_size = size;
            self.operand_size = op_size;

            if self.transfer_n > 1 {
                self.transfer_n = 0;
            }
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
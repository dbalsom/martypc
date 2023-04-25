
use crate::cpu_808x::*;
use crate::bytequeue::*;

#[cfg(feature = "cpu_validator")]
use crate::cpu_validator::ReadType;

macro_rules! validate_read_u8 {
    ($myself: expr, $addr: expr, $data: expr, $rtype: expr) => {
        {
            #[cfg(feature = "cpu_validator")]
            if let Some(ref mut validator) = &mut $myself.validator {
                validator.emu_read_byte($addr, $data, $rtype)
            }
        }
    };
}

macro_rules! validate_write_u8 {
    ($myself: expr, $addr: expr, $data: expr) => {
        {
            #[cfg(feature = "cpu_validator")]
            if let Some(ref mut validator) = &mut $myself.validator {
                validator.emu_write_byte($addr, $data)
            }
        }
    };
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

    fn clear_delay(&mut self) {
        self.fetch_delay = 0;
    }

    fn q_read_u8(&mut self, dtype: QueueType) -> u8 {
        self.biu_queue_read(dtype)
    }

    fn q_read_i8(&mut self, dtype: QueueType) -> i8 {
        self.biu_queue_read(dtype) as i8
    }

    fn q_read_u16(&mut self, dtype: QueueType) -> u16 {
        let lo = self.biu_queue_read(dtype);
        let ho = self.biu_queue_read(QueueType::Subsequent);
        
        (ho as u16) << 8 | (lo as u16)
    }

    fn q_read_i16(&mut self, dtype: QueueType) -> i16 {
        let lo = self.biu_queue_read(dtype);
        let ho = self.biu_queue_read(QueueType::Subsequent);
        
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
}


impl<'a> Cpu<'a> {

    /// Read a byte from the instruction queue.
    /// Regardless of 8088 or 8086, the queue is read from one byte at a time.
    /// Either return a byte currently in the queue, or fetch a byte into the queue and 
    /// then return it.
    pub fn biu_queue_read(&mut self, dtype: QueueType) -> u8 {
        let byte;

        if let Some(preload_byte) = self.queue.get_preload() {
            // We have a pre-loaded byte from finalizing the last instruction
            self.last_queue_op = QueueOp::First;
            self.cycle();
            self.last_queue_byte = preload_byte;
            return preload_byte
        }

        if self.queue.len() > 0 {
            // The queue is not empty. Return byte from queue.

            self.trigger_prefetch_on_queue_read();

            // Handle fetch delays.
            // Delays are set during decode from instructions with no modrm or jcxz, loop & loopne/loope
            while self.fetch_delay > 0 {
                //log::trace!("Fetch delay skip: {}", self.fetch_delay);
                self.fetch_delay -= 1;
                self.trace_comment("fetch delay");
                self.cycle();
            }

            //self.trace_print("biu_queue_read: pop()");
            self.trace_comment("Q_READ");
            byte = self.queue.pop();

            // TODO: These enums duplicate functionality
            self.queue_op = match dtype {
                QueueType::First => QueueOp::First,
                QueueType::Subsequent => QueueOp::Subsequent
            };

            self.cycle();
            self.last_queue_byte = byte;
        }
        else {
            // Queue is empty, first fetch byte
            byte = self.biu_fetch_u8(dtype);
            self.last_queue_byte = byte;

            //trace_print!(self, "biu_queue_read: cycle()");
            self.cycle();            
        }
        byte
    }

    pub fn trigger_prefetch_on_queue_read(&mut self) {
        match self.cpu_type {
            CpuType::Intel8088 => {
                if self.queue.len() == 4 {
                    // We can fetch again after this read
                    self.biu_schedule_fetch();
                }
            }
            CpuType::Intel8086 => {
                if self.queue.len() == 5 {
                    // We can fetch again after this read
                    self.biu_schedule_fetch();
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
    }

    pub fn biu_schedule_fetch(&mut self) {
        if let FetchState::Scheduled(_) = self.fetch_state {
            // Fetch already scheduled, do nothing
        }
        else {
            self.fetch_state = FetchState::Scheduled(0);
        }
    }

    pub fn biu_abort_fetch(&mut self) {

        self.fetch_state = FetchState::Aborted(0);
        self.t_cycle = TCycle::T1;
        self.bus_status = BusStatus::Passive;
        self.i8288.ale = false;
        self.trace_comment("ABORT");
        self.cycles(2);
    }

    pub fn biu_try_cancel_fetch(&mut self) {

        match self.fetch_state {
            FetchState::Scheduled(0) => {
                // Fetch was scheduled this cycle, cancel it
                self.trace_comment("CANCEL");

                self.fetch_state = FetchState::BlockedByEU;
            }
            _=> {
                // Can't cancel.
            }
        }
    }

    pub fn biu_queue_flush(&mut self) {
        self.pc -= self.queue.len() as u32;
        self.queue.flush();
        self.queue_op = QueueOp::Flush;
        self.trace_comment("FLUSH");
        self.biu_update_pc();
        
        //trace_print!("Fetch state to idle");
        self.fetch_state = FetchState::Idle;
        self.fetch_suspended = false;
    }

    pub fn biu_update_pc(&mut self) {
        //log::trace!("Resetting PC to CS:IP: {:04X}:{:04X}", self.cs, self.ip);
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

        let can_fetch = match self.fetch_state {
            //FetchState::BlockedByEU | FetchState::Suspended => false,
            FetchState::BlockedByEU => false,  // we CAN schedule a fetch while suspended (?)
            _=> true
        };

        //if self.biu_queue_has_room() && can_fetch && self.queue_op != QueueOp::Flush {
        if can_fetch && self.queue_op != QueueOp::Flush { 
            // 8088 schedules fetch even when queue is full
            self.biu_schedule_fetch();
        }
    }

    pub fn biu_tick_prefetcher(&mut self) {
        match &mut self.fetch_state {
            FetchState::Scheduled(c) => {
                *c = c.saturating_add(1);
            }
            FetchState::Aborted(c) => {
                *c = c.saturating_add(1);

                if *c == FETCH_DELAY {
                    self.fetch_state = FetchState::Idle;
                }                
            }
            _=> {}
        }
    }

    pub fn biu_fetch_u8(&mut self, dtype: QueueType) -> u8 {
        // Fetching should be automatic, we shouldn't have to request it.
        // Therefore, just cycle the cpu until there is a byte in the queue and return it.

        while self.queue.len() == 0 {
            self.cycle();
        }

        self.trace_comment("Q_READ");
        let byte = self.queue.pop();
        self.queue_op = match dtype {
            QueueType::First => QueueOp::First,
            QueueType::Subsequent => QueueOp::Subsequent
        };
        //self.cycle(); // It takes 1 cycle to read from the queue.
        
        byte
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
        
        validate_read_u8!(self, addr, (self.data_bus & 0x00FF) as u8, ReadType::Data);

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
        
        validate_write_u8!(self, addr, (self.data_bus & 0x00FF) as u8);
    }

    pub fn biu_io_read_u8(&mut self, addr: u16) -> u8 {

        self.biu_bus_begin(
            BusStatus::IORead, 
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
            BusStatus::IOWrite, 
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

                validate_read_u8!(self, addr, (self.data_bus & 0x00FF) as u8, ReadType::Data);

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

                validate_read_u8!(self, addr + 1, (self.data_bus & 0x00FF) as u8, ReadType::Data);
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

                validate_write_u8!(self, addr, (word & 0x00FF) as u8);

                self.biu_bus_wait_finish();

                self.biu_bus_begin(
                    BusStatus::MemWrite, 
                    seg, 
                    addr.wrapping_add(1), 
                    (word >> 8) & 0x00FF, 
                    TransferSize::Byte,
                    OperandSize::Operand16,
                    false);

                validate_write_u8!(self, addr + 1, ((word >> 8) & 0x00FF) as u8);

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
            BusStatus::MemRead | BusStatus::MemWrite | BusStatus::IORead | BusStatus::IOWrite | BusStatus::CodeFetch => {
                while self.t_cycle != TCycle::T4 {
                    self.cycle();
                    bus_cycles_elapsed += 1;
                }
                return bus_cycles_elapsed
            }
            _ => {
                // Handle other statuses
                return 0
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
            BusStatus::MemRead | BusStatus::MemWrite | BusStatus::IORead | BusStatus::IOWrite | BusStatus::CodeFetch => {
        
                if target_state == TCycle::Tw {
                    // Interpret waiting for Tw as waiting for T3 or Last Tw
                    loop {
                        match (self.t_cycle, self.wait_states) {
                            (TCycle::T3, 0) => {
                                if self.bus_wait_states == 0 {
                                    return bus_cycles_elapsed
                                }
                                else {
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

        // Check this address for a memory access breakpoint
        if self.bus.get_flags(address as usize) & MEM_BPA_BIT != 0 {
            // Breakpoint hit
            self.state = CpuState::BreakpointHit;
        }

        // Save current fetch state
        let old_fetch_state = self.fetch_state;

        if new_bus_status != BusStatus::CodeFetch {
            // The EU has requested a Read/Write cycle, if we haven't scheduled a prefetch, block 
            // prefetching until the bus transfer is complete.

            self.bus_pending_eu = true; 
            if let FetchState::Scheduled(_) = self.fetch_state {
                // Don't block prefetching if already scheduled.
            }
            else if self.is_before_last_wait() && !self.fetch_suspended {

                //trace_print!(self, "Blocking fetch: T:{:?}", self.t_cycle);
                self.fetch_state = FetchState::BlockedByEU;
            }
        }

        // Wait for the current bus cycle to terminate.
        let mut _waited_cycles = self.biu_bus_wait_finish();
        if self.t_cycle == TCycle::T4 {
            self.cycle();
            _waited_cycles += 1;
        }
        
        //trace_print!(self, "biu_bus_begin(): Done waiting for mcycle complete: ({})", waited_cycles);

        if self.fetch_state == FetchState::BlockedByEU {
            self.fetch_state = FetchState::Idle;
        }
        
        self.bus_pending_eu = false;

        // Reset the transfer number if this is the first transfer of a word
        if first {
            self.transfer_n = 0;
        }        
        
        //log::trace!("Bus begin! {:?}:[{:05X}]", bus_status, address);

        if new_bus_status == BusStatus::CodeFetch {
            // Prefetch is starting so reset prefetch scheculed flag
            self.fetch_state = FetchState::InProgress;
            self.transfer_n = 0;
        }

        if self.bus_status == BusStatus::Passive || self.bus_status == BusStatus::CodeFetch || self.t_cycle == TCycle::T4 {

            let fetch_scheduled = if let FetchState::Scheduled(_) = self.fetch_state { true } else { false };

            //self.trace_print(&format!("biu_bus_begin(): fetch is scheduled? {}", fetch_scheduled));
            if new_bus_status != BusStatus::CodeFetch && (fetch_scheduled || self.fetch_state == FetchState::InProgress) {
                // A fetch was scheduled already, so we have to abort and incur a two cycle penalty.
                //self.trace_print("Aborting prefetch!");

                let mut penalty_cycle = false;
                if self.fetch_suspended {
                    // Oddly, suspending prefetch does not avoid a prefetch abort on a bus request if a prefetch was already scheduled.
                    // This costs an extra cycle.
                    penalty_cycle = true;
                }

                // If the prefetcher is unable to fetch after two cycles due to the queue being full, it enters a 'paused' state that
                // incurs a one cycle delay to abort. Detect if our bus request originated during this state.
                if let FetchState::Scheduled(x) = old_fetch_state {
                    if !self.biu_queue_has_room() && (x > 1) {
                        penalty_cycle = true;
                    }
                }

                if penalty_cycle {
                    trace_print!(self, "Stalled prefetch penalty cycle");
                    self.cycle();
                }

                self.biu_abort_fetch(); 
            }
            
            
            self.bus_status = new_bus_status;
            self.bus_segment = bus_segment;
            self.t_cycle = TCycle::TInit;

            //trace_print!(self, "biu_bus_begin(): address {:05X}", address);
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
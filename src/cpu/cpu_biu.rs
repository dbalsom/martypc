use crate::cpu::*;
use crate::bus::BusInterface;
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

    fn q_peek_u8(&self) -> u8 {
        let (byte, _cost) = self.bus.read_u8(self.pc as usize - self.queue.len()).unwrap();
        byte
    }

    fn q_peek_i8(&self) -> i8 {
        let (byte, _cost) = self.bus.read_i8(self.pc as usize - self.queue.len()).unwrap();
        byte
    }

    fn q_peek_u16(&self) -> u16 {
        let (word, _cost) = self.bus.read_u16(self.pc as usize - self.queue.len()).unwrap();
        word
    }    

    fn q_peek_i16(&self) -> i16 {
        let (word, _cost) = self.bus.read_i16(self.pc as usize - self.queue.len()).unwrap();
        word
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
            self.queue_op = QueueOp::First;
            self.cycle();
            self.last_queue_byte = preload_byte;
            return preload_byte
        }

        if self.queue.len() > 0 {
            // The queue is not empty. Return byte from queue.

            // Handle fetch delays.
            // Delays are set during decode from instructions with no modrm or jcxz, loop & loopne/loope
            while self.fetch_delay > 0 {
                log::trace!("Fetch delay skip: {}", self.fetch_delay);
                self.fetch_delay -= 1;
                self.cycle();
            }

            //self.trace_print("biu_queue_read: pop()");
            self.trace_comment("Q_READ");
            byte = self.queue.pop();
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

            //self.trace_print("biu_queue_read: cycle()");
            self.cycle();            
        }
        byte
    }

    /*
    pub fn biu_queue_fetch(&mut self, bus: &mut BusInterface) {

        self.bus_begin(BusStatus::CodeFetch, self.pc, 0, TransferSize::Byte);
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
        //self.fetch_state = FetchState::Suspended;
        self.fetch_suspended = true;
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
                //self.trace_print("CANCEL");
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
        
        self.trace_print("Fetch state to idle");
        self.fetch_state = FetchState::Idle;
        self.fetch_suspended = false;
    }

    pub fn biu_update_pc(&mut self) {
        //log::trace!("Resetting PC to CS:IP: {:04X}:{:04X}", self.cs, self.ip);
        self.pc = Cpu::calc_linear_address(self.cs, self.ip);
    }

    pub fn biu_queue_has_room(&mut self) -> bool {
        match self.cpu_type {
            CpuType::Cpu8088 => {
                self.queue.len() < 4
            }
            CpuType::Cpu8086 => {
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

        if self.biu_queue_has_room() && can_fetch && self.queue_op != QueueOp::Flush {
            self.biu_schedule_fetch();
        }
    }

    pub fn biu_tick_prefetcher(&mut self) {
        match &mut self.fetch_state {
            FetchState::Scheduled(c) if c < &mut 2 => {
                *c += 1;

                if *c == FETCH_DELAY {

                }
            }
            FetchState::Aborted(c) => {
                *c +=1 ;

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

        self.bus_begin(
            BusStatus::MemRead, 
            seg, 
            addr, 
            0, 
            TransferSize::Byte,
            OperandSize::Operand8,
            true
        );
        let cycles_waited = self.bus_wait_finish();
        
        validate_read_u8!(self, addr, (self.data_bus & 0x00FF) as u8, ReadType::Data);

        (self.data_bus & 0x00FF) as u8
        /*
        match self.bus_status {
            BusStatus::CodeFetch => {
                // Abort fetch
                self.bus_status = BusStatus::Passive;        
                self.cycle();

                self.bus_status = BusStatus::Passive;
                (byte, _cost) = self.bus.read_u8(addr as usize).unwrap();
                
                #[cfg(feature = "cpu_validator")]
                self.validator.as_mut().unwrap().emu_read_byte(addr, byte, ReadType::Data);

                // TODO: Handle wait states here
                self.cycles(4);
                self.bus_status = BusStatus::Passive;
            }
            BusStatus::Passive => {
                self.bus_status = BusStatus::MemRead;
                (byte, _cost) = self.bus.read_u8(addr as usize).unwrap();
                
                #[cfg(feature = "cpu_validator")]
                self.validator.as_mut().unwrap().emu_read_byte(addr, byte, ReadType::Data);

                // TODO: Handle wait states here
                self.cycles(4);
                self.bus_status = BusStatus::Passive;
            }
            BusStatus::MemRead | BusStatus::MemWrite => {
                panic!("Overlapping read/write!");
            }
            _ => {
                // Handle other states
                byte = 0;
            }
        }
        
        byte
        */
    }

    pub fn biu_write_u8(&mut self, seg: Segment, addr: u32, byte: u8, flag: ReadWriteFlag) {

        self.bus_begin(
            BusStatus::MemWrite, 
            seg, 
            addr, 
            byte as u16, 
            TransferSize::Byte,
            OperandSize::Operand8,
            true
        );
        match flag {
            ReadWriteFlag::Normal => self.bus_wait_finish(),
            ReadWriteFlag::RNI => self.bus_wait_until(TCycle::T3)
        };
        
        validate_write_u8!(self, addr, (self.data_bus & 0x00FF) as u8);

        /*
        let _result;

        match self.bus_status {
            BusStatus::CodeFetch => {
                // Abort fetch
                self.bus_status = BusStatus::Passive;        
                self.cycle();

                self.bus_status = BusStatus::MemWrite;
                _result = self.bus.write_u8(addr as usize, byte).unwrap();
                // TODO: Handle wait states here

                #[cfg(feature = "cpu_validator")]
                self.validator.as_mut().unwrap().emu_write_byte(addr, byte);


                self.cycles(4);
                self.bus_status = BusStatus::Passive;
            }
            BusStatus::Passive => {
                self.bus_status = BusStatus::MemWrite;
                _result = self.bus.write_u8(addr as usize, byte).unwrap();
                // TODO: Handle wait states here

                #[cfg(feature = "cpu_validator")]
                self.validator.as_mut().unwrap().emu_write_byte(addr, byte);

                self.cycles(4);
                self.bus_status = BusStatus::Passive;
            }
            BusStatus::MemRead | BusStatus::MemWrite => {
                panic!("Overlapping read/write state!");
            }
            _ => {
                // Handle other status            
            }
        }
        */
    }

    pub fn biu_read_u16(&mut self, seg: Segment, addr: u32, flag: ReadWriteFlag) -> u16 {

        let mut word;

        match self.cpu_type {
            CpuType::Cpu8088 => {
                // 8088 performs two consecutive byte transfers
                self.bus_begin(
                    BusStatus::MemRead, 
                    seg, 
                    addr, 
                    0, 
                    TransferSize::Byte,
                    OperandSize::Operand16,
                    true
                );
                self.bus_wait_finish();
                word = self.data_bus & 0x00FF;

                validate_read_u8!(self, addr, (self.data_bus & 0x00FF) as u8, ReadType::Data);

                self.bus_begin(
                    BusStatus::MemRead, 
                    seg, 
                    addr.wrapping_add(1), 
                    0, 
                    TransferSize::Byte,
                    OperandSize::Operand16,
                    false
                );
                match flag {
                    ReadWriteFlag::Normal => self.bus_wait_finish(),
                    ReadWriteFlag::RNI => self.bus_wait_until(TCycle::T3)
                };
                word |= (self.data_bus & 0x00FF) << 8;

                validate_read_u8!(self, addr + 1, (self.data_bus & 0x00FF) as u8, ReadType::Data);
                word
            }
            CpuType::Cpu8086 => {
                self.bus_begin(
                    BusStatus::MemRead, 
                    seg, 
                    addr, 
                    0, 
                    TransferSize::Word,
                    OperandSize::Operand16,
                    true
                );
                match flag {
                    ReadWriteFlag::Normal => self.bus_wait_finish(),
                    ReadWriteFlag::RNI => self.bus_wait_until(TCycle::T3)
                };

                self.data_bus
            }
        }

        /*
        let _cost;

        match self.bus_status {
            BusStatus::CodeFetch => {
                // Abort fetch
                self.bus_status = BusStatus::Passive;        
                self.cycle();

                self.bus_status = BusStatus::MemRead;
                (word, _cost) = self.bus.read_u16(addr as usize).unwrap();
                // TODO: Handle wait states here

                #[cfg(feature = "cpu_validator")]
                {
                    self.validator.as_mut().unwrap().emu_read_byte(addr, (word & 0xFF) as u8, ReadType::Data);
                    self.validator.as_mut().unwrap().emu_read_byte(addr, (word >> 8) as u8, ReadType::Data);
                }
                self.cycles(8);
                self.bus_status = BusStatus::Passive;
            }
            BusStatus::Passive => {
                self.bus_status = BusStatus::MemRead;
                (word, _cost) = self.bus.read_u16(addr as usize).unwrap();
                // TODO: Handle wait states here

                #[cfg(feature = "cpu_validator")]
                {
                    self.validator.as_mut().unwrap().emu_read_byte(addr, (word & 0xFF) as u8, ReadType::Data);
                    self.validator.as_mut().unwrap().emu_read_byte(addr, (word >> 8) as u8, ReadType::Data);
                }
                self.cycles(8);
                self.bus_status = BusStatus::Passive;
            }
            BusStatus::MemRead | BusStatus::MemWrite => {
                panic!("Overlapping read/write!");
                
            }
            _ => {
                // Handle other states
                word = 0;
            }
        }
        word
        */
    }

    pub fn biu_write_u16(&mut self, seg: Segment, addr: u32, word: u16, flag: ReadWriteFlag) {

        match self.cpu_type {
            CpuType::Cpu8088 => {
                // 8088 performs two consecutive byte transfers
                self.bus_begin(
                    BusStatus::MemWrite, 
                    seg, 
                    addr, 
                    word & 0x00FF, 
                    TransferSize::Byte,
                    OperandSize::Operand16,
                    true);

                validate_write_u8!(self, addr, (word & 0x00FF) as u8);

                self.bus_wait_finish();

                self.bus_begin(
                    BusStatus::MemWrite, 
                    seg, 
                    addr.wrapping_add(1), 
                    (word >> 8) & 0x00FF, 
                    TransferSize::Byte,
                    OperandSize::Operand16,
                    false);

                validate_write_u8!(self, addr + 1, ((word >> 8) & 0x00FF) as u8);

                match flag {
                    ReadWriteFlag::Normal => self.bus_wait_finish(),
                    ReadWriteFlag::RNI => self.bus_wait_until(TCycle::T3)
                };
            }
            CpuType::Cpu8086 => {
                self.bus_begin(
                    BusStatus::MemWrite, 
                    seg, 
                    addr, 
                    word, 
                    TransferSize::Word,
                    OperandSize::Operand16,
                    true);
                match flag {
                    ReadWriteFlag::Normal => self.bus_wait_finish(),
                    ReadWriteFlag::RNI => self.bus_wait_until(TCycle::T3)
                };
            }
        }


        /*
        let _result;

        match self.bus_status {
            BusStatus::CodeFetch => {
                // Abort fetch
                self.bus_status = BusStatus::Passive;        
                self.cycle();

                self.bus_status = BusStatus::MemWrite;
                _result = self.bus.write_u16(addr as usize, word).unwrap();
                // TODO: Handle wait states here

                #[cfg(feature = "cpu_validator")]
                {
                    self.validator.as_mut().unwrap().emu_write_byte(addr, (word & 0xFF) as u8);
                    self.validator.as_mut().unwrap().emu_write_byte(addr, (word >> 8) as u8);
                }
                self.cycles(8);
                self.bus_status = BusStatus::Passive;
            }
            BusStatus::Passive => {
                self.bus_status = BusStatus::MemWrite;
                _result = self.bus.write_u16(addr as usize, word).unwrap();
                // TODO: Handle wait states here

                #[cfg(feature = "cpu_validator")]
                {
                    self.validator.as_mut().unwrap().emu_write_byte(addr, (word & 0xFF) as u8);
                    self.validator.as_mut().unwrap().emu_write_byte(addr, (word >> 8) as u8);
                }
                self.cycles(8);
                self.bus_status = BusStatus::Passive;
            }
            BusStatus::MemRead | BusStatus::MemWrite => {
                panic!("Overlapping read/write state!");
            }
            _ => {
                // Handle other states
            }
        }
        */
    }    

}
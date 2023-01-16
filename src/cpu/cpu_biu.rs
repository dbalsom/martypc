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

pub enum WriteFlag {
    Normal,
    RNI
}

impl ByteQueue for Cpu<'_> {
    fn seek(&mut self, _pos: usize) {
        // Instruction queue does not support seeking
    }

    fn tell(&self) -> usize {
        //log::trace!("pc: {:05X} qlen: {}", self.pc, self.queue.len());
        self.pc as usize - self.queue.len()
    }

    fn delay(&mut self, delay: u32) {
        self.fetch_delay += delay;
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
}


impl<'a> Cpu<'a> {

    /// Read a byte from the instruction queue.
    /// Regardless of 8088 or 8086, the queue is read from one byte at a time.
    /// Either return a byte currently in the queue, or fetch a byte into the queue and 
    /// then return it.
    pub fn biu_queue_read(&mut self, dtype: QueueType) -> u8 {
        let byte;

        if self.queue.len() > 0 {
            // The queue is not empty. Return byte from queue.

            // Handle fetch delays.
            // Delays are set during decode from instructions with no modrm or jcxz, loop & loopne/loope
            while self.fetch_delay > 0 {
                log::trace!("Fetch delay skip: {}", self.fetch_delay);
                self.fetch_delay -= 1;
                self.cycle();
            }

            byte = self.queue.pop();

            self.last_queue_op = match dtype {
                QueueType::First => QueueOp::First,
                QueueType::Subsequent => QueueOp::Subsequent
            };
            self.last_queue_byte = byte;
            self.cycle();
        }
        else {
            // Queue is empty, first fetch byte
            byte = self.biu_fetch_u8(self.pc);

            self.last_queue_op = match dtype {
                QueueType::First => QueueOp::First,
                QueueType::Subsequent => QueueOp::Subsequent
            };
            self.last_queue_byte = byte;
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
        self.prefetch_suspended = true;
    }

    pub fn biu_resume_fetch(&mut self) {
        self.prefetch_suspended = false;
    }

    pub fn biu_queue_flush(&mut self) {
        self.pc -= self.queue.len() as u32;
        self.queue.flush();
    }

    pub fn biu_update_pc(&mut self) {
        //log::trace!("Resetting PC to CS:IP: {:04X}:{:04X}", self.cs, self.ip);
        self.pc = Cpu::calc_linear_address(self.cs, self.ip);
    }

    pub fn biu_fetch_u8(&mut self, addr: u32) -> u8 {
        let byte;
        let mut cycles_waited: u32 = 0;
        //let _cost;

        match self.bus_status {
            BusStatus::CodeFetch => {
                // Fetch already in progress?
                // Wait for fetch to complete
                cycles_waited = self.bus_wait_finish();
                self.fetch_delay = self.fetch_delay.saturating_sub(cycles_waited);
                while self.fetch_delay > 0 {
                    // Wait any more remaining fetch delay cycles                    
                    self.cycle();
                    self.fetch_delay -= 1;
                }
                // Byte should be in queue now
                byte = self.queue.pop();
                self.cycle();
            }
            BusStatus::Passive => {
                // Begin fetch
                self.bus_begin(BusStatus::CodeFetch, Segment::CS, self.pc, 0, TransferSize::Byte);
                cycles_waited = self.bus_wait_finish();
                self.fetch_delay = self.fetch_delay.saturating_sub(cycles_waited);
                while self.fetch_delay > 0 {
                    // Wait any more remaining fetch delay cycles
                    self.cycle();
                    self.fetch_delay -= 1;
                }
                // Byte should be in queue now
                byte = self.queue.pop();
                self.cycle();
            }
            _ => {
                // Handle other states
                self.bus_wait_finish();
                self.bus_begin(BusStatus::CodeFetch, Segment::CS, self.pc, 0, TransferSize::Byte);
                cycles_waited = self.bus_wait_finish();
                self.fetch_delay = self.fetch_delay.saturating_sub(cycles_waited);
                while self.fetch_delay > 0 {
                    // Wait any more remaining fetch delay cycles
                    self.cycle();
                    self.fetch_delay -= 1;
                }
                // Byte should be in queue now
                byte = self.queue.pop();
                self.cycle();                
            }                
        }

        byte
    }

    pub fn biu_read_u8(&mut self, seg: Segment, addr: u32) -> u8 {

        self.bus_begin(BusStatus::MemRead, seg, addr, 0, TransferSize::Byte);
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

    pub fn biu_write_u8(&mut self, seg: Segment, addr: u32, byte: u8, flag: WriteFlag) {

        self.bus_begin(BusStatus::MemWrite, seg, addr, byte as u16, TransferSize::Byte);
        match flag {
            WriteFlag::Normal => self.bus_wait_finish(),
            WriteFlag::RNI => self.bus_wait_until(CycleState::T2)
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

    pub fn biu_read_u16(&mut self, seg: Segment, addr: u32) -> u16 {

        let mut word;


        match self.cpu_type {
            CpuType::Cpu8088 => {
                // 8088 performs two consecutive byte transfers
                self.bus_begin(BusStatus::MemRead, seg, addr, 0, TransferSize::Byte);
                self.bus_wait_finish();
                word = self.data_bus & 0x00FF;

                validate_read_u8!(self, addr, (self.data_bus & 0x00FF) as u8, ReadType::Data);

                self.bus_begin(BusStatus::MemRead, seg, addr + 1, 0, TransferSize::Byte);
                self.bus_wait_finish();
                word |= (self.data_bus & 0x00FF) << 8;

                validate_read_u8!(self, addr + 1, (self.data_bus & 0x00FF) as u8, ReadType::Data);
                word
            }
            CpuType::Cpu8086 => {
                self.bus_begin(BusStatus::MemRead, seg, addr, 0, TransferSize::Word);
                self.bus_wait_finish();

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

    pub fn biu_write_u16(&mut self, seg: Segment, addr: u32, word: u16, flag: WriteFlag) {

        match self.cpu_type {
            CpuType::Cpu8088 => {
                // 8088 performs two consecutive byte transfers
                self.bus_begin(BusStatus::MemWrite, seg, addr, word & 0x00FF, TransferSize::Byte);

                validate_write_u8!(self, addr, (word & 0x00FF) as u8);

                self.bus_wait_finish();

                self.bus_begin(BusStatus::MemWrite, seg, addr + 1, (word >> 8) & 0x00FF, TransferSize::Byte);

                validate_write_u8!(self, addr + 1, ((word >> 8) & 0x00FF) as u8);

                match flag {
                    WriteFlag::Normal => self.bus_wait_finish(),
                    WriteFlag::RNI => self.bus_wait_until(CycleState::T2)
                };
            }
            CpuType::Cpu8086 => {
                self.bus_begin(BusStatus::MemWrite, seg, addr, word, TransferSize::Word);
                match flag {
                    WriteFlag::Normal => self.bus_wait_finish(),
                    WriteFlag::RNI => self.bus_wait_until(CycleState::T2)
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
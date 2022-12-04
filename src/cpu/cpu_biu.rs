use crate::cpu::{Cpu, BusStatus, BusState};
use crate::bus::BusInterface;

use crate::bytequeue::ByteQueue;

impl ByteQueue for Cpu {
    fn seek(&mut self, pos: usize) {

    }
    fn tell(&self) -> usize {
        self.pc as usize
    }

    fn q_read_u8(&mut self) -> u8 {
        self.biu_queue_read()
    }
    fn q_read_i8(&mut self) -> i8 {
        self.biu_queue_read() as i8
    }

    fn q_read_u16(&mut self) -> u16 {
        let lo = self.biu_queue_read();
        let ho = self.biu_queue_read();
        
        (ho as u16) << 8 | (lo as u16)
    }
    fn q_read_i16(&mut self) -> i16 {
        let lo = self.biu_queue_read();
        let ho = self.biu_queue_read();
        
        ((ho as u16) << 8 | (lo as u16)) as i16
    }
}


impl Cpu {

    #[inline(always)]
    pub fn biu_queue_full(&mut self) -> bool {
        self.piq.len() >= self.piq_size
    }

    pub fn biu_queue_fetch(&mut self, bus: &mut BusInterface) {

        let byte;
        let _cost;
        if self.bus_state == BusState::T3 {
            (byte, _cost) = bus.read_u8(self.pc as usize).unwrap();
            // TODO: Handle wait states here
            
            // Proceed to T4 and store byte
            self.cycle();
            self.piq.push_back(byte).unwrap();
            self.pc += 1;
        }
        self.bus_status = BusStatus::Idle;
    }

    pub fn biu_queue_flush(&mut self) {
        self.pc -= self.piq.len() as u32;
        self.piq.clear();
    }

    pub fn biu_update_pc(&mut self) {
        self.pc = Cpu::calc_linear_address(self.cs, self.ip);
    }

    pub fn biu_queue_read(&mut self) -> u8 {
        let byte;

        if self.piq.len() > 0 {
            // Return byte from queue 
            byte = self.piq.pop_front().unwrap();
            self.cycle();
        }
        else {
            // Queue is empty, fetch directly
            byte = self.biu_read_u8(self.pc);
            self.pc = (self.pc + 1) & 0xFFFFFu32;
        }
        byte
    }

    pub fn biu_read_u8(&mut self, addr: u32) -> u8 {

        let byte;
        let _cost;

        match self.bus_status {
            BusStatus::Fetch => {
                // Abort fetch
                self.bus_status = BusStatus::Idle;        
                self.cycle();

                self.bus_status = BusStatus::Read;
                (byte, _cost) = self.bus.read_u8(addr as usize).unwrap();
                // TODO: Handle wait states here
                self.cycles(4);
                self.bus_status = BusStatus::Idle;
            }
            BusStatus::Idle => {
                self.bus_status = BusStatus::Read;
                (byte, _cost) = self.bus.read_u8(addr as usize).unwrap();
                // TODO: Handle wait states here
                self.cycles(4);
                self.bus_status = BusStatus::Idle;
            }
            BusStatus::Read | BusStatus::Write => {
                panic!("Overlapping read/write!");
            }
        }
        byte
    }

    pub fn biu_write_u8(&mut self, addr: u32, value: u8) {

        let _result;

        match self.bus_status {
            BusStatus::Fetch => {
                // Abort fetch
                self.bus_status = BusStatus::Idle;        
                self.cycle();

                self.bus_status = BusStatus::Write;
                _result = self.bus.write_u8(addr as usize, value).unwrap();
                // TODO: Handle wait states here
                self.cycles(4);
                self.bus_status = BusStatus::Idle;
            }
            BusStatus::Idle => {
                self.bus_status = BusStatus::Write;
                _result = self.bus.write_u8(addr as usize, value).unwrap();
                // TODO: Handle wait states here
                self.cycles(4);
                self.bus_status = BusStatus::Idle;
            }
            BusStatus::Read | BusStatus::Write => {
                panic!("Overlapping read/write state!");
            }
        }
    }

    pub fn biu_read_u16(&mut self, addr: u32) -> u16 {

        let word;
        let _cost;

        match self.bus_status {
            BusStatus::Fetch => {
                // Abort fetch
                self.bus_status = BusStatus::Idle;        
                self.cycle();

                self.bus_status = BusStatus::Read;
                (word, _cost) = self.bus.read_u16(addr as usize).unwrap();
                // TODO: Handle wait states here
                self.cycles(8);
                self.bus_status = BusStatus::Idle;
            }
            BusStatus::Idle => {
                self.bus_status = BusStatus::Read;
                (word, _cost) = self.bus.read_u16(addr as usize).unwrap();
                // TODO: Handle wait states here
                self.cycles(8);
                self.bus_status = BusStatus::Idle;
            }
            BusStatus::Read | BusStatus::Write => {
                panic!("Overlapping read/write!");
            }
        }
        word
    }

    pub fn biu_write_u16(&mut self, addr: u32, value: u16) {

        let _result;

        match self.bus_status {
            BusStatus::Fetch => {
                // Abort fetch
                self.bus_status = BusStatus::Idle;        
                self.cycle();

                self.bus_status = BusStatus::Write;
                _result = self.bus.write_u16(addr as usize, value).unwrap();
                // TODO: Handle wait states here
                self.cycles(8);
                self.bus_status = BusStatus::Idle;
            }
            BusStatus::Idle => {
                self.bus_status = BusStatus::Write;
                _result = self.bus.write_u16(addr as usize, value).unwrap();
                // TODO: Handle wait states here
                self.cycles(8);
                self.bus_status = BusStatus::Idle;
            }
            BusStatus::Read | BusStatus::Write => {
                panic!("Overlapping read/write state!");
            }
        }
    }    

}
use crate::cpu::{Cpu, BusStatus, BusState};
use crate::bus::BusInterface;

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
            self.cycle(bus);
            self.piq.push_back(byte).unwrap();
            self.pc += 1;
        }
        self.bus_status = BusStatus::Idle;
    }

    pub fn biu_queue_flush(&mut self) {
        self.pc -= self.piq.len() as u32;
        self.piq.clear();
    }

    pub fn biu_queue_read(&mut self, bus: &mut BusInterface) -> u8 {
        let byte;

        if self.piq.len() > 0 {
            // Return byte from queue 
            byte = self.piq.pop_front().unwrap();
            self.cycle(bus);
        }
        else {
            // Queue is empty, fetch directly
            byte = self.biu_read_u8(bus, self.pc);
        }
        byte
    }

    pub fn biu_read_u8(&mut self, bus: &mut BusInterface, addr: u32) -> u8 {

        let byte;
        let _cost;

        match self.bus_status {
            BusStatus::Fetch => {
                // Abort fetch
                self.bus_status = BusStatus::Idle;        
                self.cycle(bus);

                self.bus_status = BusStatus::Read;
                (byte, _cost) = bus.read_u8(addr as usize).unwrap();
                // TODO: Handle wait states here
                self.cycles(4, bus);
                self.bus_status = BusStatus::Idle;
            }
            BusStatus::Idle => {
                self.bus_status = BusStatus::Read;
                (byte, _cost) = bus.read_u8(addr as usize).unwrap();
                // TODO: Handle wait states here
                self.cycles(4, bus);
                self.bus_status = BusStatus::Idle;
            }
            BusStatus::Read | BusStatus::Write => {
                panic!("Overlapping read/write!");
            }
        }


        byte
    }
}
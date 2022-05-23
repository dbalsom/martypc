use crate::cpu::{Cpu, Flag};
use crate::bus::{BusInterface};
use crate::arch::{Opcode, Register8, Register16};
use crate::util;

impl Cpu {
    pub fn string_op(&mut self, bus: &mut BusInterface, opcode: Opcode) {
        match opcode {
            Opcode::STOSB => {
                // STOSB affects no flags
                let dest_addr = util::get_linear_address(self.es, self.di);

                // Write AL to [es:di]
                bus.write_u8(dest_addr as usize, self.al).unwrap();

                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.di = self.di.wrapping_add(1);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.di = self.di.wrapping_sub(1);
                    }
                }
            }
            Opcode::LODSB => {
                // LODSB affects no flags
                // Store [es:di] in AL
                let src_addr = util::get_linear_address(self.es, self.di);

                let (data, _cost) = bus.read_u8(src_addr as usize).unwrap();
                self.set_register8(Register8::AL, data);                
            }
            Opcode::STOSW => {
                // STOSW affects no flags
                let dest_addr = util::get_linear_address(self.es, self.di);

                // Write AX to [es:di]
                bus.write_u16(dest_addr as usize, self.ax).unwrap();

                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.di = self.di.wrapping_add(2);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.di = self.di.wrapping_sub(2);
                    }
                }                
            }
            Opcode::LODSW => {
                // LODSWB affects no flags
                // Store [es:di] in AX
                let src_addr = util::get_linear_address(self.es, self.di);

                let (data, _cost) = bus.read_u16(src_addr as usize).unwrap();
                self.set_register16(Register16::AX, data);                
            }
            _ => {
                panic!("CPU: Unhandled opcode to string_op(): {:?}", opcode);
            }            
        }
    }

}
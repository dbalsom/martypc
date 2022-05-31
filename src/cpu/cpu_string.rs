use crate::cpu::{Cpu, Flag};
use crate::bus::{BusInterface};
use crate::arch::{Opcode, Register8, Register16, SegmentOverride};
use crate::util;

impl Cpu {
    pub fn string_op(&mut self, bus: &mut BusInterface, opcode: Opcode, segment: SegmentOverride) {

        let segment_base_default_ds: u16 = match segment {
            SegmentOverride::NoOverride => self.ds,
            SegmentOverride::SegmentES => self.es,
            SegmentOverride::SegmentCS => self.cs,
            SegmentOverride::SegmentSS => self.ss,
            SegmentOverride::SegmentDS => self.ds
        };

        match opcode {
            Opcode::STOSB => {
                // STOSB - Write AL to [es:di]  (ES prefix cannot be overridden)
                // No flags affected
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
            Opcode::STOSW => {
                // STOSW - Write AX to [es:di] (ES prefix cannot be overridden)
                // No flags affected
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
            Opcode::LODSB => {
                // LODSB affects no flags
                // Store byte [ds:si] in AL   (Segment overrideable)
                let src_addr = util::get_linear_address(segment_base_default_ds, self.si);

                let (data, _cost) = bus.read_u8(src_addr as usize).unwrap();
                self.set_register8(Register8::AL, data);

                // Increment or Decrement SI according to Direction flag
                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.si = self.si.wrapping_add(1);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.si = self.si.wrapping_sub(1);
                    }
                }    
            }
            Opcode::LODSW => {
                // LODSW affects no flags
                // Store word [ds:si] in AX   (Segment overrideable)
                let src_addr = util::get_linear_address(segment_base_default_ds, self.si);

                let (data, _cost) = bus.read_u16(src_addr as usize).unwrap();
                self.set_register16(Register16::AX, data);  

                // Increment or Decrement SI according to Direction flag
                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.si = self.si.wrapping_add(2);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.si = self.si.wrapping_sub(2);
                    }
                }                   
            }            
            Opcode::MOVSB => {
                // Store byte from [ds:si] in [es:di]  (DS Segment overrideable)
                let src_addr = util::get_linear_address(segment_base_default_ds, self.si);
                let dst_addr = util::get_linear_address(self.es, self.di);

                let (data, _cost) = bus.read_u8(src_addr as usize).unwrap();
                bus.write_u8(dst_addr as usize, data).unwrap();

                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.si = self.si.wrapping_add(1);
                        self.di = self.di.wrapping_add(1);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.si = self.si.wrapping_sub(1);
                        self.di = self.di.wrapping_sub(1);
                    }
                }                    
            }
            Opcode::MOVSW => {
                // Store word from [ds:si] in [es:di] (DS Segment overrideable)
                let src_addr = util::get_linear_address(segment_base_default_ds, self.si);
                let dst_addr = util::get_linear_address(self.es, self.di);

                let (data, _cost) = bus.read_u16(src_addr as usize).unwrap();
                bus.write_u16(dst_addr as usize, data).unwrap();

                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.si = self.si.wrapping_add(2);
                        self.di = self.di.wrapping_add(2);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.si = self.si.wrapping_sub(2);
                        self.di = self.di.wrapping_sub(2);
                    }
                }                   
            }
            Opcode::SCASB => {
                // SCASB: Compare byte with value in AL.  
                // Flags: o..szapc
                // Override: ES cannot be overridden
                let scan_addr = util::get_linear_address(self.es, self.di);
                let (byte, _cost) = bus.read_u8(scan_addr as usize).unwrap();

                let (result, carry, overflow, aux_carry) = Cpu::sub_u8(self.al, byte, false );
                // Test operation behaves like CMP
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u8(result);                

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
            Opcode::SCASW => {
                // SCASB: Compare word with value in AX.  
                // Flags: o..szapc
                // Override: ES cannot be overridden                
                let scan_addr = util::get_linear_address(self.es, self.di);
                let (word, _cost) = bus.read_u16(scan_addr as usize).unwrap();

                let (result, carry, overflow, aux_carry) = Cpu::sub_u16(self.ax, word, false );
                // Test operation behaves like CMP
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u16(result);                

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
            Opcode::CMPSB => {
                // CMPSB: Compare bytes
                // Flags: o..szapc
                // Override: DS can be overridden
                let esdi_addr = util::get_linear_address(self.es, self.di);
                let dssi_addr = util::get_linear_address(segment_base_default_ds, self.si);
                let (esdi_op, _cost1) = bus.read_u8(esdi_addr as usize).unwrap();
                let (dssi_op, _cost2) = bus.read_u8(dssi_addr as usize).unwrap();

                let (result, carry, overflow, aux_carry) = Cpu::sub_u8(dssi_op, esdi_op, false);
                // Test operation behaves like CMP
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u8(result);                    

                match self.get_flag(Flag::Direction) {
                    false => {
                        // Direction flag clear, process forwards
                        self.si = self.si.wrapping_add(2);
                        self.di = self.di.wrapping_add(2);
                    }
                    true => {
                        // Direction flag set, process backwards
                        self.si = self.si.wrapping_sub(2);
                        self.di = self.di.wrapping_sub(2);
                    }
                }
            }
            _ => {
                panic!("CPU: Unhandled opcode to string_op(): {:?}", opcode);
            }            
        }
    }

}
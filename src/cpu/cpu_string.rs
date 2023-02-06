use crate::cpu::*;
use crate::bus::BusInterface;

impl<'a> Cpu<'a> {
    pub fn string_op(&mut self, opcode: Mnemonic, segment_override: SegmentOverride) {

        let (segment_value_base_ds, segment_base_ds) = match segment_override {
            SegmentOverride::None => (self.ds, Segment::DS),
            SegmentOverride::ES  => (self.es, Segment::ES),
            SegmentOverride::CS  => (self.cs, Segment::CS),
            SegmentOverride::SS  => (self.ss, Segment::SS),
            SegmentOverride::DS  => (self.ds, Segment::DS),
        };   

        match opcode {
            Mnemonic::STOSB => {
                // STOSB - Write AL to [es:di]  (ES prefix cannot be overridden)
                // No flags affected
                let dest_addr = Cpu::calc_linear_address(self.es, self.di);

                // Write AL to [es:di]
                self.biu_write_u8(Segment::ES, dest_addr, self.al, ReadWriteFlag::Normal);
                self.cycles(1);

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
            Mnemonic::STOSW => {
                // STOSW - Write AX to [es:di] (ES prefix cannot be overridden)
                // No flags affected
                let dest_addr = Cpu::calc_linear_address(self.es, self.di);

                // Write AX to [es:di]
                self.biu_write_u16(Segment::ES, dest_addr, self.ax, ReadWriteFlag::Normal);
                self.cycles(1);

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
            Mnemonic::LODSB => {
                // LODSB affects no flags
                // Store byte [ds:si] in AL   (Segment overrideable)
                let src_addr = Cpu::calc_linear_address(segment_value_base_ds, self.si);

                let data = self.biu_read_u8(segment_base_ds, src_addr);
                self.cycles_i(3, &[0x12e, MC_JUMP, 0x1f8]);

                //let (data, _cost) = self.bus.read_u8(src_addr as usize).unwrap();
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
            Mnemonic::LODSW => {
                // LODSW affects no flags
                // Store word [ds:si] in AX   (Segment overrideable)
                let src_addr = Cpu::calc_linear_address(segment_value_base_ds, self.si);

                let data = self.biu_read_u16(segment_base_ds, src_addr, ReadWriteFlag::Normal);
                self.cycles_i(3, &[0x12e, MC_JUMP, 0x1f8]);

                //let (data, _cost) = self.bus.read_u16(src_addr as usize).unwrap();
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
            Mnemonic::MOVSB => {
                // Store byte from [ds:si] in [es:di]  (DS Segment overrideable)
                let src_addr = Cpu::calc_linear_address(segment_value_base_ds, self.si);
                let dst_addr = Cpu::calc_linear_address(self.es, self.di);

                // Check REP prefixs
                self.cycles(2);
                let data = self.biu_read_u8(segment_base_ds, src_addr);
                self.cycle();
                self.biu_write_u8(Segment::ES, dst_addr, data, ReadWriteFlag::Normal);
                self.cycles(2);

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
            Mnemonic::MOVSW => {
                // Store word from [ds:si] in [es:di] (DS Segment overrideable)
                let src_addr = Cpu::calc_linear_address(segment_value_base_ds, self.si);
                let dst_addr = Cpu::calc_linear_address(self.es, self.di);

                let data = self.biu_read_u16(segment_base_ds, src_addr, ReadWriteFlag::Normal);
                self.biu_write_u16(Segment::ES, dst_addr, data, ReadWriteFlag::Normal);
                //let (data, _cost) = self.bus.read_u16(src_addr as usize).unwrap();
                //self.bus.write_u16(dst_addr as usize, data).unwrap();

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
            Mnemonic::SCASB => {
                // SCASB: Compare byte from [es:di] with value in AL.  
                // Flags: o..szapc
                // Override: ES cannot be overridden
                let scan_addr = Cpu::calc_linear_address(self.es, self.di);

                self.cycles_i(2, &[0x121, MC_JUMP]);
                let data = self.biu_read_u8(Segment::ES, scan_addr);
                //let (byte, _cost) = self.bus.read_u8(scan_addr as usize).unwrap();
                self.cycles_i(4, &[0x126, 0x127, 0x128, MC_JUMP]);

                let (result, carry, overflow, aux_carry) = Cpu::sub_u8(self.al, data, false );
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
            Mnemonic::SCASW => {
                // SCASW: Compare word from [es:di] with value in AX.  
                // Flags: o..szapc
                // Override: ES cannot be overridden                
                let scan_addr = Cpu::calc_linear_address(self.es, self.di);

                let data = self.biu_read_u16(Segment::ES, scan_addr, ReadWriteFlag::Normal);
                //let (word, _cost) = self.bus.read_u16(scan_addr as usize).unwrap();

                let (result, carry, overflow, aux_carry) = Cpu::sub_u16(self.ax, data, false );
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
            Mnemonic::CMPSB => {
                // CMPSB: Compare bytes from [es:di] to [ds:si]
                // Flags: The CF, OF, SF, ZF, AF, and PF flags are set according to the temporary result of the comparison.
                // Override: DS can be overridden
                let dssi_addr = Cpu::calc_linear_address(segment_value_base_ds, self.si);
                let esdi_addr = Cpu::calc_linear_address(self.es, self.di);
                
                //self.cycles_i(2, &[0x121, 0x122]);
                self.cycle_i(0x121);
                let dssi_op = self.biu_read_u8(segment_base_ds, dssi_addr);
                self.cycles_i(2, &[0x123, 0x124]);
                let esdi_op = self.biu_read_u8(Segment::ES, esdi_addr);
                self.cycles_i(2, &[0x127, 0x128]);

                let (result, carry, overflow, aux_carry) = Cpu::sub_u8(dssi_op, esdi_op, false);

                // Test operation behaves like CMP
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u8(result);                    

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
            Mnemonic::CMPSW => {
                // CMPSW: Compare words from [es:di] to [ds:si]
                // Flags: The CF, OF, SF, ZF, AF, and PF flags are set according to the temporary result of the comparison.
                // Override: DS can be overridden
                let dssi_addr = Cpu::calc_linear_address(segment_value_base_ds, self.si);
                let esdi_addr = Cpu::calc_linear_address(self.es, self.di);

                self.cycles(2);
                
                let dssi_op = self.biu_read_u16(segment_base_ds, dssi_addr, ReadWriteFlag::Normal);
                let esdi_op = self.biu_read_u16(Segment::ES, esdi_addr, ReadWriteFlag::Normal);

                //let (dssi_op, _cost2) = self.bus.read_u16(dssi_addr as usize).unwrap();
                //let (esdi_op, _cost1) = self.bus.read_u16(esdi_addr as usize).unwrap();

                let (result, carry, overflow, aux_carry) = Cpu::sub_u16(dssi_op, esdi_op, false);

                // Test operation behaves like CMP
                self.set_flag_state(Flag::Carry, carry);
                self.set_flag_state(Flag::Overflow, overflow);
                self.set_flag_state(Flag::AuxCarry, aux_carry);
                self.set_flags_from_result_u16(result);                    

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
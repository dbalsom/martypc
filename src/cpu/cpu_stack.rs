use crate::cpu::*;
use crate::cpu::cpu_biu::*;
use super::CPU_FLAG_RESERVED1;

impl<'a> Cpu<'a> {

    pub fn push_u8(&mut self, data: u8, flag: ReadWriteFlag) {
        
        // Stack pointer grows downwards
        self.sp = self.sp.wrapping_sub(2); 
        let stack_addr = Cpu::calc_linear_address(self.ss, self.sp);
        self.biu_write_u8(Segment::SS, stack_addr, data, flag);
    }

    pub fn push_u16(&mut self, data: u16, flag: ReadWriteFlag) {

        // Stack pointer grows downwards
        self.sp = self.sp.wrapping_sub(2);

        let stack_addr = Cpu::calc_linear_address(self.ss, self.sp);
        //let _cost = self.bus.write_u16(stack_addr as usize, data).unwrap();
        self.biu_write_u16(Segment::SS, stack_addr, data, flag);
    }

    pub fn pop_u16(&mut self) -> u16 {

        let stack_addr = Cpu::calc_linear_address(self.ss, self.sp);
        
        //let (result, _cost) = self.bus.read_u16(stack_addr as usize).unwrap();
        let result = self.biu_read_u16(Segment::SS, stack_addr, ReadWriteFlag::Normal);
        
        // Stack pointer shrinks upwards
        self.sp = self.sp.wrapping_add(2);
        result
    }

    pub fn push_register16(&mut self, reg: Register16, flag: ReadWriteFlag) {
        
        // Stack pointer grows downwards
        self.sp = self.sp.wrapping_sub(2);
        
        let data = match reg {
            Register16::AX => self.ax,
            Register16::BX => self.bx,
            Register16::CX => self.cx,
            Register16::DX => self.dx,
            Register16::SP => self.sp,
            Register16::BP => self.bp,
            Register16::SI => self.si,
            Register16::DI => self.di,
            Register16::CS => self.cs,
            Register16::DS => self.ds,
            Register16::SS => self.ss,
            Register16::ES => self.es,
            Register16::IP => self.ip,    
            _ => panic!("Invalid register")            
        };
        
        let stack_addr = Cpu::calc_linear_address(self.ss, self.sp);

        //let _cost = self.bus.write_u16(stack_addr as usize, data).unwrap();
        self.biu_write_u16(Segment::SS, stack_addr, data, flag);

    }

    pub fn pop_register16(&mut self, reg: Register16, flag: ReadWriteFlag) {

        let stack_addr = Cpu::calc_linear_address(self.ss, self.sp);
    
        let data = self.biu_read_u16(Segment::SS, stack_addr, flag);

        let mut update_sp = true;
        match reg {
            Register16::AX => self.set_register16(reg, data),
            Register16::BX => self.set_register16(reg, data),
            Register16::CX => self.set_register16(reg, data),
            Register16::DX => self.set_register16(reg, data),
            Register16::SP => {
                self.sp = data;
                update_sp = false;
            }
            Register16::BP => self.bp = data,
            Register16::SI => self.si = data,
            Register16::DI => self.di = data,
            Register16::CS => self.cs = data,
            Register16::DS => self.ds = data,
            Register16::SS => {
                self.ss = data;
                // Inhibit interrupts for one instruction after issuing POP SS
                self.interrupt_inhibit = true
            },
            Register16::ES => self.es = data,     
            Register16::IP => self.ip = data,      
            _ => panic!("Invalid register")            
        };
        // Stack pointer grows downwards
        if update_sp {
            self.sp = self.sp.wrapping_add(2);
        }
    }

    pub fn push_flags(&mut self, wflag: ReadWriteFlag) {

        // TODO: Handle stack exception per Intel manual when SP==1

        // Stack pointer grows downwards
        self.sp = self.sp.wrapping_sub(2);

        let stack_addr = Cpu::calc_linear_address(self.ss, self.sp);

        //let _cost = self.bus.write_u16(stack_addr as usize, self.flags).unwrap();
        self.biu_write_u16(Segment::SS, stack_addr, self.flags, wflag);
    }

    pub fn pop_flags(&mut self) {

        let stack_addr = Cpu::calc_linear_address(self.ss, self.sp);
        //let (result, _cost) = self.bus.read_u16(stack_addr as usize).unwrap();
        let result = self.biu_read_u16(Segment::SS, stack_addr, ReadWriteFlag::Normal);

        // Ensure state of reserved flag bits
        self.flags = result & FLAGS_POP_MASK;
        self.flags |= CPU_FLAGS_RESERVED_ON;

        // Stack pointer grows downwards
        self.sp = self.sp.wrapping_add(2);
    }

    pub fn release(&mut self, disp: u16) {

        // TODO: Stack exceptions?
        self.sp = self.sp.wrapping_add(disp);
    }
}
use crate::cpu::{self, Cpu, ExecutionResult, Flag};
use crate::bus::{BusInterface};
use crate::arch::{Register8, Register16};
use crate::util;

use super::CPU_FLAG_RESERVED1;

impl Cpu {

    pub fn push_u16(&mut self, bus: &mut BusInterface, data: u16) {

        // Stack pointer grows downwards
        self.sp = self.sp.wrapping_sub(2);

        let stack_addr = util::get_linear_address(self.ss, self.sp);
        let _cost = bus.write_u16(stack_addr as usize, data).unwrap();
    }

    pub fn pop_u16(&mut self, bus: &mut BusInterface) -> u16 {

        let stack_addr = util::get_linear_address(self.ss, self.sp);
        let (result, _cost) = bus.read_u16(stack_addr as usize).unwrap();
        
        // Stack pointer grows downwards
        self.sp = self.sp.wrapping_add(2);
        result
    }

    pub fn push_register16(&mut self, bus: &mut BusInterface, reg: Register16) {
        
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
        
        let stack_addr = util::get_linear_address(self.ss, self.sp);
        let _cost = bus.write_u16(stack_addr as usize, data).unwrap();

    }

    pub fn pop_register16(&mut self, bus: &mut BusInterface, reg: Register16) {

        let stack_addr = util::get_linear_address(self.ss, self.sp);
        let (data, _cost) = bus.read_u16(stack_addr as usize).unwrap();
        match reg {
            Register16::AX => self.set_register16(reg, data),
            Register16::BX => self.set_register16(reg, data),
            Register16::CX => self.set_register16(reg, data),
            Register16::DX => self.set_register16(reg, data),
            Register16::SP => self.sp = data,
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
        self.sp = self.sp.wrapping_add(2);
    }

    pub fn push_flags(&mut self, bus: &mut BusInterface) {

        // TODO: Handle stack exception per Intel manual when SP==1

        // Stack pointer grows downwards
        self.sp = self.sp.wrapping_sub(2);

        let stack_addr = util::get_linear_address(self.ss, self.sp);
        let _cost = bus.write_u16(stack_addr as usize, self.eflags).unwrap();
    }

    pub fn pop_flags(&mut self, bus: &mut BusInterface) {

        let stack_addr = util::get_linear_address(self.ss, self.sp);
        let (result, _cost) = bus.read_u16(stack_addr as usize).unwrap();

        // Ensure state of reserved flag bits
        self.eflags = result & cpu::EFLAGS_POP_MASK;
        self.eflags |= CPU_FLAG_RESERVED1;

        // Stack pointer grows downwards
        self.sp = self.sp.wrapping_add(2);
    }

    pub fn release(&mut self, disp: u16) {

        // TODO: Stack exceptions?
        self.sp = self.sp.wrapping_add(disp);
    }
}
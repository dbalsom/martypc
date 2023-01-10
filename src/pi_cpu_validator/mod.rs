/*
    Raspberry Pi 8088 CPU Validator

    Original code Copyright (c) 2019-2022 Andreas T Jonsson <mail@andreasjonsson.se>
    Ported to Rust for the Marty emulator by Daniel Balsom

    Original Copyright notice follows.
*/

// Copyright (c) 2019-2022 Andreas T Jonsson <mail@andreasjonsson.se>
//
// This software is provided 'as-is', without any express or implied
// warranty. In no event will the authors be held liable for any damages
// arising from the use of this software.
//
// Permission is granted to anyone to use this software for any purpose,
// including commercial applications, and to alter it and redistribute it
// freely, subject to the following restrictions:
//
// 1. The origin of this software must not be misrepresented; you must not
//    claim that you wrote the original software. If you use this software in
//    a product, an acknowledgment (see the following) in the product
//    documentation is required.
//
//    Portions Copyright (c) 2019-2022 Andreas T Jonsson <mail@andreasjonsson.se>
//
// 2. Altered source versions must be plainly marked as such, and must not be
//    misrepresented as being the original software.
//
// 3. This notice may not be removed or altered from any source distribution.

use log;
use sysfs_gpio::{Direction, Pin};

mod udmask;
use udmask::{FLAG_MASK_LOOKUP};

use crate::cpu_validator::{CpuValidator, VRegisters};

static PROGRAM_LOAD: &'static [u8] = include_bytes!("load.bin");
static PROGRAM_STORE: &'static [u8] = include_bytes!("store.bin");

pub const VFLAG_CARRY: u16     = 0x001;
pub const VFLAG_PARITY: u16    = 0x004;
pub const VFLAG_AUXILIARY: u16 = 0x010;
pub const VFLAG_ZERO: u16      = 0x040;
pub const VFLAG_SIGN: u16      = 0x080;
pub const VFLAG_TRAP: u16      = 0x100;
pub const VFLAG_INTERRUPT: u16 = 0x200;
pub const VFLAG_DIRECTION: u16 = 0x400;
pub const VFLAG_OVERFLOW: u16  = 0x800;

const VISIT_ONCE: bool = false;
const NUM_INVALID_FETCHES: usize = 6;
const NUM_MEM_OPS: usize = 0x20000 + 16;
const V_INVALID_POINTER: u32 = 0xFFFFFFFF;
const UPPER_MEMORY: u32 = 0xA0000;

#[derive (PartialEq, Debug)]
pub enum ValidatorState {
    Setup,
    Execute,
    Readback,
    Finished
}

#[derive (PartialEq, Debug)]
pub enum AccessType {
    AccAlternateData = 0x0,
    AccStack,
    AccCodeOrNone,
    AccData,
}

pub const MOF_UNUSED: u8 = 0x00;
pub const MOF_EMULATOR: u8 = 0x01;
pub const MOF_PI8088: u8 = 0x02;

#[derive (Copy, Clone, Default)]
pub struct MemOp {
    addr: u32,
    data: u8,
    flags: u8
}



#[derive (Default)]
pub struct Frame {
    name: String,
    opcode: u8,
    ext_opcode: u8,
    modregrm: bool,
    discard: bool,
    next_fetch: bool,
    num_nop: i32,

    regs: Vec<VRegisters>,

    prefetch_addr: Vec<u32>,
    reads: Vec<MemOp>,
    writes: Vec<MemOp>,
}

impl Frame {
    pub fn new() -> Self {

        Self {
            name: "NewFrame".to_string(),
            opcode: 0,
            ext_opcode: 0,
            modregrm: false,
            discard: false,
            next_fetch: false,
            num_nop: 0,
            regs: vec![VRegisters::default(); 2],

            prefetch_addr: vec![0; NUM_INVALID_FETCHES],
            reads: vec![MemOp::default(); NUM_MEM_OPS],
            writes: vec![MemOp::default(); NUM_MEM_OPS]
        }
    }
}

pub struct PiValidator {

    current_frame: Frame,
    state: ValidatorState,

    // Programs
    load_code: Vec<u8>,
    store_code: Vec<u8>,

    // Output pins
    clock_line: Pin,
    reset_line: Pin,
    test_line: Pin,

    ss0_line: Pin,
    rd_line: Pin,
    wr_line: Pin,
    iom_line: Pin,
    ale_line: Pin,

    ad_0_7_line: Vec<Pin>,
    a_8_19_line: Vec<Pin>,

    cycle_count: u64,

    rd_signal: bool,
    wr_signal: bool,
    iom_signal: bool,
    ale_signal: bool,

    address_latch: u32,

    cpu_memory_access: AccessType,
    cpu_interrupt_enabled: bool,

    scratchpad: Vec<u8>,
    code_as_data_skip: bool,
    readback_ptr: usize,
    trigger_addr: u32,

    visit_once: bool,
    visited: Vec<bool>,

    log_prefix: String
}
/*
macro_rules! open_output_pin {
    ($pin_no:expr) => {
        match gpio::sysfs::SysFsGpioOutput::open($pin_no) {
            Ok(pin) => pin,
            Err(e) => panic!("Couldn't open pin {}: {}", $pin_no, e)
        }
    }
}

macro_rules! open_input_pin {
    ($pin_no:expr) => {
        match gpio::sysfs::SysFsGpioInput::open($pin_no) {
            Ok(pin) => pin,
            Err(e) => panic!("Couldn't open pin {}: {}", $pin_no, e)
        }
    }
}


*/

pub fn open_output_pin(pin_no: u64) -> Pin {
    let new_pin = Pin::new(pin_no);

    match new_pin.export() {
        Ok(()) => {},
        Err(e) => panic!("Couldn't export pin {}: {}", pin_no, e)
    };

    match new_pin.set_direction(Direction::Out) {
        Ok(()) => {},
        Err(e) => panic!("Couldn't set pin {} direction: {}", pin_no, e)
    };

    new_pin
}

pub fn open_input_pin(pin_no: u64) -> Pin {
    let new_pin = Pin::new(pin_no);

    match new_pin.export() {
        Ok(()) => {},
        Err(e) => panic!("Couldn't export pin {}: {}", pin_no, e)
    }

    match new_pin.set_direction(Direction::In) {
        Ok(()) => {},
        Err(e) => panic!("Couldn't set pin {} direction: {}", pin_no, e)
    };

    new_pin
}   

pub fn get_bool( pin: &Pin ) -> bool {
    let value = match pin.get_value() {
        Ok(val) => val,
        Err(e) => panic!("Couldn't read pin {} value: {}", pin.get_pin_num(), e)
    };

    match value {
        0 => {},
        1 => {},
        _ => panic!("Read non-bool value from pin: {}", pin.get_pin_num())
    }
    value != 0
}

pub fn get_int( pin: &Pin ) -> u32 {
    let value = match pin.get_value() {
        Ok(val) => val,
        Err(e) => panic!("Couldn't read pin {} value: {}", pin.get_pin_num(), e)
    };

    match value {
        0 => {},
        1 => {},
        _ => panic!("Read non-zero or one value from pin: {}", pin.get_pin_num())
    }
    value as u32
}

pub fn make_pointer(base: u16, offset: u16) -> u32 {
    return (((base as u32) << 4) + offset as u32 ) & 0xFFFFF;
}

impl CpuValidator for PiValidator {

    fn begin(&mut self, regs: &VRegisters ) {

        self.current_frame.discard = false;
        self.current_frame.regs[0] = regs.clone();

        let ip_addr = make_pointer(regs.cs, regs.ip);

        //println!("{} : {}", self.trigger_addr, ip_addr);
        if self.trigger_addr == ip_addr {
            log::info!("Trigger address hit, begin validation...");
            self.trigger_addr = V_INVALID_POINTER;
        }

        if (self.trigger_addr != V_INVALID_POINTER) 
            || (self.visit_once && ip_addr >= UPPER_MEMORY && self.visited[ip_addr as usize]) {
            self.current_frame.discard = true;
            return;
        }

        self.current_frame.reads.fill(MemOp::default());
        self.current_frame.writes.fill(MemOp::default());
    }    


    fn end(&mut self, name: String, opcode: u8, modregrm: bool, cycles: i32, regs: &VRegisters) {

        let ip_addr = make_pointer(self.current_frame.regs[0].cs, self.current_frame.regs[0].ip);

        if (self.trigger_addr != V_INVALID_POINTER) 
            || (self.visit_once && ip_addr >= UPPER_MEMORY &&  self.visited[ip_addr as usize]) {
            return
        }

        self.visited[ip_addr as usize] = true;

        self.current_frame.name = name.clone();
        self.current_frame.opcode = opcode;
        self.current_frame.ext_opcode = 0xFF;
        self.current_frame.modregrm = modregrm;
        self.current_frame.num_nop = 0;
        self.current_frame.next_fetch = false;
        self.current_frame.regs[1] = *regs;

        // Set opcode extension if modrm
        if modregrm {
            for i in 0..(NUM_MEM_OPS - 1) {
                if self.current_frame.reads[i].data == opcode {
                    self.current_frame.ext_opcode = (self.current_frame.reads[i + 1].data >> 3) & 0x07;
                    break;
                }
            }
        }

        // We must discard the first instructions after boot.
        if ((ip_addr >= 0xFFFF0) || (ip_addr <= 0xFF)) {
            log::debug!("Instruction out of range: Discarding...");
            self.current_frame.discard = true;
        }

        let discard_or_validate = match self.current_frame.discard {
            true => "DISCARD",
            false => "VALIDATE"
        };

        log::debug!("{}: {} (0x{:02X}) @ [{:04X}:{:04X}]", discard_or_validate, name, opcode, self.current_frame.regs[0].cs, self.current_frame.regs[0].ip);
        if self.current_frame.discard {
            return;
        }

        // memset prefetch
        self.current_frame.prefetch_addr.fill(0);

        // Create scratchpad
        self.scratchpad.fill(0);
        
        // Load 'load' procedure into scratchpad
        for i in 0..self.load_code.len() {
            self.scratchpad[i] = self.load_code[i];
        }

        // Patch load procedure with current register values
        let r = self.current_frame.regs[0];

        self.write_u16_scratch(0x00, r.flags);
        
        self.write_u16_scratch(0x0B, r.bx);
        self.write_u16_scratch(0x0E, r.cx);
        self.write_u16_scratch(0x11, r.dx);

        self.write_u16_scratch(0x14, r.ss);
        self.write_u16_scratch(0x19, r.ds);
        self.write_u16_scratch(0x1E, r.es);
        self.write_u16_scratch(0x23, r.sp);
        self.write_u16_scratch(0x28, r.bp);
        self.write_u16_scratch(0x2D, r.si);
        self.write_u16_scratch(0x32, r.di);

        self.write_u16_scratch(0x37, r.ax);
        self.write_u16_scratch(0x3A, r.ip);
        self.write_u16_scratch(0x3C, r.cs);

        // JMP 0:2
        let jmp: Vec<u8> = vec![0xEA, 0x02, 0x00, 0x00, 0x00];
        for i in 0..jmp.len() {
            self.scratchpad[0xFFFF0 + i] = jmp[i];
        }

        self.code_as_data_skip = false;

        self.reset_sequence();

        self.state = ValidatorState::Setup;

        loop {
            self.execute_bus_cycle();
            self.next_bus_cycle();

            if self.state == ValidatorState::Finished {
                break;
            }
        }
        
        if self.code_as_data_skip {
            return
        }

        for i in 0..NUM_MEM_OPS {
            //self.validate_mem_op(&self.current_frame.reads[i]);
            //self.validate_mem_op(&self.current_frame.writes[i]);
            self.validate_mem_op_read(i);
            self.validate_mem_op_write(i);
        }

        self.validate_registers();
    }

    fn emu_read_byte(&mut self, addr: u32, data: u8) {
        if self.current_frame.discard {
            return;
        }

        for i in 0..NUM_MEM_OPS {

            let op = &self.current_frame.reads[i];
            if(op.flags == 0 || ((op.flags & MOF_EMULATOR != 0) && (op.addr == addr) && (op.data == data))) {
                self.current_frame.reads[i].addr = addr;
                self.current_frame.reads[i].data = data;
                self.current_frame.reads[i].flags = MOF_EMULATOR;

                log::debug!("EMU read: [{:05X}] -> 0x{:02X}", addr, data);
                return;
            }
        }

        log::error!("read_byte error")
    }


    fn emu_write_byte(&mut self, addr: u32, data: u8) {
        
        self.visited[(addr & 0xFFFFF) as usize] = false;
        
        if self.current_frame.discard {
            return;
        }

        for i in 0..NUM_MEM_OPS {

            let op = &self.current_frame.writes[i];
            if(op.flags == 0 || ((op.flags & MOF_EMULATOR != 0) && (op.addr == addr) && (op.data == data))) {
                self.current_frame.writes[i].addr = addr;
                self.current_frame.writes[i].data = data;
                self.current_frame.writes[i].flags = MOF_EMULATOR;

                log::debug!("EMU write: [{:05X}] <- 0x{:02X}", addr, data);
                return;
            }
        }

        log::error!("write_byte error")
    }

    fn discard_op(&mut self) {
        self.current_frame.discard = true;
    }
}

impl PiValidator {

    pub fn new() -> Self {

        // Trigger addr is address at which to start validation
        // if trigger_addr == V_INVALID_POINTER then validate
        
        let trigger_addr = V_INVALID_POINTER;
        //let trigger_addr = 0x9d643;

        //let mut file_path: String = path.to_string();
        //file_path.push_str("load");
        //let load_vec = match std::fs::read(file_path.clone()) {
        //    Ok(file_vec) => file_vec,
        //    Err(e) => panic!("Couldn't open program {}: {}", file_path, e)
        //};
        //
        //let mut file_path: String = path.to_string();
        //file_path.push_str("store");
        //let store_vec = match std::fs::read(file_path.clone()) {
        //    Ok(file_vec) => file_vec,
        //    Err(e) => panic!("Couldn't open program {}: {}", file_path, e)
        //};        

        log::info!("Initializing GPIO pins...");

        // Output pins
        let clock_line = open_output_pin(20);
        let reset_line = open_output_pin(21);
        let test_line = open_output_pin(23);

        // Input pins
        let ss0_line = open_input_pin(22);
        let rd_line = open_input_pin(24);
        let wr_line = open_input_pin(25);
        let iom_line = open_input_pin(26);
        let ale_line = open_input_pin(27);

        // Address and data pins
        let mut ad_0_7_line: Vec<Pin> = Vec::new();

        for i in 0..8 {
            let new_ad = open_input_pin(i);
            ad_0_7_line.push(new_ad);
        }

        let mut a_8_19_line: Vec<Pin> = Vec::new();
        
        for i in 0..12 {
            let new_a = open_input_pin(i + 8);
            a_8_19_line.push(new_a);
        }

        PiValidator {
            current_frame: Frame::new(),
            state: ValidatorState::Setup,
            load_code: PROGRAM_LOAD.to_vec(),
            store_code: PROGRAM_STORE.to_vec(),
            clock_line,
            reset_line,
            test_line,
            ss0_line,
            rd_line,
            wr_line,
            iom_line,
            ale_line,
            ad_0_7_line,
            a_8_19_line,
            cycle_count: 0,
            rd_signal: false,
            wr_signal: false, 
            iom_signal: false,
            ale_signal: false,   
            address_latch: 0,
            cpu_memory_access: AccessType::AccAlternateData,
            cpu_interrupt_enabled: false,

            scratchpad: vec![0; 0x100000],
            code_as_data_skip: false,
            readback_ptr: 0,
            trigger_addr,

            visit_once: VISIT_ONCE,
            visited: vec![false; 0x100000],

            log_prefix: String::new()
        }
    }

    pub fn reset_sequence(&mut self) {
        log::debug!("Resetting CPU...");

        match self.reset_line.set_value(1) {
            Ok(_) => {},
            Err(e) => panic!("Failed to set line: {}", e)
        }

        self.pulse_clock(4);

        match self.reset_line.set_value(0) {
            Ok(_) => {},
            Err(e) => panic!("Failed to set line: {}", e)
        }        

        log::debug!("Waiting for ALE...");

        let mut ale_cycles = 0;
        while !self.ale_signal {
            ale_cycles += 1;
            self.pulse_clock(1);
        }
        println!("It took {} cycles for ALE signal.", ale_cycles);
        log::debug!("CPU initialized!");
        //println!("CPU initialized!");
    }

    pub fn pulse_clock(&mut self, ticks: i32 ) {

        for _ in 0..ticks {
            self.clock_line.set_value(1).unwrap();
            self.clock_line.set_value(0).unwrap();
            self.cycle_count += 1;
        }

        self.rd_signal = !get_bool(&self.rd_line);
        self.wr_signal = !get_bool(&self.wr_line);

        self.iom_signal = get_bool(&self.iom_line);
        self.ale_signal = get_bool(&self.ale_line);

        //assert!((self.rd_signal == false) || (self.ale_signal == false));
        //println!("{},{}", self.rd_signal, self.wr_signal);
        if !self.ale_signal {
            // RD and WR active in T2,T3,T4. Only one should be active at a time
            //assert!(self.rd_signal != self.wr_signal);

            // Pins S3 and S4 indicate segment register for the current bus cycle
            let s3_signal = self.a_8_19_line[8].get_value().unwrap() as u32;
            let s4_signal = self.a_8_19_line[9].get_value().unwrap() as u32;

            let access = (s4_signal << 1) | s3_signal;
            self.cpu_memory_access = match access {
                0b00 => AccessType::AccAlternateData,
                0b01 => AccessType::AccStack,
                0b10 => AccessType::AccCodeOrNone,
                0b11 => AccessType::AccData,
                _ => unreachable!("")
            }
            // Do mem access
        }
    }

    /// Read the address lines from the CPU and store the value in address_latch
    pub fn latch_address(&mut self) {

        assert!(self.ale_signal);

        self.address_latch = 0;
        for i in 0..8 {
            self.address_latch |= get_int(&self.ad_0_7_line[i]) << i;
        }
        for i in 0..12 {
            self.address_latch |= get_int(&self.a_8_19_line[i]) << (i + 8);
        }

        log::debug!("Address latched: {:05X}", self.address_latch);
    }

    pub fn set_bus_direction_out(&mut self) {
        for pin in &self.ad_0_7_line {
            match pin.set_direction(Direction::In) {
            
                Ok(()) => {},
                Err(e) => {
                    panic!("Error setting pin {} direction IN: {}", pin.get_pin_num(), e);
                }
            }
        }
    }

    pub fn set_bus_direction_in(&mut self, data: u8) {

        assert!(self.rd_signal);

        for (i, pin) in self.ad_0_7_line.iter_mut().enumerate() {
            match pin.set_direction(Direction::Out) {
            
                Ok(()) => {},
                Err(e) => {
                    panic!("Error setting pin {} direction OUT: {}", pin.get_pin_num(), e);
                }
            }

            match pin.set_value((data >> i) & 0x01) { 
                Ok(()) => {},
                Err(e) => {
                    panic!("Error setting pin {} value: {}", pin.get_pin_num(), e);
                }
            }
        }
    }    

    pub fn read_cpu_pins(&mut self) -> u8 {

        let mut data = 0u8;
        for i in 0..8 {
            data |= (get_int(&self.ad_0_7_line[i]) as u8) << i;
        }
        data
    }

    pub fn validate_data_write(&mut self, addr: u32, data: u8) {

        log::debug!("CPU write: [0x{:05X}] <- 0x{:02X}", addr, data);

        match self.state {
            ValidatorState::Readback => {},
            ValidatorState::Execute => {},
            _ => {
                panic!("validate_data_write(): Invalid CPU state: {:?}", self.state);
            }
        }

        if self.state == ValidatorState::Readback {
            if self.iom_signal {
                if (addr == 0xFF) && (data == 0xFF) {
                    self.state = ValidatorState::Finished;
                    log::debug!("Validator state change: {:?}", self.state);
                    return;
                }
            }
            else {
                assert!((addr == 0) || (addr == 1));
            }
            self.scratchpad[addr as usize] = data;
            return;
        }

        assert!(!self.iom_signal);

        for i in 0..NUM_MEM_OPS {

            let mut op = &mut self.current_frame.writes[i];

            if ((op.flags & MOF_PI8088) == 0) && ((op.flags & MOF_EMULATOR) != 0) {
                if op.addr == addr {
                    op.flags |= MOF_PI8088;
                    op.data = data;
                    return
                }
            }

        }

        log::error!("Not a valid write! ([0x{:05X}] <- 0x{:02X})", addr, data);
        panic!("Invalid write");

    }

    pub fn validate_data_read(&mut self, addr: u32) -> u8 {

        //log::debug!("CPU read: [0x{:05X}] ->", addr);
        self.log_prefix = format!("CPU read: [0x{:05X}] ->", addr);
        
        assert!(!self.iom_signal);

        loop {
            match self.state {
                ValidatorState::Setup => {
                    // Trigger state change?
                    if addr != self.current_frame.reads[0].addr {
                        let data = self.scratchpad[addr as usize];
                        log::debug!("{} 0x{:02X}", self.log_prefix, data);
                        return data;
                    }
                        
                    self.cycle_count = 0;
                    self.state = ValidatorState::Execute;
                    log::debug!("Validator state change from Setup to Execute");
                }
                ValidatorState::Execute => {
                    let next_inst_addr = 
                        make_pointer(
                            self.current_frame.regs[1].cs, 
                            self.current_frame.regs[1].ip);

                    // TODO: This is a bug in the validator! The emulator needs to indicate data or code fetch to avoid this.
                    match self.cpu_memory_access {
                        AccessType::AccCodeOrNone => {},
                        _ => {
                            if i32::abs((addr as i32) - (next_inst_addr as i32)) < 6 {
                                log::warn!("Fetching next instruction as data is not supported!");
                                self.code_as_data_skip = true;
                                self.state = ValidatorState::Finished;
                                return 0;
                            }
                        }
                    }

                    for i in 0..NUM_MEM_OPS {
                        let mut op = &mut self.current_frame.reads[i];
                        if !((op.flags & MOF_PI8088) != 0) && ((op.flags & MOF_EMULATOR) != 0) {
                            if op.addr == addr {
                                // YES: This is a valid read!
                                op.flags |= MOF_PI8088;
                                log::debug!("{} 0x{:02X}", self.log_prefix, op.data);
                                return op.data;
                            }
                        }
                    }

                    if self.cpu_memory_access == AccessType::AccCodeOrNone {
                        if addr == next_inst_addr {
                            self.current_frame.next_fetch = true;
                        }

                        // Allow for N invalid fetches that we assume is the prefetch.
                        // This is intended to fill the prefetch queue so the instruction can finish.
                        if (self.current_frame.num_nop as usize) < NUM_INVALID_FETCHES {
                            self.current_frame.prefetch_addr[self.current_frame.num_nop as usize] = addr;
                            self.current_frame.num_nop += 1;
                            log::debug!("Execute: Prefetch NOP");
                            return 0x90; // NOP
                        }                    
                    }

                    // executed_cycles = cycle_count   (not used?)
                    self.state = ValidatorState::Readback;
                    log::debug!("Validator state change from Execute to Readback");

                    // Clear scratchpad
                    self.scratchpad.fill(0);
                    self.readback_ptr = 2;
                }
                ValidatorState::Readback => {

                    // Assume this is prefetch.
                    if self.readback_ptr >= self.store_code.len() {

                        if let AccessType::AccCodeOrNone = self.cpu_memory_access {
                            log::debug!("{} NOP", self.log_prefix);
                            return 0x90; // NOP
                        }
                        else {
                            panic!("validate_data_read(): READBACK: Unexpected memory access mode: {:?}", self.cpu_memory_access)
                        }
                    }

                    // We assume store code fetches linear memory.
                    let data = self.store_code[self.readback_ptr];
                    self.readback_ptr += 1;

                    log::debug!("{} 0x{:02X}", self.log_prefix, data);
                    return data;
                }
                ValidatorState::Finished => {
                    if let AccessType::AccCodeOrNone = self.cpu_memory_access {
                        log::debug!("Finished: NOP");
                        return 0x90; // NOP                    
                    }
                    else {
                        panic!("Unexpected memory access mode: {:?}", self.cpu_memory_access)                    
                    }
                }
            }
        } 

        unreachable!("validate_date_read(): Invalid read");
        0
    }

    pub fn next_bus_cycle(&mut self) {
        log::trace!("Waiting for bus cycle...");
        while !self.ale_signal {
            self.pulse_clock(1);
        }

        log::trace!("Got bus cycle!");
    }

    pub fn execute_bus_cycle(&mut self) {
        log::trace!("Execute bus cycle...");

        assert!(self.ale_signal);
        self.latch_address();
        self.pulse_clock(1);

        log::trace!("Memory access type: {:?}", self.cpu_memory_access);

        if self.wr_signal {
            // CPU is writing data to the bus.
            let data = self.read_cpu_pins();
            self.validate_data_write(self.address_latch, data);
            self.pulse_clock(2);
        }
        else if self.rd_signal {
            // CPU is reading data from the bus
            let data = self.validate_data_read(self.address_latch);
            self.set_bus_direction_in(data);
            self.pulse_clock(2);
            self.set_bus_direction_out();
        }
        else {
            log::warn!("CPU in neither wr or rd");
        }
    }



    pub fn validate_mem_op(&mut self, op: &MemOp) {
        if self.state == ValidatorState::Finished {

            if (op.flags != 0) && (op.flags != (MOF_EMULATOR | MOF_PI8088)) {
                // TODO: print more info
                panic!("validate_mem_op(): Memory operations don't match!")
            }
        }
        else {
            panic!("validate_mem_op(): Invalid validator state: {:?}", self.state);
        }
    }

    pub fn validate_mem_op_read(&mut self, idx: usize) {

        let op = self.current_frame.reads[idx];
        
        if self.state == ValidatorState::Finished {

            if (op.flags != 0) && (op.flags != (MOF_EMULATOR | MOF_PI8088)) {
                // TODO: print more info
                panic!("validate_mem_op_read(): Memory operations don't match!")
            }
        }
        else {
            panic!("validate_mem_op_read(): Invalid validator state: {:?}", self.state);
        }
    }
    
    pub fn validate_mem_op_write(&mut self, idx: usize) {

        let op = self.current_frame.writes[idx];

        if self.state == ValidatorState::Finished {

            if (op.flags != 0) && (op.flags != (MOF_EMULATOR | MOF_PI8088)) {
                // TODO: print more info
                panic!("validate_mem_op_write(): Memory operations don't match!")
            }
        }
        else {
            panic!("validate_mem_op_write(): Invalid validator state: {:?}", self.state);
        }
    }    

    pub fn mask_undefined_flags(&mut self, flags: u16) -> u16 {

        let mut masked_flags = flags & 0xCD5; // Ignore I and T

        let mut i = 0;
        while FLAG_MASK_LOOKUP[i].opcode != -1 {

            let iop = self.current_frame.opcode;

            if FLAG_MASK_LOOKUP[i].opcode == iop as i16 {

                if FLAG_MASK_LOOKUP[i].ext != -1 {
                    assert!(self.current_frame.modregrm == true);

                    while FLAG_MASK_LOOKUP[i].opcode == iop as i16 {

                        if FLAG_MASK_LOOKUP[i].ext == self.current_frame.ext_opcode as i16 {
                            masked_flags &= !(FLAG_MASK_LOOKUP[i].mask);
                            return masked_flags;
                        }
                        i += 1;
                    }

                    // Nothing to mask!
                    return masked_flags;
                }

                masked_flags &= !(FLAG_MASK_LOOKUP[i].mask);
                return masked_flags;
            }

            i += 1;
        }

        masked_flags
    }

    pub fn read_u16_scratch(&mut self, offset: usize) -> u16 {

        let w: u16 = self.scratchpad[offset] as u16 | ((self.scratchpad[offset+1] as u16) << 8);

        w
    }

    pub fn write_u16_scratch(&mut self, offset: usize, w: u16) {

        self.scratchpad[offset] = (w & 0xFF) as u8;
        self.scratchpad[offset+1] = (w >> 8) as u8;
    }

    pub fn validate_registers(&mut self) {

        if self.state == ValidatorState::Finished {

            if !self.current_frame.next_fetch {
                panic!("Next instruction was never fetched! Possible bad jump?");
            }
        }
        else {
            panic!("State was not Finished!");
        }

        //let r: &VRegisters = &mut self.current_frame.regs[1];
        let mut offset: usize = 2;

        let v = self.read_u16_scratch(offset);
        offset += 2;
        assert_eq!(self.current_frame.regs[1].ax, v);

        let v = self.read_u16_scratch(offset);
        offset += 2;
        assert_eq!(self.current_frame.regs[1].bx, v);
        
        let v = self.read_u16_scratch(offset);
        offset += 2;
        assert_eq!(self.current_frame.regs[1].cx, v);
        
        let v = self.read_u16_scratch(offset);
        offset += 2;
        assert_eq!(self.current_frame.regs[1].dx, v);
        
        let v = self.read_u16_scratch(offset);
        offset += 2;
        assert_eq!(self.current_frame.regs[1].ss, v);
        
        let v = self.read_u16_scratch(offset);
        offset += 2;
        assert_eq!(self.current_frame.regs[1].sp, v);
        
        let v = self.read_u16_scratch(offset);
        offset += 2;
        assert_eq!(self.current_frame.regs[1].cs, v);
        
        let v = self.read_u16_scratch(offset);
        offset += 2;
        assert_eq!(self.current_frame.regs[1].ds, v);
        
        let v = self.read_u16_scratch(offset);
        offset += 2;
        assert_eq!(self.current_frame.regs[1].es, v);

        let v = self.read_u16_scratch(offset);
        offset += 2;
        assert_eq!(self.current_frame.regs[1].bp, v);
        
        let v = self.read_u16_scratch(offset);
        offset += 2;
        assert_eq!(self.current_frame.regs[1].si, v);

        let v = self.read_u16_scratch(offset);
        offset += 2;
        assert_eq!(self.current_frame.regs[1].di, v);

        let cpu_flags = self.read_u16_scratch(0);
        let cpu_flags_masked = self.mask_undefined_flags(cpu_flags);

        let emu_flags = self.current_frame.regs[1].flags;
        let emu_flags_masked = self.mask_undefined_flags(emu_flags);

        if cpu_flags_masked != emu_flags_masked {
            log::error!("CPU flags mismatch! EMU: 0b{:08b} != CPU: 0b{:08b}", emu_flags_masked, cpu_flags_masked);
            panic!("CPU flag mismatch!")
        }
    }


}
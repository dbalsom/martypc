/*
  Marty PC Emulator 
  (C)2023 Daniel Balsom
  https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/
#![allow(dead_code)]

use std::backtrace::Backtrace;
use std::cmp;

use crate::cpu_validator::{ValidatorError, CpuValidator, VRegisters, ReadType};
use crate::cpu::{
    QueueOp,
    CPU_FLAG_CARRY,
    CPU_FLAG_PARITY,
    CPU_FLAG_AUX_CARRY,
    CPU_FLAG_ZERO,
    CPU_FLAG_SIGN,
    CPU_FLAG_TRAP,
    CPU_FLAG_INT_ENABLE,
    CPU_FLAG_DIRECTION,
    CPU_FLAG_OVERFLOW
};

mod queue;
mod udmask;

use crate::arduino8088_client::*;
use crate::cpu_validator::*;
use crate::arduino8088_validator::queue::*;

const ADDRESS_SPACE: usize = 1_048_576;

const VISIT_ONCE: bool = false;
const NUM_INVALID_FETCHES: usize = 6;
const NUM_MEM_OPS: usize = 0x20000 + 16;
const V_INVALID_POINTER: u32 = 0xFFFFFFFF;
const UPPER_MEMORY: u32 = 0xA0000;
const CYCLE_LIMIT: u32 = 1000;

pub const MOF_UNUSED: u8 = 0x00;
pub const MOF_EMULATOR: u8 = 0x01;
pub const MOF_PI8088: u8 = 0x02;

const DATA_PROGRAM: u8 = 0;
const DATA_FINALIZE: u8 = 1;

const OPCODE_NOP: u8 = 0x90;

#[derive (Default, Debug)]
pub struct RemoteCpuRegisters {
    
    ax: u16,
    bx: u16,
    cx: u16,
    dx: u16,
    ss: u16,
    ds: u16,
    es: u16,
    sp: u16,
    bp: u16,
    si: u16,
    di: u16,
    cs: u16,
    ip: u16,
    flags: u16
}

#[derive (PartialEq, Debug)]
pub enum ValidatorState {
    Setup,
    Execute,
    Readback,
    Finished
}

#[derive (Copy, Clone, Debug, PartialEq)]
pub enum BusOpType {
    CodeRead,
    MemRead,
    MemWrite,
    IoRead,
    IoWrite,
}

#[derive (Copy, Clone)]
pub struct BusOp {
    op_type: BusOpType,
    addr: u32,
    data: u8,
    flags: u8
}

#[derive (Default)]
pub struct Frame {
    name: String,
    instr: Vec<u8>,
    opcode: u8,
    modrm: u8,
    has_modrm: bool,
    discard: bool,
    next_fetch: bool,
    num_nop: i32,

    regs: Vec<VRegisters>,

    prefetch_addr: Vec<u32>,
    
    emu_prefetch: Vec<BusOp>,
    emu_ops: Vec<BusOp>,
    cpu_prefetch: Vec<BusOp>,
    cpu_ops: Vec<BusOp>,
    mem_op_n: usize,

    cpu_states: Vec<CycleState>
}

impl Frame {
    pub fn new() -> Self {

        Self {
            name: "NewFrame".to_string(),
            instr: Vec::new(),
            opcode: 0,
            modrm: 0,
            has_modrm: false,
            discard: false,
            next_fetch: false,
            num_nop: 0,
            regs: vec![VRegisters::default(); 2],

            prefetch_addr: vec![0; NUM_INVALID_FETCHES],

            emu_prefetch: Vec::new(),
            emu_ops: Vec::new(),
            cpu_prefetch: Vec::new(),
            cpu_ops: Vec::new(),
            mem_op_n: 0,
            cpu_states: Vec::new()
        }
    }
}

pub struct RemoteCpu {
    cpu_client: CpuClient,
    regs: RemoteCpuRegisters,
    memory: Vec<u8>,
    pc: usize,
    end_addr: usize,
    program_state: ProgramState,

    address_latch: u32,
    status: u8,
    command_status: u8,
    control_status: u8,
    data_bus: u8,
    data_type: QueueDataType,

    cycle_num: u32,
    mcycle_state: BusState,
    bus_state: BusState,
    bus_cycle: BusCycle,
    access_type: AccessType,

    queue: InstructionQueue,
    queue_byte: u8,
    queue_type: QueueDataType,
    queue_first_fetch: bool,
    queue_fetch_n: u8,
    queue_fetch_addr: u32,
    opcode: u8,
    instr_str: String,
    instr_addr: u32,
    finalize: bool,
    flushed: bool,

    // Validator stuff
    busop_n: usize,
    prefetch_n: usize,
    v_pc: usize,
}

impl RemoteCpu {
    pub fn new(cpu_client: CpuClient) -> RemoteCpu {
        RemoteCpu {
            cpu_client,
            regs: Default::default(),
            memory: vec![0; ADDRESS_SPACE],
            pc: 0,
            end_addr: 0,
            program_state: ProgramState::Reset,
            address_latch: 0,
            status: 0,
            command_status: 0,
            control_status: 0,
            data_bus: 0,
            data_type: QueueDataType::Program,
            cycle_num: 0,
            mcycle_state: BusState::PASV,
            bus_state: BusState::PASV,
            bus_cycle: BusCycle::T1,
            access_type: AccessType::AccCodeOrNone,
            queue: InstructionQueue::new(),
            queue_byte: 0,
            queue_type: QueueDataType::Program,
            queue_first_fetch: true,
            queue_fetch_n: 0,
            queue_fetch_addr: 0,
            opcode: 0,
            instr_str: String::new(),
            instr_addr: 0,
            finalize: false,
            flushed: false,

            busop_n: 0,
            prefetch_n: 0,
            v_pc: 0,
        }
    }
}

pub struct ArduinoValidator {

    //cpu_client: Option<CpuClient>,
    cpu: RemoteCpu,

    current_frame: Frame,
    state: ValidatorState,

    cycle_count: u64,
    cycle_trace: bool,

    rd_signal: bool,
    wr_signal: bool,
    iom_signal: bool,
    ale_signal: bool,

    address_latch: u32,

    //cpu_memory_access: AccessType,
    cpu_interrupt_enabled: bool,

    scratchpad: Vec<u8>,
    code_as_data_skip: bool,
    readback_ptr: usize,
    trigger_addr: u32,

    mask_flags: bool,

    visit_once: bool,
    visited: Vec<bool>,

    log_prefix: String
}

impl ArduinoValidator {

    pub fn new() -> Self {

        // Trigger addr is address at which to start validation
        // if trigger_addr == V_INVALID_POINTER then validate        
        let trigger_addr = V_INVALID_POINTER;

        let cpu_client = match CpuClient::init() {
            Ok(client) => client,
            Err(e) => {
                panic!("Failed to initialize ArduinoValidator");
            }
        };

        ArduinoValidator {
            cpu: RemoteCpu::new(cpu_client),

            current_frame: Frame::new(),
            state: ValidatorState::Setup,

            cycle_count: 0,
            cycle_trace: false,
            rd_signal: false,
            wr_signal: false, 
            iom_signal: false,
            ale_signal: false,   
            address_latch: 0,
            //cpu_memory_access: AccessType::AccAlternateData,
            cpu_interrupt_enabled: false,

            scratchpad: vec![0; 0x100000],
            code_as_data_skip: false,
            readback_ptr: 0,
            trigger_addr,
            mask_flags: true,
            visit_once: VISIT_ONCE,
            visited: vec![false; 0x100000],

            log_prefix: String::new()
        }
    }

    pub fn regs_to_buf(buf: &mut [u8], regs: &VRegisters) {
        // AX, BX, CX, DX, SS, SP, FLAGS, IP, CS, DS, ES, BP, SI, DI
        buf[0] = (regs.ax & 0xFF) as u8;
        buf[1] = ((regs.ax >> 8) & 0xFF) as u8;

        buf[2] = (regs.bx & 0xFF) as u8;
        buf[3] = ((regs.bx >> 8) & 0xFF) as u8;

        buf[4] = (regs.cx & 0xFF) as u8;
        buf[5] = ((regs.cx >> 8) & 0xFF) as u8;
        
        buf[6] = (regs.dx & 0xFF) as u8;
        buf[7] = ((regs.dx >> 8) & 0xFF) as u8;        

        buf[8] = (regs.ss & 0xFF) as u8;
        buf[9] = ((regs.ss >> 8) & 0xFF) as u8;
        
        buf[10] = (regs.sp & 0xFF) as u8;
        buf[11] = ((regs.sp >> 8) & 0xFF) as u8;
        
        buf[12] = (regs.flags & 0xFF) as u8;
        buf[13] = ((regs.flags >> 8) & 0xFF) as u8;       
        
        buf[14] = (regs.ip & 0xFF) as u8;
        buf[15] = ((regs.ip >> 8) & 0xFF) as u8;
        
        buf[16] = (regs.cs & 0xFF) as u8;
        buf[17] = ((regs.cs >> 8) & 0xFF) as u8;
        
        buf[18] = (regs.ds & 0xFF) as u8;
        buf[19] = ((regs.ds >> 8) & 0xFF) as u8;
        
        buf[20] = (regs.es & 0xFF) as u8;
        buf[21] = ((regs.es >> 8) & 0xFF) as u8;
        
        buf[22] = (regs.bp & 0xFF) as u8;
        buf[23] = ((regs.bp >> 8) & 0xFF) as u8;
        
        buf[24] = (regs.si & 0xFF) as u8;
        buf[25] = ((regs.si >> 8) & 0xFF) as u8;
        
        buf[26] = (regs.di & 0xFF) as u8;
        buf[27] = ((regs.di >> 8) & 0xFF) as u8;
    }

    pub fn buf_to_regs(buf: &[u8]) -> VRegisters {
        VRegisters {
            ax: buf[0] as u16 | ((buf[1] as u16) << 8),
            bx: buf[2] as u16 | ((buf[3] as u16) << 8),
            cx: buf[4] as u16 | ((buf[5] as u16) << 8),
            dx: buf[6] as u16 | ((buf[7] as u16) << 8),
            ss: buf[8] as u16 | ((buf[9] as u16) << 8),
            sp: buf[10] as u16 | ((buf[11] as u16) << 8),
            flags: buf[12]  as u16| ((buf[13] as u16) << 8),
            ip: buf[14] as u16 | ((buf[15] as u16) << 8),
            cs: buf[16] as u16 | ((buf[17] as u16) << 8),
            ds: buf[18] as u16 | ((buf[19] as u16) << 8),
            es: buf[20] as u16 | ((buf[21] as u16) << 8),
            bp: buf[22] as u16 | ((buf[23] as u16) << 8),
            si: buf[24] as u16 | ((buf[25] as u16) << 8),
            di: buf[26] as u16| ((buf[27] as u16) << 8),
        }
    }

    pub fn validate_mem_ops(&mut self) -> bool {

        if self.current_frame.emu_ops.len() != self.current_frame.cpu_ops.len() {
            log::error!(
                "Validator error: Memory op count mismatch. Emu: {} CPU: {}", 
                self.current_frame.emu_ops.len(),
                self.current_frame.cpu_ops.len()
            );

            return false;
        }


        for i in 0..self.current_frame.emu_ops.len() {

            if self.current_frame.emu_ops[i].op_type != self.current_frame.cpu_ops[i].op_type {
                log::error!(
                    "Bus op #{} type mismatch: EMU:{:?} CPU:{:?}",
                    i,
                    self.current_frame.emu_ops[i].op_type,
                    self.current_frame.cpu_ops[i].op_type
                );
                return false;
            }

            if self.current_frame.emu_ops[i].addr != self.current_frame.cpu_ops[i].addr {
                log::error!(
                    "Bus op #{} addr mismatch: EMU:{:05X} CPU:{:05X}",
                    i,
                    self.current_frame.emu_ops[i].addr,
                    self.current_frame.cpu_ops[i].addr
                );
                return false;
            }

            if self.current_frame.emu_ops[i].data != self.current_frame.cpu_ops[i].data {
                log::error!(
                    "Bus op #{} data mismatch: EMU:{:05X} CPU:{:05X}",
                    i,
                    self.current_frame.emu_ops[i].data,
                    self.current_frame.cpu_ops[i].data
                );
                return false;
            }                   
        }

        return true;
    }

    pub fn validate_registers(&mut self, regs: &VRegisters) -> bool {

        let mut regs_validate = true;

        if self.current_frame.regs[1].ax != regs.ax {
            regs_validate = false;
        }
        if self.current_frame.regs[1].bx != regs.bx {
            regs_validate = false;
        }
        if self.current_frame.regs[1].cx != regs.cx {
            regs_validate = false;
        }
        if self.current_frame.regs[1].dx != regs.dx {
            regs_validate = false;
        }
        if self.current_frame.regs[1].cs != regs.cs {
            regs_validate = false;
        }
        if self.current_frame.regs[1].ds != regs.ds {
            regs_validate = false;
        }
        if self.current_frame.regs[1].es != regs.es {
            regs_validate = false;
        }
        if self.current_frame.regs[1].sp != regs.sp {
            regs_validate = false;
        }
        if self.current_frame.regs[1].sp != regs.sp {
            regs_validate = false;
        }    
        if self.current_frame.regs[1].bp != regs.bp {
            regs_validate = false;
        }    
        if self.current_frame.regs[1].si != regs.si {
            regs_validate = false;
        }    
        if self.current_frame.regs[1].di != regs.di {
            regs_validate = false;
        }
        
        /*
        if self.current_frame.regs[1] != *regs {
            regs_validate = false;
        }
        */

        let mut emu_flags_masked = self.current_frame.regs[1].flags;
        let mut cpu_flags_masked = regs.flags;

        if self.mask_flags {
            emu_flags_masked = ArduinoValidator::mask_undefined_flags(self.current_frame.opcode, self.current_frame.modrm, self.current_frame.regs[1].flags);
            cpu_flags_masked = ArduinoValidator::mask_undefined_flags(self.current_frame.opcode, self.current_frame.modrm, regs.flags);
        }

        if emu_flags_masked != cpu_flags_masked {

            log::error!("CPU flags mismatch! EMU: 0b{:08b} != CPU: 0b{:08b}", emu_flags_masked, cpu_flags_masked);
            //log::error!("Unmasked: EMU: 0b{:08b} != CPU: 0b{:08b}", self.current_frame.regs[1].flags, regs.flags);            
            regs_validate = false;

            let flag_diff = emu_flags_masked ^ cpu_flags_masked;

            if flag_diff & CPU_FLAG_CARRY != 0 {
                log::error!("CARRY flag differs.");
            }
            if flag_diff & CPU_FLAG_PARITY != 0 {
                log::error!("PARITY flag differs.");
            }
            if flag_diff & CPU_FLAG_AUX_CARRY != 0 {
                log::error!("AUX CARRY flag differs.");
            }
            if flag_diff & CPU_FLAG_ZERO != 0 {
                log::error!("ZERO flag differs.");
            }
            if flag_diff & CPU_FLAG_SIGN != 0 {
                log::error!("SIGN flag differs.");
            }
            if flag_diff & CPU_FLAG_TRAP != 0 {
                log::error!("TRAP flag differs.");
            }
            if flag_diff & CPU_FLAG_INT_ENABLE != 0 {
                log::error!("INT flag differs.");
            }
            if flag_diff & CPU_FLAG_DIRECTION != 0 {
                log::error!("DIRECTION flag differs.");
            }
            if flag_diff & CPU_FLAG_OVERFLOW != 0 {
                log::error!("OVERFLOW flag differs.");
            }                    
            //panic!("CPU flag mismatch!")
        }

        regs_validate
    }

    pub fn validate_cycles(
        &mut self, 
        cpu_states: &Vec::<CycleState>, 
        emu_states: &Vec::<CycleState>
    ) -> (bool, usize) {

        if emu_states.len() != cpu_states.len() {
            // Cycle count mismatch
            return (false, 0)
        }

        for i in 0..cpu_states.len() {

            if emu_states[i] != cpu_states[i] {
                // Cycle state mismatch
                return (false, i)
            }
        }

        (true, 0)
    }

    pub fn correct_queue_counts(&mut self, cpu_states: &mut Vec::<CycleState>) {

        for i in 0..cpu_states.len() {

            match cpu_states[i].q_op {
                QueueOp::First | QueueOp::Subsequent => {
                    if i > 0 {
                        // Queue was read out on previous cycle, adjust.
                        cpu_states[i-1].q_len -= 1;
                    }
                }
                QueueOp::Flush => {
                    if i > 0 {
                        // Queue was flushed on previous cycle, adjust.
                        cpu_states[i-1].q_len = 0;
                    }
                }
                _ => {}
            }
        }
    }

    pub fn print_cycle_diff(&mut self, cpu_states: &Vec::<CycleState>, emu_states: &Vec::<CycleState>) {

        let max_lines = cmp::max(emu_states.len(), cpu_states.len());

        for i in 0..max_lines {

            let cpu_str;
            let emu_str;
            
            if i < cpu_states.len() {
                cpu_str = RemoteCpu::get_cycle_state_str(&cpu_states[i])
            }
            else {
                cpu_str = String::new();
            }

            if i < emu_states.len() {
                emu_str = RemoteCpu::get_cycle_state_str(&emu_states[i])
            }
            else {
                emu_str = String::new();
            }

            println!("{:<80} | {:<80}", cpu_str, emu_str);
        }
    }    
}

impl RemoteCpu {

    pub fn update_state(&mut self) -> CycleState {

        /*
        self.program_state = self.cpu_client.get_program_state().expect("Failed to get program state!");
        self.status = self.cpu_client.read_status().expect("Failed to get status!");
        self.command_status = self.cpu_client.read_8288_command().expect("Failed to get 8288 command status!");
        self.control_status = self.cpu_client.read_8288_control().expect("Failed to get 8288 control status!");
        self.data_bus = self.cpu_client.read_data_bus().expect("Failed to get data bus!");
        */

        (
            self.program_state, 
            self.control_status, 
            self.status, 
            self.command_status, 
            self.data_bus
        ) = self.cpu_client.get_cycle_state().expect("Failed to get cycle state!");

        self.access_type = get_access_type!(self.status);
        self.bus_state = get_bus_state!(self.status);
        let q_op = get_queue_op!(self.status);

        CycleState {
            n: self.cycle_num,
            addr: self.address_latch,
            t_state: self.bus_cycle,
            a_type: self.access_type,
            b_state: self.bus_state,
            ale: self.command_status & COMMAND_ALE_BIT != 0,
            mrdc: self.command_status & COMMAND_MRDC_BIT != 0,
            amwc: self.command_status & COMMAND_AMWC_BIT != 0,
            mwtc: self.command_status & COMMAND_MWTC_BIT != 0,
            iorc: self.command_status & COMMAND_IORC_BIT != 0,
            aiowc: self.command_status & COMMAND_AIOWC_BIT != 0,
            iowc: self.command_status & COMMAND_IOWC_BIT != 0,
            inta: self.command_status & COMMAND_INTA_BIT != 0,
            q_op: q_op,
            q_byte: 0,
            q_len: 0,
            data_bus: 0,
        }
    }

    pub fn set_instr_string(&mut self, instr_str: String) {
        self.instr_str = instr_str;
    }

    pub fn reset(&mut self) {
        self.bus_cycle = BusCycle::T1;
        self.queue.flush();
    }

    pub fn cycle(
        &mut self,
        instr: &[u8],
        emu_prefetch: &Vec<BusOp>,
        emu_mem_ops: &Vec<BusOp>,
        cpu_prefetch: &mut Vec<BusOp>, 
        cpu_mem_ops: &mut Vec<BusOp>
    ) -> Result<CycleState, ValidatorError> {

        self.cpu_client.cycle().expect("Failed to cycle cpu!");
        
        self.bus_cycle = match self.bus_cycle {
            BusCycle::T1 => {
                // Capture the state of the bus transfer in T1, as the state will go PASV in t3-t4
                self.mcycle_state = get_bus_state!(self.status);
                
                // Only exit T1 state if bus transfer state indicates a bus transfer
                match get_bus_state!(self.status) {
                    BusState::PASV => BusCycle::T1,
                    BusState::HALT => BusCycle::T1,
                    _ => BusCycle::T2
                }
            }
            BusCycle::T2 => {
                BusCycle::T3
            }
            BusCycle::T3 => {
                // TODO: Handle wait states
                if self.mcycle_state == BusState::CODE {
                    // We completed a code fetch, so add to prefetch queue
                    self.queue.push(self.data_bus, self.data_type, self.address_latch);
                }
                BusCycle::T4
            }
            BusCycle::Tw => {
                // TODO: Handle wait states
                BusCycle::T4
            }            
            BusCycle::T4 => {

                BusCycle::T1
            }            
        };

        let mut cycle_info = self.update_state();
        if self.program_state == ProgramState::ExecuteDone {
            return Ok(cycle_info)
        }

        if(self.command_status & COMMAND_ALE_BIT) != 0 {
            if self.bus_cycle != BusCycle::T1 {
                log::warn!("ALE on non-T1 cycle state! CPU desynchronized.");
                self.bus_cycle = BusCycle::T1;
                return Err(ValidatorError::CpuDesynced);
            }

            self.address_latch = self.cpu_client.read_address_latch().expect("Failed to get address latch!");
            cycle_info.addr = self.address_latch;
        }

        // Do reads & writes if we are in execute state.
        if self.program_state == ProgramState::Execute {
            // MRDC status is active-low.
            if (self.command_status & COMMAND_MRDC_BIT) == 0 {
                // CPU is reading from bus.

                if self.bus_state == BusState::CODE {
                    // CPU is reading code.
                    if self.v_pc < instr.len() {
                        // Feed current instruction to CPU
                        self.data_bus = instr[self.v_pc];
                        self.data_type = QueueDataType::Program;
                        self.v_pc += 1;
                    }
                    else {
                        // Fetch past end of instruction. Send NOP
                        self.data_bus = OPCODE_NOP;
                        self.data_type = QueueDataType::Finalize;
                    }

                    //log::trace!("CPU fetch: {:02X}", self.data_bus);
                    self.cpu_client.write_data_bus(self.data_bus).expect("Failed to write data bus.");
                }
                if self.bus_state == BusState::MEMR {
                    // CPU is reading data from memory.
                    if self.busop_n < emu_mem_ops.len() {
                        
                        assert!(emu_mem_ops[self.busop_n].op_type == BusOpType::MemRead);
                        // Feed emulator byte to CPU
                        self.data_bus = emu_mem_ops[self.busop_n].data;
                        // Add emu op to CPU BusOp list
                        cpu_mem_ops.push(emu_mem_ops[self.busop_n].clone());
                        self.busop_n += 1;

                        //log::trace!("CPU read: {:02X}", self.data_bus);
                        self.cpu_client.write_data_bus(self.data_bus).expect("Failed to write data bus.");
                    }
                }             
            }

            // MWTC status is active-low.
            if (self.command_status & COMMAND_MWTC_BIT) == 0 {
                // CPU is writing to bus. MWTC is only active on t3 so we don't need an additional check.

                if self.busop_n < emu_mem_ops.len() {

                    //assert!(emu_mem_ops[self.busop_n].op_type == BusOpType::MemWrite);
                
                    // Read byte from CPU
                    self.data_bus = self.cpu_client.read_data_bus().expect("Failed to read data bus.");
                
                    //log::trace!("CPU write: [{:05X}] <- {:02X}", self.address_latch, self.data_bus);

                    // Add write op to CPU BusOp list
                    cpu_mem_ops.push(
                        BusOp {
                            op_type: BusOpType::MemWrite,
                            addr: self.address_latch,
                            data: self.data_bus,
                            flags: 0
                        }
                    );
                    self.busop_n += 1;
                }                   
            }

            // IORC status is active-low.
            if (self.command_status & COMMAND_IORC_BIT) == 0 {
                // CPU is reading from IO address.
                log::trace!("validator: Unhandled IO op");
            }

            // IOWC status is active-low.
            if (self.command_status & COMMAND_IOWC_BIT) == 0 {
                // CPU is writing to IO address.
                log::trace!("validator: Unhandled IO op");
            }
        }

        // Handle queue activity
        let q_op = get_queue_op!(self.status);

        match q_op {
            QueueOp::First | QueueOp::Subsequent => {
                // We fetched a byte from the queue last cycle
                (self.queue_byte, self.queue_type, self.queue_fetch_addr) = self.queue.pop();
                if q_op == QueueOp::First {
                    // First byte of instruction fetched.
                    self.queue_first_fetch = true;
                    self.queue_fetch_n = 0;
                    self.opcode = self.queue_byte;

                    // Is this opcode flagged as the end of execution?
                    if self.queue_type == QueueDataType::Finalize {
                        //log::trace!("Finalizing execution!");
                        self.cpu_client.finalize().expect("Failed to finalize!");
                        self.finalize = true;
                    }
                }
                else {
                    // Subsequent byte of instruction fetched
                    self.queue_fetch_n += 1;
                }
            }
            QueueOp::Flush => {
                // Queue was flushed last cycle
                self.flushed = true;
                self.queue.flush();
            }
            _ => {}
        }

        cycle_info.q_byte = self.queue_byte;
        cycle_info.data_bus = self.data_bus as u16;
        cycle_info.q_len = self.queue.len() as u32;

        self.cycle_num += 1;
        cycle_info.n = self.cycle_num;
        Ok(cycle_info)
    }

    pub fn run(
        &mut self, 
        instr: &[u8],
        instr_addr: u32,
        cycle_trace: bool,
        emu_prefetch: &Vec<BusOp>,
        emu_mem_ops: &Vec<BusOp>,
        cpu_prefetch: &mut Vec<BusOp>, 
        cpu_mem_ops: &mut Vec<BusOp>
    ) -> Result<Vec::<CycleState>, ValidatorError> {
    
        self.instr_addr = instr_addr;
        self.busop_n = 0;
        self.prefetch_n = 0;
        self.v_pc = 0;
        self.cycle_num = 0;
        self.finalize = false;
        self.flushed = false;

        self.address_latch = self.cpu_client.read_address_latch().expect("Failed to get address latch!");
        
        let mut cycle_vec: Vec<CycleState> = Vec::new();

        let cycle_state = self.update_state();

        cycle_vec.push(cycle_state);
        
        // ALE should be active at start of execution
        if self.command_status & COMMAND_ALE_BIT == 0 {
            log::warn!("Execution is not starting on T1.");
        }

        // cycle trace if enabled
        if cycle_trace == true {
            //println!("{}", self.get_cpu_state_str());
        }

        while self.program_state != ProgramState::ExecuteDone {
            let cycle_state = self.cycle(instr, emu_prefetch, emu_mem_ops, cpu_prefetch, cpu_mem_ops)?;
            
            
            if !self.finalize {
                cycle_vec.push(cycle_state);
            }

            // cycle trace if enabled
            if cycle_trace == true {
                //println!("{}", self.get_cpu_state_str());
            }
        }

        Ok(cycle_vec)
    }

    pub fn load(&mut self, reg_buf: &[u8]) -> Result<bool, CpuClientError> {

        self.cpu_client.load_registers_from_buf(&reg_buf)?;
        Ok(true)
    }

    /*
    pub fn store(&mut self) -> Result<VRegisters, CpuClientError> {

        // Enter store state
        self.cpu_client.begin_store()?;

        // Run Store state until StoreDone
        while self.program_state != ProgramState::StoreDone {
            //log::trace!("Validator state: {:?}", self.program_state);
            self.cpu_client.cycle()?;
            self.program_state = self.cpu_client.get_program_state().expect("Failed to get program state!");
        }

        let mut buf: [u8; 28] = [0; 28];
        self.cpu_client.store_registers_to_buf(&mut buf)?;

        let regs = ArduinoValidator::buf_to_regs(&buf);
        
        RemoteCpu::print_regs(&regs);
        Ok(regs)
    } 
    */
    pub fn store(&mut self) -> Result<VRegisters, CpuClientError> {
        let mut buf: [u8; 28] = [0; 28];
        self.cpu_client.store_registers_to_buf(&mut buf)?;

        Ok(ArduinoValidator::buf_to_regs(&buf))
    }

    pub fn calc_linear_address(segment: u16, offset: u16) -> u32 {
        ((segment as u32) << 4) + offset as u32 & 0xFFFFFu32
    }

    /// Adjust stored IP register to address of terminating opcode fetch
    pub fn adjust_ip(&mut self, regs: &mut VRegisters) {

        let flat_csip = RemoteCpu::calc_linear_address(regs.cs, regs.ip);

        let ip_offset = flat_csip.wrapping_sub(self.queue_fetch_addr);

        regs.ip = regs.ip.wrapping_sub(ip_offset as u16);
    }

    pub fn get_cycle_state_str(c: &CycleState) -> String {

        let ale_str = match c.ale {
            true => "A:",
            false => "  "
        };

        let mut seg_str = "  ";
        if c.t_state != BusCycle::T1 {
            // Segment status only valid in T2+
            seg_str = match c.a_type {
                AccessType::AccAlternateData  => "ES",
                AccessType::AccStack => "SS",
                AccessType::AccCodeOrNone => "CS",
                AccessType::AccData => "DS"
            };    
        }

        let q_op_chr = match c.q_op {
            QueueOp::Idle => ' ',
            QueueOp::First => 'F',
            QueueOp::Flush => 'E',
            QueueOp::Subsequent => 'S'
        };

        // All read/write signals are active/low
        let rs_chr   = match !c.mrdc {
            true => 'R',
            false => '.',
        };
        let aws_chr  = match !c.aiowc {
            true => 'A',
            false => '.',
        };
        let ws_chr   = match !c.mwtc {
            true => 'W',
            false => '.',
        };
        let ior_chr  = match !c.iorc {
            true => 'R',
            false => '.',
        };
        let aiow_chr = match !c.aiowc {
            true => 'A',
            false => '.',
        };
        let iow_chr  = match !c.iowc {
            true => 'W',
            false => '.',
        };        

        let bus_str = match c.b_state {
            BusState::INTA => "INTA",
            BusState::IOR  => "IOR ",
            BusState::IOW  => "IOW ",
            BusState::HALT => "HALT",
            BusState::CODE => "CODE",
            BusState::MEMR => "MEMR",
            BusState::MEMW => "MEMW",
            BusState::PASV => "PASV"           
        };

        let t_str = match c.t_state {
            BusCycle::T1 => "T1",
            BusCycle::T2 => "T2",
            BusCycle::T3 => "T3",
            BusCycle::T4 => "T4",
            BusCycle::Tw => "Tw",
        };

        let is_reading = !c.mrdc;
        let is_writing = !c.mwtc;

        let mut xfer_str = "      ".to_string();
        if is_reading {
            xfer_str = format!("<-r {:02X}", c.data_bus);
        }
        else if is_writing {
            xfer_str = format!("w-> {:02X}", c.data_bus);
        }

        let mut q_read_str = String::new();

        if c.q_op == QueueOp::First {
            // First byte of opcode read from queue. Decode it to opcode or group specifier
            q_read_str = format!("<-q {:02X}", c.q_byte);
        }
        else if c.q_op == QueueOp::Subsequent {
            q_read_str = format!("<-q {:02X}", c.q_byte);
        }         
      
        format!(
            //"{:08} {:02}[{:05X}] {:02} M:{}{}{} I:{}{}{} {:04} {:02} {:06} | {:1}{:1} [{:08}] {}",
            "{:08} {:02}[{:05X}] {:02} M:{}{}{} I:{}{}{} {:04} {:02} {:06} | {:1}{:1} {}",
            c.n,
            ale_str,
            c.addr,
            seg_str,
            rs_chr, aws_chr, ws_chr, ior_chr, aiow_chr, iow_chr,
            bus_str,
            t_str,
            xfer_str,
            q_op_chr,
            c.q_len,
            //self.queue.to_string(),
            q_read_str
        )        
    }

    pub fn get_cpu_state_str(&mut self) -> String {

        let ale_str = match self.command_status & COMMAND_ALE_BIT != 0 {
            true => "A:",
            false => "  "
        };

        let mut seg_str = "  ";
        if self.bus_cycle != BusCycle::T1 {
            // Segment status only valid in T2+
            seg_str = match get_segment!(self.status) {
                Segment::ES => "ES",
                Segment::SS => "SS",
                Segment::CS => "CS",
                Segment::DS => "DS"
            };    
        }

        let q_op = get_queue_op!(self.status);
        let q_op_chr = match q_op {
            QueueOp::Idle => ' ',
            QueueOp::First => 'F',
            QueueOp::Flush => 'E',
            QueueOp::Subsequent => 'S'
        };

        // All read/write signals are active/low
        let rs_chr   = match self.command_status & 0b0000_0001 == 0 {
            true => 'R',
            false => '.',
        };
        let aws_chr  = match self.command_status & 0b0000_0010 == 0 {
            true => 'A',
            false => '.',
        };
        let ws_chr   = match self.command_status & 0b0000_0100 == 0 {
            true => 'W',
            false => '.',
        };
        let ior_chr  = match self.command_status & 0b0000_1000 == 0 {
            true => 'R',
            false => '.',
        };
        let aiow_chr = match self.command_status & 0b0001_0000 == 0 {
            true => 'A',
            false => '.',
        };

        let iow_chr  = match self.command_status & 0b0010_0000 == 0 {
            true => 'W',
            false => '.',
        };

        let bus_str = match get_bus_state!(self.status) {
            BusState::INTA => "INTA",
            BusState::IOR  => "IOR ",
            BusState::IOW  => "IOW ",
            BusState::HALT => "HALT",
            BusState::CODE => "CODE",
            BusState::MEMR => "MEMR",
            BusState::MEMW => "MEMW",
            BusState::PASV => "PASV"           
        };

        let t_str = match self.bus_cycle {
            BusCycle::T1 => "T1",
            BusCycle::T2 => "T2",
            BusCycle::T3 => "T3",
            BusCycle::T4 => "T4",
            BusCycle::Tw => "Tw",
        };

        let is_reading = is_reading!(self.command_status);
        let is_writing = is_writing!(self.command_status);

        let mut xfer_str = "      ".to_string();
        if is_reading {
            xfer_str = format!("<-r {:02X}", self.data_bus);
        }
        else if is_writing {
            xfer_str = format!("w-> {:02X}", self.data_bus);
        }

        // Handle queue activity

        let mut q_read_str = String::new();

        if q_op == QueueOp::First {
            // First byte of opcode read from queue. Decode it to opcode or group specifier
            q_read_str = format!("<-q {:02X} {}", self.queue_byte, self.instr_str);
        }
        else if q_op == QueueOp::Subsequent {
            q_read_str = format!("<-q {:02X}", self.queue_byte);
        }         
      
        format!(
            "{:08} {:02}[{:05X}] {:02} M:{}{}{} I:{}{}{} {:04} {:02} {:06} | {:1}{:1} [{:08}] {}",
            self.cycle_num,
            ale_str,
            self.address_latch,
            seg_str,
            rs_chr, aws_chr, ws_chr, ior_chr, aiow_chr, iow_chr,
            bus_str,
            t_str,
            xfer_str,
            q_op_chr,
            self.queue.len(),
            self.queue.to_string(),
            q_read_str
        )
    }

    pub fn print_regs(regs: &VRegisters) {
        let reg_str = format!(
            "AX: {:04x} BX: {:04x} CX: {:04x} DX: {:04x}\n\
            SP: {:04x} BP: {:04x} SI: {:04x} DI: {:04x}\n\
            CS: {:04x} DS: {:04x} ES: {:04x} SS: {:04x}\n\
            IP: {:04x}\n\
            FLAGS: {:04x}",
            regs.ax, regs.bx, regs.cx, regs.dx,
            regs.sp, regs.bp, regs.si, regs.di,
            regs.cs, regs.ds, regs.es, regs.ss,
            regs.ip,
            regs.flags );
        
          println!("{}", reg_str);        
    }
}

pub fn make_pointer(base: u16, offset: u16) -> u32 {
    return (((base as u32) << 4) + offset as u32 ) & 0xFFFFF;
}

impl CpuValidator for ArduinoValidator {

    fn init(&mut self, mask_flags: bool, cycle_trace: bool, visit_once: bool) -> bool {

        self.cycle_trace = cycle_trace;
        self.mask_flags = mask_flags;
        self.visit_once = visit_once;
        true
    }

    fn begin(&mut self, regs: &VRegisters ) {

        self.current_frame.discard = false;
        self.current_frame.regs[0] = regs.clone();

        //RemoteCpu::print_regs(&self.current_frame.regs[0]);

        let ip_addr = make_pointer(regs.cs, regs.ip);

        //println!("{} : {}", self.trigger_addr, ip_addr);
        if self.trigger_addr == ip_addr {
            log::info!("Trigger address hit, begin validation...");
            self.trigger_addr = V_INVALID_POINTER;
        }

        /*
        if (self.trigger_addr != V_INVALID_POINTER) 
            || (self.visit_once && ip_addr >= UPPER_MEMORY && self.visited[ip_addr as usize]) {
            self.current_frame.discard = true;
            return;
        }
        */
        if (self.visit_once && ip_addr >= UPPER_MEMORY && self.visited[ip_addr as usize]) {
            self.current_frame.discard = true;
        }

        self.current_frame.emu_ops.clear();
        self.current_frame.emu_prefetch.clear();
        self.current_frame.cpu_ops.clear();
        self.current_frame.cpu_prefetch.clear();
    }    

    fn validate(
        &mut self, 
        name: String, 
        instr: &[u8], 
        has_modrm: bool, 
        cycles: i32, 
        regs: &VRegisters,
        emu_states: &Vec<CycleState>,
    ) -> Result<bool, ValidatorError>  {

        let ip_addr = make_pointer(self.current_frame.regs[0].cs, self.current_frame.regs[0].ip);

        /*
        if (self.trigger_addr != V_INVALID_POINTER) 
            || (self.visit_once && ip_addr >= UPPER_MEMORY &&  self.visited[ip_addr as usize]) {
            return Ok(true);
        }
        */

        if instr.len() == 0 {
            log::error!("Instruction length was 0");
            return Err(ValidatorError::ParameterError);
        }

        self.visited[ip_addr as usize] = true;

        let mut i = 0;

        // Scan through prefix bytes to find opcode
        loop {
            let mut instr_byte = instr[i];
            match instr_byte {
                0x26 | 0x2E | 0x36 | 0x3E | 0xF0 | 0xF2 | 0xF3 => {
                    i += 1;
                }
                _ => {
                    break;
                }
            }
        }

        self.current_frame.name = name.clone();
        self.current_frame.opcode = instr[i];
        self.current_frame.instr = instr.to_vec();
        self.current_frame.has_modrm = has_modrm;
        self.current_frame.num_nop = 0;
        self.current_frame.next_fetch = false;
        self.current_frame.regs[1] = regs.clone();

        if self.current_frame.regs[1].flags == 0 {
            log::error!("Invalid emulator flags");
            return Err(ValidatorError::ParameterError);
        }

        //self.current_frame.emu_states.clone_from(&emu_states);

        RemoteCpu::print_regs(&self.current_frame.regs[0]);

        if has_modrm {
            if i > (instr.len() - 2) {
                log::error!("validate(): modrm specified but instruction length < ");
                log::error!(
                    "instruction: {} opcode: {} instr: {:02X?}",
                    self.current_frame.name,
                    self.current_frame.opcode,
                    self.current_frame.instr
                );
                return Err(ValidatorError::ParameterError);
            }
            self.current_frame.modrm = instr[i + 1];
        }
        else {
            self.current_frame.modrm = 0;
        }

        let discard_or_validate = match self.current_frame.discard {
            true => "DISCARD",
            false => "VALIDATE"
        };

        self.cpu.reset();
        self.cpu.set_instr_string(name.clone());

        log::debug!(
            "{}: {} {:02X?} @ [{:04X}:{:04X}] Memops: {}", 
            discard_or_validate, 
            name, 
            self.current_frame.instr, 
            self.current_frame.regs[0].cs, 
            self.current_frame.regs[0].ip,
            self.current_frame.emu_ops.len(),
        );

        if self.current_frame.discard {
            return Ok(true);
        }

        let mut reg_buf: [u8; 28] = [0; 28];
        ArduinoValidator::regs_to_buf(&mut reg_buf, &self.current_frame.regs[0]);

        self.cpu.load(&reg_buf).expect("validate() error: Load registers failed.");

        let instr_addr = RemoteCpu::calc_linear_address(self.current_frame.regs[0].cs, self.current_frame.regs[0].ip);

        let mut cpu_states = self.cpu.run(
            &self.current_frame.instr,
            instr_addr,
            self.cycle_trace,
            &mut self.current_frame.emu_prefetch, 
            &mut self.current_frame.emu_ops, 
            &mut self.current_frame.cpu_prefetch, 
            &mut self.current_frame.cpu_ops
        )?;


        // Enter store state

        //log::trace!("Validator state: {:?}", self.cpu.program_state);

        let mut regs = self.cpu.store().expect("Failed to store registers!");

        if !self.validate_mem_ops() {
            log::error!("Memory validation failure. EMU:");
            RemoteCpu::print_regs(&self.current_frame.regs[1]);
            log::error!("CPU:");    
            RemoteCpu::print_regs(&regs);

            return Err(ValidatorError::MemOpMismatch);            
        }

        self.cpu.adjust_ip(&mut regs);

        if !self.validate_registers(&regs) {
            log::error!("Register validation failure. EMU BEFORE:");    
            RemoteCpu::print_regs(&self.current_frame.regs[0]);
            log::error!("EMU AFTER:");
            RemoteCpu::print_regs(&self.current_frame.regs[1]);

            log::error!("CPU AFTER:");   
            RemoteCpu::print_regs(&regs);

            return Err(ValidatorError::RegisterMismatch);
        }

        if emu_states.len() > 0 {
            // Only validate CPU cycles if any were provided

            self.correct_queue_counts(&mut cpu_states);
            let (result, cycle_num) = self.validate_cycles(&cpu_states, &emu_states);

            if !result {
                log::error!("Cycle state validation failure @ cycle {}", cycle_num);    
                self.print_cycle_diff(&cpu_states, &emu_states);
                return Err(ValidatorError::CycleMismatch);
            }
            else {
                self.print_cycle_diff(&cpu_states, &emu_states);
            }
        }


        Ok(true)
    }

    fn emu_read_byte(&mut self, addr: u32, data: u8, read_type: ReadType) {
        if self.current_frame.discard {
            return;
        }

        match read_type { 
            ReadType::Code => {
                self.current_frame.emu_prefetch.push(
                    BusOp {
                        op_type: BusOpType::CodeRead,
                        addr,
                        data,
                        flags: MOF_EMULATOR
                    }
                );
                log::trace!("EMU fetch: [{:05X}] -> 0x{:02X}", addr, data);
            }
            ReadType::Data => {
                let ops_len = self.current_frame.emu_ops.len();
                if ops_len > 0 {
                    if self.current_frame.emu_ops[ops_len - 1].addr == addr {
                        log::trace!("EMU duplicate read!");
                        println!("Custom backtrace: {}", Backtrace::force_capture());
                    }
                }

                self.current_frame.emu_ops.push(
                    BusOp {
                        op_type: BusOpType::MemRead,
                        addr,
                        data,
                        flags: MOF_EMULATOR
                    }
                );
                log::trace!("EMU read: [{:05X}] -> 0x{:02X}", addr, data);
            }
        }

    }

    fn emu_write_byte(&mut self, addr: u32, data: u8) {
        
        self.visited[(addr & 0xFFFFF) as usize] = false;
        
        if self.current_frame.discard {
            return;
        }

        self.current_frame.emu_ops.push(
            BusOp {
                op_type: BusOpType::MemWrite,
                addr,
                data,
                flags: MOF_EMULATOR
            }
        );
        //log::trace!("EMU write: [{:05X}] <- 0x{:02X}", addr, data);
    }

    fn discard_op(&mut self) {
        self.current_frame.discard = true;
    }
}

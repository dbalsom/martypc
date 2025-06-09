/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2025 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the “Software”),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.
*/
#![allow(dead_code)]

mod code_stream;
mod queue;
pub mod remote_cpu;
mod remote_program;
mod udmask;

pub use ard808x_client::BusState;
use ard808x_client::*;

use crate::{
    cpu_808x::{
        CPU_FLAG_AUX_CARRY,
        CPU_FLAG_CARRY,
        CPU_FLAG_DIRECTION,
        CPU_FLAG_INT_ENABLE,
        CPU_FLAG_OVERFLOW,
        CPU_FLAG_PARITY,
        CPU_FLAG_SIGN,
        CPU_FLAG_TRAP,
        CPU_FLAG_ZERO,
    },
    cpu_common::{CpuType, QueueOp},
    cpu_validator::*,
    tracelogger::TraceLogger,
};
use remote_cpu::*;
use std::{cmp, collections::HashSet};

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
const OPCODE_NOPS: u16 = 0x9090;

macro_rules! trace {
    ($self:ident, $($t:tt)*) => {{
        $self.trace_logger.print(&format!($($t)*));
        $self.trace_logger.print("\n".to_string());
    }};
}

macro_rules! trace_debug {
    ($self:ident, $($t:tt)*) => {{
        log::debug!("{}", &format!($($t)*));
        $self.trace_logger.print(&format!($($t)*));
        $self.trace_logger.print("\n".to_string());
    }};
}

macro_rules! trace_error {
    ($self:ident, $($t:tt)*) => {{
        log::error!("{}", &format!($($t)*));
        $self.trace_logger.print(&format!($($t)*));
        $self.trace_logger.print("\n".to_string());
    }};
}

pub struct ValidatorOptions {
    pub vtype: ValidatorType,
    pub trace: TraceLogger,
    pub mode:  ValidatorMode,
    pub baud:  Option<u32>,
    pub port:  Option<String>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RegisterValidationResult {
    Ok,
    GeneralMismatch,
    FlagMismatch,
    BothMismatch,
}

#[derive(Default)]
pub struct InstructionContext {
    name: String,
    instr: Vec<u8>,
    instr_end: usize,
    prefetch: bool,
    opcode: u8,
    modrm: u8,
    has_modrm: bool,
    discard: bool,
    next_fetch: bool,

    initial_queue: Vec<u8>,
    regs: [VRegisters; 2],
    cpu_after_regs: Option<VRegisters>,

    emu_fetches: Vec<BusOp>,
    emu_ops: Vec<BusOp>,
    cpu_fetches: Vec<BusOp>,
    cpu_ops: Vec<BusOp>,
    mem_op_n: usize,

    visited_fetches: HashSet<u32>,

    cpu_states: Vec<CycleState>,
}

impl InstructionContext {
    pub fn new() -> Self {
        Self {
            ..InstructionContext::default()
        }
    }
}

pub fn difference<T: Ord + std::ops::Sub<T, Output = T>>(a: T, b: T) -> T {
    if a > b {
        a - b
    }
    else {
        b - a
    }
}
pub struct ArduinoValidator {
    //cpu_client: Option<CpuClient>,
    mode: ValidatorMode,
    cpu: RemoteCpu,
    cpu_type: CpuType,

    current_instr: InstructionContext,

    //state: ValidatorState,
    cycle_count:    u64,
    do_cycle_trace: bool,

    rd_signal:  bool,
    wr_signal:  bool,
    iom_signal: bool,
    ale_signal: bool,

    address_latch: u32,
    address_bus:   u32,

    //cpu_memory_access: AccessType,
    cpu_interrupt_enabled: bool,

    scratchpad: Vec<u8>,
    code_as_data_skip: bool,
    readback_ptr: usize,
    trigger_addr: u32,
    end_addr: usize,

    mask_flags: bool,

    visit_once: bool,
    visited:    Vec<bool>,

    last_cpu_states: Vec<CycleState>,
    last_emu_ops:    Vec<BusOp>,
    last_cpu_ops:    Vec<BusOp>,
    last_cpu_queue:  Vec<u8>,

    log_prefix:   String,
    trace_logger: TraceLogger,

    opt_ignore_underflow: bool,
    opt_validate_regs: bool,
    opt_validate_flags: bool,
    opt_validate_mem: bool,
    opt_validate_cycles: bool,
}

impl ArduinoValidator {
    pub fn new(cpu_type: CpuType, trace_logger: TraceLogger, port: Option<String>, _baud_rate: Option<u32>) -> Self {
        // Trigger addr is address at which to start validation
        // if trigger_addr == V_INVALID_POINTER then validate
        let trigger_addr = V_INVALID_POINTER;

        let cpu_client = match CpuClient::init(port) {
            Ok(client) => client,
            Err(e) => {
                panic!("Failed to initialize ArduinoValidator: {}", e);
            }
        };

        ArduinoValidator {
            mode: ValidatorMode::Cycle,
            cpu: RemoteCpu::new(cpu_type, cpu_client),
            cpu_type,

            current_instr: InstructionContext::new(),
            //state: ValidatorState::Setup,
            cycle_count: 0,
            do_cycle_trace: false,
            rd_signal: false,
            wr_signal: false,
            iom_signal: false,
            ale_signal: false,
            address_latch: 0,
            address_bus: 0,
            //cpu_memory_access: AccessType::AccAlternateData,
            cpu_interrupt_enabled: false,

            scratchpad: vec![0; 0x100000],
            code_as_data_skip: false,
            readback_ptr: 0,
            trigger_addr,
            end_addr: 0,
            mask_flags: true,
            visit_once: VISIT_ONCE,
            visited: vec![false; 0x100000],

            last_emu_ops: Vec::new(),
            last_cpu_ops: Vec::new(),
            last_cpu_states: Vec::new(),
            last_cpu_queue: Vec::new(),

            log_prefix: String::new(),

            trace_logger,

            opt_ignore_underflow: false,
            opt_validate_cycles: true,
            opt_validate_regs: true,
            opt_validate_flags: true,
            opt_validate_mem: true,
        }
    }

    pub fn set_end_addr(&mut self, end_addr: usize) {
        self.end_addr = end_addr;
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
            ax:    buf[0] as u16 | ((buf[1] as u16) << 8),
            bx:    buf[2] as u16 | ((buf[3] as u16) << 8),
            cx:    buf[4] as u16 | ((buf[5] as u16) << 8),
            dx:    buf[6] as u16 | ((buf[7] as u16) << 8),
            ss:    buf[8] as u16 | ((buf[9] as u16) << 8),
            sp:    buf[10] as u16 | ((buf[11] as u16) << 8),
            flags: buf[12] as u16 | ((buf[13] as u16) << 8),
            ip:    buf[14] as u16 | ((buf[15] as u16) << 8),
            cs:    buf[16] as u16 | ((buf[17] as u16) << 8),
            ds:    buf[18] as u16 | ((buf[19] as u16) << 8),
            es:    buf[20] as u16 | ((buf[21] as u16) << 8),
            bp:    buf[22] as u16 | ((buf[23] as u16) << 8),
            si:    buf[24] as u16 | ((buf[25] as u16) << 8),
            di:    buf[26] as u16 | ((buf[27] as u16) << 8),
        }
    }

    pub fn validate_mem_ops(&mut self, discard: bool, flags: u8) -> bool {
        if discard {
            if !self.current_instr.emu_ops.is_empty() {
                if self.current_instr.emu_ops[0].op_type != BusOpType::CodeRead {
                    trace_error!(
                        self,
                        "Cannot discard op type of {:?}!",
                        self.current_instr.emu_ops[0].op_type
                    );
                    return false;
                }
                else {
                    self.current_instr.emu_ops.remove(0);
                }
            }
            else {
                trace_error!(self, "Discard flag set but no emu ops!");
                return false;
            }
        }

        let ops_should_match = (flags & VAL_NO_READS == 0) && (flags & VAL_NO_WRITES == 0);

        if ops_should_match && (self.current_instr.emu_ops.len() != self.current_instr.cpu_ops.len()) {
            trace_error!(
                self,
                "Validator error: Memory op count mismatch. Emu: {} CPU: {}",
                self.current_instr.emu_ops.len(),
                self.current_instr.cpu_ops.len()
            );

            return false;
        }

        let min_op_n = cmp::min(self.current_instr.emu_ops.len(), self.current_instr.cpu_ops.len());

        for i in 0..min_op_n {
            if self.current_instr.emu_ops[i].op_type != self.current_instr.cpu_ops[i].op_type {
                trace_error!(
                    self,
                    "Bus op #{} type mismatch: EMU:{:?} CPU:{:?}",
                    i,
                    self.current_instr.emu_ops[i].op_type,
                    self.current_instr.cpu_ops[i].op_type
                );
                return false;
            }

            if self.current_instr.emu_ops[i].addr != self.current_instr.cpu_ops[i].addr {
                trace_error!(
                    self,
                    "Bus op #{} addr mismatch: EMU:{:?}:{:05X} CPU:{:?}:{:05X}",
                    i,
                    self.current_instr.emu_ops[i].op_type,
                    self.current_instr.emu_ops[i].addr,
                    self.current_instr.cpu_ops[i].op_type,
                    self.current_instr.cpu_ops[i].addr
                );
                return false;
            }

            let validate_data = match self.current_instr.emu_ops[i].op_type {
                BusOpType::MemWrite if flags & VAL_NO_WRITES != 0 => false,
                BusOpType::MemRead if flags & VAL_NO_READS != 0 => false,
                _ => true,
            };

            if validate_data && (self.current_instr.emu_ops[i].data != self.current_instr.cpu_ops[i].data) {
                trace_error!(
                    self,
                    "Bus op #{} data mismatch: EMU:{:?}:{:05X} CPU:{:?}:{:05X}",
                    i,
                    self.current_instr.emu_ops[i].op_type,
                    self.current_instr.emu_ops[i].data,
                    self.current_instr.cpu_ops[i].op_type,
                    self.current_instr.cpu_ops[i].data
                );
                return false;
            }
        }

        true
    }

    pub fn validate_registers(&mut self, regs: &VRegisters) -> RegisterValidationResult {
        let mut regs_validate = true;
        let flags_validate = true;

        if self.current_instr.regs[1].ax != regs.ax {
            regs_validate = false;
        }
        if self.current_instr.regs[1].bx != regs.bx {
            regs_validate = false;
        }
        if self.current_instr.regs[1].cx != regs.cx {
            regs_validate = false;
        }
        if self.current_instr.regs[1].dx != regs.dx {
            regs_validate = false;
        }
        if self.current_instr.regs[1].cs != regs.cs {
            regs_validate = false;
        }
        if self.current_instr.regs[1].ds != regs.ds {
            regs_validate = false;
        }
        if self.current_instr.regs[1].es != regs.es {
            regs_validate = false;
        }
        if self.current_instr.regs[1].sp != regs.sp {
            regs_validate = false;
        }
        if self.current_instr.regs[1].sp != regs.sp {
            regs_validate = false;
        }
        if self.current_instr.regs[1].bp != regs.bp {
            regs_validate = false;
        }
        if self.current_instr.regs[1].si != regs.si {
            regs_validate = false;
        }
        if self.current_instr.regs[1].di != regs.di {
            regs_validate = false;
        }

        let mut emu_flags_masked = self.current_instr.regs[1].flags;
        let mut cpu_flags_masked = regs.flags;

        if self.mask_flags {
            emu_flags_masked = ArduinoValidator::mask_undefined_flags(
                self.cpu_type,
                self.current_instr.opcode,
                self.current_instr.modrm,
                self.current_instr.regs[1].flags,
            );
            cpu_flags_masked = ArduinoValidator::mask_undefined_flags(
                self.cpu_type,
                self.current_instr.opcode,
                self.current_instr.modrm,
                regs.flags,
            );
        }

        if emu_flags_masked != cpu_flags_masked {
            trace_error!(
                self,
                "CPU flags mismatch! EMU: 0b{:08b} != CPU: 0b{:08b}",
                emu_flags_masked,
                cpu_flags_masked
            );
            //trace_error!(self, "Unmasked: EMU: 0b{:08b} != CPU: 0b{:08b}", self.current_frame.regs[1].flags, regs.flags);
            regs_validate = false;

            let flag_diff = emu_flags_masked ^ cpu_flags_masked;

            if flag_diff & CPU_FLAG_CARRY != 0 {
                trace_error!(
                    self,
                    "CARRY flag differs (cpu:{}, emu:{})",
                    cpu_flags_masked & CPU_FLAG_CARRY != 0,
                    emu_flags_masked & CPU_FLAG_CARRY != 0
                );
            }
            if flag_diff & CPU_FLAG_PARITY != 0 {
                trace_error!(
                    self,
                    "PARITY flag differs. (cpu:{}, emu:{})",
                    cpu_flags_masked & CPU_FLAG_PARITY != 0,
                    emu_flags_masked & CPU_FLAG_PARITY != 0
                );
            }
            if flag_diff & CPU_FLAG_AUX_CARRY != 0 {
                trace_error!(
                    self,
                    "AUX CARRY flag differs. (cpu:{}, emu:{})",
                    cpu_flags_masked & CPU_FLAG_AUX_CARRY != 0,
                    emu_flags_masked & CPU_FLAG_AUX_CARRY != 0
                );
            }
            if flag_diff & CPU_FLAG_ZERO != 0 {
                trace_error!(
                    self,
                    "ZERO flag differs. (cpu:{}, emu:{})",
                    cpu_flags_masked & CPU_FLAG_ZERO != 0,
                    emu_flags_masked & CPU_FLAG_ZERO != 0
                );
            }
            if flag_diff & CPU_FLAG_SIGN != 0 {
                trace_error!(
                    self,
                    "SIGN flag differs. (cpu:{}, emu:{})",
                    cpu_flags_masked & CPU_FLAG_SIGN != 0,
                    emu_flags_masked & CPU_FLAG_SIGN != 0
                );
            }
            if flag_diff & CPU_FLAG_TRAP != 0 {
                trace_error!(
                    self,
                    "TRAP flag differs. (cpu:{}, emu:{})",
                    cpu_flags_masked & CPU_FLAG_TRAP != 0,
                    emu_flags_masked & CPU_FLAG_TRAP != 0
                );
            }
            if flag_diff & CPU_FLAG_INT_ENABLE != 0 {
                trace_error!(
                    self,
                    "INT flag differs. (cpu:{}, emu:{})",
                    cpu_flags_masked & CPU_FLAG_INT_ENABLE != 0,
                    emu_flags_masked & CPU_FLAG_INT_ENABLE != 0
                );
            }
            if flag_diff & CPU_FLAG_DIRECTION != 0 {
                trace_error!(
                    self,
                    "DIRECTION flag differs. (cpu:{}, emu:{})",
                    cpu_flags_masked & CPU_FLAG_DIRECTION != 0,
                    emu_flags_masked & CPU_FLAG_DIRECTION != 0
                );
            }
            if flag_diff & CPU_FLAG_OVERFLOW != 0 {
                trace_error!(
                    self,
                    "OVERFLOW flag differs. (cpu:{}, emu:{})",
                    cpu_flags_masked & CPU_FLAG_OVERFLOW != 0,
                    emu_flags_masked & CPU_FLAG_OVERFLOW != 0
                );
            }
            //panic!("CPU flag mismatch!")
        }

        match (regs_validate, flags_validate) {
            (true, true) => RegisterValidationResult::Ok,
            (false, true) => RegisterValidationResult::GeneralMismatch,
            (true, false) => RegisterValidationResult::FlagMismatch,
            (false, false) => RegisterValidationResult::BothMismatch,
        }
    }

    pub fn validate_cycles(
        &mut self,
        flags: u8,
        cpu_states: &[CycleState],
        emu_states: &[CycleState],
    ) -> (bool, usize) {
        let difference = difference(emu_states.len(), cpu_states.len());

        // Allow a one cycle variance if appropriate flag is set, otherwise require lengths match.

        if flags & VAL_ALLOW_ONE != 0 {
            // Difference of up to one cycle is allowed..
            if difference > 1 {
                // But exceeded, fail!
                return (false, 0);
            }
            else if difference == 1 {
                // Cycle states are going to be different, so don't bother comparing.
                return (true, 0);
            }
            // Difference is 0, so continue as normal.
        }
        else if emu_states.len() != cpu_states.len() {
            // No difference was allowed, and difference was found. Failed.
            return (false, 0);
        }

        if difference == 0 || (flags & VAL_ALLOW_ONE == 0) {
            for i in 0..cpu_states.len() {
                if emu_states[i] != cpu_states[i] {
                    // Cycle state mismatch
                    return (false, i);
                }
            }
        }

        (true, 0)
    }

    pub fn correct_queue_counts(&mut self, cpu_states: &mut Vec<CycleState>) {
        for i in 0..cpu_states.len() {
            match cpu_states[i].q_op {
                QueueOp::First | QueueOp::Subsequent => {
                    if i > 0 {
                        // Queue was read out on previous cycle, adjust.
                        cpu_states[i - 1].q_len -= 1;
                    }
                }
                QueueOp::Flush => {
                    if i > 0 {
                        // Queue was flushed on previous cycle, adjust.
                        cpu_states[i - 1].q_len = 0;
                    }
                }
                _ => {}
            }
        }
    }

    pub fn print_cycle_diff(&mut self, cpu_states: &Vec<CycleState>, emu_states: &[CycleState]) {
        let max_lines = cmp::max(emu_states.len(), cpu_states.len());

        for i in 0..max_lines {
            let cpu_str = if i < cpu_states.len() {
                cpu_states[i].to_string()
            }
            else {
                String::new()
            };

            let emu_str = if i < emu_states.len() {
                emu_states[i].to_string()
            }
            else {
                String::new()
            };

            trace!(self, "{:<80} | {:<80}", cpu_str, emu_str);
        }
    }

    fn reset_after_validation(&mut self, cpu_states: Option<Vec<CycleState>>) {
        self.last_cpu_states = cpu_states.unwrap_or_default();
        self.last_cpu_ops = self.current_instr.cpu_ops.clone();
        self.last_emu_ops = self.current_instr.emu_ops.clone();
        self.last_cpu_queue = self.cpu.queue();
        self.reset_instruction();
    }
}

pub fn make_pointer(base: u16, offset: u16) -> u32 {
    (((base as u32) << 4) + offset as u32) & 0xFFFFF
}

impl CpuValidator for ArduinoValidator {
    fn init(&mut self, mode: ValidatorMode, mask_flags: bool, cycle_trace: bool, visit_once: bool) -> bool {
        self.mode = mode;
        self.do_cycle_trace = cycle_trace;
        self.mask_flags = mask_flags;
        self.visit_once = visit_once;
        true
    }

    fn reset_instruction(&mut self) {
        self.current_instr.emu_ops.clear();
        self.current_instr.emu_fetches.clear();
        self.current_instr.cpu_ops.clear();
        self.current_instr.cpu_fetches.clear();
    }

    fn begin_instruction(&mut self, regs: &VRegisters, end_instr: usize, end_program: usize) {
        self.current_instr.discard = false;
        self.current_instr.regs[0] = *regs;

        //log::debug!(">>> printing regs!");
        //RemoteCpu::print_regs(&self.current_instr.regs[0]);

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

        /* disable discarding for now
        if let ValidatorMode::Instruction = self.mode {
            // Cannot discard instructions in Cycle mode.
            if self.visit_once && ip_addr >= UPPER_MEMORY && self.visited[ip_addr as usize] {
                log::warn!("Discarding BIOS instruction");
                self.current_instr.discard = true;
            }
        }
        */

        self.end_addr = end_program;

        self.current_instr.instr_end = end_instr;
        trace_debug!(self, "Set end addr for instruction: {:05X}", end_instr);
        self.cpu.set_instr_end_addr(end_instr);
        self.cpu.set_program_end_addr(end_program);
    }

    /// Instruct the validator to fully prefetch the next instruction.
    /// This is typically not called within CPU execution, but before calling step during CPU test
    /// generation.
    fn set_prefetch(&mut self, state: bool) {
        trace_debug!(self, "Setting prefetch to {}", state);
        self.current_instr.prefetch = state;
    }

    /// Initialize the physical CPU with a provided register state.
    /// Can only be done after a reset or jump
    fn set_regs(&mut self) {
        trace_debug!(self, "Setting register state...");
        self.cpu.reset();

        let mut reg_buf: [u8; 28] = [0; 28];

        let mut adjusted_regs = self.current_instr.regs[0];

        // If we are prefetching the next instruction, we need to adjust IP by the size of the
        // prefetch program.
        if self.current_instr.prefetch {
            if let Some(program) = self.cpu.prefetch_program() {
                let pgm_len = program.ip_adjustment(adjusted_regs.ip);
                adjusted_regs.ip = adjusted_regs.ip.wrapping_sub(pgm_len as u16);
                trace_debug!(
                    self,
                    "Adjusting IP by prefetch program length: {} new_ip: {:04X} new_addr: {:05X}",
                    pgm_len,
                    adjusted_regs.ip,
                    RemoteCpu::calc_linear_address(adjusted_regs.cs, adjusted_regs.ip)
                );
            }

            // On 8088/8086, we need to adjust DI by 4 depending on flag direction.
            if let CpuType::Intel8088 | CpuType::Intel8086 = self.cpu_type {
                // Adjust DI. This depends on the state of the Direction flag.
                if adjusted_regs.flags & CPU_FLAG_DIRECTION == 0 {
                    // Direction forward. Decrement DI.
                    trace_debug!(self, "Adjusting DI for 8088 prefetch, -= 4... ");
                    adjusted_regs.di = adjusted_regs.di.wrapping_sub(4);
                }
                else {
                    // Direction backwards. Increment DI.
                    trace_debug!(self, "Adjusting DI for 8088 prefetch, += 4... ");
                    adjusted_regs.di = adjusted_regs.di.wrapping_add(4);
                }
            }
        }
        else {
            trace!(self, "begin_instruction(): Not prefetching.");
        }

        ArduinoValidator::regs_to_buf(&mut reg_buf, &adjusted_regs);

        self.cpu
            .load(&reg_buf)
            .expect("validate() error: Load registers failed.");
    }

    fn set_opts(
        &mut self,
        ignore_underflow: bool,
        validate_cycles: bool,
        validate_regs: bool,
        validate_flags: bool,
        validate_mem: bool,
    ) {
        self.opt_ignore_underflow = ignore_underflow;
        self.opt_validate_cycles = validate_cycles;
        self.opt_validate_regs = validate_regs;
        self.opt_validate_flags = validate_flags;
        self.opt_validate_mem = validate_mem;
    }

    fn validate_instruction(
        &mut self,
        name: String,
        instr: &[u8],
        flags: u8,
        peek_fetch: u16,
        has_modrm: bool,
        _cycles: i32,
        regs: &VRegisters,
        emu_states: &[CycleState],
    ) -> Result<ValidatorResult, ValidatorError> {
        let ip_addr = make_pointer(self.current_instr.regs[0].cs, self.current_instr.regs[0].ip);

        /*
        if (self.trigger_addr != V_INVALID_POINTER)
            || (self.visit_once && ip_addr >= UPPER_MEMORY &&  self.visited[ip_addr as usize]) {
            return Ok(true);
        }
        */

        if instr.is_empty() {
            trace_error!(self, "Instruction length was 0");
            return Err(ValidatorError::ParameterError);
        }

        self.visited[ip_addr as usize] = true;

        let mut i = 0;

        // Scan through prefix bytes to find opcode
        loop {
            let instr_byte = instr[i];

            if self.cpu.is_prefix(instr_byte) {
                i += 1;
                continue;
            }
            else {
                break;
            }
        }

        self.current_instr.name = name.clone();
        self.current_instr.opcode = instr[i];
        self.current_instr.instr = instr.to_vec();
        self.current_instr.has_modrm = has_modrm;
        self.current_instr.initial_queue = Vec::new();
        self.current_instr.next_fetch = false;
        self.current_instr.regs[1] = *regs;

        if self.current_instr.regs[1].flags == 0 {
            trace_error!(self, "Invalid emulator flags");
            return Err(ValidatorError::ParameterError);
        }

        //self.current_frame.emu_states.clone_from(&emu_states);
        //RemoteCpu::print_regs(&self.current_instr.regs[0]);

        if has_modrm {
            if i > (instr.len().saturating_sub(2)) {
                trace_error!(self, "validate(): modrm specified but instruction length < ");
                trace_error!(
                    self,
                    "instruction: {} opcode: {} instr: {:02X?}",
                    self.current_instr.name,
                    self.current_instr.opcode,
                    self.current_instr.instr
                );
                self.reset_after_validation(None);
                return Err(ValidatorError::ParameterError);
            }
            self.current_instr.modrm = instr[i + 1];
        }
        else {
            self.current_instr.modrm = 0;
        }

        let discard_or_validate = match self.current_instr.discard {
            true => "DISCARD",
            false => "VALIDATE",
        };

        self.cpu.set_instr_string(name.clone());
        self.cpu.set_instr_bytes(instr);

        trace_debug!(
            self,
            "{}: {} {:02X?} @ [{:04X}:{:04X}] Memops: {} Fetches: {} Start: {:05X} End: {:05X} Prefetch: {}",
            discard_or_validate,
            name,
            self.current_instr.instr,
            self.current_instr.regs[0].cs,
            self.current_instr.regs[0].ip,
            self.current_instr.emu_ops.len(),
            self.current_instr.emu_fetches.len(),
            ip_addr,
            self.current_instr.instr_end,
            self.current_instr.prefetch
        );

        trace_debug!(
            self,
            "validate_instruction() registers:\n{}",
            &RemoteCpu::get_reg_str(&self.current_instr.regs[0])
        );

        if self.current_instr.discard {
            return Ok(ValidatorResult::Ok);
        }

        let instr_addr = RemoteCpu::calc_linear_address(self.current_instr.regs[0].cs, self.current_instr.regs[0].ip);

        // Run the instruction on the physical CPU
        let (mut cpu_states, discard) = match self.cpu.step(
            self.mode,
            self.current_instr.prefetch,
            &self.current_instr.instr,
            instr_addr,
            self.do_cycle_trace,
            peek_fetch,
            &self.current_instr.emu_fetches,
            &self.current_instr.emu_ops,
            &mut self.current_instr.cpu_fetches,
            &mut self.current_instr.cpu_ops,
            &mut self.current_instr.initial_queue,
            self.opt_ignore_underflow,
            &mut self.trace_logger,
        ) {
            Ok(stepresult) => stepresult,
            Err(_) => {
                return match self.cpu.get_error() {
                    Some(RemoteCpuError::BusOpUnderflow) => {
                        // Handle the specific error
                        trace_error!(self, "Memory validation failure. CPU bus op underflow.");

                        let states = self.cpu.get_states().clone();
                        self.print_cycle_diff(&states, emu_states);
                        self.trace_logger.flush();
                        self.reset_after_validation(None);
                        Err(ValidatorError::MemOpMismatch)
                    }
                    // You can add more error handlers here
                    // For instance:
                    // MyError::OtherError => { ... }
                    None => {
                        log::error!("Unknown CPU error!");
                        Err(ValidatorError::CpuError)
                    }
                    _ => {
                        self.reset_after_validation(None);
                        Err(ValidatorError::CpuError)
                    } // Propagate other errors
                };
            }
        };

        if cpu_states.is_empty() {
            trace_error!(self, "No CPU states returned from step()");
            return Err(ValidatorError::CpuError);
        }

        if self.current_instr.opcode != 0x9C {
            // We ignore PUSHF results due to undefined flags causing write mismatches

            if !self.opt_validate_mem {
                trace!(self, "Skipping memory validation");
            }
            else if !self.validate_mem_ops(discard, flags) {
                trace_error!(self, "Memory validation failure. EMU:");
                trace_error!(self, "\n{}", &RemoteCpu::get_reg_str(&self.current_instr.regs[1]));
                trace_error!(self, "CPU:");
                trace_error!(self, "\n{}", &RemoteCpu::get_reg_str(regs));

                self.print_cycle_diff(&cpu_states, emu_states);
                self.trace_logger.flush();
                self.reset_after_validation(Some(cpu_states));
                return Err(ValidatorError::MemOpMismatch);
            }
            else {
                trace!(self, "Memops validated!");
            }
        }

        if self.opt_validate_cycles && (flags & VAL_NO_CYCLES == 0) && (!emu_states.is_empty()) {
            // Only validate CPU cycles if any were provided

            self.correct_queue_counts(&mut cpu_states);
            let (result, cycle_num) = self.validate_cycles(flags, &cpu_states, emu_states);

            if !result {
                trace_error!(
                    self,
                    "Cycle state validation failure @ cycle {}/{}",
                    cycle_num,
                    cpu_states.len()
                );
                self.print_cycle_diff(&cpu_states, emu_states);
                trace_error!(self, "EMU AFTER:");
                trace_error!(self, "\n{}", &RemoteCpu::get_reg_str(&self.current_instr.regs[1]));

                trace_error!(self, "CPU AFTER:");
                trace_error!(self, "\n{}", &RemoteCpu::get_reg_str(regs));
                self.trace_logger.flush();
                self.reset_after_validation(Some(cpu_states));
                return Err(ValidatorError::CycleMismatch);
            }
            else {
                self.print_cycle_diff(&cpu_states, emu_states);
            }
        }

        self.reset_after_validation(Some(cpu_states));

        // Did this instruction enter finalize state?
        if self.cpu.in_finalize() {
            trace!(self, " >>> Validator finalizing!");
            Ok(ValidatorResult::OkEnd)
        }
        else {
            trace!(self, " >>> Validator finished validating instruction");
            Ok(ValidatorResult::Ok)
        }
    }

    fn validate_regs(&mut self, regs: &VRegisters) -> Result<(), ValidatorError> {
        let mut store_regs = match self.cpu.store() {
            Ok(regs) => {
                trace!(self, "CPU regs:\n{}", regs);
                regs
            }
            Err(e) => {
                log::error!("validate_regs failed: {}", e);
                match self.cpu.get_last_error() {
                    Ok(error_str) => log::error!("get_last_error(): {}", error_str),
                    Err(e) => log::error!("get_last_error() failed: {}", e),
                };
                self.trace_logger.flush();
                return Err(ValidatorError::CpuError);
            }
        };

        self.cpu.adjust_ip(&mut store_regs);
        self.current_instr.cpu_after_regs = Some(store_regs);

        match self.validate_registers(regs) {
            RegisterValidationResult::Ok => Ok(()),
            RegisterValidationResult::GeneralMismatch => {
                trace_error!(self, "Register validation failure. EMU BEFORE:");
                trace_error!(self, "{}", &RemoteCpu::get_reg_str(&self.current_instr.regs[0]));
                trace_error!(self, "EMU AFTER:");
                trace_error!(self, "{}", &RemoteCpu::get_reg_str(&self.current_instr.regs[1]));
                trace_error!(self, "CPU AFTER:");
                RemoteCpu::print_regs(regs);

                Err(ValidatorError::RegisterMismatch)
            }
            RegisterValidationResult::FlagMismatch => {
                trace_error!(self, "Flag validation failure. EMU BEFORE:");
                trace_error!(self, "{}", &RemoteCpu::get_reg_str(&self.current_instr.regs[0]));
                trace_error!(self, "EMU AFTER:");
                trace_error!(self, "{}", &RemoteCpu::get_reg_str(&self.current_instr.regs[1]));
                trace_error!(self, "CPU AFTER:");
                RemoteCpu::print_regs(regs);

                Err(ValidatorError::FlagsMismatch)
            }
            RegisterValidationResult::BothMismatch => {
                trace_error!(self, "Register and Flag validation failure. EMU BEFORE:");
                trace_error!(self, "{}", &RemoteCpu::get_reg_str(&self.current_instr.regs[0]));
                trace_error!(self, "EMU AFTER:");
                trace_error!(self, "{}", &RemoteCpu::get_reg_str(&self.current_instr.regs[1]));
                trace_error!(self, "CPU AFTER:");
                RemoteCpu::print_regs(regs);

                Err(ValidatorError::BothMismatch)
            }
        }
    }

    fn emu_read_byte(&mut self, addr: u32, data: u8, bus_type: BusType, read_type: ReadType) {
        if self.current_instr.discard {
            return;
        }

        // BHE will be enabled if the address is odd.
        let bhe = (addr & 0x01) != 0;
        let mut data = data as u16;
        // Shift odd byte to high side of bus.
        if bhe {
            data <<= 8;
        }

        match (bus_type, read_type) {
            (BusType::Mem, ReadType::Code) => {
                self.current_instr.emu_fetches.push(BusOp {
                    op_type: BusOpType::CodeRead,
                    addr,
                    bhe,
                    data,
                    flags: MOF_EMULATOR,
                });
                trace!(
                    self,
                    "EMU fetch: [{:05X}] -> 0x{:04X} ({})",
                    addr,
                    data,
                    self.current_instr.emu_ops.len()
                );
            }
            (BusType::Mem, ReadType::Data) => {
                self.current_instr.emu_ops.push(BusOp {
                    op_type: BusOpType::MemRead,
                    addr,
                    bhe,
                    data,
                    flags: MOF_EMULATOR,
                });
                trace!(
                    self,
                    "EMU read: [{:05X}] -> 0x{:04X} ({})",
                    addr,
                    data,
                    self.current_instr.emu_ops.len()
                );
            }
            (BusType::Io, _) => {
                self.current_instr.emu_ops.push(BusOp {
                    op_type: BusOpType::IoRead,
                    addr,
                    bhe,
                    data,
                    flags: MOF_EMULATOR,
                });
                trace!(
                    self,
                    "EMU IN: [{:05X}] -> 0x{:04X} ({})",
                    addr,
                    data,
                    self.current_instr.emu_ops.len()
                );
            }
        }
    }

    fn emu_read_word(&mut self, addr: u32, data: u16, bus_type: BusType, read_type: ReadType) {
        if self.current_instr.discard {
            return;
        }

        // BHE will be enabled for all word writes.
        let bhe = true;

        match (bus_type, read_type) {
            (BusType::Mem, ReadType::Code) => {
                self.current_instr.emu_fetches.push(BusOp {
                    op_type: BusOpType::CodeRead,
                    addr,
                    bhe,
                    data,
                    flags: MOF_EMULATOR,
                });
                trace!(
                    self,
                    "EMU fetch: [{:05X}] -> 0x{:04X} ({})",
                    addr,
                    data,
                    self.current_instr.emu_ops.len()
                );
            }
            (BusType::Mem, ReadType::Data) => {
                self.current_instr.emu_ops.push(BusOp {
                    op_type: BusOpType::MemRead,
                    addr,
                    bhe,
                    data,
                    flags: MOF_EMULATOR,
                });
                trace!(
                    self,
                    "EMU read: [{:05X}] -> 0x{:04X} ({})",
                    addr,
                    data,
                    self.current_instr.emu_ops.len()
                );
            }
            (BusType::Io, _) => {
                self.current_instr.emu_ops.push(BusOp {
                    op_type: BusOpType::IoRead,
                    addr,
                    bhe,
                    data,
                    flags: MOF_EMULATOR,
                });
                trace!(
                    self,
                    "EMU IN: [{:05X}] -> 0x{:04X} ({})",
                    addr,
                    data,
                    self.current_instr.emu_ops.len()
                );
            }
        }
    }

    fn emu_write_byte(&mut self, addr: u32, data: u8, bus_type: BusType) {
        self.visited[(addr & 0xFFFFF) as usize] = false;

        if self.current_instr.discard {
            return;
        }

        // BHE will be enabled if the address is odd.
        let bhe = (addr & 0x01) != 0;
        let mut data = data as u16;
        if bhe {
            // Shift odd byte to high side of bus.
            data <<= 8;
        }

        match bus_type {
            BusType::Mem => {
                self.current_instr.emu_ops.push(BusOp {
                    op_type: BusOpType::MemWrite,
                    addr,
                    bhe,
                    data,
                    flags: MOF_EMULATOR,
                });

                trace!(self, "EMU write (Byte): [{:05X}] <- 0x{:02X}", addr, data);
            }
            BusType::Io => {
                self.current_instr.emu_ops.push(BusOp {
                    op_type: BusOpType::IoWrite,
                    addr,
                    bhe,
                    data,
                    flags: MOF_EMULATOR,
                });

                trace!(self, "EMU OUT (Byte): [{:05X}] <- 0x{:02X}", addr, data);
            }
        }
    }

    fn emu_write_word(&mut self, addr: u32, data: u16, bus_type: BusType) {
        self.visited[(addr & 0xFFFFF) as usize] = false;

        if self.current_instr.discard {
            return;
        }

        // BHE will be enabled for all word writes
        let bhe = true;

        match bus_type {
            BusType::Mem => {
                self.current_instr.emu_ops.push(BusOp {
                    op_type: BusOpType::MemWrite,
                    addr,
                    bhe,
                    data,
                    flags: MOF_EMULATOR,
                });
                trace!(self, "EMU write (Word): [{:05X}] <- 0x{:04X}", addr, data);
            }
            BusType::Io => {
                self.current_instr.emu_ops.push(BusOp {
                    op_type: BusOpType::IoWrite,
                    addr,
                    bhe,
                    data,
                    flags: MOF_EMULATOR,
                });
                trace!(self, "EMU OUT (Word): [{:05X}] <- 0x{:04X}", addr, data);
            }
        }
    }

    fn discard_op(&mut self) {
        self.current_instr.discard = true;
    }

    fn flush(&mut self) {
        self.trace_logger.flush();
    }

    /// Get a reference to the vector of CycleStates, presumably after an instruction has
    /// been successfully validated
    fn cycle_states(&self) -> &Vec<CycleState> {
        &self.last_cpu_states
    }

    /// Return the name of the current test - should be the disassembled instruction form
    fn name(&self) -> String {
        self.current_instr.name.clone()
    }

    fn instr_bytes(&self) -> Vec<u8> {
        self.current_instr.instr.clone()
    }

    fn initial_regs(&self) -> VRegisters {
        self.current_instr.regs[0]
    }

    fn initial_queue(&self) -> Vec<u8> {
        self.current_instr.initial_queue.clone()
    }

    fn final_emu_regs(&self) -> VRegisters {
        self.current_instr.regs[1]
    }

    fn final_cpu_regs(&self) -> Option<VRegisters> {
        self.current_instr.cpu_after_regs
    }

    /// Return all operations performed by the emulator during last instruction validation.
    fn emu_ops(&self) -> Vec<BusOp> {
        self.last_emu_ops.clone()
    }

    /// Return all operations performed by the cpu during last instruction validation.
    fn cpu_ops(&self) -> Vec<BusOp> {
        self.last_cpu_ops.clone()
    }

    /// Return the initial reads performed by this instruction, stopping when a write is
    /// encountered.
    fn cpu_reads(&self) -> Vec<BusOp> {
        // Copy ops vec up until the first write

        //log::debug!("filtering {} bus ops from CPU", self.last_cpu_ops.len());

        let mut read_vec: Vec<_> = self
            .last_cpu_ops
            .iter()
            .take_while(|&&op| matches!(op.op_type, BusOpType::CodeRead | BusOpType::MemRead | BusOpType::IoRead))
            .cloned()
            .collect();

        // Filter out fetches
        read_vec.retain(|&op| !matches!(op.op_type, BusOpType::CodeRead));

        read_vec
    }

    fn cpu_queue(&self) -> Vec<u8> {
        self.last_cpu_queue.clone()
    }

    fn cpu_type(&self) -> CpuType {
        self.cpu_type
    }
}

/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2024 Daniel Balsom

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

    ---------------------------------------------------------------------------

    cpu_common::mod.rs

    Implements common functionality shared by different CPU types.

*/

#![allow(dead_code)]

pub mod addressing;
pub mod alu;
pub mod builder;
pub mod error;
pub mod instruction;
pub mod mnemonic;
pub mod operands;

use enum_dispatch::enum_dispatch;
use serde::Deserialize;
use std::str::FromStr;

pub use addressing::{AddressingMode, CpuAddress, Displacement};
pub use error::CpuError;
pub use instruction::Instruction;
pub use mnemonic::Mnemonic;
pub use operands::OperandType;

use crate::{
    breakpoints::{BreakPointType, StopWatchData},
    bus::BusInterface,
    bytequeue::ByteQueue,
    cpu_808x::Intel808x,
    cpu_validator::{CycleState, VRegisters},
    cpu_vx0::NecVx0,
    syntax_token::{SyntaxToken, SyntaxTokenize},
};

#[cfg(feature = "cpu_validator")]
use crate::cpu_validator::CpuValidator;

// Instruction prefixes
pub const OPCODE_PREFIX_0F: u32 = 0b_1000_0000_0000_0000;
pub const OPCODE_PREFIX_ES_OVERRIDE: u32 = 0b_0000_0000_0100;
pub const OPCODE_PREFIX_CS_OVERRIDE: u32 = 0b_0000_0000_1000;
pub const OPCODE_PREFIX_SS_OVERRIDE: u32 = 0b_0000_0001_0000;
pub const OPCODE_PREFIX_DS_OVERRIDE: u32 = 0b_0000_0010_0000;
pub const OPCODE_SEG_OVERRIDE_MASK: u32 = 0b_0000_0011_1100;
pub const OPCODE_PREFIX_LOCK: u32 = 0b_0000_1000_0000;
pub const OPCODE_PREFIX_REP1: u32 = 0b_0001_0000_0000;
pub const OPCODE_PREFIX_REP2: u32 = 0b_0010_0000_0000;
pub const OPCODE_PREFIX_REP3: u32 = 0b_0100_0000_0000;
pub const OPCODE_PREFIX_REP4: u32 = 0b_1000_0000_0000;
pub const OPCODE_PREFIX_REPMASK: u32 = 0b1111_0000_0000;
// Some CPUs can restore up to 3 prefixes when returning to an interrupted string operation.
// The first two bits of the prefixes field stores the number of prefixes to restore from 0-3.
pub const OPCODE_PREFIX_CT_MASK: u32 = 0b0000_0000_0011;

#[derive(Debug, Default, PartialEq)]
pub enum ExecutionResult {
    #[default]
    Okay,
    OkayJump,
    OkayRep,
    //UnsupportedOpcode(u8),        // All opcodes implemented.
    ExecutionError(String),
    ExceptionError(CpuException),
    Halt,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum CpuException {
    NoException,
    DivideError,
    BoundsException,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Register8 {
    AL,
    CL,
    DL,
    BL,
    AH,
    CH,
    DH,
    BH,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Register16 {
    AX,
    CX,
    DX,
    BX,
    SP,
    BP,
    SI,
    DI,
    ES,
    CS,
    SS,
    DS,
    PC,
    InvalidRegister,
}

#[derive(Copy, Clone, Default, Debug)]
pub enum Segment {
    None,
    ES,
    #[default]
    CS,
    SS,
    DS,
}

#[derive(Default, Debug, Clone)]
pub struct CpuStringState {
    pub ah: String,
    pub al: String,
    pub ax: String,
    pub bh: String,
    pub bl: String,
    pub bx: String,
    pub ch: String,
    pub cl: String,
    pub cx: String,
    pub dh: String,
    pub dl: String,
    pub dx: String,
    pub sp: String,
    pub bp: String,
    pub si: String,
    pub di: String,
    pub cs: String,
    pub ds: String,
    pub ss: String,
    pub es: String,
    pub pc: String,
    pub ip: String,
    pub flags: String,
    //odiszapc
    pub c_fl: String,
    pub p_fl: String,
    pub a_fl: String,
    pub z_fl: String,
    pub s_fl: String,
    pub t_fl: String,
    pub i_fl: String,
    pub d_fl: String,
    pub o_fl: String,
    pub piq: String,
    pub instruction_count: String,
    pub cycle_count: String,
}

#[derive(Copy, Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub enum CpuType {
    Intel8088,
    Intel8086,
    NecV20,
    NecV30,
}

impl FromStr for CpuType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        match s.to_lowercase().as_str() {
            "intel8088" => Ok(CpuType::Intel8088),
            "intel8086" => Ok(CpuType::Intel8086),
            "necv20" => Ok(CpuType::NecV20),
            "necv30" => Ok(CpuType::NecV30),
            _ => Err("Bad value for cputype".to_string()),
        }
    }
}

impl CpuType {
    pub fn decode(&self, bytes: &mut impl ByteQueue, peek: bool) -> Result<Instruction, Box<dyn std::error::Error>> {
        match self {
            CpuType::Intel8088 | CpuType::Intel8086 => Intel808x::decode(bytes, peek),
            CpuType::NecV20 | CpuType::NecV30 => NecVx0::decode(bytes, peek),
        }
    }
    pub fn tokenize_instruction(&self, instruction: &Instruction) -> Vec<SyntaxToken> {
        match self {
            CpuType::Intel8088 | CpuType::Intel8086 => instruction.tokenize(),
            CpuType::NecV20 | CpuType::NecV30 => instruction.tokenize(),
        }
    }
}

#[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq)]
pub enum CpuSubType {
    #[default]
    None,
    Intel8088,
    Intel8086,
    Harris80C88,
}

pub enum CycleTraceMode {
    Text,
    Csv,
    Sigrok,
}

#[derive(Copy, Clone, Debug, Deserialize, PartialEq)]
pub enum TraceMode {
    None,
    CycleText,
    CycleCsv,
    CycleSigrok,
    Instruction,
}

impl FromStr for TraceMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        match s.to_lowercase().as_str() {
            "none" => Ok(TraceMode::None),
            "cycletext" => Ok(TraceMode::CycleText),
            "cyclecsv" => Ok(TraceMode::CycleCsv),
            "cyclesigrok" => Ok(TraceMode::CycleSigrok),
            "instruction" => Ok(TraceMode::Instruction),
            _ => Err("Bad value for tracemode".to_string()),
        }
    }
}
impl Default for TraceMode {
    fn default() -> Self {
        TraceMode::None
    }
}

impl Default for CpuType {
    fn default() -> Self {
        CpuType::Intel8088
    }
}

#[derive(Debug)]
pub enum CpuOption {
    InstructionHistory(bool),
    ScheduleInterrupt(bool, u32, u32, bool),
    ScheduleDramRefresh(bool, u32, u32, bool),
    DramRefreshAdjust(u32),
    HaltResumeDelay(u32),
    OffRailsDetection(bool),
    EnableWaitStates(bool),
    TraceLoggingEnabled(bool),
    EnableServiceInterrupt(bool),
}

#[derive(Debug)]
pub enum StepResult {
    Normal,
    // If a call occurred, we return the address of the next instruction after the call
    // so that we can step over the call in the debugger.
    Call(CpuAddress),
    // If we are in a REP prefixed string operation, we return the address of the next instruction
    // so that we can step over the string operation.
    Rep(CpuAddress),
    BreakpointHit,
    StepOverHit,
    ProgramEnd,
}

// Internal Emulator interrupt service events. These are returned to the machine when
// the internal service interrupt is called to request an emulator action that cannot
// be handled by the CPU alone.
#[derive(Copy, Clone, Debug)]
pub enum ServiceEvent {
    TriggerPITLogging,
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum QueueOp {
    #[default]
    Idle,
    First,
    Flush,
    Subsequent,
}

pub fn calc_linear_address(segment: u16, offset: u16) -> u32 {
    (((segment as u32) << 4) + offset as u32) & 0xFFFFFu32
}

#[enum_dispatch]
pub enum CpuDispatch {
    Intel808x,
    NecVx0,
}

#[enum_dispatch(CpuDispatch)]
pub trait Cpu {
    // General CPU control
    fn reset(&mut self);
    fn set_reset_vector(&mut self, address: CpuAddress);
    fn set_reset_queue_contents(&mut self, contents: Vec<u8>);
    fn set_end_address(&mut self, address: CpuAddress);
    fn set_nmi(&mut self, state: bool);
    fn set_intr(&mut self, state: bool);
    fn step(&mut self, skip_breakpoint: bool) -> Result<(StepResult, u32), CpuError>;
    fn step_finish(&mut self) -> Result<StepResult, CpuError>;

    fn in_rep(&self) -> bool;
    fn get_type(&self) -> CpuType;

    fn get_ip(&mut self) -> u16;
    fn get_register16(&self, reg: Register16) -> u16;
    fn set_register16(&mut self, reg: Register16, value: u16);
    fn get_register8(&self, reg: Register8) -> u8;
    fn set_register8(&mut self, reg: Register8, value: u8);
    fn get_flags(&self) -> u16;
    fn set_flags(&mut self, flags: u16);
    fn get_cycle_ct(&self) -> (u64, u64);
    fn get_instruction_ct(&self) -> u64;
    fn flat_ip(&self) -> u32;
    fn flat_ip_disassembly(&self) -> u32;
    fn flat_sp(&self) -> u32;
    fn dump_instruction_history_string(&self) -> String;
    fn dump_instruction_history_tokens(&self) -> Vec<Vec<SyntaxToken>>;
    fn dump_call_stack(&self) -> String;
    fn get_service_event(&mut self) -> Option<ServiceEvent>;
    fn get_cycle_states(&self) -> &Vec<CycleState>;
    fn get_cycle_trace(&self) -> &Vec<String>;
    fn get_cycle_trace_tokens(&self) -> &Vec<Vec<SyntaxToken>>;

    fn get_string_state(&self) -> CpuStringState;

    // Eval
    fn eval_address(&self, expr: &str) -> Option<CpuAddress>;

    // Breakpoints
    fn clear_breakpoint_flag(&mut self);
    fn set_breakpoints(&mut self, bp_list: Vec<BreakPointType>);
    fn get_step_over_breakpoint(&self) -> Option<CpuAddress>;
    fn set_step_over_breakpoint(&mut self, address: CpuAddress);
    fn get_sw_data(&self) -> Vec<StopWatchData>;
    fn set_stopwatch(&mut self, sw_idx: usize, start: u32, stop: u32);

    // CPU options
    fn set_option(&mut self, opt: CpuOption);
    fn get_option(&self, opt: CpuOption) -> bool;

    // Bus methods
    fn bus(&self) -> &BusInterface;
    fn bus_mut(&mut self) -> &mut BusInterface;

    // Logging methods
    fn cycle_table_header(&self) -> Vec<String>;
    fn emit_header(&mut self);
    fn trace_flush(&mut self);

    // Validation methods
    #[cfg(feature = "cpu_validator")]
    fn get_vregisters(&self) -> VRegisters;
    #[cfg(feature = "cpu_validator")]
    fn get_validator(&self) -> &Option<Box<dyn CpuValidator>>;
    #[cfg(feature = "cpu_validator")]
    fn get_validator_mut(&mut self) -> &mut Option<Box<dyn CpuValidator>>;
    fn randomize_seed(&mut self, seed: u64);
    fn randomize_mem(&mut self);
    fn randomize_regs(&mut self);
    fn random_grp_instruction(&mut self, opcode: u8, extension_list: &[u8]);
    fn random_inst_from_opcodes(&mut self, opcode_list: &[u8], prefix: Option<u8>);
}

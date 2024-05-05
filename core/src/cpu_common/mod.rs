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

pub mod alu;
pub mod builder;

use crate::{
    breakpoints::{BreakPointType, StopWatchData},
    bus::{BusInterface, ClockFactor},
    bytequeue::ByteQueue,
    cpu_808x::{
        mnemonic::Mnemonic,
        CpuAddress,
        CpuError,
        Intel808x,
        OperandSize,
        OperandType,
        Segment,
        ServiceEvent,
        StepResult,
    },
    cpu_validator::{CycleState, VRegisters},
    syntax_token::SyntaxToken,
};
use enum_dispatch::enum_dispatch;
use serde::Deserialize;
use std::str::FromStr;

#[derive(Copy, Clone, PartialEq)]
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

#[derive(Copy, Clone, Debug, Deserialize, PartialEq)]
pub enum CpuType {
    Intel808x,
}

impl CpuType {
    pub fn decode(&self, bytes: &mut impl ByteQueue, peek: bool) -> Result<Instruction, Box<dyn std::error::Error>> {
        match self {
            CpuType::Intel808x => Intel808x::decode(bytes, peek),
        }
    }
    pub fn tokenize_instruction(&self, instruction: &Instruction) -> Vec<SyntaxToken> {
        match self {
            CpuType::Intel808x => Intel808x::tokenize_instruction(instruction),
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
        CpuType::Intel808x
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

pub fn calc_linear_address(segment: u16, offset: u16) -> u32 {
    (((segment as u32) << 4) + offset as u32) & 0xFFFFFu32
}

#[derive(Clone)]
pub struct Instruction {
    pub decode_idx: usize,
    pub opcode: u8,
    pub prefixes: u32,
    pub address: u32,
    pub size: u32,
    pub mnemonic: Mnemonic,
    pub segment_override: Option<Segment>,
    pub operand1_type: OperandType,
    pub operand1_size: OperandSize,
    pub operand2_type: OperandType,
    pub operand2_size: OperandSize,
}

impl Default for Instruction {
    fn default() -> Self {
        Self {
            decode_idx: 0,
            opcode: 0,
            prefixes: 0,
            address: 0,
            size: 1,
            mnemonic: Mnemonic::NOP,
            segment_override: None,
            operand1_type: OperandType::NoOperand,
            operand1_size: OperandSize::NoOperand,
            operand2_type: OperandType::NoOperand,
            operand2_size: OperandSize::NoOperand,
        }
    }
}

#[enum_dispatch]
pub enum CpuDispatch {
    Intel808x,
}

#[enum_dispatch(CpuDispatch)]
pub trait Cpu {
    // General CPU control
    fn reset(&mut self);
    fn set_reset_vector(&mut self, address: CpuAddress);
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
    #[cfg(feature = "cpu_validator")]
    fn get_vregisters(&self) -> VRegisters;
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
}

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

    cpu_808x::mod.rs

    Implements the 8088 (And eventually 8086) CPU.

*/

#![allow(clippy::unusual_byte_groupings)]

pub use crate::cpu_common::Cpu;
use crate::cpu_common::{
    instruction::Instruction,
    CpuAddress,
    CpuStringState,
    CpuSubType,
    ExecutionResult,
    LogicAnalyzer,
    Mnemonic,
    QueueOp,
    Segment,
    ServiceEvent,
};
use core::fmt::Display;
use lazy_static::lazy_static;
use regex::Regex;
use std::{collections::VecDeque, fmt, path::Path};

// Pull in all CPU module components
mod addressing;
mod alu;
mod bcd;
mod bitwise;
mod biu;
mod cpu;
mod cycle;
mod decode;
mod display;
mod execute;
mod fuzzer;
mod gdr;
mod instruction;
mod interrupt;
mod jump;
mod logging;
mod microcode;
pub mod mnemonic;
mod modrm;
mod muldiv;
mod queue;
mod stack;
mod step;
mod string;

use crate::{
    breakpoints::{BreakPointType, CycleStopWatch, StopWatchData},
    bus::{BusInterface, MEM_BPA_BIT, MEM_BPE_BIT, MEM_RET_BIT, MEM_SW_BIT},
    bytequeue::*,
    cpu_808x::{microcode::*, queue::InstructionQueue},
    cpu_common::{CpuType, TraceMode},
    cycles_mc,
    syntax_token::*,
    tracelogger::TraceLogger,
};

// Make ReadWriteFlag available to benchmarks
pub use crate::cpu_808x::biu::ReadWriteFlag;

#[cfg(feature = "cpu_validator")]
use crate::cpu_validator::ValidatorType;

#[cfg(feature = "cpu_validator")]
use crate::cpu_validator::{
    AccessType,
    BusCycle,
    BusState,
    CpuValidator,
    CycleState,
    VRegisters,
    ValidatorMode,
    ValidatorResult,
    VAL_ALLOW_ONE,
    VAL_NO_CYCLES,
    VAL_NO_FLAGS,
    VAL_NO_WRITES,
};

#[cfg(feature = "arduino_validator")]
use crate::arduino8088_validator::ArduinoValidator;

macro_rules! trace_print {
    ($self:ident, $($t:tt)*) => {{
        if $self.trace_enabled {
            if let TraceMode::CycleText = $self.trace_mode  {
                $self.trace_print(&format!($($t)*));
            }
        }
    }};
}

#[macro_export]
macro_rules! gdr {
    ($inst:expr) => {
        &DECODE[$inst.decode_idx as usize].gdr
    };
}

use crate::{
    bus::ClockFactor,
    cpu_common::{operands::OperandSize, services::CPUDebugServices, Register16, Register8},
};
use trace_print;

const QUEUE_MAX: usize = 6;
const FETCH_DELAY: u8 = 2;

const CPU_HISTORY_LEN: usize = 32;
const CPU_CALL_STACK_LEN: usize = 48;

const INTERRUPT_VEC_LEN: usize = 4;
const INTERRUPT_BREAKPOINT: u8 = 1;

const IO_READ_BREAKPOINT: u8 = 0b0000_0001;
const IO_WRITE_BREAKPOINT: u8 = 0b0000_0010;

pub const CPU_FLAG_CARRY: u16 = 0b0000_0000_0000_0001;
pub const CPU_FLAG_RESERVED1: u16 = 0b0000_0000_0000_0010;
pub const CPU_FLAG_PARITY: u16 = 0b0000_0000_0000_0100;
pub const CPU_FLAG_RESERVED3: u16 = 0b0000_0000_0000_1000;
pub const CPU_FLAG_AUX_CARRY: u16 = 0b0000_0000_0001_0000;
pub const CPU_FLAG_RESERVED5: u16 = 0b0000_0000_0010_0000;
pub const CPU_FLAG_ZERO: u16 = 0b0000_0000_0100_0000;
pub const CPU_FLAG_SIGN: u16 = 0b0000_0000_1000_0000;
pub const CPU_FLAG_TRAP: u16 = 0b0000_0001_0000_0000;
pub const CPU_FLAG_INT_ENABLE: u16 = 0b0000_0010_0000_0000;
pub const CPU_FLAG_DIRECTION: u16 = 0b0000_0100_0000_0000;
pub const CPU_FLAG_OVERFLOW: u16 = 0b0000_1000_0000_0000;

/*
const CPU_FLAG_RESERVED12: u16 = 0b0001_0000_0000_0000;
const CPU_FLAG_RESERVED13: u16 = 0b0010_0000_0000_0000;
const CPU_FLAG_RESERVED14: u16 = 0b0100_0000_0000_0000;
const CPU_FLAG_RESERVED15: u16 = 0b1000_0000_0000_0000;
*/

const CPU_FLAGS_RESERVED_ON: u16 = 0b1111_0000_0000_0010;
const CPU_FLAGS_RESERVED_OFF: u16 = !(CPU_FLAG_RESERVED3 | CPU_FLAG_RESERVED5);

const FLAGS_POP_MASK: u16 = 0b0000_1111_1101_0101;

const REGISTER_HI_MASK: u16 = 0b0000_0000_1111_1111;
const REGISTER_LO_MASK: u16 = 0b1111_1111_0000_0000;

pub const MAX_INSTRUCTION_SIZE: usize = 15;

const OPCODE_REGISTER_SELECT_MASK: u8 = 0b0000_0111;

// The parity flag is calculated from the lower 8 bits of an alu operation regardless
// of the operand width.  It is trivial to precalculate an 8-bit parity table.
pub const PARITY_TABLE: [bool; 256] = {
    let mut table = [false; 256];
    let mut index = 0;
    loop {
        table[index] = index.count_ones() % 2 == 0;
        index += 1;

        if index == 256 {
            break;
        }
    }
    table
};

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct GeneralRegisterBytes {
    pub l: u8,
    pub h: u8,
}

#[repr(C)]
pub union GeneralRegister {
    b: GeneralRegisterBytes,
    w: u16,
}
impl Default for GeneralRegister {
    fn default() -> Self {
        GeneralRegister { w: 0 }
    }
}

impl GeneralRegister {
    // Safety: It is safe to access fields of a union comprised of unsigned integer types.
    #[inline(always)]
    pub fn x(&self) -> u16 {
        unsafe { self.w }
    }
    #[inline(always)]
    pub fn set_x(&mut self, value: u16) {
        self.w = value;
    }
    #[inline(always)]
    pub fn incr_x(&mut self) {
        self.w = unsafe { self.w.wrapping_add(1) };
    }
    #[inline(always)]
    pub fn decr_x(&mut self) {
        self.w = unsafe { self.w.wrapping_sub(1) };
    }
    #[inline(always)]
    pub fn h(&self) -> u8 {
        unsafe { self.b.h }
    }
    #[inline(always)]
    pub fn set_h(&mut self, value: u8) {
        self.b.h = value;
    }
    #[inline(always)]
    pub fn incr_h(&mut self) {
        self.b.h = unsafe { self.b.h.wrapping_add(1) };
    }
    #[inline(always)]
    pub fn decr_h(&mut self) {
        self.b.h = unsafe { self.b.h.wrapping_sub(1) };
    }
    #[inline(always)]
    pub fn l(&self) -> u8 {
        unsafe { self.b.l }
    }
    #[inline(always)]
    pub fn set_l(&mut self, value: u8) {
        self.b.l = value;
    }
    #[inline(always)]
    pub fn incr_l(&mut self) {
        self.b.l = unsafe { self.b.l.wrapping_add(1) };
    }
    #[inline(always)]
    pub fn decr_l(&mut self) {
        self.b.l = unsafe { self.b.l.wrapping_sub(1) };
    }
}

pub const REGISTER16_LUT: [Register16; 8] = [
    Register16::AX,
    Register16::CX,
    Register16::DX,
    Register16::BX,
    Register16::SP,
    Register16::BP,
    Register16::SI,
    Register16::DI,
];

pub const SEGMENT_REGISTER16_LUT: [Register16; 4] = [Register16::ES, Register16::CS, Register16::SS, Register16::DS];

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum CpuState {
    Normal,
    BreakpointHit,
}
impl Default for CpuState {
    fn default() -> Self {
        CpuState::Normal
    }
}

#[derive(Copy, Clone, Debug)]
pub enum CallStackEntry {
    Call {
        ret_cs:  u16,
        ret_ip:  u16,
        call_ip: u16,
    },
    CallF {
        ret_cs:  u16,
        ret_ip:  u16,
        call_cs: u16,
        call_ip: u16,
    },
    Interrupt {
        ret_cs: u16,
        ret_ip: u16,
        call_cs: u16,
        call_ip: u16,
        itype: InterruptType,
        number: u8,
        ah: u8,
    },
}

/// Representation of a flag in the eFlags CPU register
pub enum Flag {
    Carry,
    Parity,
    AuxCarry,
    Zero,
    Sign,
    Trap,
    Interrupt,
    Direction,
    Overflow,
}

/*
pub enum Register {
    AH,
    AL,
    AX,
    BH,
    BL,
    BX,
    CH,
    CL,
    CX,
    DH,
    DL,
    DX,
    SP,
    BP,
    SI,
    DI,
    CS,
    DS,
    SS,
    ES,
    IP,
}*/

#[derive(Copy, Clone, Default, Debug)]
pub enum DmaState {
    #[default]
    Idle,
    Dreq,
    Hrq,
    HoldA,
    Operating(u8),
    End,
    //DmaWait(u8)
}

#[derive(Default, Debug)]
pub enum RepType {
    #[default]
    NoRep,
    Rep,
    Repne,
    Repe,
    MulDiv,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum InterruptType {
    NMI,
    Exception,
    Software,
    Hardware,
}

#[derive(Copy, Clone, Default, Debug, PartialEq)]
pub enum BusPendingType {
    #[default]
    None,
    EuEarly,
    EuLate,
}

#[derive(Copy, Clone, Default, Debug, PartialEq)]
pub enum FetchState {
    #[default]
    Normal,
    PausedFull,
    Delayed(u8),
    Suspended,
    Halted,
}

pub enum HistoryEntry {
    InstructionEntry {
        cs: u16,
        ip: u16,
        cycles: u16,
        interrupt: bool,
        jump: bool,
        i: Instruction,
    },
    InterruptEntry {
        cs: u16,
        ip: u16,
        cycles: u16,
        iv: u8,
    },
    NmiEntry {
        cs: u16,
        ip: u16,
    },
    TrapEntry {
        cs: u16,
        ip: u16,
    },
}

#[derive(Copy, Clone)]
pub struct InterruptDescriptor {
    itype: InterruptType,
    number: u8,
    ah: u8,
}

impl Default for InterruptDescriptor {
    fn default() -> Self {
        InterruptDescriptor {
            itype: InterruptType::Hardware,
            number: 0,
            ah: 0,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum TransferSize {
    Byte,
    Word,
}

impl Default for TransferSize {
    fn default() -> TransferSize {
        TransferSize::Byte
    }
}

#[derive(Default)]
pub struct I8288 {
    // Command bus
    mrdc:  bool,
    amwc:  bool,
    mwtc:  bool,
    iorc:  bool,
    aiowc: bool,
    iowc:  bool,
    inta:  bool,
    // Control output
    _dtr:  bool,
    ale:   bool,
    _pden: bool,
    _den:  bool,
}

#[derive(Default)]
pub struct Intel808x {
    cpu_type: CpuType,
    cpu_subtype: CpuSubType,
    state: CpuState,

    a: GeneralRegister,
    b: GeneralRegister,
    c: GeneralRegister,
    d: GeneralRegister,
    sp: u16,
    bp: u16,
    si: u16,
    di: u16,
    cs: u16,
    ds: u16,
    ss: u16,
    es: u16,
    //ip:    u16,
    flags: u16,

    address_bus: u32,
    address_latch: u32,
    data_bus: u16,
    last_ea: u16,      // Last calculated effective address. Used by 0xFE instructions
    bus: BusInterface, // CPU owns Bus
    i8288: I8288,      // Intel 8288 Bus Controller
    pc: u16,           // Program counter points to the next instruction to be fetched
    mc_pc: u16,        // Microcode program counter.
    nx: bool,
    rni: bool,
    ea_opr: u16, // Operand loaded by EALOAD. Masked to 8 bits as appropriate.

    intr: bool,         // State of INTR line
    intr_pending: bool, // INTR line active and not processed
    in_int: bool,
    int_count: u64,
    iret_count: u64,
    interrupt_inhibit: bool,

    // Operand and result state
    /*
    op1_8: u8,
    op1_16: u16,
    op2_8: u8,
    op2_16: u16,
    result_8: u8,
    result_16: u16,
    */
    // BIU stuff
    ready: bool, // READY line from 8284
    queue: InstructionQueue,
    fetch_size: TransferSize,
    fetch_state: FetchState,
    bus_pending: BusPendingType, // Has the EU requested a bus operation?
    queue_op: QueueOp,
    last_queue_op: QueueOp,
    queue_byte: u8,
    last_queue_byte: u8,
    last_queue_len: usize,
    t_cycle: TCycle,             // The current cycle of the main bus cycle (T1-T4)
    ta_cycle: TaCycle,           // The current cycle of the bus address cycle (Tr-T0)
    bus_status: BusStatus,       // The current bus status type, valid on T1-T2.
    bus_status_latch: BusStatus, // The current bus status type, valid on T1-T4.
    pl_status: BusStatus,        // The upcoming (pipelined) bus status type.
    pl_slot: bool,
    bus_segment: Segment,
    transfer_size: TransferSize, // Width of current bus transfer
    operand_size: OperandSize,   // Width of the operand being transferred
    transfer_n: u32,             // Current transfer number (Either 1 or 2, for byte or word operand, respectively)
    final_transfer: bool, // Flag that determines if the current bus transfer is the final transfer for this bus request
    bus_wait_states: u32,
    io_wait_states: u32,
    wait_states: u32,
    lock: bool, // LOCK pin. Asserted during 2nd INTA bus cycle.

    // Halt-related stuff
    halted: bool,
    reported_halt: bool, // Only error on halt once. The caller can determine if it wants to continue.
    halt_not_hold: bool, // Internal halt signal
    wake_timer: u32,

    is_running: bool,
    is_error:   bool,

    // Rep prefix handling
    in_rep: bool,
    rep_init: bool,
    rep_mnemonic: Mnemonic,
    rep_type: RepType,

    cycle_num: u64,
    halt_cycles: u64,
    t_stamp: f64,
    t_step: f64,
    t_step_h: f64,
    instr_cycle: u32,
    device_cycles: u32,
    int_elapsed: u32,
    instr_elapsed: u32,
    instruction_count: u64,
    i: Instruction, // Currently executing instruction
    instruction_ip: u16,
    instruction_reentrant: bool, // Is an instruction reentrant? (REP stringop, HLT)
    last_cs: u16,
    last_ip: u16,
    last_intr: bool,
    jumped: bool,
    instruction_address: u32,
    instruction_history_on: bool,
    instruction_history: VecDeque<HistoryEntry>,

    services:    CPUDebugServices,
    call_stack:  VecDeque<CallStackEntry>,
    exec_result: ExecutionResult,

    // Breakpoints
    breakpoints: Vec<BreakPointType>,
    stopwatches: Vec<Option<CycleStopWatch>>,
    stopwatch_running: bool,
    step_over_target: Option<CpuAddress>,
    step_over_breakpoint: Option<u32>,

    reset_vector: CpuAddress,
    reset_queue:  Option<Vec<u8>>,

    enable_service_interrupt: bool,
    trace_enabled: bool,
    trace_mode: TraceMode,
    trace_logger: TraceLogger,
    trace_comment: Vec<&'static str>,
    trace_instr: u16,
    trace_str_vec: Vec<String>,
    trace_token_vec: Vec<Vec<SyntaxToken>>,

    enable_wait_states: bool,
    off_rails_detection: bool,
    opcode0_counter: u32,

    rng: Option<rand::rngs::StdRng>,

    #[cfg(feature = "cpu_validator")]
    validator: Option<Box<dyn CpuValidator>>,
    #[cfg(feature = "cpu_validator")]
    vregs: VRegisters,
    #[cfg(feature = "cpu_validator")]
    cycle_states: Vec<CycleState>,
    #[cfg(feature = "cpu_validator")]
    validator_state: CpuValidatorState,
    #[cfg(feature = "cpu_validator")]
    validator_mode: ValidatorMode,
    #[cfg(feature = "cpu_validator")]
    validator_end: usize,
    #[cfg(feature = "cpu_validator")]
    peek_fetch: u8,
    #[cfg(feature = "cpu_validator")]
    instr_slice: Vec<u8>,

    end_addr: usize,

    service_events: VecDeque<ServiceEvent>,

    // Interrupt scheduling
    interrupt_scheduling:   bool,
    interrupt_cycle_period: u32,
    interrupt_cycle_num:    u32,
    interrupt_retrigger:    bool,

    clk0: bool,

    // DMA stuff
    dma_state: DmaState,
    dram_refresh_simulation: bool,
    dram_refresh_cycle_period: u32,
    dram_refresh_cycle_num: u32,
    dram_refresh_adjust: u32,
    dram_refresh_tc: bool,
    dram_refresh_retrigger: bool,
    dma_aen: bool,
    dma_holda: bool,
    dma_req: bool,
    dma_ack: bool,
    dma_wait_states: u32,

    // Trap stuff
    trap_enable_delay:  u32,  // Number of cycles to delay trap flag enablement.
    trap_disable_delay: u32,  // Number of cycles to delay trap flag disablement.
    trap_suppressed:    bool, // Suppress trap handling for the last executed instruction.

    nmi: bool,           // Status of NMI line.
    nmi_triggered: bool, // Has NMI been edge-triggered?

    halt_resume_delay: u32,
    int_flags: Vec<u8>,
    io_flags: Vec<u8>,

    analyzer: LogicAnalyzer,
}

#[cfg(feature = "cpu_validator")]
#[derive(PartialEq, Copy, Clone)]
pub enum CpuValidatorState {
    Uninitialized,
    Running,
    Hung,
    Ended,
}

#[cfg(feature = "cpu_validator")]
impl Default for CpuValidatorState {
    fn default() -> Self {
        CpuValidatorState::Uninitialized
    }
}

pub struct CpuRegisterState {
    pub ah:    u8,
    pub al:    u8,
    pub ax:    u16,
    pub bh:    u8,
    pub bl:    u8,
    pub bx:    u16,
    pub ch:    u8,
    pub cl:    u8,
    pub cx:    u16,
    pub dh:    u8,
    pub dl:    u8,
    pub dx:    u16,
    pub sp:    u16,
    pub bp:    u16,
    pub si:    u16,
    pub di:    u16,
    pub cs:    u16,
    pub ds:    u16,
    pub ss:    u16,
    pub es:    u16,
    pub pc:    u16,
    pub ip:    u16,
    pub flags: u16,
}

/*
pub enum RegisterType {
    Register8(u8),
    Register16(u16)
}
*/

/// The 8088 has a 7-cycle bus access time. 3 of these cycles can be pipelined during the previous
/// bus cycle. These cycles can alternatively be considered an 'address cycle'.
/// Tr: The cycle on which the EU or prefetcher requests a bus cycle
/// Ts: The first cycle of address calculation
/// T0: The last cycle of address calculation (may repeat)
/// Td: No address calculation in progress (done)
/// https://martypc.blogspot.com/2024/02/the-complete-bus-logic-of-intel-8088.html
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum TaCycle {
    #[default]
    Tr, // T-Request. This is the cycle on which the EU or prefetcher requests a bus cycle.
    Ts, // T-Start. This is the cycle on which the first cycle of address calculation occurs.
    T0, // T-Zero. This is the cycle on which address calculation completes. T0 may be repeated.
    Td, // T-done. Not a real cycle state, but indicates that no address cycle is in progress.
    Ta, // T-abort. Signals that the address cycle has been aborted.
}

impl Display for TaCycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaCycle::Tr => write!(f, "Tr"),
            TaCycle::Ts => write!(f, "Ts"),
            TaCycle::T0 => write!(f, "T0"),
            TaCycle::Td => write!(f, "  "),
            TaCycle::Ta => write!(f, "Ta"),
        }
    }
}

/// The traditional bus cycle model of the 8088 includes T-states from T1 to T4. The CPU executes
/// 'Ti' or idle T-states when not in an active bus transaction.
/// Tinit is not a real T-cycle but a state that indicates a new bus cycle has just been initiated
/// and should be moved to a valid state.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum TCycle {
    Tinit,
    #[default]
    Ti,
    T1,
    T2,
    T3,
    Tw,
    T4,
}

impl Display for TCycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TCycle::Tinit => write!(f, "Tx"),
            TCycle::Ti => write!(f, "Ti"),
            TCycle::T1 => write!(f, "T1"),
            TCycle::T2 => write!(f, "T2"),
            TCycle::T3 => write!(f, "T3"),
            TCycle::Tw => write!(f, "Tw"),
            TCycle::T4 => write!(f, "T4"),
        }
    }
}

/// The 8088 has 8 possible bus cycle types. These are advertised as an octal value on CPU status
/// pins S0-S2 in Maximum mode.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum BusStatus {
    InterruptAck = 0, // IRQ Acknowledge
    IoRead = 1,       // IO Read
    IoWrite = 2,      // IO Write
    Halt = 3,         // Halt
    CodeFetch = 4,    // Code Access
    MemRead = 5,      // Memory Read
    MemWrite = 6,     // Memory Write
    #[default]
    Passive = 7, // Passive
}

impl Display for BusStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BusStatus::InterruptAck => write!(f, "INTA"),
            BusStatus::IoRead => write!(f, "IOR "),
            BusStatus::IoWrite => write!(f, "IOW "),
            BusStatus::Halt => write!(f, "HALT"),
            BusStatus::CodeFetch => write!(f, "CODE"),
            BusStatus::MemRead => write!(f, "MEMR"),
            BusStatus::MemWrite => write!(f, "MEMW"),
            BusStatus::Passive => write!(f, "PASV"),
        }
    }
}

impl Intel808x {
    pub fn new(
        cpu_type: CpuType,
        cpu_subtype: CpuSubType,
        clock_factor: Option<ClockFactor>,
        trace_mode: TraceMode,
        trace_logger: TraceLogger,
        #[cfg(feature = "cpu_validator")] validator_type: ValidatorType,
        #[cfg(feature = "cpu_validator")] validator_trace: TraceLogger,
        #[cfg(feature = "cpu_validator")] validator_mode: ValidatorMode,
        #[cfg(feature = "cpu_validator")] validator_baud: u32,
    ) -> Self {
        let mut cpu: Intel808x = Default::default();

        match cpu_subtype {
            CpuSubType::Harris80C88 | CpuSubType::Intel8088 => {
                cpu.queue.set_size(4, 1);
                cpu.fetch_size = TransferSize::Byte;
            }
            CpuSubType::Intel8086 => {
                cpu.queue.set_size(6, 2);
                cpu.fetch_size = TransferSize::Word;
            }
            _ => {
                panic!("Invalid CPU subtype.")
            }
        }

        #[cfg(feature = "cpu_validator")]
        {
            cpu.validator = match validator_type {
                #[cfg(feature = "arduino_validator")]
                ValidatorType::Arduino8088 => Some(Box::new(ArduinoValidator::new(
                    cpu_type,
                    validator_trace,
                    validator_baud,
                ))),
                _ => None,
            };

            if let Some(ref mut validator) = cpu.validator {
                match validator.init(validator_mode, true, true, false) {
                    true => {}
                    false => {
                        panic!("Failed to init cpu validator.");
                    }
                }
            }

            cpu.validator_mode = validator_mode;
        }

        cpu.trace_logger = trace_logger;
        cpu.trace_mode = trace_mode;
        cpu.cpu_type = cpu_type;
        cpu.cpu_subtype = cpu_subtype;

        //cpu.instruction_history_on = true; // Control this from config/GUI instead
        cpu.instruction_history = VecDeque::with_capacity(16);

        cpu.reset_vector = CpuAddress::Segmented(0xFFFF, 0x0000);

        cpu.int_flags = vec![0; 256];
        cpu.io_flags = vec![0; 0x10000];

        if let Some(clock_factor) = clock_factor {
            cpu.analyzer.set_factor(clock_factor);
        }

        cpu.reset();
        cpu
    }

    pub fn reset(&mut self) {
        log::debug!("CPU Resetting...");
        /*
        let trace_logger = std::mem::replace(&mut self.trace_logger, TraceLogger::None);

        // Save non-default values
        *self = Self {
            // Save parameters to new()
            cpu_type: self.cpu_type,
            reset_vector: self.reset_vector,
            trace_mode: self.trace_mode,
            trace_logger,
            // Save options
            instruction_history_on: self.instruction_history_on,
            dram_refresh_simulation: self.dram_refresh_simulation,
            halt_resume_delay: self.halt_resume_delay,
            off_rails_detection: self.off_rails_detection,
            enable_wait_states: self.enable_wait_states,
            trace_enabled: self.trace_enabled,

            // Copy bus
            bus: self.bus,

            #[cfg(feature = "cpu_validator")]
            validator_type: ValidatorType,
            #[cfg(feature = "cpu_validator")]
            validator_trace: TraceLogger,
            ..Self::default()
        };
        */

        self.state = CpuState::Normal;

        self.set_register16(Register16::AX, 0);
        self.set_register16(Register16::BX, 0);
        self.set_register16(Register16::CX, 0);
        self.set_register16(Register16::DX, 0);
        self.set_register16(Register16::SP, 0);
        self.set_register16(Register16::BP, 0);
        self.set_register16(Register16::SI, 0);
        self.set_register16(Register16::DI, 0);
        self.set_register16(Register16::ES, 0);

        self.set_register16(Register16::SS, 0);
        self.set_register16(Register16::DS, 0);

        self.flags = CPU_FLAGS_RESERVED_ON;

        self.queue.flush();

        if let CpuAddress::Segmented(segment, offset) = self.reset_vector {
            self.set_register16(Register16::CS, segment);
            self.set_register16(Register16::PC, offset);
        }
        else {
            panic!("Invalid CpuAddress for reset vector.");
        }

        self.address_latch = 0;
        self.bus_status = BusStatus::Passive;
        self.bus_status_latch = BusStatus::Passive;
        self.t_cycle = TCycle::Ti;
        self.ta_cycle = TaCycle::Td;
        self.pl_status = BusStatus::Passive;
        self.pl_slot = false;

        self.fetch_state = FetchState::Normal;

        self.instruction_count = 0;
        self.int_count = 0;
        self.iret_count = 0;
        self.instr_cycle = 0;
        self.cycle_num = 1;
        self.halt_cycles = 0;
        self.t_stamp = 0.0;
        self.t_step = 0.00000021;
        self.t_step_h = 0.000000105;
        self.ready = true;
        self.in_rep = false;
        self.halted = false;
        self.reported_halt = false;
        self.halt_not_hold = false;
        self.opcode0_counter = 0;
        self.interrupt_inhibit = false;
        self.intr_pending = false;
        self.in_int = false;
        self.is_error = false;
        self.instruction_history.clear();
        self.call_stack.clear();
        //self.int_flags = vec![0; 256];
        //self.io_flags = vec![0; 0x10000];

        self.instruction_reentrant = false;
        self.last_ip = 0;
        self.last_cs = 0;
        self.last_intr = false;
        self.jumped = false;

        self.queue_op = QueueOp::Idle;
        self.last_queue_op = QueueOp::Idle;

        self.i8288.ale = false;
        self.i8288.mrdc = false;
        self.i8288.amwc = false;
        self.i8288.mwtc = false;
        self.i8288.iorc = false;
        self.i8288.aiowc = false;
        self.i8288.iowc = false;

        self.dram_refresh_tc = false;
        self.dram_refresh_retrigger = false;

        self.step_over_target = None;
        self.step_over_breakpoint = None;
        self.end_addr = 0xFFFFF;
        self.stopwatch_running = false;

        self.nx = false;
        self.rni = false;

        self.halt_resume_delay = 4;

        self.analyzer.clear();

        trace_print!(self, "Resetting CPU!");

        // Reset takes 6 cycles before first fetch
        self.cycle();
        self.biu_fetch_suspend();
        cycles_mc!(self, 0x1e4, 0x1e5);

        // If reset queue contents are provided, set the queue contents instead of flushing.
        if let Some(reset_queue) = self.reset_queue.clone() {
            self.set_queue_contents(reset_queue);
        }
        else {
            self.biu_queue_flush();
        }
        self.reset_queue = None;

        cycles_mc!(self, 0x1e6, 0x1e7, 0x1e8);

        #[cfg(feature = "cpu_validator")]
        {
            self.validator_state = CpuValidatorState::Uninitialized;
            self.cycle_states.clear();
        }

        trace_print!(self, "Reset CPU! CS: {:04X} IP: {:04X}", self.cs, self.ip());
    }

    pub fn get_instruction_ct(&self) -> u64 {
        self.instruction_count
    }

    /// Calculate the value of IP as needed. The IP register on the 808X is not a physical register,
    /// but produced on demand by adjusting PC by the size of the queue.
    #[inline]
    pub fn ip(&self) -> u16 {
        self.pc.wrapping_sub(self.queue.len_p() as u16)
    }

    #[inline]
    pub fn ip_v(&self) -> u16 {
        self.pc.wrapping_sub(self.queue.len() as u16)
    }

    /// Return the IP value for disassembly purposes. We wish to adjust the real value of IP in
    /// certain circumstances, such as when a reentrant instruction is being executed. This is so
    /// that reentrant instructions stay in the disassembly viewer until completed.
    pub fn disassembly_ip(&self) -> u16 {
        if self.instruction_reentrant {
            self.pc.wrapping_sub(self.queue.len_p() as u16 + self.i.size as u16)
        }
        else {
            self.pc.wrapping_sub(self.queue.len_p() as u16)
        }
    }

    pub fn flat_sp(&self) -> u32 {
        Intel808x::calc_linear_address(self.ss, self.sp)
    }

    /// Execute the CORR (Correct PC) microcode routine.
    /// This is used to correct the PC in anticipation of a jump or call, where the queue will
    /// be flushed. Unlike most microcode instructions, CORR takes two cycles to execute.
    #[inline]
    pub fn corr(&mut self) {
        self.pc = self.pc.wrapping_sub(self.queue.len() as u16);
        self.cycle_i(MC_CORR);
    }

    #[allow(dead_code)]
    pub fn in_rep(&self) -> bool {
        self.in_rep
    }

    pub fn get_csip(&self) -> CpuAddress {
        CpuAddress::Segmented(self.cs, self.ip())
    }

    #[inline(always)]
    pub fn is_last_wait_t3tw(&self) -> bool {
        self.wait_states == 0 && self.io_wait_states == 0 && self.dma_wait_states == 0
    }

    #[inline]
    pub fn is_last_wait(&self) -> bool {
        match self.t_cycle {
            TCycle::T3 | TCycle::Tw => self.is_last_wait_t3tw(),
            _ => false,
        }
    }

    #[inline]
    pub fn is_before_last_wait(&self) -> bool {
        match self.t_cycle {
            TCycle::T1 | TCycle::T2 => true,
            TCycle::T3 | TCycle::Tw => self.is_last_wait_t3tw(),
            _ => false,
        }
    }

    #[inline]
    pub fn is_before_t3(&self) -> bool {
        match self.t_cycle {
            TCycle::T1 | TCycle::T2 => true,
            _ => false,
        }
    }

    #[cfg(feature = "cpu_validator")]
    pub fn get_cycle_state(&mut self) -> CycleState {
        let mut q = [0; 4];
        self.queue.to_slice(&mut q);

        CycleState {
            n: self.instr_cycle,
            addr: self.address_latch,
            t_state: match self.t_cycle {
                TCycle::Tinit | TCycle::T1 => BusCycle::T1,
                TCycle::Ti => BusCycle::T1,
                TCycle::T2 => BusCycle::T2,
                TCycle::T3 => BusCycle::T3,
                TCycle::Tw => BusCycle::Tw,
                TCycle::T4 => BusCycle::T4,
            },
            a_type: match self.bus_segment {
                Segment::ES => AccessType::AlternateData,
                Segment::SS => AccessType::Stack,
                Segment::DS => AccessType::Data,
                Segment::None | Segment::CS => AccessType::CodeOrNone,
            },
            // TODO: Unify these enums?
            b_state: match self.t_cycle {
                TCycle::T1 | TCycle::T2 => match self.bus_status_latch {
                    BusStatus::InterruptAck => BusState::INTA,
                    BusStatus::IoRead => BusState::IOR,
                    BusStatus::IoWrite => BusState::IOW,
                    BusStatus::Halt => BusState::HALT,
                    BusStatus::CodeFetch => BusState::CODE,
                    BusStatus::MemRead => BusState::MEMR,
                    BusStatus::MemWrite => BusState::MEMW,
                    BusStatus::Passive => BusState::PASV,
                },
                _ => BusState::PASV,
            },
            ale: self.i8288.ale,
            mrdc: !self.i8288.mrdc,
            amwc: !self.i8288.amwc,
            mwtc: !self.i8288.mwtc,
            iorc: !self.i8288.iorc,
            aiowc: !self.i8288.aiowc,
            iowc: !self.i8288.iowc,
            inta: !self.i8288.inta,
            bhe: false,
            q_op: self.last_queue_op,
            q_byte: self.last_queue_byte,
            q_len: self.queue.len() as u32,
            q,
            data_bus: self.data_bus,
        }
    }

    pub fn is_error(&self) -> bool {
        self.is_error
    }

    pub fn set_nmi(&mut self, nmi_state: bool) {
        if nmi_state == false {
            self.nmi_triggered = false;
        }
        self.nmi = nmi_state;
    }

    #[inline(always)]
    pub fn set_flag(&mut self, flag: Flag) {
        self.flags |= match flag {
            Flag::Carry => CPU_FLAG_CARRY,
            Flag::Parity => CPU_FLAG_PARITY,
            Flag::AuxCarry => CPU_FLAG_AUX_CARRY,
            Flag::Zero => CPU_FLAG_ZERO,
            Flag::Sign => CPU_FLAG_SIGN,
            Flag::Trap => CPU_FLAG_TRAP,
            Flag::Interrupt => {
                // Only inhibit interrupts if the interrupt flag was not previously set
                if !self.get_flag(Flag::Interrupt) {
                    self.interrupt_inhibit = true;
                }
                CPU_FLAG_INT_ENABLE
            }
            Flag::Direction => CPU_FLAG_DIRECTION,
            Flag::Overflow => CPU_FLAG_OVERFLOW,
        };
    }

    #[inline(always)]
    pub fn clear_flag(&mut self, flag: Flag) {
        self.flags &= match flag {
            Flag::Carry => !CPU_FLAG_CARRY,
            Flag::Parity => !CPU_FLAG_PARITY,
            Flag::AuxCarry => !CPU_FLAG_AUX_CARRY,
            Flag::Zero => !CPU_FLAG_ZERO,
            Flag::Sign => !CPU_FLAG_SIGN,
            Flag::Trap => !CPU_FLAG_TRAP,
            Flag::Interrupt => !CPU_FLAG_INT_ENABLE,
            Flag::Direction => !CPU_FLAG_DIRECTION,
            Flag::Overflow => !CPU_FLAG_OVERFLOW,
        };
    }

    pub fn set_flags(&mut self, mut flags: u16) {
        // Clear reserved 0 flags
        flags &= CPU_FLAGS_RESERVED_OFF;
        // Set reserved 1 flags
        flags |= CPU_FLAGS_RESERVED_ON;

        self.flags = flags;
    }

    #[inline(always)]
    pub fn set_flag_state(&mut self, flag: Flag, state: bool) {
        if state {
            self.set_flag(flag)
        }
        else {
            self.clear_flag(flag)
        }
    }

    pub fn store_flags(&mut self, bits: u16) {
        // Clear SF, ZF, AF, PF & CF flags
        let flag_mask = !(CPU_FLAG_CARRY | CPU_FLAG_PARITY | CPU_FLAG_AUX_CARRY | CPU_FLAG_ZERO | CPU_FLAG_SIGN);
        self.flags &= flag_mask;

        // Copy flag state
        self.flags |= bits & !flag_mask;
    }

    pub fn load_flags(&mut self) -> u16 {
        // Return 8 LO bits of flags register
        self.flags & 0x00FF
    }

    #[inline]
    pub fn get_flag(&self, flag: Flag) -> bool {
        self.flags
            & match flag {
                Flag::Carry => CPU_FLAG_CARRY,
                Flag::Parity => CPU_FLAG_PARITY,
                Flag::AuxCarry => CPU_FLAG_AUX_CARRY,
                Flag::Zero => CPU_FLAG_ZERO,
                Flag::Sign => CPU_FLAG_SIGN,
                Flag::Trap => CPU_FLAG_TRAP,
                Flag::Interrupt => CPU_FLAG_INT_ENABLE,
                Flag::Direction => CPU_FLAG_DIRECTION,
                Flag::Overflow => CPU_FLAG_OVERFLOW,
            }
            != 0
    }

    #[cfg(feature = "cpu_validator")]
    pub fn get_vregisters(&self) -> VRegisters {
        VRegisters {
            ax:    self.a.x(),
            bx:    self.b.x(),
            cx:    self.c.x(),
            dx:    self.d.x(),
            cs:    self.cs,
            ss:    self.ss,
            ds:    self.ds,
            es:    self.es,
            sp:    self.sp,
            bp:    self.bp,
            si:    self.si,
            di:    self.di,
            ip:    self.ip(),
            flags: self.flags,
        }
    }

    /*
    pub fn get_register(&self, reg: Register) -> RegisterType {
        match reg {
            Register::AH => RegisterType::Register8(self.ah),
            Register::AL => RegisterType::Register8(self.al),
            Register::AX => RegisterType::Register16(self.ax),
            Register::BH => RegisterType::Register8(self.bh),
            Register::BL => RegisterType::Register8(self.bl),
            Register::BX => RegisterType::Register16(self.bx),
            Register::CH => RegisterType::Register8(self.ch),
            Register::CL => RegisterType::Register8(self.cl),
            Register::CX => RegisterType::Register16(self.cx),
            Register::DH => RegisterType::Register8(self.dh),
            Register::DL => RegisterType::Register8(self.dl),
            Register::DX => RegisterType::Register16(self.dx),
            Register::SP => RegisterType::Register16(self.sp),
            Register::BP => RegisterType::Register16(self.bp),
            Register::SI => RegisterType::Register16(self.si),
            Register::DI => RegisterType::Register16(self.di),
            Register::CS => RegisterType::Register16(self.cs),
            Register::DS => RegisterType::Register16(self.ds),
            Register::SS => RegisterType::Register16(self.ss),
            Register::ES => RegisterType::Register16(self.es),
            _ => panic!("Invalid register")
        }
    }
    */

    #[inline]
    pub fn get_register8(&self, reg: Register8) -> u8 {
        match reg {
            Register8::AH => self.a.h(),
            Register8::AL => self.a.l(),
            Register8::BH => self.b.h(),
            Register8::BL => self.b.l(),
            Register8::CH => self.c.h(),
            Register8::CL => self.c.l(),
            Register8::DH => self.d.h(),
            Register8::DL => self.d.l(),
        }
    }

    #[inline]
    pub fn get_register16(&self, reg: Register16) -> u16 {
        match reg {
            Register16::AX => self.a.x(),
            Register16::BX => self.b.x(),
            Register16::CX => self.c.x(),
            Register16::DX => self.d.x(),
            Register16::SP => self.sp,
            Register16::BP => self.bp,
            Register16::SI => self.si,
            Register16::DI => self.di,
            Register16::CS => self.cs,
            Register16::DS => self.ds,
            Register16::SS => self.ss,
            Register16::ES => self.es,
            Register16::PC => self.pc,
            _ => panic!("Invalid register"),
        }
    }

    #[inline]
    pub fn get_flags(&self) -> u16 {
        self.flags
    }

    // Set one of the 8 bit registers.
    #[inline]
    pub fn set_register8(&mut self, reg: Register8, value: u8) {
        match reg {
            Register8::AH => self.a.set_h(value),
            Register8::AL => self.a.set_l(value),
            Register8::BH => self.b.set_h(value),
            Register8::BL => self.b.set_l(value),
            Register8::CH => self.c.set_h(value),
            Register8::CL => self.c.set_l(value),
            Register8::DH => self.d.set_h(value),
            Register8::DL => self.d.set_l(value),
        }
    }

    // Set one of the 16 bit registers.
    #[inline]
    pub fn set_register16(&mut self, reg: Register16, value: u16) {
        match reg {
            Register16::AX => self.a.set_x(value),
            Register16::BX => self.b.set_x(value),
            Register16::CX => self.c.set_x(value),
            Register16::DX => self.d.set_x(value),
            Register16::SP => self.sp = value,
            Register16::BP => self.bp = value,
            Register16::SI => self.si = value,
            Register16::DI => self.di = value,
            Register16::CS => self.cs = value,
            Register16::DS => self.ds = value,
            Register16::SS => self.ss = value,
            Register16::ES => self.es = value,
            Register16::PC => self.pc = value,
            _ => panic!("bad register16"),
        }
    }

    /// Converts a Register8 into a Register16.
    /// Only really useful for r forms of FE.03-07 which operate on 8 bits of a memory
    /// operand but 16 bits of a register operand. We don't support 'hybrid' 8/16 bit
    /// instruction templates, so we have to convert.
    #[inline]
    pub fn reg8to16(reg: Register8) -> Register16 {
        match reg {
            Register8::AH => Register16::AX,
            Register8::AL => Register16::AX,
            Register8::BH => Register16::BX,
            Register8::BL => Register16::BX,
            Register8::CH => Register16::CX,
            Register8::CL => Register16::CX,
            Register8::DH => Register16::DX,
            Register8::DL => Register16::DX,
        }
    }

    #[inline]
    pub fn decrement_register8(&mut self, reg: Register8) {
        match reg {
            Register8::AH => self.a.decr_h(),
            Register8::AL => self.a.decr_l(),
            Register8::BH => self.b.decr_h(),
            Register8::BL => self.b.decr_l(),
            Register8::CH => self.c.decr_h(),
            Register8::CL => self.c.decr_l(),
            Register8::DH => self.d.decr_h(),
            Register8::DL => self.d.decr_l(),
        }
    }

    #[inline]
    pub fn decrement_register16(&mut self, reg: Register16) {
        match reg {
            Register16::AX => self.a.decr_x(),
            Register16::BX => self.b.decr_x(),
            Register16::CX => self.c.decr_x(),
            Register16::DX => self.d.decr_x(),
            Register16::SP => self.sp = self.sp.wrapping_sub(1),
            Register16::BP => self.bp = self.bp.wrapping_sub(1),
            Register16::SI => self.si = self.si.wrapping_sub(1),
            Register16::DI => self.di = self.di.wrapping_sub(1),
            Register16::CS => self.cs = self.cs.wrapping_sub(1),
            Register16::DS => self.ds = self.ds.wrapping_sub(1),
            Register16::SS => self.ss = self.ss.wrapping_sub(1),
            Register16::ES => self.es = self.es.wrapping_sub(1),
            _ => {}
        }
    }

    pub fn set_reset_vector(&mut self, reset_vector: CpuAddress) {
        self.reset_vector = reset_vector;
    }

    pub fn get_reset_vector(&self) -> CpuAddress {
        self.reset_vector
    }

    pub fn reset_address(&mut self) {
        if let CpuAddress::Segmented(segment, offset) = self.reset_vector {
            self.cs = segment;
            self.pc = offset;
        }
    }

    pub fn get_state(&self) -> CpuRegisterState {
        CpuRegisterState {
            ah:    self.a.h(),
            al:    self.a.l(),
            ax:    self.a.x(),
            bh:    self.b.h(),
            bl:    self.b.l(),
            bx:    self.b.x(),
            ch:    self.c.h(),
            cl:    self.c.l(),
            cx:    self.c.x(),
            dh:    self.d.h(),
            dl:    self.d.l(),
            dx:    self.d.x(),
            sp:    self.sp,
            bp:    self.bp,
            si:    self.si,
            di:    self.di,
            cs:    self.cs,
            ds:    self.ds,
            ss:    self.ss,
            es:    self.es,
            ip:    self.ip(),
            pc:    self.pc,
            flags: self.flags,
        }
    }

    /// Get a string representation of the CPU state.
    /// This is used to display the CPU state viewer window in the debug GUI.
    pub fn get_string_state(&self) -> CpuStringState {
        CpuStringState {
            ah:   format!("{:02x}", self.a.h()),
            al:   format!("{:02x}", self.a.l()),
            ax:   format!("{:04x}", self.a.x()),
            bh:   format!("{:02x}", self.b.h()),
            bl:   format!("{:02x}", self.b.l()),
            bx:   format!("{:04x}", self.b.x()),
            ch:   format!("{:02x}", self.c.h()),
            cl:   format!("{:02x}", self.c.l()),
            cx:   format!("{:04x}", self.c.x()),
            dh:   format!("{:02x}", self.d.h()),
            dl:   format!("{:02x}", self.d.l()),
            dx:   format!("{:04x}", self.d.x()),
            sp:   format!("{:04x}", self.sp),
            bp:   format!("{:04x}", self.bp),
            si:   format!("{:04x}", self.si),
            di:   format!("{:04x}", self.di),
            cs:   format!("{:04x}", self.cs),
            ds:   format!("{:04x}", self.ds),
            ss:   format!("{:04x}", self.ss),
            es:   format!("{:04x}", self.es),
            ip:   format!("{:04x}", self.ip()),
            pc:   format!("{:04x}", self.pc),
            c_fl: {
                let fl = self.flags & CPU_FLAG_CARRY > 0;
                format!("{:1}", fl as u8)
            },
            p_fl: {
                let fl = self.flags & CPU_FLAG_PARITY > 0;
                format!("{:1}", fl as u8)
            },
            a_fl: {
                let fl = self.flags & CPU_FLAG_AUX_CARRY > 0;
                format!("{:1}", fl as u8)
            },
            z_fl: {
                let fl = self.flags & CPU_FLAG_ZERO > 0;
                format!("{:1}", fl as u8)
            },
            s_fl: {
                let fl = self.flags & CPU_FLAG_SIGN > 0;
                format!("{:1}", fl as u8)
            },
            t_fl: {
                let fl = self.flags & CPU_FLAG_TRAP > 0;
                format!("{:1}", fl as u8)
            },
            i_fl: {
                let fl = self.flags & CPU_FLAG_INT_ENABLE > 0;
                format!("{:1}", fl as u8)
            },
            d_fl: {
                let fl = self.flags & CPU_FLAG_DIRECTION > 0;
                format!("{:1}", fl as u8)
            },
            o_fl: {
                let fl = self.flags & CPU_FLAG_OVERFLOW > 0;
                format!("{:1}", fl as u8)
            },

            piq: self.queue.to_string(),
            flags: format!("{:04}", self.flags),
            instruction_count: format!("{}", self.instruction_count),
            cycle_count: format!("{}", self.cycle_num),
            dma_state: format!("{:?}", self.dma_state),
        }
    }

    /// Evaluate a string expression such as 'cs:ip' to an address.
    /// Basic forms supported are [reg:reg], [reg:offset], [seg:offset]
    pub fn eval_address(&self, expr: &str) -> Option<CpuAddress> {
        lazy_static! {
            static ref FLAT_REX: Regex = Regex::new(r"(?P<flat>[A-Fa-f\d]{5})$").unwrap();
            static ref SEGMENTED_REX: Regex =
                Regex::new(r"(?P<segment>[A-Fa-f\d]{4}):(?P<offset>[A-Fa-f\d]{4})$").unwrap();
            static ref REGREG_REX: Regex = Regex::new(r"(?P<reg1>cs|ds|ss|es):(?P<reg2>\w{2})$").unwrap();
            static ref REGOFFSET_REX: Regex = Regex::new(r"(?P<reg1>cs|ds|ss|es):(?P<offset>[A-Fa-f\d]{4})$").unwrap();
        }

        if FLAT_REX.is_match(expr) {
            match u32::from_str_radix(expr, 16) {
                Ok(address) => Some(CpuAddress::Flat(address)),
                Err(_) => None,
            }
        }
        else if let Some(caps) = SEGMENTED_REX.captures(expr) {
            let segment_str = &caps["segment"];
            let offset_str = &caps["offset"];

            let segment_u16r = u16::from_str_radix(segment_str, 16);
            let offset_u16r = u16::from_str_radix(offset_str, 16);

            match (segment_u16r, offset_u16r) {
                (Ok(segment), Ok(offset)) => Some(CpuAddress::Segmented(segment, offset)),
                _ => None,
            }
        }
        else if let Some(caps) = REGREG_REX.captures(expr) {
            let reg1 = &caps["reg1"];
            let reg2 = &caps["reg2"];

            let segment = match reg1 {
                "cs" => self.cs,
                "ds" => self.ds,
                "ss" => self.ss,
                "es" => self.es,
                _ => 0,
            };

            let offset = match reg2 {
                "ah" => self.a.h() as u16,
                "al" => self.a.l() as u16,
                "ax" => self.a.x(),
                "bh" => self.b.h() as u16,
                "bl" => self.b.l() as u16,
                "bx" => self.b.x(),
                "ch" => self.c.h() as u16,
                "cl" => self.c.l() as u16,
                "cx" => self.c.x(),
                "dh" => self.d.h() as u16,
                "dl" => self.d.l() as u16,
                "dx" => self.d.x(),
                "sp" => self.sp,
                "bp" => self.bp,
                "si" => self.si,
                "di" => self.di,
                "cs" => self.cs,
                "ds" => self.ds,
                "ss" => self.ss,
                "es" => self.es,
                "ip" => self.disassembly_ip(), // Use reentrant IP for disassembly
                _ => 0,
            };

            Some(CpuAddress::Segmented(segment, offset))
        }
        else if let Some(caps) = REGOFFSET_REX.captures(expr) {
            let reg1 = &caps["reg1"];
            let offset_str = &caps["offset"];

            let segment = match reg1 {
                "cs" => self.cs,
                "ds" => self.ds,
                "ss" => self.ss,
                "es" => self.es,
                _ => 0,
            };

            let offset_u16r = u16::from_str_radix(offset_str, 16);

            match offset_u16r {
                Ok(offset) => Some(CpuAddress::Segmented(segment, offset)),
                _ => None,
            }
        }
        else {
            None
        }
    }

    /// Push an entry on to the call stack. This can either be a CALL or an INT.
    pub fn push_call_stack(&mut self, entry: CallStackEntry, cs: u16, ip: u16) {
        if self.call_stack.len() < CPU_CALL_STACK_LEN {
            self.call_stack.push_back(entry);

            // Flag the specified CS:IP as a return address
            let return_addr = Intel808x::calc_linear_address(cs, ip);

            self.bus.set_flags(return_addr as usize, MEM_RET_BIT);
        }
        else {
            // TODO: set a flag to indicate that the call stack has overflowed?
        }
    }

    /// Rewind the call stack to the specified address.
    ///
    /// We have to rewind the call stack to the earliest appearance of this address we returned to,
    /// because popping the call stack clears the return flag from the memory location, so we don't
    /// support reentrancy.
    ///
    /// Maintaining a call stack is trickier than expected. JUMPs can RET, CALLS can JMP back, ISRs
    /// may not always IRET, so there is no other reliable way to pop a "return" from CALL/INT other
    /// than to mark the return address as the end of that CALL/INT and rewind when we reach that
    /// address again. It isn't perfect, but "good enough" for debugging.
    pub fn rewind_call_stack(&mut self, addr: u32) {
        let mut return_addr: u32 = 0;

        let pos = self.call_stack.iter().position(|&call| {
            return_addr = match call {
                CallStackEntry::CallF { ret_cs, ret_ip, .. } => Intel808x::calc_linear_address(ret_cs, ret_ip),
                CallStackEntry::Call { ret_cs, ret_ip, .. } => Intel808x::calc_linear_address(ret_cs, ret_ip),
                CallStackEntry::Interrupt { ret_cs, ret_ip, .. } => Intel808x::calc_linear_address(ret_cs, ret_ip),
            };

            return_addr == addr
        });

        if let Some(found_idx) = pos {
            let drained = self.call_stack.drain(found_idx..);

            drained.for_each(|drained_call| {
                return_addr = match drained_call {
                    CallStackEntry::CallF { ret_cs, ret_ip, .. } => Intel808x::calc_linear_address(ret_cs, ret_ip),
                    CallStackEntry::Call { ret_cs, ret_ip, .. } => Intel808x::calc_linear_address(ret_cs, ret_ip),
                    CallStackEntry::Interrupt { ret_cs, ret_ip, .. } => Intel808x::calc_linear_address(ret_cs, ret_ip),
                };

                // Clear flags for returns we popped
                self.bus.clear_flags(return_addr as usize, MEM_RET_BIT)
            })
        }
        else {
            log::warn!("rewind_call_stack(): no matching return for [{:05X}]", addr);
        }
    }

    /// Resume from halted state
    pub fn resume(&mut self) {
        if self.halted {
            //log::debug!("Resuming from halt");
            // It takes 6 or 7 cycles after INTR to enter INTA.
            // 3 of these are resuming from suspend, so not accounted from here.
            self.trace_comment("INTR");
            //log::debug!("resuming from halt with {} cycles", self.halt_resume_delay);
            self.cycles(self.halt_resume_delay);
        }
        else {
            log::warn!("resume() called but not halted!");
        }
        self.halted = false;
    }

    /// Set the status of the CPU's INTR line.
    #[inline]
    pub fn set_intr(&mut self, status: bool) {
        self.intr = status;
    }

    /// Set a terminating code address for the CPU. This is mostly used in conjunction with the
    /// CPU validator or running standalone binaries.
    pub fn set_end_address(&mut self, end: usize) {
        #[cfg(feature = "cpu_validator")]
        {
            self.validator_end = end;
        }

        self.end_addr = end;
    }

    /// Removes any cycle states at address 0
    #[cfg(feature = "cpu_validator")]
    fn clear_reset_cycle_states(&mut self) {
        self.cycle_states.retain(|&x| x.addr != 0);
    }

    /// Set CPU breakpoints from provided list.
    ///
    /// Clears bus breakpoint flags from previous breakpoint list before applying new.
    pub fn set_breakpoints(&mut self, bp_list: Vec<BreakPointType>) {
        // Clear bus flags for current breakpoints
        self.breakpoints.iter().for_each(|bp| match bp {
            BreakPointType::ExecuteFlat(addr) => {
                log::debug!("Clearing breakpoint on execute at address: {:05X}", *addr);
                self.bus.clear_flags(*addr as usize, MEM_BPE_BIT);
            }
            BreakPointType::MemAccessFlat(addr) => {
                self.bus.clear_flags(*addr as usize, MEM_BPA_BIT);
            }
            BreakPointType::Interrupt(vector) => {
                self.int_flags[*vector as usize] = 0;
            }
            BreakPointType::StartWatch(addr) => {
                self.bus.clear_flags(*addr as usize, MEM_SW_BIT);
            }
            BreakPointType::StopWatch(addr) => {
                self.bus.clear_flags(*addr as usize, MEM_SW_BIT);
            }
            BreakPointType::IoAccess(addr) => {
                self.io_flags[*addr as usize] = 0;
            }
            _ => {}
        });

        // Replace current breakpoint list
        self.breakpoints = bp_list;

        // Set bus flags for new breakpoints
        self.breakpoints.iter().for_each(|bp| match bp {
            BreakPointType::ExecuteFlat(addr) => {
                log::debug!("Setting breakpoint on execute at address: {:05X}", *addr);
                self.bus.set_flags(*addr as usize, MEM_BPE_BIT);
            }
            BreakPointType::MemAccessFlat(addr) => {
                log::debug!("Setting breakpoint on memory access at address: {:05X}", *addr);
                self.bus.set_flags(*addr as usize, MEM_BPA_BIT);
            }
            BreakPointType::Interrupt(vector) => {
                self.int_flags[*vector as usize] = INTERRUPT_BREAKPOINT;
            }
            BreakPointType::StartWatch(addr) => {
                self.bus.set_flags(*addr as usize, MEM_SW_BIT);
            }
            BreakPointType::StopWatch(addr) => {
                self.bus.set_flags(*addr as usize, MEM_SW_BIT);
            }
            BreakPointType::IoAccess(addr) => {
                log::debug!("Setting IO breakpoint flags at address: {:04X}", *addr);
                self.io_flags[*addr as usize] = IO_READ_BREAKPOINT | IO_WRITE_BREAKPOINT;
            }
            _ => {}
        });
    }

    pub fn add_breakpoint(&mut self, bp: &BreakPointType) {
        match bp {
            BreakPointType::ExecuteFlat(addr) => {
                log::debug!("Clearing breakpoint on execute at address: {:05X}", *addr);
                self.bus.set_flags(*addr as usize, MEM_BPE_BIT);
            }
            BreakPointType::MemAccessFlat(addr) => {
                self.bus.set_flags(*addr as usize, MEM_BPA_BIT);
            }
            BreakPointType::Interrupt(vector) => {
                self.int_flags[*vector as usize] = INTERRUPT_BREAKPOINT;
            }
            BreakPointType::StartWatch(addr) => {
                self.bus.set_flags(*addr as usize, MEM_SW_BIT);
            }
            BreakPointType::StopWatch(addr) => {
                self.bus.set_flags(*addr as usize, MEM_SW_BIT);
            }
            _ => {
                // Unsupported breakpoint type - ignore for now
                return;
            }
        }
    }

    pub fn set_stopwatch(&mut self, sw_idx: usize, start: u32, stop: u32) {
        if self.stopwatches.is_empty() {
            self.stopwatches.push(None);
        }
        self.stopwatches[sw_idx] = Some(CycleStopWatch::new(start, stop));
    }

    pub fn start_stopwatch(&mut self, sw_idx: usize) {
        if let Some(sw) = &mut self.stopwatches[sw_idx] {
            sw.start();
        }
    }

    pub fn stop_stopwatch(&mut self, sw_idx: usize) {
        if let Some(sw) = &mut self.stopwatches[sw_idx] {
            sw.stop();
        }
    }

    fn update_sw_flag(&mut self) {
        self.stopwatch_running = false;
        for sw in self.stopwatches.iter().flatten() {
            if sw.running() {
                self.stopwatch_running = true;
            }
        }
    }

    pub fn get_sw_data(&self) -> Vec<StopWatchData> {
        self.stopwatches.iter().flatten().map(|sw| sw.get_data()).collect()
    }

    pub fn set_step_over_breakpoint(&mut self, target: CpuAddress) {
        self.step_over_breakpoint = Some(u32::from(target));
    }

    pub fn get_step_over_breakpoint(&self) -> Option<CpuAddress> {
        match self.step_over_breakpoint {
            Some(addr) => Some(CpuAddress::Flat(addr)),
            None => None,
        }
    }

    pub fn get_breakpoint_flag(&self) -> bool {
        matches!(self.state, CpuState::BreakpointHit)
    }

    pub fn set_breakpoint_flag(&mut self) {
        self.state = CpuState::BreakpointHit;
    }

    pub fn clear_breakpoint_flag(&mut self) {
        self.state = CpuState::Normal;
    }

    pub fn dump_instruction_history_string(&self) -> String {
        let mut disassembly_string = String::new();

        for i in &self.instruction_history {
            match i {
                HistoryEntry::InstructionEntry {
                    cs,
                    ip,
                    cycles: _,
                    interrupt,
                    jump: _,
                    i,
                } => {
                    let i_string = format!(
                        "{:05X}{} [{:04X}:{:04X}] {}\n",
                        i.address,
                        if *interrupt { '*' } else { ' ' },
                        *cs,
                        *ip,
                        i
                    );
                    disassembly_string.push_str(&i_string);
                }
                _ => {}
            }
        }
        disassembly_string
    }

    pub fn dump_instruction_history_tokens(&self) -> Vec<Vec<SyntaxToken>> {
        let mut history_vec = Vec::new();

        for i in &self.instruction_history {
            let mut i_token_vec = Vec::new();
            match i {
                HistoryEntry::InstructionEntry {
                    cs,
                    ip,
                    cycles,
                    interrupt,
                    jump,
                    i,
                } => {
                    if *jump {
                        i_token_vec.push(SyntaxToken::Formatter(SyntaxFormatType::HighlightLine(
                            HighlightType::Info,
                        )));
                    }
                    i_token_vec.push(SyntaxToken::MemoryAddressFlat(i.address, format!("{:05X}", i.address)));
                    i_token_vec.push(SyntaxToken::MemoryAddressSeg16(
                        *cs,
                        *ip,
                        format!("{:04X}:{:04X}{}", cs, ip, if *interrupt { '*' } else { ' ' }),
                    ));
                    i_token_vec.push(SyntaxToken::InstructionBytes(format!("{:012}", " ")));
                    i_token_vec.extend(i.tokenize());
                    i_token_vec.push(SyntaxToken::Formatter(SyntaxFormatType::Tab));
                    i_token_vec.push(SyntaxToken::Text(format!("{}", *cycles)));
                }
                HistoryEntry::InterruptEntry { cs, ip, cycles: _, iv } => {
                    i_token_vec.push(SyntaxToken::Formatter(SyntaxFormatType::HighlightLine(
                        HighlightType::Alert,
                    )));
                    i_token_vec.push(SyntaxToken::MemoryAddressFlat(0, format!("{:05}", "")));
                    i_token_vec.push(SyntaxToken::MemoryAddressSeg16(*cs, *ip, String::from("          ")));
                    i_token_vec.push(SyntaxToken::InstructionBytes(format!("{:012}", "")));
                    i_token_vec.push(SyntaxToken::Text(format!("INT {:02X}", iv)));
                }
                HistoryEntry::NmiEntry { cs, ip } => {
                    i_token_vec.push(SyntaxToken::Formatter(SyntaxFormatType::HighlightLine(
                        HighlightType::Alert,
                    )));
                    i_token_vec.push(SyntaxToken::MemoryAddressFlat(0, format!("{:05}", "")));
                    i_token_vec.push(SyntaxToken::MemoryAddressSeg16(*cs, *ip, String::from("          ")));
                    i_token_vec.push(SyntaxToken::InstructionBytes(format!("{:012}", "")));
                    i_token_vec.push(SyntaxToken::Text(String::from("NMI")));
                }
                HistoryEntry::TrapEntry { cs, ip } => {
                    i_token_vec.push(SyntaxToken::Formatter(SyntaxFormatType::HighlightLine(
                        HighlightType::Alert,
                    )));
                    i_token_vec.push(SyntaxToken::MemoryAddressFlat(0, format!("{:05}", "")));
                    i_token_vec.push(SyntaxToken::MemoryAddressSeg16(*cs, *ip, String::from("          ")));
                    i_token_vec.push(SyntaxToken::InstructionBytes(format!("{:012}", "")));
                    i_token_vec.push(SyntaxToken::Text(String::from("TRAP")));
                }
            }
            history_vec.push(i_token_vec);
        }
        history_vec
    }

    pub fn dump_call_stack(&self) -> String {
        let mut call_stack_string = String::new();

        for call in &self.call_stack {
            match call {
                CallStackEntry::Call {
                    ret_cs,
                    ret_ip,
                    call_ip,
                } => {
                    call_stack_string.push_str(&format!("{:04X}:{:04X} CALL {:04X}\n", ret_cs, ret_ip, call_ip));
                }
                CallStackEntry::CallF {
                    ret_cs,
                    ret_ip,
                    call_cs,
                    call_ip,
                } => {
                    call_stack_string.push_str(&format!(
                        "{:04X}:{:04X} CALL FAR {:04X}:{:04X}\n",
                        ret_cs, ret_ip, call_cs, call_ip
                    ));
                }
                CallStackEntry::Interrupt {
                    ret_cs,
                    ret_ip,
                    call_cs,
                    call_ip,
                    itype,
                    number,
                    ah,
                } => {
                    call_stack_string.push_str(&format!(
                        "{:04X}:{:04X} INT {:02X}h {:04X}:{:04X} type={:?} AH=={:02X}\n",
                        ret_cs, ret_ip, number, call_cs, call_ip, itype, ah
                    ));
                }
            }
        }

        call_stack_string
    }

    #[inline]
    pub fn trace_print(&mut self, trace_str: &str) {
        if self.trace_logger.is_some() {
            self.trace_logger.println(trace_str);
        }
    }

    #[inline]
    pub fn trace_emit(&mut self, trace_str: &str) {
        if self.trace_logger.is_some() {
            self.trace_logger.println(trace_str);
        }
    }

    pub fn trace_flush(&mut self) {
        if self.trace_logger.is_some() {
            self.trace_logger.flush();
        }

        #[cfg(feature = "cpu_validator")]
        {
            if let Some(val) = &mut self.validator {
                val.flush();
            }
        }
    }

    #[inline]
    pub fn trace_comment(&mut self, comment: &'static str) {
        if self.trace_enabled && (self.trace_mode == TraceMode::CycleText) {
            self.trace_comment.push(comment);
        }
    }

    #[inline]
    pub fn trace_instr(&mut self, instr: u16) {
        self.trace_instr = instr;
    }

    pub fn dump_cs(&self, path: &Path) {
        let filename = path.to_path_buf();

        let len = 0x10000;
        let address = (self.cs as usize) << 4;
        log::debug!("Dumping {} bytes at address {:05X}", len, address);
        let cs_slice = self.bus.get_slice_at(address, len);

        match std::fs::write(filename.clone(), &cs_slice) {
            Ok(_) => {
                log::debug!("Wrote memory dump: {}", filename.display())
            }
            Err(e) => {
                log::error!("Failed to write memory dump '{}': {}", filename.display(), e)
            }
        }
    }

    pub fn get_service_event(&mut self) -> Option<ServiceEvent> {
        self.service_events.pop_front()
    }
    pub fn get_cycle_trace(&self) -> &Vec<String> {
        &self.trace_str_vec
    }
    pub fn get_cycle_trace_tokens(&self) -> &Vec<Vec<SyntaxToken>> {
        &self.trace_token_vec
    }
    pub fn get_cycle_ct(&self) -> (u64, u64) {
        (self.cycle_num, self.halt_cycles)
    }

    #[cfg(feature = "cpu_validator")]
    pub fn get_validator_state(&self) -> CpuValidatorState {
        self.validator_state
    }

    #[cfg(feature = "cpu_validator")]
    pub fn get_validator(&self) -> &Option<Box<dyn CpuValidator>> {
        &self.validator
    }

    #[cfg(feature = "cpu_validator")]
    pub fn get_validator_mut(&mut self) -> &mut Option<Box<dyn CpuValidator>> {
        &mut self.validator
    }

    /// Specify queue contents to be set on next reset.
    pub fn set_reset_queue_contents(&mut self, contents: Vec<u8>) {
        self.reset_queue = Some(contents);
    }

    /// Set queue contents to the specified byte vector.
    pub fn set_queue_contents(&mut self, contents: Vec<u8>) {
        let old_len = self.queue.len();
        self.pc = self.pc.wrapping_sub(old_len as u16);
        self.queue.flush();
        for (i, byte) in contents.iter().enumerate() {
            if i < self.queue.get_size() {
                self.queue.push8(*byte);
                self.pc = self.pc.wrapping_add(1);
            }
        }
    }
}

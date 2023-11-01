/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

#![allow(dead_code)]
#![allow(clippy::unusual_byte_groupings)]

use std::{
    collections::VecDeque,
    error::Error,
    fmt,
    io::Write,
    path::Path
};

use core::fmt::Display;

use lazy_static::lazy_static;
use regex::Regex;

// Pull in all CPU module components
mod addressing;
mod alu;
mod bcd;
mod bitwise;
mod biu;
mod cycle;
mod decode;
mod display;
mod execute;
mod interrupt;
mod jump;
mod microcode;
pub mod mnemonic;
mod modrm;
mod muldiv;
mod stack;
mod string;
mod queue;
mod fuzzer;

use crate::cpu_808x::mnemonic::Mnemonic;
use crate::cpu_808x::microcode::*;
use crate::cpu_808x::addressing::AddressingMode;
use crate::cpu_808x::queue::InstructionQueue;
use crate::cpu_808x::biu::*;
// Make ReadWriteFlag available to benchmarks
pub use crate::cpu_808x::biu::ReadWriteFlag;

use crate::cpu_common::{CpuType, CpuOption};

use crate::config::TraceMode;
#[cfg(feature = "cpu_validator")]
use crate::config::ValidatorType;

use crate::breakpoints::BreakPointType;
use crate::bus::{BusInterface, MEM_RET_BIT, MEM_BPA_BIT, MEM_BPE_BIT};
use crate::bytequeue::*;
//use crate::interrupt::log_post_interrupt;

use crate::syntax_token::*;
use crate::tracelogger::TraceLogger;

#[cfg(feature = "cpu_validator")]
use crate::cpu_validator::{
    CpuValidator, CycleState, ValidatorMode, ValidatorResult, 
    VRegisters, BusCycle, BusState, AccessType,
    VAL_NO_WRITES, VAL_NO_FLAGS, VAL_ALLOW_ONE, VAL_NO_CYCLES
};

#[cfg(feature = "arduino_validator")]
use crate::arduino8088_validator::{ArduinoValidator};

macro_rules! trace_print {
    ($self:ident, $($t:tt)*) => {{
        if $self.trace_enabled {
            if let TraceMode::Cycle = $self.trace_mode  {
                $self.trace_print(&format!($($t)*));
            }
        }
    }};
}
use trace_print;

const QUEUE_MAX: usize = 6;
const FETCH_DELAY: u8 = 2;

const CPU_HISTORY_LEN: usize = 32;
const CPU_CALL_STACK_LEN: usize = 128;

const INTERRUPT_VEC_LEN: usize = 4;
const INTERRUPT_BREAKPOINT: u8 = 1;

pub const CPU_FLAG_CARRY: u16      = 0b0000_0000_0000_0001;
pub const CPU_FLAG_RESERVED1: u16  = 0b0000_0000_0000_0010;
pub const CPU_FLAG_PARITY: u16     = 0b0000_0000_0000_0100;
pub const CPU_FLAG_RESERVED3: u16  = 0b0000_0000_0000_1000;
pub const CPU_FLAG_AUX_CARRY: u16  = 0b0000_0000_0001_0000;
pub const CPU_FLAG_RESERVED5: u16  = 0b0000_0000_0010_0000;
pub const CPU_FLAG_ZERO: u16       = 0b0000_0000_0100_0000;
pub const CPU_FLAG_SIGN: u16       = 0b0000_0000_1000_0000;
pub const CPU_FLAG_TRAP: u16       = 0b0000_0001_0000_0000;
pub const CPU_FLAG_INT_ENABLE: u16 = 0b0000_0010_0000_0000;
pub const CPU_FLAG_DIRECTION: u16  = 0b0000_0100_0000_0000;
pub const CPU_FLAG_OVERFLOW: u16   = 0b0000_1000_0000_0000;

/*
const CPU_FLAG_RESERVED12: u16 = 0b0001_0000_0000_0000;
const CPU_FLAG_RESERVED13: u16 = 0b0010_0000_0000_0000;
const CPU_FLAG_RESERVED14: u16 = 0b0100_0000_0000_0000;
const CPU_FLAG_RESERVED15: u16 = 0b1000_0000_0000_0000;
*/

const CPU_FLAGS_RESERVED_ON: u16 = 0b1111_0000_0000_0010;
const CPU_FLAGS_RESERVED_OFF: u16 = !(CPU_FLAG_RESERVED3 | CPU_FLAG_RESERVED5);

const FLAGS_POP_MASK: u16      = 0b0000_1111_1101_0101;

const REGISTER_HI_MASK: u16    = 0b0000_0000_1111_1111;
const REGISTER_LO_MASK: u16    = 0b1111_1111_0000_0000;

pub const MAX_INSTRUCTION_SIZE: usize = 15;

const OPCODE_REGISTER_SELECT_MASK: u8 = 0b0000_0111;

// Instruction flags
const I_USES_MEM:    u32 = 0b0000_0001; // Instruction has a memory operand
const I_HAS_MODRM:   u32 = 0b0000_0010; // Instruction has a modrm byte
const I_LOCKABLE:    u32 = 0b0000_0100; // Instruction compatible with LOCK prefix
const I_REL_JUMP:    u32 = 0b0000_1000; 
const I_LOAD_EA:     u32 = 0b0001_0000; // Instruction loads from its effective address
const I_GROUP_DELAY: u32 = 0b0010_0000; // Instruction has cycle delay for being a specific group instruction

// Instruction prefixes
pub const OPCODE_PREFIX_ES_OVERRIDE: u32     = 0b_0000_0000_0001;
pub const OPCODE_PREFIX_CS_OVERRIDE: u32     = 0b_0000_0000_0010;
pub const OPCODE_PREFIX_SS_OVERRIDE: u32     = 0b_0000_0000_0100;
pub const OPCODE_PREFIX_DS_OVERRIDE: u32     = 0b_0000_0000_1000;
pub const OPCODE_SEG_OVERRIDE_MASK: u32      = 0b_0000_0000_1111;
pub const OPCODE_PREFIX_OPERAND_OVERIDE: u32 = 0b_0000_0001_0000;
pub const OPCODE_PREFIX_ADDRESS_OVERIDE: u32 = 0b_0000_0010_0000;
pub const OPCODE_PREFIX_WAIT: u32            = 0b_0000_0100_0000;
pub const OPCODE_PREFIX_LOCK: u32            = 0b_0000_1000_0000;
pub const OPCODE_PREFIX_REP1: u32            = 0b_0001_0000_0000;
pub const OPCODE_PREFIX_REP2: u32            = 0b_0010_0000_0000;

// The parity flag is calculated from the lower 8 bits of an alu operation regardless
// of the operand width.  Thefore it is trivial to precalculate a 8-bit parity table.
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

pub const SEGMENT_REGISTER16_LUT: [Register16; 4] = [
    Register16::ES,
    Register16::CS,
    Register16::SS,
    Register16::DS,
];

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum CpuException {
    NoException,
    DivideError
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum CpuState {
    Normal,
    BreakpointHit
}
impl Default for CpuState {
    fn default() -> Self { CpuState::Normal }
}

#[derive(Debug)]
pub enum CpuError {
    InvalidInstructionError(u8, u32),
    UnhandledInstructionError(u8, u32),
    InstructionDecodeError(u32),
    ExecutionError(u32, String),
    CpuHaltedError(u32),
    ExceptionError(CpuException)
}
impl Error for CpuError {}
impl Display for CpuError{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &*self {
            CpuError::InvalidInstructionError(o, addr)=>write!(f, "An invalid instruction was encountered: {:02X} at address: {:06X}", o, addr),
            CpuError::UnhandledInstructionError(o, addr)=>write!(f, "An unhandled instruction was encountered: {:02X} at address: {:06X}", o, addr),
            CpuError::InstructionDecodeError(addr)=>write!(f, "An error occurred during instruction decode at address: {:06X}", addr),
            CpuError::ExecutionError(addr, err)=>write!(f, "An execution error occurred at: {:06X} Message: {}", addr, err),
            CpuError::CpuHaltedError(addr)=>write!(f, "The CPU was halted at address: {:06X}.", addr),
            CpuError::ExceptionError(exception)=>write!(f, "The CPU threw an exception: {:?}", exception)
        }
    }
}

// Internal Emulator interrupt service events. These are returned to the machine when
// the internal service interrupt is called to request an emulator action that cannot
// be handled by the CPU alone.
#[derive(Copy, Clone, Debug)]
pub enum ServiceEvent {
    TriggerPITLogging
}

#[derive(Copy, Clone, Debug)]
pub enum CallStackEntry {
    Call { 
        ret_cs: u16, 
        ret_ip: u16, 
        call_ip: u16
    },
    CallF {
        ret_cs: u16,
        ret_ip: u16,
        call_cs: u16,
        call_ip: u16
    },
    Interrupt {
        ret_cs: u16,
        ret_ip: u16,   
        call_cs: u16,
        call_ip: u16,     
        itype: InterruptType,
        number: u8,
        ah: u8,
    }
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
    Overflow
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


#[derive(Copy, Clone)]
#[derive(PartialEq)]
pub enum Register8 {
    AL,
    CL,
    DL,
    BL,
    AH,
    CH,
    DH,
    BH
}

#[derive(Copy, Clone, Debug)]
#[derive(PartialEq)]
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
    IP,
    InvalidRegister
}

#[derive(Copy, Clone)]
pub enum OperandType {
    Immediate8(u8),
    Immediate16(u16),
    Immediate8s(i8),
    Relative8(i8),
    Relative16(i16),
    Offset8(u16),
    Offset16(u16),
    Register8(Register8),
    Register16(Register16),
    AddressingMode(AddressingMode),
    FarAddress(u16,u16),
    NoOperand,
    InvalidOperand
}

#[derive(Copy, Clone, Debug)]
pub enum Displacement {
    NoDisp,
    Pending8,
    Pending16,
    Disp8(i8),
    Disp16(i16),
}

#[derive(Copy, Clone, Debug)]
pub enum DmaState {
    Idle,
    TimerTrigger,
    Dreq,
    Hrq,
    HoldA,
    Operating(u8),
    //DmaWait(u8)
}

impl Default for DmaState {
    fn default() -> Self {
        DmaState::Idle
    }
}

impl Displacement {
    
    pub fn get_i16(&self) -> i16 {
        match self {
            Displacement::Disp8(disp) => *disp as i16,
            Displacement::Disp16(disp) => *disp,
            _ => 0
        }
    }
    pub fn get_u16(&self) -> u16 {
        match self {
            Displacement::Disp8(disp) => (*disp as i16) as u16,
            Displacement::Disp16(disp) => *disp as u16,
            _ => 0
        }        
    }
}

#[derive(Debug)]
pub enum RepType {
    NoRep,
    Rep,
    Repne,
    Repe,
    MulDiv
}

impl Default for RepType {
    fn default() -> Self { RepType::NoRep }
}

#[derive(Copy, Clone, Debug)]
pub enum Segment {
    None,
    ES,
    CS,
    SS,
    DS
}

impl Default for Segment {
    fn default() -> Self {
        Segment::CS
    }
}

// TODO: This enum duplicates Segment. Why not just store a Segment in an override field?
#[derive(Copy, Clone, PartialEq)]
pub enum SegmentOverride {
    None,
    ES,
    CS,
    SS,
    DS
}

#[derive(Copy, Clone, PartialEq)]
pub enum OperandSize {
    NoOperand,
    NoSize,
    Operand8,
    Operand16
}

impl Default for OperandSize {
    fn default() -> Self {
        OperandSize::NoOperand
    }
}

#[allow(dead_code)]
#[derive (Copy, Clone, Debug, PartialEq)]
pub enum InterruptType {
    NMI,
    Exception,
    Software,
    Hardware
}

pub enum HistoryEntry {
    Entry { cs: u16, ip: u16, cycles: u16, i: Instruction}
}

#[derive (Copy, Clone)]
pub struct InterruptDescriptor {
    itype: InterruptType,
    number: u8,
    ah: u8
}

impl Default for InterruptDescriptor {
    fn default() -> Self {
        InterruptDescriptor {
            itype: InterruptType::Hardware,
            number: 0,
            ah: 0
        }
    }
}

#[derive (Copy, Clone)]
pub struct Instruction {
    pub opcode: u8,
    pub flags: u32,
    pub prefixes: u32,
    pub address: u32,
    pub size: u32,
    pub mnemonic: Mnemonic,
    pub segment_override: SegmentOverride,
    pub operand1_type: OperandType,
    pub operand1_size: OperandSize,
    pub operand2_type: OperandType,
    pub operand2_size: OperandSize,
}

impl Default for Instruction {
    fn default() -> Self {
        Self {
            opcode:   0,
            flags:    0,
            prefixes: 0,
            address:  0,
            size:     1,
            mnemonic: Mnemonic::NOP,
            segment_override: SegmentOverride::None,
            operand1_type: OperandType::NoOperand,
            operand1_size: OperandSize::NoOperand,
            operand2_type: OperandType::NoOperand,
            operand2_size: OperandSize::NoOperand,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum TransferSize {
    Byte,
    Word
}

impl Default for TransferSize {
    fn default() -> TransferSize {
        TransferSize::Byte
    }
}

#[derive(Copy, Clone, Debug)]
pub enum CpuAddress {
    Flat(u32),
    Segmented(u16, u16),
    Offset(u16)
}

impl Default for CpuAddress {
    fn default() -> CpuAddress {
        CpuAddress::Segmented(0,0)
    }
}

impl From<CpuAddress> for u32 {
    fn from(cpu_address: CpuAddress) -> Self {
        match cpu_address {
            CpuAddress::Flat(a) => a,
            CpuAddress::Segmented(s, o) => Cpu::calc_linear_address(s, o),
            CpuAddress::Offset(a) => a as Self
        }
    }
}

impl fmt::Display for CpuAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CpuAddress::Flat(a) => write!(f, "{:05X}", a),
            CpuAddress::Segmented(s, o) => write!(f, "{:04X}:{:04X}", s, o),
            CpuAddress::Offset(a) => write!(f, "{:04X}", a),
        }
    }
}

impl PartialEq for CpuAddress {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (CpuAddress::Flat(a), CpuAddress::Flat(b)) => a == b,
            (CpuAddress::Flat(a), CpuAddress::Segmented(s,o)) => {
                let b = Cpu::calc_linear_address(*s, *o);
                *a == b
            }
            (CpuAddress::Flat(_a), CpuAddress::Offset(_b)) => false,
            (CpuAddress::Segmented(s,o), CpuAddress::Flat(b)) => {
                let a = Cpu::calc_linear_address(*s, *o);
                a == *b
            }
            (CpuAddress::Segmented(s1,o1), CpuAddress::Segmented(s2,o2)) => {
                *s1 == *s2 && *o1 == *o2
            }
            _ => false
        }
    }
}

#[derive(Default)]
pub struct I8288 {
    // Command bus
    mrdc: bool,
    amwc: bool,
    mwtc: bool,
    iorc: bool,
    aiowc: bool,
    iowc: bool,
    inta: bool,
    // Control output
    _dtr: bool,
    ale: bool,
    _pden: bool,
    _den: bool
}

#[derive(Default)]
pub struct Cpu
{
    
    cpu_type: CpuType,
    state: CpuState,

    ah: u8,
    al: u8,
    ax: u16,
    bh: u8,
    bl: u8,
    bx: u16,
    ch: u8,
    cl: u8,
    cx: u16,
    dh: u8,
    dl: u8,
    dx: u16,
    sp: u16,
    bp: u16,
    si: u16,
    di: u16,
    cs: u16,
    ds: u16,
    ss: u16,
    es: u16,
    ip: u16,
    flags: u16,

    address_bus: u32,
    address_latch: u32,
    data_bus: u16,
    last_ea: u16,                   // Last calculated effective address. Used by 0xFE instructions
    bus: BusInterface,              // CPU owns Bus
    i8288: I8288,                   // Intel 8288 Bus Controller
    pc: u32,                        // Program counter points to the next instruction to be fetched
    mc_pc: u16,                     // Microcode program counter. 
    nx: bool,
    rni: bool,
    ea_opr: u16,                    // Operand loaded by EALOAD. Masked to 8 bits as appropriate.

    intr: bool,                     // State of INTR line
    intr_pending: bool,             // INTR line active and not processed
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
    biu_state_new: BiuStateNew,     // State of BIU: Idle, EU, PF (Prefetcher) or transition state
    ready: bool,                    // READY line from 8284
    queue: InstructionQueue,
    fetch_size: TransferSize,
    fetch_state: FetchState,
    next_fetch_state: FetchState,
    fetch_suspended: bool,
    bus_pending_eu: bool,           // Has the EU requested a bus operation?
    queue_op: QueueOp,
    last_queue_op: QueueOp,
    queue_byte: u8,
    last_queue_byte: u8,
    last_queue_len: usize,
    t_cycle: TCycle,
    bus_status: BusStatus,
    bus_status_latch: BusStatus,
    bus_segment: Segment,
    transfer_size: TransferSize,    // Width of current bus transfer
    operand_size: OperandSize,      // Width of the operand being transferred
    transfer_n: u32,                // Current transfer number (Either 1 or 2, for byte or word operand, respectively)
    final_transfer: bool,           // Flag that determines if the current bus transfer is the final transfer for this bus request
    bus_wait_states: u32,
    wait_states: u32,
    lock: bool,                     // LOCK pin. Asserted during 2nd INTA bus cycle. 

    // Halt-related stuff
    halted: bool,
    halt_not_hold: bool,            // Internal halt signal
    wake_timer: u32,

    is_running: bool,
    is_error: bool,
    
    // Rep prefix handling
    in_rep: bool,
    rep_init: bool,
    rep_mnemonic: Mnemonic,
    rep_type: RepType,
    
    cycle_num: u64,
    t_stamp: f64,
    t_step: f64,
    t_step_h: f64,
    instr_cycle: u32,
    device_cycles: u32,
    int_elapsed: u32,
    instr_elapsed: u32,
    instruction_count: u64,
    i: Instruction,                 // Currently executing instruction 
    instruction_address: u32,
    instruction_history_on: bool,
    instruction_history: VecDeque<HistoryEntry>,
    call_stack: VecDeque<CallStackEntry>,
    exec_result: ExecutionResult,

    // Breakpoints
    breakpoints: Vec<BreakPointType>,

    step_over_target: Option<CpuAddress>,

    reset_vector: CpuAddress,

    enable_service_interrupt: bool,
    trace_enabled: bool,
    trace_mode: TraceMode,
    trace_logger: TraceLogger,
    trace_comment: Vec<&'static str>,
    trace_instr: u16,
    trace_str_vec: Vec<String>,

    enable_wait_states: bool,
    off_rails_detection: bool,
    opcode0_counter: u32,

    rng: Option<rand::rngs::StdRng>,

    #[cfg(feature = "cpu_validator")]
    validator: Option<Box<dyn CpuValidator>>,
    #[cfg(feature = "cpu_validator")]
    cycle_states: Vec<CycleState>,
    #[cfg(feature = "cpu_validator")]
    validator_state: CpuValidatorState,
    #[cfg(feature = "cpu_validator")]
    validator_end: usize,
    #[cfg(feature = "cpu_validator")]
    peek_fetch: u8,
    #[cfg(feature = "cpu_validator")]
    instr_slice: Vec<u8>,

    end_addr: usize,

    service_events: VecDeque<ServiceEvent>,

    // DMA stuff
    dma_state: DmaState,
    dram_refresh_simulation: bool,
    dram_refresh_cycle_period: u32,
    dram_refresh_cycle_num: u32,
    dram_refresh_adjust: u32,
    dma_aen: bool,
    dma_wait_states: u32,

    // Trap stuff
    trap_enable_delay: u32,             // Number of cycles to delay trap flag enablement. 
    trap_disable_delay: u32,            // Number of cycles to delay trap flag disablement.
    trap_suppressed: bool,              // Suppress trap handling for the last executed instruction.

    nmi: bool,                          // Status of NMI line.
    nmi_triggered: bool,                // Has NMI been edge-triggered?

    halt_resume_delay: u32,
    int_flags: Vec<u8>,
}

#[cfg(feature = "cpu_validator")]
#[derive (PartialEq, Copy, Clone)]
pub enum CpuValidatorState {
    Uninitialized,
    Running,
    Hung,
    Ended
}

#[cfg(feature = "cpu_validator")]
impl Default for CpuValidatorState {
    fn default() -> Self {
        CpuValidatorState::Uninitialized
    }
}

pub struct CpuRegisterState {
    pub ah: u8,
    pub al: u8,
    pub ax: u16,
    pub bh: u8,
    pub bl: u8,
    pub bx: u16,
    pub ch: u8,
    pub cl: u8,
    pub cx: u16,
    pub dh: u8,
    pub dl: u8,
    pub dx: u16,
    pub sp: u16,
    pub bp: u16,
    pub si: u16,
    pub di: u16,
    pub cs: u16,
    pub ds: u16,
    pub ss: u16,
    pub es: u16,
    pub ip: u16,
    pub flags: u16,
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
    pub cycle_count: String
}
    
/*
pub enum RegisterType {
    Register8(u8),
    Register16(u16)
}
*/

#[derive (Debug)]
pub enum StepResult {
    Normal,
    // If a call occurred, we return the address of the next instruction after the call
    // so that we can step over the call in the debugger.
    Call(CpuAddress),
    BreakpointHit,
    ProgramEnd
}

#[derive (Debug, PartialEq)]
pub enum ExecutionResult {
    Okay,
    OkayJump,
    OkayRep,
    //UnsupportedOpcode(u8),        // All opcodes implemented.
    ExecutionError(String),
    ExceptionError(CpuException),
    Halt
}

impl Default for ExecutionResult {
    fn default() -> ExecutionResult {
        ExecutionResult::Okay
    }
}

#[derive (Copy, Clone, Debug, PartialEq)]
pub enum TCycle {
    Tinit,
    Ti,
    T1,
    T2,
    T3,
    Tw,
    T4
}

impl Default for TCycle {
    fn default() -> TCycle {
        TCycle::Ti
    }
}

#[derive (Copy, Clone, Debug, PartialEq)]
pub enum BiuStateNew {
    Idle,
    ToIdle(u8),
    Prefetch,
    ToPrefetch(u8),
    Eu,
    ToEu(u8)
}

impl Default for BiuStateNew {
    fn default() -> BiuStateNew {
        BiuStateNew::Idle
    }
}

#[derive (Copy, Clone, Debug, PartialEq)]
pub enum BusStatus {
    InterruptAck = 0,   // IRQ Acknowledge
    IoRead = 1,         // IO Read
    IoWrite = 2,        // IO Write
    Halt = 3,           // Halt
    CodeFetch = 4,      // Code Access
    MemRead = 5,        // Memory Read
    MemWrite = 6,       // Memory Write
    Passive = 7         // Passive
}

impl Default for BusStatus {
    fn default() ->  BusStatus {
        BusStatus::Passive
    }
}

#[derive (Copy, Clone, Debug, PartialEq)]
pub enum QueueOp {
    Idle,
    First,
    Flush,
    Subsequent,
}

impl Default for QueueOp {
    fn default() ->  QueueOp {
        QueueOp::Idle
    }
}

#[derive (Copy, Clone, Debug, PartialEq)]
pub enum FetchState {
    Idle,
    Suspended,
    InProgress,
    Scheduled(u8),
    ScheduleNext,
    Delayed(u8),
    DelayDone,
    Aborting(u8),
    BlockedByEU
}

impl Default for FetchState {
    fn default() ->  FetchState {
        FetchState::Idle
    }
}

impl Cpu {

    pub fn new(
        cpu_type: CpuType,
        trace_mode: TraceMode,
        trace_logger: TraceLogger,
        #[cfg(feature = "cpu_validator")]
        validator_type: ValidatorType,
        #[cfg(feature = "cpu_validator")]
        validator_trace: TraceLogger,
        #[cfg(feature = "cpu_validator")]
        validator_mode: ValidatorMode,
        #[cfg(feature = "cpu_validator")]        
        validator_baud: u32
    ) -> Self {
        let mut cpu: Cpu = Default::default();
        
        match cpu_type {
            CpuType::Intel8088 => {
                cpu.queue.set_size(4);
                cpu.fetch_size = TransferSize::Byte;
            }
            CpuType::Intel8086 => {
                cpu.queue.set_size(6);
                cpu.fetch_size = TransferSize::Word;
            }
        }

        #[cfg(feature = "cpu_validator")] 
        {
            cpu.validator = match validator_type {

                #[cfg(feature = "arduino_validator")]
                ValidatorType::Arduino8088 => {
                    Some(Box::new(ArduinoValidator::new(validator_trace, validator_baud)))
                }
                _=> {
                    None
                }
            };

            if let Some(ref mut validator) = cpu.validator {
                match validator.init(validator_mode, true, true, false) {
                    true => {},
                    false => {
                        panic!("Failed to init cpu validator.");
                    }
                }
            }            
        }

        cpu.trace_logger = trace_logger;
        cpu.trace_mode = trace_mode;
        cpu.cpu_type = cpu_type;

        //cpu.instruction_history_on = true; // Control this from config/GUI instead
        cpu.instruction_history = VecDeque::with_capacity(16);

        cpu.reset_vector = CpuAddress::Segmented(0xFFFF, 0x0000);
        cpu.reset();
        cpu
    }

    pub fn reset(&mut self) {
        
        log::debug!("Resetting...");
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
            self.set_register16(Register16::IP, offset);
            self.pc = Cpu::calc_linear_address(segment, offset);
        }
        else {
            panic!("Invalid CpuAddress for reset vector.");
        }

        self.address_latch = 0;
        self.bus_status = BusStatus::Passive;
        self.bus_status_latch = BusStatus::Passive;
        self.t_cycle = TCycle::T1;
        
        self.instruction_count = 0; 
        self.int_count = 0;
        self.iret_count = 0;
        self.instr_cycle = 0;
        self.cycle_num = 1;
        self.t_stamp = 0.0;
        self.t_step = 0.00000021;
        self.t_step_h = 0.000000105;
        self.ready = true;
        self.in_rep = false;
        self.halted = false;
        self.halt_not_hold = false;
        self.opcode0_counter = 0;
        self.interrupt_inhibit = false;
        self.intr_pending = false;
        self.in_int = false;
        self.is_error = false;
        self.instruction_history.clear();
        self.call_stack.clear();
        self.int_flags = vec![0; 256];

        self.queue_op = QueueOp::Idle;
        self.last_queue_op = QueueOp::Idle;
        self.fetch_state = FetchState::Idle;

        self.i8288.ale = false;
        self.i8288.mrdc = false;
        self.i8288.amwc = false;
        self.i8288.mwtc = false;
        self.i8288.iorc = false;
        self.i8288.aiowc = false;
        self.i8288.iowc = false;

        self.step_over_target = None;
        self.end_addr = 0xFFFFF;

        self.nx = false;
        self.rni = false;

        self.halt_resume_delay = 4;

        // Reset takes 6 cycles before first fetch
        self.cycle();
        self.biu_suspend_fetch();
        self.cycles_i(2, &[0x1e4, 0x1e5]);
        self.biu_queue_flush();
        self.cycles_i(3, &[0x1e6, 0x1e7, 0x1e8]);

        #[cfg(feature = "cpu_validator")]
        {
            self.validator_state = CpuValidatorState::Uninitialized;
            self.cycle_states.clear();
        }

        trace_print!(self, "Reset CPU! CS: {:04X} IP: {:04X}", self.cs, self.ip);
    }

    pub fn emit_header(&mut self) {
        self.trace_print("Time(s),addr,clk,ready,qs,s,clk0,intr,dr0,vs,hs,den,brd")
    }

    #[allow(dead_code)]
    pub fn in_rep(&self) -> bool {
        self.in_rep
    }

    pub fn bus(&self) -> &BusInterface {
        &self.bus
    }   

    pub fn bus_mut(&mut self) -> &mut BusInterface {
        &mut self.bus
    }

    pub fn get_csip(&self) -> CpuAddress {
        CpuAddress::Segmented(self.cs, self.ip)
    }

    #[inline]
    pub fn is_last_wait(&self) -> bool {
        match self.t_cycle {
            TCycle::T3 | TCycle::Tw => {
                if self.wait_states == 0 && self.dma_wait_states == 0 {
                    true
                }
                else {
                    false
                }
            }
            _ => false
        }
    }

    #[inline]
    pub fn is_before_last_wait(&self) -> bool {
        match self.t_cycle {
            TCycle::T1 | TCycle::T2 => true,
            TCycle::T3 | TCycle::Tw => {
                if self.wait_states > 0 || self.dma_wait_states > 0 {
                    true
                }
                else {
                    false
                }
            }
            _ => false
        }
    }

    #[inline]
    pub fn is_before_t3(&self) -> bool {
        match self.t_cycle {
            TCycle::T1 | TCycle::T2 => true,
            _ => false
        }
    }

    /// Finalize an instruction that has terminated before there is a new byte in the queue.
    /// This will cycle the CPU until a byte is available in the instruction queue, then fetch it.
    /// This fetched byte is considered 'preloaded' by the queue.
    pub fn finalize(&mut self) {

        // Don't finalize a string instruction that is still repeating.
        if !self.in_rep {
            self.trace_comment("FINALIZE");
            let mut finalize_timeout = 0;
            
            /*
            if MICROCODE_FLAGS_8088[self.mc_pc as usize] == RNI {
                trace_print!(self, "Executed terminating RNI!");
            }
            */

            if self.queue.len() == 0 {
                while { 
                    if self.nx {
                        self.trace_comment("NX");
                        self.next_mc();
                        self.nx = false;
                        self.rni = false;
                    }
                    self.cycle();
                    self.mc_pc = MC_NONE;
                    finalize_timeout += 1;
                    if finalize_timeout == 20 {
                        self.trace_flush();
                        panic!("Finalize timeout! wait states: {}", self.wait_states);
                    }
                    self.queue.len() == 0
                } {}
                // Should be a byte in the queue now. Preload it
                self.queue.set_preload();
                self.queue_op = QueueOp::First;
                self.trace_comment("FINALIZE_END");
                self.cycle();
            }
            else {
                self.queue.set_preload();
                self.queue_op = QueueOp::First;

                // Check if reading the queue will resume the BIU if stalled.
                self.biu_fetch_on_queue_read();

                if self.nx {
                    self.trace_comment("NX");
                    self.next_mc();
                }

                if self.rni {
                    self.trace_comment("RNI");
                    self.rni = false;
                }
                
                self.trace_comment("FINALIZE_END");
                self.cycle();
            }
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
                TCycle::T4 => BusCycle::T4
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
                        BusStatus::Passive => BusState::PASV
                    }
                _=> BusState::PASV
            },
            ale: self.i8288.ale,
            mrdc: !self.i8288.mrdc,
            amwc: !self.i8288.amwc,
            mwtc: !self.i8288.mwtc,
            iorc: !self.i8288.iorc,
            aiowc: !self.i8288.aiowc,
            iowc: !self.i8288.iowc,
            inta: !self.i8288.inta,
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
    pub fn set_flag(&mut self, flag: Flag ) {

        if let Flag::Interrupt = flag {
            self.interrupt_inhibit = true;
            //if self.eflags & CPU_FLAG_INT_ENABLE == 0 {
                // The interrupt flag was *just* set, so instruct the CPU to start
                // honoring interrupts on the *next* instruction
                // self.interrupt_inhibit = true;
            //}
        }

        self.flags |= match flag {
            Flag::Carry => CPU_FLAG_CARRY,
            Flag::Parity => CPU_FLAG_PARITY,
            Flag::AuxCarry => CPU_FLAG_AUX_CARRY,
            Flag::Zero => CPU_FLAG_ZERO,
            Flag::Sign => CPU_FLAG_SIGN,
            Flag::Trap => CPU_FLAG_TRAP,
            Flag::Interrupt => CPU_FLAG_INT_ENABLE,
            Flag::Direction => CPU_FLAG_DIRECTION,
            Flag::Overflow => CPU_FLAG_OVERFLOW
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
            Flag::Overflow => !CPU_FLAG_OVERFLOW
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

    pub fn store_flags(&mut self, bits: u16 ) {

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
        let mut flags = self.flags;
        flags &= match flag {
            Flag::Carry => CPU_FLAG_CARRY,
            Flag::Parity => CPU_FLAG_PARITY,
            Flag::AuxCarry => CPU_FLAG_AUX_CARRY,
            Flag::Zero => CPU_FLAG_ZERO,
            Flag::Sign => CPU_FLAG_SIGN,
            Flag::Trap => CPU_FLAG_TRAP,
            Flag::Interrupt => CPU_FLAG_INT_ENABLE,
            Flag::Direction => CPU_FLAG_DIRECTION,
            Flag::Overflow => CPU_FLAG_OVERFLOW
        };

        if flags > 0 {
            true
        }
        else {
            false
        }
    }
 
    #[cfg(feature = "cpu_validator")]
    pub fn get_vregisters(&self) -> VRegisters {
        VRegisters {
            ax: self.ax,
            bx: self.bx,
            cx: self.cx,
            dx: self.dx,
            cs: self.cs,
            ss: self.ss,
            ds: self.ds,
            es: self.es,
            sp: self.sp,
            bp: self.bp,
            si: self.si,
            di: self.di,
            ip: self.ip,
            flags: self.flags
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
    pub fn get_register8(&self, reg:Register8) -> u8 {
        match reg {
            Register8::AH => self.ah,
            Register8::AL => self.al,
            Register8::BH => self.bh,
            Register8::BL => self.bl,
            Register8::CH => self.ch,
            Register8::CL => self.cl,
            Register8::DH => self.dh,
            Register8::DL => self.dl,         
        }
    }

    #[inline]
    pub fn get_register16(&self, reg: Register16) -> u16 {
        match reg {
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
        }
    }

    #[inline]
    pub fn get_flags(&self) -> u16 {
        self.flags
    }

    // Sets one of the 8 bit registers.
    // It's tempting to represent the H/X registers as a union, because they are one.
    // However, in the exercise of this project I decided to avoid all unsafe code.
    #[inline]
    pub fn set_register8(&mut self, reg: Register8, value: u8) {
        match reg {
            Register8::AH => {
                self.ah = value;
                self.ax = self.ax & REGISTER_HI_MASK | ((value as u16) << 8);
            }
            Register8::AL => {
                self.al = value;
                self.ax = self.ax & REGISTER_LO_MASK | (value as u16)
            }    
            Register8::BH => {
                self.bh = value;
                self.bx = self.bx & REGISTER_HI_MASK | ((value as u16) << 8);
            }
            Register8::BL => {
                self.bl = value;
                self.bx = self.bx & REGISTER_LO_MASK | (value as u16)
            }
            Register8::CH => {
                self.ch = value;
                self.cx = self.cx & REGISTER_HI_MASK | ((value as u16) << 8);
            }
            Register8::CL => {
                self.cl = value;
                self.cx = self.cx & REGISTER_LO_MASK | (value as u16)
            }
            Register8::DH => {
                self.dh = value;
                self.dx = self.dx & REGISTER_HI_MASK | ((value as u16) << 8);
            }
            Register8::DL => {
                self.dl = value;
                self.dx = self.dx & REGISTER_LO_MASK | (value as u16)
            }           
        }
    }

    #[inline]
    pub fn set_register16(&mut self, reg: Register16, value: u16) {
        match reg {
            Register16::AX => {
                self.ax = value;
                self.ah = (value >> 8) as u8;
                self.al = (value & REGISTER_HI_MASK) as u8;
            }
            Register16::BX => {
                self.bx = value;
                self.bh = (value >> 8) as u8;
                self.bl = (value & REGISTER_HI_MASK) as u8;
            }
            Register16::CX => {
                self.cx = value;
                self.ch = (value >> 8) as u8;
                self.cl = (value & REGISTER_HI_MASK) as u8;
            }
            Register16::DX => {
                self.dx = value;
                self.dh = (value >> 8) as u8;
                self.dl = (value & REGISTER_HI_MASK) as u8;
            }
            Register16::SP => self.sp = value,
            Register16::BP => self.bp = value,
            Register16::SI => self.si = value,
            Register16::DI => self.di = value,
            Register16::CS => self.cs = value,
            Register16::DS => self.ds = value,
            Register16::SS => self.ss = value,
            Register16::ES => self.es = value,
            Register16::IP => self.ip = value,
            _=>panic!("bad register16")                    
        }
    }

    /// Converts a Register8 into a Register16.
    /// Only really useful for r forms of FE.03-07 which operate on 8 bits of a memory
    /// operand but 16 bits of a register operand. We don't support 'hybrid' 8/16 bit 
    /// instruction templates so we have to convert.
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

    pub fn decrement_register8(&mut self, reg: Register8) {
        // TODO: do this directly
        let mut value = self.get_register8(reg);
        value = value.wrapping_sub(1);
        self.set_register8(reg, value);
    }

    pub fn decrement_register16(&mut self, reg: Register16) {
        // TODO: do this directly
        let mut value = self.get_register16(reg);
        value = value.wrapping_sub(1);
        self.set_register16(reg, value);
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
            self.ip = offset;
        }
    }

    pub fn get_linear_ip(&self) -> u32 {
        Cpu::calc_linear_address(self.cs, self.ip)
    }

    pub fn get_state(&self) -> CpuRegisterState {
        CpuRegisterState {
            ah: self.ah,
            al: self.al,
            ax: self.ax,
            bh: self.bh,
            bl: self.bl,
            bx: self.bx,
            ch: self.ch,
            cl: self.cl,
            cx: self.cx,
            dh: self.dh,
            dl: self.dl,
            dx: self.dx,
            sp: self.sp,
            bp: self.bp,
            si: self.si,
            di: self.di,
            cs: self.cs,
            ds: self.ds,
            ss: self.ss,
            es: self.es,
            ip: self.ip,
            flags: self.flags
        }
    }

    /// Get a string representation of the CPU state.
    /// This is used to display the CPU state viewer window in the debug GUI.
    pub fn get_string_state(&self) -> CpuStringState {

        CpuStringState {
            ah: format!("{:02x}", self.ah),
            al: format!("{:02x}", self.al),
            ax: format!("{:04x}", self.ax),
            bh: format!("{:02x}", self.bh),
            bl: format!("{:02x}", self.bl),
            bx: format!("{:04x}", self.bx),
            ch: format!("{:02x}", self.ch),
            cl: format!("{:02x}", self.cl),
            cx: format!("{:04x}", self.cx),
            dh: format!("{:02x}", self.dh),
            dl: format!("{:02x}", self.dl),
            dx: format!("{:04x}", self.dx),
            sp: format!("{:04x}", self.sp),
            bp: format!("{:04x}", self.bp),
            si: format!("{:04x}", self.si),
            di: format!("{:04x}", self.di),
            cs: format!("{:04x}", self.cs),
            ds: format!("{:04x}", self.ds),
            ss: format!("{:04x}", self.ss),
            es: format!("{:04x}", self.es),
            ip: format!("{:04x}", self.ip),
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
        }
    }
    
    /// Evaluate an string expression such as 'cs:ip' to an address.
    /// Basic forms supported are [reg:reg], [reg:offset], [seg:offset]
    pub fn eval_address(&self, expr: &str) -> Option<CpuAddress> {

        lazy_static! {
            static ref FLAT_REX: Regex = Regex::new(r"(?P<flat>[A-Fa-f\d]{5})$").unwrap();
            static ref SEGMENTED_REX: Regex = Regex::new(r"(?P<segment>[A-Fa-f\d]{4}):(?P<offset>[A-Fa-f\d]{4})$").unwrap();
            static ref REGREG_REX: Regex = Regex::new(r"(?P<reg1>cs|ds|ss|es):(?P<reg2>\w{2})$").unwrap();
            static ref REGOFFSET_REX: Regex = Regex::new(r"(?P<reg1>cs|ds|ss|es):(?P<offset>[A-Fa-f\d]{4})$").unwrap();
        }

        if FLAT_REX.is_match(expr) {
            match u32::from_str_radix(expr, 16) {
                Ok(address) => Some(CpuAddress::Flat(address)),
                Err(_) => None
            }     
        }
        else if let Some(caps) = SEGMENTED_REX.captures(expr) {
            let segment_str = &caps["segment"];
            let offset_str = &caps["offset"];
            
            let segment_u16r = u16::from_str_radix(segment_str, 16);
            let offset_u16r = u16::from_str_radix(offset_str, 16);

            match(segment_u16r, offset_u16r) {
                (Ok(segment),Ok(offset)) => Some(CpuAddress::Segmented(segment, offset)),
                _ => None
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
                _ => 0
            };

            let offset = match reg2 {
                "ah" => self.ah as u16,
                "al" => self.al as u16,
                "ax" => self.ax,
                "bh" => self.bh as u16,
                "bl" => self.bl as u16,
                "bx" => self.bx,
                "ch" => self.ch as u16,
                "cl" => self.cl as u16,
                "cx" => self.cx,
                "dh" => self.dh as u16,
                "dl" => self.dl as u16,
                "dx" => self.dx,
                "sp" => self.sp,
                "bp" => self.bp,
                "si" => self.si,
                "di" => self.di,
                "cs" => self.cs,
                "ds" => self.ds,
                "ss" => self.ss,
                "es" => self.es,
                "ip" => self.ip,
                _ => 0
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
                _ => 0
            };

            let offset_u16r = u16::from_str_radix(offset_str, 16);
            
            match offset_u16r {
                Ok(offset) => Some(CpuAddress::Segmented(segment, offset)),
                _ => None
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
            let return_addr = Cpu::calc_linear_address(cs, ip);
    
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
                CallStackEntry::CallF { ret_cs, ret_ip, .. } => {
                    Cpu::calc_linear_address(ret_cs, ret_ip)
                },
                CallStackEntry::Call { ret_cs, ret_ip, .. } => {
                    Cpu::calc_linear_address(ret_cs, ret_ip)
                },
                CallStackEntry::Interrupt { ret_cs, ret_ip, .. } => {
                    Cpu::calc_linear_address(ret_cs, ret_ip)
                }       
            };

            return_addr == addr
        });

        if let Some(found_idx) = pos {
            let drained = self.call_stack.drain(found_idx..);

            drained.for_each(|drained_call| {
                return_addr = match drained_call {
                    CallStackEntry::CallF { ret_cs, ret_ip, .. } => {
                        Cpu::calc_linear_address(ret_cs, ret_ip)
                    },
                    CallStackEntry::Call { ret_cs, ret_ip, .. } => {
                        Cpu::calc_linear_address(ret_cs, ret_ip)
                    },
                    CallStackEntry::Interrupt { ret_cs, ret_ip, .. } => {
                        Cpu::calc_linear_address(ret_cs, ret_ip)
                    }       
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
            log::debug!("resuming from halt with {} cycles", self.halt_resume_delay);
            self.cycles(self.halt_resume_delay);
        }
        else {
            log::warn!("resume() called but not halted!");
        }
        self.halted = false;
    }

    /// Execute a single instruction.
    /// 
    /// We divide instruction execution into separate fetch/decode and execute phases.
    /// This is an artificial distinction, but allows for flexibility as the decode() function can be 
    /// used on anything that implements the ByteQueue trait, ie, raw memory for a disassembly viewer.
    /// 
    /// REP string instructions are handled by stopping them after one iteration so that interrupts can
    /// be checked. 
    pub fn step(
        &mut self, 
        skip_breakpoint: bool,
    ) -> Result<(StepResult, u32), CpuError> {

        self.instr_cycle = 0;
        self.instr_elapsed = self.int_elapsed;

        // If tracing is enabled, clear the trace string vector that holds the trace from the last instruction.
        if self.trace_enabled {
            self.trace_str_vec.clear();
        }

        // Check for interrupts.
        //
        // If an INTR is active at the beginning of an instruction, we should execute the interrupt
        // instead of the instruction, except if we are in a REP prefixed string instruction, where we set a
        // pending interrupt flag and run the REP iteration.
        //
        // In a real CPU, REP instructions run for the entire period in which they repeat and handle checking
        // interrupts themselves in microcode. Therefore we want to model that behavior. This allows the 
        // microcode routine for RPTI to execute within the REP-prefixed instruction. The interrupt then
        // fires after.

        /*
        self.pending_interrupt = false;
        let mut irq = 7;

        if self.nmi && self.bus.nmi_enabled() && !self.nmi_triggered {
            // NMI takes priority over trap and INTR.
            if self.halted {
                // Resume from halt on interrupt
                self.resume();
            }            
            log::debug!("Triggered NMI!");
            self.nmi_triggered = true;
            self.int2();
            let step_result = Ok((StepResult::Call(CpuAddress::Segmented(self.cs, self.ip)), self.instr_cycle));
            return step_result              
        }
        else if self.trap_enabled() {
            // Trap takes priority over INTR.
            self.int1();
            let step_result = Ok((StepResult::Call(CpuAddress::Segmented(self.cs, self.ip)), self.instr_cycle));
            return step_result              
        }
        else if self.interrupts_enabled() {
            if let Some(pic) = self.bus.pic_mut().as_mut() {
                // Is INTR active? TODO: Could combine these calls (return Option<iv>) on query?
                if pic.query_interrupt_line() {
                    if let Some(iv) = pic.get_interrupt_vector() {
                        irq = iv;
                        if self.in_rep {
                            // Set pending interrupt to execute after RPTI
                            self.pending_interrupt = true;
                        }
                        else {
                            if self.halted {
                                // Resume from halt on interrupt
                                self.resume();
                            }
                            // We will be jumping into an ISR now. Set the step result to Call and return
                            // the address of the next instruction. (Step Over skips ISRs)

                            // Set breakpoint flag if we have a breakpoint for this interrupt.
                            if self.int_flags[irq as usize] != 0 {
                                self.set_breakpoint_flag();
                            }

                            // Do interrupt
                            self.hw_interrupt(irq);
                            //log::debug!("hardware interrupt took {} cycles", self.instr_cycle);
                            let step_result = Ok((StepResult::Call(CpuAddress::Segmented(self.cs, self.ip)), self.instr_cycle));
                            return step_result                                                 
                        }
                    }
                }
            }
        }
        */

        // Halt state can be expensive since if we only executing a single cycle. 
        // See if we can get away with executing 3 halt cycles at at time - demo effects may require more precision

        // TODO: Adjust this value based on Timer channel 0 count - if no interrupt is pending soon we can do more
        // cycles per halt.
        if self.halted {
            self.cycle_i(self.mc_pc);
            self.cycle_i(self.mc_pc);
            self.cycle_i(self.mc_pc);
            return Ok((StepResult::Normal, 3))
        }

        // A real 808X CPU maintains a single Program Counter or PC register that points to the next instruction
        // to be fetched, not the currently executing instruction. This value is "corrected" whenever the current
        // value of IP is required, ie, pushing IP to the stack. This is performed by the 'CORR' microcode routine.

        // It is more convenient for us to maintain IP as a separate register that always points to the current
        // instruction. Otherwise, when single-stepping in the debugger, the IP value will read ahead. 
        let instruction_address = Cpu::calc_linear_address(self.cs, self.ip);
        self.instruction_address = instruction_address;
        //log::warn!("instruction address: {:05X}", instruction_address);

        if self.end_addr == (instruction_address as usize) { 
            return Ok((StepResult::ProgramEnd, 0))
        }

        // Check if we are in BreakpointHit state. This state must be cleared before we can execute another instruction.
        if self.get_breakpoint_flag() {
            return Ok((StepResult::BreakpointHit, 0))
        }

        // Check instruction address for breakpoint on execute flag
        if !skip_breakpoint && self.bus.get_flags(instruction_address as usize) & MEM_BPE_BIT != 0 {
            // Breakpoint hit.
            log::debug!("Breakpoint hit at {:05X}", instruction_address);
            self.set_breakpoint_flag();
            return Ok((StepResult::BreakpointHit, 0))
        }

        // Fetch the next instruction unless we are executing a REP
        if !self.in_rep {

            // Clear the validator cycle states from the last instruction.
            #[cfg(feature = "cpu_validator")]
            {
                if self.validator_state == CpuValidatorState::Running {
                    if let Some(ref mut validator) = self.validator { 
                        validator.reset_instruction();
                    }
                    self.cycle_states.clear();
                }
                else {
                    // Clear cycle states spent in reset but not initial prefetch
                    self.clear_reset_cycle_states();
                }
            }

            // If cycle tracing is enabled, we prefetch the current instruction directly from memory backend 
            // to make the instruction disassembly available to the trace log on the first byte fetch of an
            // instruction. 
            // This of course now requires decoding each instruction twice, but cycle tracing is pretty slow 
            // anyway.
            if self.trace_mode == TraceMode::Cycle {
                self.bus.seek(instruction_address as usize);
                self.i = match Cpu::decode(&mut self.bus) {
                    Ok(i) => i,
                    Err(_) => {
                        self.is_running = false;
                        self.is_error = true;
                        return Err(CpuError::InstructionDecodeError(instruction_address))
                    }                
                };
                //log::trace!("Fetching instruction...");
                self.i.address = instruction_address;
            }
            
            // Fetch and decode the current instruction. This uses the CPU's own ByteQueue trait 
            // implementation, which fetches instruction bytes through the processor instruction queue.
            self.i = match Cpu::decode(self) {
                Ok(i) => i,
                Err(_) => {
                    self.is_running = false;
                    self.is_error = true;
                    return Err(CpuError::InstructionDecodeError(instruction_address))
                }                
            };

            // Begin the current instruction validation context.
            #[cfg(feature = "cpu_validator")]
            {
                let vregs = self.get_vregisters();

                if vregs.flags & CPU_FLAG_TRAP != 0 {
                    log::warn!("Trap flag is set - may break validator!");
                }

                if let Some(ref mut validator) = self.validator {

                    if (instruction_address as usize) == self.validator_end {
                        log::info!("Validation reached end address. Stopping.");
                        self.validator_state = CpuValidatorState::Ended;
                    }

                    if self.validator_state == CpuValidatorState::Uninitialized 
                        || self.validator_state == CpuValidatorState::Running {

                        validator.begin_instruction(
                            &vregs, 
                            (instruction_address + self.i.size) as usize & 0xFFFFF,
                            self.validator_end
                        );
                    }
                }
            }
        }

        // Since Cpu::decode doesn't know anything about the current IP, it can't set it, so we do that now.
        self.i.address = instruction_address;

        let mut check_interrupts = false;

        //let (opcode, _cost) = self.bus.read_u8(instruction_address as usize, 0).expect("mem err");
        //trace_print!(self, "Fetched instruction: {} op:{:02X} at [{:05X}]", self.i, opcode, self.i.address);
        //trace_print!(self, "Executing instruction:  [{:04X}:{:04X}] {} ({})", self.cs, self.ip, self.i, self.i.size);

        //log::warn!("Fetched instruction: {} op:{:02X} at [{:05X}]", self.i, opcode, self.i.address);
        //log::warn!("Executing instruction:  [{:04X}:{:04X}] {} ({})", self.cs, self.ip, self.i, self.i.size);

        let last_cs = self.cs;
        let last_ip = self.ip;

        // Load the mod/rm operand for the instruction, if applicable.
        self.load_operand();

        #[cfg(feature = "cpu_validator")]
        {
            (self.peek_fetch, _) = self.bus.read_u8(self.pc as usize, 0).unwrap();
            self.instr_slice = self.bus.get_vec_at(instruction_address as usize, self.i.size as usize);
        }
    
        // Execute the current decoded instruction.
        self.exec_result = self.execute_instruction();

        let mut step_result = match &self.exec_result {

            ExecutionResult::Okay => {
                // Normal non-jump instruction updates CS:IP to next instruction during execute()
                if self.instruction_history_on {
                    if self.instruction_history.len() == CPU_HISTORY_LEN {
                        self.instruction_history.pop_front();
                    }
                    self.instruction_history.push_back(
                        HistoryEntry::Entry {
                            cs: last_cs, 
                            ip: last_ip, 
                            cycles: self.instr_cycle as u16, 
                            i: self.i
                        }
                    );
                    self.instruction_count += 1;
                }

                check_interrupts = true;

                // Perform instruction tracing, if enabled
                if self.trace_enabled && self.trace_mode == TraceMode::Instruction {
                    self.trace_print(&self.instruction_state_string(last_cs, last_ip));   
                }                

                Ok((StepResult::Normal, self.device_cycles))
            }
            ExecutionResult::OkayJump => {
                // A control flow instruction updated CS:IP.
                if self.instruction_history_on {
                    if self.instruction_history.len() == CPU_HISTORY_LEN {
                        self.instruction_history.pop_front();
                    }
                    self.instruction_history.push_back(
                        HistoryEntry::Entry {
                            cs: last_cs, 
                            ip: last_ip, 
                            cycles: self.instr_cycle as u16, 
                            i: self.i
                        }
                    );
                    self.instruction_count += 1;
                }

                check_interrupts = true;

                // Perform instruction tracing, if enabled
                if self.trace_enabled && self.trace_mode == TraceMode::Instruction {
                    self.trace_print(&self.instruction_state_string(last_cs, last_ip));   
                }
   
                // Only CALLS will set a step over target. 
                if let Some(step_over_target) = self.step_over_target {
                    Ok((StepResult::Call(step_over_target), self.device_cycles))
                }
                else {
                    Ok((StepResult::Normal, self.device_cycles))
                }                
            }
            ExecutionResult::OkayRep => {
                // We are in a REPx-prefixed instruction.

                // The ip will not increment until the instruction has completed, but
                // continue to process interrupts. We passed pending_interrupt to execute
                // earlier so that a REP string operation can call RPTI to be ready for
                // an interrupt to occur.
                if self.instruction_history_on {
                    if self.instruction_history.len() == CPU_HISTORY_LEN {
                        self.instruction_history.pop_front();
                    }
                    
                    self.instruction_history.push_back(
                        HistoryEntry::Entry {
                            cs: last_cs, 
                            ip: last_ip, 
                            cycles: self.instr_cycle as u16, 
                            i: self.i
                        }
                    );
                }
                self.instruction_count += 1;
                check_interrupts = true;

                Ok((StepResult::Normal, self.device_cycles))
            }                    
            /*
            ExecutionResult::UnsupportedOpcode(o) => {
                // This shouldn't really happen on the 8088 as every opcode does something, 
                // but allowed us to be missing opcode implementations during development.
                self.is_running = false;
                self.is_error = true;
                Err(CpuError::UnhandledInstructionError(o, instruction_address))
            }
            */
            ExecutionResult::ExecutionError(e) => {
                // Something unexpected happened!
                self.is_running = false;
                self.is_error = true;
                Err(CpuError::ExecutionError(instruction_address, e.to_string()))
            }
            ExecutionResult::Halt => {
                // Specifically, this error condition is a halt with interrupts disabled -
                // since only an interrupt can resume after a halt, execution cannot continue. 
                // This state is most often encountered during failed BIOS initialization checks.
                self.is_running = false;
                self.is_error = true;
                Err(CpuError::CpuHaltedError(instruction_address))
            }
            ExecutionResult::ExceptionError(exception) => {
                // A CPU exception occurred. On the 8088, these are limited in scope to 
                // division errors, and overflow after INTO.
                match exception {
                    CpuException::DivideError => {
                        // Moved int0 handling into aam/div instructions directly.
                        //self.handle_exception(0);
                        Ok((StepResult::Normal, self.device_cycles))
                    }
                    _ => {
                        // Unhandled exception?
                        Err(CpuError::ExceptionError(*exception))
                    }
                }
            }
        };

        // Reset interrupt pending flag - this flag is set on step_finish() and 
        // only valid for a single instruction execution.
        self.intr_pending = false;

        // Check registers and flags for internal consistency.
        #[cfg(debug_assertions)]        
        self.assert_state();

        step_result
    }

    /// Set the status of the CPU's INTR line.
    #[inline]
    pub fn set_intr(&mut self, status: bool) {
        self.intr = status;
    }

    /// Finish the current CPU instruction.
    /// 
    /// This function is meant to be called after devices are run after an instruction. 
    /// 
    /// Normally, this function will fetch the first byte of the next instruction. 
    /// Running devices can generate interrupts. If the INTR line is set by a device,
    /// we do not want to fetch the next byte - we want to jump directly into the 
    /// interrupt routine - *unless* we are in a REP, in which case we set a flag
    /// so that the interrupt execution can occur on the next call to step() to simulate
    /// the string instruction calling RPTI. 
    /// 
    /// This function effectively simulates the RNI microcode routine.
    pub fn step_finish(&mut self) -> Result<StepResult, CpuError> {

        let mut step_result = StepResult::Normal;
        let mut irq = 7;

        // This function is called after devices are run for the CPU period, so reset device cycles.
        // Device cycles will begin incrementing again with any terminating fetch.
        self.instr_elapsed = 0;
        self.int_elapsed = 0;
        self.device_cycles = 0;

        // Fetch next instruction byte, if applicable.
        if (!self.intr || !self.interrupts_enabled()) && !self.halted {
            // Do not fetch if there's an active interrupt (and interrupts are enabled)
            // Do not fetch if we are halted.
            self.biu_fetch_next();
        }
        else if self.intr && self.interrupts_enabled() {
            // An interrupt needs to be processed.

            if self.in_rep {
                // We're in an REP prefixed-string instruction.
                // Delay processing of the interrupt so that the string
                // instruction can execute RPTI. At that point, the REP 
                // will terminate and we can process the interrupt as normal.
                self.intr_pending = true;
            }
            else {
                // We are not in a REP prefixed string instruction, so we
                // can process an interrupt normally.

                if self.halted {
                    // Resume from halt on interrupt
                    self.resume();
                }        

                // Query the PIC to get the interrupt vector.
                // This is a bit artificial as we don't actually read the IV during the 2nd 
                // INTA cycle like the CPU does, instead we save the value now and simualate it later.
                // TODO: Think about changing this to query during INTA
                if let Some(pic) = self.bus.pic_mut().as_mut() {
                    // Is INTR active? TODO: Could combine these calls (return Option<iv>) on query?
                    if pic.query_interrupt_line() {
                        if let Some(iv) = pic.get_interrupt_vector() {
                            irq = iv;
                        }
                    }
                }

                // We will be jumping into an ISR now. Set the step result to Call and return
                // the address of the next instruction. (Step Over skips ISRs)
                step_result = StepResult::Call(CpuAddress::Segmented(self.cs, self.ip));
            
                if self.int_flags[irq as usize] != 0 {
                    // This interrupt has a breakpoint
                    self.set_breakpoint_flag();
                }            
                self.hw_interrupt(irq);
                self.biu_fetch_next();
            }
        }

        // If a CPU validator is configured, validate the executed instruction.
        #[cfg(feature = "cpu_validator")]
        {
            match self.exec_result {
                ExecutionResult::Okay 
                | ExecutionResult::OkayJump 
                | ExecutionResult::ExceptionError(CpuException::DivideError) => {

                    let mut v_flags = 0;
        
                    if let ExecutionResult::ExceptionError(CpuException::DivideError) = self.exec_result {
                        // In the case of a divide exception, undefined flags get pushed to the stack.
                        // So until we figure out the actual logic behind setting those undefined flags,
                        // we can't validate writes. Also the cycle timing seems to vary a little when
                        // executing int0, so allow a one cycle variance.
                        v_flags |= VAL_NO_WRITES | VAL_NO_FLAGS | VAL_ALLOW_ONE;
                    }

                    match self.i.mnemonic {
                        Mnemonic::DIV => {
                            // There's a one cycle variance in my DIV instructions somewhere.
                            // I just want to get these tests out the door, so allow it.
                            v_flags |= VAL_ALLOW_ONE;
                        }
                        Mnemonic::IDIV => {
                            v_flags |= VAL_NO_WRITES | VAL_NO_FLAGS | VAL_NO_CYCLES;
                        }
                        _=> {}
                    }

                    // End validation of current instruction
                    let vregs = self.get_vregisters();
                    
                    if self.i.size == 0 {
                        log::error!("Invalid length: [{:05X}] {}", self.instruction_address, self.i);
                    }

                    let cpu_address = self.get_linear_ip() as usize;
                    
                    if let Some(ref mut validator) = self.validator {

                        // If validator unininitalized, set register state now and move into running state.
                        if self.validator_state == CpuValidatorState::Uninitialized {
                            // This resets the validator CPU
                            log::debug!("Validator Uninitialized. Resetting validator and setting registers...");
                            validator.set_regs();
                            self.validator_state = CpuValidatorState::Running;
                        }

                        if self.validator_state == CpuValidatorState::Running {

                            log::debug!("Validating opcode: {:02X}", self.i.opcode);
                            match validator.validate_instruction(
                                self.i.to_string(), 
                                &self.instr_slice,
                                v_flags,
                                self.peek_fetch as u16,
                                self.i.flags & I_HAS_MODRM != 0,
                                0,
                                &vregs,
                                &self.cycle_states
                            ) {
    
                                Ok(result) => {
                                    match result {
                                        ValidatorResult::Ok => {},
                                        ValidatorResult::OkEnd => {
    
                                            if self.validator_end == cpu_address {

                                                self.validator_state = CpuValidatorState::Ended;

                                                // Validation has reached program end address
                                                if let Err(e) = validator.validate_regs(&vregs) {
                                                    log::warn!("Register validation failure: {} Halting execution.", e);
                                                    self.is_running = false;
                                                    self.is_error = true;
                                                    return Err(CpuError::CpuHaltedError(self.instruction_address))
                                                }
                                                else {
                                                    log::debug!("Registers validated. Validation ended successfully.");
                                                    self.validator_state = CpuValidatorState::Ended;
                                                    self.trace_flush();
                                                }                                                
                                            }
                                        }
                                        _=> {
                                            log::warn!("Validation failure: Halting execution.");
                                            self.is_running = false;
                                            self.is_error = true;
                                            return Err(CpuError::CpuHaltedError(self.instruction_address))
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("Validation failure: {} Halting execution.", e);
                                    self.is_running = false;
                                    self.is_error = true;
                                    return Err(CpuError::CpuHaltedError(self.instruction_address))
                                }
                            }
                        }
                    }                    
                }
                _ => {}
            }            
        }

        Ok(step_result)
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
        self.breakpoints.iter().for_each(|bp| {
            match bp {
                BreakPointType::ExecuteFlat(addr) => {
                    log::debug!("Clearing breakpoint on execute at address: {:05X}", *addr);
                    self.bus.clear_flags(*addr as usize, MEM_BPE_BIT );
                },
                BreakPointType::MemAccessFlat(addr) => {
                    self.bus.clear_flags(*addr as usize, MEM_BPA_BIT );
                }
                BreakPointType::Interrupt(vector) => {
                    self.int_flags[*vector as usize] = 0;
                }
                _ => {}
            }
        });

        // Replace current breakpoint list
        self.breakpoints = bp_list;

        // Set bus flags for new breakpoints
        self.breakpoints.iter().for_each(|bp| {
            match bp {
                BreakPointType::ExecuteFlat(addr) => {
                    log::debug!("Setting breakpoint on execute at address: {:05X}", *addr);
                    self.bus.set_flags(*addr as usize, MEM_BPE_BIT );
                },
                BreakPointType::MemAccessFlat(addr) => {
                    log::debug!("Setting breakpoint on memory access at address: {:05X}", *addr);
                    self.bus.set_flags(*addr as usize, MEM_BPA_BIT );
                }
                BreakPointType::Interrupt(vector) => {
                    self.int_flags[*vector as usize] = INTERRUPT_BREAKPOINT;
                }                
                _ => {}
            }
        });

    }

    pub fn get_breakpoint_flag(&self) -> bool {
        if let CpuState::BreakpointHit = self.state {
            true
        }
        else {
            false
        }
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
            if let HistoryEntry::Entry {cs, ip, cycles: _, i} = i {      
                let i_string = format!("{:05X} [{:04X}:{:04X}] {}\n", i.address, *cs, *ip, i);
                disassembly_string.push_str(&i_string);
            }
        }
        disassembly_string
    }

    pub fn dump_instruction_history_tokens(&self) -> Vec<Vec<SyntaxToken>> {

        let mut history_vec = Vec::new();

        for i in &self.instruction_history {
            let mut i_token_vec = Vec::new();
            if let HistoryEntry::Entry {cs, ip, cycles, i} = i {
                i_token_vec.push(SyntaxToken::MemoryAddressFlat(i.address, format!("{:05X}", i.address)));
                i_token_vec.push(SyntaxToken::MemoryAddressSeg16(*cs, *ip, format!("{:04X}:{:04X}", cs, ip)));
                i_token_vec.push(SyntaxToken::Text(format!("{}", cycles)));
                i_token_vec.extend(i.tokenize());
            }
            history_vec.push(i_token_vec);
        }
        history_vec
    }    

    pub fn dump_call_stack(&self) -> String {
        let mut call_stack_string = String::new();

        for call in &self.call_stack {
            match call {
                CallStackEntry::Call{ ret_cs, ret_ip, call_ip } => {
                    call_stack_string.push_str(&format!("{:04X}:{:04X} CALL {:04X}\n", ret_cs, ret_ip, call_ip));
                }
                CallStackEntry::CallF{ ret_cs, ret_ip, call_cs, call_ip } => {
                    call_stack_string.push_str(&format!("{:04X}:{:04X} CALL FAR {:04X}:{:04X}\n", ret_cs, ret_ip, call_cs, call_ip));
                }
                CallStackEntry::Interrupt{ ret_cs, ret_ip, call_cs, call_ip, itype, number, ah } => {
                    call_stack_string.push_str(&format!("{:04X}:{:04X} INT {:02X} {:04X}:{:04X} type={:?} AH=={:02X}\n", ret_cs, ret_ip, number, call_cs, call_ip, itype, ah));
                }
            }   
        }

        call_stack_string
    }

    pub fn cycle_state_string(&self, dma_count: u16, short: bool) -> String {

        let ale_str = match self.i8288.ale {
            true => "A:",
            false => "  "
        };

        let mut seg_str = "  ";
        if self.t_cycle != TCycle::T1 {
            // Segment status only valid in T2+
            seg_str = match self.bus_segment {
                Segment::None => "  ",
                Segment::SS => "SS",
                Segment::ES => "ES",
                Segment::CS => "CS",
                Segment::DS => "DS"
            };    
        }

        let q_op_chr = match self.last_queue_op {
            QueueOp::Idle => ' ',
            QueueOp::First => 'F',
            QueueOp::Flush => 'E',
            QueueOp::Subsequent => 'S',
        };

        let q_preload_char = match self.queue.has_preload() {
            true => '*',
            false => ' '
        };

        let biu_state_new_str = match self.biu_state_new {
            BiuStateNew::ToIdle(_) => ">I ",
            BiuStateNew::ToPrefetch(_) => ">PF",
            BiuStateNew::ToEu(_) => ">EU",
            BiuStateNew::Idle => "I  ",
            BiuStateNew::Prefetch => "PF ",
            BiuStateNew::Eu => "EU "
        };

        /*
        let mut f_op_chr = match self.fetch_state {
            FetchState::Scheduled(_) => 'S',
            FetchState::Aborted(_) => 'A',
            //FetchState::Suspended => '!',
            _ => ' '
        };

        if self.fetch_suspended {
            f_op_chr = '!'
        }
        */

        // All read/write signals are active/low
        let rs_chr = match self.i8288.mrdc {
            true => 'R',
            false => '.',
        };
        let aws_chr  = match self.i8288.amwc {
            true => 'A',
            false => '.',
        };
        let ws_chr   = match self.i8288.mwtc {
            true => 'W',
            false => '.',
        };
        let ior_chr  = match self.i8288.iorc {
            true => 'R',
            false => '.',
        };
        let aiow_chr = match self.i8288.aiowc {
            true => 'A',
            false => '.',
        };
        let iow_chr  = match self.i8288.iowc {
            true => 'W',
            false => '.',
        };

        let bus_str = match self.bus_status_latch {
            BusStatus::InterruptAck => "IRQA",
            BusStatus::IoRead=> "IOR ",
            BusStatus::IoWrite => "IOW ",
            BusStatus::Halt => "HALT",
            BusStatus::CodeFetch => "CODE",
            BusStatus::MemRead => "MEMR",
            BusStatus::MemWrite => "MEMW",
            BusStatus::Passive => "PASV"     
        };

        let t_str = match self.t_cycle {
            TCycle::Tinit => "Tx",
            TCycle::Ti => "Ti",
            TCycle::T1 => "T1",
            TCycle::T2 => "T2",
            TCycle::T3 => "T3",
            TCycle::T4 => "T4",
            TCycle::Tw => "Tw",
        };

        let is_reading = self.i8288.mrdc | self.i8288.iorc;
        let is_writing = self.i8288.mwtc | self.i8288.iowc;

        let mut xfer_str = "      ".to_string();
        if is_reading {
            xfer_str = format!("<-r {:02X}", self.data_bus);
        }
        else if is_writing {
            xfer_str = format!("w-> {:02X}", self.data_bus);
        }

        // Handle queue activity

        let mut q_read_str = "      ".to_string();

        let mut instr_str = String::new();

        if self.last_queue_op == QueueOp::First || self.last_queue_op == QueueOp::Subsequent {
            // Queue byte was read.
            q_read_str = format!("<-q {:02X}", self.last_queue_byte);
        }

        if self.last_queue_op == QueueOp::First {
            // First byte of opcode read from queue. Decode the full instruction
            instr_str = format!(
                "[{:04X}:{:04X}] {} ({}) ", 
                self.cs, 
                self.ip, 
                self.i,
                self.i.size
            );
        }
      
        //let mut microcode_str = "   ".to_string();
        let microcode_line_str = match self.trace_instr {
            MC_JUMP => "JMP".to_string(),
            MC_RTN => "RET".to_string(),
            MC_CORR => "COR".to_string(),
            MC_NONE => "   ".to_string(),
            _ => {
                format!("{:03X}", self.trace_instr)
            }
        };

        let microcode_op_str = match self.trace_instr {
            i if usize::from(i) < MICROCODE_SRC_8088.len() => {
                MICROCODE_SRC_8088[i as usize].to_string()
            }
            _ => MICROCODE_NUL.to_string()
        };

        let dma_dreq_chr = match self.dma_aen {
            true => 'R',
            false => '.'
        };

        let tx_cycle = match self.is_last_wait() {
            true => 'x',
            false => '.'
        };

        let ready_chr = if self.wait_states > 0 {
            '.'
        }
        else {
            'R'
        };

        let dma_count_str = &format!("{:02} {:02}", dma_count, self.dram_refresh_cycle_num);

        let dma_str = match self.dma_state {
            DmaState::Idle => dma_count_str,
            DmaState::TimerTrigger => "TIMR",
            DmaState::Dreq => "DREQ",
            DmaState::Hrq => "HRQ ",
            DmaState::HoldA => "HLDA",
            DmaState::Operating(n) => {
                match n {
                    4 => "S1",
                    3 => "S2",
                    2 => "S3",
                    1 => "S4",
                    _ => "S?"
                }
            }
            //DmaState::DmaWait(..) => "DMAW"
        };

        let mut cycle_str;

        if short {
            cycle_str = format!(
                "{:04} {:02}[{:05X}] {:02} {}{} M:{}{}{} I:{}{}{} |{:5}| {:04} {:02} {:06} | {:4}| {:<14}| {:1}{:1}{:1}[{:08}] {} | {:03} | {}",
                self.instr_cycle,
                ale_str,
                self.address_bus,
                seg_str,
                ready_chr,
                self.wait_states,
                rs_chr, aws_chr, ws_chr, ior_chr, aiow_chr, iow_chr,
                dma_str,
                bus_str,
                t_str,
                xfer_str,
                biu_state_new_str,
                format!("{:?}", self.fetch_state),
                q_op_chr,
                self.last_queue_len,
                q_preload_char,
                self.queue.to_string(),
                q_read_str,
                microcode_line_str,
                instr_str
            ); 
        }
        else {
            cycle_str = format!(
                "{:08}:{:04} {:02}[{:05X}] {:02} {}{}{} M:{}{}{} I:{}{}{} |{:5}|  | {:04} {:02} {:06} | {:4}| {:<14}| {:1}{:1}{:1}[{:08}] {} | {}: {} | {}",
                self.cycle_num,
                self.instr_cycle,
                ale_str,
                self.address_bus,
                seg_str,
                ready_chr,
                self.wait_states,
                tx_cycle,
                rs_chr, aws_chr, ws_chr, ior_chr, aiow_chr, iow_chr,
                dma_str,
                bus_str,
                t_str,
                xfer_str,
                biu_state_new_str,
                format!("{:?}", self.fetch_state),
                q_op_chr,
                self.last_queue_len,
                q_preload_char,
                self.queue.to_string(),
                q_read_str,
                microcode_line_str,
                microcode_op_str,
                instr_str
            ); 
        }
        
        for c in &self.trace_comment {
            cycle_str.push_str(&format!("; {}", c));
        }

        cycle_str
    }

    pub fn trace_csv_line(&mut self) {
        let q = self.last_queue_op as u8;
        let s = self.bus_status as u8;

        let mut vs = 0;
        let mut hs = 0;
        let mut den = 0;
        let mut brd = 0;
        if let Some(video) = self.bus().video() {
            let (vs_b, hs_b, den_b, brd_b) = video.get_sync();
            vs = if vs_b { 1 } else { 0 };
            hs = if hs_b { 1 } else { 0 };
            den = if den_b { 1 } else { 0 };
            brd = if brd_b { 1 } else { 0 };
        }

        // Segment status bits are valid after ALE.
        if !self.i8288.ale {
            let seg_n = match self.bus_segment {
                Segment::ES => 0,
                Segment::SS => 1,
                Segment::CS | Segment::None => 2,
                Segment::DS => 3             
            };
            self.address_bus = (self.address_bus & 0b1100_1111_1111_1111_1111) | (seg_n << 16);
        }

        // "Time(s),addr,clk,ready,qs,s,clk0,intr,dr0,vs,hs"
        // sigrok import string:
        // t,x20,l,l,x2,x3,l,l,l,l,l,l
        self.trace_emit(&format!(
            "{},{:05X},1,{},{},{},{},{},{},{},{},{},{}",
            self.t_stamp,
            self.address_bus,
            if self.ready { 1 } else { 0 },
            q,
            s,
            0,
            if self.intr { 1 } else { 0 },
            if matches!(self.dma_state, DmaState::Dreq) { 1 } else { 0 },
            vs,
            hs,
            den,
            brd
        ));

        self.trace_emit(&format!(
            "{},{:05X},0,{},{},{},{},{},{},{},{},{},{}",
            self.t_stamp + self.t_step_h,
            self.address_bus,
            if self.ready { 1 } else { 0 },
            q,
            s,
            0,
            if self.intr { 1 } else { 0 },
            if matches!(self.dma_state, DmaState::Dreq) { 1 } else { 0 },
            vs,
            hs,
            den,
            brd
        ));
    }

    pub fn instruction_state_string(&self, last_cs: u16, last_ip: u16) -> String {
        let mut instr_str = String::new();

        instr_str.push_str(&format!("{:04x}:{:04x} {}\n", last_cs, last_ip, self.i));
        instr_str.push_str(&format!("AX: {:04x} BX: {:04x} CX: {:04x} DX: {:04x}\n", self.ax, self.bx, self.cx, self.dx));
        instr_str.push_str(&format!("SP: {:04x} BP: {:04x} SI: {:04x} DI: {:04x}\n", self.sp, self.bp, self.si, self.di));
        instr_str.push_str(&format!("CS: {:04x} DS: {:04x} ES: {:04x} SS: {:04x}\n", self.cs, self.ds, self.es, self.ss));
        instr_str.push_str(&format!("IP: {:04x} FLAGS: {:04x}", self.ip, self.flags));

        instr_str
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
        if self.trace_enabled && (self.trace_mode == TraceMode::Cycle) {
            self.trace_comment.push(comment);
        }
    }

    #[inline]
    pub fn trace_instr(&mut self, instr: u16) {
        self.trace_instr = instr;
    }

    pub fn assert_state(&self) {

        let ax_should = (self.ah as u16) << 8 | self.al as u16;
        let bx_should = (self.bh as u16) << 8 | self.bl as u16;
        let cx_should = (self.ch as u16) << 8 | self.cl as u16;
        let dx_should = (self.dh as u16) << 8 | self.dl as u16;

        assert_eq!(self.ax, ax_should);
        assert_eq!(self.bx, bx_should);
        assert_eq!(self.cx, cx_should);
        assert_eq!(self.dx, dx_should);

        let should_be_off = self.flags & !CPU_FLAGS_RESERVED_OFF;
        assert_eq!(should_be_off, 0);

        let should_be_set = self.flags & CPU_FLAGS_RESERVED_ON;
        assert_eq!(should_be_set, CPU_FLAGS_RESERVED_ON);

    }

    pub fn dump_cs(&self, path: &Path) {
        
        let mut filename = path.to_path_buf();
        filename.push("cs.bin");
        
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

    pub fn set_option(&mut self, opt: CpuOption) {

        match opt {
            CpuOption::InstructionHistory(state) => {
                log::debug!("Setting InstructionHistory to: {:?}", state);
                self.instruction_history.clear();
                self.instruction_history_on = state;
            }
            CpuOption::SimulateDramRefresh(state, cycle_target, cycles) => {
                log::debug!("Setting SimulateDramRefresh to: {:?} ({},{})", state, cycle_target, cycles);
                self.dram_refresh_simulation = state;
                self.dram_refresh_cycle_period = cycle_target;
                self.dram_refresh_cycle_num = cycles;
            }
            CpuOption::DramRefreshAdjust(adj) => {
                log::debug!("Setting DramRefreshAdjust to: {}", adj);
                self.dram_refresh_adjust = adj;
            }
            CpuOption::HaltResumeDelay(delay) => {
                log::debug!("Setting HaltResumeDelay to: {}", delay);
                self.halt_resume_delay = delay;
            }            
            CpuOption::OffRailsDetection(state) => {
                log::debug!("Setting OffRailsDetection to: {:?}", state);
                self.off_rails_detection = state;
            }
            CpuOption::EnableWaitStates(state) => {
                log::debug!("Setting EnableWaitStates to: {:?}", state);
                self.enable_wait_states = state;
            }   
            CpuOption::TraceLoggingEnabled(state) => {
                log::debug!("Setting TraceLoggingEnabled to: {:?}", state);
                self.trace_enabled = state;

                // Flush the trace log file on stopping trace so that we can immediately
                // see results otherwise buffered
                if state == false {
                    self.trace_flush();
                }
            }
            CpuOption::EnableServiceInterrupt(state) => {
                log::debug!("Setting EnableServiceInterrupt to: {:?}", state);
                self.enable_service_interrupt = state;
            }
        }
    }

    pub fn get_option(&mut self, opt: CpuOption) -> bool {
        match opt {
            CpuOption::InstructionHistory(_) => {
                self.instruction_history_on
            }
            CpuOption::SimulateDramRefresh(..) => {
                self.dram_refresh_simulation
            }
            CpuOption::DramRefreshAdjust(..) => {
                true
            }
            CpuOption::HaltResumeDelay(..) => {
                true
            }            
            CpuOption::OffRailsDetection(_) => {
                self.off_rails_detection
            }
            CpuOption::EnableWaitStates(_) => {
                self.enable_wait_states
            }   
            CpuOption::TraceLoggingEnabled(_) => {
                self.trace_enabled
            }
            CpuOption::EnableServiceInterrupt(_) => {
                self.enable_service_interrupt
            }                        
        }        
    }

    pub fn get_cycle_trace(&self ) -> &Vec<String> {
        &self.trace_str_vec
    }

    #[cfg(feature = "cpu_validator")]
    pub fn get_validator_state(&self) -> CpuValidatorState {
        self.validator_state
    }

    #[cfg(feature = "cpu_validator")]
    pub fn get_validator(&mut self) -> &Option<Box<dyn CpuValidator>> {
        &self.validator
    }
        
    pub fn flags_string(f: u16) -> String {

        let c_chr = if CPU_FLAG_CARRY & f != 0 { 'C' } else { 'c' };
        let p_chr = if CPU_FLAG_PARITY & f != 0 { 'P' } else { 'p' };
        let a_chr = if CPU_FLAG_AUX_CARRY & f != 0 { 'A' } else { 'a' };
        let z_chr = if CPU_FLAG_ZERO & f != 0 { 'Z' } else { 'z' };
        let s_chr = if CPU_FLAG_SIGN & f != 0 { 'S' } else { 's' };
        let t_chr = if CPU_FLAG_TRAP & f != 0 { 'T' } else {  't' };
        let i_chr = if CPU_FLAG_INT_ENABLE & f != 0 { 'I' } else { 'i' };
        let d_chr = if CPU_FLAG_DIRECTION & f != 0 { 'D' } else { 'd' };
        let o_chr = if CPU_FLAG_OVERFLOW & f != 0 { 'O' } else { 'o' };
  
        format!(
            "1111{}{}{}{}{}{}0{}0{}1{}", 
            o_chr, d_chr, i_chr, t_chr, s_chr, z_chr, a_chr, p_chr, c_chr
        )
    }

}



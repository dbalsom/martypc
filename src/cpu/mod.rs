#![allow(dead_code)]

use std::{
    rc::Rc,
    cell::RefCell,
    collections::VecDeque,
    error::Error,
    fmt,
    io::Write
};

use core::fmt::Display;

use lazy_static::lazy_static;
use regex::Regex;

// Pull in all CPU module components
mod cpu_addressing;
mod cpu_alu;
mod cpu_bcd;
mod cpu_bitwise;
mod cpu_biu;
mod cpu_decode;
mod cpu_display;
mod cpu_execute;
mod cpu_modrm;
pub mod cpu_mnemonic;
mod cpu_stack;
mod cpu_string;
mod cpu_queue;
mod cpu_fuzzer;

use crate::cpu::cpu_mnemonic::Mnemonic;
use crate::cpu::cpu_addressing::AddressingMode;
use crate::cpu::cpu_queue::InstructionQueue;
use crate::cpu::cpu_biu::*;

use crate::config::TraceMode;
#[cfg(feature = "cpu_validator")]
use crate::config::ValidatorType;

use crate::bus::BusInterface;
use crate::pic::Pic;
use crate::bytequeue::*;
use crate::io::IoBusInterface;

#[cfg(feature = "cpu_validator")]
use crate::cpu_validator::{CpuValidator, CycleState, VRegisters, BusCycle, BusState, AccessType};
#[cfg(feature = "pi_validator")]
use crate::pi_cpu_validator::{PiValidator};
#[cfg(feature = "arduino_validator")]
use crate::arduino8088_validator::{ArduinoValidator};

pub const CPU_MHZ: f64 = 4.77272666;

const QUEUE_MAX: usize = 6;
const FETCH_DELAY: u8 = 2;

const CPU_HISTORY_LEN: usize = 32;
const CPU_CALL_STACK_LEN: usize = 16;

const INTERRUPT_VEC_LEN: usize = 4;

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

const CPU_FLAG_RESERVED12: u16 = 0b0001_0000_0000_0000;
const CPU_FLAG_RESERVED13: u16 = 0b0010_0000_0000_0000;
const CPU_FLAG_RESERVED14: u16 = 0b0100_0000_0000_0000;
const CPU_FLAG_RESERVED15: u16 = 0b1000_0000_0000_0000;

const CPU_FLAGS_RESERVED_ON: u16 = 0b1111_0000_0000_0010;

const FLAGS_POP_MASK: u16      = 0b0000_1111_1101_0101;

const REGISTER_HI_MASK: u16    = 0b0000_0000_1111_1111;
const REGISTER_LO_MASK: u16    = 0b1111_1111_0000_0000;

pub const MAX_INSTRUCTION_SIZE: usize = 15;

const OPCODE_REGISTER_SELECT_MASK: u8 = 0b0000_0111;

const MODRM_REG_MASK:          u8 = 0b00_111_000;
const MODRM_ADDR_MASK:         u8 = 0b11_000_111;
const MODRM_MOD_MASK:          u8 = 0b11_000_000;

const MODRM_ADDR_BX_SI:        u8 = 0b00_000_000;
const MODRM_ADDR_BX_DI:        u8 = 0b00_000_001;
const MODRM_ADDR_BP_SI:        u8 = 0b00_000_010;
const MODRM_ADDR_BP_DI:        u8 = 0b00_000_011;
const MODRM_ADDR_SI:           u8 = 0b00_000_100;
const MODRM_ADDR_DI:           u8 = 0b00_000_101;
const MODRM_ADDR_DISP16:       u8 = 0b00_000_110;
const MODRM_ADDR_BX:           u8 = 0b00_000_111;

const MODRM_ADDR_BX_SI_DISP8:  u8 = 0b01_000_000;
const MODRM_ADDR_BX_DI_DISP8:  u8 = 0b01_000_001;
const MODRM_ADDR_BP_SI_DISP8:  u8 = 0b01_000_010;
const MODRM_ADDR_BP_DI_DISP8:  u8 = 0b01_000_011;
const MODRM_ADDR_SI_DISP8:     u8 = 0b01_000_100;
const MODRM_ADDR_DI_DISP8:     u8 = 0b01_000_101;
const MODRM_ADDR_BP_DISP8:     u8 = 0b01_000_110;
const MODRM_ADDR_BX_DISP8:     u8 = 0b01_000_111;

const MODRM_ADDR_BX_SI_DISP16: u8 = 0b10_000_000;
const MODRM_ADDR_BX_DI_DISP16: u8 = 0b10_000_001;
const MODRM_ADDR_BP_SI_DISP16: u8 = 0b10_000_010;
const MODRM_ADDR_BP_DI_DISP16: u8 = 0b10_000_011;
const MODRM_ADDR_SI_DISP16:    u8 = 0b10_000_100;
const MODRM_ADDR_DI_DISP16:    u8 = 0b10_000_101;
const MODRM_ADDR_BP_DISP16:    u8 = 0b10_000_110;
const MODRM_ADDR_BX_DISP16:    u8 = 0b10_000_111;

const MODRM_EG_AX_OR_AL:       u8 = 0b00_000_000;
const MODRM_REG_CX_OR_CL:      u8 = 0b00_000_001;
const MODRM_REG_DX_OR_DL:      u8 = 0b00_000_010;
const MODRM_REG_BX_OR_BL:      u8 = 0b00_000_011;
const MODRM_REG_SP_OR_AH:      u8 = 0b00_000_100;
const MODRM_REG_BP_OR_CH:      u8 = 0b00_000_101;
const MODRM_REG_SI_OR_DH:      u8 = 0b00_000_110;
const MODRM_RED_DI_OR_BH:      u8 = 0b00_000_111;

// Instruction flags
const INSTRUCTION_USES_MEM:    u32 = 0b0000_0001;
const INSTRUCTION_HAS_MODRM:   u32 = 0b0000_0010;
const INSTRUCTION_LOCKABLE:    u32 = 0b0000_0100;
const INSTRUCTION_REL_JUMP:    u32 = 0b0000_1000;

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

pub enum CpuType {
    Cpu8088,
    Cpu8086,
}

impl Default for CpuType {
    fn default() -> Self { CpuType::Cpu8088 }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum CpuException {
    NoException,
    DivideError
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

#[derive(Debug)]
pub enum CallStackEntry {
    Call(u16,u16,u16),
    CallF(u16,u16,u16,u16),
    Interrupt(u16,u16,u8)
}

/// Representation of the state of a REPeated string instruction, saved on interrupt
pub enum RepState {
    NoState,
    StosbState(u16, u16, u16), // dst: [es:di], cx
    StoswState(u16, u16, u16), // dst: [es:di], cx
    LodsbState(Register16, u16, u16, u16), // src: [ds*:si], cx
    LodswState(Register16, u16, u16, u16), // src: [ds*:si], cx
    MovsbState(Register16, u16, u16, u16, u16, u16), // src: [ds*:si], dst: [es:di], cx
    MovswState(Register16, u16, u16, u16, u16, u16), // src: [ds*:si]. dst: [es:di], cx
    ScasbState(u16,u16,u16), // src: [es:di], cx
    ScaswState(u16,u16,u16), // src: [es:di], cx
    CmpsbState(Register16, u16, u16, u16, u16, u16), // src: [ds*:si], dst: [es:di], cx
    CmpswState(Register16, u16, u16, u16, u16, u16), // src: [ds*:si], dst: [es:di], cx
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
}

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
    Relative8(i8),
    Relative16(i16),
    Offset8(u16),
    Offset16(u16),
    Register8(Register8),
    Register16(Register16),
    AddressingMode(AddressingMode),
    NearAddress(u16),
    FarAddress(u16,u16),
    NoOperand,
    InvalidOperand
}

#[derive(Copy, Clone)]
pub enum DispType {
    NoDisp,
    Disp8,
    Disp16,
}

#[derive(Copy, Clone, Debug)]
pub enum Displacement {
    NoDisp,
    Pending8,
    Pending16,
    Disp8(i8),
    Disp16(i16),
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

impl fmt::Display for Displacement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Displacement::Pending8 | Displacement::Pending16 | Displacement::NoDisp => write!(f,"Invalid Displacement"),
            Displacement::Disp8(i) => write!(f,"{:#04x}", i),
            Displacement::Disp16(i) => write!(f,"{:#06x}", i),
        }
    }
}

#[derive(Debug)]
pub enum RepType {
    NoRep,
    Rep,
    Repne,
    Repe
}
impl Default for RepType {
    fn default() -> Self { RepType::NoRep }
}
// pub enum RepDirection {
//     RepForward,
//     RepReverse
// }


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

#[derive (Copy, Clone)]
pub struct Instruction {
    pub(crate) opcode: u8,
    pub(crate) flags: u32,
    pub(crate) prefixes: u32,
    pub(crate) address: u32,
    pub(crate) size: u32,
    pub(crate) mnemonic: Mnemonic,
    pub(crate) segment_override: SegmentOverride,
    pub(crate) operand1_type: OperandType,
    pub(crate) operand1_size: OperandSize,
    pub(crate) operand2_type: OperandType,
    pub(crate) operand2_size: OperandSize,
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
    dtr: bool,
    ale: bool,
    pden: bool,
    den: bool
}

#[derive(Default)]
pub struct Cpu<'a> {
    
    cpu_type: CpuType,

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
    data_bus: u16,
    last_ea: u16,                   // Last calculated effective address
    bus: BusInterface,              // CPU owns Bus
    i8288: I8288,                   // Intel 8288 Bus Controller
    pc: u32,                        // Program counter points to the next instruction to be fetched

    queue: InstructionQueue,
    fetch_size: TransferSize,
    fetch_state: FetchState,
    fetch_delay: u32,               // Number of cycles until prefetch starts
    bus_pending_eu: bool,           // Has the EU requested a bus operation?
    last_queue_op: QueueOp,
    last_queue_byte: u8,
    t_cycle: TCycle,
    bus_status: BusStatus,
    bus_segment: Segment,
    transfer_size: TransferSize,    // Width of current bus transfer
    operand_size: OperandSize,     // Width of the operand being transferred. Prefetch 
    transfer_n: u32,                // Byte number of current operand (ex: 1/2 bytes of Word operand)
    wait_states: u32,

    // Bookkeeping
    halted: bool,
    is_running: bool,
    is_single_step: bool,
    is_error: bool,
    in_rep: bool,
    
    rep_mnemonic: Mnemonic,
    rep_type: RepType,
    rep_state: Vec<(u16, u16, RepState)>,
    error_string: String,
    cycle_num: u64,
    instr_cycle: u32,
    instruction_count: u64,
    i: Instruction,                 // Currently executing instruction 
    instruction_history: VecDeque<Instruction>,
    call_stack: VecDeque<CallStackEntry>,
    interrupt_inhibit: bool,
    reset_seg: u16,
    reset_offset: u16,

    trace_mode: TraceMode,
    trace_writer: Option<Box<dyn Write + 'a>>,
    trace_comment: &'static str,

    opcode0_counter: u32,

    rng: Option<rand::rngs::StdRng>,

    #[cfg(feature = "cpu_validator")]
    validator: Option<Box<dyn CpuValidator>>,
    #[cfg(feature = "cpu_validator")]
    cycle_states: Vec<CycleState>
}

pub struct CpuRegisterState {
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
    pub instruction_count: String,
}
    
pub enum RegisterType {
    Register8(u8),
    Register16(u16)
}

#[derive (PartialEq)]
pub enum ExecutionResult {

    Okay,
    OkayJump,
    OkayRep,
    UnsupportedOpcode(u8),
    ExecutionError(String),
    ExceptionError(CpuException),
    Halt
}

#[derive (Copy, Clone, Debug, PartialEq)]
pub enum TCycle {
    TInit,
    T1,
    T2,
    T3,
    Tw,
    T4
}

impl Default for TCycle {
    fn default() -> TCycle {
        TCycle::T1
    }
}

#[derive (Copy, Clone, Debug, PartialEq)]
pub enum BusStatus {
    InterruptAck = 0,   // IRQ Acknowledge
    IORead  = 1,        // IO Read
    IOWrite = 2,        // IO Write
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
    InProgress,
    Suspended,
    Scheduled(u8),
    Aborted(u8),
    BlockedByEU,
    BusBusy,
}

impl Default for FetchState {
    fn default() ->  FetchState {
        FetchState::Idle
    }
}

impl<'a> Cpu<'a> {

    pub fn new<TraceWriter: Write + 'a>(
        cpu_type: CpuType,
        trace_mode: TraceMode,
        trace_writer: Option<TraceWriter>,
        #[cfg(feature = "cpu_validator")]
        validator_type: ValidatorType
    ) -> Self {
        let mut cpu: Cpu = Default::default();
        
        match cpu_type {
            CpuType::Cpu8088 => {
                cpu.queue.set_size(4);
                cpu.fetch_size = TransferSize::Byte;
            }
            CpuType::Cpu8086 => {
                cpu.queue.set_size(6);
                cpu.fetch_size = TransferSize::Word;
            }
        }

        #[cfg(feature = "cpu_validator")] 
        {
            cpu.validator = match validator_type {
                #[cfg(feature = "pi_validator")]
                ValidatorType::Pi8088 => {
                    Some(Box::new(PiValidator::new()))
                }
                #[cfg(feature = "arduino_validator")]
                ValidatorType::Arduino8088 => {
                    Some(Box::new(ArduinoValidator::new()))
                }
                _=> {
                    None
                }
            };

            if let Some(ref mut validator) = cpu.validator {
                match validator.init(true, true, true) {
                    true => {},
                    false => {
                        panic!("Failed to init cpu validator.");
                    }
                }
            }            
        }

        cpu.trace_mode = trace_mode;
        // Unwrap the writer Option and stick it in an Option<Box<>> or None if None
        cpu.trace_writer = trace_writer.map_or(None, |trace_writer| Some(Box::new(trace_writer)));
        cpu.cpu_type = cpu_type;
        cpu.instruction_history = VecDeque::with_capacity(16);
        cpu.reset_seg = 0xFFFF;
        cpu.reset_offset = 0x0000;
        cpu.reset(cpu.reset_seg, cpu.reset_offset);
        cpu
    }

    pub fn reset(&mut self, segment: u16, offset: u16) {
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
        
        self.set_register16(Register16::CS, segment);
        self.set_register16(Register16::IP, offset);

        self.flags = CPU_FLAGS_RESERVED_ON;
        
        // Reset BIU
        self.queue.flush();
        self.pc = Cpu::calc_linear_address(segment, offset);
        self.bus_status = BusStatus::Passive;
        self.t_cycle = TCycle::T1;
        
        self.instruction_count = 0; 
        
        self.in_rep = false;
        self.rep_state.clear();
        self.halted = false;
        self.opcode0_counter = 0;
        self.interrupt_inhibit = false;
        self.is_error = false;
        self.instruction_history.clear();
        self.call_stack.clear();

        self.cycle_num = 1;
        self.i8288.ale = false;
        self.i8288.mrdc = false;
        self.i8288.amwc = false;
        self.i8288.mwtc = false;
        self.i8288.iorc = false;
        self.i8288.aiowc = false;
        self.i8288.iowc = false;

        self.address_bus = 0;

        // Reset takes 7 cycles
        self.biu_suspend_fetch();
        self.cycles(4);
        self.biu_queue_flush();
        self.cycles(2);

        self.trace_print(&format!("Reset CPU! CS: {:04X} IP: {:04X}", self.cs, self.ip));
    }

    pub fn bus(&self) -> &BusInterface {
        &self.bus
    }

    pub fn bus_mut(&mut self) -> &mut BusInterface {
        &mut self.bus
    }

    /// If in an active bus cycle, cycle the cpu until the bus cycle has reached T4.
    pub fn bus_wait_finish(&mut self) -> u32 {
        let mut bus_cycles_elapsed = 0;
        match self.bus_status {
            BusStatus::Passive => {
                // No active bus transfer
                return 0
            }
            BusStatus::MemRead | BusStatus::MemWrite | BusStatus::IORead | BusStatus::IOWrite | BusStatus::CodeFetch => {
                while self.t_cycle != TCycle::T4 {
                    self.cycle();
                    bus_cycles_elapsed += 1;
                }
                //self.cycle();
                return bus_cycles_elapsed
            }
            _ => {
                // Handle other statuses
                return 0
            }
        }
    }

    /// If in an active bus cycle, cycle the CPU until the target T-state is reached.
    /// This function is usually used on a terminal write to wait for T2 to process RNI
    pub fn bus_wait_until(&mut self, target_state: TCycle) -> u32 {
        let mut bus_cycles_elapsed = 0;
        match self.bus_status {
            BusStatus::Passive => {
                // No active bus transfer
                return 0
            }
            BusStatus::MemRead | BusStatus::MemWrite | BusStatus::IORead | BusStatus::IOWrite | BusStatus::CodeFetch => {
                while self.t_cycle != target_state {
                    self.cycle();
                    bus_cycles_elapsed += 1;
                }
                //self.cycle();
                return bus_cycles_elapsed
            }
            _ => {
                // Handle other statuses
                return 0
            }
        }
    }    

    /// Begin a new bus cycle of the specified type. Set the address latch and set the data bus appropriately.
    pub fn bus_begin(
        &mut self, 
        new_bus_status: BusStatus, 
        bus_segment: Segment, 
        address: u32, 
        data: u16, 
        size: TransferSize,
        op_size: OperandSize,
        first: bool,
    ) {
        // We can cancel a scheduled fetch if it happened this cycle.
        self.biu_try_cancel_fetch();

        self.trace_print(&format!("Bus begin! {:?}:[{:05X}]", new_bus_status, address));

        if new_bus_status != BusStatus::CodeFetch {
            // The EU has requested a Read/Write cycle, if we haven't scheduled a prefetch, block 
            // prefetching until the bus transfer is complete.

            if self.is_before_last_wait() && self.fetch_state != FetchState::Suspended {
                self.trace_print(&format!("Blocking fetch: T:{:?}", self.t_cycle));
                self.fetch_state = FetchState::BlockedByEU;
                self.bus_pending_eu = true;
            }
        }

        // Wait for the current bus cycle to terminate.
        self.bus_wait_finish();

        if self.fetch_state == FetchState::BlockedByEU {
            self.fetch_state = FetchState::Idle;
        }
        
        self.bus_pending_eu = false;

        // Reset the transfer number if this is the first transfer of a word
        if first {
            self.transfer_n = 0;
        }        
        
        //log::trace!("Bus begin! {:?}:[{:05X}]", bus_status, address);

        if new_bus_status == BusStatus::CodeFetch {
            // Prefetch is starting so reset prefetch scheculed flag
            self.fetch_state = FetchState::InProgress;
            self.transfer_n = 0;
        }

        if self.bus_status == BusStatus::Passive || self.t_cycle == TCycle::T4 {

            let fetch_scheduled = if let FetchState::Scheduled(_) = self.fetch_state { true } else { false };

            if new_bus_status != BusStatus::CodeFetch && fetch_scheduled {
                // A fetch was scheduled already, so we have to abort and incur a two cycle penalty.
                //self.trace_print("Aborting prefetch!");
                self.biu_abort_fetch(); 
            }
            self.bus_status = new_bus_status;
            self.bus_segment = bus_segment;
            self.t_cycle = TCycle::TInit;
            self.address_bus = address;
            self.i8288.ale = true;
            self.data_bus = data as u16;
            self.transfer_size = size;
            self.operand_size = op_size;
            if self.transfer_n > 1 {
                self.transfer_n = 0;
            }
        }
        else {
            panic!("bus_begin: Attempted to start bus cycle in unhandled state: {:?}:{:?}", self.bus_status, self.t_cycle);
        }

        /*
        match self.bus_status {
            BusStatus::Passive => {
                // Since we waited for previous bus transfer to finish this should be the default state
                self.bus_status = bus_status;
                self.cycle_state = CycleState::TInit;
                self.address_bus = address;
                self.i8288.ale = true;
                self.data_bus = data as u16;
                self.transfer_size = size;
            }
            _ => {
                log::warn!("bus_begin8: Attempted to start bus cycle in unhandled state: {:?}", self.bus_status);
                // Handle other statuses
            }
        }
        */
    }

    pub fn bus_end(&mut self) {

        // Reset i8288 signals
        self.i8288.mrdc = false;
        self.i8288.amwc = false;
        self.i8288.mwtc = false;
        self.i8288.iorc = false;
        self.i8288.aiowc = false;
        self.i8288.iowc = false;

        self.bus_pending_eu = false;
    }

    #[inline]
    pub fn is_last_wait(&self) -> bool {
        match self.t_cycle {
            TCycle::T3 | TCycle::Tw => {
                if self.wait_states == 0 {
                    true
                }
                else {
                    false
                }
            }
            _ => false
        }
    }

    pub fn is_before_last_wait(&self) -> bool {
        match self.t_cycle {
            TCycle::T1 | TCycle::T2 => true,
            TCycle::T3 | TCycle::Tw => {
                if self.wait_states != 0 {
                    true
                }
                else {
                    false
                }
            }
            _ => false
        }

    }

    pub fn is_operand_complete(&self) -> bool {
        match self.operand_size {
            OperandSize::Operand8 => {
                self.transfer_n == 1
            }
            OperandSize::Operand16 => {
                self.transfer_n == 2
            }
            _ => true
        }
    }

    pub fn cycle(&mut self) {

        let byte;

        // Bus is idle, or previous bus cycle is ending. Make a prefetch decision.

        //self.trace_print(&format!("{:?}", self.is_operand_complete()));
        //self.trace_print(&format!("{:?}, {:?}, {:?}, {}", self.fetch_suspended, self.bus_status, self.t_cycle, self.fetch_delay));
        
        //self.trace_print(&format!("cycle(): {:?} bus_status: {:?}", self.fetch_state, self.bus_status));

        // Transition to next t-state

        //self.trace_print(&format!("t_cycle: {:?}", self.t_cycle));

        self.t_cycle = match self.t_cycle {
            TCycle::TInit => {
                // A new bus cycle has been initiated, begin it
                TCycle::T1
            }
            TCycle::T1 => {
                match self.bus_status {
                    BusStatus::Passive => TCycle::T1,
                    BusStatus::MemRead | BusStatus::MemWrite | BusStatus::IORead | BusStatus::IOWrite | BusStatus::CodeFetch => {
                        TCycle::T2
                    },
                    _=> self.t_cycle
                }
            }
            TCycle::T2 => TCycle::T3,
            TCycle::T3 => {
                if self.wait_states == 0 {
                    self.bus_end();
                    TCycle::T4
                }
                else {
                    TCycle::Tw
                }
            }
            TCycle::Tw => {
                if self.wait_states > 0 {
                    self.wait_states -= 1;
                    TCycle::Tw
                }
                else {
                    self.bus_end();
                    TCycle::T4
                }                
            }
            TCycle::T4 => {

                //self.trace_print(&format!("In T4: {:?}, {:?}", self.bus_status, self.transfer_size));
                
                self.bus_status = BusStatus::Passive;
                TCycle::T1
            }            
        };

        match self.fetch_state {
            FetchState::Scheduled(2) => {
                //self.trace_print("Scheduled fetch!");
                if self.biu_queue_has_room() {

                    self.fetch_state = FetchState::InProgress;

                    /*
                    self.bus_begin(
                        BusStatus::CodeFetch, 
                        Segment::CS, 
                        self.pc, 
                        0, 
                        self.fetch_size,
                        OperandSize::Operand8,
                        true
                    );
                    */

                    self.bus_status = BusStatus::CodeFetch;
                    self.bus_segment = Segment::CS;
                    self.t_cycle = TCycle::T1;
                    self.address_bus = self.pc;
                    self.i8288.ale = true;
                    self.data_bus = 0;
                    self.transfer_size = self.fetch_size;
                    self.operand_size = match self.fetch_size {
                        TransferSize::Byte => OperandSize::Operand8,
                        TransferSize::Word => OperandSize::Operand16
                    };
                    self.transfer_n = 0;
                }
                else {
                    self.fetch_state = FetchState::Idle;
                    self.trace_print(&format!("Fetch cancelled"));
                }
            }
            FetchState::Idle => {
                if (self.bus_status == BusStatus::Passive) && (self.t_cycle == TCycle::T1) {
                    // Nothing is scheduled, suspended, aborted, and bus is idle. Make a prefetch decision.
                    self.biu_make_fetch_decision();
                }
            }
            _ => {}
        }

        // Operate the new t-state
        match self.bus_status {
            BusStatus::Passive => {
                self.transfer_n = 0;
            }
            BusStatus::MemRead | BusStatus::MemWrite | BusStatus::IORead | BusStatus::IOWrite | BusStatus::CodeFetch => {
                match self.t_cycle {
                    TCycle::TInit => {
                        panic!("Can't execute TInit state");
                    },
                    TCycle::T1 => {
                    },
                    TCycle::T2 => {
                        // Turn off ale signal on T2
                        self.i8288.ale = false;

                        // Read/write signals go high on T2.
                        match self.bus_status {
                            BusStatus::CodeFetch | BusStatus::MemRead => {
                                self.i8288.mrdc = true;
                            }
                            BusStatus::MemWrite => {
                                // Only AMWC goes high on T2. MWTC delayed to T3.
                                self.i8288.amwc = true;
                            }
                            BusStatus::IORead => {
                                self.i8288.iorc = true;
                            }
                            BusStatus::IOWrite => {
                                // Only AIOWC goes high on T2. IOWC delayed to T3.
                                self.i8288.aiowc = true;
                            }
                            _ => {}
                        }
                    }
                    TCycle::T3 => {
                        // Reading/writing occurs on T3. The READY handshake is not simulated, instead the BusInterface
                        // methods will return the number of wait states appropriate for each read/write.
                        match (self.bus_status, self.transfer_size) {
                            (BusStatus::CodeFetch, TransferSize::Byte) => {
                                (byte, self.wait_states) = self.bus.read_u8(self.address_bus as usize).unwrap();
                                self.data_bus = byte as u16;
                                self.transfer_n += 1;
                            }
                            (BusStatus::CodeFetch, TransferSize::Word) => {
                                (self.data_bus, self.wait_states) = self.bus.read_u16(self.address_bus as usize).unwrap();
                                self.transfer_n += 1;
                            }
                            (BusStatus::MemRead, TransferSize::Byte) => {
                                (byte, self.wait_states) = self.bus.read_u8(self.address_bus as usize).unwrap();
                                self.data_bus = byte as u16;
                                self.transfer_n += 1;
                            }
                            (BusStatus::MemRead, TransferSize::Word) => {
                                (self.data_bus, self.wait_states) = self.bus.read_u16(self.address_bus as usize).unwrap();
                                self.transfer_n += 1;
                            }
                            (BusStatus::MemWrite, TransferSize::Byte) => {
                                self.i8288.mwtc = true;
                                self.bus.write_u8(self.address_bus as usize, (self.data_bus & 0x00FF) as u8).unwrap();
                                self.transfer_n += 1;
                            }
                            (BusStatus::MemWrite, TransferSize::Word) => {
                                self.i8288.mwtc = true;
                                self.bus.write_u16(self.address_bus as usize, self.data_bus).unwrap();
                                self.transfer_n += 1;
                            }                                                          
                            _=> {
                                // Handle other bus operations
                            }
                        }

                        if self.is_last_wait() && self.is_operand_complete() {
                            self.biu_make_fetch_decision();
                        }
                    }
                    TCycle::Tw => {
                        if self.is_last_wait() && self.is_operand_complete() {
                            self.biu_make_fetch_decision();
                        }                        
                    }
                    TCycle::T4 => {
                        match (self.bus_status, self.transfer_size) {
                            (BusStatus::CodeFetch, TransferSize::Byte) => {
                                //log::debug!("Code fetch completed!");
                                //log::debug!("Pushed byte {:02X} to queue!", self.data_bus as u8);
                                self.queue.push8(self.data_bus as u8);
                                self.pc = (self.pc + 1) & 0xFFFFFu32;
                            }
                            (BusStatus::CodeFetch, TransferSize::Word) => {
                                self.queue.push16(self.data_bus);
                                self.pc = (self.pc + 2) & 0xFFFFFu32;
                            }
                            _=> {}                        
                        }
                    }
                }
            }
            _ => {
                // Handle other states
            }
        };

        // Perform cycle tracing, if enabled
        if self.trace_mode == TraceMode::Cycle {
            self.trace_print(&self.cycle_state_string());   
        }

        #[cfg(feature = "cpu_validator")]
        {
            let cycle_state = self.get_cycle_state();
            self.cycle_states.push(cycle_state);
        }

        // Decrement fetch delay.
        self.biu_tick_prefetcher();

        //self.fetch_delay = self.fetch_delay.saturating_sub(1);

        // Reset queue operation
        self.last_queue_op = QueueOp::Idle;
        self.last_queue_byte = 0;

        self.instr_cycle += 1 ;
        self.cycle_num += 1;

        self.trace_comment = ""; 
    }

    #[inline]
    pub fn cycle_nx(&self) {
        // Do nothing
    }

    #[inline]
    pub fn cycles(&mut self, ct: u32) {
        for _ in 0..ct {
            self.cycle();
        }
    }

    #[inline]
    pub fn cycles_nx(&mut self, ct: u32) {
        self.cycles(ct - 1);
    }

    /// Finalize an instruction that has terminated before there is a new byte in the queue.
    /// This will cycle the CPU until a byte is available in the instruction queue, then fetch it.
    /// This fetched byte is considered 'preloaded' by the queue.
    pub fn finalize(&mut self) {

        self.trace_comment("FINALIZE");

        if self.queue.len() == 0 {
            while { 
                self.trace_print("No byte in queue");
                self.cycle();
                self.queue.len() == 0
            } {}
            // Should be a byte in the queue now. Preload it
            self.queue.set_preload();
            self.trace_comment("FINALIZE_END");
            self.cycle();
        }
        else {
            self.queue.set_preload();
            self.trace_comment("FINALIZE_END");
            self.cycle();
        }

        
    }

    #[cfg(feature = "cpu_validator")]
    pub fn get_cycle_state(&mut self) -> CycleState {

        CycleState {
            n: self.instr_cycle,
            addr: self.address_bus,
            t_state: match self.t_cycle {
                TCycle::TInit | TCycle::T1 => BusCycle::T1,
                TCycle::T2 => BusCycle::T2,
                TCycle::T3 => BusCycle::T3,
                TCycle::Tw => BusCycle::Tw,
                TCycle::T4 => BusCycle::T4
            },
            a_type: match self.bus_segment { 
                Segment::ES => AccessType::AccAlternateData,
                Segment::SS => AccessType::AccStack,
                Segment::DS => AccessType::AccData,
                Segment::None | Segment::CS => AccessType::AccCodeOrNone,
            },
            // Unify these enums?
            b_state: match self.t_cycle {
                
                TCycle::T1 | TCycle::T2 => match self.bus_status {
                        BusStatus::InterruptAck => BusState::INTA,
                        BusStatus::IORead => BusState::IOR,
                        BusStatus::IOWrite => BusState::IOW,
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
            data_bus: self.data_bus,
        }
    }

    pub fn is_error(&self) -> bool {
        self.is_error
    }

    pub fn error_string(&self) -> &str {
        &self.error_string
    }

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

    pub fn set_reset_vector(&mut self, segment: u16, offset: u16) {
        self.reset_seg = segment;
        self.reset_offset = offset;
    }

    pub fn get_reset_vector(&self) -> (u16, u16) {
        (self.reset_seg, self.reset_offset)
    }

    pub fn reset_address(&mut self) {
        self.cs = self.reset_seg;
        self.ip = self.reset_offset;
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
            
            flags: format!("{:04}", self.flags),
            instruction_count: format!("{}", self.instruction_count)
        }
    }
    
    pub fn eval_address(&self, expr: &str) -> Option<u32> {

        lazy_static! {
            static ref FLAT_REX: Regex = Regex::new(r"(?P<flat>[A-Fa-f\d]{5})$").unwrap();
            static ref SEGMENTED_REX: Regex = Regex::new(r"(?P<segment>[A-Fa-f\d]{4}):(?P<offset>[A-Fa-f\d]{4})$").unwrap();
            static ref REGREG_REX: Regex = Regex::new(r"(?P<reg1>cs|ds|ss|es):(?P<reg2>\w{2})$").unwrap();
            static ref REGOFFSET_REX: Regex = Regex::new(r"(?P<reg1>cs|ds|ss|es):(?P<offset>[A-Fa-f\d]{4})$").unwrap();
        }

        if FLAT_REX.is_match(expr) {
            match u32::from_str_radix(expr, 16) {
                Ok(address) => Some(address),
                Err(_) => None
            }     
        }
        else if let Some(caps) = SEGMENTED_REX.captures(expr) {
            let segment_str = &caps["segment"];
            let offset_str = &caps["offset"];
            
            let segment_u16r = u16::from_str_radix(segment_str, 16);
            let offset_u16r = u16::from_str_radix(offset_str, 16);

            match(segment_u16r, offset_u16r) {
                (Ok(segment),Ok(offset)) => Some(Cpu::calc_linear_address(segment,offset)),
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

            Some(Cpu::calc_linear_address(segment, offset))
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
                Ok(offset) => Some(Cpu::calc_linear_address(segment, offset)),
                _ => None
            }
        }
        else {
            None
        }

    }

    pub fn end_interrupt(&mut self) {

        self.pop_register16(Register16::IP, ReadWriteFlag::Normal);
        self.pop_register16(Register16::CS, ReadWriteFlag::Normal);
        //log::trace!("CPU: Return from interrupt to [{:04X}:{:04X}]", self.cs, self.ip);
        self.pop_flags();

        // temporary timings
        self.biu_suspend_fetch();
        self.cycles(4);
        self.biu_queue_flush();        
    }

    /// Perform a software interrupt
    pub fn sw_interrupt(&mut self, interrupt: u8) {

        // When an interrupt occurs the following happens:
        // 1. CPU pushes flags register to stack
        // 2. CPU pushes far return address into the stack
        // 3. CPU fetches the four byte interrupt vector from the IVT
        // 4. CPU transfers control to the routine specified by the interrupt vector
        // (AoA 17.1)

        self.push_flags(ReadWriteFlag::Normal);

        // Push return address of next instruction onto stack (INT instructions should increment IP on execute)
        self.push_register16(Register16::CS, ReadWriteFlag::Normal);
        self.push_register16(Register16::IP, ReadWriteFlag::Normal);
        
        // Clear interrupt flag
        self.clear_flag(Flag::Interrupt);
        // Clear trap flag
        self.clear_flag(Flag::Trap);
        // Clear auxilliary carry (?)
        self.clear_flag(Flag::AuxCarry);

        // Read the IVT
        let ivt_addr = Cpu::calc_linear_address(0x0000, (interrupt as usize * INTERRUPT_VEC_LEN) as u16);

        let new_ip = self.biu_read_u16(Segment::None, ivt_addr, ReadWriteFlag::Normal);
        let new_cs = self.biu_read_u16(Segment::None, ivt_addr + 2, ReadWriteFlag::Normal);

        //let (new_ip, _cost) = self.bus.read_u16(ivt_addr as usize).unwrap();
        //let (new_cs, _cost) = self.bus.read_u16((ivt_addr + 2) as usize ).unwrap();

        if interrupt == 0x13 {
            // Disk interrupts
            if self.dl & 0x80 != 0 {
                // Hard disk request
                match self.ah {
                    0x03 => {
                        log::trace!("Hard disk int13h: Write Sectors: Num: {} Drive: {:02X} C: {} H: {} S: {}",
                            self.al,
                            self.dl,
                            self.ch,
                            self.dh,
                            self.cl)
                    }
                    _=> log::trace!("Hard disk requested in int13h. AH: {:02X}", self.ah)
                }
                
            }
        }

        if interrupt == 0x10 && self.ah==0x00 {
            log::trace!("CPU: int10h: Set Mode {:02X} Return [{:04X}:{:04X}]", interrupt, self.cs, self.ip);
        }        

        if interrupt == 0x21 {
            //log::trace!("CPU: int21h: AH: {:02X} [{:04X}:{:04X}]", self.ah, self.cs, self.ip);
            if self.ah == 0x4B {
                log::trace!("EXEC/Load and Execute Program");
            }
        }         

        self.ip = new_ip;
        self.cs = new_cs;        

        // Flush queue
        self.biu_queue_flush();
        self.biu_update_pc();                
    }

    /// Handle a CPU exception
    pub fn handle_exception(&mut self, exception: u8) {

        // 
        self.push_flags(ReadWriteFlag::Normal);

        // Push return address of next instruction onto stack
        self.push_register16(Register16::CS, ReadWriteFlag::Normal);

        // Don't push address of next instruction
        self.push_u16(self.ip, ReadWriteFlag::Normal);
        
        if exception == 0x0 {
            log::trace!("CPU Exception: {:02X} Saving return: {:04X}:{:04X}", exception, self.cs, self.ip);
        }
        // Read the IVT
        let ivt_addr = Cpu::calc_linear_address(0x0000, (exception as usize * INTERRUPT_VEC_LEN) as u16);
        let (new_ip, _cost) = self.bus.read_u16(ivt_addr as usize).unwrap();
        let (new_cs, _cost) = self.bus.read_u16((ivt_addr + 2) as usize ).unwrap();
        self.ip = new_ip;
        self.cs = new_cs;

        // Flush queue
        self.biu_queue_flush();
        self.biu_update_pc();        
    }    

    pub fn log_interrupt(&self, interrupt: u8) {

        match interrupt {
            0x10 => {
                // Video Services
                match self.ah {
                    0x00 => {
                        log::trace!("CPU: Video Interrupt: {:02X} (AH:{:02X} Set video mode) Video Mode: {:02X}", 
                            interrupt, self.ah, self.al);
                    }
                    0x01 => {
                        log::trace!("CPU: Video Interrupt: {:02X} (AH:{:02X} Set text-mode cursor shape: CH:{:02X}, CL:{:02X})", 
                            interrupt, self.ah, self.ch, self.cl);
                    }
                    0x02 => {
                        log::trace!("CPU: Video Interrupt: {:02X} (AH:{:02X} Set cursor position): Page:{:02X} Row:{:02X} Col:{:02X}",
                            interrupt, self.ah, self.bh, self.dh, self.dl);
                        
                        if self.dh == 0xFF {
                            log::trace!(" >>>>>>>>>>>>>>>>>> Row was set to 0xff at address [{:04X}:{:04X}]", self.cs, self.ip);
                        }
                    }
                    0x09 => {
                        log::trace!("CPU: Video Interrupt: {:02X} (AH:{:02X} Write character and attribute): Char:'{}' Page:{:02X} Color:{:02x} Ct:{:02}", 
                            interrupt, self.ah, self.al as char, self.bh, self.bl, self.cx);
                    }
                    0x10 => {
                        log::trace!("CPU: Video Interrupt: {:02X} (AH:{:02X} Write character): Char:'{}' Page:{:02X} Ct:{:02}", 
                            interrupt, self.ah, self.al as char, self.bh, self.cx);
                    }
                    _ => {}
                }
            }
            _ => {}
        };
    }

    /// Perform a hardware interrupt
    pub fn hw_interrupt(&mut self, interrupt: u8) {

        // When an interrupt occurs the following happens:
        // 1. CPU pushes flags register to stack
        // 2. CPU pushes far return address into the stack
        // 3. CPU fetches the four byte interrupt vector from the IVT
        // 4. CPU transfers control to the routine specified by the interrupt vector
        // (AoA 17.1)

        self.push_flags(ReadWriteFlag::Normal);
        // Push cs:ip return address to stack
        self.push_register16(Register16::CS, ReadWriteFlag::Normal);
        self.push_register16(Register16::IP, ReadWriteFlag::Normal);

        // If we are in a repeated string instruction (REP prefix) we need to save the state of the REP instruction
        if self.in_rep {
            let (src_reg, src_reg_val) = match self.i.segment_override {
                SegmentOverride::CS => (Register16::CS, self.cs),
                SegmentOverride::ES => (Register16::ES, self.es),
                SegmentOverride::SS => (Register16::SS, self.ss),
                _=> (Register16::DS, self.ds)
            };

            let state: RepState = match self.i.mnemonic {
                Mnemonic::LODSB => {
                    RepState::LodsbState(src_reg, src_reg_val, self.si, self.cx)
                }
                Mnemonic::LODSW => {
                    RepState::LodswState(src_reg, src_reg_val, self.si, self.cx)
                } 
                Mnemonic::MOVSB => {
                    RepState::MovsbState(src_reg, src_reg_val, self.si, self.es, self.di, self.cx)
                } 
                Mnemonic::MOVSW => {
                    RepState::MovswState(src_reg, src_reg_val, self.si, self.es, self.di, self.cx)
                } 
                Mnemonic::CMPSB => {
                    RepState::CmpsbState(src_reg, src_reg_val, self.si, self.es, self.di, self.cx)
                } 
                Mnemonic::CMPSW => {
                    RepState::CmpswState(src_reg, src_reg_val, self.si, self.es, self.di, self.cx)
                }
                Mnemonic::STOSB => {
                    RepState::StosbState(self.es, self.di, self.cx)
                } 
                Mnemonic::STOSW => {
                    RepState::StoswState(self.es, self.di, self.cx)
                } 
                Mnemonic::SCASB => {
                    RepState::ScasbState(self.es, self.di, self.cx)
                } 
                Mnemonic::SCASW => {
                    RepState::ScaswState(self.es, self.di, self.cx)
                }
                _=> {
                    log::warn!("Invalid instruction saving REP state: {:?}", self.i.mnemonic);
                    RepState::NoState
                }
            };

            self.rep_state.push((self.cs, self.ip, state));
            self.in_rep = false;
        }

        // Read the IVT
        let ivt_addr = Cpu::calc_linear_address(0x0000, (interrupt as usize * INTERRUPT_VEC_LEN) as u16);
        let (new_ip, _cost) = self.bus.read_u16(ivt_addr as usize).unwrap();
        let (new_cs, _cost) = self.bus.read_u16((ivt_addr + 2) as usize ).unwrap();
        self.ip = new_ip;
        self.cs = new_cs;

        // Flush queue
        self.biu_queue_flush();
        self.biu_update_pc();

        // timer interrupt to noisy to log
        if interrupt != 8 {
            self.log_interrupt(interrupt);
            //log::trace!("CPU: Interrupt: {} Saving return [{:04X}:{:04X}]", interrupt, self.cs, self.ip);
            //log::trace!("CPU: Hardware Interrupt: {} Jumping to IV [{:04X}:{:04X}]", interrupt, new_cs, new_ip);
        }

    }

    /// Return true if an interrupt can occur under current execution state
    pub fn interrupts_enabled(&self) -> bool {
        self.get_flag(Flag::Interrupt) && !self.interrupt_inhibit
    }
    
    /// Resume from halted state
    pub fn resume(&mut self) {
        if self.halted {
            log::trace!("Resuming from halt");
        }
        self.halted = false;
    }

    pub fn step(&mut self, io_bus: &mut IoBusInterface, pic_ref: Rc<RefCell<Pic>>) -> Result<u32, CpuError> {

        self.instr_cycle = 0;

        self.trace_print(&format!("STEP"));

        // When halted, the CPU waits for an interrupt to fire before resuming execution
        if self.halted {
            
            // Check for hardware interrupts if Interrupt Flag is set and not in wait cycle
            if self.interrupts_enabled() {
                let mut pic = pic_ref.borrow_mut();
                if pic.query_interrupt_line() {
                    match pic.get_interrupt_vector() {
                        Some(irq) => {
                            self.hw_interrupt(irq);
                            self.resume();
                        },
                        None => {}
                    }
                }
            }

            return Ok(1)
        }

        // A real 808X CPU maintains a single Program Counter or PC register that points to the next instruction
        // to be fetched, not the currently executing instruction. This value is "corrected" whenever the current
        // value of IP is required, ie, pushing IP to the stack. This is performed by the 'CORR' microcode routine.

        // It is more convenient for us to maintain IP as a separate register that always points to the current
        // instruction. Otherwise, when single-stepping in the debugger, the IP value will read ahead. 
        let instruction_address = Cpu::calc_linear_address(self.cs, self.ip);

        // Fetch the next instruction unless we are executing a REP
        if !self.in_rep {
            
            #[cfg(feature = "cpu_validator")]
            {
                self.cycle_states.clear();

                // Begin validation of current instruction
                let vregs = self.get_vregisters();
                if let Some(ref mut validator) = self.validator {
                    validator.begin(&vregs);
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
            
            // Fetch and decode the current instruction. This uses the CPU's own ByteQueue trait implementation, 
            // which fetches instruction bytes through the processor instruction queue.
            
            //self.trace_comment("FETCH");
            self.trace_print(&format!("FETCH {:05X}", self.pc));
            self.i = match Cpu::decode(self) {
                Ok(i) => i,
                Err(_) => {
                    self.is_running = false;
                    self.is_error = true;
                    return Err(CpuError::InstructionDecodeError(instruction_address))
                }                
            };
            self.trace_comment("EXECUTE");
            
        }

        // Since Cpu::decode doesn't know anything about the current IP, it can't set it, so we do that now.
        self.i.address = instruction_address;
        let mut check_interrupts = false;

        let (opcode, _cost) = self.bus.read_u8(instruction_address as usize).expect("mem err");
        
        self.trace_print(&format!("Fetched instruction: {} op:{:02X} at [{:05X}]", self.i, opcode, self.i.address));
        self.trace_print(&format!("Executing instruction:  [{:04X}:{:04X}] {} ({})", self.cs, self.ip, self.i, self.i.size));

        let exec_result = self.execute_instruction(io_bus);
        self.finalize();

        #[cfg(feature = "cpu_validator")]
        {
            match exec_result {
                ExecutionResult::Okay | ExecutionResult::OkayJump => {
                    // End validation of current instruction
                    let mut vregs = self.get_vregisters();

                    if exec_result == ExecutionResult::Okay {
                        vregs.ip = self.ip.wrapping_add(self.i.size as u16);
                    }
                    
                    let instr_slice = self.bus.get_slice_at(instruction_address as usize, self.i.size as usize);

                    if self.i.size == 0 {
                        log::error!("Invalid length: [{:05X}] {}", instruction_address, self.i);
                    }

                    if let Some(ref mut validator) = self.validator {
                        match validator.validate(
                            self.i.to_string(), 
                            &instr_slice,
                            self.i.flags & INSTRUCTION_HAS_MODRM != 0,
                            0,
                            &vregs,
                            &self.cycle_states
                        ) {

                            Ok(_) => {},
                            Err(e) => {
                                log::debug!("Validation failure: {} Halting execution.", e);
                                self.is_running = false;
                                self.is_error = true;
                                return Err(CpuError::CpuHaltedError(instruction_address))
                            }
                        }


                    }                    
                }
                _ => {}
            }            
        }

       let cpu_result = match exec_result {

            ExecutionResult::Okay => {
                // Normal non-jump instruction updates CS:IP to next instruction

                // temp debugging
                {
                    //let dbg_addr = self.calc_linear_address_seg(Segment::ES, self.bx);
                    let (word, _) = self.bus.read_u16(0x2905C as usize).unwrap();
                    if word == 0xCCCC {
                        log::trace!("Jump target trashed at {:05X}: {}", self.i.address, self.i);
                    }
                }
                
                
                self.ip = self.ip.wrapping_add(self.i.size as u16);

                //log::trace!("Execution complete; updated CS:IP: [{:04X}:{:04X}]", self.cs, self.ip);

                if self.instruction_history.len() == CPU_HISTORY_LEN {
                    self.instruction_history.pop_front();
                }
                self.instruction_history.push_back(self.i);
                self.instruction_count += 1;
                check_interrupts = true;

                // Perform instruction tracing, if enabled
                if self.trace_mode == TraceMode::Instruction {
                    self.trace_print(&self.instruction_state_string());   
                }                

                Ok(self.instr_cycle + 1)
            }
            ExecutionResult::OkayJump => {

                // Flush PIQ on jump
                
                //self.biu_queue_flush();
                //self.biu_update_pc();

                if self.instruction_history.len() == CPU_HISTORY_LEN {
                    self.instruction_history.pop_front();
                }
                self.instruction_history.push_back(self.i);
                self.instruction_count += 1;

                check_interrupts = true;

                // Perform instruction tracing, if enabled
                if self.trace_mode == TraceMode::Instruction {
                    self.trace_print(&self.instruction_state_string());   
                }                          
                Ok(self.instr_cycle + 1)
            }
            ExecutionResult::OkayRep => {
                // We are in a REPx-prefixed instruction.
                // The ip will not increment until the instruction has completed
                // But process interrupts
                if self.instruction_history.len() == CPU_HISTORY_LEN {
                    self.instruction_history.pop_front();
                }
                self.instruction_history.push_back(self.i);
                self.instruction_count += 1;
                check_interrupts = true;
                Ok(self.instr_cycle + 1)
            }                    
            ExecutionResult::UnsupportedOpcode(o) => {
                self.is_running = false;
                self.is_error = true;
                Err(CpuError::UnhandledInstructionError(o, instruction_address))
            }
            ExecutionResult::ExecutionError(e) => {
                self.is_running = false;
                self.is_error = true;
                Err(CpuError::ExecutionError(instruction_address, e))
            }
            ExecutionResult::Halt => {
                // Specifically, this error condition is a halt with interrupts disabled -
                // execution cannot continue. This state is most encountered during BIOS
                // initialization.
                self.is_running = false;
                self.is_error = true;
                Err(CpuError::CpuHaltedError(instruction_address))
            }
            ExecutionResult::ExceptionError(exception) => {
                // Handle DIV by 0 here
                match exception {
                    CpuException::DivideError => {
                        self.handle_exception(0);
                        Ok(self.instr_cycle + 1)
                    }
                    _ => {
                        // Unhandled exception?
                        Err(CpuError::ExceptionError(exception))
                    }
                }
            }
        };

        if check_interrupts {
            // Check for hardware interrupts if Interrupt Flag is set and not in wait cycle
            if self.interrupts_enabled() {
                let mut pic = pic_ref.borrow_mut();
                if pic.query_interrupt_line() {
                    match pic.get_interrupt_vector() {
                        Some(irq) => {
                            self.hw_interrupt(irq);
                            self.resume();
                        },
                        None => {}
                    }
                }
            }
        }

        #[cfg(debug_assertions)]        
        self.assert_state();

        cpu_result
    }

    pub fn dump_instruction_history(&self) -> String {

        let mut disassembly_string = String::new();

        for i in &self.instruction_history {
            let i_string = format!("{:05X} {}\n", i.address, i);
            disassembly_string.push_str(&i_string);
        }
        disassembly_string
    }

    pub fn dump_call_stack(&self) -> String {
        let mut call_stack_string = String::new();

        for call in &self.call_stack {
            match call {
                CallStackEntry::Call(cs,ip,rel16) => {
                    call_stack_string.push_str(&format!("{:04X}:{:04X} CALL {:04X}\n", cs, ip, rel16));
                }
                CallStackEntry::CallF(cs,ip,seg,off) => {
                    call_stack_string.push_str(&format!("{:04X}:{:04X} CALL FAR {:04X}:{:04X}\n", cs, ip, seg, off));
                }
                CallStackEntry::Interrupt(cs, ip, inum) => {
                    call_stack_string.push_str(&format!("{:04X}:{:04X} INT {:02X}\n", cs, ip, inum));
                }
            }   
        }

        call_stack_string
    }

    pub fn cycle_state_string(&self) -> String {

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
            QueueOp::Subsequent => 'S'
        };

        let f_op_chr = match self.fetch_state {
            FetchState::Scheduled(_) => 'S',
            FetchState::Aborted(_) => 'A',
            FetchState::Suspended => '!',
            _ => ' '
        };        

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

        let bus_str = match self.bus_status {
            BusStatus::InterruptAck => "IRQA",
            BusStatus::IORead => "IOR ",
            BusStatus::IOWrite => "IOW ",
            BusStatus::Halt => "HALT",
            BusStatus::CodeFetch => "CODE",
            BusStatus::MemRead => "MEMR",
            BusStatus::MemWrite => "MEMW",
            BusStatus::Passive => "PASV"     
        };

        let t_str = match self.t_cycle {
            TCycle::TInit => "T0",
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

        let mut q_read_str = "       |".to_string();

        if self.last_queue_op == QueueOp::First {
            // First byte of opcode read from queue. Decode the full instruction
            q_read_str = format!(
                "<-q {:02X} | [{:04X}:{:04X}] {} ({})", 
                self.last_queue_byte, 
                self.cs, 
                self.ip, 
                self.i,
                self.i.size
            );
        }
        else if self.last_queue_op == QueueOp::Subsequent {
            q_read_str = format!("<-q {:02X} |", self.last_queue_byte);
        }        
      
        let cycle_str = format!(
            "{:08}:{:04} {:02}[{:05X}] {:02} M:{}{}{} I:{}{}{} {:04} {:02} {:06} ({}) | {:1}{:1} {:<16} | {:1}{:1} [{:08}] {} {}",
            self.cycle_num,
            self.instr_cycle,
            ale_str,
            self.address_bus,
            seg_str,
            rs_chr, aws_chr, ws_chr, ior_chr, aiow_chr, iow_chr,
            bus_str,
            t_str,
            xfer_str,
            self.transfer_n,
            f_op_chr,
            self.fetch_delay,
            format!("{:?}", self.fetch_state),
            q_op_chr,
            self.queue.len(),
            self.queue.to_string(),
            q_read_str,
            self.trace_comment
        );        

        cycle_str
    }

    pub fn instruction_state_string(&self) -> String {
        let mut instr_str = String::new();

        instr_str.push_str(&format!("{:04x}:{:04x} {}\n", self.cs, self.ip, self.i));
        instr_str.push_str(&format!("AX: {:04x} BX: {:04x} CX: {:04x} DX: {:04x}\n", self.ax, self.bx, self.cx, self.dx));
        instr_str.push_str(&format!("SP: {:04x} BP: {:04x} SI: {:04x} DI: {:04x}\n", self.sp, self.bp, self.si, self.di));
        instr_str.push_str(&format!("CS: {:04x} DS: {:04x} ES: {:04x} SS: {:04x}\n", self.cs, self.ds, self.es, self.ss));
        instr_str.push_str(&format!("IP: {:04x} FLAGS: {:04x}", self.ip, self.flags));

        instr_str
    }

    pub fn trace_print(&mut self, trace_str: &str) {
        if let Some(w) = self.trace_writer.as_mut() {
            let mut _r = w.write_all(trace_str.as_bytes());
            _r = w.write_all("\n".as_bytes());
        }
    }

    pub fn trace_comment(&mut self, comment: &'static str) {
        self.trace_comment = comment;
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
    }
}



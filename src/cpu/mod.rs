#![allow(dead_code)]
use std::error::Error;
use core::fmt::Display;
use std::collections::VecDeque;

use lazy_static::lazy_static;
use regex::Regex;

// Pull in all CPU module components
mod cpu_opcodes;
mod cpu_addressing;
mod cpu_stack;
mod cpu_bitwise;
mod cpu_alu;
mod cpu_string;
mod cpu_bcd;


use crate::bus::BusInterface;
use crate::byteinterface::ByteInterface;
use crate::io::IoBusInterface;

use crate::arch::{OperandType, Instruction, Opcode, SegmentOverride, decode, RepType, Register8, Register16};

pub const CPU_MHZ: f64 = 4.77272666;
const CPU_HISTORY_LEN: usize = 32;
const CPU_CALL_STACK_LEN: usize = 16;

const INTERRUPT_VEC_LEN: usize = 4;

const CPU_FLAG_CARRY: u16      = 0b0000_0000_0000_0001;
const CPU_FLAG_RESERVED1: u16  = 0b0000_0000_0000_0010;
const CPU_FLAG_PARITY: u16     = 0b0000_0000_0000_0100;
const CPU_FLAG_RESERVED3: u16  = 0b0000_0000_0000_1000;
const CPU_FLAG_AUX_CARRY: u16  = 0b0000_0000_0001_0000;
const CPU_FLAG_RESERVED5: u16  = 0b0000_0000_0010_0000;
const CPU_FLAG_ZERO: u16       = 0b0000_0000_0100_0000;
const CPU_FLAG_SIGN: u16       = 0b0000_0000_1000_0000;
const CPU_FLAG_TRAP: u16       = 0b0000_0001_0000_0000;
const CPU_FLAG_INT_ENABLE: u16 = 0b0000_0010_0000_0000;
const CPU_FLAG_DIRECTION: u16  = 0b0000_0100_0000_0000;
const CPU_FLAG_OVERFLOW: u16   = 0b0000_1000_0000_0000;

const CPU_FLAG_RESERVED12: u16 = 0b0001_0000_0000_0000;
const CPU_FLAG_RESERVED13: u16 = 0b0010_0000_0000_0000;
const CPU_FLAG_RESERVED14: u16 = 0b0100_0000_0000_0000;
const CPU_FLAG_RESERVED15: u16 = 0b1000_0000_0000_0000;

const EFLAGS_POP_MASK: u16     = 0b0000_1111_1101_0101;

const REGISTER_HI_MASK: u16    = 0b0000_0000_1111_1111;
const REGISTER_LO_MASK: u16    = 0b1111_1111_0000_0000;


pub enum CpuType {
    Cpu8088,
    Cpu8086,
    Cpu8186
}
impl Default for CpuType {
    fn default() -> Self { CpuType::Cpu8088 }
}

#[derive(Debug, Copy, Clone)]
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

#[derive(Default)]
pub struct Cpu {
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
    eflags: u16,
    halted: bool,
    is_running: bool,
    is_single_step: bool,
    is_error: bool,
    in_rep: bool,
    rep_mnemonic: Opcode,
    rep_type: RepType,
    rep_state: Vec<(u16, u16, RepState)>,
    piq_len: u32,
    piq_capacity: u32,
    error_string: String,
    instruction_count: u64,
    current_instruction: Instruction,
    instruction_history: VecDeque<Instruction>,
    call_stack: VecDeque<CallStackEntry>,
    interrupt_inhibit: bool,
    reset_seg: u16,
    reset_offset: u16,
    
    opcode0_counter: u32
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
    eflags: u16,
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

pub enum ExecutionResult {

    Okay,
    OkayJump,
    OkayRep,
    UnsupportedOpcode(u8),
    ExecutionError(String),
    ExceptionError(CpuException),
    Halt
}

impl Cpu {

    pub fn new(cpu_type: CpuType, piq_capacity: u32) -> Self {
        let mut cpu: Cpu = Default::default();
        cpu.cpu_type = cpu_type;
        cpu.eflags = CPU_FLAG_RESERVED1;
        cpu.piq_capacity = piq_capacity;
        cpu.instruction_history = VecDeque::with_capacity(16);
        cpu.reset_seg = 0xFFFF;
        cpu.reset_offset = 0x0000;
        cpu
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

        self.eflags |= match flag {
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
        self.eflags &= match flag {
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
        self.eflags &= flag_mask;

        // Copy flag state
        self.eflags |= bits & !flag_mask;
    }

    pub fn load_flags(&mut self) -> u16 {
        // Return 8 LO bits of eFlags register
        self.eflags & 0x00FF
    }

    #[inline]
    pub fn get_flag(&self, flag: Flag) -> bool {
        let mut flags = self.eflags;
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
                self.ax = self.ax & REGISTER_LO_MASK | value as u16
            }    
            Register8::BH => {
                self.bh = value;
                self.bx = self.bx & REGISTER_HI_MASK | ((value as u16) << 8);
            }
            Register8::BL => {
                self.bl = value;
                self.bx = self.bx & REGISTER_LO_MASK | value as u16
            }
            Register8::CH => {
                self.ch = value;
                self.cx = self.cx & REGISTER_HI_MASK | ((value as u16) << 8);
            }
            Register8::CL => {
                self.cl = value;
                self.cx = self.cx & REGISTER_LO_MASK | value as u16
            }
            Register8::DH => {
                self.dh = value;
                self.dx = self.dx & REGISTER_HI_MASK | ((value as u16) << 8);
            }
            Register8::DL => {
                self.dl = value;
                self.dx = self.dx & REGISTER_LO_MASK | value as u16
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

    pub fn set_reset_address(&mut self, segment: u16, offset: u16) {
        self.reset_seg = segment;
        self.reset_offset = offset;
    }

    pub fn reset_address(&mut self) {
        self.cs = self.reset_seg;
        self.ip = self.reset_offset;
    }

    pub fn reset(&mut self) {
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
        
        self.set_register16(Register16::CS, self.reset_seg);
        self.set_register16(Register16::IP, self.reset_offset);

        self.eflags = CPU_FLAG_RESERVED1;
        self.instruction_count = 0;

        self.in_rep = false;
        self.rep_state.clear();
        self.piq_len = 0;
        self.halted = false;
        self.interrupt_inhibit = false;
        self.is_error = false;
        self.instruction_history.clear();
        self.call_stack.clear();
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
            eflags: self.eflags
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
                let fl = self.eflags & CPU_FLAG_CARRY > 0;
                format!("{:1}", fl as u8)
            },
            p_fl: {
                let fl = self.eflags & CPU_FLAG_PARITY > 0;
                format!("{:1}", fl as u8)
            },
            a_fl: {
                let fl = self.eflags & CPU_FLAG_AUX_CARRY > 0;
                format!("{:1}", fl as u8)
            },
            z_fl: {
                let fl = self.eflags & CPU_FLAG_ZERO > 0;
                format!("{:1}", fl as u8)
            },
            s_fl: {
                let fl = self.eflags & CPU_FLAG_SIGN > 0;
                format!("{:1}", fl as u8)
            },
            t_fl: {
                let fl = self.eflags & CPU_FLAG_TRAP > 0;
                format!("{:1}", fl as u8)
            },
            i_fl: {
                let fl = self.eflags & CPU_FLAG_INT_ENABLE > 0;
                format!("{:1}", fl as u8)
            },
            d_fl: {
                let fl = self.eflags & CPU_FLAG_DIRECTION > 0;
                format!("{:1}", fl as u8)
            },
            o_fl: {
                let fl = self.eflags & CPU_FLAG_OVERFLOW > 0;
                format!("{:1}", fl as u8)
            },
            
            flags: format!("{:04}", self.eflags),
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

    pub fn end_interrupt(&mut self, bus: &mut BusInterface) {

        self.pop_register16(bus, Register16::IP);
        self.pop_register16(bus, Register16::CS);
        //log::trace!("CPU: Return from interrupt to [{:04X}:{:04X}]", self.cs, self.ip);
        self.pop_flags(bus);
    }

    /// Perform a software interrupt
    pub fn do_sw_interrupt(&mut self, bus: &mut BusInterface, interrupt: u8) {

        // When an interrupt occurs the following happens:
        // 1. CPU pushes flags register to stack
        // 2. CPU pushes far return address into the stack
        // 3. CPU fetches the four byte interrupt vector from the IVT
        // 4. CPU transfers control to the routine specified by the interrupt vector
        // (AoA 17.1)

        self.push_flags(bus);

        // Push return address of next instruction onto stack
        self.push_register16(bus, Register16::CS);
        
        // We need to push the address past the current instruction (skip size of INT)
        // INT instruction = two bytes
        let ip = self.ip.wrapping_add(2);
        self.push_u16(bus, ip);
        
        // Read the IVT
        let ivt_addr = Cpu::calc_linear_address(0x0000, (interrupt as usize * INTERRUPT_VEC_LEN) as u16);
        let (new_ip, _cost) = BusInterface::read_u16(&bus, ivt_addr as usize).unwrap();
        let (new_cs, _cost) = BusInterface::read_u16(&bus, (ivt_addr + 2) as usize ).unwrap();
        self.ip = new_ip;
        self.cs = new_cs;

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
    }

    /// Handle a CPU exception
    pub fn handle_exception(&mut self, bus: &mut BusInterface, exception: u8) {

        // 
        self.push_flags(bus);

        // Push return address of next instruction onto stack
        self.push_register16(bus, Register16::CS);

        // Don't push address of next instruction
        self.push_u16(bus, self.ip);
        
        if exception == 0x0 {
            log::trace!("CPU Exception: {:02X} Saving return: {:04X}:{:04X}", exception, self.cs, self.ip);
        }
        // Read the IVT
        let ivt_addr = Cpu::calc_linear_address(0x0000, (exception as usize * INTERRUPT_VEC_LEN) as u16);
        let (new_ip, _cost) = BusInterface::read_u16(&bus, ivt_addr as usize).unwrap();
        let (new_cs, _cost) = BusInterface::read_u16(&bus, (ivt_addr + 2) as usize ).unwrap();
        self.ip = new_ip;
        self.cs = new_cs;
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
    pub fn do_hw_interrupt(&mut self, bus: &mut BusInterface, interrupt: u8) {

        // When an interrupt occurs the following happens:
        // 1. CPU pushes flags register to stack
        // 2. CPU pushes far return address into the stack
        // 3. CPU fetches the four byte interrupt vector from the IVT
        // 4. CPU transfers control to the routine specified by the interrupt vector
        // (AoA 17.1)

        self.push_flags(bus);
        // Push cs:ip return address to stack
        self.push_register16(bus, Register16::CS);
        self.push_register16(bus, Register16::IP);

        // If we are in a repeated string instruction (REP prefix) we need to save the state of the REP instruction
        if self.in_rep {
            let (src_reg, src_reg_val) = match self.current_instruction.segment_override {
                SegmentOverride::SegmentCS => (Register16::CS, self.cs),
                SegmentOverride::SegmentES => (Register16::ES, self.es),
                SegmentOverride::SegmentSS => (Register16::SS, self.ss),
                _=> (Register16::DS, self.ds)
            };

            let state: RepState = match self.current_instruction.mnemonic {
                Opcode::LODSB => {
                    RepState::LodsbState(src_reg, src_reg_val, self.si, self.cx)
                }
                Opcode::LODSW => {
                    RepState::LodswState(src_reg, src_reg_val, self.si, self.cx)
                } 
                Opcode::MOVSB => {
                    RepState::MovsbState(src_reg, src_reg_val, self.si, self.es, self.di, self.cx)
                } 
                Opcode::MOVSW => {
                    RepState::MovswState(src_reg, src_reg_val, self.si, self.es, self.di, self.cx)
                } 
                Opcode::CMPSB => {
                    RepState::CmpsbState(src_reg, src_reg_val, self.si, self.es, self.di, self.cx)
                } 
                Opcode::CMPSW => {
                    RepState::CmpswState(src_reg, src_reg_val, self.si, self.es, self.di, self.cx)
                }
                Opcode::STOSB => {
                    RepState::StosbState(self.es, self.di, self.cx)
                } 
                Opcode::STOSW => {
                    RepState::StoswState(self.es, self.di, self.cx)
                } 
                Opcode::SCASB => {
                    RepState::ScasbState(self.es, self.di, self.cx)
                } 
                Opcode::SCASW => {
                    RepState::ScaswState(self.es, self.di, self.cx)
                }
                _=> {
                    log::warn!("Invalid instruction saving REP state: {:?}", self.current_instruction.mnemonic);
                    RepState::NoState
                }
            };

            self.rep_state.push((self.cs, self.ip, state));
            self.in_rep = false;
        }

        // Read the IVT
        let ivt_addr = Cpu::calc_linear_address(0x0000, (interrupt as usize * INTERRUPT_VEC_LEN) as u16);
        let (new_ip, _cost) = BusInterface::read_u16(&bus, ivt_addr as usize).unwrap();
        let (new_cs, _cost) = BusInterface::read_u16(&bus, (ivt_addr + 2) as usize ).unwrap();
        self.ip = new_ip;
        self.cs = new_cs;

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

    pub fn step(&mut self, bus: &mut BusInterface, io_bus: &mut IoBusInterface) -> Result<(), CpuError> {

        // When halted, the CPU waits for an interrupt to fire before resuming execution
        if self.halted {
            return Ok(())
        }

        let instruction_address = Cpu::calc_linear_address(self.cs, self.ip);

        bus.set_cursor(instruction_address as usize);

        match decode(bus) {
            Ok(mut i) => {
                self.current_instruction = i;
                match self.execute_instruction(&i, bus, io_bus) {

                    ExecutionResult::Okay => {
                        // Normal non-jump instruction updates CS:IP to next instruction
                        self.assert_state();
                        self.ip = self.ip.wrapping_add(i.size as u16);

                        i.address = instruction_address;
                        if self.instruction_history.len() == CPU_HISTORY_LEN {
                            self.instruction_history.pop_front();
                        }
                        self.instruction_history.push_back(i);
                        self.instruction_count += 1;
                        Ok(())
                    }
                    ExecutionResult::OkayJump => {
                        self.assert_state();
                        // Flush PIQ on jump
                        self.piq_len = 0;

                        i.address = instruction_address;
                        if self.instruction_history.len() == CPU_HISTORY_LEN {
                            self.instruction_history.pop_front();
                        }
                        self.instruction_history.push_back(i);
                        self.instruction_count += 1;
                        Ok(())
                    }
                    ExecutionResult::OkayRep => {
                        // We are in a REPx-prefixed instruction.
                        // The ip will not increment until the instruction has completed
                        // But process interrupts
                        self.assert_state();
                        i.address = instruction_address;
                        if self.instruction_history.len() == CPU_HISTORY_LEN {
                            self.instruction_history.pop_front();
                        }
                        self.instruction_history.push_back(i);
                        self.instruction_count += 1;
                        Ok(())
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
                                self.handle_exception(bus, 0);
                                Ok(())
                            }
                            _ => {
                                // Unhandled exception?
                                Err(CpuError::ExceptionError(exception))
                            }
                        }
                    }
                }
            }
            Err(_) => {
                self.is_running = false;
                self.is_error = true;
                Err(CpuError::InstructionDecodeError(instruction_address))
            }
        }
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



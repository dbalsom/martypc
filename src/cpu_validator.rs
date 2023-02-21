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

use std::{ 
    error::Error,
    fmt::Display,
};

use crate::cpu_808x::QueueOp;

#[derive (PartialEq, Copy, Clone)]
pub enum ReadType {
    Code,
    Data
}

#[derive (Copy, Clone, Default, PartialEq)]
pub struct VRegisters {
    pub ax: u16,
    pub bx: u16,
    pub cx: u16,
    pub dx: u16,
    pub cs: u16,
    pub ss: u16,
    pub ds: u16,
    pub es: u16,
    pub sp: u16,
    pub bp: u16,
    pub si: u16,
    pub di: u16,
    pub ip: u16,
    pub flags: u16
}

#[derive (Debug)]
pub enum ValidatorError {
    ParameterError,
    CpuError,
    MemOpMismatch,
    RegisterMismatch,
    CpuDesynced,
    CycleMismatch
}

impl Error for ValidatorError {}
impl Display for ValidatorError{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            ValidatorError::ParameterError => {
                write!(f, "The validator was passed a bad parameter." )
            }
            ValidatorError::CpuError => {
                write!(f, "The CPU client encountered an error." )
            }
            ValidatorError::MemOpMismatch => {
                write!(f, "Instruction memory operands did not validate.")
            }
            ValidatorError::RegisterMismatch => {
                write!(f, "Instruction registers did not validate.")
            }
            ValidatorError::CpuDesynced => {
                write!(f, "CPU state desynced with client.")
            }
            ValidatorError::CycleMismatch => {
                write!(f, "Instruction cycle states did not validate.")
            }                  
        }
    }
}

#[derive (Copy, Clone, PartialEq)]
pub enum BusCycle {
    T1,
    T2,
    T3,
    T4,
    Tw
}

#[derive (Copy, Clone, PartialEq, Debug)]
pub enum AccessType {
    AccAlternateData = 0x0,
    AccStack,
    AccCodeOrNone,
    AccData,
}


#[derive (Copy, Clone, Debug, PartialEq)]
pub enum BusState {
    INTA = 0,   // IRQ Acknowledge
    IOR  = 1,   // IO Read
    IOW  = 2,   // IO Write
    HALT = 3,   // Halt
    CODE = 4,   // Code
    MEMR = 5,   // Memory Read
    MEMW = 6,   // Memory Write
    PASV = 7    // Passive
}

#[derive (Copy, Clone)]
pub struct CycleState {
    pub n: u32,
    pub addr: u32,
    pub t_state: BusCycle,
    pub a_type: AccessType,
    pub b_state: BusState,
    pub ale: bool,
    pub mrdc: bool,
    pub amwc: bool,
    pub mwtc: bool,
    pub iorc: bool,
    pub aiowc: bool,
    pub iowc: bool,
    pub inta: bool,
    pub q_op: QueueOp,
    pub q_byte: u8,
    pub q_len: u32,
    pub data_bus: u16,
    
}

impl PartialEq<CycleState> for CycleState {
    fn eq(&self, other: &CycleState) -> bool {

        let equals_a = 
            self.t_state == other.t_state
            && self.b_state == other.b_state
            && self.ale == other.ale
            && self.mrdc == other.mrdc
            && self.amwc == other.amwc
            && self.mwtc == other.mwtc
            && self.iorc == other.iorc
            //&& self.inta == other.inta
            && self.q_op == other.q_op;

        let equals_b = match self.t_state {
            BusCycle::T1 => {
                if self.ale {
                    self.addr == other.addr
                }
                else {
                    true
                }
            },
            BusCycle::T4 => {
                //(self.q_len == other.q_len) && (self.a_type == other.a_type)
                self.a_type == other.a_type
            }
            BusCycle::T3 => {
                //(self.data_bus == other.data_bus) && (self.a_type == other.a_type)
                self.a_type == other.a_type
            }
            _=> self.a_type == other.a_type
        };

        equals_a && equals_b
    }
}

pub trait CpuValidator {
    fn init(&mut self, mask_flags: bool, cycle_trace: bool, visit_once: bool) -> bool;
    fn begin(&mut self, regs: &VRegisters );
    fn validate(
        &mut self, 
        name: String, 
        instr: &[u8], 
        has_modrm: bool, 
        cycles: i32, 
        regs: &VRegisters, 
        emu_states: &Vec<CycleState>) 
            -> Result<bool, ValidatorError>;

    fn emu_read_byte(&mut self, addr: u32, data: u8, read_type: ReadType);
    fn emu_write_byte(&mut self, addr: u32, data: u8);
    fn discard_op(&mut self);
}


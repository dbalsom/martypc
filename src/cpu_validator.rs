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
        }
    }
}

pub trait CpuValidator {
    fn init(&mut self, mask_flags: bool, cycle_trace: bool, visit_once: bool) -> bool;
    fn begin(&mut self, regs: &VRegisters );
    fn validate(&mut self, name: String, instr: &[u8], has_modrm: bool, cycles: i32, regs: &VRegisters) -> Result<bool, ValidatorError>;

    fn emu_read_byte(&mut self, addr: u32, data: u8, read_type: ReadType);
    fn emu_write_byte(&mut self, addr: u32, data: u8);
    fn discard_op(&mut self);
}


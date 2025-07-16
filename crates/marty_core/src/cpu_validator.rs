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

    --------------------------------------------------------------------------

    cpu_validator.rs

    Implements the CpuValidator trait, implemented by a specific CPU Validator
    implementation.
*/

#![allow(dead_code)]

use std::{
    error::Error,
    fmt::{self, Display},
    str::FromStr,
};

#[cfg(feature = "arduino_validator")]
pub use ard808x_client::BusState;

#[cfg(not(feature = "arduino_validator"))]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BusState {
    INTA = 0, // IRQ Acknowledge
    IOR = 1,  // IO Read
    IOW = 2,  // IO Write
    HALT = 3, // Halt
    CODE = 4, // Code
    MEMR = 5, // Memory Read
    MEMW = 6, // Memory Write
    PASV = 7, // Passive
}

use crate::cpu_common::{CpuType, QueueOp};
use serde::{
    de::{self, Deserializer, SeqAccess, Visitor},
    ser::SerializeSeq,
    Deserialize,
    Serialize,
    Serializer,
};

pub const VAL_NO_READS: u8 = 0b0000_0001; // Don't validate read op data
pub const VAL_NO_WRITES: u8 = 0b0000_0010; // Don't validate write op data
pub const VAL_NO_REGS: u8 = 0b0000_0100; // Don't validate registers
pub const VAL_NO_FLAGS: u8 = 0b0000_1000; // Don't validate flags
pub const VAL_ALLOW_ONE: u8 = 0b0001_0000; // Allow a one-cycle variance in cycle states.
pub const VAL_NO_CYCLES: u8 = 0b0010_0000; // Don't validate cycle states.

#[derive(Copy, Clone, Debug, Deserialize, PartialEq, Default)]
pub enum ValidatorType {
    #[default]
    None,
    Pi8088,
    Arduino8088,
}

impl FromStr for ValidatorType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        match s.to_lowercase().as_str() {
            "pi8088" => Ok(ValidatorType::Pi8088),
            "arduino8088" => Ok(ValidatorType::Arduino8088),
            _ => Err("Bad value for validatortype".to_string()),
        }
    }
}

#[derive(PartialEq, Debug, Copy, Clone, Default)]
pub enum ValidatorMode {
    #[default]
    Instruction,
    Cycle,
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum ValidatorResult {
    Ok,
    OkEnd,
    Error,
}

#[derive(PartialEq, Copy, Clone)]
pub enum BusType {
    Mem,
    Io,
}

#[derive(PartialEq, Copy, Clone)]
pub enum ReadType {
    Code,
    Data,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BusOpType {
    CodeRead,
    MemRead,
    MemWrite,
    IoRead,
    IoWrite,
}

#[derive(Copy, Clone)]
pub struct BusOp {
    pub op_type: BusOpType,
    pub addr: u32,
    pub bhe: bool,
    pub data: u16,
    pub flags: u8,
}

#[derive(Copy, Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct VRegisters {
    pub ax:    u16,
    pub bx:    u16,
    pub cx:    u16,
    pub dx:    u16,
    pub cs:    u16,
    pub ss:    u16,
    pub ds:    u16,
    pub es:    u16,
    pub sp:    u16,
    pub bp:    u16,
    pub si:    u16,
    pub di:    u16,
    pub ip:    u16,
    pub flags: u16,
}

impl Display for VRegisters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AX: {:04x} BX: {:04x} CX: {:04x} DX: {:04x}\n\
            SP: {:04x} BP: {:04x} SI: {:04x} DI: {:04x}\n\
            CS: {:04x} DS: {:04x} ES: {:04x} SS: {:04x}\n\
            IP: {:04x}\n\
            FLAGS: {:04x}",
            self.ax,
            self.bx,
            self.cx,
            self.dx,
            self.sp,
            self.bp,
            self.si,
            self.di,
            self.cs,
            self.ds,
            self.es,
            self.ss,
            self.ip,
            self.flags
        )
    }
}

#[derive(Copy, Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct VRegistersDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ax:    Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bx:    Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cx:    Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dx:    Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cs:    Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ss:    Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds:    Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub es:    Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sp:    Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bp:    Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub si:    Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub di:    Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip:    Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<u16>,
}

impl From<VRegisters> for VRegistersDelta {
    fn from(regs: VRegisters) -> Self {
        VRegistersDelta {
            ax:    Some(regs.ax),
            bx:    Some(regs.bx),
            cx:    Some(regs.cx),
            dx:    Some(regs.dx),
            cs:    Some(regs.cs),
            ss:    Some(regs.ss),
            ds:    Some(regs.ds),
            es:    Some(regs.es),
            sp:    Some(regs.sp),
            bp:    Some(regs.bp),
            si:    Some(regs.si),
            di:    Some(regs.di),
            ip:    Some(regs.ip),
            flags: Some(regs.flags),
        }
    }
}

impl Display for VRegistersDelta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let spacer = "----".to_string();
        write!(
            f,
            "AX: {} BX: {} CX: {} DX: {}\n\
            SP: {} BP: {} SI: {} DI: {}\n\
            CS: {} DS: {} ES: {} SS: {}\n\
            IP: {}\n\
            FLAGS: {}",
            self.ax.map_or(spacer.clone(), |v| format!("{:04x}", v)),
            self.bx.map_or(spacer.clone(), |v| format!("{:04x}", v)),
            self.cx.map_or(spacer.clone(), |v| format!("{:04x}", v)),
            self.dx.map_or(spacer.clone(), |v| format!("{:04x}", v)),
            self.sp.map_or(spacer.clone(), |v| format!("{:04x}", v)),
            self.bp.map_or(spacer.clone(), |v| format!("{:04x}", v)),
            self.si.map_or(spacer.clone(), |v| format!("{:04x}", v)),
            self.di.map_or(spacer.clone(), |v| format!("{:04x}", v)),
            self.cs.map_or(spacer.clone(), |v| format!("{:04x}", v)),
            self.ds.map_or(spacer.clone(), |v| format!("{:04x}", v)),
            self.es.map_or(spacer.clone(), |v| format!("{:04x}", v)),
            self.ss.map_or(spacer.clone(), |v| format!("{:04x}", v)),
            self.ip.map_or(spacer.clone(), |v| format!("{:04x}", v)),
            self.flags.map_or(spacer.clone(), |v| format!("{:04x}", v))
        )
    }
}

impl VRegistersDelta {
    /// A delta that changes everything can't be valid. Check for this condition, so we can print
    /// an error.
    pub fn is_valid(&self) -> bool {
        let delta_all_changed = self.ax.is_some()
            && self.bx.is_some()
            && self.cx.is_some()
            && self.dx.is_some()
            && self.cs.is_some()
            && self.ss.is_some()
            && self.ds.is_some()
            && self.es.is_some()
            && self.sp.is_some()
            && self.bp.is_some()
            && self.si.is_some()
            && self.di.is_some()
            && self.ip.is_some()
            && self.flags.is_some();

        !delta_all_changed
    }
}

impl VRegisters {
    pub fn apply_delta(mut self, delta: &VRegistersDelta) -> Self {
        if let Some(val) = delta.ax {
            self.ax = val;
        }
        if let Some(val) = delta.bx {
            self.bx = val;
        }
        if let Some(val) = delta.cx {
            self.cx = val;
        }
        if let Some(val) = delta.dx {
            self.dx = val;
        }
        if let Some(val) = delta.cs {
            self.cs = val;
        }
        if let Some(val) = delta.ss {
            self.ss = val;
        }
        if let Some(val) = delta.ds {
            self.ds = val;
        }
        if let Some(val) = delta.es {
            self.es = val;
        }
        if let Some(val) = delta.sp {
            self.sp = val;
        }
        if let Some(val) = delta.bp {
            self.bp = val;
        }
        if let Some(val) = delta.si {
            self.si = val;
        }
        if let Some(val) = delta.di {
            self.di = val;
        }
        if let Some(val) = delta.ip {
            self.ip = val;
        }
        if let Some(val) = delta.flags {
            self.flags = val;
        }
        self
    }

    pub fn create_delta(self, initial_regs: &VRegisters) -> VRegistersDelta {
        let mut delta: VRegistersDelta = self.into();
        if self.ax == initial_regs.ax {
            delta.ax = None;
        }
        if self.bx == initial_regs.bx {
            delta.bx = None;
        }
        if self.cx == initial_regs.cx {
            delta.cx = None;
        }
        if self.dx == initial_regs.dx {
            delta.dx = None;
        }
        if self.cs == initial_regs.cs {
            delta.cs = None;
        }
        if self.ss == initial_regs.ss {
            delta.ss = None;
        }
        if self.ds == initial_regs.ds {
            delta.ds = None;
        }
        if self.es == initial_regs.es {
            delta.es = None;
        }
        if self.sp == initial_regs.sp {
            delta.sp = None;
        }
        if self.bp == initial_regs.bp {
            delta.bp = None;
        }
        if self.si == initial_regs.si {
            delta.si = None;
        }
        if self.di == initial_regs.di {
            delta.di = None;
        }
        if self.ip == initial_regs.ip {
            delta.ip = None;
        }
        if self.flags == initial_regs.flags {
            delta.flags = None;
        }
        delta
    }
}

#[derive(Debug)]
pub enum ValidatorError {
    ParameterError,
    CpuError,
    MemOpMismatch,
    RegisterMismatch,
    FlagsMismatch,
    BothMismatch,
    CpuDesynced,
    CycleMismatch,
}

impl Error for ValidatorError {}
impl Display for ValidatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            ValidatorError::ParameterError => {
                write!(f, "The validator was passed a bad parameter.")
            }
            ValidatorError::CpuError => {
                write!(f, "The CPU client encountered an error.")
            }
            ValidatorError::MemOpMismatch => {
                write!(f, "Instruction memory operands did not validate.")
            }
            ValidatorError::RegisterMismatch => {
                write!(f, "Instruction registers did not validate.")
            }
            ValidatorError::FlagsMismatch => {
                write!(f, "Instruction flags did not validate.")
            }
            ValidatorError::BothMismatch => {
                write!(f, "Instruction registers and flags did not validate.")
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

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BusCycle {
    Ti,
    T1,
    T2,
    T3,
    T4,
    Tw,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum AccessType {
    AlternateData = 0x0,
    Stack,
    CodeOrNone,
    Data,
}

impl From<u8> for AccessType {
    fn from(value: u8) -> Self {
        match (value >> 3) & 0x03 {
            0x0 => AccessType::AlternateData,
            0x1 => AccessType::Stack,
            0x2 => AccessType::CodeOrNone,
            0x3 => AccessType::Data,
            _ => AccessType::CodeOrNone, // Default to CodeOrNone for invalid values
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct CycleState {
    pub n: u32,
    pub addr: u32,
    pub t_state: BusCycle,
    pub a_type: AccessType,
    pub b_state: BusState,
    pub ale: bool,
    pub bhe: bool,
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
    pub q: [u8; 6],
    pub data_bus: u16,
}

impl CycleState {
    pub fn queue_vec(&self) -> Vec<u8> {
        let mut q_vec = Vec::new();
        for i in 0..(self.q_len as usize) {
            q_vec.push(self.q[i]);
        }
        q_vec
    }
}

impl Serialize for CycleState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let q_byte;

        let fields_as_strings = [
            format!("{}", if self.ale { 1 } else { 0 }),
            format!("{:05X}", self.addr),
            format!(
                "{:02}",
                if self.ale || matches!(self.t_state, BusCycle::Ti) {
                    "--"
                }
                else {
                    match self.a_type {
                        AccessType::AlternateData => "ES",
                        AccessType::Stack => "SS",
                        AccessType::CodeOrNone => "CS",
                        AccessType::Data => "DS",
                    }
                }
            ),
            format!("{:03}", {
                let mut mem_str = String::new();
                // status lines are active-low
                mem_str.push(if !self.mrdc { 'R' } else { '-' });
                mem_str.push(if !self.amwc { 'A' } else { '-' });
                mem_str.push(if !self.mwtc { 'W' } else { '-' });
                mem_str
            }),
            format!("{:03}", {
                let mut io_str = String::new();
                // status lines are active-low
                io_str.push(if !self.iorc { 'R' } else { '-' });
                io_str.push(if !self.aiowc { 'A' } else { '-' });
                io_str.push(if !self.iowc { 'W' } else { '-' });
                io_str
            }),
            format!("{}", if self.bhe { 1 } else { 0 }),
            format!("{:?}", self.data_bus),
            format!("{:?}", self.b_state),
            format!("{:?}", self.t_state),
            (match self.q_op {
                QueueOp::First => "F",
                QueueOp::Subsequent => "S",
                QueueOp::Flush => "E",
                _ => "-",
            })
            .to_string(),
            (if matches!(self.q_op, QueueOp::Idle) {
                "--"
            }
            else {
                q_byte = format!("{:02X}", self.q_byte);
                &q_byte
            })
            .to_string(),
        ];

        let mut seq = serializer.serialize_seq(Some(fields_as_strings.len()))?;

        for (i, field) in fields_as_strings.iter().enumerate() {
            match i {
                0 => seq.serialize_element(&(self.ale as u8)),
                1 => seq.serialize_element(&self.addr),
                5 => seq.serialize_element(&(self.bhe as u8)),
                6 => seq.serialize_element(&self.data_bus),
                10 => seq.serialize_element(&self.q_byte),
                _ => seq.serialize_element(field),
            }?;
        }

        seq.end()
    }
}

impl<'de> de::Deserialize<'de> for CycleState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CycleStateVisitor;

        impl<'de> Visitor<'de> for CycleStateVisitor {
            type Value = CycleState;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a sequence of strings representing a CycleState")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<CycleState, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let ale_int: u8 = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let ale = ale_int != 0;

                let addr = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(1, &self))?;

                let a_type = match seq
                    .next_element::<String>()?
                    .ok_or_else(|| de::Error::invalid_length(2, &self))?
                    .as_str()
                {
                    "ES" => AccessType::AlternateData,
                    "SS" => AccessType::Stack,
                    "CS" => AccessType::CodeOrNone,
                    "DS" => AccessType::Data,
                    "--" => AccessType::CodeOrNone,
                    _ => return Err(de::Error::custom("invalid a_type")),
                };

                let mem_str: String = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(3, &self))?;
                let mrdc = mem_str.chars().nth(0) != Some('R');
                let amwc = mem_str.chars().nth(1) != Some('A');
                let mwtc = mem_str.chars().nth(2) != Some('W');

                let io_str: String = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(4, &self))?;
                let iorc = io_str.chars().nth(0) != Some('R');
                let aiowc = io_str.chars().nth(1) != Some('A');
                let iowc = io_str.chars().nth(2) != Some('W');

                let bhe_int: u8 = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(5, &self))?;
                let bhe = bhe_int != 0;

                let data_bus = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(6, &self))?;

                let b_state = match seq
                    .next_element::<String>()?
                    .ok_or_else(|| de::Error::invalid_length(7, &self))?
                    .as_str()
                {
                    "CODE" => BusState::CODE,
                    "MEMR" => BusState::MEMR,
                    "MEMW" => BusState::MEMW,
                    "PASV" => BusState::PASV,
                    "IOW" => BusState::IOW,
                    "IOR" => BusState::IOR,
                    "INTA" => BusState::INTA,
                    _ => return Err(de::Error::custom("invalid b_state")),
                };

                let t_state = match seq
                    .next_element::<String>()?
                    .ok_or_else(|| de::Error::invalid_length(8, &self))?
                    .as_str()
                {
                    "T1" => BusCycle::T1,
                    "T2" => BusCycle::T2,
                    "T3" => BusCycle::T3,
                    "Tw" => BusCycle::Tw,
                    "T4" => BusCycle::T4,
                    "Ti" => BusCycle::Ti,
                    _ => return Err(de::Error::custom("invalid a_type")),
                };

                let q_op = match seq
                    .next_element::<String>()?
                    .ok_or_else(|| de::Error::invalid_length(9, &self))?
                    .as_str()
                {
                    "F" => QueueOp::First,
                    "S" => QueueOp::Subsequent,
                    "E" => QueueOp::Flush,
                    "-" => QueueOp::Idle,
                    _ => return Err(de::Error::custom("invalid q_op")),
                };

                let q_byte = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(10, &self))?;

                // Return the constructed CycleState at the end.
                Ok(CycleState {
                    n: 0,
                    addr,
                    t_state,
                    a_type,
                    b_state,
                    ale,
                    mrdc,
                    amwc,
                    mwtc,
                    iorc,
                    aiowc,
                    iowc,
                    inta: false,
                    bhe,
                    q_op,
                    q_byte,
                    q_len: 0,
                    q: [0; 6],
                    data_bus,
                })
            }
        }

        deserializer.deserialize_seq(CycleStateVisitor)
    }
}

impl Display for CycleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ale_str = match self.ale {
            true => "A:",
            false => "  ",
        };

        let mut seg_str = "  ";
        if self.t_state != BusCycle::T1 {
            // Segment status only valid in T2+
            seg_str = match self.a_type {
                AccessType::AlternateData => "ES",
                AccessType::Stack => "SS",
                AccessType::CodeOrNone => "CS",
                AccessType::Data => "DS",
            };
        }

        let q_op_chr = match self.q_op {
            QueueOp::Idle => ' ',
            QueueOp::First => 'F',
            QueueOp::Flush => 'E',
            QueueOp::Subsequent => 'S',
        };

        // All read/write signals are active/low
        let rs_chr = match !self.mrdc {
            true => 'R',
            false => '.',
        };
        let aws_chr = match !self.aiowc {
            true => 'A',
            false => '.',
        };
        let ws_chr = match !self.mwtc {
            true => 'W',
            false => '.',
        };
        let ior_chr = match !self.iorc {
            true => 'R',
            false => '.',
        };
        let aiow_chr = match !self.aiowc {
            true => 'A',
            false => '.',
        };
        let iow_chr = match !self.iowc {
            true => 'W',
            false => '.',
        };

        let bhe_chr = match self.bhe {
            true => 'B',
            false => '.',
        };

        let intr_chr = '.';
        let inta_chr = '.';

        let bus_str = match self.b_state {
            BusState::INTA => "INTA",
            BusState::IOR => "IOR ",
            BusState::IOW => "IOW ",
            BusState::HALT => "HALT",
            BusState::CODE => "CODE",
            BusState::MEMR => "MEMR",
            BusState::MEMW => "MEMW",
            BusState::PASV => "PASV",
        };

        let t_str = match self.t_state {
            BusCycle::Ti => "Ti",
            BusCycle::T1 => "T1",
            BusCycle::T2 => "T2",
            BusCycle::T3 => "T3",
            BusCycle::T4 => "T4",
            BusCycle::Tw => "Tw",
        };

        let is_reading = (!self.mrdc | !self.iorc) && matches!(self.b_state, BusState::PASV);
        let is_writing = (!self.mwtc | !self.iowc) && matches!(self.b_state, BusState::PASV);

        let mut xfer_str = "        ".to_string();
        if is_reading {
            xfer_str = format!("r-> {:04X}", self.data_bus);
        }
        else if is_writing {
            xfer_str = format!("<-w {:04X}", self.data_bus);
        }

        let mut q_read_str = String::new();

        if self.q_op == QueueOp::First {
            // First byte of opcode read from queue. Decode it to opcode or group specifier
            q_read_str = format!("<-q {:02X}", self.q_byte);
        }
        else if self.q_op == QueueOp::Subsequent {
            q_read_str = format!("<-q {:02X}", self.q_byte);
        }

        write!(
            f,
            "{cycle_num:08} {ale_str:02}[{addr:05X}] \
            {seg_str:02} M:{rs_chr}{aws_chr}{ws_chr} I:{ior_chr}{aiow_chr}{iow_chr} \
            P:{intr_chr}{inta_chr}{bhe_chr} {bus_str:04} {t_str:02} {xfer_str:06} {q_op_chr:1}{q_len:1}{q_str:width$} {q_read_str}",
            cycle_num = self.n,
            ale_str = ale_str,
            addr = self.addr,
            seg_str = seg_str,
            rs_chr = rs_chr,
            aws_chr = aws_chr,
            ws_chr = ws_chr,
            ior_chr = ior_chr,
            aiow_chr = aiow_chr,
            iow_chr = iow_chr,
            intr_chr = intr_chr,
            inta_chr = inta_chr,
            bhe_chr = bhe_chr,
            bus_str = bus_str,
            t_str = t_str,
            xfer_str = xfer_str,
            q_op_chr = q_op_chr,
            q_len = self.q_len,
            q_str = get_queue_str(&self.q, self.q_len as usize),
            width = 12,
            q_read_str = q_read_str,
        )
    }
}

pub fn get_queue_str(q: &[u8], len: usize) -> String {
    let mut outer = "[".to_string();
    let mut inner = String::new();

    for i in 0..len {
        inner.push_str(&format!("{:02X}", q[i]));
    }
    outer.push_str(&format!("{:12}]", inner));
    outer
}

impl PartialEq<CycleState> for CycleState {
    fn eq(&self, other: &CycleState) -> bool {
        let equals_a = self.t_state == other.t_state
            && self.b_state == other.b_state
            && self.ale == other.ale
            && self.mrdc == other.mrdc
            && self.amwc == other.amwc
            && self.mwtc == other.mwtc
            && self.iorc == other.iorc
            //&& self.inta == other.inta
            && self.q_op == other.q_op;

        let equals_b = match self.t_state {
            BusCycle::Ti => true,
            BusCycle::T1 => {
                if self.ale {
                    self.addr == other.addr
                }
                else {
                    true
                }
            }
            BusCycle::T4 => {
                //(self.q_len == other.q_len) && (self.a_type == other.a_type)
                self.a_type == other.a_type
            }
            BusCycle::T3 => {
                //(self.data_bus == other.data_bus) && (self.a_type == other.a_type)
                self.a_type == other.a_type
            }
            _ => self.a_type == other.a_type,
        };

        equals_a && equals_b
    }
}

pub trait CpuValidator {
    fn init(&mut self, mode: ValidatorMode, mask_flags: bool, cycle_trace: bool, visit_once: bool) -> bool;
    fn reset_instruction(&mut self);
    fn begin_instruction(&mut self, regs: &VRegisters, end_instr: usize, end_program: usize);

    fn set_prefetch(&mut self, state: bool);
    fn set_regs(&mut self);
    fn set_opts(
        &mut self,
        ignore_underflow: bool,
        validate_cycles: bool,
        validate_regs: bool,
        validate_flags: bool,
        validate_mem: bool,
    );
    fn validate_instruction(
        &mut self,
        name: String,
        instr: &[u8],
        flags: u8,
        peek_fetch: u16,
        has_modrm: bool,
        cycles: i32,
        regs: &VRegisters,
        emu_states: &[CycleState],
    ) -> Result<ValidatorResult, ValidatorError>;
    fn validate_regs(&mut self, regs: &VRegisters) -> Result<(), ValidatorError>;
    fn emu_read_byte(&mut self, addr: u32, data: u8, bus_type: BusType, read_type: ReadType);
    fn emu_read_word(&mut self, addr: u32, data: u16, bus_type: BusType, read_type: ReadType);
    fn emu_write_byte(&mut self, addr: u32, data: u8, bus_type: BusType);
    fn emu_write_word(&mut self, addr: u32, data: u16, bus_type: BusType);
    fn discard_op(&mut self);
    fn flush(&mut self);

    fn cycle_states(&self) -> &Vec<CycleState>;
    fn name(&self) -> String;
    fn instr_bytes(&self) -> Vec<u8>;
    fn initial_regs(&self) -> VRegisters;
    fn initial_queue(&self) -> Vec<u8>;
    fn final_emu_regs(&self) -> VRegisters;
    fn final_cpu_regs(&self) -> Option<VRegisters>;

    fn emu_ops(&self) -> Vec<BusOp>;
    fn cpu_ops(&self) -> Vec<BusOp>;
    fn cpu_reads(&self) -> Vec<BusOp>;
    fn cpu_queue(&self) -> Vec<u8>;

    fn cpu_type(&self) -> CpuType;
}

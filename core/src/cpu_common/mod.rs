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

    cpu_common::mod.rs

    Implements common functionality shared by different CPU types.

*/

#![allow(dead_code)]

use serde::Deserialize;
use std::str::FromStr;

#[derive(Copy, Clone, Debug)]
pub enum CpuType {
    Intel8088,
    Intel8086,
}

#[derive(Copy, Clone, Debug, Deserialize, PartialEq)]
pub enum TraceMode {
    None,
    Cycle,
    Sigrok,
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
            "cycle" => Ok(TraceMode::Cycle),
            "sigrok" => Ok(TraceMode::Sigrok),
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
    SimulateDramRefresh(bool, u32, u32),
    DramRefreshAdjust(u32),
    HaltResumeDelay(u32),
    OffRailsDetection(bool),
    EnableWaitStates(bool),
    TraceLoggingEnabled(bool),
    EnableServiceInterrupt(bool),
}

use crate::cpu_808x::*;

pub mod alu;

impl Cpu {
    pub fn common_test(&self) {
        //log::trace!("I'm a common cpu function!");
    }
}

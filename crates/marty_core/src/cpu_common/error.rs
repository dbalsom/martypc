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

    ---------------------------------------------------------------------------

    cpu_common::error.rs

    This module defines a common error type for CPUs.

*/

use crate::cpu_common::CpuException;
use std::{error::Error, fmt, fmt::Display};

#[derive(Debug)]
pub enum CpuError {
    InvalidInstructionError(u8, u32),
    UnhandledInstructionError(u8, u32),
    InstructionDecodeError(u32),
    ExecutionError(u32, String),
    CpuHaltedError(u32),
    ExceptionError(CpuException),
}
impl Error for CpuError {}
impl Display for CpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &*self {
            CpuError::InvalidInstructionError(o, addr) => write!(
                f,
                "An invalid instruction was encountered: {:02X} at address: {:06X}",
                o, addr
            ),
            CpuError::UnhandledInstructionError(o, addr) => write!(
                f,
                "An unhandled instruction was encountered: {:02X} at address: {:06X}",
                o, addr
            ),
            CpuError::InstructionDecodeError(addr) => write!(
                f,
                "An error occurred during instruction decode at address: {:06X}",
                addr
            ),
            CpuError::ExecutionError(addr, err) => {
                write!(f, "An execution error occurred at: {:06X} Message: {}", addr, err)
            }
            CpuError::CpuHaltedError(addr) => {
                write!(f, "The CPU was halted at address: {:06X}.", addr)
            }
            CpuError::ExceptionError(exception) => {
                write!(f, "The CPU threw an exception: {:?}", exception)
            }
        }
    }
}

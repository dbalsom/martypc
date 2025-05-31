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

    cpu_common::builder.rs

    Implements common functionality shared by different CPU types.

*/
use crate::{
    bus::ClockFactor,
    cpu_808x::Intel808x,
    cpu_common::{CpuDispatch, CpuSubType, CpuType, TraceMode},
    cpu_vx0::NecVx0,
    tracelogger::TraceLogger,
};
use anyhow::{bail, Result};

#[cfg(feature = "cpu_validator")]
use crate::cpu_validator::{ValidatorMode, ValidatorType};

#[derive(Default)]
pub struct CpuBuilder {
    cpu_type: Option<CpuType>,
    cpu_subtype: Option<CpuSubType>,
    clock_factor: Option<ClockFactor>,
    trace_mode: TraceMode,
    trace_logger: Option<TraceLogger>,
    #[cfg(feature = "cpu_validator")]
    validator_type: ValidatorType,
    #[cfg(feature = "cpu_validator")]
    validator_mode: Option<ValidatorMode>,
    #[cfg(feature = "cpu_validator")]
    validator_logger: Option<TraceLogger>,
    #[cfg(feature = "cpu_validator")]
    validator_baud: Option<u32>,
}

impl CpuBuilder {
    pub fn new() -> CpuBuilder {
        CpuBuilder { ..Default::default() }
    }

    pub fn build(&mut self) -> Result<CpuDispatch> {
        // Build the CPU

        if let Some(cpu_type) = self.cpu_type {
            match cpu_type {
                CpuType::Intel8088 => {
                    let cpu = Intel808x::new(
                        CpuType::Intel8088,
                        CpuSubType::Intel8088,
                        self.clock_factor,
                        self.trace_mode,
                        self.trace_logger.take().unwrap_or_default(),
                        #[cfg(feature = "cpu_validator")]
                        self.validator_type,
                        #[cfg(feature = "cpu_validator")]
                        self.validator_logger.take().unwrap_or_default(),
                        #[cfg(feature = "cpu_validator")]
                        self.validator_mode.take().unwrap_or_default(),
                        #[cfg(feature = "cpu_validator")]
                        self.validator_baud.take().unwrap_or_default(),
                    );
                    Ok(cpu.into())
                }
                CpuType::Intel8086 => {
                    let cpu = Intel808x::new(
                        CpuType::Intel8086,
                        CpuSubType::Intel8086,
                        self.clock_factor,
                        self.trace_mode,
                        self.trace_logger.take().unwrap_or_default(),
                        #[cfg(feature = "cpu_validator")]
                        self.validator_type,
                        #[cfg(feature = "cpu_validator")]
                        self.validator_logger.take().unwrap_or_default(),
                        #[cfg(feature = "cpu_validator")]
                        self.validator_mode.take().unwrap_or_default(),
                        #[cfg(feature = "cpu_validator")]
                        self.validator_baud.take().unwrap_or_default(),
                    );
                    Ok(cpu.into())
                }
                CpuType::NecV20(_) => {
                    let cpu = NecVx0::new(
                        CpuType::NecV20(Default::default()),
                        self.trace_mode,
                        self.trace_logger.take().unwrap_or_default(),
                        #[cfg(feature = "cpu_validator")]
                        self.validator_type,
                        #[cfg(feature = "cpu_validator")]
                        self.validator_logger.take().unwrap_or_default(),
                        #[cfg(feature = "cpu_validator")]
                        self.validator_mode.take().unwrap_or_default(),
                        #[cfg(feature = "cpu_validator")]
                        self.validator_baud.take().unwrap_or_default(),
                    );
                    Ok(cpu.into())
                }
                _ => {
                    bail!("Unimplemented CPU type: {:?}", cpu_type);
                }
            }
        }
        else {
            bail!("CpuType is required.");
        }
    }

    pub fn with_cpu_type(mut self, cpu_type: CpuType) -> Self {
        self.cpu_type = Some(cpu_type);
        self
    }

    pub fn with_clock_factor(mut self, clock_factor: ClockFactor) -> Self {
        self.clock_factor = Some(clock_factor);
        self
    }

    pub fn with_cpu_subtype(mut self, cpu_subtype: CpuSubType) -> Self {
        self.cpu_subtype = Some(cpu_subtype);
        self
    }

    pub fn with_trace_mode(mut self, trace_mode: TraceMode) -> Self {
        self.trace_mode = trace_mode;
        self
    }

    pub fn with_trace_logger(mut self, trace_logger: TraceLogger) -> Self {
        self.trace_logger = Some(trace_logger);
        self
    }

    #[cfg(feature = "cpu_validator")]
    pub fn with_validator_type(mut self, validator_type: ValidatorType) -> Self {
        self.validator_type = validator_type;
        self
    }

    #[cfg(feature = "cpu_validator")]
    pub fn with_validator_mode(mut self, validator_mode: ValidatorMode) -> Self {
        self.validator_mode = Some(validator_mode);
        self
    }

    #[cfg(feature = "cpu_validator")]
    pub fn with_validator_logger(mut self, validator_logger: TraceLogger) -> Self {
        self.validator_logger = Some(validator_logger);
        self
    }

    #[cfg(feature = "cpu_validator")]
    pub fn with_validator_baud(mut self, validator_baud: u32) -> Self {
        self.validator_baud = Some(validator_baud);
        self
    }
}

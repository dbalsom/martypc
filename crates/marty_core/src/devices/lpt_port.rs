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

    devices::lpt_port.rs

    Implementation of a basic Centronics printer port. This is a component
    implementation, and must be embedded into a card implementation that can
    decode the proper port address.

*/

use crate::tracelogger::TraceLogger;
use modular_bitfield::{bitfield, prelude::*};

pub const LPT_DEFAULT_IRQ: u16 = 7;

#[bitfield]
#[derive(Copy, Clone)]
pub struct ParallelStatus {
    #[skip]
    pub unused: B3,
    pub error: B1,
    pub select: B1,
    pub paper_out: B1,
    pub ack: B1,
    pub busy: B1,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct ParallelControl {
    pub strobe: B1,
    pub auto_line_feed: B1,
    pub initialize: B1,
    pub select_in: B1,
    pub enable_irq: B1,
    #[skip]
    pub unused2: B3,
}

#[allow(dead_code)]
pub struct ParallelPort {
    data: u8,
    status: ParallelStatus,
    control: ParallelControl,
    irq: u16,
    trace_logger: TraceLogger,
}

impl Default for ParallelPort {
    fn default() -> Self {
        Self {
            data: 0,
            status: ParallelStatus::from_bytes([0]),
            control: ParallelControl::from_bytes([0]),
            irq: LPT_DEFAULT_IRQ,
            trace_logger: TraceLogger::None,
        }
    }
}

impl ParallelPort {
    pub fn new(irq: Option<u16>, trace_logger: TraceLogger) -> Self {
        Self {
            irq: irq.unwrap_or(LPT_DEFAULT_IRQ),
            trace_logger,
            ..Default::default()
        }
    }

    pub fn port_write(&mut self, port: u16, data: u8) {
        match port & 0x03 {
            0 => {
                self.data_register_write(data);
            }
            1 => {
                self.status_register_write(data);
            }
            2 => {
                self.control_register_write(data);
            }
            _ => {}
        }
    }

    pub fn port_read(&mut self, port: u16) -> u8 {
        match port & 0x03 {
            0 => {
                // CRTC address register is not readable
                self.data_register_read()
            }
            1 => {
                // CRTC data register is partially readable (depends on register selected)
                self.status_register_read()
            }
            2 => self.control_register_read(),
            _ => 0xFF,
        }
    }

    pub fn data_register_write(&mut self, data: u8) {
        self.data = data;
        self.trace_logger
            .print(format!("LPT: Data register write: {:#02X}", data));
    }

    pub fn status_register_write(&mut self, data: u8) {
        self.status = ParallelStatus::from_bytes([data]);
        self.trace_logger
            .print(format!("LPT: Status register write: {:#02X}", data));
    }

    pub fn control_register_write(&mut self, data: u8) {
        self.control = ParallelControl::from_bytes([data]);
        self.trace_logger
            .print(format!("LPT: Control register write: {:#02X}", data));
    }

    pub fn data_register_read(&mut self) -> u8 {
        self.trace_logger
            .print(format!("LPT: Data register read: {:#02X}", self.data));
        self.data
    }

    pub fn status_register_read(&mut self) -> u8 {
        let byte = self.status.into_bytes()[0];
        self.trace_logger
            .print(format!("LPT: Status register read: {:#02X}", byte));
        byte
    }

    pub fn control_register_read(&mut self) -> u8 {
        let byte = self.control.into_bytes()[0];
        self.trace_logger
            .print(format!("LPT: Control register read: {:#02X}", byte));
        byte
    }
}

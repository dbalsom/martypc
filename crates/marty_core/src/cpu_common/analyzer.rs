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
*/

//! Data structures for logic analyzer log format.

use std::collections::VecDeque;

use crate::bus::ClockFactor;

#[derive(Clone, Default)]
pub struct AnalyzerEntry {
    pub cycle: u64,
    pub address_bus: u32,
    pub ready: bool,
    pub q: bool,
    pub q_op: u8,
    pub bus_status: u8,
    pub dma_req: bool,
    pub dma_holda: bool,
    pub rni: bool,
    pub intr: bool,
    // Timer stuff
    pub clk0: bool,
    pub out0: bool,
    pub out1: bool,
    // CRTC stuff
    pub vs: bool,
    pub hs: bool,
    pub den: bool,
    // Io processed flag
    pub io: bool,
    pub io_visits: u8,
}

impl AnalyzerEntry {
    pub fn emit_header() -> &'static str {
        // Sigrok import string
        // t,x20,l,l,x2,x3,l,l,l,l,l,l,l,l,l,l,l,l
        "Time(s),addr,clk,rdy,qs,s,dr0,holda,intr,clk0,out0,out1,vs,hs,den,rni,q,io"
    }

    pub fn emit_edge(&self, clk: u8, timestep: f64) -> String {
        format!(
            "{},{:05X},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.cycle as f64 * timestep,
            self.address_bus,
            clk,
            self.ready as u8,
            self.q_op,
            self.bus_status,
            self.dma_req as u8,
            self.dma_holda as u8,
            self.intr as u8,
            self.clk0 as u8,
            self.out0 as u8,
            self.out1 as u8,
            self.vs as u8,
            self.hs as u8,
            self.den as u8,
            self.rni as u8,
            self.q as u8,
            self.io as u8
        )
    }
}

/// A representation of a logic analyzer interface for the CPU and Bus.
/// The CPU produces [AnalyzerEntry]s, one per CPU clock cycle. When devices are ran after a CPU
/// instruction is executed, they will fill out their relevant fields in the [AnalyzerEntry] deque.
/// Due to the granularity of device ticks, not every device will be ticked every CPU instruction.
/// Devices accumulate system ticks or time when run() and only tick internally when they have a
/// full tick to execute.
/// Due to this, we need to set the expected number of devices that will write to the analyzer.
/// Each device should increment the io_visits field in the AnalyzerEntry when it has written to it.
/// When all devices have written to the AnalyzerEntry, it can be popped off the deque.
pub struct LogicAnalyzer {
    pub devices:    u8,
    pub need_flush: bool,
    pub cpu_factor: ClockFactor,
    pub entries:    VecDeque<AnalyzerEntry>,
}

impl Default for LogicAnalyzer {
    fn default() -> Self {
        LogicAnalyzer {
            devices:    1,
            need_flush: false,
            cpu_factor: ClockFactor::Divisor(3),      // For 4.77Mhz PC/XT
            entries:    VecDeque::with_capacity(256), // ~200 clock cycles should be maximum instruction length
        }
    }
}

impl LogicAnalyzer {
    pub fn new(devices: u8) -> Self {
        Self {
            devices,
            ..Default::default()
        }
    }

    pub fn push(&mut self, entry: AnalyzerEntry) {
        self.entries.push_back(entry);
    }
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    pub fn set_factor(&mut self, factor: ClockFactor) {
        self.cpu_factor = factor;
    }
    pub fn commit(&mut self) -> usize {
        self.need_flush = true;
        self.entries.len()
    }
    pub fn need_flush(&self) -> bool {
        self.need_flush
    }
    pub fn pop_complete(&mut self) -> Option<AnalyzerEntry> {
        if let Some(front) = self.entries.front() {
            if front.io_visits >= self.devices {
                return self.entries.pop_front();
            }
        }
        None
    }
}

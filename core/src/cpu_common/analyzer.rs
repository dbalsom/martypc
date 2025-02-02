/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2024 Daniel Balsom

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

use crate::bus::ClockFactor;

#[derive(Clone, Default)]
pub struct AnalyzerEntry {
    pub timestamp: f64,
    pub address_bus: u32,
    pub pclk: u8,
    pub ready: u8,
    pub q_op: u8,
    pub bus_status: u8,
    pub dma_req: u8,
    pub dma_holda: u8,
    pub intr: u8,
    // Timer stuff
    pub clk0: u8,
    pub out0: u8,
    pub out1: u8,
    // CRTC stuff
    pub vs: u8,
    pub hs: u8,
    pub den: u8,
}

impl AnalyzerEntry {
    pub fn emit_header() -> &'static str {
        "Time(s),addr,clk,ready,qs,s,dr0,holda,intr,clk0,out0,out1,vs,hs,den"
    }

    pub fn emit_edge(&self, clk: u8) -> String {
        format!(
            "{},{:05X},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.timestamp,
            self.address_bus,
            clk,
            self.pclk,
            self.ready,
            self.q_op,
            self.bus_status,
            self.dma_req,
            self.dma_holda,
            self.intr,
            self.clk0,
            self.out0,
            self.out1,
            self.vs,
            self.hs,
            self.den,
        )
    }
}

pub struct LogicAnalyzer {
    pub cpu_factor: ClockFactor,
    pub entries:    Vec<AnalyzerEntry>,
}

impl Default for LogicAnalyzer {
    fn default() -> Self {
        LogicAnalyzer {
            cpu_factor: ClockFactor::Divisor(3), // For 4.77Mhz PC/XT
            entries:    Vec::with_capacity(256), // ~200 clock cycles should be maximum instruction length
        }
    }
}

impl LogicAnalyzer {
    pub fn push(&mut self, entry: AnalyzerEntry) {
        self.entries.push(entry);
    }
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    pub fn set_factor(&mut self, factor: ClockFactor) {
        self.cpu_factor = factor;
    }
}

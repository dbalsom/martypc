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

    breakpoints.rs

    Implement enum for breakpoint definitions.

*/

#[allow(dead_code)]
pub enum BreakPointType {
    StepOver(u32),       // Breakpoint on next decoded instruction
    Execute(u16, u16),   // Breakpoint on CS:IP
    ExecuteOffset(u16),  // Breakpoint on *::IP
    ExecuteFlat(u32),    // Breakpoint on CS<<4+IP
    MemAccess(u16, u16), // Breakpoint on memory access, seg::offset
    MemAccessFlat(u32),  // Breakpoint on memory access, seg<<4+offset
    Interrupt(u8),       // Breakpoint on interrupt #
    IoAccess(u16),       // Breakpoint on I/O port access
    StartWatch(u32),     // Start stopwatch at address
    StopWatch(u32),      // Stop stopwatch at address
}

pub enum StopWatchType {
    Start(u32),
    Stop(u32),
}

pub struct StopWatchData {
    pub measurements:   u64,
    pub last_cycles:    u64,
    pub total_cycles:   u64,
    pub last_duration:  u64,
    pub total_duration: u64,
}

#[derive(Clone)]
pub struct CycleStopWatch {
    pub start: u32,
    pub stop: u32,
    pub running: bool,
    pub measurements: u64,
    pub run_cycles: u64,
    pub last_cycles: u64,
    pub total_cycles: u64,
    pub last_total_cycles: u64,
    pub last_duration: u64,
    pub total_duration: u64,
}
impl Default for CycleStopWatch {
    fn default() -> Self {
        Self {
            start: 0,
            stop: 0,
            running: false,
            measurements: 0,
            run_cycles: 0,
            last_cycles: 0,
            total_cycles: 0,
            last_total_cycles: 0,
            last_duration: 0,
            total_duration: 0,
        }
    }
}

impl CycleStopWatch {
    pub fn new(start: u32, stop: u32) -> CycleStopWatch {
        CycleStopWatch {
            start,
            stop,
            ..Self::default()
        }
    }
    pub fn start(&mut self) {
        self.measurements += 1;
        self.running = true;
    }
    pub fn stop(&mut self) {
        self.last_cycles = self.run_cycles;
        self.last_total_cycles = self.total_cycles;
        self.run_cycles = 0;
        self.total_duration += self.last_duration;
        self.running = false;
    }
    #[inline]
    pub fn tick(&mut self, cycles: u64) {
        if self.running {
            self.run_cycles += cycles;
            self.total_cycles += cycles;
        }
    }
    pub fn get_data(&self) -> StopWatchData {
        StopWatchData {
            measurements:   self.measurements,
            last_cycles:    self.last_cycles,
            total_cycles:   self.last_total_cycles,
            last_duration:  self.last_duration,
            total_duration: self.total_duration,
        }
    }
    #[inline(always)]
    pub fn running(&self) -> bool {
        self.running
    }
}

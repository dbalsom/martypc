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

    /cpu_test/mod.rs - Implement data structures for JSON test generation mode.

*/

use marty_core::cpu_validator::{CycleState, VRegisters};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TestState {
    pub regs:  VRegisters,
    pub ram:   Vec<[u32; 2]>,
    pub queue: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct CpuTest {
    pub name:  String,  // Human readable name (disassembly)
    pub bytes: Vec<u8>, // Instruction bytes

    #[serde(rename = "initial")]
    pub initial_state: TestState, // Initial state of CPU before test execution

    #[serde(rename = "final")]
    pub final_state: TestState, // Final state of CPU after test execution

    pub cycles: Vec<CycleState>,
}

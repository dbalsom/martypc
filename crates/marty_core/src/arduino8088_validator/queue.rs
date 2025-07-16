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
*/
#![allow(dead_code)]

use ard808x_client::{CpuWidth, DataWidth};
use std::fmt::Display;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum QueueDataType {
    Program,
    PrefetchProgram,
    PrefetchProgramHalf,
    EndInstruction,
    Finalize,
    FinalizeHalf,
    Fill,
}

#[derive(Copy, Clone)]
pub struct QueueEntry {
    opcode: u8,
    dtype:  QueueDataType,
    addr:   u32,
}

pub struct InstructionQueue {
    width: CpuWidth,
    size: usize,
    len: usize,
    back: usize,
    front: usize,
    q: Vec<QueueEntry>,
}

impl Display for InstructionQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut base_str = "".to_string();
        for i in 0..self.len {
            base_str.push_str(&format!("{:02X}", self.q[(self.back + i) % self.size].opcode));
        }
        write!(f, "{base_str:12}")
    }
}

impl InstructionQueue {
    pub fn new(width: CpuWidth) -> Self {
        Self {
            width,
            size: width.queue_size(),
            len: 0,
            back: 0,
            front: 0,
            q: vec![
                QueueEntry {
                    opcode: 0,
                    dtype:  QueueDataType::Program,
                    addr:   0,
                };
                width.queue_size()
            ],
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn has_room(&self) -> bool {
        self.len() + usize::from(self.width) <= self.size
    }

    pub fn push(&mut self, data: u16, width: DataWidth, dtype: QueueDataType, addr: u32) {
        if self.has_room() {
            match width {
                DataWidth::EightHigh => {
                    self.q[self.front] = QueueEntry {
                        opcode: (data >> 8) as u8,
                        dtype,
                        addr,
                    };
                    self.front = (self.front + 1) % self.size;
                    self.len += 1;
                }
                DataWidth::Sixteen => {
                    let (byte_type0, byte_type1) = if matches!(dtype, QueueDataType::FinalizeHalf) {
                        (QueueDataType::Program, QueueDataType::Finalize)
                    }
                    else if matches!(dtype, QueueDataType::PrefetchProgramHalf) {
                        (QueueDataType::PrefetchProgram, QueueDataType::Program)
                    }
                    else {
                        (dtype, dtype)
                    };

                    self.q[self.front] = QueueEntry {
                        opcode: data as u8,
                        dtype: byte_type0,
                        addr,
                    };
                    self.front = (self.front + 1) % self.size;
                    self.q[self.front] = QueueEntry {
                        opcode: (data >> 8) as u8,
                        dtype:  byte_type1,
                        addr:   addr + 1,
                    };
                    self.front = (self.front + 1) % self.size;
                    self.len += 2;
                }
                _ => {
                    log::error!("Bad DataWidth for queue push: {:?}", width);
                }
            }
        }
        else {
            //panic!("Queue overrun!");
            log::error!("Queue overrun!");
        }
    }

    pub fn pop(&mut self) -> (u8, QueueDataType, u32) {
        if self.len > 0 {
            let q_entry = self.q[self.back];
            //let dt = self.dt[self.back];

            self.back = (self.back + 1) % self.size;
            self.len -= 1;

            return (q_entry.opcode, q_entry.dtype, q_entry.addr);
        }

        panic!("Queue underrun!");
    }

    pub fn flush(&mut self) {
        self.len = 0;
        self.back = 0;
        self.front = 0;
    }

    /// Write the contents of the processor instruction queue in order to the
    /// provided slice of u8.
    pub fn to_slice(&self, slice: &mut [u8]) {
        slice
            .iter_mut()
            .zip((0..self.len).map(|i| self.q[(self.back + i) % self.size].opcode))
            .for_each(|(slot, opcode)| *slot = opcode);
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut q_vec = Vec::new();

        for i in 0..self.len {
            q_vec.push(self.q[(self.back + i) % self.size].opcode);
        }

        q_vec
    }
}

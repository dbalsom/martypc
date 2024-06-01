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
*/
#![allow(dead_code)]

pub const QUEUE_SIZE: usize = 4;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum QueueDataType {
    Program,
    PrefetchProgram,
    EndInstruction,
    Finalize,
}

#[derive(Copy, Clone)]
pub struct QueueEntry {
    opcode: u8,
    dtype:  QueueDataType,
    addr:   u32,
}

pub struct InstructionQueue {
    len: usize,
    back: usize,
    front: usize,
    q: [QueueEntry; QUEUE_SIZE],
}

impl InstructionQueue {
    pub fn new() -> Self {
        Self {
            len: 0,
            back: 0,
            front: 0,
            q: [QueueEntry {
                opcode: 0,
                dtype:  QueueDataType::Program,
                addr:   0,
            }; QUEUE_SIZE],
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn push(&mut self, byte: u8, dtype: QueueDataType, addr: u32) {
        if self.len < QUEUE_SIZE {
            self.q[self.front] = QueueEntry {
                opcode: byte,
                dtype,
                addr,
            };
            //self.dt[self.front] = dtype;

            self.front = (self.front + 1) % QUEUE_SIZE;
            self.len += 1;
        }
        else {
            panic!("Queue overrun!");
        }
    }

    pub fn pop(&mut self) -> (u8, QueueDataType, u32) {
        if self.len > 0 {
            let q_entry = self.q[self.back];
            //let dt = self.dt[self.back];

            self.back = (self.back + 1) % QUEUE_SIZE;
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

    pub fn to_string(&self) -> String {
        let mut base_str = "".to_string();

        for i in 0..self.len {
            base_str.push_str(&format!("{:02X}", self.q[(self.back + i) % QUEUE_SIZE].opcode));
        }

        base_str
    }

    /// Write the contents of the processor instruction queue in order to the
    /// provided slice of u8.
    pub fn to_slice(&self, slice: &mut [u8]) {
        for i in 0..self.len {
            slice[i] = self.q[(self.back + i) % QUEUE_SIZE].opcode;
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut q_vec = Vec::new();

        for i in 0..self.len {
            q_vec.push(self.q[(self.back + i) % QUEUE_SIZE].opcode);
        }

        q_vec
    }
}

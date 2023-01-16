/*
  Marty PC Emulator 
  (C)2023 Daniel Balsom
  https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/
#![allow(dead_code)]

pub const QUEUE_SIZE: usize = 4;

#[derive (Copy, Clone, PartialEq)]
pub enum QueueDataType {
    Program,
    Finalize
}

#[derive (Copy, Clone)]
pub struct QueueEntry {
    opcode: u8,
    dtype: QueueDataType,
    addr: u32
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
            q: [
                QueueEntry {
                    opcode: 0,
                    dtype: QueueDataType::Program,
                    addr: 0,
                }; QUEUE_SIZE
            ],
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
                addr
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

            return (q_entry.opcode, q_entry.dtype, q_entry.addr)
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


}
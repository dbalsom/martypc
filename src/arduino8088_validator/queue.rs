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

pub const QUEUE_SIZE: usize = 4;

#[derive (Copy, Clone)]
pub enum QueueDataType {
    Program,
    Finalize
}

pub struct InstructionQueue {
    len: usize,
    back: usize,
    front: usize,
    q: [u8; QUEUE_SIZE],
    dt: [QueueDataType; QUEUE_SIZE]
}

impl InstructionQueue {
    pub fn new() -> Self {
        Self {
            len: 0,
            back: 0,
            front: 0,
            q: [0,0,0,0],
            dt: [
                QueueDataType::Program,
                QueueDataType::Program,
                QueueDataType::Program,
                QueueDataType::Program
            ]
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn push(&mut self, byte: u8, dtype: QueueDataType) {
        if self.len < QUEUE_SIZE {

            self.q[self.front] = byte;
            self.dt[self.front] = dtype;

            self.front = (self.front + 1) % QUEUE_SIZE;
            self.len += 1;
        }
        else {
            panic!("Queue overrun!");
        }
    }

    pub fn pop(&mut self) -> (u8, QueueDataType) {
        if self.len > 0 {
            let byte = self.q[self.back];
            let dt = self.dt[self.back];

            self.back = (self.back + 1) % QUEUE_SIZE;
            self.len -= 1;

            return (byte, dt)
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
            base_str.push_str(&format!("{:02X}", self.q[(self.back + i) % QUEUE_SIZE]));
        }

        base_str
    }


}
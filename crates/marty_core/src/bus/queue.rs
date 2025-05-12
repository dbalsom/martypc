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
*/

//! An implementation of the ByteQueue trait for Bus.
//! This allows a CPU to decode instructions directly from memory.

use super::*;
use crate::bytequeue::{ByteQueue, QueueReader, QueueType};

impl ByteQueue for BusInterface {
    fn seek(&mut self, pos: usize) {
        self.cursor = pos;
    }

    fn tell(&self) -> usize {
        self.cursor
    }

    fn wait(&mut self, _cycles: u32) {}
    fn wait_i(&mut self, _cycles: u32, _instr: &[u16]) {}
    fn wait_comment(&mut self, _comment: &str) {}
    fn set_pc(&mut self, _pc: u16) {}

    fn q_read_u8(&mut self, _dtype: QueueType, _reader: QueueReader) -> u8 {
        if self.cursor < self.memory.len() {
            let (b, _) = self.read_u8(self.cursor, 0).unwrap_or((0xFF, 0));
            self.cursor += 1;
            return b;
        }
        0xffu8
    }

    fn q_read_i8(&mut self, _dtype: QueueType, _reader: QueueReader) -> i8 {
        if self.cursor < self.memory.len() {
            let (b, _) = self.read_u8(self.cursor, 0).unwrap_or((0xFF, 0));
            self.cursor += 1;
            return b as i8;
        }
        -1i8
    }

    fn q_read_u16(&mut self, _dtype: QueueType, _reader: QueueReader) -> u16 {
        if self.cursor < self.memory.len() - 1 {
            let (b0, _) = self.read_u8(self.cursor, 0).unwrap_or((0xFF, 0));
            let (b1, _) = self.read_u8(self.cursor + 1, 0).unwrap_or((0xFF, 0));
            self.cursor += 2;
            return b0 as u16 | (b1 as u16) << 8;
        }
        0xffffu16
    }

    fn q_read_i16(&mut self, _dtype: QueueType, _reader: QueueReader) -> i16 {
        if self.cursor < self.memory.len() - 1 {
            let (b0, _) = self.read_u8(self.cursor, 0).unwrap_or((0xFF, 0));
            let (b1, _) = self.read_u8(self.cursor + 1, 0).unwrap_or((0xFF, 0));
            self.cursor += 2;
            return (b0 as u16 | (b1 as u16) << 8) as i16;
        }
        -1i16
    }

    fn q_peek_u8(&mut self) -> u8 {
        if self.cursor < self.memory.len() {
            let b = self.peek_u8(self.cursor).unwrap_or(0xFF);
            return b;
        }
        0xffu8
    }

    fn q_peek_i8(&mut self) -> i8 {
        if self.cursor < self.memory.len() {
            let b = self.peek_u8(self.cursor).unwrap_or(0xFF);
            return b as i8;
        }
        -1i8
    }

    fn q_peek_u16(&mut self) -> u16 {
        if self.cursor < self.memory.len() - 1 {
            return self.peek_u8(self.cursor).unwrap_or(0xFF) as u16
                | (self.peek_u8(self.cursor + 1).unwrap_or(0xFF) as u16) << 8;
        }
        0xffffu16
    }

    fn q_peek_i16(&mut self) -> i16 {
        if self.cursor < self.memory.len() - 1 {
            return (self.peek_u8(self.cursor).unwrap_or(0xFF) as u16
                | (self.peek_u8(self.cursor + 1).unwrap_or(0xFF) as u16) << 8) as i16;
        }
        -1i16
    }

    fn q_peek_farptr16(&mut self) -> (u16, u16) {
        if self.cursor < self.memory.len() - 3 {
            let offset: u16 = self.peek_u8(self.cursor).unwrap_or(0xFF) as u16
                | (self.peek_u8(self.cursor + 1).unwrap_or(0xFF) as u16) << 8;
            let segment: u16 = self.peek_u8(self.cursor + 2).unwrap_or(0xFF) as u16
                | (self.peek_u8(self.cursor + 3).unwrap_or(0xFF) as u16) << 8;
            return (segment, offset);
        }
        (0xffffu16, 0xffffu16)
    }
}

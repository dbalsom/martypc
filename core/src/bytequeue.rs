/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

    bytequeue.rs

    Implements the ByteQueue trait. ByteQueue is implemented by both Bus and 
    CPU to permit decoding of instructions from either emulator memory or the
    emulated processor instruction queue.
*/

#[derive (Copy, Clone)]
pub enum QueueType {
    First,
    Subsequent
}

#[derive (Copy, Clone)]
pub enum QueueReader {
    Biu,
    Eu,
}

impl Default for QueueType {
    fn default() -> Self { 
        QueueType::First 
    }
}

pub trait ByteQueue {
    fn seek(&mut self, pos: usize);
    fn tell(&self) -> usize;

    fn delay(&mut self, delay: u32);
    fn clear_delay(&mut self);

    fn wait(&mut self, cycles: u32);
    fn wait_i(&mut self, cycles: u32, instr: &[u16]);
    fn wait_comment(&mut self, comment: &'static str);
    fn set_pc(&mut self, pc: u16);
    
    fn q_read_u8(&mut self, qtype: QueueType, reader: QueueReader) -> u8;
    fn q_read_i8(&mut self, qtype: QueueType, reader: QueueReader) -> i8;
    fn q_read_u16(&mut self, qtype: QueueType, reader: QueueReader) -> u16;
    fn q_read_i16(&mut self, qtype: QueueType, reader: QueueReader) -> i16;

    fn q_peek_u8(&mut self) -> u8;
    fn q_peek_i8(&mut self) -> i8;
    fn q_peek_u16(&mut self) -> u16;
    fn q_peek_i16(&mut self) -> i16;
    fn q_peek_farptr16(&mut self) -> (u16, u16);
}
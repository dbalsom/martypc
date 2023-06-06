/*
    MartyPC Emulator 
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
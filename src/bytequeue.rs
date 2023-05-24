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
#[derive (Copy, Clone)]
pub enum QueueType {
    First,
    Subsequent
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
    fn wait(&mut self, cycles: u32);
    fn wait_i(&mut self, cycles: u32, instr: &[u16]);

    fn clear_delay(&mut self);

    fn q_read_u8(&mut self, qtype: QueueType) -> u8;
    fn q_read_i8(&mut self, qtype: QueueType) -> i8;
    fn q_read_u16(&mut self, qtype: QueueType) -> u16;
    fn q_read_i16(&mut self, qtype: QueueType) -> i16;

    fn q_peek_u8(&self) -> u8;
    fn q_peek_i8(&self) -> i8;
    fn q_peek_u16(&self) -> u16;
    fn q_peek_i16(&self) -> i16;
}
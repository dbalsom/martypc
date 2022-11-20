pub trait ByteQueue {
    fn seek(&mut self, pos: usize);
    fn tell(&self) -> usize;

    fn q_read_u8(&mut self) -> u8;
    fn q_read_i8(&mut self) -> i8;

    fn q_read_u16(&mut self) -> u16;
    fn q_read_i16(&mut self) -> i16;
}
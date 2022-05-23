
pub trait ByteInterface {
    fn set_cursor(&mut self, pos: usize);
    fn tell(&self) -> usize;
    fn read_u8(&mut self, cost: &mut u32) -> u8;
    fn read_i8(&mut self, cost: &mut u32) -> i8;
    fn write_u8(&mut self, data: u8, cost: &mut u32);
    fn write_i8(&mut self, data: i8, cost: &mut u32);

    fn read_u16(&mut self, cost: &mut u32) -> u16;
    fn read_i16(&mut self, cost: &mut u32) -> i16;
    fn write_u16(&mut self, data: u16, cost: &mut u32);
    fn write_i16(&mut self, data: i16, cost: &mut u32);
}
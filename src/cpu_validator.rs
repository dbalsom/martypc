
#[derive (PartialEq, Copy, Clone)]
pub enum ReadType {
    Code,
    Data
}

#[derive (Copy, Clone, Default, PartialEq)]
pub struct VRegisters {
    pub ax: u16,
    pub bx: u16,
    pub cx: u16,
    pub dx: u16,
    pub cs: u16,
    pub ss: u16,
    pub ds: u16,
    pub es: u16,
    pub sp: u16,
    pub bp: u16,
    pub si: u16,
    pub di: u16,
    pub ip: u16,
    pub flags: u16
}
pub trait CpuValidator {
    fn init(&mut self, mask_flags: bool) -> bool;
    fn begin(&mut self, regs: &VRegisters );
    fn validate(&mut self, name: String, instr: &[u8], has_modrm: bool, cycles: i32, regs: &VRegisters);

    fn emu_read_byte(&mut self, addr: u32, data: u8, read_type: ReadType);
    fn emu_write_byte(&mut self, addr: u32, data: u8);
    fn discard_op(&mut self);
}


#[derive (PartialEq, Copy, Clone)]
pub enum ValidatorType {
    NoValidator,
    PiValidator,
    ArduinoValidator
}

#[derive (Copy, Clone, Default)]
pub struct VRegisters {
    pub ax: u16,
    pub ah: u8,
    pub al: u8,
    pub bx: u16,
    pub bh: u8,
    pub bl: u8,
    pub cx: u16,
    pub ch: u8,
    pub cl: u8,
    pub dx: u16,
    pub dh: u8,
    pub dl: u8,

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

    //fn new(path: &str);
    fn begin(&mut self, regs: &VRegisters );
    fn end(&mut self, name: String, opcode: u8, modregrm: bool, cycles: i32, regs: &VRegisters);
    fn emu_read_byte(&mut self, addr: u32, data: u8);
    fn emu_write_byte(&mut self, addr: u32, data: u8);
    fn discard_op(&mut self);
}
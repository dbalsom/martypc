

use log;

pub enum InterruptType {
    NonMaskable,
    Hardware,
    Software
}

pub struct InterruptInterface {

    interrupts: Vec<Interrupt>
}

pub struct Interrupt {
    itype: InterruptType,
    irq: u8,
    
}

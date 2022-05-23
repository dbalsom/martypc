/*
    pic.rc
    Implement the 8259 PIC (Programmable Interrupt Controller)

*/

use crate::io::{IoBusInterface, IoDevice};


pub const PIC_INTERRUPT_OFFSET: u8 = 8;

pub const PIC_COMMAND_PORT: u16 = 0x20;
pub const PIC_DATA_PORT: u16    = 0x21;

const ICW1_ICW4_NEEDED: u8      = 0b0000_0001; // Bit set if a 4th control world is required (not supported)
const ICW1_SINGLE_MODE: u8      = 0b0000_0010; // Bit is set if PIC is operating in signle mode (only supported configuration)
const ICW1_ADI: u8              = 0b0000_0100; // Bit is set if PIC is using a call address interval of 4, otherwise 8
const ICW1_LTIML: u8            = 0b0000_1000; // Bit is set if PIC is in Level Triggered Mode
const ICW1_IS_ICW1: u8          = 0b0001_0000; // Bit determines if input is ICW1

const ICW4_8088_MODE: u8        = 0b0000_0001; // Bit on if 8086/8088 mode (required)
const ICW4_AEOI_MODE: u8        = 0b0000_0010; // Bit on if Auto EOI is enabled
const ICW4_BUFFERED:  u8        = 0b0000_1000; // Bit on if Buffered mode
const ICW4_NESTED: u8           = 0b0001_0000; // Bit on if Fully Nested mode

const OCW1_IS_OCW1: u8          = 0b1000_0000; // Bit determines if input is OCW1

pub enum InitializationState {
    Normal,             // Normal operation, can receive an ICW1 at any point
    ExpectingICW2,      // In initialization sequence, expecting ICW2
    ExpectingICW4       // In initialization sequence, expecting ICW4
}
pub enum ReadSelect {
    ISR,
    IRR
}

pub type PicRequestFn = fn (&mut Pic, interrupt: u8);
pub struct Pic {

    init_state: InitializationState,    // Initialization state for expecting various ICWs
    int_offset: u8,          // Interrupt Vector Offset (Always 8 on IBM PC)
    imr: u8,                 // Interrupt Mask Register
    isr: u8,                 // In-Service Register
    irr: u8,                 // Interrupt Request Register
    read_select: ReadSelect, // Select register to read.  True=ISR, False=IRR
    irq: u8,                 // IRQ Number
    int_request: bool,       // INT request line of PIC
    buffered: bool,          // Buffered mode
    nested: bool,            // Nested mode
    special_nested: bool,    // Special fully nested mode
    polled: bool,            // Polled mode
    auto_eoi: bool,          // Auto-EOI mode
    rotate_on_aeoi: bool,    // Should rotate in Auto-EOI mode
    expecting_icw2: bool,
    expecting_icw4: bool,    // ICW3 not supported in Single mode operation
    error: bool,             // We encountered an invalid condition or request
}

impl IoDevice for Pic {

    fn read_u8(&mut self, port: u16) -> u8 {
        match port {
            PIC_COMMAND_PORT => {
                self.handle_command_register_read()
            },
            PIC_DATA_PORT => {
                self.handle_data_register_read()
            },
            _ => unreachable!("PIC: Bad port #")
        }        
    }
    fn write_u8(&mut self, port: u16, data: u8) {
        match port {
            PIC_COMMAND_PORT => {
                self.handle_command_register_write(data);
            },
            PIC_DATA_PORT => {
                self.handle_data_register_write(data);
            },
            _ => unreachable!("PIC: Bad port #")
        }    
    }    
    fn read_u16(&mut self, port: u16) -> u16 {
        match port {
            PIC_COMMAND_PORT => {
                0
            },
            PIC_DATA_PORT => {
                0
            },
            _ => unreachable!("PIC: Bad port #")
        }
    }
    fn write_u16(&mut self, port: u16, data: u16) {
        match port {
            PIC_COMMAND_PORT => {
            },
            PIC_DATA_PORT => {
                // Set Mask
            },
            _ => unreachable!("PIC: Bad port #")
        }
    }
}

impl Pic {
    pub fn new() -> Self {
        Self {
            init_state: InitializationState::Normal,
            int_offset: PIC_INTERRUPT_OFFSET,    // Interrupt Vector Offset is always 8
            imr: 0xFF,                           // All IRQs initially masked
            isr: 0x00,
            irr: 0,
            read_select: ReadSelect::IRR,
            irq: 0,
            int_request: false,
            buffered: false,
            nested: true,
            special_nested: false,
            polled: false,
            auto_eoi: false,
            rotate_on_aeoi: false,
            expecting_icw2: false,
            expecting_icw4: false,
            error: false,
        }
    }

    pub fn handle_command_register_write(&mut self, byte: u8) {
        // Specific bit set inidicates an Initialization Command Word 1 (ICW1) (actually a byte)

        if byte & ICW1_IS_ICW1 != 0 {
            // Parse Initialization Command Word
            if let InitializationState::Normal = self.init_state {
                log::debug!("PIC: Read ICW1: {:02X}", byte);
            }
            else {
                log::warn!("PIC: Warning: Received unexpected ICW1: {:02X}", byte);
            }

            if byte & ICW1_SINGLE_MODE == 0 { 
                log::error!("PIC: Error: Chained mode not supported");
                self.error = true;
            }
            if byte & ICW1_ADI != 0 {
                log::error!("PIC: Error: 4 byte ADI unsupported");
                self.error = true;
            }

            self.init_state = InitializationState::ExpectingICW2;
            if byte & ICW1_ICW4_NEEDED != 0 {
                self.expecting_icw4 = true;
            }
        }
    }

    pub fn handle_data_register_write(&mut self, byte: u8) {
        // Handle ICW2 & ICW4 (ICW3 skipped in Single mode)
        match self.init_state {
            InitializationState::Normal => {
                // We aren't expecting any ICWs, so treat this write as a set of the IMR
                log::trace!("PIC: Set IMR to: {:02X}", byte);
                self.set_imr(byte);
            }
            InitializationState::ExpectingICW2 => {
                // This value should be an ICW2 based on just receiving an ICW1 on control port

                log::debug!("PIC: Read ICW2: {:02X}", byte);
                self.init_state = InitializationState::ExpectingICW4;
            }
            InitializationState::ExpectingICW4 => {
                // This value should be an ICW4 based on receiving an ICW2 (ICW3 skipped in Single mode)
                log::debug!("PIC: Read ICW4: {:02X}", byte);
                self.init_state = InitializationState::Normal;

                if byte & ICW4_8088_MODE == 0 {
                    log::error!("PIC: Error: MCS-80/85 mode unsupported");
                    self.error = true;
                }
                self.auto_eoi = byte & ICW4_AEOI_MODE != 0;
                self.buffered = byte & ICW4_BUFFERED != 0;
                if byte & ICW4_NESTED != 0 {
                    log::error!("PIC: Error: MCS-80/85 mode unsupported");
                    self.error = true;
                }
            }
        }

        // Handle Operational Control Words (again, only bytes)

    }

    pub fn handle_command_register_read(&mut self) -> u8 {
        match self.read_select {
            ReadSelect::ISR => {
                self.isr
            }
            ReadSelect::IRR => {
                self.irr
            }
        }
    }

    pub fn handle_data_register_read(&mut self) -> u8 {
        self.imr
    }

    fn set_imr(&mut self, byte: u8) {
        self.imr = byte;
    }

    pub fn request_interrupt(&mut self, interrupt: u8) {
        // Called by a device to request interrupt service

        if interrupt > 7 {
            panic!("PIC: Received interrupt out of range: {}", interrupt);
        }
        //log::trace!("PIC: Interrupt {} requested by device", interrupt);

        // Interrupts 0-7 map to bits 0-7 in IMR register
        let intr_bit: u8 = 0x01 << interrupt;

        if self.imr & intr_bit != 0 {
            // If the corresponding bit is set in the IMR, do not process
        }
        else if self.isr & intr_bit != 0 {
            // If the corresponding bit is set in the ISR, do not process
        }
        else {
            // Interrupt is not masked or in service, process
            // Set bit in Interrupt Request Register
            self.irr |= intr_bit; 

            // set INT request line high
            self.int_request = true;
        }

    }

    pub fn query_interrupt_line(&self) -> bool {
         
        self.int_request
    }

    pub fn get_interrupt_vector(&mut self) -> Option<u8> {

        // Only handling timer interrupts for now
        // Return the highest priority vector from the IRR
        let intr_bit: u8 = 0x01;
        for irq in 0..8 {
            let intr_bit = intr_bit << irq;
            if self.irr & intr_bit  != 0 {
                
                // found highest priority IRR

                // Clear it
                self.irr &= !intr_bit;
                // Set it in ISR being serviced
                self.isr |= intr_bit;
                self.irq = irq;
                // INT line low
                self.int_request = false;

                return Some(irq + PIC_INTERRUPT_OFFSET)
            }
        }
        None
    }

    pub fn end_of_interrupt(&mut self) {
        // Clear ISR bit
        let intr_bit: u8 = 0x01 << self.irq;
        self.isr &= !intr_bit;
    }
}
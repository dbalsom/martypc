/*
    pic.rc
    Implement the 8259 PIC (Programmable Interrupt Controller)

*/

#![allow(dead_code)]

//use std::io::Read;

use crate::bus::{BusInterface, IoDevice, DeviceRunTimeUnit};


pub const PIC_INTERRUPT_OFFSET: u8 = 8;

pub const PIC_COMMAND_PORT: u16 = 0x20;
pub const PIC_DATA_PORT: u16    = 0x21;

const ICW1_ICW4_NEEDED: u8      = 0b0000_0001; // Bit set if a 4th control world is required (not supported)
const ICW1_SINGLE_MODE: u8      = 0b0000_0010; // Bit is set if PIC is operating in signle mode (only supported configuration)
const ICW1_ADI: u8              = 0b0000_0100; // Bit is set if PIC is using a call address interval of 4, otherwise 8
const ICW1_LTIM: u8             = 0b0000_1000; // Bit is set if PIC is in Level Triggered Mode
const ICW1_IS_ICW1: u8          = 0b0001_0000; // Bit determines if input is ICW1

const ICW4_8088_MODE: u8        = 0b0000_0001; // Bit on if 8086/8088 mode (required)
const ICW4_AEOI_MODE: u8        = 0b0000_0010; // Bit on if Auto EOI is enabled
const ICW4_BUFFERED:  u8        = 0b0000_1000; // Bit on if Buffered mode
const ICW4_NESTED: u8           = 0b0001_0000; // Bit on if Fully Nested mode

const OCW_IS_OCW3: u8           = 0b0000_1000; // Bit on if OCW is OCW3

const OCW2_NONSPECIFIC_EOI: u8  = 0b0010_0000;
const OCW2_SPECIFIC_EOI: u8     = 0b0110_0000;
const OCW3_POLL_COMMAND: u8     = 0b0000_0100;
const OCW3_RR_COMMAND: u8       = 0b0000_0011;

pub enum InitializationState {
    Normal,             // Normal operation, can receive an ICW1 at any point
    ExpectingICW2,      // In initialization sequence, expecting ICW2
    ExpectingICW4       // In initialization sequence, expecting ICW4
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TriggerMode {
    Edge,
    Level
}


#[derive(Copy, Clone)]
pub enum ReadSelect {
    ISR,
    IRR
}

#[derive(Copy, Clone)]
pub struct InterruptStats {
    imr_masked_count: u64,
    isr_masked_count: u64,
    serviced_count: u64
}


impl InterruptStats {
    pub fn new() -> Self {
        Self {
            imr_masked_count: 0,
            isr_masked_count: 0,
            serviced_count: 0
        }
    }
}

pub type PicRequestFn = fn (&mut Pic, interrupt: u8);
pub struct Pic {

    init_state: InitializationState,    // Initialization state for expecting various ICWs
    int_offset: u8,          // Interrupt Vector Offset (Always 8 on IBM PC)
    imr: u8,                 // Interrupt Mask Register
    isr: u8,                 // In-Service Register
    irr: u8,                 // Interrupt Request Register
    ir: u8,                  // IR lines (bitfield)
    read_select: ReadSelect, // Select register to read.  True=ISR, False=IRR
    irq: u8,                 // IRQ Number
    intr: bool,       // INT request line of PIC
    buffered: bool,          // Buffered mode
    nested: bool,            // Nested mode
    special_nested: bool,    // Special fully nested mode
    polled: bool,            // Polled mode
    auto_eoi: bool,          // Auto-EOI mode
    rotate_on_aeoi: bool,    // Should rotate in Auto-EOI mode
    trigger_mode: TriggerMode,
    expecting_icw2: bool,
    expecting_icw4: bool,    // ICW3 not supported in Single mode operation
    error: bool,             // We encountered an invalid condition or request

    interrupt_stats: Vec<InterruptStats>
}

#[derive(Clone, Default)]
pub struct PicStringState {
    pub imr: String,
    pub isr: String,
    pub irr: String,
    pub ir: String,
    pub intr: String,
    pub autoeoi: String,
    pub trigger_mode: String,
    pub interrupt_stats: Vec<(String, String, String)>
}

impl IoDevice for Pic {

    fn read_u8(&mut self, port: u16, _delta: DeviceRunTimeUnit) -> u8 {
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
    fn write_u8(&mut self, port: u16, data: u8, bus: Option<&mut BusInterface>, _delta: DeviceRunTimeUnit) {
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

    fn port_list(&self) -> Vec<u16> {
        vec![PIC_COMMAND_PORT, PIC_DATA_PORT]
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
            ir: 0,
            read_select: ReadSelect::IRR,
            irq: 0,
            intr: false,
            buffered: false,
            nested: true,
            special_nested: false,
            polled: false,
            auto_eoi: false,
            trigger_mode: TriggerMode::Edge,
            rotate_on_aeoi: false,
            expecting_icw2: false,
            expecting_icw4: false,
            error: false,
            interrupt_stats: vec![InterruptStats::new(); 8]
        }
    }

    pub fn reset(&mut self) {
        self.init_state = InitializationState::Normal;
        self.imr = 0xFF;
        self.isr = 0x00;
        self.irr = 0x00;
        self.ir = 0x00;
        self.read_select = ReadSelect::IRR;
        self.irq = 0;
        self.intr = false;
        self.buffered = false;
        self.nested = true;
        self.special_nested = false;
        self.polled = false;
        self.auto_eoi = false;
        self.rotate_on_aeoi = false;
        self.expecting_icw2 = false;
        self.expecting_icw4 = false;
        self.error = false;

        for stat_entry in &mut self.interrupt_stats {
            stat_entry.imr_masked_count = 0;
            stat_entry.isr_masked_count = 0;
            stat_entry.serviced_count = 0;
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

            if byte & ICW1_LTIM != 0 {
                self.trigger_mode = TriggerMode::Level;
            }
            else {
                self.trigger_mode = TriggerMode::Edge;
            }

            self.init_state = InitializationState::ExpectingICW2;
            if byte & ICW1_ICW4_NEEDED != 0 {
                self.expecting_icw4 = true;
            }
        }
        else if byte & OCW2_NONSPECIFIC_EOI != 0 {
            self.eoi(None);
        }
        else if byte & OCW2_SPECIFIC_EOI != 0 {
            self.eoi(Some(byte & 0x07));
        }
        else if byte & OCW_IS_OCW3 != 0  { 
            
            let rr = match byte & OCW3_RR_COMMAND {
                0b10 => {
                    //log::debug!("PIC: OCW3 Read Selected IRR register");
                    ReadSelect::IRR
                },
                0b11 => {
                    //log::debug!("PIC: OCW3 Read Selected ISR register");
                    ReadSelect::ISR
                }
                _ => self.read_select
            };
            self.read_select = rr;

        }
        else {
            log::trace!("PIC: Unhandled command: {:02X}", byte)
        }
    }

    /// Perform an EOI (End of interrupt)
    /// An EOI resets a bit in the ISR.
    /// If an IR number is provided, it will perform a specific EOI and reset a specific bit.
    /// If None is provided, it will perform a non-specific EOI and reset the highest priority bit.
    pub fn eoi(&mut self, line: Option<u8>)  {

        if let Some(ir) = line {
            // Specific EOI

            self.isr = Pic::clear_bit(self.isr, ir);
            // Is there a corresponding bit set in the IRR?
            if Pic::check_bit(self.irr, ir) {
                // Raise INTR for new interrupt.
                self.intr = true;
            }
        }
        else {

            let ir = self.get_highest_priority_is();

            self.isr = Pic::clear_bit(self.isr, ir);
            // Is there a corresponding bit set in the IRR?
            if Pic::check_bit(self.irr, ir) {
                // Raise INTR for new interrupt.
                self.intr = true;
            }            
        }
    }

    pub fn get_highest_priority_ir(&self) -> u8 {

        let mask: u8 = 0x01;
        let mut ir = 0;
        
        for i in 0..8 {
            ir = i;
            if self.irr & (mask << ir) != 0 {
                break;
            }
        }
        ir
    }

    pub fn get_highest_priority_is(&self) -> u8 {

        let mask: u8 = 0x01;
        let mut ir = 0;

        for i in 0..8 {
            ir = i;
            if self.isr & (mask << ir) != 0 {
                break;
            }
        }
        ir
    }    

    pub fn clear_lsb(byte: u8) -> u8 {

        let mut mask: u8 = 0x01;
        let mut byte = byte;
        for _ in 0..8 {
            if byte & mask != 0 {
                byte &= !mask;
                break;
            }
            mask <<= 1;
        }
        byte
    }

    pub fn clear_bit(byte: u8, bitn: u8) -> u8 {

        let mut mask: u8 = 0x01;
        mask <<= bitn;

        byte & !mask
    }

    pub fn check_bit(byte: u8, bitn: u8) -> bool {

        let mut mask: u8 = 0x01;
        mask <<= bitn;

        byte & mask != 0
    }

    pub fn handle_data_register_write(&mut self, byte: u8) {
        // Handle ICW2 & ICW4 (ICW3 skipped in Single mode)
        match self.init_state {
            InitializationState::Normal => {
                // We aren't expecting any ICWs, so treat this write as a set of the IMR
                log::trace!("PIC: Set IMR to: {:02X}", byte);
                self.set_imr(byte);
                return;
            }
            InitializationState::ExpectingICW2 => {
                // This value should be an ICW2 based on just receiving an ICW1 on control port

                log::debug!("PIC: Read ICW2: {:02X}", byte);
                self.init_state = InitializationState::ExpectingICW4;
                return;
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
                return;
            }
        }

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

        // Changing the IMR will allow devices with current high IR lines to generate interrupts
        // in Level triggered mode.  In Edge triggered mode they will not.
        self.imr = byte;

        let mut ir_bit = 0x01;
        for interrupt in 0..8 {

            let have_request = ir_bit & self.irr != 0;
            let is_masked = ir_bit & self.imr != 0;
            let is_in_service = ir_bit & self.isr != 0;

            if self.trigger_mode == TriggerMode::Level && have_request && !is_masked && !is_in_service {
                // (Set INT request line high)
                self.intr = true;
                self.interrupt_stats[interrupt as usize].serviced_count += 1;
            }

            ir_bit <<= 1;
        }
    }

    pub fn request_interrupt(&mut self, interrupt: u8) {
        // Called by a device to request interrupt service
        // Simulates IR line going high

        if interrupt > 7 {
            panic!("PIC: Received interrupt out of range: {}", interrupt);
        }

        //log::trace!("PIC: Interrupt {} requested by device", interrupt);

        // Interrupts 0-7 map to bits 0-7 in IMR register
        let intr_bit: u8 = 0x01 << interrupt;
        // Set IR line high and set the request bit in the IRR register 
        self.ir |= intr_bit;
        self.irr |= intr_bit; 

        if self.imr & intr_bit != 0 {
            // If the corresponding bit is set in the IMR, it is masked: do not process right now
            self.interrupt_stats[interrupt as usize].imr_masked_count += 1;
        }
        else if self.isr & intr_bit != 0 {
            // If the corresponding bit is set in the ISR, do not process right now
            self.interrupt_stats[interrupt as usize].isr_masked_count += 1;
        }
        else {
            // Interrupt is not masked or already in service, process it...
            // (Set INT request line high)
            self.intr = true;
            self.interrupt_stats[interrupt as usize].serviced_count += 1;
        }
    }

    pub fn clear_interrupt(&mut self, interrupt: u8) {
        // Called by device to withdraw interrupt service request
        // Simulates IR line going low
        if interrupt > 7 {
            panic!("PIC: Received interrupt out of range: {}", interrupt);
        }

        // Clear the corresponding bit in the IR lines
        let intr_bit: u8 = 0x01 << interrupt;
        self.ir &= !intr_bit;
    }

    pub fn query_interrupt_line(&self) -> bool {
        self.intr
    }

    /// Represents the PIC's response to the 2nd INTA 'pulse'. The PIC will put the 
    /// highest-priority interrupt vector onto the bus.
    pub fn get_interrupt_vector(&mut self) -> Option<u8> {

        //log::trace!("Getting interrupt vector, auto-eoi: {:?}.", self.auto_eoi);

        // Return the highest priority vector not currently masked from the IRR
        let mut ir_bit: u8 = 0x01;
        for irq in 0..8 {

            let have_request = ir_bit & self.irr != 0;
            let is_masked = ir_bit & self.imr != 0;
            let _is_in_service = ir_bit & self.isr != 0;

            if have_request && !is_masked {
                // found highest priority IRR not masked

                // Clear its bit in the IR...
                self.irr &= !ir_bit;
                // ...and set it in ISR being serviced
                self.isr |= ir_bit;
                // ...unless Auto-EOI is on
                if self.auto_eoi {
                    //log::trace!("Executing Auto-EOI");
                    self.isr &= !ir_bit;
                }
                self.irq = irq;
                // INT line low
                self.intr = false;

                return Some(irq + PIC_INTERRUPT_OFFSET)
            }
            ir_bit <<= 1;
        }

        None
    }

    pub fn get_string_state(&self) -> PicStringState {
    
        let mut state = PicStringState {
            imr: format!("{:08b}", self.imr),
            isr: format!("{:08b}", self.isr),
            irr: format!("{:08b}", self.irr),
            ir: format!("{:08b}", self.ir),
            intr: format!("{}", self.intr),
            autoeoi: format!("{:?}", self.auto_eoi),
            trigger_mode: format!("{:?}", self.trigger_mode),
            interrupt_stats: Vec::new()
        };

        for i in 0..8 {
            state.interrupt_stats.push(
                ( 
                    format!("{}", self.interrupt_stats[i].imr_masked_count), 
                    format!("{}", self.interrupt_stats[i].isr_masked_count), 
                    format!("{}", self.interrupt_stats[i].serviced_count )
                ));
        }
        state
    }
}
/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2024 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the “Software”),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.

    --------------------------------------------------------------------------

    devices::pic.rs

    Implements the 8259 PIC (Programmable Interrupt Controller)

*/

#![allow(dead_code)]

//use std::io::Read;

use crate::bus::{BusInterface, DeviceRunTimeUnit, IoDevice};

//pub const PIC_INTERRUPT_OFFSET: u8 = 8;

pub const PIC_COMMAND_PORT: u16 = 0x20;
pub const PIC_DATA_PORT: u16 = 0x21;

const ICW1_ICW4_NEEDED: u8 = 0b0000_0001; // Bit set if a 4th control world is required (not supported)
const ICW1_SINGLE_MODE: u8 = 0b0000_0010; // Bit is set if PIC is operating in single mode (only supported configuration)
const ICW1_ADI: u8 = 0b0000_0100; // Bit is set if PIC is using a call address interval of 4, otherwise 8
const ICW1_LTIM: u8 = 0b0000_1000; // Bit is set if PIC is in Level Triggered Mode
const ICW1_IS_ICW1: u8 = 0b0001_0000; // Bit determines if input is ICW1

const ICW2_MASK: u8 = 0b1111_1000; // Bit mask for ICW2 offset

const ICW4_8088_MODE: u8 = 0b0000_0001; // Bit on if 8086/8088 mode (required)
const ICW4_AEOI_MODE: u8 = 0b0000_0010; // Bit on if Auto EOI is enabled
const ICW4_BUFFERED: u8 = 0b0000_1000; // Bit on if Buffered mode
const ICW4_NESTED: u8 = 0b0001_0000; // Bit on if Fully Nested mode

const OCW_IS_OCW3: u8 = 0b0000_1000; // Bit on if OCW is OCW3

const OCW2_NONSPECIFIC_EOI: u8 = 0b0010_0000;
const OCW2_SPECIFIC_EOI: u8 = 0b0110_0000;
const OCW3_POLL_COMMAND: u8 = 0b0000_0100;
const OCW3_RR_COMMAND: u8 = 0b0000_0011;

const SPURIOUS_INTERRUPT: u8 = 7;

pub enum InitializationState {
    Normal,        // Normal operation, can receive an ICW1 at any point
    ExpectingICW2, // In initialization sequence, expecting ICW2
    ExpectingICW4, // In initialization sequence, expecting ICW4
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TriggerMode {
    Edge,
    Level,
}

#[derive(Copy, Clone)]
pub enum ReadSelect {
    ISR,
    IRR,
}

#[derive(Copy, Clone)]
pub struct InterruptStats {
    imr_masked_count: u64,
    isr_masked_count: u64,
    serviced_count:   u64,
}

impl InterruptStats {
    pub fn new() -> Self {
        Self {
            imr_masked_count: 0,
            isr_masked_count: 0,
            serviced_count:   0,
        }
    }
}

pub type PicRequestFn = fn(&mut Pic, interrupt: u8);

pub struct Pic {
    init_state: InitializationState, // Initialization state for expecting various ICWs
    int_offset: u8,                  // Interrupt Vector Offset (Always 8 on IBM PC)
    imr: u8,                         // Interrupt Mask Register
    isr: u8,                         // In-Service Register
    irr: u8,                         // Interrupt Request Register
    ir: u8,                          // IR lines (bitfield)
    read_select: ReadSelect,         // Select register to read.  True=ISR, False=IRR
    irq: u8,                         // IRQ Number
    intr: bool,                      // INT request line of PIC
    buffered: bool,                  // Buffered mode
    nested: bool,                    // Nested mode
    special_nested: bool,            // Special fully nested mode
    polled: bool,                    // Polled mode
    auto_eoi: bool,                  // Auto-EOI mode
    rotate_on_aeoi: bool,            // Should rotate in Auto-EOI mode
    trigger_mode: TriggerMode,
    expecting_icw2: bool,
    expecting_icw4: bool, // ICW3 not supported in Single mode operation
    error: bool,          // We encountered an invalid condition or request

    spurious_irqs: u64,
    interrupt_stats: Vec<InterruptStats>,
    intr_scheduled: bool,
    intr_timer: u32,
}

impl Default for Pic {
    fn default() -> Self {
        Self {
            init_state: InitializationState::Normal,
            int_offset: 0,
            imr: 0xFF, // All IRQs initially masked
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

            spurious_irqs: 0,
            interrupt_stats: vec![InterruptStats::new(); 8],
            intr_scheduled: false,
            intr_timer: 0,
        }
    }
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
    pub spurious_irqs: String,
    pub interrupt_stats: Vec<(String, String, String)>,
}

impl IoDevice for Pic {
    fn read_u8(&mut self, port: u16, _delta: DeviceRunTimeUnit) -> u8 {
        match port {
            PIC_COMMAND_PORT => self.handle_command_register_read(),
            PIC_DATA_PORT => self.handle_data_register_read(),
            _ => unreachable!("PIC: Bad port #"),
        }
    }
    fn write_u8(&mut self, port: u16, data: u8, _bus: Option<&mut BusInterface>, _delta: DeviceRunTimeUnit) {
        match port {
            PIC_COMMAND_PORT => {
                self.handle_command_register_write(data);
            }
            PIC_DATA_PORT => {
                self.handle_data_register_write(data);
            }
            _ => unreachable!("PIC: Bad port #"),
        }
    }

    fn port_list(&self) -> Vec<(String, u16)> {
        vec![
            (String::from("PIC Command Port"), PIC_COMMAND_PORT),
            (String::from("PIC Data Port"), PIC_DATA_PORT),
        ]
    }
}

impl Pic {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn reset(&mut self) {
        *self = Default::default();
    }

    pub fn handle_command_register_write(&mut self, byte: u8) {
        // Specific bit set indicates an Initialization Command Word 1 (ICW1) (actually a byte)
        if byte & ICW1_IS_ICW1 != 0 {
            // Parse Initialization Command Word
            if let InitializationState::Normal = self.init_state {
                // Reset the IMR & ISR on ICW
                self.isr = 0;
                self.imr = 0;

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
        else if byte & OCW_IS_OCW3 != 0 {
            self.read_select = match byte & OCW3_RR_COMMAND {
                0b10 => {
                    //log::debug!("PIC: OCW3 Read Selected IRR register");
                    ReadSelect::IRR
                }
                0b11 => {
                    //log::debug!("PIC: OCW3 Read Selected ISR register");
                    ReadSelect::ISR
                }
                _ => self.read_select,
            };
        }
        else {
            log::trace!("PIC: Unhandled command: {:02X}", byte)
        }
    }

    /// Perform an EOI (End of interrupt)
    /// An EOI resets a bit in the ISR.
    /// If an IR number is provided, it will perform a specific EOI and reset a specific bit.
    /// If None is provided, it will perform a non-specific EOI and reset the highest priority bit.
    pub fn eoi(&mut self, line: Option<u8>) {
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

    #[inline]
    pub fn clear_bit(byte: u8, bitn: u8) -> u8 {
        let mut mask: u8 = 0x01;
        mask <<= bitn;

        byte & !mask
    }

    #[inline]
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
                self.int_offset = byte & ICW2_MASK;
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
            ReadSelect::ISR => self.isr,
            ReadSelect::IRR => self.irr,
        }
    }

    pub fn handle_data_register_read(&mut self) -> u8 {
        self.imr
    }

    /// Set the value of the Interrupt Mask Register (IMR).
    /// Changing the IMR will allow devices with current high IR lines to generate interrupts.
    /// It will also lower INTR if the currently requesting lines are masked.
    fn set_imr(&mut self, byte: u8) {
        self.imr = byte;
        let (intr, irq) = self.calc_intr();

        if intr {
            self.schedule_intr(3); // TODO: Placeholder value. we should measure the actual delay with a scope.
            self.interrupt_stats[irq as usize].serviced_count += 1;
        }
        else if self.intr {
            // If INTR is high, and any triggering IRR bits are now masked, lower it.
            self.intr_scheduled = false;
            self.intr = false;
        }
    }

    /// Called by a device to request interrupt service.
    /// Simulates a low-to-high transition of the corresponding IR line.
    pub fn request_interrupt(&mut self, interrupt: u8) {
        if interrupt > 7 {
            panic!("PIC: Received interrupt out of range: {}", interrupt);
        }

        //log::trace!("PIC: Interrupt {} requested by device", interrupt);

        // Interrupts 0-7 map to bits 0-7 in IMR register
        let ir_bit: u8 = 0x01 << interrupt;
        // Set IR line high and set the request bit in the IRR register
        self.ir |= ir_bit;
        self.irr |= ir_bit;

        if self.imr & ir_bit != 0 {
            // If the corresponding bit is set in the IMR, it is masked: do not process right now
            self.interrupt_stats[interrupt as usize].imr_masked_count += 1;
        }
        else if self.isr & ir_bit != 0 {
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

    /// Called by a device that pulses the IR line to request service (like the keyboard)
    /// Simulates a low-to-high-to-low transition of the corresponding IR line.
    pub fn pulse_interrupt(&mut self, interrupt: u8) {
        if interrupt > 7 {
            panic!("PIC: Received interrupt out of range: {}", interrupt);
        }

        //log::trace!("PIC: Interrupt {} requested by device", interrupt);

        // Interrupts 0-7 map to bits 0-7 in IMR register
        let intr_bit: u8 = 0x01 << interrupt;

        // Set the request bit in the IRR register directly.
        // Since the IR line is 'pulsed' we clear it now. It is likely too short to register in any
        // debug display anyway (kb IR is ~100ns)
        self.ir &= !intr_bit;
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
            // Interrupt is not masked or already in service, elevate it...
            self.intr = true;
            self.interrupt_stats[interrupt as usize].serviced_count += 1;
        }

        // TODO: Schedule high to low transition of IR line after some time
    }

    /// Called by device to withdraw interrupt service request
    /// Simulates a high-to-low transition of the corresponding IR line.
    pub fn clear_interrupt(&mut self, interrupt: u8) {
        if interrupt > 7 {
            panic!("PIC: Received interrupt out of range: {}", interrupt);
        }

        // Clear the corresponding bit in the IR lines.
        let intr_bit: u8 = 0x01 << interrupt;
        self.ir &= !intr_bit;

        // We also clear the bit in the IRR register - it is not clear from the datasheet but bus sniffing
        // implies that a high to low transition in edge-triggered mode can de-assert INTR.
        self.irr &= !intr_bit;

        // Recalculate INTR in case lowering this IR line would withdraw the interrupt.
        self.intr = self.calc_intr().0;
    }

    pub fn query_interrupt_line(&self) -> bool {
        self.intr
    }

    /// Represents the PIC's response to the 2nd INTA pulse. The PIC will put the
    /// highest-priority interrupt vector onto the bus. If there is no pending IRR
    /// bit set, it will return the spurious interrupt #7.
    pub fn get_interrupt_vector(&mut self) -> Option<u8> {
        //log::trace!("Getting interrupt vector, auto-eoi: {:?}.", self.auto_eoi);
        if !self.intr {
            log::warn!("get_interrupt_vector() called when INTR is not asserted");
            return None;
        }

        // Return the highest priority vector.
        let mut ir_bit: u8 = 0x01;
        for irq in 0..8 {
            let have_request = self.irr & ir_bit != 0;
            let in_service = self.isr & ir_bit != 0;
            let is_masked = self.imr & ir_bit != 0;

            // TODO: Can an interrupt vector be delivered while in service? We assume not for now.
            if have_request && !in_service && !is_masked {
                // Found the highest priority IRR not in service

                // If in edge triggered mode, clear the bit in the IRR.
                // The IR line will need to make another low-to-high transition to re-assert the IRR bit.
                if let TriggerMode::Edge = self.trigger_mode {
                    self.irr &= !ir_bit;
                }
                // Set the bit in the ISR to mark as in service. (This technically occurs during the first INTA pulse.)
                self.isr |= ir_bit;
                // If Auto-EOI is enabled, the ISR bit is cleared during the second INTA pulse.
                if self.auto_eoi {
                    //log::trace!("Executing Auto-EOI");
                    self.isr &= !ir_bit;
                }
                self.irq = irq;

                // Finally, set INTR line low
                self.intr = false;

                return Some(irq | self.int_offset);
            }
            ir_bit <<= 1;
        }

        // If no bit in the IRR was found to be set, then a spurious interrupt occurs.
        // Note that in the event of a spurious interrupt, no bit in the ISR is set to indicate an interrupt is being
        // serviced. This provides a method of determining whether an IR7 is spurious or real.
        self.spurious_irqs += 1;
        Some(SPURIOUS_INTERRUPT)
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
            spurious_irqs: format!("{}", self.spurious_irqs),
            interrupt_stats: Vec::new(),
        };

        for i in 0..8 {
            state.interrupt_stats.push((
                format!("{}", self.interrupt_stats[i].imr_masked_count),
                format!("{}", self.interrupt_stats[i].isr_masked_count),
                format!("{}", self.interrupt_stats[i].serviced_count),
            ));
        }
        state
    }

    pub fn schedule_intr(&mut self, sys_ticks: u32) {
        self.intr_scheduled = true;
        self.intr_timer = sys_ticks;
    }

    /// Calculate the intended INTR line state based on the current state of the PIC.
    #[inline]
    pub fn calc_intr(&self) -> (bool, u8) {
        let mut ir_bit: u8 = 0x01;
        let irq = 0;
        for irq in 0..8 {
            let have_request = self.irr & ir_bit != 0;
            let is_not_masked = self.imr & ir_bit == 0;
            let is_not_in_service = self.isr & ir_bit == 0;

            if have_request && is_not_masked && is_not_in_service {
                return (true, irq);
            }

            ir_bit <<= 1;
        }
        (false, irq)
    }

    /// Run the PIC. This is primarily used to effect a delay in raising INTR when the IMR is changed.
    pub fn run(&mut self, sys_ticks: u32) {
        if self.intr_scheduled {
            self.intr_timer = self.intr_timer.saturating_sub(sys_ticks);
            if self.intr_timer == 0 {
                self.intr = true;
                self.intr_scheduled = false;
            }
        }

        // If INTR is low and not pending, check for unmasked bits in the IRR and raise it again if found.
        if !self.intr && !self.intr_scheduled {
            if self.calc_intr().0 {
                self.schedule_intr(100);
            }
        }
        else if self.intr {
            // If INTR is high check for unmasked bits in the IRR and lower it if none are found.
            self.intr = self.calc_intr().0;
        }
    }
}

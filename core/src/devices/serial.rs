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

    devices::serial.rs

    Implements the IBM Asynchronous Communications Adapter based on the
    INS8250 Serial Controller chip.

    Two adapters are emulated, a primary and secondary controller.

    Primary Documentation:
    IBM Publication 6361501
    "IBM Asynchronous Communications Adapter"
*/

use std::{collections::VecDeque, io::Read};

use crate::{
    bus::{BusInterface, DeviceRunTimeUnit, IoDevice},
    devices::pic,
};

/*  1.8Mhz Oscillator.
    Divided by 16, then again by programmable Divisor to select baud rate.
    The 8250 has a maximum baud of 9600.
    Interestingly, a minimum divisor of 1 provides a baud rate of 115200, which is a number some
    nerds might recognize.
*/
const SERIAL_CLOCK: f64 = 1.8432;

pub const SERIAL1_IRQ: u8 = 4;
pub const SERIAL2_IRQ: u8 = 3;

/* - Ports -

    Ports 0x3F8 & 0x3F9 (And their corresponding secondary ports) are multiplexed via
    use of the Divisor Latch Access Bit (DSLAB). If this bit is set in the Line Control Register,
    These two ports access the LSB and MSB of the clock Divisor instead of the RX/TX Buffer.
*/
pub const SERIAL1_RX_TX_BUFFER: u16 = 0x3F8;
//pub const SERIAL1_DIVISOR_LATCH_LSB: u16 = 0x3F8;
//pub const SERIAL1_DIVISOR_LATCH_MSB: u16 = 0x3F9;
pub const SERIAL1_INTERRUPT_ENABLE: u16 = 0x3F9;
pub const SERIAL1_INTERRUPT_ID: u16 = 0x3FA;
pub const SERIAL1_LINE_CONTROL: u16 = 0x3FB;
pub const SERIAL1_MODEM_CONTROL: u16 = 0x3FC;
pub const SERIAL1_LINE_STATUS: u16 = 0x3FD;
pub const SERIAL1_MODEM_STATUS: u16 = 0x3FE;

pub const SERIAL2_RX_TX_BUFFER: u16 = 0x2F8;
//pub const SERIAL2_DIVISOR_LATCH_LSB: u16 = 0x2F8;
//pub const SERIAL2_DIVISOR_LATCH_MSB: u16 = 0x2F9;
pub const SERIAL2_INTERRUPT_ENABLE: u16 = 0x2F9;
pub const SERIAL2_INTERRUPT_ID: u16 = 0x2FA;
pub const SERIAL2_LINE_CONTROL: u16 = 0x2FB;
pub const SERIAL2_MODEM_CONTROL: u16 = 0x2FC;
pub const SERIAL2_LINE_STATUS: u16 = 0x2FD;
pub const SERIAL2_MODEM_STATUS: u16 = 0x2FE;

// Line Control Register constants
const WORD_LENGTH_SELECT_MASK: u8 = 0b0000_0011;
const STOP_BIT_SELECT_BIT: u8 = 0b0000_0100;
const PARITY_ENABLE_BIT: u8 = 0b0000_1000;
const DIVISOR_LATCH_ACCESS_BIT: u8 = 0b1000_0000;

// Line Status Register constants
const STATUS_DATA_READY: u8 = 0b0000_0001;
//const STATUS_OVERRUN_ERROR: u8 = 0b0000_0010;
//const STATUS_PARITY_ERROR: u8 = 0b0000_0100;
//const STATUS_FRAMING_ERROR: u8 = 0b0000_1000;
//const STATUS_BREAK_INTERRUPT: u8 = 0b0001_0000;
const STATUS_TRANSMIT_EMPTY: u8 = 0b0010_0000;
//const STATUS_TX_SHIFT_EMPTY: u8 = 0b0100_0000;

const INTERRUPT_ID_MASK: u8 = 0b0000_0011;

const INTERRUPT_DATA_AVAIL: u8 = 0b0000_0001;
const INTERRUPT_TX_EMPTY: u8 = 0b0000_0010;
const INTERRUPT_RX_LINE_STATUS: u8 = 0b0000_0100;
const INTERRUPT_MODEM_STATUS: u8 = 0b0000_1000;

//const INTERRUPT_PRIORITY_0: u8 = 0b0000_0001;
//const INTERRUPT_PRIORITY_1: u8 = 0b0000_0010;
//const INTERRUPT_PRIORITY_2: u8 = 0b0000_0100;
//const INTERRUPT_PRIORITY_3: u8 = 0b0000_1000;
//const INTERRUPT_PRIORITY_MASK: u8 = 0b0000_1111;

// Modem Control Register bits
const MODEM_CONTROL_DTR: u8 = 0b0000_0001;
const MODEM_CONTROL_RTS: u8 = 0b0000_0010;
const MODEM_CONTROL_OUT1: u8 = 0b0000_0100;
const MODEM_CONTROL_OUT2: u8 = 0b0000_1000;
const MODEM_CONTROL_LOOP: u8 = 0b0001_0000;

const MODEM_STATUS_DCTS: u8 = 0b0000_0001;
const MODEM_STATUS_DDSR: u8 = 0b0000_0010;
//const MODEM_STATUS_TERI: u8 = 0b0000_0100;
//const MODEM_STATUS_DRLSD: u8 = 0b0000_1000;
const MODEM_STATUS_CTS: u8 = 0b0001_0000;
const MODEM_STATUS_DSR: u8 = 0b0010_0000;
const MODEM_STATUS_RI: u8 = 0b0100_0000;
const MODEM_STATUS_RLSD: u8 = 0b1000_0000;

impl IoDevice for SerialPortController {
    fn read_u8(&mut self, port: u16, _delta: DeviceRunTimeUnit) -> u8 {
        match port {
            SERIAL1_RX_TX_BUFFER => self.port[0].rx_buffer_read(),
            SERIAL2_RX_TX_BUFFER => self.port[1].rx_buffer_read(),
            SERIAL1_INTERRUPT_ENABLE => self.port[0].interrupt_enable_read(),
            SERIAL2_INTERRUPT_ENABLE => self.port[1].interrupt_enable_read(),
            SERIAL1_INTERRUPT_ID => self.port[0].interrupt_id_read(),
            SERIAL2_INTERRUPT_ID => self.port[1].interrupt_id_read(),
            SERIAL1_LINE_CONTROL => self.port[0].line_control_read(),
            SERIAL2_LINE_CONTROL => self.port[1].line_control_read(),
            SERIAL1_MODEM_CONTROL => 0,
            SERIAL2_MODEM_CONTROL => 0,
            SERIAL1_LINE_STATUS => self.port[0].line_status_read(),
            SERIAL2_LINE_STATUS => self.port[1].line_status_read(),
            SERIAL1_MODEM_STATUS => self.port[0].modem_status_read(),
            SERIAL2_MODEM_STATUS => self.port[1].modem_status_read(),
            _ => 0,
        }
    }

    fn write_u8(&mut self, port: u16, byte: u8, _bus: Option<&mut BusInterface>, _delta: DeviceRunTimeUnit) {
        match port {
            SERIAL1_RX_TX_BUFFER => self.port[0].tx_buffer_write(byte),
            SERIAL2_RX_TX_BUFFER => self.port[1].tx_buffer_write(byte),
            SERIAL1_INTERRUPT_ENABLE => self.port[0].interrupt_enable_write(byte),
            SERIAL2_INTERRUPT_ENABLE => self.port[1].interrupt_enable_write(byte),
            SERIAL1_INTERRUPT_ID => {}
            SERIAL2_INTERRUPT_ID => {}
            SERIAL1_LINE_CONTROL => self.port[0].line_control_write(byte),
            SERIAL2_LINE_CONTROL => self.port[1].line_control_write(byte),
            SERIAL1_MODEM_CONTROL => self.port[0].modem_control_write(byte),
            SERIAL2_MODEM_CONTROL => self.port[1].modem_control_write(byte),
            SERIAL1_LINE_STATUS => {}
            SERIAL2_LINE_STATUS => {}
            SERIAL1_MODEM_STATUS => {}
            SERIAL2_MODEM_STATUS => {}
            _ => {}
        }
    }

    fn port_list(&self) -> Vec<u16> {
        vec![
            SERIAL1_RX_TX_BUFFER,
            SERIAL1_INTERRUPT_ENABLE,
            SERIAL1_INTERRUPT_ID,
            SERIAL1_LINE_CONTROL,
            SERIAL1_MODEM_CONTROL,
            SERIAL1_LINE_STATUS,
            SERIAL1_MODEM_STATUS,
            SERIAL2_RX_TX_BUFFER,
            SERIAL2_INTERRUPT_ENABLE,
            SERIAL2_INTERRUPT_ID,
            SERIAL2_LINE_CONTROL,
            SERIAL2_MODEM_CONTROL,
            SERIAL2_LINE_STATUS,
            SERIAL2_MODEM_STATUS,
        ]
    }
}

#[derive(Debug)]
pub enum StopBits {
    One,
    OneAndAHalf,
    Two,
}

#[derive(Debug)]
pub enum IntrAction {
    None,
    Raise,
    Lower,
}

pub struct SerialPortDebuggerState {
    name: String,
    irq: u8,
    line_control_reg: u8,
    line_status_reg: u8,
    ii_reg: u8,
    interrupts_active: u8,
    interrupt_enable_reg: u8,
    modem_control_reg: u8,
    modem_status_reg: u8,
    rx_byte: u8,
    tx_byte: u8,
}

pub struct SerialPort {
    name: String,
    irq: u8,
    line_control_reg: u8,
    word_length: u8,
    stop_bits: StopBits,
    parity_enable: bool,
    divisor_latch_access: bool,
    divisor: u16,
    line_status_reg: u8,
    interrupts_active: u8,
    interrupt_enable_reg: u8,
    intr_action: IntrAction,
    modem_control_reg: u8,
    loopback: bool,
    modem_status_reg: u8,
    rx_byte: u8,
    rx_count: usize,
    rx_was_read: bool,
    tx_holding_reg: u8,
    tx_holding_empty: bool,
    rx_queue: VecDeque<u8>,
    rx_timer: f64,
    tx_count: usize,
    tx_queue: VecDeque<u8>,
    tx_timer: f64,
    us_per_byte: f64,

    // Serial port bridge
    bridge_port: Option<Box<dyn serialport::SerialPort>>,
    bridge_buf:  Vec<u8>,
}

impl Default for SerialPort {
    fn default() -> Self {
        Self {
            name: String::new(),
            irq: 4,
            line_control_reg: 0,
            word_length: 8,
            stop_bits: StopBits::One,
            parity_enable: false,
            divisor_latch_access: false,
            divisor: 12, // 9600 baud
            line_status_reg: crate::devices::serial::STATUS_TRANSMIT_EMPTY,
            interrupts_active: 0,
            interrupt_enable_reg: 0,
            intr_action: IntrAction::None,
            modem_control_reg: 0,
            loopback: false,
            modem_status_reg: 0,
            rx_byte: 0,
            rx_count: 0,
            rx_was_read: false,
            tx_holding_reg: 0,
            tx_holding_empty: true,
            rx_queue: VecDeque::new(),
            rx_timer: 0.0,
            tx_count: 0,
            tx_queue: VecDeque::new(),
            tx_timer: 0.0,
            us_per_byte: 833.333, // 9600 baud

            bridge_port: None,
            bridge_buf:  vec![0; 1000],
        }
    }
}

impl SerialPort {
    pub fn new(name: String, irq: u8) -> Self {
        Self {
            name,
            irq,
            ..Default::default()
        }
    }

    pub fn reset(&mut self) {
        *self = Self {
            name: self.name.clone(),
            irq: self.irq,
            ..Default::default()
        }
    }

    /// Convert the integer divisor value into baud rate
    fn divisor_to_baud(divisor: u16) -> u16 {
        return ((SERIAL_CLOCK * 1_000_000.0) / divisor as f64 / 16.0) as u16;
    }

    /// Sets the value of us_per_byte, the microsecond delay between sending a byte out of the
    /// Send or receive queue based on the current baud rate.
    /// This function should be called whenever the divisor has changed.
    fn set_timing(&mut self) {
        if self.divisor < 12 {
            // Minimum divisor of 12 (9600 baud)
            self.divisor = 12;
        }
        let bytes_per_second = SerialPort::divisor_to_baud(self.divisor) / self.word_length as u16;
        self.us_per_byte = 1.0 / bytes_per_second as f64 * 1_000_000.0;
    }

    fn line_control_read(&self) -> u8 {
        self.line_control_reg
    }

    fn line_control_write(&mut self, byte: u8) {
        self.line_control_reg = byte;

        let stop_bit_select = byte & STOP_BIT_SELECT_BIT != 0;

        (self.word_length, self.stop_bits) = match (byte & WORD_LENGTH_SELECT_MASK, stop_bit_select) {
            (0b00, false) => (5, StopBits::One),
            (0b00, true) => (5, StopBits::OneAndAHalf),
            (0b01, false) => (6, StopBits::One),
            (0b01, true) => (6, StopBits::Two),
            (0b10, false) => (7, StopBits::One),
            (0b10, true) => (7, StopBits::Two),
            (0b11, false) => (8, StopBits::One),
            (0b11, true) => (8, StopBits::Two),
            _ => {
                unreachable!("invalid")
            }
        };

        self.parity_enable = byte & PARITY_ENABLE_BIT != 0;
        self.divisor_latch_access = byte & DIVISOR_LATCH_ACCESS_BIT != 0;

        log::trace!(
            "{}: Write to Line Control Register: {:02X} Word Length: {} Parity: {} Stop Bits: {:?}",
            self.name,
            byte,
            self.word_length,
            self.parity_enable,
            self.stop_bits
        );
    }

    /// Handle a read of the RX buffer register
    /// or if DSLAB is active, read the Divisor Latch LSB
    fn rx_buffer_read(&mut self) -> u8 {
        // If DSLAB, send Divisor Latch LSB
        if self.divisor_latch_access {
            return (self.divisor & 0xFF) as u8;
        }
        else {
            // Read the byte in the RX buffer
            if !self.rx_was_read {
                //log::trace!("{}: Rx buffer read: {:02X}", self.name, self.rx_byte );
            }
            let byte = self.rx_byte;
            self.rx_was_read = true;
            self.rx_byte = 0;
            // Clear DR bit in Line Status Register
            self.line_status_reg &= !STATUS_DATA_READY;
            // Clear any pending Data Available interrupt.
            self.lower_interrupt_type(INTERRUPT_DATA_AVAIL);

            byte
        }
    }

    /// Send a byte to the serial port tx buffer register.
    /// For COM1, COM1 is always attached to Mouse which ignores input.
    /// COM2 may be bridged to a host serial port.
    fn tx_buffer_write(&mut self, byte: u8) {
        // If DSLAB, set Divisor Latch LSB
        if self.divisor_latch_access {
            self.divisor &= 0xFF00;
            self.divisor |= byte as u16;
            self.set_timing();
            log::trace!(
                "{}: Divisor LSB set. Divisor: {} Baud: {}",
                self.name,
                self.divisor,
                SerialPort::divisor_to_baud(self.divisor)
            );
        }
        else {
            log::trace!("{}: Tx buffer write: {:02X}", self.name, byte);
            self.tx_holding_reg = byte;
            self.tx_holding_empty = false;
            self.line_status_reg &= !STATUS_TRANSMIT_EMPTY;
        }
    }

    /// Handle reading the interrupt enable register,
    /// or if DSLAB is active, handle a read of the Divisor Latch MSB
    fn interrupt_enable_read(&self) -> u8 {
        // If DSLAB, send Divisor Latch MSB
        if self.divisor_latch_access {
            return (self.divisor >> 8 & 0xFF) as u8;
        }
        self.interrupt_enable_reg
    }

    /// Handle a write to the interrupt enable register,
    /// or if DSLAB is active, handle a write to the Divisor Latch MSB
    fn interrupt_enable_write(&mut self, byte: u8) {
        // If DSLAB, set Divisor Latch MSB
        if self.divisor_latch_access {
            self.divisor &= 0x00FF;
            self.divisor |= (byte as u16) << 8;
            self.set_timing();
            log::trace!(
                "{}: Divisor MSB set. Divisor: {} Baud: {}",
                self.name,
                self.divisor,
                SerialPort::divisor_to_baud(self.divisor)
            );
        }
        else {
            log::trace!("{}: Write to Interrupt Enable Register: {:04b}", self.name, byte & 0x0F);

            self.set_interrupt_enable_mask(byte & 0x0F);
        }
    }

    fn set_interrupt_enable_mask(&mut self, mask: u8) {
        let old_enable_reg = self.interrupt_enable_reg;
        self.interrupt_enable_reg = mask & 0x0F;

        // COMTEST from ctmouse suite seems to indicate that a TX Holding Register Empty interrupt
        // will be triggered immediately after it is enabled.
        if mask & INTERRUPT_TX_EMPTY != 0 && (old_enable_reg & INTERRUPT_TX_EMPTY == 0) && self.tx_holding_empty {
            self.raise_interrupt_type(INTERRUPT_TX_EMPTY);
        }

        if mask & INTERRUPT_RX_LINE_STATUS == 0 {
            self.lower_interrupt_type(INTERRUPT_RX_LINE_STATUS);
        }
        if mask & INTERRUPT_DATA_AVAIL == 0 {
            self.lower_interrupt_type(INTERRUPT_DATA_AVAIL);
        }
        if mask & INTERRUPT_TX_EMPTY == 0 {
            self.lower_interrupt_type(INTERRUPT_TX_EMPTY);
        }
        if mask & INTERRUPT_MODEM_STATUS == 0 {
            self.lower_interrupt_type(INTERRUPT_MODEM_STATUS);
        }
    }

    // Handle reading the Line Status Register
    fn line_status_read(&self) -> u8 {
        self.line_status_reg
    }

    /// Handle a read of the Interrupt ID Register.
    ///
    /// The Interrupt ID Register returns a value representing the highest priority interrupt
    /// currently active.
    fn interrupt_id_read(&mut self) -> u8 {
        let mut byte = self.calc_irr();

        if self.interrupts_active & INTERRUPT_TX_EMPTY != 0 {
            // IBM Docs state that reading the IRR clears this interrupt
            self.lower_interrupt_type(INTERRUPT_TX_EMPTY);
        }

        log::debug!("{}: Read Interrupt ID Register: {:04b}", self.name, byte);
        byte
    }

    fn calc_irr(&self) -> u8 {
        let mut byte = 0;

        // Set bit 0 to 1 if interrupt is NOT pending
        if self.interrupts_active & INTERRUPT_ID_MASK == 0 {
            byte |= 1;
        }

        // Set interrupt ID bits (Bits 1 & 2)
        // Convert the highest priority interrupt into an 2 bit field 3-0
        // Note: Priority does not match the order of bits in the Interrupt Enable register.
        // 0b11 -> Receiver Line Status
        // 0b10 -> Received Data Available
        // 0b01 -> Transmitter Holding Register Empty
        // 0b00 -> Modem Status
        if self.interrupts_active & INTERRUPT_RX_LINE_STATUS != 0 {
            byte |= 3 << 1;
        }
        else if self.interrupts_active & INTERRUPT_DATA_AVAIL != 0 {
            byte |= 2 << 1;
        }
        else if self.interrupts_active & INTERRUPT_TX_EMPTY != 0 {
            byte |= 1 << 1;
        }
        else {
            // Modem status interrupt == 0
        }
        byte
    }

    /// Handle writing to the Modem Control Register
    fn modem_control_write(&mut self, byte: u8) {
        log::trace!("{}: Write to Modem Control Register: {:05b}", self.name, byte & 0x1F);
        self.modem_control_reg = byte & 0x1F;

        self.loopback = self.modem_control_reg & MODEM_CONTROL_LOOP != 0;
        if self.loopback {
            log::trace!("{}: Loopback mode enabled", self.name);
        }
    }

    /// Handle reading from the Modem Status register
    fn modem_status_read(&mut self) -> u8 {
        if self.loopback {
            // In loopback mode, the four HO bits in the Modem status register reflect
            // the four LO bits in the Modem Control register as follows:
            let mut byte = self.modem_status_reg & 0x0F;

            if self.modem_control_reg & MODEM_CONTROL_RTS != 0 {
                byte |= MODEM_STATUS_CTS;
            }
            if self.modem_control_reg & MODEM_CONTROL_DTR != 0 {
                byte |= MODEM_STATUS_DSR;
            }
            if self.modem_control_reg & MODEM_CONTROL_OUT1 != 0 {
                byte |= MODEM_STATUS_RI;
            }
            if self.modem_control_reg & MODEM_CONTROL_OUT2 != 0 {
                byte |= MODEM_STATUS_RLSD;
            }
            byte
        }
        else {
            let byte = self.modem_status_reg;

            // Clear DCTS and DDSR flags
            self.modem_status_reg &= !MODEM_STATUS_DCTS;
            self.modem_status_reg &= !MODEM_STATUS_DDSR;

            byte
        }
    }

    fn set_modem_status_connected(&mut self) {
        if self.modem_status_reg & MODEM_STATUS_CTS == 0 {
            self.modem_status_reg |= MODEM_STATUS_CTS;
            self.modem_status_reg |= MODEM_STATUS_DCTS;
        }

        if self.modem_status_reg & MODEM_STATUS_DSR == 0 {
            self.modem_status_reg |= MODEM_STATUS_DSR;
            self.modem_status_reg |= MODEM_STATUS_DDSR;
        }
    }

    fn raise_interrupt_type(&mut self, interrupt_flag: u8) {
        // Interrupt enable register completely disables interrupts
        if interrupt_flag & self.interrupt_enable_reg != 0 {
            self.interrupts_active |= interrupt_flag;

            // IBM: To allow the communications adapter to send interrupts to the system,
            // bit 3 of the modem control resister must be set to 1
            if self.modem_control_reg & MODEM_CONTROL_OUT2 != 0 {
                //log::trace!("Sending interrupt. Interrupts active: {:04b}", self.interrupts_active);
                self.intr_action = IntrAction::Raise;
            }
        }
    }

    fn lower_interrupt_type(&mut self, interrupt_flag: u8) {
        // Clear bit from active interrupts
        self.interrupts_active &= !interrupt_flag;

        // Any remaining interrupts active? Deassert IRQ if no.
        if self.interrupts_active == 0 {
            self.intr_action = IntrAction::Lower;
        }
    }

    fn bridge_port(&mut self, port_name: String) -> anyhow::Result<bool> {
        let port_result = serialport::new(port_name.clone(), 9600)
            .timeout(std::time::Duration::from_millis(5))
            .stop_bits(serialport::StopBits::One)
            .parity(serialport::Parity::None)
            .open();

        match port_result {
            Ok(bridge_port) => {
                log::trace!("Successfully opened host port {}", port_name);
                self.bridge_port = Some(bridge_port);
                self.set_modem_status_connected();
                Ok(true)
            }
            Err(e) => {
                log::trace!("Error opening host port: {}", e);
                anyhow::bail!("Error opening host port: {}", e)
            }
        }
    }
}

pub struct SerialPortController {
    port: [SerialPort; 2],
}

impl SerialPortController {
    pub fn new() -> Self {
        Self {
            port: [
                SerialPort::new("COM1".to_string(), SERIAL1_IRQ),
                SerialPort::new("COM2".to_string(), SERIAL2_IRQ),
            ],
        }
    }

    pub fn get_debug_state(&self) -> Vec<SerialPortDebuggerState> {
        let mut state = Vec::new();

        for port in &self.port {
            state.push(SerialPortDebuggerState {
                name: port.name.clone(),
                irq: port.irq,
                line_control_reg: port.line_control_reg,
                line_status_reg: port.line_status_reg,
                ii_reg: port.calc_irr(),
                interrupt_enable_reg: port.interrupt_enable_reg,
                interrupts_active: port.interrupts_active,
                modem_control_reg: port.modem_control_reg,
                modem_status_reg: port.modem_status_reg,
                rx_byte: port.rx_byte,
                tx_byte: port.tx_holding_reg,
            });
        }

        state
    }

    /// Get status of specified serial port's RTS line
    pub fn get_rts(&self, port: usize) -> bool {
        self.port[port].modem_control_reg & MODEM_CONTROL_RTS != 0
    }

    /// Get status of the specified serial port's DTR line
    #[allow(dead_code)]
    pub fn get_dtr(&self, port: usize) -> bool {
        self.port[port].modem_control_reg & MODEM_CONTROL_DTR != 0
    }

    /// Queue a byte for delivery to the specified serial port's RX buffer
    pub fn queue_byte(&mut self, port: usize, byte: u8) {
        self.port[port].rx_queue.push_back(byte);
    }

    /// Bridge the specified serial port
    pub fn bridge_port(&mut self, port: usize, port_name: String) -> anyhow::Result<bool> {
        self.port[port].bridge_port(port_name)
    }

    /// Run the serial ports for the specified number of microseconds
    pub fn run(&mut self, pic: &mut pic::Pic, us: f64) {
        for port in self.port.iter_mut() {
            // Handle pending interrupt action
            match port.intr_action {
                IntrAction::Raise => {
                    //log::trace!("asserting irq: {}", port.irq);
                    pic.request_interrupt(port.irq);
                }
                IntrAction::Lower => {
                    //log::trace!("deasserting irq: {}", port.irq);
                    pic.clear_interrupt(port.irq);
                }
                IntrAction::None => {}
            }
            port.intr_action = IntrAction::None;

            // Receive bytes from queue
            port.rx_timer += us;
            while port.rx_timer > port.us_per_byte {
                // Time to receive a byte at current baud rate
                if let Some(b) = port.rx_queue.pop_front() {
                    // We have a byte to receive

                    if !port.rx_was_read {
                        // Previous byte was never read :(
                        // TODO: Handle overflow interrupt
                    }

                    port.rx_byte = b;
                    port.rx_was_read = false;
                    // Set Data Available bit in LSR
                    port.line_status_reg |= STATUS_DATA_READY;

                    // Raise Data Available interrupt if not masked
                    port.raise_interrupt_type(INTERRUPT_DATA_AVAIL);

                    if port.name.eq("COM2") {
                        log::trace!("{}: Received byte: {:02X}", port.name, b);
                    }
                    //log::trace!("{}: Received byte: {:02X}", port.name, b );
                }

                port.rx_timer -= port.us_per_byte;
            }

            // Transmit byte timer
            port.tx_timer += us;
            while port.tx_timer > port.us_per_byte {
                // Is there a byte waiting to be sent in the tx holding register?
                if !port.tx_holding_empty {
                    // If we have bridged this serial port, send the byte to the tx queue
                    if let Some(_) = &port.bridge_port {
                        //log::trace!("{}: Sending byte: {:02X}", port.name, port.tx_holding_reg);
                        port.tx_queue.push_back(port.tx_holding_reg);
                    }

                    port.tx_holding_reg = 0;
                    port.tx_holding_empty = true;
                    port.line_status_reg |= STATUS_TRANSMIT_EMPTY;

                    port.raise_interrupt_type(INTERRUPT_TX_EMPTY);
                }

                port.tx_timer -= port.us_per_byte;
            }
        }
    }

    /// The update function is called per-frame, instead of within the emulation loop.
    /// This allows bridging realtime events with virtual device.
    pub fn update(&mut self) {
        for port in &mut self.port {
            match &mut port.bridge_port {
                Some(bridge_port) => {
                    // Write any pending bytes
                    if port.tx_queue.len() > 0 {
                        port.tx_queue.make_contiguous();
                        let (tx1, _) = port.tx_queue.as_slices();

                        match bridge_port.write(tx1) {
                            Ok(_) => {
                                //log::trace!("Wrote bytes: {:?}", tx1);
                            }
                            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => (),
                            Err(e) => log::error!("Error writing byte: {:?}", e),
                        }

                        port.tx_queue.clear();
                    }

                    // Read any pending bytes
                    match bridge_port.read(port.bridge_buf.as_mut_slice()) {
                        Ok(ct) => {
                            if ct > 0 {
                                log::trace!("Read {} bytes from serial port", ct);
                            }
                            for i in 0..ct {
                                // TODO: Must be a more efficient way to copy the vec to vecdeque?
                                let byte = port.bridge_buf[i];
                                port.rx_queue.push_back(byte);
                                //log::trace!("Wrote byte : {:02X} to buf", byte);
                            }
                        }
                        Err(_) => {
                            //log::error!("Error reading serial device: {}", e);
                        }
                    }
                }
                None => {}
            }
        }
    }
}

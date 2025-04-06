/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2025 Daniel Balsom

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

    devices::lpt_port.rs

    Implementation of a basic Centronics printer port. This is a component
    implementation, and must be embedded into a card implementation that can
    decode the proper port address.

*/

use crate::{channel::BidirectionalChannel, devices::lpt_card::LPT_DEFAULT_IRQ, tracelogger::TraceLogger};
use modular_bitfield::{bitfield, prelude::*};

#[derive(Clone)]
pub enum ParallelMessage {
    Data(u8),
    Status(ParallelStatus),
    Control(ParallelControl),
}

pub const POLL_TIME: f64 = 50.0; // 100 microseconds

#[bitfield]
#[derive(Copy, Clone, Default)]
pub struct ParallelStatus {
    #[skip]
    pub unused: B3,
    pub error: bool,
    pub select: bool,
    pub paper_out: bool,
    pub ack: bool,
    pub busy: bool,
}

#[bitfield]
#[derive(Copy, Clone, Default)]
pub struct ParallelControl {
    pub strobe: bool,
    pub auto_line_feed: bool,
    pub initialize: bool,
    pub select_in: bool,
    pub enable_irq: bool,
    #[skip]
    pub unused2: B3,
}

#[allow(dead_code)]
pub struct ParallelPort {
    update_accum: f64,
    data: u8,
    status: ParallelStatus,
    control: ParallelControl,
    irq: u8,
    send_interrupt: bool,
    device_channel: BidirectionalChannel<ParallelMessage>,
    port_channel: BidirectionalChannel<ParallelMessage>,
    trace_logger: TraceLogger,
}

impl Default for ParallelPort {
    fn default() -> Self {
        let (device_channel, port_channel) = BidirectionalChannel::new_pair();

        Self {
            update_accum: 0.0,
            data: 0,
            status: ParallelStatus::from_bytes([0]),
            control: ParallelControl::from_bytes([0]),
            irq: LPT_DEFAULT_IRQ,
            send_interrupt: false,
            device_channel,
            port_channel,
            trace_logger: TraceLogger::None,
        }
    }
}

impl ParallelPort {
    pub fn new(irq: Option<u16>, trace_logger: TraceLogger) -> Self {
        Self {
            irq: irq.unwrap_or(LPT_DEFAULT_IRQ as u16) as u8,
            trace_logger,
            ..Default::default()
        }
    }

    pub fn intr_enabled(&self) -> bool {
        self.control.enable_irq()
    }

    pub fn device_channel(&self) -> BidirectionalChannel<ParallelMessage> {
        self.device_channel.clone()
    }

    pub fn port_write(&mut self, port: u16, data: u8) {
        match port & 0x03 {
            0 => {
                self.data_register_write(data);
            }
            1 => {
                self.status_register_write(data);
            }
            2 => {
                self.control_register_write(data);
            }
            _ => {}
        }
    }

    pub fn port_read(&mut self, port: u16) -> u8 {
        match port & 0x03 {
            0 => {
                // CRTC address register is not readable
                self.data_register_read()
            }
            1 => {
                // CRTC data register is partially readable (depends on register selected)
                self.status_register_read()
            }
            2 => self.control_register_read(),
            _ => 0xFF,
        }
    }

    pub fn data_register_read(&mut self) -> u8 {
        self.trace_logger
            .print(format!("LPT: Data register read: {:#02X}", self.data));
        self.data
    }

    pub fn data_register_write(&mut self, data: u8) {
        self.data = data;
        self.trace_logger
            .print(format!("LPT: Data register write: {:#02X}", data));

        let _ = self.port_channel.send(ParallelMessage::Data(data));
    }

    pub fn status_register_read(&mut self) -> u8 {
        let byte = self.status.into_bytes()[0];
        log::trace!("LPT: Status register read: {:02X}", byte);
        self.trace_logger
            .print(format!("LPT: Status register read: {:02X}", byte));
        byte
    }

    pub fn status_register_write(&mut self, data: u8) {
        self.status = ParallelStatus::from_bytes([data]);
        self.trace_logger
            .print(format!("LPT: Status register write: {:#02X}", data));

        let _ = self.port_channel.send(ParallelMessage::Status(self.status));
    }

    pub fn control_register_read(&mut self) -> u8 {
        let byte = self.control.into_bytes()[0];
        self.trace_logger
            .print(format!("LPT: Control register read: {:#02X}", byte));
        byte
    }

    pub fn control_register_write(&mut self, data: u8) {
        self.control = ParallelControl::from_bytes([data]);

        log::trace!(
            "LPT: Control register write: {:02X} interrupts enabled: {}",
            data,
            self.control.enable_irq()
        );

        self.trace_logger.print(format!(
            "LPT: Control register write: {:02X} interrupts enabled: {}",
            data,
            self.control.enable_irq()
        ));

        let _ = self.port_channel.send(ParallelMessage::Control(self.control));
    }

    pub fn run(&mut self, usec: f64) -> bool {
        self.update_accum += usec;

        if self.update_accum >= POLL_TIME {
            self.update_accum -= POLL_TIME;

            // Poll the device for any updates
            while let Ok(msg) = self.port_channel.try_recv() {
                match msg {
                    ParallelMessage::Data(_data) => {
                        log::trace!("LPT: Data from device unimplemented.");
                    }
                    ParallelMessage::Status(status) => {
                        let new_status = status;
                        log::trace!(
                            "LPT: Status from device: {:#02X} ack {}",
                            new_status.into_bytes()[0],
                            status.ack()
                        );

                        if !self.status.ack() && new_status.ack() {
                            // Acknowledge the interrupt
                            log::trace!("LPT: Acknowledging interrupt.");
                            self.send_interrupt = true;
                        }
                        self.status = new_status;
                    }
                    ParallelMessage::Control(_control) => {
                        log::trace!("LPT: Control from device unimplemented.");
                    }
                }
            }
        }

        let irq = self.send_interrupt;
        self.send_interrupt = false;
        irq
    }
}

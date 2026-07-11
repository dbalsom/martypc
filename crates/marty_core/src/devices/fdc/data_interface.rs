/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the "Software"),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.
yyyyy
    --------------------------------------------------------------------------
*/

//! Byte transport interface for FDC execution phase transfers.
//!
//! The controller operation produces or consumes bytes at the disk data rate. This
//! interface owns the single-byte 765 latch and hides whether the byte is serviced
//! by DMA or by CPU port I/O.

use crate::{
    bus::BusInterface,
    devices::{dma, fdc::controller::FDC_DMA},
};

pub const FDC_MFM_BYTE_PERIOD_US: f64 = 13.0;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FdcDataMode {
    Dma,
    Pio,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FdcDataDirection {
    ToHost,
    FromHost,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FdcDataError {
    Inactive,
    WrongDirection,
    Timeout,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct FdcDataStatus {
    pub busy: bool,
    pub mrq: bool,
    pub dio_to_cpu: bool,
}

#[derive(Clone, Debug)]
pub struct FdcDataInterface {
    active: bool,
    mode: FdcDataMode,
    direction: FdcDataDirection,
    latch: Option<u8>,
    terminal_count: bool,
    byte_count: usize,
}

impl Default for FdcDataInterface {
    fn default() -> Self {
        Self {
            active: false,
            mode: FdcDataMode::Pio,
            direction: FdcDataDirection::ToHost,
            latch: None,
            terminal_count: false,
            byte_count: 0,
        }
    }
}

impl FdcDataInterface {
    pub fn begin(&mut self, mode: FdcDataMode, direction: FdcDataDirection) {
        self.active = true;
        self.mode = mode;
        self.direction = direction;
        self.latch = None;
        self.terminal_count = false;
        self.byte_count = 0;
    }

    pub fn end(&mut self) {
        self.active = false;
        self.latch = None;
        self.terminal_count = false;
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn mode(&self) -> FdcDataMode {
        self.mode
    }

    pub fn direction(&self) -> FdcDataDirection {
        self.direction
    }

    pub fn byte_period_us(&self) -> f64 {
        FDC_MFM_BYTE_PERIOD_US * 3.0
    }

    pub fn byte_count(&self) -> usize {
        self.byte_count
    }

    pub fn terminal_count(&self) -> bool {
        self.terminal_count
    }

    pub fn latch_empty(&self) -> bool {
        self.latch.is_none()
    }

    pub fn clear_terminal_count(&mut self) {
        self.terminal_count = false;
    }

    pub fn send(&mut self, byte: u8, dma: &mut dma::DMAController, bus: &mut BusInterface) -> Result<(), FdcDataError> {
        if !self.active {
            return Err(FdcDataError::Inactive);
        }
        if self.direction != FdcDataDirection::ToHost {
            return Err(FdcDataError::WrongDirection);
        }

        if self.mode == FdcDataMode::Dma {
            self.service_dma(dma, bus);
        }

        if self.latch.is_some() {
            return Err(FdcDataError::Timeout);
        }

        self.latch = Some(byte);
        if self.mode == FdcDataMode::Dma {
            self.service_dma(dma, bus);
        }
        Ok(())
    }

    pub fn recv(&mut self, dma: &mut dma::DMAController, bus: &mut BusInterface) -> Result<u8, FdcDataError> {
        if !self.active {
            return Err(FdcDataError::Inactive);
        }
        if self.direction != FdcDataDirection::FromHost {
            return Err(FdcDataError::WrongDirection);
        }

        if self.mode == FdcDataMode::Dma && self.latch.is_none() {
            self.service_dma(dma, bus);
        }

        self.latch.take().ok_or(FdcDataError::Timeout)
    }

    pub fn run(&mut self, _us: f64, dma: &mut dma::DMAController, bus: &mut BusInterface) {
        if !self.active || self.mode != FdcDataMode::Dma {
            return;
        }

        self.service_dma(dma, bus);
    }

    fn service_dma(&mut self, dma: &mut dma::DMAController, bus: &mut BusInterface) {
        match self.direction {
            FdcDataDirection::ToHost => {
                if let Some(byte) = self.latch {
                    if dma.check_dma_ready(FDC_DMA) {
                        dma.do_dma_write_u8(bus, FDC_DMA, byte);
                        self.latch = None;
                        self.byte_count += 1;
                        self.terminal_count |= dma.check_terminal_count(FDC_DMA);
                    }
                }
            }
            FdcDataDirection::FromHost => {
                if self.latch.is_none() && dma.check_dma_ready(FDC_DMA) {
                    let byte = dma.do_dma_read_u8(bus, FDC_DMA);
                    self.latch = Some(byte);
                    self.byte_count += 1;
                    self.terminal_count |= dma.check_terminal_count(FDC_DMA);
                }
            }
        }
    }

    pub fn cpu_read(&mut self) -> Option<u8> {
        if self.active && self.mode == FdcDataMode::Pio && self.direction == FdcDataDirection::ToHost {
            let byte = self.latch.take();
            if byte.is_some() {
                self.byte_count += 1;
            }
            byte
        }
        else {
            None
        }
    }

    pub fn cpu_write(&mut self, byte: u8) -> Result<(), FdcDataError> {
        if !self.active {
            return Err(FdcDataError::Inactive);
        }
        if self.mode != FdcDataMode::Pio || self.direction != FdcDataDirection::FromHost {
            return Err(FdcDataError::WrongDirection);
        }
        if self.latch.is_some() {
            return Err(FdcDataError::Timeout);
        }

        self.latch = Some(byte);
        Ok(())
    }

    pub fn status(&self) -> FdcDataStatus {
        if !self.active {
            return FdcDataStatus::default();
        }

        match (self.mode, self.direction) {
            (FdcDataMode::Dma, FdcDataDirection::ToHost) => FdcDataStatus {
                busy: true,
                mrq: false,
                dio_to_cpu: true,
            },
            (FdcDataMode::Dma, FdcDataDirection::FromHost) => FdcDataStatus {
                busy: true,
                mrq: false,
                dio_to_cpu: false,
            },
            (FdcDataMode::Pio, FdcDataDirection::ToHost) => FdcDataStatus {
                busy: true,
                mrq: self.latch.is_some(),
                dio_to_cpu: true,
            },
            (FdcDataMode::Pio, FdcDataDirection::FromHost) => FdcDataStatus {
                busy: true,
                mrq: self.latch.is_none(),
                dio_to_cpu: false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FdcDataDirection, FdcDataError, FdcDataInterface, FdcDataMode};
    use crate::{bus::BusInterface, devices::dma};

    fn dma_and_bus() -> (dma::DMAController, BusInterface) {
        (dma::DMAController::new(), BusInterface::default())
    }

    #[test]
    fn pio_to_host_second_send_times_out_until_cpu_read() {
        let mut interface = FdcDataInterface::default();
        let (mut dma, mut bus) = dma_and_bus();
        interface.begin(FdcDataMode::Pio, FdcDataDirection::ToHost);

        assert_eq!(interface.send(0x12, &mut dma, &mut bus), Ok(()));
        assert_eq!(interface.send(0x34, &mut dma, &mut bus), Err(FdcDataError::Timeout));
        assert_eq!(interface.cpu_read(), Some(0x12));
        assert_eq!(interface.send(0x34, &mut dma, &mut bus), Ok(()));
    }

    #[test]
    fn pio_from_host_recv_times_out_until_cpu_write() {
        let mut interface = FdcDataInterface::default();
        let (mut dma, mut bus) = dma_and_bus();
        interface.begin(FdcDataMode::Pio, FdcDataDirection::FromHost);

        assert_eq!(interface.recv(&mut dma, &mut bus), Err(FdcDataError::Timeout));
        assert_eq!(interface.cpu_write(0x56), Ok(()));
        assert_eq!(interface.recv(&mut dma, &mut bus), Ok(0x56));
    }

    #[test]
    fn dma_to_host_send_services_latch_immediately() {
        let mut interface = FdcDataInterface::default();
        let (mut dma, mut bus) = dma_and_bus();
        interface.begin(FdcDataMode::Dma, FdcDataDirection::ToHost);

        assert_eq!(interface.send(0x78, &mut dma, &mut bus), Ok(()));

        assert!(interface.latch_empty());
        assert_eq!(interface.byte_count(), 1);
    }
}

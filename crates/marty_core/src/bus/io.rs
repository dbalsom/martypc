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
*/

//! IO routines for [BusInterface].

use std::io::Write;

use crate::{
    bus::{
        BusInterface,
        ClockFactor,
        DeviceRunTimeUnit,
        IoDevice,
        IoDeviceStats,
        IoDeviceType,
        NO_IO_BYTE,
        NULL_DELTA_US,
    },
    cpu_common::LogicAnalyzer,
};

impl BusInterface {
    /// Read an 8-bit value from an IO port.
    ///
    /// We provide the elapsed cycle count for the current instruction. This allows a device
    /// to optionally tick itself to bring itself in sync with CPU state.
    pub fn io_read_u8(&mut self, port: u16, cycles: u32) -> u8 {
        // Convert cycles to system clock ticks
        let sys_ticks = match self.cpu_factor {
            ClockFactor::Divisor(d) => d as u32 * cycles,
            ClockFactor::Multiplier(m) => cycles / m as u32,
        };
        let nul_delta = DeviceRunTimeUnit::Microseconds(0.0);
        let mut byte = None;
        if let Some(device_id) = self.io_map.get(&port) {
            match device_id {
                IoDeviceType::A0Register => {
                    if let Some(a0) = &mut self.a0 {
                        byte = Some(a0.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::Ppi => {
                    if let Some(ppi) = &mut self.ppi {
                        byte = Some(ppi.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::Pit => {
                    // There will always be a PIT, so safe to unwrap
                    byte = Some(
                        self.pit
                            .as_mut()
                            .unwrap()
                            .read_u8(port, DeviceRunTimeUnit::SystemTicks(sys_ticks)),
                    );
                    //self.pit.as_mut().unwrap().read_u8(port, nul_delta)
                }
                IoDeviceType::DmaPrimary => {
                    // There will always be a primary DMA, so safe to unwrap
                    byte = Some(self.dma1.as_mut().unwrap().read_u8(port, nul_delta));
                }
                IoDeviceType::DmaSecondary => {
                    // Secondary DMA may not exist
                    if let Some(dma2) = &mut self.dma2 {
                        byte = Some(dma2.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::PicPrimary => {
                    // There will always be a primary PIC, so safe to unwrap
                    byte = Some(self.pic1.as_mut().unwrap().read_u8(port, nul_delta));
                }
                IoDeviceType::PicSecondary => {
                    // Secondary PIC may not exist
                    if let Some(pic2) = &mut self.pic2 {
                        byte = Some(pic2.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::FloppyController => {
                    if let Some(fdc) = &mut self.fdc {
                        byte = Some(fdc.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::HardDiskController => {
                    if let Some(hdc) = &mut self.hdc {
                        byte = Some(hdc.read_u8(port, nul_delta));
                    }
                    else if let Some(xtide) = &mut self.xtide {
                        byte = Some(xtide.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::Serial => {
                    if let Some(serial) = &mut self.serial {
                        // Serial port write does not need bus.
                        byte = Some(serial.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::Parallel => {
                    if let Some(parallel) = &mut self.parallel {
                        byte = Some(parallel.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::Ems => {
                    if let Some(ems) = &mut self.ems {
                        byte = Some(ems.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::GamePort => {
                    if let Some(game_port) = &mut self.game_port {
                        byte = Some(game_port.read_u8(port, nul_delta));
                    }
                }
                IoDeviceType::Video(vid) => {
                    if let Some(video_dispatch) = self.videocards.get_mut(&vid) {
                        byte = Some(video_dispatch.io_read_u8(port, DeviceRunTimeUnit::SystemTicks(sys_ticks)));
                    }
                }
                IoDeviceType::Sound =>
                {
                    #[cfg(feature = "opl")]
                    if let Some(adlib) = &mut self.adlib {
                        byte = Some(adlib.read_u8(port, nul_delta));
                    }
                }
                _ => {}
            }
        }

        let byte_val = byte.unwrap_or(NO_IO_BYTE);

        self.io_stats
            .entry(port)
            .and_modify(|e| {
                e.1.last_read = byte_val;
                e.1.reads += 1;
                e.1.reads_dirty = true;
            })
            .or_insert((byte.is_some(), IoDeviceStats::one_read()));

        byte_val
    }

    /// Write an 8-bit value to an IO port.
    ///
    /// We provide the elapsed cycle count for the current instruction. This allows a device
    /// to optionally tick itself to bring itself in sync with CPU state.
    pub fn io_write_u8(&mut self, port: u16, data: u8, cycles: u32, analyzer: Option<&mut LogicAnalyzer>) {
        // Convert cycles to system clock ticks
        let sys_ticks = match self.cpu_factor {
            ClockFactor::Divisor(n) => cycles * (n as u32),
            ClockFactor::Multiplier(n) => cycles / (n as u32),
        };

        // Handle terminal debug port
        if let Some(terminal_port) = self.terminal_port {
            if port == terminal_port {
                //log::debug!("Write to terminal port: {:02X}", data);

                // Filter Escape character to avoid terminal shenanigans.
                // See: https://www.cyberark.com/resources/threat-research-blog/dont-trust-this-title-abusing-terminal-emulators-with-ansi-escape-characters
                if data != 0x1B {
                    print!("{}", data as char);
                    _ = std::io::stdout().flush();
                }
            }
        }

        let mut resolved = false;
        if let Some(device_id) = self.io_map.get(&port) {
            match device_id {
                IoDeviceType::A0Register => {
                    if let Some(a0) = &mut self.a0 {
                        a0.write_u8(port, data, None, NULL_DELTA_US, analyzer);
                        resolved = true;
                    }
                }
                IoDeviceType::Ppi => {
                    if let Some(mut ppi) = self.ppi.take() {
                        ppi.write_u8(port, data, Some(self), NULL_DELTA_US, analyzer);
                        resolved = true;
                        self.ppi = Some(ppi);
                    }
                }
                IoDeviceType::Pit => {
                    if let Some(mut pit) = self.pit.take() {
                        //log::debug!("writing PIT with {} cycles", cycles);
                        pit.write_u8(
                            port,
                            data,
                            Some(self),
                            DeviceRunTimeUnit::SystemTicks(sys_ticks),
                            analyzer,
                        );
                        resolved = true;
                        self.pit = Some(pit);
                    }
                }
                IoDeviceType::DmaPrimary => {
                    if let Some(mut dma1) = self.dma1.take() {
                        dma1.write_u8(port, data, Some(self), NULL_DELTA_US, analyzer);
                        resolved = true;
                        self.dma1 = Some(dma1);
                    }
                }
                IoDeviceType::DmaSecondary => {
                    if let Some(mut dma2) = self.dma2.take() {
                        dma2.write_u8(port, data, Some(self), NULL_DELTA_US, analyzer);
                        resolved = true;
                        self.dma2 = Some(dma2);
                    }
                }
                IoDeviceType::PicPrimary => {
                    if let Some(mut pic1) = self.pic1.take() {
                        pic1.write_u8(port, data, Some(self), NULL_DELTA_US, analyzer);
                        resolved = true;
                        self.pic1 = Some(pic1);
                    }
                }
                IoDeviceType::PicSecondary => {
                    if let Some(mut pic2) = self.pic2.take() {
                        pic2.write_u8(port, data, Some(self), NULL_DELTA_US, analyzer);
                        resolved = true;
                        self.pic2 = Some(pic2);
                    }
                }
                IoDeviceType::FloppyController => {
                    if let Some(mut fdc) = self.fdc.take() {
                        fdc.write_u8(port, data, Some(self), NULL_DELTA_US, analyzer);
                        resolved = true;
                        self.fdc = Some(fdc);
                    }
                }
                IoDeviceType::HardDiskController => {
                    if let Some(mut hdc) = self.hdc.take() {
                        hdc.write_u8(port, data, Some(self), NULL_DELTA_US, analyzer);
                        resolved = true;
                        self.hdc = Some(hdc);
                    }
                    else if let Some(mut xtide) = self.xtide.take() {
                        xtide.write_u8(port, data, Some(self), NULL_DELTA_US, analyzer);
                        resolved = true;
                        self.xtide = Some(xtide);
                    }
                }
                IoDeviceType::Serial => {
                    if let Some(serial) = &mut self.serial {
                        // Serial port write does not need bus.
                        serial.write_u8(port, data, None, NULL_DELTA_US, analyzer);
                        resolved = true;
                    }
                }
                IoDeviceType::Parallel => {
                    if let Some(parallel) = &mut self.parallel {
                        parallel.write_u8(port, data, None, NULL_DELTA_US, analyzer);
                        resolved = true;
                    }
                }
                IoDeviceType::Ems => {
                    if let Some(ems) = &mut self.ems {
                        ems.write_u8(port, data, None, NULL_DELTA_US, analyzer);
                        resolved = true;
                    }
                }
                IoDeviceType::GamePort => {
                    if let Some(game_port) = &mut self.game_port {
                        game_port.write_u8(port, data, None, NULL_DELTA_US, analyzer);
                        resolved = true;
                    }
                }
                IoDeviceType::Video(vid) => {
                    if let Some(video_dispatch) = self.videocards.get_mut(&vid) {
                        video_dispatch.io_write_u8(
                            port,
                            data,
                            None,
                            DeviceRunTimeUnit::SystemTicks(sys_ticks),
                            analyzer,
                        );
                        resolved = true;
                    }
                }
                IoDeviceType::Sound =>
                {
                    #[cfg(feature = "opl")]
                    if let Some(adlib) = &mut self.adlib {
                        IoDevice::write_u8(adlib, port, data, None, NULL_DELTA_US, analyzer);
                    }
                }
                _ => {}
            }
        }

        self.io_stats
            .entry(port)
            .and_modify(|e| {
                e.1.writes += 1;
                e.1.writes_dirty = true;
            })
            .or_insert((resolved, IoDeviceStats::one_write()));
    }
}

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

//! Enum-based dispatch for devices on the system bus.

use crate::{
    bus::{BusInterface, DeviceRunTimeUnit, IoDevice, MemoryMappedDevice, NO_IO_BYTE},
    cpu_common::LogicAnalyzer,
    device_traits::videocard::VideoCardDispatch,
};

impl VideoCardDispatch {
    pub fn io_read_u8(&mut self, port: u16, delta: DeviceRunTimeUnit) -> u8 {
        match self {
            VideoCardDispatch::None => NO_IO_BYTE,
            VideoCardDispatch::Mda(mda) => IoDevice::read_u8(mda, port, delta),
            VideoCardDispatch::Cga(cga) => IoDevice::read_u8(cga, port, delta),
            VideoCardDispatch::Tga(tga) => IoDevice::read_u8(tga, port, delta),
            #[cfg(feature = "ega")]
            VideoCardDispatch::Ega(ega) => IoDevice::read_u8(ega, port, delta),
            #[cfg(feature = "vga")]
            VideoCardDispatch::Vga(vga) => IoDevice::read_u8(vga, port, delta),
        }
    }

    pub fn io_write_u8(
        &mut self,
        port: u16,
        data: u8,
        bus: Option<&mut BusInterface>,
        delta: DeviceRunTimeUnit,
        analyzer: Option<&mut LogicAnalyzer>,
    ) {
        match self {
            VideoCardDispatch::None => {}
            VideoCardDispatch::Mda(mda) => IoDevice::write_u8(mda, port, data, bus, delta, analyzer),
            VideoCardDispatch::Cga(cga) => IoDevice::write_u8(cga, port, data, bus, delta, analyzer),
            VideoCardDispatch::Tga(tga) => IoDevice::write_u8(tga, port, data, bus, delta, analyzer),
            #[cfg(feature = "ega")]
            VideoCardDispatch::Ega(ega) => IoDevice::write_u8(ega, port, data, bus, delta, analyzer),
            #[cfg(feature = "vga")]
            VideoCardDispatch::Vga(vga) => IoDevice::write_u8(vga, port, data, bus, delta, analyzer),
        }
    }

    pub fn mmio_peek_u8(&self, address: usize, cpumem: Option<&[u8]>) -> u8 {
        match self {
            VideoCardDispatch::None => NO_IO_BYTE,
            VideoCardDispatch::Mda(mda) => MemoryMappedDevice::mmio_peek_u8(mda, address, cpumem),
            VideoCardDispatch::Cga(cga) => MemoryMappedDevice::mmio_peek_u8(cga, address, cpumem),
            VideoCardDispatch::Tga(tga) => MemoryMappedDevice::mmio_peek_u8(tga, address, cpumem),
            #[cfg(feature = "ega")]
            VideoCardDispatch::Ega(ega) => MemoryMappedDevice::mmio_peek_u8(ega, address, cpumem),
            #[cfg(feature = "vga")]
            VideoCardDispatch::Vga(vga) => MemoryMappedDevice::mmio_peek_u8(vga, address, cpumem),
        }
    }

    pub fn mmio_read_u8(&mut self, address: usize, ticks: u32, cpumem: Option<&[u8]>) -> (u8, u32) {
        match self {
            VideoCardDispatch::None => (NO_IO_BYTE, 0),
            VideoCardDispatch::Mda(mda) => MemoryMappedDevice::mmio_read_u8(mda, address, ticks, cpumem),
            VideoCardDispatch::Cga(cga) => MemoryMappedDevice::mmio_read_u8(cga, address, ticks, cpumem),
            VideoCardDispatch::Tga(tga) => MemoryMappedDevice::mmio_read_u8(tga, address, ticks, cpumem),
            #[cfg(feature = "ega")]
            VideoCardDispatch::Ega(ega) => MemoryMappedDevice::mmio_read_u8(ega, address, ticks, cpumem),
            #[cfg(feature = "vga")]
            VideoCardDispatch::Vga(vga) => MemoryMappedDevice::mmio_read_u8(vga, address, ticks, cpumem),
        }
    }
}

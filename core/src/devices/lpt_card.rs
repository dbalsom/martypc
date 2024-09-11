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

    devices::lpt_card.rs

    Implementation of an ISA card hosting a single parallel port.

*/

use crate::{
    bus::{BusInterface, DeviceRunTimeUnit, IoDevice, NO_IO_BYTE},
    devices::{lpt_port::ParallelPort},
};

pub const LPT_DEFAULT_IO_BASE: u16 = 0x3BC;
pub const LPT_PORT_MASK: u16 = !0x003;

pub struct ParallelController {
    lpt_port_base: u16,
    lpt: ParallelPort,
}

impl Default for ParallelController {
    fn default() -> Self {
        ParallelController {
            lpt_port_base: LPT_DEFAULT_IO_BASE,
            lpt: ParallelPort::default(),
        }
    }
}

impl ParallelController {
    pub fn new(port_base: Option<u16>) -> Self {
        ParallelController {
            lpt_port_base: port_base.unwrap_or(LPT_DEFAULT_IO_BASE),
            ..Default::default()
        }
    }
}

impl IoDevice for ParallelController {
    fn read_u8(&mut self, port: u16, _delta: DeviceRunTimeUnit) -> u8 {
        // Catch up to CPU state.
        //let _ticks = self.catch_up(delta, false);
        //self.rw_op(ticks, 0, port as u32, RwSlotType::Io);

        if (port & LPT_PORT_MASK) == self.lpt_port_base {
            self.lpt.port_read(port)
        }
        else {
            NO_IO_BYTE
        }
    }

    fn write_u8(&mut self, port: u16, data: u8, _bus: Option<&mut BusInterface>, _delta: DeviceRunTimeUnit) {
        if (port & LPT_PORT_MASK) == self.lpt_port_base {
            // Read is from LPT port.
            self.lpt.port_write(port, data);
        }
    }

    fn port_list(&self) -> Vec<(String, u16)> {
        vec![
            ("LPT Data".to_string(), self.lpt_port_base),
            ("LPT Status".to_string(), self.lpt_port_base + 1),
            ("LPT Control".to_string(), self.lpt_port_base + 2),
        ]
    }
}

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

    devices::a0.rs

    Implements the A0 NMI port

    // TODO: Move this a component model, and make it part of a motherboard type
*/

#[derive(Copy, Clone, Debug)]
pub enum A0Type {
    PCXT,
    PCJr,
    Tandy1000,
}

use crate::{
    bus::{BusInterface, DeviceRunTimeUnit, IoDevice},
    devices::pit::Pit,
};

pub struct A0Register {
    a0type:  A0Type,
    a0_byte: u8,

    // PCJr specific
    nmi_latch: bool,
    // register bits 0-3
    nmi_enabled: bool,
    ir_test_ena: bool,
    clock_1_select: bool,
    hrq_disable: bool,

    clear_nmi_latch: bool,
}

impl IoDevice for A0Register {
    fn read_u8(&mut self, _port: u16, _delta: DeviceRunTimeUnit) -> u8 {
        // Catch up to CPU state.
        //self.catch_up(delta);

        match self.a0type {
            A0Type::PCJr => {
                log::debug!("flagging nmi latch to be cleared.");
                self.clear_nmi_latch = true;
                // Value returned not important?
                0xFF
            }
            _ => 0xFF,
        }
    }

    fn write_u8(&mut self, _port: u16, data: u8, _bus_opt: Option<&mut BusInterface>, _delta: DeviceRunTimeUnit) {
        self.a0_byte = data;
        log::debug!("A0 NMI Control Register Write: {:08b}", data);
        match self.a0type {
            A0Type::PCJr => {
                self.nmi_enabled = (data & 0x80) != 0;
                self.ir_test_ena = (data & 0x40) != 0;
                self.clock_1_select = (data & 0x20) != 0;
                self.hrq_disable = (data & 0x10) != 0;
            }
            _ => {}
        }
    }

    fn port_list(&self) -> Vec<(String, u16)> {
        vec![(String::from("A0 NMI Control"), 0xA0)]
    }
}

impl A0Register {
    pub fn new(a0type: A0Type) -> A0Register {
        A0Register {
            a0type,
            a0_byte: 0,
            nmi_latch: false,
            nmi_enabled: false,
            ir_test_ena: false,
            clock_1_select: false,
            hrq_disable: false,
            clear_nmi_latch: false,
        }
    }

    pub fn enable_nmi(&mut self, state: bool) {
        self.nmi_enabled = state;
    }

    pub fn set_nmi_latch(&mut self, state: bool) {
        log::debug!("Setting nmi latch: {}, enabled: {}", state, self.nmi_enabled);
        if state && self.nmi_enabled {
            self.nmi_latch = true;
        }
        else {
            self.nmi_latch = false;
        }
    }

    pub fn is_nmi_enabled(&self) -> bool {
        self.nmi_enabled
    }

    pub fn ir_test_ena(&self) -> bool {
        self.ir_test_ena
    }

    pub fn clock_1_select(&self) -> bool {
        self.clock_1_select
    }

    pub fn hrq_disable(&self) -> bool {
        self.hrq_disable
    }

    pub fn read(&self) -> u8 {
        self.a0_byte
    }

    pub fn run(&mut self, pit: &mut Pit, _us: f64) -> bool {
        // The run method doesn't need to process time. If the clear_nmi_latch flag is set, then we
        // clear the NMI latch.
        // Otherwise, return the value of the latch.

        if self.clear_nmi_latch {
            log::warn!("Clearing NMI latch");
            self.clear_nmi_latch = false;
            self.nmi_latch = false;
        }

        self.nmi_latch
    }
}

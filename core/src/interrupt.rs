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

    interrupt.rs

    Interrupt logging routines.
*/

#![allow(dead_code)]

use log;

use crate::{bus::BusInterface, cpu_808x::CpuRegisterState};

/// Function to log interrupt return values - called on return from interrupt (IRET)
pub fn log_post_interrupt(int: u8, ah: u8, regs: &CpuRegisterState, bus: &mut BusInterface) {
    match int {
        0x10 => {
            // Video services
        }
        0x21 => {
            // Dos services

            log_post_interrupt21(ah, regs, bus);
        }
        _ => {}
    }
}

pub fn log_post_interrupt21(ah: u8, regs: &CpuRegisterState, bus: &mut BusInterface) {
    match ah {
        0x4b => {
            // Load and Execute Program

            let seg = regs.es;
            let offset = regs.bx;

            let cs_offset = offset.wrapping_add(0x12); // Offset of CS:IP in parameter block
            let ip_offset = offset.wrapping_add(0x14);

            let cs_addr = ((seg as usize) << 4) + cs_offset as usize;
            let ip_addr = ((seg as usize) << 4) + ip_offset as usize;

            let (cs, _) = bus.read_u16(cs_addr, 0).unwrap();
            let (ip, _) = bus.read_u16(ip_addr, 0).unwrap();

            log::trace!("int21h: 4B Load and Execute Program: CS:IP: [{:04X}]:[{:04X}]", cs, ip);
        }
        _ => {
            log::trace!("int21h: {:02X}", ah);
        }
    }
}

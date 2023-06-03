/*
    MartyPC Emulator 
    (C)2023 Daniel Balsom
    https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

    --------------------------------------------------------------------------

    interrupt.rs

    Interrupt logging routines.
*/

#![allow(dead_code)]

use log;

use crate::cpu_808x::CpuRegisterState;
use crate::bus::BusInterface;

/// Function to log interrupt return values - called on return from interrupt (IRET)
pub fn log_post_interrupt(int: u8, ah: u8, regs: &CpuRegisterState, bus: &mut BusInterface ) {

    match int {

        0x10 => {
            // Video services
        },
        0x21 => {
            // Dos services

            log_post_interrupt21(ah, regs, bus);
        },
        _ => {}
    }
}

pub fn log_post_interrupt21(ah: u8, regs: &CpuRegisterState, bus: &mut BusInterface ) {

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

        },
        _ => {
            log::trace!("int21h: {:02X}", ah);
        }
    }
}
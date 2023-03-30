
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

            let (cs, _) = bus.read_u16(cs_addr).unwrap();
            let (ip, _) = bus.read_u16(ip_addr).unwrap();

            log::trace!("int21h: 4B Load and Execute Program: CS:IP: [{:04X}]:[{:04X}]", cs, ip);

        },
        _ => {
            log::trace!("int21h: {:02X}", ah);
        }
    }
}
/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

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

//! Guest memory fast-copy service.

use crate::cpu_common::{Cpu, Register16};

use super::{clear_carry, read_guest_u8, set_service_error, write_guest_u8, ServiceError, ServiceInterruptManager};

const DIRECTION_FLAG: u16 = 0x0400;

impl ServiceInterruptManager {
    /// This function replicates the behavior of MOVSB but performs an instantaneous copy.
    /// Segment wrapping is not modelled, with the exception of the last byte of a segment after
    /// which DI is permitted to be 0.
    pub(super) fn fast_copy<C: Cpu>(&self, cpu: &mut C) {
        let mut remaining = cpu.get_register16(Register16::CX);
        if remaining == 0 {
            clear_carry(cpu);
            return;
        }

        let source_segment = cpu.get_register16(Register16::DS);
        let destination_segment = cpu.get_register16(Register16::ES);
        let mut source_offset = cpu.get_register16(Register16::SI);
        let mut destination_offset = cpu.get_register16(Register16::DI);
        let reverse = cpu.get_flags() & DIRECTION_FLAG != 0;
        let span = remaining - 1;

        let crosses_segment = if reverse {
            span > source_offset || span > destination_offset
        }
        else {
            u32::from(source_offset) + u32::from(span) > u32::from(u16::MAX)
                || u32::from(destination_offset) + u32::from(span) > u32::from(u16::MAX)
        };
        if crosses_segment {
            set_service_error(cpu, ServiceError::InvalidParameter);
            return;
        }

        let step = if reverse { u16::MAX } else { 1 };
        let error = loop {
            if remaining == 0 {
                break None;
            }
            // Keep reads and writes interleaved so overlap and mapped-device accesses
            // have the same ordering as MOVSB.
            let result = read_guest_u8(cpu, source_segment, source_offset)
                .and_then(|value| write_guest_u8(cpu, destination_segment, destination_offset, value));
            if let Err(error) = result {
                break Some(error);
            }
            source_offset = source_offset.wrapping_add(step);
            destination_offset = destination_offset.wrapping_add(step);
            remaining -= 1;
        };

        // On a bus error, expose progress through the last successfully copied byte.
        cpu.set_register16(Register16::SI, source_offset);
        cpu.set_register16(Register16::DI, destination_offset);
        cpu.set_register16(Register16::CX, remaining);
        if let Some(error) = error {
            set_service_error(cpu, error);
        }
        else {
            clear_carry(cpu);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        super::{
            test_support::{read_guest_bytes, write_guest_bytes},
            ServiceControl,
            ServiceFunction,
            CARRY_FLAG,
            SERVICE_CTRL_BX,
            SERVICE_CTRL_CX,
        },
        *,
    };
    use crate::cpu_common::{Register8, ServiceEvent};

    fn prepare_fast_copy<C: Cpu>(cpu: &mut C, source: (u16, u16), destination: (u16, u16), count: u16, reverse: bool) {
        cpu.set_register16(Register16::AX, 0x12AB); // AL is ignored and preserved on success.
        cpu.set_register16(Register16::DS, source.0);
        cpu.set_register16(Register16::SI, source.1);
        cpu.set_register16(Register16::ES, destination.0);
        cpu.set_register16(Register16::DI, destination.1);
        cpu.set_register16(Register16::CX, count);
        cpu.set_flags(0x08D5 | if reverse { DIRECTION_FLAG } else { 0 });
    }

    fn check_fast_copy_boundaries<C: Cpu>(cpu: &mut C) {
        let mut manager = ServiceInterruptManager::default();
        // Direction, initial offset, count, expected final offset.
        for (reverse, offset, count, final_offset) in [
            (false, 0xFFFF, 0, 0xFFFF),
            (true, 0x0000, 0, 0x0000),
            (false, 0x0000, 1, 0x0001),
            (true, 0xFFFF, 1, 0xFFFE),
            (false, 0xFFFF, 1, 0x0000),
            (true, 0x0000, 1, 0xFFFF),
            (false, 0xFFF8, 8, 0x0000),
            (true, 0x0007, 8, 0xFFFF),
            (false, 0x0000, 0xFFFF, 0xFFFF),
            (true, 0xFFFE, 0xFFFF, 0xFFFF),
        ] {
            let low_offset = if reverse && count > 0 {
                offset - (count - 1)
            }
            else {
                offset
            };
            let bytes: Vec<u8> = (0..count).map(|index| (index % 251 + 1) as u8).collect();
            write_guest_bytes(cpu, 0x1000, low_offset, &bytes);
            write_guest_bytes(cpu, 0x3000, low_offset, &vec![0; usize::from(count)]);
            prepare_fast_copy(cpu, (0x1000, offset), (0x3000, offset), count, reverse);
            let preserved = [
                Register16::AX,
                Register16::BX,
                Register16::DX,
                Register16::BP,
                Register16::SP,
                Register16::CS,
                Register16::DS,
                Register16::ES,
                Register16::SS,
            ]
            .map(|reg| (reg, cpu.get_register16(reg)));
            let flags = cpu.get_flags();
            let cycles = cpu.get_cycle_ct();

            assert!(manager.handle_interrupt(ServiceFunction::FastCopy, cpu).is_none());

            assert_eq!(read_guest_bytes(cpu, 0x3000, low_offset, bytes.len()), bytes);
            assert_eq!(cpu.get_register16(Register16::SI), final_offset);
            assert_eq!(cpu.get_register16(Register16::DI), final_offset);
            assert_eq!(cpu.get_register16(Register16::CX), 0);
            assert_eq!(cpu.get_flags(), flags & !CARRY_FLAG);
            assert_eq!(cpu.get_cycle_ct(), cycles);
            for (reg, value) in preserved {
                assert_eq!(cpu.get_register16(reg), value, "{reg:?}");
            }
        }
    }

    #[test]
    fn copy_respects_segments_and_registers() {
        check_fast_copy_boundaries(&mut crate::cpu_808x::Intel808x::default());
        check_fast_copy_boundaries(&mut crate::cpu_vx0::NecVx0::default());
    }

    #[test]
    fn copy_rejects_segment_overflow() {
        let mut manager = ServiceInterruptManager::default();
        let mut cpu = crate::cpu_808x::Intel808x::default();
        for (reverse, source, destination, count) in [
            (false, 0xFFFF, 0x0100, 2),
            (false, 0x0100, 0xFFFF, 2),
            (false, 0xFFF8, 0xFFF8, 9),
            (false, 0xFFFF, 0xFFFF, 0xFFFF),
            (true, 0x0000, 0x0100, 2),
            (true, 0x0100, 0x0000, 2),
            (true, 0x0007, 0x0007, 9),
            (true, 0x0001, 0x0001, 0xFFFF),
        ] {
            prepare_fast_copy(&mut cpu, (0x1000, source), (0x3000, destination), count, reverse);
            // A read would fail, proving range validation takes precedence over bus access.
            let address = crate::cpu_common::calc_linear_address(0x1000, source) as usize;
            cpu.bus_mut().set_flags(address, crate::bus::MEM_MMIO_BIT);
            let before = cpu.bus_mut().get_vec_at(0, 0x100000);
            let flags = cpu.get_flags() & !CARRY_FLAG;
            cpu.set_flags(flags);

            manager.handle_interrupt(ServiceFunction::FastCopy, &mut cpu);

            assert_eq!(
                cpu.get_register16(Register16::AX),
                u16::from(ServiceError::InvalidParameter)
            );
            assert_eq!(cpu.get_register16(Register16::SI), source);
            assert_eq!(cpu.get_register16(Register16::DI), destination);
            assert_eq!(cpu.get_register16(Register16::CX), count);
            assert_eq!(cpu.get_flags(), flags | CARRY_FLAG);
            assert_eq!(cpu.bus_mut().get_slice_at(0, 0x100000), before);
            cpu.bus_mut().clear_flags(address, crate::bus::MEM_MMIO_BIT);
        }
    }

    #[test]
    fn overlapping_copy_follows_direction() {
        let mut manager = ServiceInterruptManager::default();
        let mut cpu = crate::cpu_808x::Intel808x::default();
        for (reverse, source, destination, expected) in [
            (false, 0, 1, &b"aaaaaa"[..]),
            (true, 5, 4, &b"ffffff"[..]),
            (false, 1, 0, &b"bcdeff"[..]),
            (true, 4, 5, &b"aabcde"[..]),
            (false, 0, 0, &b"abcdef"[..]),
        ] {
            write_guest_bytes(&mut cpu, 0x1000, 0x0100, b"abcdef");
            // 1000:0100 and 1010:0000 refer to the same physical byte.
            prepare_fast_copy(&mut cpu, (0x1000, 0x0100 + source), (0x1010, destination), 5, reverse);
            manager.handle_interrupt(ServiceFunction::FastCopy, &mut cpu);
            assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);
            assert_eq!(read_guest_bytes(&mut cpu, 0x1000, 0x0100, 6), expected);
        }
    }

    #[test]
    fn copy_wraps_at_one_megabyte() {
        let mut manager = ServiceInterruptManager::default();
        let mut cpu = crate::cpu_808x::Intel808x::default();
        for reverse in [false, true] {
            for high_source in [false, true] {
                let (source, destination) = if high_source {
                    (0xFFFF, 0x1000)
                }
                else {
                    (0x1000, 0xFFFF)
                };
                write_guest_bytes(&mut cpu, source, 0x000E, b"wrap");
                write_guest_bytes(&mut cpu, destination, 0x000E, &[0; 4]);
                let offset = if reverse { 0x0011 } else { 0x000E };
                prepare_fast_copy(&mut cpu, (source, offset), (destination, offset), 4, reverse);
                manager.handle_interrupt(ServiceFunction::FastCopy, &mut cpu);
                assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);
                assert_eq!(read_guest_bytes(&mut cpu, destination, 0x000E, 4), b"wrap");
            }
        }
    }

    #[test]
    fn copy_reports_progress_on_failure() {
        let mut manager = ServiceInterruptManager::default();
        for reverse in [false, true] {
            let mut cpu = crate::cpu_808x::Intel808x::default();
            write_guest_bytes(&mut cpu, 0x1000, 0x0100, b"abcd");
            write_guest_bytes(&mut cpu, 0x3000, 0x0100, b"----");
            let offset = if reverse { 0x0103 } else { 0x0100 };
            let failure_offset = if reverse { 0x0101 } else { 0x0102 };
            cpu.bus_mut()
                .set_flags(0x10000 + usize::from(failure_offset), crate::bus::MEM_MMIO_BIT);
            prepare_fast_copy(&mut cpu, (0x1000, offset), (0x3000, offset), 4, reverse);
            let flags = cpu.get_flags();
            let cycles = cpu.get_cycle_ct();

            manager.handle_interrupt(ServiceFunction::FastCopy, &mut cpu);

            assert_eq!(cpu.get_register16(Register16::AX), u16::from(ServiceError::InvalidData));
            assert_eq!(cpu.get_flags(), flags | CARRY_FLAG);
            assert_eq!(cpu.get_register16(Register16::SI), failure_offset);
            assert_eq!(cpu.get_register16(Register16::DI), failure_offset);
            assert_eq!(cpu.get_register16(Register16::CX), 2);
            assert_eq!(cpu.get_cycle_ct(), cycles);
            assert_eq!(
                read_guest_bytes(&mut cpu, 0x3000, 0x0100, 4),
                if reverse { b"--cd" } else { b"ab--" }
            );

            prepare_fast_copy(&mut cpu, (0x1000, failure_offset), (0x3000, failure_offset), 0, reverse);
            manager.handle_interrupt(ServiceFunction::FastCopy, &mut cpu);
            assert_eq!(cpu.get_flags(), flags & !CARRY_FLAG);
            assert_eq!(cpu.get_register16(Register16::AX), 0x12AB);
            assert_eq!(cpu.get_register16(Register16::SI), failure_offset);
            assert_eq!(cpu.get_register16(Register16::DI), failure_offset);
            assert_eq!(cpu.get_register16(Register16::CX), 0);
        }
    }

    #[test]
    fn copy_uses_mapped_memory() {
        use crate::{bus::MemoryMappedDevice, devices::lotech_ems::LotechEmsCard};

        let mut cpu = crate::cpu_808x::Intel808x::default();
        let mut manager = ServiceInterruptManager::default();
        let mut ems = LotechEmsCard::new(None, Some(0xE000));
        ems.page_reg_write(0, 7);
        for (index, value) in b"EMS!".iter().copied().enumerate() {
            ems.mmio_write_u8(0xE0100 + index, value, 0, None);
        }
        crate::bus::test_support::install_ems(cpu.bus_mut(), ems);

        // Read backward from EMS into RAM, then write forward from RAM into EMS.
        for (source, destination, offset, flags, bytes) in [
            (0xE000, 0x1000, 0x0103, 0x0401, &b"EMS!"[..]),
            (0x1000, 0xE000, 0x0100, 0x0001, &b"BACK"[..]),
        ] {
            if source == 0x1000 {
                write_guest_bytes(&mut cpu, 0x1000, 0x0100, bytes);
            }
            cpu.set_register16(Register16::DS, source);
            cpu.set_register16(Register16::ES, destination);
            cpu.set_register16(Register16::SI, offset);
            cpu.set_register16(Register16::DI, offset);
            cpu.set_register16(Register16::CX, 4);
            cpu.set_flags(flags);
            let cycles = cpu.get_cycle_ct();

            manager.handle_interrupt(ServiceFunction::FastCopy, &mut cpu);

            assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);
            assert_eq!(cpu.get_cycle_ct(), cycles);
            assert_eq!(cpu.bus_mut().get_slice_at(0x10100, 4), bytes);
            assert_eq!(cpu.bus_mut().get_slice_at(0xE0100, 4), &[0; 4]);
            for (index, value) in bytes.iter().copied().enumerate() {
                assert_eq!(cpu.bus_mut().peek_u8(0xE0100 + index).unwrap(), value);
            }
        }
    }

    #[test]
    fn copy_write_failure_reports_progress() {
        let mut cpu = crate::cpu_808x::Intel808x::default();
        let mut manager = ServiceInterruptManager::default();
        write_guest_bytes(&mut cpu, 0x1000, 0, b"copy");
        // Restrict backing memory to inject a write failure after two copied bytes.
        crate::bus::test_support::truncate_memory(cpu.bus_mut(), 0x20002);
        cpu.set_register16(Register16::DS, 0x1000);
        cpu.set_register16(Register16::ES, 0x2000);
        cpu.set_register16(Register16::SI, 0);
        cpu.set_register16(Register16::DI, 0);
        cpu.set_register16(Register16::CX, 4);
        cpu.set_flags(0);

        manager.handle_interrupt(ServiceFunction::FastCopy, &mut cpu);

        assert_eq!(cpu.get_flags() & CARRY_FLAG, 1);
        assert_eq!(cpu.get_register16(Register16::AX), u16::from(ServiceError::InvalidData));
        assert_eq!(cpu.get_register16(Register16::SI), 2);
        assert_eq!(cpu.get_register16(Register16::DI), 2);
        assert_eq!(cpu.get_register16(Register16::CX), 2);
        assert_eq!(cpu.bus_mut().get_slice_at(0x20000, 2), b"co");
    }

    fn check_fast_copy_dispatch<C: Cpu>(cpu: &mut C, interrupt: impl Fn(&mut C, u8)) {
        use crate::cpu_common::CpuOption;

        let mut manager = ServiceInterruptManager::new(Some(0xFC), false);
        cpu.set_option(CpuOption::ServiceInterruptVector(Some(0xFC)));
        cpu.set_option(CpuOption::ServiceInterruptEnabled(false));
        write_guest_bytes(cpu, 0x1000, 0, b"test");
        prepare_fast_copy(cpu, (0x1000, 0), (0x3000, 0), 4, false);
        let flags = cpu.get_flags();
        manager.handle_interrupt(ServiceFunction::FastCopy, cpu);
        assert_eq!(cpu.get_flags(), flags);
        assert_eq!(cpu.get_register16(Register16::CX), 4);
        assert_eq!(read_guest_bytes(cpu, 0x3000, 0, 4), [0; 4]);

        cpu.set_register8(Register8::AH, ServiceFunction::ServiceControl.into());
        cpu.set_register8(Register8::AL, ServiceControl::Enable.into());
        cpu.set_register16(Register16::BX, SERVICE_CTRL_BX);
        cpu.set_register16(Register16::CX, SERVICE_CTRL_CX);
        interrupt(cpu, 0xFC);
        assert!(matches!(
            cpu.get_service_event(),
            Some(ServiceEvent::ServiceInterrupt(0))
        ));
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::ServiceControl, cpu),
            Some(ServiceEvent::ServiceInterruptEnabled(true))
        ));
        cpu.set_option(CpuOption::ServiceInterruptEnabled(true));

        prepare_fast_copy(cpu, (0x1000, 0), (0x3000, 0), 4, false);
        interrupt(cpu, 0xFC);
        let Some(ServiceEvent::ServiceInterrupt(function)) = cpu.get_service_event()
        else {
            panic!("fast copy did not dispatch a service event");
        };
        assert_eq!(function, 0x12);
        let cycles = cpu.get_cycle_ct();
        manager.handle_interrupt(ServiceFunction::try_from(function).unwrap(), cpu);
        assert_eq!(cpu.get_cycle_ct(), cycles);
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(read_guest_bytes(cpu, 0x3000, 0, 4), b"test");
    }

    #[test]
    fn copy_uses_configured_interrupt() {
        check_fast_copy_dispatch(&mut crate::cpu_808x::Intel808x::default(), |cpu, vector| {
            cpu.sw_interrupt(vector)
        });
        check_fast_copy_dispatch(&mut crate::cpu_vx0::NecVx0::default(), |cpu, vector| {
            cpu.sw_interrupt(vector)
        });
    }
}

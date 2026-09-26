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

//! Host-provided key/value services.

use crate::cpu_common::{Cpu, Register16, Register8};

use super::{read_guest_u8, write_guest_u8, ServiceError, ServiceInterruptManager};

/// Maximum key length in ASCII bytes, excluding the guest NUL terminator.
pub const KEY_VALUE_MAX_KEY_LEN: usize = 64;
/// Maximum value length in ASCII bytes, excluding the guest NUL terminator.
pub const KEY_VALUE_MAX_VALUE_LEN: usize = 255;
pub const KEY_VALUE_GET: u8 = 0x00;
pub const KEY_VALUE_QUERY: u8 = 0x01;

impl ServiceInterruptManager {
    /// Set or replace a host-provided value, normalizing the key to uppercase.
    /// Keys must match `[A-Za-z][A-Za-z0-9_]*` and fit `KEY_VALUE_MAX_KEY_LEN`.
    /// Values may be empty, but must be ASCII without NUL and fit `KEY_VALUE_MAX_VALUE_LEN`.
    /// Invalid input returns `InvalidParameter` without changing the store.
    pub fn set_key_value(&mut self, key: &str, value: &str) -> Result<(), ServiceError> {
        let key = normalize_service_key(key)?;
        if value.len() > KEY_VALUE_MAX_VALUE_LEN || !value.is_ascii() || value.as_bytes().contains(&0) {
            return Err(ServiceError::InvalidParameter);
        }
        self.key_values.insert(key, value.to_owned());
        Ok(())
    }

    /// Query a key case-insensitively. Missing keys return `None`; malformed keys
    /// return `InvalidParameter`. An empty stored value is returned as `Some("")`.
    pub fn get_key_value(&self, key: &str) -> Result<Option<&str>, ServiceError> {
        let key = normalize_service_key(key)?;
        Ok(self.key_values.get(&key).map(String::as_str))
    }

    /// Delete a key case-insensitively, returning whether it existed.
    /// A malformed key returns `InvalidParameter` without changing the store.
    pub fn delete_key_value(&mut self, key: &str) -> Result<bool, ServiceError> {
        let key = normalize_service_key(key)?;
        Ok(self.key_values.remove(&key).is_some())
    }

    /// Retrieve a value (`AL=00h`) or query its presence (`AL=01h`). Retrieval
    /// permits truncation; a presence query never accesses an output buffer.
    pub(super) fn lookup_key_value<C: Cpu>(&self, cpu: &mut C) -> Result<(), ServiceError> {
        let capacity = match cpu.get_register8(Register8::AL) {
            KEY_VALUE_GET => {
                let capacity = usize::from(cpu.get_register16(Register16::CX));
                if capacity == 0 {
                    return Err(ServiceError::InvalidParameter);
                }
                Some(capacity)
            }
            KEY_VALUE_QUERY => None,
            _ => return Err(ServiceError::InvalidParameter),
        };
        let key_segment = cpu.get_register16(Register16::DS);
        let key_offset = cpu.get_register16(Register16::SI);
        let key = read_guest_key(cpu, key_segment, key_offset)?;
        let value = self.key_values.get(&key);
        if let Some(capacity) = capacity {
            let bytes = value.map_or(&[][..], |value| value.as_bytes());
            let copy_len = bytes.len().min(capacity - 1);
            let segment = cpu.get_register16(Register16::ES);
            let offset = cpu.get_register16(Register16::DI);
            let terminator_offset = offset
                .checked_add(copy_len as u16)
                .ok_or(ServiceError::InvalidParameter)?;

            // The entire key is already captured, so overlapping input/output is safe.
            // Write NUL before the value bytes so it is in place if a later write fails.
            write_guest_u8(cpu, segment, terminator_offset, 0)?;
            for (index, byte) in bytes[..copy_len].iter().copied().enumerate() {
                write_guest_u8(cpu, segment, offset + index as u16, byte)?;
            }
        }
        if value.is_none() {
            return Err(ServiceError::NotFound);
        }
        Ok(())
    }
}

fn normalize_service_key(key: &str) -> Result<String, ServiceError> {
    let bytes = key.as_bytes();
    if bytes.is_empty()
        || bytes.len() > KEY_VALUE_MAX_KEY_LEN
        || !bytes[0].is_ascii_alphabetic()
        || !bytes.iter().all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        return Err(ServiceError::InvalidParameter);
    }
    Ok(key.to_ascii_uppercase())
}

fn read_guest_key<C: Cpu>(cpu: &mut C, segment: u16, offset: u16) -> Result<String, ServiceError> {
    let mut bytes = [0; KEY_VALUE_MAX_KEY_LEN + 1];
    for index in 0..bytes.len() {
        let offset = offset.checked_add(index as u16).ok_or(ServiceError::InvalidParameter)?;
        bytes[index] = read_guest_u8(cpu, segment, offset)?;
        if bytes[index] == 0 {
            let key = std::str::from_utf8(&bytes[..index]).map_err(|_| ServiceError::InvalidParameter)?;
            return normalize_service_key(key);
        }
    }
    Err(ServiceError::InvalidParameter)
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
    use crate::cpu_common::ServiceEvent;

    #[test]
    fn keys_ignore_case_and_survive_reset() {
        let mut manager = ServiceInterruptManager::new(Some(0xF5), false);
        assert_eq!(manager.get_key_value("Game_Path"), Ok(None));
        manager.set_key_value("Game_Path", r"C:\Games\Demo").unwrap();
        assert_eq!(manager.get_key_value("game_path"), Ok(Some(r"C:\Games\Demo")));
        manager.set_key_value("GAME_PATH", "replacement").unwrap();
        manager.reset();
        assert!(!manager.enabled());
        assert_eq!(manager.get_key_value("gAmE_pAtH"), Ok(Some("replacement")));
        assert_eq!(manager.delete_key_value("game_PATH"), Ok(true));
        assert_eq!(manager.delete_key_value("GAME_PATH"), Ok(false));
        assert_eq!(manager.get_key_value("Game_Path"), Ok(None));
        manager.set_key_value("EMPTY", "").unwrap();
        assert_eq!(manager.get_key_value("empty"), Ok(Some("")));
    }

    #[test]
    fn invalid_entries_leave_store_unchanged() {
        let mut manager = ServiceInterruptManager::default();
        manager.set_key_value("Keep", "original").unwrap();
        for key in [
            "",
            "_NAME",
            "1NAME",
            "HAS SPACE",
            "A-B",
            "A.B",
            "A\0B",
            "é",
            "Aé",
            &"A".repeat(65),
        ] {
            assert_eq!(manager.set_key_value(key, "bad"), Err(ServiceError::InvalidParameter));
            assert_eq!(manager.get_key_value(key), Err(ServiceError::InvalidParameter));
            assert_eq!(manager.delete_key_value(key), Err(ServiceError::InvalidParameter));
        }
        for value in ["embedded\0nul", "café", &"V".repeat(256)] {
            assert_eq!(
                manager.set_key_value("KEEP", value),
                Err(ServiceError::InvalidParameter)
            );
        }
        assert_eq!(manager.get_key_value("keep"), Ok(Some("original")));
        manager.set_key_value("a_Z09", "ASCII\t\r\n").unwrap();
        assert_eq!(manager.get_key_value("A_z09"), Ok(Some("ASCII\t\r\n")));
    }

    fn prepare_key_value_lookup<C: Cpu>(cpu: &mut C, key: &[u8], capacity: u16) {
        write_guest_bytes(cpu, 0x1000, 0x0100, key);
        write_guest_bytes(cpu, 0x2000, 0x0100, &[0xCC; 258]);
        cpu.set_register16(Register16::AX, 0x1300);
        cpu.set_register16(Register16::DS, 0x1000);
        cpu.set_register16(Register16::SI, 0x0100);
        cpu.set_register16(Register16::ES, 0x2000);
        cpu.set_register16(Register16::DI, 0x0100);
        cpu.set_register16(Register16::CX, capacity);
        cpu.set_flags(0x0CD5); // DF and CF set: lookup ignores DF and clears CF on success.
    }

    fn check_key_value_result<C: Cpu>(manager: &mut ServiceInterruptManager, cpu: &mut C, error: Option<ServiceError>) {
        if error.is_some() {
            cpu.set_flags(cpu.get_flags() & !CARRY_FLAG);
        }
        let registers = [
            Register16::AX,
            Register16::BX,
            Register16::CX,
            Register16::DX,
            Register16::SI,
            Register16::DI,
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
        assert!(manager.handle_interrupt(ServiceFunction::KeyValueLookup, cpu).is_none());
        if error == Some(ServiceError::NotFound) {
            assert_eq!(cpu.get_register16(Register16::AX), 0x0490);
        }
        assert_eq!(cpu.get_cycle_ct(), cycles);
        assert_eq!(
            cpu.get_flags(),
            if error.is_some() {
                flags | CARRY_FLAG
            }
            else {
                flags & !CARRY_FLAG
            }
        );
        for (reg, value) in registers {
            let expected = if reg == Register16::AX {
                error.map_or(value, u16::from)
            }
            else {
                value
            };
            assert_eq!(cpu.get_register16(reg), expected, "{reg:?}");
        }
    }

    fn check_key_value_buffers<C: Cpu>(cpu: &mut C) {
        let mut manager = ServiceInterruptManager::default();
        manager.set_key_value("PATH", r"C:\DOS").unwrap();
        manager.set_key_value("EMPTY", "").unwrap();
        for (key, capacity, expected, error) in [
            (&b"pAtH\0"[..], 16, &b"C:\\DOS\0"[..], None),
            (&b"PATH\0"[..], 7, &b"C:\\DOS\0"[..], None),
            (&b"path\0"[..], 4, &b"C:\\\0"[..], None),
            (&b"PATH\0"[..], 1, &b"\0"[..], None),
            (&b"empty\0"[..], 1, &b"\0"[..], None),
            (&b"missing\0"[..], 16, &b"\0"[..], Some(ServiceError::NotFound)),
        ] {
            prepare_key_value_lookup(cpu, key, capacity);
            write_guest_u8(cpu, 0x2000, 0x00FF, 0xCC).unwrap();
            check_key_value_result(&mut manager, cpu, error);
            assert_eq!(read_guest_bytes(cpu, 0x2000, 0x0100, expected.len()), expected);
            assert_eq!(
                read_guest_bytes(cpu, 0x2000, 0x0100 + expected.len() as u16, 258 - expected.len()),
                vec![0xCC; 258 - expected.len()]
            );
            assert_eq!(read_guest_u8(cpu, 0x2000, 0x00FF), Ok(0xCC));
        }
    }

    #[test]
    fn lookup_respects_buffer_size() {
        check_key_value_buffers(&mut crate::cpu_808x::Intel808x::default());
        check_key_value_buffers(&mut crate::cpu_vx0::NecVx0::default());
    }

    #[test]
    fn lookup_accepts_max_length_entries() {
        let mut manager = ServiceInterruptManager::default();
        let mut cpu = crate::cpu_808x::Intel808x::default();
        let key = "k".repeat(64);
        let value = "v".repeat(255);
        manager.set_key_value(&key, &value).unwrap();
        assert_eq!(
            manager.get_key_value(&key.to_ascii_uppercase()),
            Ok(Some(value.as_str()))
        );
        prepare_key_value_lookup(&mut cpu, format!("{key}\0").as_bytes(), 256);
        check_key_value_result(&mut manager, &mut cpu, None);
        assert_eq!(
            read_guest_bytes(&mut cpu, 0x2000, 0x0100, 257),
            [value.as_bytes(), &[0, 0xCC]].concat()
        );
    }

    #[test]
    fn bad_lookups_leave_output_untouched() {
        let mut manager = ServiceInterruptManager::default();
        let mut cpu = crate::cpu_808x::Intel808x::default();
        let unterminated = [b'A'; 65];
        for key in [
            &b"\0"[..],
            &b"_NAME\0"[..],
            &b"9NAME\0"[..],
            &b"A-B\0"[..],
            &b"A\xFF\0"[..],
            &unterminated,
        ] {
            prepare_key_value_lookup(&mut cpu, key, 16);
            // Reading past the maximum scan length would return a different error.
            cpu.bus_mut().set_flags(0x10141, crate::bus::MEM_MMIO_BIT);
            check_key_value_result(&mut manager, &mut cpu, Some(ServiceError::InvalidParameter));
            assert_eq!(read_guest_bytes(&mut cpu, 0x2000, 0x0100, 258), [0xCC; 258]);
        }
        prepare_key_value_lookup(&mut cpu, b"KEY\0", 0);
        cpu.bus_mut().set_flags(0x10100, crate::bus::MEM_MMIO_BIT);
        check_key_value_result(&mut manager, &mut cpu, Some(ServiceError::InvalidParameter));
        assert_eq!(read_guest_bytes(&mut cpu, 0x2000, 0x0100, 258), [0xCC; 258]);
    }

    #[test]
    fn lookup_rejects_segment_overflow() {
        let mut manager = ServiceInterruptManager::default();
        let mut cpu = crate::cpu_808x::Intel808x::default();
        manager.set_key_value("K", "abc").unwrap();
        for (key_offset, output_offset, capacity, error) in [
            (0xFFFE, 0xFFFC, 0xFFFF, None), // Key and result terminate at FFFFh.
            (0xFFFE, 0xFFFF, 1, None),      // Only the NUL needs to fit.
            (0xFFFE, 0xFFFD, 4, Some(ServiceError::InvalidParameter)),
            (0xFFFF, 0x0100, 4, Some(ServiceError::InvalidParameter)),
        ] {
            prepare_key_value_lookup(&mut cpu, b"K\0", capacity);
            write_guest_bytes(&mut cpu, 0x1000, key_offset, b"K\0");
            write_guest_bytes(&mut cpu, 0x2000, output_offset, &[0xCC; 5]);
            cpu.set_register16(Register16::SI, key_offset);
            cpu.set_register16(Register16::DI, output_offset);
            check_key_value_result(&mut manager, &mut cpu, error);
            let expected: &[u8] = match (error, capacity) {
                (Some(_), _) => &[0xCC; 5],
                (None, 1) => &[0, 0xCC, 0xCC, 0xCC, 0xCC],
                _ => b"abc\0\xCC",
            };
            assert_eq!(read_guest_bytes(&mut cpu, 0x2000, output_offset, 5), expected);
        }
    }

    #[test]
    fn lookup_handles_overlap_and_wraparound() {
        let mut manager = ServiceInterruptManager::default();
        let mut cpu = crate::cpu_808x::Intel808x::default();
        manager.set_key_value("PATH", "abcdef").unwrap();
        for (key_segment, key_offset, output_segment, output_offset) in [
            (0x1000, 0x0100, 0x1010, 0x0000), // Same physical buffer via segment alias.
            (0x1000, 0x0100, 0x1000, 0x0101), // Output overwrites unread key bytes if read lazily.
            (0xFFFF, 0x000E, 0x1000, 0x0100), // Key wraps at 1 MB.
            (0x1000, 0x0100, 0xFFFF, 0x000E), // Output wraps at 1 MB.
        ] {
            prepare_key_value_lookup(&mut cpu, b"PATH\0", 7);
            write_guest_bytes(&mut cpu, key_segment, key_offset, b"PATH\0");
            cpu.set_register16(Register16::DS, key_segment);
            cpu.set_register16(Register16::SI, key_offset);
            cpu.set_register16(Register16::ES, output_segment);
            cpu.set_register16(Register16::DI, output_offset);
            check_key_value_result(&mut manager, &mut cpu, None);
            assert_eq!(
                read_guest_bytes(&mut cpu, output_segment, output_offset, 7),
                b"abcdef\0"
            );
        }
    }

    #[test]
    fn failed_lookup_preserves_output() {
        let mut manager = ServiceInterruptManager::default();
        let mut cpu = crate::cpu_808x::Intel808x::default();
        prepare_key_value_lookup(&mut cpu, b"KEY\0", 16);
        cpu.bus_mut().set_flags(0x10101, crate::bus::MEM_MMIO_BIT);
        check_key_value_result(&mut manager, &mut cpu, Some(ServiceError::InvalidData));
        assert_eq!(read_guest_bytes(&mut cpu, 0x2000, 0x0100, 258), [0xCC; 258]);
    }

    #[test]
    fn lookup_write_failure_preserves_output() {
        let mut cpu = crate::cpu_808x::Intel808x::default();
        let mut manager = ServiceInterruptManager::default();
        manager.set_key_value("KEY", "abc").unwrap();
        write_guest_bytes(&mut cpu, 0x1000, 0, b"KEY\0");
        write_guest_bytes(&mut cpu, 0x2000, 0, b"--");
        // The destination prefix exists but its terminator address does not.
        crate::bus::test_support::truncate_memory(cpu.bus_mut(), 0x20002);
        cpu.set_register16(Register16::AX, 0x1300);
        cpu.set_register16(Register16::DS, 0x1000);
        cpu.set_register16(Register16::SI, 0);
        cpu.set_register16(Register16::ES, 0x2000);
        cpu.set_register16(Register16::DI, 0);
        cpu.set_register16(Register16::CX, 4);
        cpu.set_flags(0);

        manager.handle_interrupt(ServiceFunction::KeyValueLookup, &mut cpu);

        assert_eq!(cpu.get_flags() & CARRY_FLAG, 1);
        assert_eq!(cpu.get_register16(Register16::AX), u16::from(ServiceError::InvalidData));
        assert_eq!(cpu.get_register16(Register16::SI), 0);
        assert_eq!(cpu.get_register16(Register16::DI), 0);
        assert_eq!(cpu.get_register16(Register16::CX), 4);
        // NUL is written first; its failure leaves the existing prefix intact.
        assert_eq!(cpu.bus_mut().get_slice_at(0x20000, 2), b"--");
    }

    fn check_key_value_presence<C: Cpu>(cpu: &mut C) {
        let mut manager = ServiceInterruptManager::default();
        manager.set_key_value("PATH", r"C:\DOS").unwrap();
        manager.set_key_value("EMPTY", "").unwrap();
        for (key, read_failure, error) in [
            (&b"PATH\0"[..], false, None),
            (&b"pAtH\0"[..], false, None),
            (&b"empty\0"[..], false, None),
            (&b"missing\0"[..], false, Some(ServiceError::NotFound)),
            (&b"_BAD\0"[..], false, Some(ServiceError::InvalidParameter)),
            (&b"KEY\0"[..], true, Some(ServiceError::InvalidData)),
        ] {
            for (offset, capacity) in [(0x0100, 0), (0xFFFF, 0xFFFF), (0x0100, 16)] {
                prepare_key_value_lookup(cpu, key, capacity);
                cpu.set_register8(Register8::AL, KEY_VALUE_QUERY);
                cpu.set_register16(Register16::DI, offset);
                if read_failure {
                    cpu.bus_mut().set_flags(0x10100, crate::bus::MEM_MMIO_BIT);
                }
                // A destination read would fail, and a retrieval would cross the segment.
                cpu.bus_mut().set_flags(0x2FFFF, crate::bus::MEM_MMIO_BIT);
                let before = cpu.bus_mut().get_vec_at(0, 0x100000);

                check_key_value_result(&mut manager, cpu, error);

                assert_eq!(cpu.bus_mut().get_slice_at(0, 0x100000), before);
                cpu.bus_mut().clear_flags(0x10100, crate::bus::MEM_MMIO_BIT);
            }
        }
    }

    #[test]
    fn presence_checks_leave_memory_untouched() {
        check_key_value_presence(&mut crate::cpu_808x::Intel808x::default());
        check_key_value_presence(&mut crate::cpu_vx0::NecVx0::default());
    }

    #[test]
    fn invalid_lookup_skips_memory_access() {
        let mut manager = ServiceInterruptManager::default();
        let mut cpu = crate::cpu_808x::Intel808x::default();
        for subfunction in [2, 0xAB, 0xFF] {
            prepare_key_value_lookup(&mut cpu, b"KEY\0", 16);
            cpu.set_register8(Register8::AL, subfunction);
            cpu.bus_mut().set_flags(0x10100, crate::bus::MEM_MMIO_BIT);
            let before = cpu.bus_mut().get_vec_at(0, 0x100000);

            check_key_value_result(&mut manager, &mut cpu, Some(ServiceError::InvalidParameter));

            assert_eq!(cpu.bus_mut().get_slice_at(0, 0x100000), before);
        }
    }

    fn check_key_value_dispatch<C: Cpu>(cpu: &mut C, interrupt: impl Fn(&mut C, u8)) {
        use crate::cpu_common::CpuOption;

        let mut manager = ServiceInterruptManager::new(Some(0xFC), true);
        manager.set_key_value("KEY", "value").unwrap();
        cpu.set_option(CpuOption::ServiceInterruptVector(Some(0xFC)));
        cpu.set_option(CpuOption::ServiceInterruptEnabled(true));
        prepare_key_value_lookup(cpu, b"key\0", 6);
        interrupt(cpu, 0xFC);
        assert!(matches!(
            cpu.get_service_event(),
            Some(ServiceEvent::ServiceInterrupt(0x13))
        ));
        check_key_value_result(&mut manager, cpu, None);
        assert_eq!(read_guest_bytes(cpu, 0x2000, 0x0100, 6), b"value\0");

        cpu.set_register8(Register8::AH, ServiceFunction::ServiceControl.into());
        cpu.set_register8(Register8::AL, ServiceControl::Disable.into());
        cpu.set_register16(Register16::BX, SERVICE_CTRL_BX);
        cpu.set_register16(Register16::CX, SERVICE_CTRL_CX);
        manager.handle_interrupt(ServiceFunction::ServiceControl, cpu);
        prepare_key_value_lookup(cpu, b"KEY\0", 6);
        let flags = cpu.get_flags();
        assert!(manager.handle_interrupt(ServiceFunction::KeyValueLookup, cpu).is_none());
        assert_eq!(cpu.get_flags(), flags);
        assert_eq!(cpu.get_register16(Register16::AX), 0x1300);
        assert_eq!(read_guest_bytes(cpu, 0x2000, 0x0100, 6), [0xCC; 6]);
        assert_eq!(manager.get_key_value("key"), Ok(Some("value")));
    }

    #[test]
    fn lookup_uses_configured_interrupt() {
        check_key_value_dispatch(&mut crate::cpu_808x::Intel808x::default(), |cpu, vector| {
            cpu.sw_interrupt(vector)
        });
        check_key_value_dispatch(&mut crate::cpu_vx0::NecVx0::default(), |cpu, vector| {
            cpu.sw_interrupt(vector)
        });
    }
}

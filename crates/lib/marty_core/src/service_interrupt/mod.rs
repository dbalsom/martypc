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

//! MartyPC internal emulator service interrupt handling.

mod fast_copy;
mod file_transfer;
mod kv_store;
mod mouse;
mod speed_control;

#[cfg(test)]
mod test_support;

pub use file_transfer::{
    FileTransferDirection,
    FileTransferHandle,
    FileTransferOperation,
    FileTransferStatus,
    FILE_TRANSFER_ABORT,
    FILE_TRANSFER_COMMIT,
    FILE_TRANSFER_DIRECTION_MASK,
    FILE_TRANSFER_FLAG_MASK,
    FILE_TRANSFER_GUEST_TO_HOST,
    FILE_TRANSFER_HOST_TO_GUEST,
    FILE_TRANSFER_NON_INTERACTIVE,
    FILE_TRANSFER_STRUCTURE_SIZE,
};
pub use kv_store::{KEY_VALUE_GET, KEY_VALUE_MAX_KEY_LEN, KEY_VALUE_MAX_VALUE_LEN, KEY_VALUE_QUERY};
pub use mouse::{
    MOUSE_CONSUMER_RANGE_REPORT,
    MOUSE_CONSUMER_STATUS_REPORT,
    MOUSE_DISPLAY_APERTURE_QUERY,
    MOUSE_HOST_CURSOR_VISIBILITY,
    MOUSE_IRQ_QUERY,
    MOUSE_STATE_FLAG_CAPTURED,
    MOUSE_STATE_QUERY,
};
pub use speed_control::{
    DEFAULT_SPEED_CONTROL_CURRENT,
    DEFAULT_SPEED_CONTROL_MAX,
    DEFAULT_SPEED_CONTROL_MIN,
    SPEED_CONTROL_QUERY,
    SPEED_CONTROL_SET,
};

use file_transfer::{HostFileRequestState, INITIAL_FILE_TRANSFER_HANDLE};

use marty_common::MartyHashMap;

use crate::cpu_common::{Cpu, Register16, Register8, ServiceEvent};

pub const MARTYPC_PROBE_INTERRUPT: u8 = 0x2F;
pub const MARTYPC_PROBE_AX: u16 = 0xF500;
pub const MARTYPC_PROBE_BX: u16 = 0xDEAD;
pub const MARTYPC_PROBE_CX: u16 = 0xBEEF;

pub const MARTYPC_PROBE_RESPONSE_AX: u16 = 0xF5FF;
pub const MARTYPC_PROBE_RESPONSE_BX: u16 = 0x4D50; // "MP"
pub const MARTYPC_API_VERSION: u16 = 0x0100;
pub const MARTYPC_VERSION: u16 = (parse_version_byte(env!("CARGO_PKG_VERSION_MAJOR")) as u16) << 8
    | parse_version_byte(env!("CARGO_PKG_VERSION_MINOR")) as u16;

pub const SERVICE_FLAG_INTERRUPT_ENABLED: u8 = 0x01;
pub const SERVICE_FLAG_INTERRUPT_AVAILABLE: u8 = 0x02;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ServiceFunction {
    ServiceControl = 0x00,
    Debugger = 0x01,
    PitLogging = 0x02,
    Quit = 0x03,
    FileTransferBegin = 0x04,
    FileTransferBlock = 0x05,
    FileTransferEnd = 0x06,
    SpeedControl = 0x10,
    MouseState = 0x11,
    FastCopy = 0x12,
    KeyValueLookup = 0x13,
}

impl TryFrom<u8> for ServiceFunction {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::ServiceControl),
            0x01 => Ok(Self::Debugger),
            0x02 => Ok(Self::PitLogging),
            0x03 => Ok(Self::Quit),
            0x04 => Ok(Self::FileTransferBegin),
            0x05 => Ok(Self::FileTransferBlock),
            0x06 => Ok(Self::FileTransferEnd),
            0x10 => Ok(Self::SpeedControl),
            0x11 => Ok(Self::MouseState),
            0x12 => Ok(Self::FastCopy),
            0x13 => Ok(Self::KeyValueLookup),
            _ => Err(value),
        }
    }
}

impl From<ServiceFunction> for u8 {
    fn from(function: ServiceFunction) -> Self {
        function as u8
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum ServiceError {
    InvalidFunction = 0x0001,
    FileNotFound = 0x0002,
    TooManyOpenFiles = 0x0004,
    InvalidHandle = 0x0006,
    NotEnoughMemory = 0x0008,
    InvalidAccess = 0x000C,
    InvalidData = 0x000D,
    NotSupported = 0x0032,
    InvalidParameter = 0x0057,
    Busy = 0x00AA,
    NotFound = 0x0490,
}

impl From<ServiceError> for u16 {
    fn from(error: ServiceError) -> Self {
        error as u16
    }
}

pub const SERVICE_CTRL_BX: u16 = 0xDEAD;
pub const SERVICE_CTRL_CX: u16 = 0xBEEF;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ServiceControl {
    Disable = 0x00,
    Enable = 0x01,
    Query = 0x02,
}

impl TryFrom<u8> for ServiceControl {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::Disable),
            0x01 => Ok(Self::Enable),
            0x02 => Ok(Self::Query),
            _ => Err(value),
        }
    }
}

impl From<bool> for ServiceControl {
    fn from(value: bool) -> Self {
        match value {
            true => Self::Enable,
            false => Self::Disable,
        }
    }
}

impl From<ServiceControl> for u8 {
    fn from(control: ServiceControl) -> Self {
        control as u8
    }
}

/// Parse a single cargo version string into a u8 byte
const fn parse_version_byte(value: &str) -> u8 {
    let bytes = value.as_bytes();
    let mut parsed = 0u16;
    let mut index = 0;

    assert!(!bytes.is_empty(), "version component must not be empty");

    while index < bytes.len() {
        let digit = bytes[index];
        assert!(digit >= b'0' && digit <= b'9', "version component must be numeric");

        parsed = parsed * 10 + (digit - b'0') as u16;
        assert!(parsed <= u8::MAX as u16, "version component exceeds 8 bits");
        index += 1;
    }

    parsed as u8
}

const CARRY_FLAG: u16 = 0x0001;

#[derive(Debug)]
pub struct ServiceInterruptManager {
    service_interrupt_vector: Option<u8>,
    initial_enabled: bool,
    enabled: bool,
    key_values: MartyHashMap<String, String>,
    next_file_transfer_handle: FileTransferHandle,
    free_file_transfer_handles: Vec<FileTransferHandle>,
    file_transfer_operations: MartyHashMap<FileTransferHandle, FileTransferOperation>,
    host_file_request: HostFileRequestState,
    speed_control_min: u16,
    speed_control_current: u16,
    speed_control_max: u16,
}

impl Default for ServiceInterruptManager {
    fn default() -> Self {
        Self {
            service_interrupt_vector: None,
            initial_enabled: true,
            enabled: true,
            key_values: MartyHashMap::default(),
            next_file_transfer_handle: INITIAL_FILE_TRANSFER_HANDLE,
            free_file_transfer_handles: Vec::new(),
            file_transfer_operations: MartyHashMap::default(),
            host_file_request: HostFileRequestState::Idle,
            speed_control_min: DEFAULT_SPEED_CONTROL_MIN,
            speed_control_current: DEFAULT_SPEED_CONTROL_CURRENT,
            speed_control_max: DEFAULT_SPEED_CONTROL_MAX,
        }
    }
}

impl ServiceInterruptManager {
    pub fn new(service_interrupt_vector: Option<u8>, enabled: bool) -> Self {
        Self {
            service_interrupt_vector,
            initial_enabled: enabled,
            enabled,
            ..Self::default()
        }
    }

    pub fn reset(&mut self) {
        // Host-provided key/value entries survive guest and machine resets.
        self.enabled = self.initial_enabled;
        self.next_file_transfer_handle = INITIAL_FILE_TRANSFER_HANDLE;
        self.free_file_transfer_handles.clear();
        self.file_transfer_operations.clear();
        self.host_file_request = HostFileRequestState::Idle;
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Handle a service interrupt function that does not require CPU-specific execution logic.
    pub fn handle_interrupt<C: Cpu>(&mut self, function: ServiceFunction, cpu: &mut C) -> Option<ServiceEvent> {
        // Handle service control - enabling or disabling the emulator service API
        if function == ServiceFunction::ServiceControl {
            // Check the magic values to ensure this actually a call to Marty
            if !has_service_control_magic(cpu) {
                return None;
            }

            match ServiceControl::try_from(cpu.get_register8(Register8::AL)).ok()? {
                ServiceControl::Disable => self.enabled = false,
                ServiceControl::Enable => self.enabled = true,
                ServiceControl::Query => {}
            }

            // Return enabled state in AL
            cpu.set_register8(Register8::AL, ServiceControl::from(self.enabled).into());

            // Always succeeds
            clear_carry(cpu);

            return Some(ServiceEvent::ServiceInterruptEnabled(self.enabled));
        }

        if !self.enabled {
            return None;
        }

        // Dispatch to appropriate function.
        match function {
            ServiceFunction::PitLogging => Some(ServiceEvent::TriggerPITLogging),
            ServiceFunction::Quit => Some(ServiceEvent::QuitEmulator(cpu.get_register8(Register8::AL))),
            ServiceFunction::SpeedControl => self.handle_speed_control(cpu),
            ServiceFunction::MouseState => self.handle_mouse_state(cpu),
            ServiceFunction::FastCopy => {
                self.fast_copy(cpu);
                None
            }
            ServiceFunction::KeyValueLookup => {
                if let Err(error) = self.lookup_key_value(cpu) {
                    set_service_error(cpu, error);
                }
                else {
                    clear_carry(cpu);
                }
                None
            }
            ServiceFunction::FileTransferBegin => self.begin_file_transfer(cpu),
            ServiceFunction::FileTransferBlock => {
                self.transfer_file_block(cpu);
                None
            }
            ServiceFunction::FileTransferEnd => self.end_file_transfer(cpu),
            ServiceFunction::ServiceControl | ServiceFunction::Debugger => None,
        }
    }

    pub fn handle_probe<C: Cpu>(&self, cpu: &mut C) {
        let service_flags = self.service_interrupt_vector.map_or(0, |_| {
            SERVICE_FLAG_INTERRUPT_AVAILABLE
                | if self.enabled {
                    SERVICE_FLAG_INTERRUPT_ENABLED
                }
                else {
                    0
                }
        });
        let service_vector = self.service_interrupt_vector.unwrap_or(0);

        cpu.set_register16(Register16::AX, MARTYPC_PROBE_RESPONSE_AX);
        cpu.set_register16(Register16::BX, MARTYPC_PROBE_RESPONSE_BX);
        cpu.set_register16(Register16::CX, MARTYPC_VERSION);
        cpu.set_register16(
            Register16::DX,
            u16::from(service_flags) << 8 | u16::from(service_vector),
        );
        cpu.set_register16(Register16::SI, MARTYPC_API_VERSION);
    }
}

fn read_guest_u8<C: Cpu>(cpu: &mut C, segment: u16, offset: u16) -> Result<u8, ServiceError> {
    let address = crate::cpu_common::calc_linear_address(segment, offset) as usize;
    cpu.bus_mut()
        .read_u8(address, 0)
        .map(|(value, _)| value)
        .map_err(|_| ServiceError::InvalidData)
}

fn write_guest_u8<C: Cpu>(cpu: &mut C, segment: u16, offset: u16, value: u8) -> Result<(), ServiceError> {
    let address = crate::cpu_common::calc_linear_address(segment, offset) as usize;
    cpu.bus_mut()
        .write_u8(address, value, 0)
        .map(|_| ())
        .map_err(|_| ServiceError::InvalidData)
}

fn clear_carry<C: Cpu>(cpu: &mut C) {
    cpu.set_flags(cpu.get_flags() & !CARRY_FLAG);
}

fn set_service_error<C: Cpu>(cpu: &mut C, error: ServiceError) {
    cpu.set_register16(Register16::AX, error.into());
    cpu.set_flags(cpu.get_flags() | CARRY_FLAG);
}

pub fn has_martypc_probe_magic<C: Cpu>(interrupt: u8, cpu: &C) -> bool {
    interrupt == MARTYPC_PROBE_INTERRUPT
        && cpu.get_register16(Register16::AX) == MARTYPC_PROBE_AX
        && cpu.get_register16(Register16::BX) == MARTYPC_PROBE_BX
        && cpu.get_register16(Register16::CX) == MARTYPC_PROBE_CX
}

pub fn has_service_control_magic<C: Cpu>(cpu: &C) -> bool {
    cpu.get_register8(Register8::AH) == u8::from(ServiceFunction::ServiceControl)
        && cpu.get_register16(Register16::BX) == SERVICE_CTRL_BX
        && cpu.get_register16(Register16::CX) == SERVICE_CTRL_CX
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_restores_initial_state() {
        let mut manager = ServiceInterruptManager::new(None, false);
        let mut cpu = crate::cpu_808x::Intel808x::default();
        let handle = manager
            .create_file_transfer_operation("transfer.bin", 4096, FileTransferDirection::GuestToHost)
            .unwrap();

        cpu.set_register8(Register8::AH, ServiceFunction::ServiceControl.into());
        cpu.set_register8(Register8::AL, ServiceControl::Enable.into());
        cpu.set_register16(Register16::BX, SERVICE_CTRL_BX);
        cpu.set_register16(Register16::CX, SERVICE_CTRL_CX);
        manager.handle_interrupt(ServiceFunction::ServiceControl, &mut cpu);
        assert!(manager.enabled());

        manager.reset();

        assert!(!manager.enabled());
        assert_eq!(manager.file_transfer_operation(handle), None);
        assert!(manager
            .create_file_transfer_operation("new.bin", 128, FileTransferDirection::GuestToHost)
            .is_some());
    }

    #[test]
    fn service_can_be_disabled_and_reenabled() {
        use crate::cpu_808x::Intel808x;

        let mut manager = ServiceInterruptManager::new(None, true);
        let mut cpu = Intel808x::default();
        cpu.set_register8(Register8::AL, 7);

        assert!(manager.enabled());
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::PitLogging, &mut cpu),
            Some(ServiceEvent::TriggerPITLogging)
        ));

        cpu.set_register8(Register8::AH, ServiceFunction::ServiceControl.into());
        cpu.set_register16(Register16::BX, SERVICE_CTRL_BX);
        cpu.set_register16(Register16::CX, SERVICE_CTRL_CX);

        cpu.set_register8(Register8::AL, ServiceControl::Query.into());
        cpu.set_flags(cpu.get_flags() | CARRY_FLAG);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::ServiceControl, &mut cpu),
            Some(ServiceEvent::ServiceInterruptEnabled(true))
        ));
        assert_eq!(cpu.get_register8(Register8::AL), u8::from(ServiceControl::Enable));
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);

        cpu.set_register8(Register8::AL, ServiceControl::Disable.into());
        assert!(has_service_control_magic(&cpu));
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::ServiceControl, &mut cpu),
            Some(ServiceEvent::ServiceInterruptEnabled(false))
        ));
        assert!(!manager.enabled());
        assert_eq!(cpu.get_register8(Register8::AL), u8::from(ServiceControl::Disable));

        cpu.set_register8(Register8::AL, ServiceControl::Query.into());
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::ServiceControl, &mut cpu),
            Some(ServiceEvent::ServiceInterruptEnabled(false))
        ));
        assert_eq!(cpu.get_register8(Register8::AL), u8::from(ServiceControl::Disable));
        assert!(manager
            .handle_interrupt(ServiceFunction::PitLogging, &mut cpu)
            .is_none());

        cpu.set_register8(Register8::AL, ServiceControl::Enable.into());
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::ServiceControl, &mut cpu),
            Some(ServiceEvent::ServiceInterruptEnabled(true))
        ));
        assert!(manager.enabled());
        assert_eq!(cpu.get_register8(Register8::AL), u8::from(ServiceControl::Enable));

        cpu.set_register8(Register8::AL, 7);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::Quit, &mut cpu),
            Some(ServiceEvent::QuitEmulator(7))
        ));
    }

    #[test]
    fn bad_sentinels_cannot_enable_service() {
        use crate::cpu_808x::Intel808x;

        let mut manager = ServiceInterruptManager::new(None, false);
        let mut cpu = Intel808x::default();
        cpu.set_register8(Register8::AH, ServiceFunction::ServiceControl.into());
        cpu.set_register8(Register8::AL, ServiceControl::Enable.into());
        cpu.set_register16(Register16::BX, SERVICE_CTRL_BX);
        cpu.set_register16(Register16::CX, SERVICE_CTRL_CX ^ 1);

        assert!(!has_service_control_magic(&cpu));
        assert!(manager
            .handle_interrupt(ServiceFunction::ServiceControl, &mut cpu)
            .is_none());
        assert!(!manager.enabled());
    }

    #[test]
    fn probe_reports_version_and_service_state() {
        use crate::cpu_808x::Intel808x;

        let mut manager = ServiceInterruptManager::new(Some(0xFC), true);
        let mut cpu = Intel808x::default();
        cpu.set_register16(Register16::AX, MARTYPC_PROBE_AX);
        cpu.set_register16(Register16::BX, MARTYPC_PROBE_BX);
        cpu.set_register16(Register16::CX, MARTYPC_PROBE_CX);

        assert!(has_martypc_probe_magic(MARTYPC_PROBE_INTERRUPT, &cpu));
        assert!(!has_martypc_probe_magic(MARTYPC_PROBE_INTERRUPT - 1, &cpu));

        manager.handle_probe(&mut cpu);

        assert_eq!(cpu.get_register16(Register16::AX), MARTYPC_PROBE_RESPONSE_AX);
        assert_eq!(cpu.get_register16(Register16::BX), MARTYPC_PROBE_RESPONSE_BX);
        assert_eq!(cpu.get_register16(Register16::CX), MARTYPC_VERSION);
        assert_eq!(cpu.get_register8(Register8::DL), 0xFC);
        assert_eq!(
            cpu.get_register8(Register8::DH),
            SERVICE_FLAG_INTERRUPT_AVAILABLE | SERVICE_FLAG_INTERRUPT_ENABLED
        );
        assert_eq!(cpu.get_register16(Register16::SI), MARTYPC_API_VERSION);

        cpu.set_register8(Register8::AH, ServiceFunction::ServiceControl.into());
        cpu.set_register8(Register8::AL, ServiceControl::Disable.into());
        cpu.set_register16(Register16::BX, SERVICE_CTRL_BX);
        cpu.set_register16(Register16::CX, SERVICE_CTRL_CX);
        manager.handle_interrupt(ServiceFunction::ServiceControl, &mut cpu);
        manager.handle_probe(&mut cpu);

        assert_eq!(cpu.get_register8(Register8::DH), SERVICE_FLAG_INTERRUPT_AVAILABLE);
    }
}

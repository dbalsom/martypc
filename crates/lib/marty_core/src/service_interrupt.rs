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

    service_interrupt.rs

    MartyPC internal emulator service interrupt handling.
*/

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
            _ => Err(value),
        }
    }
}

impl From<ServiceFunction> for u8 {
    fn from(function: ServiceFunction) -> Self {
        function as u8
    }
}

pub const SPEED_CONTROL_QUERY: u8 = 0x00;
pub const SPEED_CONTROL_SET: u8 = 0x01;
pub const DEFAULT_SPEED_CONTROL_MIN: u16 = 100;
pub const DEFAULT_SPEED_CONTROL_CURRENT: u16 = 1000;
pub const DEFAULT_SPEED_CONTROL_MAX: u16 = 2000;
pub const MOUSE_STATE_QUERY: u8 = 0x00;
pub const MOUSE_IRQ_QUERY: u8 = 0x01;
pub const MOUSE_CONSUMER_RANGE_REPORT: u8 = 0x02;
pub const MOUSE_CONSUMER_STATUS_REPORT: u8 = 0x03;
pub const MOUSE_DISPLAY_APERTURE_QUERY: u8 = 0x04;
pub const MOUSE_HOST_CURSOR_VISIBILITY: u8 = 0x05;
pub const MOUSE_STATE_FLAG_CAPTURED: u16 = 0x0001;

pub const FILE_TRANSFER_GUEST_TO_HOST: u8 = 0x00;
pub const FILE_TRANSFER_HOST_TO_GUEST: u8 = 0x01;
/// `AH=04h` bit 0 selects the transfer direction.
pub const FILE_TRANSFER_DIRECTION_MASK: u8 = 0x01;
/// `AH=04h` bit 1 resolves the filename through the `file_transfer` resource.
pub const FILE_TRANSFER_NON_INTERACTIVE: u8 = 0x02;
pub const FILE_TRANSFER_FLAG_MASK: u8 = FILE_TRANSFER_DIRECTION_MASK | FILE_TRANSFER_NON_INTERACTIVE;
pub const FILE_TRANSFER_COMMIT: u8 = 0x00;
pub const FILE_TRANSFER_ABORT: u8 = 0x01;
pub const FILE_TRANSFER_STRUCTURE_SIZE: u16 = 10;
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum FileTransferStatus {
    Wait = 0x0000,
    Ready = 0x0001,
    Aborted = 0x0002,
    HostFileNotFound = 0x0003,
}

impl From<FileTransferStatus> for u16 {
    fn from(status: FileTransferStatus) -> Self {
        status as u16
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

pub type FileTransferHandle = u16;

const CARRY_FLAG: u16 = 0x0001;
const CRC32_INITIAL: u32 = u32::MAX;
const CRC32_POLYNOMIAL: u32 = 0xEDB8_8320;
const MAX_TRANSFER_FILENAME_LEN: usize = 255;
const INITIAL_FILE_TRANSFER_HANDLE: FileTransferHandle = 0x1000;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FileTransferDirection {
    GuestToHost,
    HostToGuest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileTransferOperation {
    filename: String,
    size: u64,
    direction: FileTransferDirection,
    data: Vec<u8>,
    transferred: usize,
    ready: bool,
    crc32: u32,
    non_interactive: bool,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct PendingHostFileRequest {
    handle: FileTransferHandle,
    structure_segment: u16,
    structure_offset: u16,
    filename_segment: u16,
    filename_offset: u16,
}

#[derive(Debug, Default)]
enum HostFileRequestState {
    #[default]
    Idle,
    Pending(PendingHostFileRequest),
}

impl FileTransferOperation {
    pub fn filename(&self) -> &str {
        &self.filename
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn direction(&self) -> FileTransferDirection {
        self.direction
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[derive(Debug)]
pub struct ServiceInterruptManager {
    service_interrupt_vector: Option<u8>,
    initial_enabled: bool,
    enabled: bool,
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
        if function == ServiceFunction::ServiceControl {
            if !is_service_control(cpu) {
                return None;
            }

            let Ok(control) = ServiceControl::try_from(cpu.get_register8(Register8::AL))
            else {
                return None;
            };
            match control {
                ServiceControl::Disable => self.enabled = false,
                ServiceControl::Enable => self.enabled = true,
                ServiceControl::Query => {}
            }

            let control = if self.enabled {
                ServiceControl::Enable
            }
            else {
                ServiceControl::Disable
            };
            cpu.set_register8(Register8::AL, control.into());
            clear_carry(cpu);

            return Some(ServiceEvent::ServiceInterruptEnabled(self.enabled));
        }

        if !self.enabled {
            return None;
        }

        match function {
            ServiceFunction::PitLogging => Some(ServiceEvent::TriggerPITLogging),
            ServiceFunction::Quit => Some(ServiceEvent::QuitEmulator(cpu.get_register8(Register8::AL))),
            ServiceFunction::SpeedControl => self.handle_speed_control(cpu),
            ServiceFunction::MouseState => self.handle_mouse_state(cpu),
            ServiceFunction::FileTransferBegin => self.begin_file_transfer(cpu),
            ServiceFunction::FileTransferBlock => {
                self.transfer_file_block(cpu);
                None
            }
            ServiceFunction::FileTransferEnd => self.end_file_transfer(cpu),
            ServiceFunction::ServiceControl | ServiceFunction::Debugger => None,
        }
    }

    fn handle_speed_control<C: Cpu>(&mut self, cpu: &mut C) -> Option<ServiceEvent> {
        match cpu.get_register8(Register8::AL) {
            SPEED_CONTROL_QUERY => {
                cpu.set_register16(Register16::BX, self.speed_control_min);
                cpu.set_register16(Register16::CX, self.speed_control_current);
                cpu.set_register16(Register16::DX, self.speed_control_max);
                clear_carry(cpu);
                None
            }
            SPEED_CONTROL_SET => {
                let requested_speed = cpu.get_register16(Register16::CX);
                let speed = requested_speed.clamp(self.speed_control_min, self.speed_control_max);
                self.speed_control_current = speed;
                clear_carry(cpu);
                Some(ServiceEvent::SetEmulationSpeed(speed))
            }
            _ => {
                set_service_error(cpu, ServiceError::InvalidParameter);
                None
            }
        }
    }

    pub fn configure_speed_control(&mut self, min: u16, current: u16, max: u16) {
        let max = max.max(min);
        self.speed_control_min = min;
        self.speed_control_max = max;
        self.speed_control_current = current.clamp(min, max);
    }

    pub fn set_speed_control_current(&mut self, current: u16) {
        self.speed_control_current = current.clamp(self.speed_control_min, self.speed_control_max);
    }

    /// Complete a virtual mouse state request after the machine has sampled the device.
    pub fn complete_mouse_state<C: Cpu>(
        &self,
        cpu: &mut C,
        state: Option<(u16, u16, u16, u16, i16, i16, u16)>,
    ) {
        let Some((x, y, buttons, change_counter, relative_x, relative_y, flags)) = state
        else {
            set_service_error(cpu, ServiceError::NotSupported);
            return;
        };

        cpu.set_register16(Register16::AX, x);
        cpu.set_register16(Register16::BX, y);
        cpu.set_register16(Register16::CX, buttons);
        cpu.set_register16(Register16::DX, change_counter);
        cpu.set_register16(Register16::SI, relative_x as u16);
        cpu.set_register16(Register16::DI, relative_y as u16);
        cpu.set_register16(Register16::BP, flags);
        clear_carry(cpu);
    }

    /// Complete a virtual mouse IRQ query after the machine has inspected the device.
    pub fn complete_mouse_irq<C: Cpu>(&self, cpu: &mut C, irq: Option<u8>) {
        let Some(irq) = irq
        else {
            set_service_error(cpu, ServiceError::NotSupported);
            return;
        };

        cpu.set_register16(Register16::DX, u16::from(irq));
        clear_carry(cpu);
    }

    /// Complete a display-aperture query after the machine has inspected its primary video card.
    pub fn complete_display_aperture_size<C: Cpu>(&self, cpu: &mut C, size: Option<(u16, u16)>) {
        let Some((width, height)) = size
        else {
            set_service_error(cpu, ServiceError::NotSupported);
            return;
        };

        cpu.set_register16(Register16::BX, width);
        cpu.set_register16(Register16::CX, height);
        clear_carry(cpu);
    }

    /// Complete a virtual mouse consumer-range report after the machine has inspected the device.
    pub fn complete_mouse_consumer_range<C: Cpu>(&self, cpu: &mut C, supported: bool) {
        if supported {
            clear_carry(cpu);
        }
        else {
            set_service_error(cpu, ServiceError::NotSupported);
        }
    }

    /// Complete a virtual mouse consumer-status report after the machine has inspected the device.
    pub fn complete_mouse_consumer_status<C: Cpu>(&self, cpu: &mut C, supported: bool) {
        if supported {
            clear_carry(cpu);
        }
        else {
            set_service_error(cpu, ServiceError::NotSupported);
        }
    }

    fn handle_mouse_state<C: Cpu>(&self, cpu: &mut C) -> Option<ServiceEvent> {
        match cpu.get_register8(Register8::AL) {
            MOUSE_STATE_QUERY => Some(ServiceEvent::GetVirtualMouseState),
            MOUSE_IRQ_QUERY => Some(ServiceEvent::GetVirtualMouseIrq),
            MOUSE_DISPLAY_APERTURE_QUERY => Some(ServiceEvent::GetDisplayApertureSize),
            MOUSE_CONSUMER_RANGE_REPORT => Some(ServiceEvent::SetVirtualMouseConsumerRange {
                min_x: cpu.get_register16(Register16::BX),
                max_x: cpu.get_register16(Register16::CX),
                min_y: cpu.get_register16(Register16::DX),
                max_y: cpu.get_register16(Register16::SI),
            }),
            MOUSE_CONSUMER_STATUS_REPORT => match cpu.get_register16(Register16::BX) {
                0 => Some(ServiceEvent::SetVirtualMouseConsumerStatus { loaded: false }),
                1 => Some(ServiceEvent::SetVirtualMouseConsumerStatus { loaded: true }),
                _ => {
                    set_service_error(cpu, ServiceError::InvalidParameter);
                    None
                }
            },
            MOUSE_HOST_CURSOR_VISIBILITY => match cpu.get_register16(Register16::BX) {
                0 => {
                    clear_carry(cpu);
                    Some(ServiceEvent::SetHostCursorVisibility { visible: false })
                }
                1 => {
                    clear_carry(cpu);
                    Some(ServiceEvent::SetHostCursorVisibility { visible: true })
                }
                _ => {
                    set_service_error(cpu, ServiceError::InvalidParameter);
                    None
                }
            },
            _ => {
                set_service_error(cpu, ServiceError::InvalidParameter);
                None
            }
        }
    }

    /// Initiates a file transfer operation between the guest and host.
    /// Returns a 16-bit handle - multiple transfers can be in progress at once.
    fn begin_file_transfer<C: Cpu>(&mut self, cpu: &mut C) -> Option<ServiceEvent> {
        if cpu.get_register16(Register16::CX) < FILE_TRANSFER_STRUCTURE_SIZE {
            set_service_error(cpu, ServiceError::InvalidParameter);
            return None;
        }

        let flags = cpu.get_register8(Register8::AL);
        if flags & !FILE_TRANSFER_FLAG_MASK != 0 {
            set_service_error(cpu, ServiceError::InvalidParameter);
            return None;
        }
        let direction = match flags & FILE_TRANSFER_DIRECTION_MASK {
            FILE_TRANSFER_GUEST_TO_HOST => FileTransferDirection::GuestToHost,
            FILE_TRANSFER_HOST_TO_GUEST => FileTransferDirection::HostToGuest,
            _ => {
                set_service_error(cpu, ServiceError::InvalidAccess);
                return None;
            }
        };
        let non_interactive = flags & FILE_TRANSFER_NON_INTERACTIVE != 0;

        // Transfer structure is read from ES:DI
        let structure_segment = cpu.get_register16(Register16::ES);
        let structure_offset = cpu.get_register16(Register16::DI);

        // Read pointer to filename of offset:segment
        let filename_offset = match read_guest_u16(cpu, structure_segment, structure_offset) {
            Ok(value) => value,
            Err(error) => {
                set_service_error(cpu, error);
                return None;
            }
        };
        let filename_segment = match read_guest_u16(cpu, structure_segment, structure_offset.wrapping_add(2)) {
            Ok(value) => value,
            Err(error) => {
                set_service_error(cpu, error);
                return None;
            }
        };

        if direction == FileTransferDirection::HostToGuest {
            if !matches!(self.host_file_request, HostFileRequestState::Idle) {
                set_service_error(cpu, ServiceError::Busy);
                return None;
            }

            if let Err(error) = write_guest_u16(
                cpu,
                structure_segment,
                structure_offset.wrapping_add(8),
                FileTransferStatus::Wait.into(),
            ) {
                set_service_error(cpu, error);
                return None;
            }

            let requested_filename = if non_interactive {
                match read_guest_filename(cpu, filename_segment, filename_offset) {
                    Ok(filename) => filename,
                    Err(error) => {
                        set_service_error(cpu, error);
                        return None;
                    }
                }
            }
            else {
                String::new()
            };

            let Some(handle) = self.create_file_transfer_operation(&requested_filename, 0, direction)
            else {
                set_service_error(cpu, ServiceError::TooManyOpenFiles);
                return None;
            };
            let operation = self
                .file_transfer_operations
                .get_mut(&handle)
                .expect("newly created file transfer handle disappeared");
            operation.ready = false;
            operation.non_interactive = non_interactive;
            self.host_file_request = HostFileRequestState::Pending(PendingHostFileRequest {
                handle,
                structure_segment,
                structure_offset,
                filename_segment,
                filename_offset,
            });

            log::debug!("Started pending host file transfer: handle={:04X}h", handle);
            cpu.set_register16(Register16::BX, handle);
            clear_carry(cpu);
            return Some(ServiceEvent::HostFileTransferRequested {
                filename: non_interactive.then_some(requested_filename),
            });
        }

        let transfer_size = match read_guest_u32(cpu, structure_segment, structure_offset.wrapping_add(4)) {
            Ok(value) => u64::from(value),
            Err(error) => {
                set_service_error(cpu, error);
                return None;
            }
        };
        let filename = match read_guest_filename(cpu, filename_segment, filename_offset) {
            Ok(filename) => filename,
            Err(error) => {
                set_service_error(cpu, error);
                return None;
            }
        };

        let Some(handle) = self.create_file_transfer_operation(filename, transfer_size, direction)
        else {
            set_service_error(cpu, ServiceError::TooManyOpenFiles);
            return None;
        };

        let operation = self
            .file_transfer_operations
            .get_mut(&handle)
            .expect("newly created file transfer handle disappeared");
        operation.non_interactive = non_interactive;

        log::debug!(
            "Started file transfer: handle={:04X}h, direction={:?}, filename='{}', size={} bytes",
            handle,
            operation.direction,
            operation.filename,
            operation.size
        );

        cpu.set_register16(Register16::BX, handle);
        clear_carry(cpu);
        None
    }

    fn transfer_file_block<C: Cpu>(&mut self, cpu: &mut C) {
        let handle = cpu.get_register16(Register16::BX);
        let length = usize::from(cpu.get_register16(Register16::CX));
        if length == 0 {
            set_service_error(cpu, ServiceError::InvalidParameter);
            return;
        }

        let Some(operation) = self.file_transfer_operations.get(&handle)
        else {
            set_service_error(cpu, ServiceError::InvalidHandle);
            return;
        };
        if !operation.ready {
            set_service_error(cpu, ServiceError::Busy);
            return;
        }

        let buffer_segment = cpu.get_register16(Register16::ES);
        let buffer_offset = cpu.get_register16(Register16::DI);

        match operation.direction {
            FileTransferDirection::GuestToHost => {
                let Some(new_size) = operation.data.len().checked_add(length)
                else {
                    set_service_error(cpu, ServiceError::NotEnoughMemory);
                    return;
                };
                if new_size as u64 > operation.size {
                    set_service_error(cpu, ServiceError::InvalidData);
                    return;
                }

                let mut block = Vec::new();
                if block.try_reserve_exact(length).is_err() {
                    set_service_error(cpu, ServiceError::NotEnoughMemory);
                    return;
                }

                for index in 0..length {
                    match read_guest_u8(cpu, buffer_segment, buffer_offset.wrapping_add(index as u16)) {
                        Ok(value) => block.push(value),
                        Err(error) => {
                            set_service_error(cpu, error);
                            return;
                        }
                    }
                }

                let operation = self
                    .file_transfer_operations
                    .get_mut(&handle)
                    .expect("validated file transfer handle disappeared");
                if operation.data.try_reserve_exact(length).is_err() {
                    set_service_error(cpu, ServiceError::NotEnoughMemory);
                    return;
                }
                operation.data.extend_from_slice(&block);
                operation.transferred += length;
                operation.crc32 = crc32_update(operation.crc32, &block);

                log::debug!(
                    "Received file transfer block: handle={:04X}h, bytes={}, transferred={}/{}",
                    handle,
                    length,
                    operation.transferred,
                    operation.size
                );

                cpu.set_register16(Register16::AX, length as u16);
            }
            FileTransferDirection::HostToGuest => {
                let transfer_length = length.min(operation.data.len().saturating_sub(operation.transferred));
                for index in 0..transfer_length {
                    let value = operation.data[operation.transferred + index];
                    if let Err(error) =
                        write_guest_u8(cpu, buffer_segment, buffer_offset.wrapping_add(index as u16), value)
                    {
                        set_service_error(cpu, error);
                        return;
                    }
                }

                let operation = self
                    .file_transfer_operations
                    .get_mut(&handle)
                    .expect("validated file transfer handle disappeared");
                operation.transferred += transfer_length;
                operation.crc32 = crc32_update(
                    operation.crc32,
                    &operation.data[operation.transferred - transfer_length..operation.transferred],
                );

                log::debug!(
                    "Transferred host file block to guest: handle={:04X}h, bytes={}, transferred={}/{}",
                    handle,
                    transfer_length,
                    operation.transferred,
                    operation.size
                );

                cpu.set_register16(Register16::AX, transfer_length as u16);
            }
        }

        clear_carry(cpu);
    }

    fn end_file_transfer<C: Cpu>(&mut self, cpu: &mut C) -> Option<ServiceEvent> {
        let handle = cpu.get_register16(Register16::BX);
        let action = cpu.get_register8(Register8::AL);

        let Some(operation) = self.file_transfer_operations.get(&handle)
        else {
            set_service_error(cpu, ServiceError::InvalidHandle);
            return None;
        };

        let action_name = match action {
            FILE_TRANSFER_COMMIT => "commit",
            FILE_TRANSFER_ABORT => "abort",
            _ => {
                set_service_error(cpu, ServiceError::InvalidParameter);
                return None;
            }
        };

        log::debug!(
            "Finalizing file transfer: handle={:04X}h, action={}, filename='{}', transferred={}/{}",
            handle,
            action_name,
            operation.filename,
            operation.transferred,
            operation.size
        );

        if action == FILE_TRANSFER_ABORT {
            if matches!(
                self.host_file_request,
                HostFileRequestState::Pending(request) if request.handle == handle
            ) {
                self.host_file_request = HostFileRequestState::Idle;
            }
            let operation = self
                .destroy_file_transfer_operation(handle)
                .expect("validated file transfer handle disappeared");

            cpu.set_register16(Register16::AX, 0);
            set_crc32_result(cpu, !operation.crc32);
            clear_carry(cpu);
            return None;
        }

        if !operation.ready {
            set_service_error(cpu, ServiceError::Busy);
            return None;
        }

        if operation.transferred as u64 != operation.size {
            set_service_error(cpu, ServiceError::InvalidData);
            return None;
        }

        let operation = self
            .destroy_file_transfer_operation(handle)
            .expect("validated file transfer handle disappeared");
        let crc32 = !operation.crc32;

        log::debug!("Finalized file transfer: handle={:04X}h, CRC-32={:08X}h", handle, crc32);

        cpu.set_register16(Register16::AX, 0);
        set_crc32_result(cpu, crc32);
        clear_carry(cpu);

        match operation.direction {
            FileTransferDirection::GuestToHost => Some(ServiceEvent::GuestFileTransferComplete {
                filename: operation.filename,
                data: operation.data,
                non_interactive: operation.non_interactive,
            }),
            FileTransferDirection::HostToGuest => None,
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

    /// Create a file transfer operation and return its unique 16-bit handle.
    ///
    /// Returns `None` if every possible handle is currently in use.
    pub fn create_file_transfer_operation(
        &mut self,
        filename: impl Into<String>,
        size: u64,
        direction: FileTransferDirection,
    ) -> Option<FileTransferHandle> {
        if self.file_transfer_operations.len() > u16::MAX as usize {
            return None;
        }

        let filename = filename.into();

        if let Some(handle) = self.free_file_transfer_handles.pop() {
            self.insert_file_transfer_operation(handle, filename, size, direction);
            return Some(handle);
        }

        loop {
            let handle = self.next_file_transfer_handle;
            self.next_file_transfer_handle = self.next_file_transfer_handle.wrapping_add(1);

            if !self.file_transfer_operations.contains_key(&handle) {
                self.insert_file_transfer_operation(handle, filename, size, direction);
                return Some(handle);
            }
        }
    }

    fn insert_file_transfer_operation(
        &mut self,
        handle: FileTransferHandle,
        filename: String,
        size: u64,
        direction: FileTransferDirection,
    ) {
        self.file_transfer_operations.insert(
            handle,
            FileTransferOperation {
                filename,
                size,
                direction,
                data: Vec::new(),
                transferred: 0,
                ready: true,
                crc32: CRC32_INITIAL,
                non_interactive: false,
            },
        );
    }

    /// Complete the pending host-file selection and publish its metadata to the guest.
    pub fn complete_host_file_request<C: Cpu>(
        &mut self,
        cpu: &mut C,
        filename: impl Into<String>,
        data: Vec<u8>,
    ) -> Result<(), ServiceError> {
        let HostFileRequestState::Pending(request) = std::mem::take(&mut self.host_file_request)
        else {
            return Err(ServiceError::InvalidHandle);
        };
        let filename = filename.into();

        if filename.is_empty() || filename.len() > MAX_TRANSFER_FILENAME_LEN || data.len() > u32::MAX as usize {
            let _ = write_guest_u16(
                cpu,
                request.structure_segment,
                request.structure_offset.wrapping_add(8),
                FileTransferStatus::Aborted.into(),
            );
            return Err(ServiceError::InvalidData);
        }

        if let Err(error) = write_guest_filename(cpu, request.filename_segment, request.filename_offset, &filename)
            .and_then(|_| {
                write_guest_u32(
                    cpu,
                    request.structure_segment,
                    request.structure_offset.wrapping_add(4),
                    data.len() as u32,
                )
            })
        {
            let _ = write_guest_u16(
                cpu,
                request.structure_segment,
                request.structure_offset.wrapping_add(8),
                FileTransferStatus::Aborted.into(),
            );
            return Err(error);
        }

        let operation = self
            .file_transfer_operations
            .get_mut(&request.handle)
            .ok_or(ServiceError::InvalidHandle)?;
        operation.filename = filename;
        operation.size = data.len() as u64;
        operation.data = data;
        operation.ready = true;

        // Publish READY last so the guest cannot observe partially-written metadata.
        if let Err(error) = write_guest_u16(
            cpu,
            request.structure_segment,
            request.structure_offset.wrapping_add(8),
            FileTransferStatus::Ready.into(),
        ) {
            operation.ready = false;
            let _ = write_guest_u16(
                cpu,
                request.structure_segment,
                request.structure_offset.wrapping_add(8),
                FileTransferStatus::Aborted.into(),
            );
            return Err(error);
        }

        log::debug!(
            "Host file ready for transfer: handle={:04X}h, filename='{}', size={} bytes",
            request.handle,
            operation.filename,
            operation.size
        );
        Ok(())
    }

    /// Mark a pending host-file selection as aborted by the user or frontend.
    pub fn abort_host_file_request<C: Cpu>(&mut self, cpu: &mut C) -> Result<(), ServiceError> {
        self.fail_host_file_request(cpu, FileTransferStatus::Aborted)
    }

    /// Mark a pending non-interactive host-file request as missing from the host resource.
    pub fn host_file_not_found<C: Cpu>(&mut self, cpu: &mut C) -> Result<(), ServiceError> {
        self.fail_host_file_request(cpu, FileTransferStatus::HostFileNotFound)
    }

    fn fail_host_file_request<C: Cpu>(&mut self, cpu: &mut C, status: FileTransferStatus) -> Result<(), ServiceError> {
        let HostFileRequestState::Pending(request) = std::mem::take(&mut self.host_file_request)
        else {
            return Err(ServiceError::InvalidHandle);
        };
        write_guest_u16(
            cpu,
            request.structure_segment,
            request.structure_offset.wrapping_add(8),
            status.into(),
        )
    }

    /// Destroy a file transfer operation, returning it if the handle was active.
    pub fn destroy_file_transfer_operation(&mut self, handle: FileTransferHandle) -> Option<FileTransferOperation> {
        let operation = self.file_transfer_operations.remove(&handle)?;
        self.free_file_transfer_handles.push(handle);
        Some(operation)
    }

    pub fn file_transfer_operation(&self, handle: FileTransferHandle) -> Option<&FileTransferOperation> {
        self.file_transfer_operations.get(&handle)
    }
}

fn read_guest_u8<C: Cpu>(cpu: &mut C, segment: u16, offset: u16) -> Result<u8, ServiceError> {
    let address = crate::cpu_common::calc_linear_address(segment, offset) as usize;
    cpu.bus_mut()
        .read_u8(address, 0)
        .map(|(value, _)| value)
        .map_err(|_| ServiceError::InvalidData)
}

fn read_guest_u16<C: Cpu>(cpu: &mut C, segment: u16, offset: u16) -> Result<u16, ServiceError> {
    let low = read_guest_u8(cpu, segment, offset)?;
    let high = read_guest_u8(cpu, segment, offset.wrapping_add(1))?;
    Ok(u16::from(low) | (u16::from(high) << 8))
}

fn read_guest_u32<C: Cpu>(cpu: &mut C, segment: u16, offset: u16) -> Result<u32, ServiceError> {
    let low = read_guest_u16(cpu, segment, offset)?;
    let high = read_guest_u16(cpu, segment, offset.wrapping_add(2))?;
    Ok(u32::from(low) | (u32::from(high) << 16))
}

fn write_guest_u8<C: Cpu>(cpu: &mut C, segment: u16, offset: u16, value: u8) -> Result<(), ServiceError> {
    let address = crate::cpu_common::calc_linear_address(segment, offset) as usize;
    cpu.bus_mut()
        .write_u8(address, value, 0)
        .map(|_| ())
        .map_err(|_| ServiceError::InvalidData)
}

fn write_guest_u16<C: Cpu>(cpu: &mut C, segment: u16, offset: u16, value: u16) -> Result<(), ServiceError> {
    write_guest_u8(cpu, segment, offset, value as u8)?;
    write_guest_u8(cpu, segment, offset.wrapping_add(1), (value >> 8) as u8)
}

fn write_guest_u32<C: Cpu>(cpu: &mut C, segment: u16, offset: u16, value: u32) -> Result<(), ServiceError> {
    write_guest_u16(cpu, segment, offset, value as u16)?;
    write_guest_u16(cpu, segment, offset.wrapping_add(2), (value >> 16) as u16)
}

fn write_guest_filename<C: Cpu>(cpu: &mut C, segment: u16, offset: u16, filename: &str) -> Result<(), ServiceError> {
    for (index, value) in filename.bytes().chain(std::iter::once(0)).enumerate() {
        write_guest_u8(cpu, segment, offset.wrapping_add(index as u16), value)?;
    }
    Ok(())
}

fn read_guest_filename<C: Cpu>(cpu: &mut C, segment: u16, offset: u16) -> Result<String, ServiceError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(MAX_TRANSFER_FILENAME_LEN)
        .map_err(|_| ServiceError::NotEnoughMemory)?;

    for index in 0..=MAX_TRANSFER_FILENAME_LEN {
        let byte = read_guest_u8(cpu, segment, offset.wrapping_add(index as u16))?;
        if byte == 0 {
            if bytes.is_empty() {
                return Err(ServiceError::InvalidData);
            }
            return Ok(String::from_utf8_lossy(&bytes).into_owned());
        }
        if index == MAX_TRANSFER_FILENAME_LEN {
            break;
        }
        bytes.push(byte);
    }

    Err(ServiceError::InvalidData)
}

fn clear_carry<C: Cpu>(cpu: &mut C) {
    cpu.set_flags(cpu.get_flags() & !CARRY_FLAG);
}

fn crc32_update(mut crc32: u32, data: &[u8]) -> u32 {
    for &byte in data {
        crc32 ^= u32::from(byte);
        for _ in 0..8 {
            crc32 = if crc32 & 1 != 0 {
                (crc32 >> 1) ^ CRC32_POLYNOMIAL
            }
            else {
                crc32 >> 1
            };
        }
    }
    crc32
}

fn set_crc32_result<C: Cpu>(cpu: &mut C, crc32: u32) {
    cpu.set_register16(Register16::CX, crc32 as u16);
    cpu.set_register16(Register16::DX, (crc32 >> 16) as u16);
}

fn set_service_error<C: Cpu>(cpu: &mut C, error: ServiceError) {
    cpu.set_register16(Register16::AX, error.into());
    cpu.set_flags(cpu.get_flags() | CARRY_FLAG);
}

pub fn is_martypc_probe<C: Cpu>(interrupt: u8, cpu: &C) -> bool {
    interrupt == MARTYPC_PROBE_INTERRUPT
        && cpu.get_register16(Register16::AX) == MARTYPC_PROBE_AX
        && cpu.get_register16(Register16::BX) == MARTYPC_PROBE_BX
        && cpu.get_register16(Register16::CX) == MARTYPC_PROBE_CX
}

pub fn is_service_control<C: Cpu>(cpu: &C) -> bool {
    cpu.get_register8(Register8::AH) == ServiceFunction::ServiceControl.into()
        && cpu.get_register16(Register16::BX) == SERVICE_CTRL_BX
        && cpu.get_register16(Register16::CX) == SERVICE_CTRL_CX
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_STRUCTURE_SEGMENT: u16 = 0x1000;
    const TEST_STRUCTURE_OFFSET: u16 = 0x0100;
    const TEST_FILENAME_OFFSET: u16 = 0x0200;
    const TEST_BUFFER_OFFSET: u16 = 0x0300;

    #[test]
    fn crc32_matches_standard_check_value() {
        assert_eq!(!crc32_update(CRC32_INITIAL, b"123456789"), 0xCBF4_3926);
    }

    fn write_guest_bytes(cpu: &mut crate::cpu_808x::Intel808x, segment: u16, offset: u16, bytes: &[u8]) {
        for (index, value) in bytes.iter().copied().enumerate() {
            let address = crate::cpu_common::calc_linear_address(segment, offset.wrapping_add(index as u16));
            cpu.bus_mut().write_u8(address as usize, value, 0).unwrap();
        }
    }

    fn read_guest_bytes(cpu: &mut crate::cpu_808x::Intel808x, segment: u16, offset: u16, length: usize) -> Vec<u8> {
        (0..length)
            .map(|index| {
                let address = crate::cpu_common::calc_linear_address(segment, offset.wrapping_add(index as u16));
                cpu.bus_mut().read_u8(address as usize, 0).unwrap().0
            })
            .collect()
    }

    fn begin_transfer(
        manager: &mut ServiceInterruptManager,
        cpu: &mut crate::cpu_808x::Intel808x,
        direction: u8,
        filename: &str,
        size: u32,
    ) -> Option<FileTransferHandle> {
        let mut structure = Vec::from(TEST_FILENAME_OFFSET.to_le_bytes());
        structure.extend_from_slice(&TEST_STRUCTURE_SEGMENT.to_le_bytes());
        structure.extend_from_slice(&size.to_le_bytes());
        write_guest_bytes(cpu, TEST_STRUCTURE_SEGMENT, TEST_STRUCTURE_OFFSET, &structure);

        let mut filename_bytes = filename.as_bytes().to_vec();
        filename_bytes.push(0);
        write_guest_bytes(cpu, TEST_STRUCTURE_SEGMENT, TEST_FILENAME_OFFSET, &filename_bytes);

        cpu.set_register8(Register8::AL, direction);
        cpu.set_register16(Register16::CX, FILE_TRANSFER_STRUCTURE_SIZE);
        cpu.set_register16(Register16::ES, TEST_STRUCTURE_SEGMENT);
        cpu.set_register16(Register16::DI, TEST_STRUCTURE_OFFSET);
        manager.handle_interrupt(ServiceFunction::FileTransferBegin, cpu);

        if cpu.get_flags() & CARRY_FLAG == 0 {
            Some(cpu.get_register16(Register16::BX))
        }
        else {
            None
        }
    }

    fn transfer_guest_block(
        manager: &mut ServiceInterruptManager,
        cpu: &mut crate::cpu_808x::Intel808x,
        handle: FileTransferHandle,
        bytes: &[u8],
    ) {
        write_guest_bytes(cpu, TEST_STRUCTURE_SEGMENT, TEST_BUFFER_OFFSET, bytes);
        cpu.set_register16(Register16::BX, handle);
        cpu.set_register16(Register16::ES, TEST_STRUCTURE_SEGMENT);
        cpu.set_register16(Register16::DI, TEST_BUFFER_OFFSET);
        cpu.set_register16(Register16::CX, bytes.len() as u16);
        manager.handle_interrupt(ServiceFunction::FileTransferBlock, cpu);
    }

    #[test]
    fn file_transfer_handles_start_at_1000h_and_increment() {
        let mut manager = ServiceInterruptManager::new(None, true);

        let first = manager.create_file_transfer_operation("first.bin", 1024, FileTransferDirection::GuestToHost);
        let second = manager.create_file_transfer_operation("second.bin", 2048, FileTransferDirection::HostToGuest);

        assert_eq!(first, Some(0x1000));
        assert_eq!(second, Some(0x1001));
        assert_eq!(manager.file_transfer_operation(0x1000).unwrap().filename(), "first.bin");
        assert_eq!(manager.file_transfer_operation(0x1000).unwrap().size(), 1024);
    }

    #[test]
    fn destroying_a_file_transfer_invalidates_its_handle() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let handle = manager
            .create_file_transfer_operation("transfer.bin", 4096, FileTransferDirection::GuestToHost)
            .unwrap();

        let operation = manager.destroy_file_transfer_operation(handle).unwrap();

        assert_eq!(operation.filename(), "transfer.bin");
        assert_eq!(operation.size(), 4096);
        assert_eq!(manager.file_transfer_operation(handle), None);
        assert_eq!(manager.destroy_file_transfer_operation(handle), None);
        assert_eq!(
            manager.create_file_transfer_operation("replacement.bin", 1, FileTransferDirection::GuestToHost),
            Some(handle)
        );
    }

    #[test]
    fn freed_handle_is_reused_without_colliding_with_active_transfers() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let first = manager
            .create_file_transfer_operation("first.bin", 1, FileTransferDirection::GuestToHost)
            .unwrap();
        let second = manager
            .create_file_transfer_operation("second.bin", 1, FileTransferDirection::GuestToHost)
            .unwrap();

        manager.destroy_file_transfer_operation(first).unwrap();
        let replacement = manager
            .create_file_transfer_operation("replacement.bin", 1, FileTransferDirection::GuestToHost)
            .unwrap();

        assert_eq!(replacement, first);
        assert_ne!(replacement, second);
        assert_eq!(
            manager.file_transfer_operation(second).unwrap().filename(),
            "second.bin"
        );
    }

    #[test]
    fn reset_destroys_transfers_and_restarts_handle_allocation() {
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
        assert_eq!(
            manager.create_file_transfer_operation("new.bin", 128, FileTransferDirection::GuestToHost),
            Some(0x1000)
        );
    }

    #[test]
    fn guest_to_host_transfer_accumulates_blocks_and_commits() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let mut cpu = crate::cpu_808x::Intel808x::default();
        let handle = begin_transfer(&mut manager, &mut cpu, FILE_TRANSFER_GUEST_TO_HOST, "OUTPUT.BIN", 6).unwrap();

        transfer_guest_block(&mut manager, &mut cpu, handle, b"abc");
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(cpu.get_register16(Register16::AX), 3);
        transfer_guest_block(&mut manager, &mut cpu, handle, b"def");

        cpu.set_register16(Register16::BX, handle);
        cpu.set_register8(Register8::AL, FILE_TRANSFER_COMMIT);
        let event = manager.handle_interrupt(ServiceFunction::FileTransferEnd, &mut cpu);

        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(
            u32::from(cpu.get_register16(Register16::DX)) << 16 | u32::from(cpu.get_register16(Register16::CX)),
            !crc32_update(CRC32_INITIAL, b"abcdef")
        );
        assert!(manager.file_transfer_operation(handle).is_none());
        match event {
            Some(ServiceEvent::GuestFileTransferComplete { filename, data, .. }) => {
                assert_eq!(filename, "OUTPUT.BIN");
                assert_eq!(data, b"abcdef");
            }
            _ => panic!("expected a committed guest file transfer"),
        }
        assert_eq!(
            manager.create_file_transfer_operation("NEXT.BIN", 1, FileTransferDirection::GuestToHost),
            Some(handle)
        );
    }

    #[test]
    fn non_interactive_guest_to_host_commit_preserves_mode() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let mut cpu = crate::cpu_808x::Intel808x::default();
        let handle = begin_transfer(
            &mut manager,
            &mut cpu,
            FILE_TRANSFER_GUEST_TO_HOST | FILE_TRANSFER_NON_INTERACTIVE,
            "OUTPUT.BIN",
            3,
        )
        .unwrap();

        transfer_guest_block(&mut manager, &mut cpu, handle, b"abc");
        cpu.set_register16(Register16::BX, handle);
        cpu.set_register8(Register8::AL, FILE_TRANSFER_COMMIT);

        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::FileTransferEnd, &mut cpu),
            Some(ServiceEvent::GuestFileTransferComplete {
                filename,
                data,
                non_interactive: true,
            }) if filename == "OUTPUT.BIN" && data == b"abc"
        ));
    }

    #[test]
    fn known_size_must_match_before_commit() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let mut cpu = crate::cpu_808x::Intel808x::default();
        let handle = begin_transfer(&mut manager, &mut cpu, FILE_TRANSFER_GUEST_TO_HOST, "SHORT.BIN", 4).unwrap();

        transfer_guest_block(&mut manager, &mut cpu, handle, b"abc");
        cpu.set_register16(Register16::BX, handle);
        cpu.set_register8(Register8::AL, FILE_TRANSFER_COMMIT);
        assert!(manager
            .handle_interrupt(ServiceFunction::FileTransferEnd, &mut cpu)
            .is_none());
        assert_ne!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(cpu.get_register16(Register16::AX), ServiceError::InvalidData.into());
        assert!(manager.file_transfer_operation(handle).is_some());

        transfer_guest_block(&mut manager, &mut cpu, handle, b"d");
        cpu.set_register16(Register16::BX, handle);
        cpu.set_register8(Register8::AL, FILE_TRANSFER_COMMIT);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::FileTransferEnd, &mut cpu),
            Some(ServiceEvent::GuestFileTransferComplete { .. })
        ));
    }

    #[test]
    fn begin_accepts_maximum_u32_file_size() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let mut cpu = crate::cpu_808x::Intel808x::default();

        let handle = begin_transfer(
            &mut manager,
            &mut cpu,
            FILE_TRANSFER_GUEST_TO_HOST,
            "MAXSIZE.BIN",
            u32::MAX,
        )
        .unwrap();

        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(
            manager.file_transfer_operation(handle).unwrap().size(),
            u64::from(u32::MAX)
        );
    }

    #[test]
    fn abort_destroys_transfer_without_emitting_save_event() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let mut cpu = crate::cpu_808x::Intel808x::default();
        let handle = begin_transfer(&mut manager, &mut cpu, FILE_TRANSFER_GUEST_TO_HOST, "ABORT.BIN", 10).unwrap();

        transfer_guest_block(&mut manager, &mut cpu, handle, b"discard me");
        cpu.set_register16(Register16::BX, handle);
        cpu.set_register8(Register8::AL, FILE_TRANSFER_ABORT);

        assert!(manager
            .handle_interrupt(ServiceFunction::FileTransferEnd, &mut cpu)
            .is_none());
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);
        assert!(manager.file_transfer_operation(handle).is_none());
    }

    #[test]
    fn staged_host_file_transfers_sequentially_to_guest_memory() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let mut cpu = crate::cpu_808x::Intel808x::default();
        let handle = begin_transfer(&mut manager, &mut cpu, FILE_TRANSFER_HOST_TO_GUEST, "", 0).unwrap();
        manager
            .complete_host_file_request(&mut cpu, "SELECTED.BIN", b"host data".to_vec())
            .unwrap();

        assert_eq!(
            read_guest_bytes(
                &mut cpu,
                TEST_STRUCTURE_SEGMENT,
                TEST_FILENAME_OFFSET,
                "SELECTED.BIN".len() + 1
            ),
            b"SELECTED.BIN\0"
        );
        assert_eq!(
            read_guest_bytes(&mut cpu, TEST_STRUCTURE_SEGMENT, TEST_STRUCTURE_OFFSET + 4, 4),
            9u32.to_le_bytes()
        );
        assert_eq!(
            read_guest_bytes(&mut cpu, TEST_STRUCTURE_SEGMENT, TEST_STRUCTURE_OFFSET + 8, 2),
            u16::from(FileTransferStatus::Ready).to_le_bytes()
        );

        cpu.set_register16(Register16::BX, handle);
        cpu.set_register16(Register16::ES, TEST_STRUCTURE_SEGMENT);
        cpu.set_register16(Register16::DI, TEST_BUFFER_OFFSET);
        cpu.set_register16(Register16::CX, 5);
        manager.handle_interrupt(ServiceFunction::FileTransferBlock, &mut cpu);
        assert_eq!(cpu.get_register16(Register16::AX), 5);

        cpu.set_register16(Register16::DI, TEST_BUFFER_OFFSET + 5);
        cpu.set_register16(Register16::CX, 10);
        manager.handle_interrupt(ServiceFunction::FileTransferBlock, &mut cpu);
        assert_eq!(cpu.get_register16(Register16::AX), 4);
        assert_eq!(
            read_guest_bytes(&mut cpu, TEST_STRUCTURE_SEGMENT, TEST_BUFFER_OFFSET, 9),
            b"host data"
        );

        cpu.set_register16(Register16::BX, handle);
        cpu.set_register8(Register8::AL, FILE_TRANSFER_COMMIT);
        assert!(manager
            .handle_interrupt(ServiceFunction::FileTransferEnd, &mut cpu)
            .is_none());
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(
            u32::from(cpu.get_register16(Register16::DX)) << 16 | u32::from(cpu.get_register16(Register16::CX)),
            !crc32_update(CRC32_INITIAL, b"host data")
        );
        assert!(manager.file_transfer_operation(handle).is_none());
    }

    #[test]
    fn host_to_guest_begin_returns_a_pending_handle() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let mut cpu = crate::cpu_808x::Intel808x::default();

        let mut structure = Vec::from(TEST_FILENAME_OFFSET.to_le_bytes());
        structure.extend_from_slice(&TEST_STRUCTURE_SEGMENT.to_le_bytes());
        structure.extend_from_slice(&0u32.to_le_bytes());
        write_guest_bytes(&mut cpu, TEST_STRUCTURE_SEGMENT, TEST_STRUCTURE_OFFSET, &structure);

        cpu.set_register8(Register8::AL, FILE_TRANSFER_HOST_TO_GUEST);
        cpu.set_register16(Register16::CX, FILE_TRANSFER_STRUCTURE_SIZE);
        cpu.set_register16(Register16::ES, TEST_STRUCTURE_SEGMENT);
        cpu.set_register16(Register16::DI, TEST_STRUCTURE_OFFSET);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::FileTransferBegin, &mut cpu),
            Some(ServiceEvent::HostFileTransferRequested { filename: None })
        ));
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);
        let handle = cpu.get_register16(Register16::BX);
        assert_eq!(handle, 0x1000);
        assert_eq!(
            read_guest_bytes(&mut cpu, TEST_STRUCTURE_SEGMENT, TEST_STRUCTURE_OFFSET + 8, 2),
            u16::from(FileTransferStatus::Wait).to_le_bytes()
        );

        cpu.set_register16(Register16::BX, handle);
        cpu.set_register16(Register16::CX, 1);
        cpu.set_register16(Register16::ES, TEST_STRUCTURE_SEGMENT);
        cpu.set_register16(Register16::DI, TEST_BUFFER_OFFSET);
        manager.handle_interrupt(ServiceFunction::FileTransferBlock, &mut cpu);
        assert_eq!(cpu.get_register16(Register16::AX), ServiceError::Busy.into());

        manager
            .complete_host_file_request(&mut cpu, "PICKED.DAT", b"selected".to_vec())
            .unwrap();
        assert_eq!(
            read_guest_bytes(&mut cpu, TEST_STRUCTURE_SEGMENT, TEST_STRUCTURE_OFFSET + 8, 2),
            u16::from(FileTransferStatus::Ready).to_le_bytes()
        );
    }

    #[test]
    fn non_interactive_host_to_guest_begin_requests_resource_filename() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let mut cpu = crate::cpu_808x::Intel808x::default();

        let mut structure = Vec::from(TEST_FILENAME_OFFSET.to_le_bytes());
        structure.extend_from_slice(&TEST_STRUCTURE_SEGMENT.to_le_bytes());
        structure.extend_from_slice(&0u32.to_le_bytes());
        structure.extend_from_slice(&u16::from(FileTransferStatus::Wait).to_le_bytes());
        write_guest_bytes(&mut cpu, TEST_STRUCTURE_SEGMENT, TEST_STRUCTURE_OFFSET, &structure);
        write_guest_bytes(
            &mut cpu,
            TEST_STRUCTURE_SEGMENT,
            TEST_FILENAME_OFFSET,
            b"RESOURCE.DAT\0",
        );

        cpu.set_register8(
            Register8::AL,
            FILE_TRANSFER_HOST_TO_GUEST | FILE_TRANSFER_NON_INTERACTIVE,
        );
        cpu.set_register16(Register16::CX, FILE_TRANSFER_STRUCTURE_SIZE);
        cpu.set_register16(Register16::ES, TEST_STRUCTURE_SEGMENT);
        cpu.set_register16(Register16::DI, TEST_STRUCTURE_OFFSET);

        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::FileTransferBegin, &mut cpu),
            Some(ServiceEvent::HostFileTransferRequested {
                filename: Some(filename)
            }) if filename == "RESOURCE.DAT"
        ));
        let operation = manager
            .file_transfer_operation(cpu.get_register16(Register16::BX))
            .unwrap();
        assert!(operation.non_interactive);
    }

    #[test]
    fn file_transfer_begin_rejects_unknown_flag_bits() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let mut cpu = crate::cpu_808x::Intel808x::default();

        assert!(begin_transfer(&mut manager, &mut cpu, 0x80, "INVALID.DAT", 1).is_none());
        assert_eq!(
            cpu.get_register16(Register16::AX),
            ServiceError::InvalidParameter.into()
        );
    }

    #[test]
    fn cancelled_host_file_request_publishes_aborted_status() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let mut cpu = crate::cpu_808x::Intel808x::default();

        let handle = begin_transfer(&mut manager, &mut cpu, FILE_TRANSFER_HOST_TO_GUEST, "", 0).unwrap();
        manager.abort_host_file_request(&mut cpu).unwrap();
        assert_eq!(
            read_guest_bytes(&mut cpu, TEST_STRUCTURE_SEGMENT, TEST_STRUCTURE_OFFSET + 8, 2),
            u16::from(FileTransferStatus::Aborted).to_le_bytes()
        );
        assert!(manager.file_transfer_operation(handle).is_some());
    }

    #[test]
    fn missing_non_interactive_host_file_publishes_not_found_status() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let mut cpu = crate::cpu_808x::Intel808x::default();

        let handle = begin_transfer(
            &mut manager,
            &mut cpu,
            FILE_TRANSFER_HOST_TO_GUEST | FILE_TRANSFER_NON_INTERACTIVE,
            "MISSING.BIN",
            0,
        )
        .unwrap();
        manager.host_file_not_found(&mut cpu).unwrap();

        assert_eq!(
            read_guest_bytes(&mut cpu, TEST_STRUCTURE_SEGMENT, TEST_STRUCTURE_OFFSET + 8, 2),
            u16::from(FileTransferStatus::HostFileNotFound).to_le_bytes()
        );
        assert!(manager.file_transfer_operation(handle).is_some());
    }

    #[test]
    fn service_control_gates_shared_service_functions() {
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
        assert_eq!(cpu.get_register8(Register8::AL), ServiceControl::Enable.into());
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);

        cpu.set_register8(Register8::AL, ServiceControl::Disable.into());
        assert!(is_service_control(&cpu));
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::ServiceControl, &mut cpu),
            Some(ServiceEvent::ServiceInterruptEnabled(false))
        ));
        assert!(!manager.enabled());
        assert_eq!(cpu.get_register8(Register8::AL), ServiceControl::Disable.into());

        cpu.set_register8(Register8::AL, ServiceControl::Query.into());
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::ServiceControl, &mut cpu),
            Some(ServiceEvent::ServiceInterruptEnabled(false))
        ));
        assert_eq!(cpu.get_register8(Register8::AL), ServiceControl::Disable.into());
        assert!(manager
            .handle_interrupt(ServiceFunction::PitLogging, &mut cpu)
            .is_none());

        cpu.set_register8(Register8::AL, ServiceControl::Enable.into());
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::ServiceControl, &mut cpu),
            Some(ServiceEvent::ServiceInterruptEnabled(true))
        ));
        assert!(manager.enabled());
        assert_eq!(cpu.get_register8(Register8::AL), ServiceControl::Enable.into());

        cpu.set_register8(Register8::AL, 7);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::Quit, &mut cpu),
            Some(ServiceEvent::QuitEmulator(7))
        ));

        assert_eq!(ServiceFunction::try_from(0xFF), Err(0xFF));
    }

    #[test]
    fn speed_control_queries_and_sets_fixed_point_percentage() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let mut cpu = crate::cpu_808x::Intel808x::default();

        manager.configure_speed_control(500, 1000, 1500);
        cpu.set_register8(Register8::AL, SPEED_CONTROL_QUERY);
        cpu.set_flags(cpu.get_flags() | CARRY_FLAG);
        assert!(manager
            .handle_interrupt(ServiceFunction::SpeedControl, &mut cpu)
            .is_none());
        assert_eq!(cpu.get_register16(Register16::BX), 500);
        assert_eq!(cpu.get_register16(Register16::CX), 1000);
        assert_eq!(cpu.get_register16(Register16::DX), 1500);
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);

        cpu.set_register8(Register8::AL, SPEED_CONTROL_SET);
        cpu.set_register16(Register16::CX, 2000);
        cpu.set_flags(cpu.get_flags() | CARRY_FLAG);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::SpeedControl, &mut cpu),
            Some(ServiceEvent::SetEmulationSpeed(1500))
        ));
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);

        cpu.set_register8(Register8::AL, SPEED_CONTROL_QUERY);
        manager.handle_interrupt(ServiceFunction::SpeedControl, &mut cpu);
        assert_eq!(cpu.get_register16(Register16::CX), 1500);
    }

    #[test]
    fn speed_control_rejects_invalid_subfunction() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let mut cpu = crate::cpu_808x::Intel808x::default();
        cpu.set_register8(Register8::AL, 0xFF);

        assert!(manager
            .handle_interrupt(ServiceFunction::SpeedControl, &mut cpu)
            .is_none());
        assert_ne!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(
            cpu.get_register16(Register16::AX),
            ServiceError::InvalidParameter.into()
        );
    }

    #[test]
    fn mouse_state_service_requests_and_completes_a_machine_snapshot() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let mut cpu = crate::cpu_808x::Intel808x::default();

        assert_eq!(ServiceFunction::try_from(0x11), Ok(ServiceFunction::MouseState));
        cpu.set_register8(Register8::AL, MOUSE_STATE_QUERY);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::MouseState, &mut cpu),
            Some(ServiceEvent::GetVirtualMouseState)
        ));

        cpu.set_flags(cpu.get_flags() | CARRY_FLAG);
        manager.complete_mouse_state(
            &mut cpu,
            Some((0x1234, 0x5678, 0x0003, 0x9ABC, -12, 34, MOUSE_STATE_FLAG_CAPTURED)),
        );

        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(cpu.get_register16(Register16::AX), 0x1234);
        assert_eq!(cpu.get_register16(Register16::BX), 0x5678);
        assert_eq!(cpu.get_register16(Register16::CX), 0x0003);
        assert_eq!(cpu.get_register16(Register16::DX), 0x9ABC);
        assert_eq!(cpu.get_register16(Register16::SI), (-12i16) as u16);
        assert_eq!(cpu.get_register16(Register16::DI), 34);
        assert_eq!(cpu.get_register16(Register16::BP), MOUSE_STATE_FLAG_CAPTURED);

        cpu.set_register8(Register8::AL, MOUSE_IRQ_QUERY);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::MouseState, &mut cpu),
            Some(ServiceEvent::GetVirtualMouseIrq)
        ));
        manager.complete_mouse_irq(&mut cpu, Some(5));
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(cpu.get_register16(Register16::DX), 5);

        cpu.set_register8(Register8::AL, MOUSE_DISPLAY_APERTURE_QUERY);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::MouseState, &mut cpu),
            Some(ServiceEvent::GetDisplayApertureSize)
        ));
        manager.complete_display_aperture_size(&mut cpu, Some((640, 480)));
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(cpu.get_register16(Register16::BX), 640);
        assert_eq!(cpu.get_register16(Register16::CX), 480);

        cpu.set_register8(Register8::AL, MOUSE_CONSUMER_RANGE_REPORT);
        cpu.set_register16(Register16::BX, 10);
        cpu.set_register16(Register16::CX, 639);
        cpu.set_register16(Register16::DX, 20);
        cpu.set_register16(Register16::SI, 199);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::MouseState, &mut cpu),
            Some(ServiceEvent::SetVirtualMouseConsumerRange {
                min_x: 10,
                max_x: 639,
                min_y: 20,
                max_y: 199,
            })
        ));
        manager.complete_mouse_consumer_range(&mut cpu, true);
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);

        cpu.set_register8(Register8::AL, MOUSE_CONSUMER_STATUS_REPORT);
        cpu.set_register16(Register16::BX, 1);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::MouseState, &mut cpu),
            Some(ServiceEvent::SetVirtualMouseConsumerStatus { loaded: true })
        ));
        manager.complete_mouse_consumer_status(&mut cpu, true);
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);

        cpu.set_register8(Register8::AL, MOUSE_HOST_CURSOR_VISIBILITY);
        cpu.set_register16(Register16::BX, 0);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::MouseState, &mut cpu),
            Some(ServiceEvent::SetHostCursorVisibility { visible: false })
        ));
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);

        cpu.set_register8(Register8::AL, MOUSE_HOST_CURSOR_VISIBILITY);
        cpu.set_register16(Register16::BX, 1);
        assert!(matches!(
            manager.handle_interrupt(ServiceFunction::MouseState, &mut cpu),
            Some(ServiceEvent::SetHostCursorVisibility { visible: true })
        ));
        assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);

        cpu.set_register8(Register8::AL, MOUSE_HOST_CURSOR_VISIBILITY);
        cpu.set_register16(Register16::BX, 2);
        assert!(manager
            .handle_interrupt(ServiceFunction::MouseState, &mut cpu)
            .is_none());
        assert_ne!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(
            cpu.get_register16(Register16::AX),
            ServiceError::InvalidParameter.into()
        );

        cpu.set_register8(Register8::AL, MOUSE_CONSUMER_STATUS_REPORT);
        cpu.set_register16(Register16::BX, 2);
        assert!(manager
            .handle_interrupt(ServiceFunction::MouseState, &mut cpu)
            .is_none());
        assert_ne!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(
            cpu.get_register16(Register16::AX),
            ServiceError::InvalidParameter.into()
        );

        cpu.set_register8(Register8::AL, 0xFF);
        assert!(manager
            .handle_interrupt(ServiceFunction::MouseState, &mut cpu)
            .is_none());
        assert_ne!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(
            cpu.get_register16(Register16::AX),
            ServiceError::InvalidParameter.into()
        );
    }

    #[test]
    fn mouse_state_service_reports_an_unconfigured_device() {
        let manager = ServiceInterruptManager::new(None, true);
        let mut cpu = crate::cpu_808x::Intel808x::default();

        manager.complete_mouse_state(&mut cpu, None);

        assert_ne!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(cpu.get_register16(Register16::AX), ServiceError::NotSupported.into());

        manager.complete_mouse_irq(&mut cpu, None);
        assert_ne!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(cpu.get_register16(Register16::AX), ServiceError::NotSupported.into());

        manager.complete_display_aperture_size(&mut cpu, None);
        assert_ne!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(cpu.get_register16(Register16::AX), ServiceError::NotSupported.into());

        manager.complete_mouse_consumer_range(&mut cpu, false);
        assert_ne!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(cpu.get_register16(Register16::AX), ServiceError::NotSupported.into());

        manager.complete_mouse_consumer_status(&mut cpu, false);
        assert_ne!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(cpu.get_register16(Register16::AX), ServiceError::NotSupported.into());
    }

    #[test]
    fn service_control_rejects_invalid_sentinels() {
        use crate::cpu_808x::Intel808x;

        let mut manager = ServiceInterruptManager::new(None, true);
        let mut cpu = Intel808x::default();
        cpu.set_register8(Register8::AH, ServiceFunction::ServiceControl.into());
        cpu.set_register8(Register8::AL, ServiceControl::Enable.into());
        cpu.set_register16(Register16::BX, SERVICE_CTRL_BX);
        cpu.set_register16(Register16::CX, SERVICE_CTRL_CX ^ 1);

        assert!(!is_service_control(&cpu));
        assert!(manager
            .handle_interrupt(ServiceFunction::ServiceControl, &mut cpu)
            .is_none());
        assert!(manager.enabled());
    }

    #[test]
    fn detects_and_answers_martypc_probe() {
        use crate::cpu_808x::Intel808x;

        let mut manager = ServiceInterruptManager::new(Some(0xFC), true);
        let mut cpu = Intel808x::default();
        cpu.set_register16(Register16::AX, MARTYPC_PROBE_AX);
        cpu.set_register16(Register16::BX, MARTYPC_PROBE_BX);
        cpu.set_register16(Register16::CX, MARTYPC_PROBE_CX);

        assert!(is_martypc_probe(MARTYPC_PROBE_INTERRUPT, &cpu));
        assert!(!is_martypc_probe(MARTYPC_PROBE_INTERRUPT - 1, &cpu));

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
        assert_eq!(
            MARTYPC_VERSION,
            u16::from(env!("CARGO_PKG_VERSION_MAJOR").parse::<u8>().unwrap()) << 8
                | u16::from(env!("CARGO_PKG_VERSION_MINOR").parse::<u8>().unwrap())
        );
    }
}

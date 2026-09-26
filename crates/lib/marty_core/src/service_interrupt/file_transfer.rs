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

//! Guest/host file transfer services.

use crate::cpu_common::{Cpu, Register16, Register8, ServiceEvent};

use super::{clear_carry, read_guest_u8, set_service_error, write_guest_u8, ServiceError, ServiceInterruptManager};

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

pub type FileTransferHandle = u16;

const CRC32_INITIAL: u32 = u32::MAX;
const CRC32_POLYNOMIAL: u32 = 0xEDB8_8320;
const MAX_TRANSFER_FILENAME_LEN: usize = 255;
pub(super) const INITIAL_FILE_TRANSFER_HANDLE: FileTransferHandle = 0x1000;

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
pub(super) struct PendingHostFileRequest {
    handle: FileTransferHandle,
    structure_segment: u16,
    structure_offset: u16,
    filename_segment: u16,
    filename_offset: u16,
}

#[derive(Debug, Default)]
pub(super) enum HostFileRequestState {
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

impl ServiceInterruptManager {
    /// Initiates a file transfer operation between the guest and host.
    /// Returns a 16-bit handle - multiple transfers can be in progress at once.
    pub(super) fn begin_file_transfer<C: Cpu>(&mut self, cpu: &mut C) -> Option<ServiceEvent> {
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

    pub(super) fn transfer_file_block<C: Cpu>(&mut self, cpu: &mut C) {
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

    pub(super) fn end_file_transfer<C: Cpu>(&mut self, cpu: &mut C) -> Option<ServiceEvent> {
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

#[cfg(test)]
mod tests {
    use super::{
        super::{
            test_support::{read_guest_bytes, write_guest_bytes},
            ServiceFunction,
            CARRY_FLAG,
        },
        *,
    };

    const TEST_STRUCTURE_SEGMENT: u16 = 0x1000;
    const TEST_STRUCTURE_OFFSET: u16 = 0x0100;
    const TEST_FILENAME_OFFSET: u16 = 0x0200;
    const TEST_BUFFER_OFFSET: u16 = 0x0300;

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
    fn reused_handles_keep_transfers_separate() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let first = manager
            .create_file_transfer_operation("first.bin", 1, FileTransferDirection::GuestToHost)
            .unwrap();
        let second = manager
            .create_file_transfer_operation("second.bin", 1, FileTransferDirection::GuestToHost)
            .unwrap();

        manager.destroy_file_transfer_operation(first).unwrap();
        assert!(manager.file_transfer_operation(first).is_none());
        // Repeated cleanup must not add the same handle to the free list twice.
        assert!(manager.destroy_file_transfer_operation(first).is_none());
        let replacement = manager
            .create_file_transfer_operation("replacement.bin", 1, FileTransferDirection::GuestToHost)
            .unwrap();

        assert_eq!(replacement, first);
        assert_ne!(replacement, second);
        assert_eq!(
            manager.file_transfer_operation(second).unwrap().filename(),
            "second.bin"
        );
        let another = manager
            .create_file_transfer_operation("another.bin", 1, FileTransferDirection::GuestToHost)
            .unwrap();
        assert_ne!(another, replacement);
        assert_ne!(another, second);
        assert_eq!(
            manager.file_transfer_operation(replacement).unwrap().filename(),
            "replacement.bin"
        );
    }

    #[test]
    fn upload_joins_blocks_and_checksums() {
        for non_interactive in [false, true] {
            let mut manager = ServiceInterruptManager::new(None, true);
            let mut cpu = crate::cpu_808x::Intel808x::default();
            let flags = FILE_TRANSFER_GUEST_TO_HOST
                | if non_interactive {
                    FILE_TRANSFER_NON_INTERACTIVE
                }
                else {
                    0
                };
            let handle = begin_transfer(&mut manager, &mut cpu, flags, "OUTPUT.BIN", 9).unwrap();

            transfer_guest_block(&mut manager, &mut cpu, handle, b"1234");
            assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);
            assert_eq!(cpu.get_register16(Register16::AX), 4);
            transfer_guest_block(&mut manager, &mut cpu, handle, b"56789");

            cpu.set_register16(Register16::BX, handle);
            cpu.set_register8(Register8::AL, FILE_TRANSFER_COMMIT);
            let event = manager.handle_interrupt(ServiceFunction::FileTransferEnd, &mut cpu);

            assert_eq!(cpu.get_flags() & CARRY_FLAG, 0);
            // Standard CRC-32 check value, independent of the implementation under test.
            assert_eq!(
                u32::from(cpu.get_register16(Register16::DX)) << 16 | u32::from(cpu.get_register16(Register16::CX)),
                0xCBF4_3926
            );
            assert!(manager.file_transfer_operation(handle).is_none());
            match event {
                Some(ServiceEvent::GuestFileTransferComplete {
                    filename,
                    data,
                    non_interactive: mode,
                }) => {
                    assert_eq!(filename, "OUTPUT.BIN");
                    assert_eq!(data, b"123456789");
                    assert_eq!(mode, non_interactive);
                }
                _ => panic!("expected a committed guest file transfer"),
            }
            assert_eq!(
                manager.create_file_transfer_operation("NEXT.BIN", 1, FileTransferDirection::GuestToHost),
                Some(handle)
            );
        }
    }

    #[test]
    fn incomplete_upload_can_resume() {
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
        assert_eq!(cpu.get_register16(Register16::AX), u16::from(ServiceError::InvalidData));
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
    fn transfer_accepts_full_u32_size() {
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
    fn aborted_upload_discards_data() {
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
    fn download_waits_then_copies_blocks() {
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
        assert_eq!(
            read_guest_bytes(&mut cpu, TEST_STRUCTURE_SEGMENT, TEST_STRUCTURE_OFFSET + 8, 2),
            u16::from(FileTransferStatus::Wait).to_le_bytes()
        );

        // A pending request cannot transfer data until the host has supplied a file.
        cpu.set_register16(Register16::BX, handle);
        cpu.set_register16(Register16::CX, 1);
        cpu.set_register16(Register16::DI, TEST_BUFFER_OFFSET);
        manager.handle_interrupt(ServiceFunction::FileTransferBlock, &mut cpu);
        assert_ne!(cpu.get_flags() & CARRY_FLAG, 0);
        assert_eq!(cpu.get_register16(Register16::AX), u16::from(ServiceError::Busy));
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
            0x083C_031F
        );
        assert!(manager.file_transfer_operation(handle).is_none());
    }

    #[test]
    fn download_requests_named_file() {
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
    fn transfer_rejects_unknown_flags() {
        let mut manager = ServiceInterruptManager::new(None, true);
        let mut cpu = crate::cpu_808x::Intel808x::default();

        assert!(begin_transfer(&mut manager, &mut cpu, 0x80, "INVALID.DAT", 1).is_none());
        assert_eq!(
            cpu.get_register16(Register16::AX),
            u16::from(ServiceError::InvalidParameter)
        );
    }

    #[test]
    fn failed_download_reports_status() {
        for (non_interactive, expected) in [
            (false, FileTransferStatus::Aborted),
            (true, FileTransferStatus::HostFileNotFound),
        ] {
            let mut manager = ServiceInterruptManager::new(None, true);
            let mut cpu = crate::cpu_808x::Intel808x::default();
            let flags = FILE_TRANSFER_HOST_TO_GUEST
                | if non_interactive {
                    FILE_TRANSFER_NON_INTERACTIVE
                }
                else {
                    0
                };
            let filename = if non_interactive { "MISSING.BIN" } else { "" };
            let handle = begin_transfer(&mut manager, &mut cpu, flags, filename, 0).unwrap();
            if non_interactive {
                manager.host_file_not_found(&mut cpu).unwrap();
            }
            else {
                manager.abort_host_file_request(&mut cpu).unwrap();
            }
            assert_eq!(
                read_guest_bytes(&mut cpu, TEST_STRUCTURE_SEGMENT, TEST_STRUCTURE_OFFSET + 8, 2),
                u16::from(expected).to_le_bytes()
            );
            assert!(manager.file_transfer_operation(handle).is_some());
        }
    }
}

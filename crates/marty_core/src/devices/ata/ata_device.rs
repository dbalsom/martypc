/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2025 Daniel Balsom

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

//! An AtaDevice implements an ATA storage device, typically a hard disk.

use crate::{
    bus::BusInterface,
    device_types::{chs::DiskChs, disk::Disk},
    devices::{
        ata::{
            ata_error::{AtaError, AtaOperationError},
            ata_identification::AtaDriveIdentification,
            ata_register16::AtaRegister16,
        },
        dma,
    },
    vhd::VirtualHardDisk,
};
use binrw::BinWrite;
use fluxfox::io::ReadBytesExt;
use modular_bitfield::bitfield;
use std::{
    collections::VecDeque,
    io::{Cursor, Seek, SeekFrom, Write},
};

const ATA_RESET_DELAY_US: f64 = 200_000.0; // 200ms
const ENABLE_DMA_MASK: u8 = 0x01;
const ENABLE_IRQ_MASK: u8 = 0x02;

const DRIVE_HEAD_BITS_ON: u8 = 0xA0; // 1010 0000
const DRIVE_HEAD_LBA_BIT: u8 = 0x40; // 0100 0000
pub const DEFAULT_SECTOR_SIZE: usize = 512;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum AtaDeviceType {
    #[default]
    HardDisk,
    CdRom,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub enum AtaState {
    Reset,
    WaitingForCommand,
    ReceivingCommand,
    ExecutingCommand,
    HaveCommandResult,
    HaveCommandStatus,
    HaveSenseBytes,
}

#[repr(u8)]
#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub enum AtaCommand {
    None,
    ReadSectorRetry = 0x20,
    ReadSector = 0x21,
    ReadVerifySector = 0x40,
    WriteSector = 0x30,
    Recalibrate = 0x10,
    Seek = 0x70,
    IdentifyDrive = 0xEC,
    SetFeatures = 0xEF,
    ReadMultiple = 0xC4,
    WriteMultiple = 0xC5,
    ReadMultipleMode,
}

type CommandDispatchFn = fn(&mut AtaDevice, Option<&mut BusInterface>) -> Continuation;

pub enum Continuation {
    CommandComplete,
    ContinueAsOperation,
}

#[bitfield]
#[derive(Copy, Clone, Debug)]
pub struct AtaStatusRegister {
    pub err:   bool, // Error
    pub idx:   bool, // Index
    pub corr:  bool, // Corrected Data
    pub drq:   bool, // Data Request
    pub dsc:   bool, // Disk Seek Complete
    pub dwf:   bool, // Drive Write Failure
    pub ready: bool, // Drive Ready
    pub busy:  bool, // Drive Busy
}

#[bitfield]
#[derive(Copy, Clone, Debug)]
pub struct AtaErrorRegister {
    pub amnf: bool, // Address Mark Not Found
    pub tk0:  bool, // Track 0 Not Found
    pub abrt: bool, // Command Aborted
    pub mcr:  bool, // Media Change Request
    pub idnf: bool, // ID Not Found
    pub mc:   bool, // Media changed
    pub unc:  bool, // Unrecoverable
    pub bbk:  bool, // Bad Block
}

#[allow(dead_code)]
#[derive(Default, Debug, Clone)]
pub struct OperationStatus {
    sectors_complete: u8,
    sectors_left: u8,
    block_ct: u8,
    block_n: u8,
    dma_bytes_left: usize,
    dma_byte_count: usize,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct AtaDevice {
    disk_idx: usize,
    disk: Option<Disk>,
    irq: Option<u8>,
    lba: bool,
    dma: bool,
    dma_channel: Option<u8>,
    state: AtaState,
    last_error: AtaOperationError,
    last_error_drive: usize,
    error_flag: bool,
    receiving_dcb: bool,
    command: AtaCommand,
    command_chs: DiskChs,
    command_lba: u32,
    command_fn: Option<CommandDispatchFn>,
    last_command: AtaCommand,
    command_byte_n: u32,
    command_queue: VecDeque<u8>,
    command_result_pending: bool,

    sector_buffer_idx: usize,
    sector_buffer: Cursor<Vec<u8>>,
    status_register: AtaStatusRegister,
    error_register: AtaErrorRegister,
    sector_count_register: u8,
    sector_number_register: u8,
    cylinder_low_register: u8,
    cylinder_high_register: u8,
    drive_head_register: u8,

    status_reads:  u64,
    data_reads:    u64,
    data_writes:   u64,
    data_register: AtaRegister16,

    operation_status: OperationStatus,

    dma_enabled: bool,
    irq_enabled: bool,

    send_interrupt: bool,
    clear_interrupt: bool,
    interrupt_active: bool,
    send_dreq: bool,
    clear_dreq: bool,
    dreq_active: bool,

    state_accumulator: f64,
}

impl Default for AtaDevice {
    fn default() -> Self {
        let mut status_register = AtaStatusRegister::from_bytes([0]);
        status_register.set_ready(true);
        let error_register = AtaErrorRegister::from_bytes([0]);

        Self {
            disk_idx: 0xFF,
            disk: None,
            irq: None,
            lba: false,
            dma: false,
            dma_channel: None,
            state: AtaState::Reset,
            last_error: AtaOperationError::NoError,
            last_error_drive: 0,
            error_flag: false,
            receiving_dcb: false,
            command: AtaCommand::None,
            command_chs: DiskChs::new(0, 0, 1),
            command_lba: 0,
            command_fn: None,
            last_command: AtaCommand::None,
            command_byte_n: 0,
            command_queue: VecDeque::new(),
            command_result_pending: false,
            sector_buffer_idx: DEFAULT_SECTOR_SIZE,
            sector_buffer: Cursor::new(vec![0; DEFAULT_SECTOR_SIZE]),
            status_register,
            error_register,
            sector_count_register: 1,
            sector_number_register: 1,
            cylinder_low_register: 0,
            cylinder_high_register: 0,
            drive_head_register: 0,

            status_reads: 0,
            data_reads: 0,
            data_writes: 0,
            data_register: AtaRegister16::new(),
            operation_status: OperationStatus::default(),
            dma_enabled: false,
            irq_enabled: false,
            send_interrupt: false,
            clear_interrupt: false,
            interrupt_active: false,
            send_dreq: false,
            clear_dreq: false,
            dreq_active: false,

            state_accumulator: 0.0,
        }
    }
}

impl AtaDevice {
    pub fn new(disk_idx: usize, disk: Disk, irq: Option<u8>, lba: bool, dma_channel: Option<u8>) -> Self {
        Self {
            disk_idx,
            disk: Some(disk),
            irq,
            lba,
            dma_channel,
            ..Default::default()
        }
    }

    pub fn reset(&mut self) {
        log::trace!("Resetting AtaDevice...");
        self.command_queue.clear();
        self.command_result_pending = false;
        self.command_byte_n = 0;

        self.interrupt_active = false;
        self.send_interrupt = false;
        self.send_dreq = false;
        self.state = AtaState::Reset;
        self.state_accumulator = 0.0;

        self.sector_count_register = 1;
        self.sector_number_register = 1;
        self.cylinder_low_register = 0;
        self.cylinder_high_register = 0;

        self.receiving_dcb = false;
        self.command = AtaCommand::None;
        self.command_fn = None;
        self.command_byte_n = 0;

        self.status_reads = 0;
        self.data_reads = 0;
        self.data_writes = 0;
        self.data_register = AtaRegister16::new();
    }

    pub fn set_vhd(&mut self, vhd: VirtualHardDisk) -> Result<(), AtaError> {
        if let Some(disk) = &mut self.disk {
            disk.set_vhd(vhd)
        }
        else {
            self.disk = Some(Disk::from_vhd(vhd));
        }

        Ok(())
    }

    pub fn unload_vhd(&mut self) {
        if let Some(disk) = &mut self.disk {
            disk.unload_vhd();
        }
    }

    pub fn disk(&self) -> Option<&Disk> {
        self.disk.as_ref()
    }

    pub fn disk_mut(&mut self) -> Option<&mut Disk> {
        self.disk.as_mut()
    }

    pub fn set_command(&mut self, command: AtaCommand, n_bytes: u32, command_fn: CommandDispatchFn) {
        self.state = AtaState::ReceivingCommand;
        self.receiving_dcb = true;
        self.command = command;
        self.command_fn = Some(command_fn);
        self.command_byte_n = n_bytes;
    }

    fn set_command_address(&mut self) {
        if self.drive_head_register & DRIVE_HEAD_LBA_BIT != 0 {
            self.construct_lba_address();
        }
        else {
            self.set_command_chs();
        }
    }

    fn set_command_chs(&mut self) {
        self.command_chs = DiskChs::new(
            self.cylinder_reg(),
            self.drive_head_register & 0x0F,
            self.sector_number_register,
        );
    }

    pub fn set_error(&mut self, error: AtaOperationError) {
        self.last_error = error;
        match error {
            AtaOperationError::NoError => self.error_flag = false,
            _ => self.error_flag = true,
        }
    }

    /// Handle a write to the Controller Select Pulse register
    pub fn handle_controller_select(&self, byte: u8) {
        // Byte written to pulse register ignored?
        // Not entirely sure the purpose of this register, but it may be used to coordinate multiple disk controllers
        log::trace!("Controller select: {:02X}", byte);
    }

    #[inline]
    pub fn error_register_read(&self) -> u8 {
        self.error_register.into_bytes()[0]
    }

    #[inline]
    pub fn sector_count_register_read(&self) -> u8 {
        self.sector_count_register
    }

    pub fn sector_count_register_write(&mut self, byte: u8) {
        self.sector_count_register = byte;
    }

    #[inline]
    pub fn sector_number_register_read(&self) -> u8 {
        self.sector_number_register
    }

    pub fn sector_number_register_write(&mut self, byte: u8) {
        self.sector_number_register = byte;
    }

    #[inline]
    pub fn cylinder_low_register_read(&self) -> u8 {
        self.cylinder_low_register
    }

    pub fn cylinder_low_register_write(&mut self, byte: u8) {
        self.cylinder_low_register = byte;
    }

    #[inline]
    pub fn cylinder_high_register_read(&self) -> u8 {
        self.cylinder_high_register
    }

    pub fn cylinder_high_register_write(&mut self, byte: u8) {
        self.cylinder_high_register = byte;
    }

    pub fn drive_head_register_read(&self) -> u8 {
        self.drive_head_register
    }

    pub fn drive_head_register_write(&mut self, byte: u8) {
        self.drive_head_register = byte | DRIVE_HEAD_BITS_ON;
    }

    fn construct_lba_address(&mut self) {
        self.command_lba = self.sector_number_register as u32;
        self.command_lba |= (self.cylinder_low_register as u32) << 8;
        self.command_lba |= (self.cylinder_high_register as u32) << 16;
        self.command_lba |= ((self.drive_head_register & 0x0F) as u32) << 24;

        if self.drive_head_register & DRIVE_HEAD_LBA_BIT != 0 {
            if let Some(disk) = self.disk.as_mut() {
                if let Some(chs) = DiskChs::from_lba(self.command_lba as usize, &disk.geometry()) {
                    self.command_chs = chs;
                }
                else {
                    log::error!("LBA address out of range: {}", self.command_lba);
                }
            }
            else {
                log::error!("Disk not set for LBA address calculation");
            }
            self.lba = true;
        }
        else {
            self.lba = false;
        }
    }

    fn distribute_lba_address(&mut self) {
        self.sector_number_register = (self.command_lba & 0xFF) as u8;
        self.cylinder_low_register = ((self.command_lba >> 8) & 0xFF) as u8;
        self.cylinder_high_register = ((self.command_lba >> 16) & 0xFF) as u8;
        self.drive_head_register = (self.drive_head_register & 0xF0) | ((self.command_lba >> 24) & 0x0F) as u8;
    }

    pub fn register_read(&mut self, reg: u8) -> u8 {
        match reg {
            0x00 => self.data_register_read(),
            0x01 => self.error_register_read(),
            0x02 => self.sector_count_register_read(),
            0x03 => self.sector_number_register_read(),
            0x04 => self.cylinder_low_register_read(),
            0x05 => self.cylinder_high_register_read(),
            0x06 => self.drive_head_register_read(),
            0x07 => self.status_register_read(),
            _ => {
                log::error!("Unknown register read: {reg}");
                0
            }
        }
    }

    pub fn register_write(&mut self, reg: u8, byte: u8, bus: Option<&mut BusInterface>) {
        match reg {
            0x00 => self.data_register_write(byte, true),
            0x01 => self.mask_register_write(byte),
            0x02 => self.sector_count_register_write(byte),
            0x03 => self.sector_number_register_write(byte),
            0x04 => self.cylinder_low_register_write(byte),
            0x05 => self.cylinder_high_register_write(byte),
            0x06 => self.drive_head_register_write(byte),
            0x07 => self.handle_command_register_write(byte, bus),
            _ => {
                log::error!("Unknown register write: {reg}");
            }
        }
    }

    /// Read from the Data Register
    ///
    /// Sense Bytes can be read after a Request Sense command, or the Status Byte otherwise
    pub fn data_register_read(&mut self) -> u8 {
        self.data_reads += 1;
        let mut byte = 0;

        if !self.status_register.drq() {
            log::warn!("Data Register read with DRQ not set");
            return 0;
        }

        let cursor_pos = self.sector_buffer.stream_position().unwrap();
        if cursor_pos < (DEFAULT_SECTOR_SIZE as u64) {
            byte = self.sector_buffer.read_u8().unwrap_or_else(|e| {
                log::error!("Error reading from sector buffer: {e}");
                0
            });
            if cursor_pos == (DEFAULT_SECTOR_SIZE as u64 - 1) {
                log::debug!("Sector buffer read complete");
                self.status_register.set_drq(false);
            }
        }
        else {
            log::warn!("Trying to read empty sector buffer!");
        }
        byte
    }

    /// Read write to the Data Register
    ///
    /// Sense Bytes can be read after a Request Sense command, or the Status Byte otherwise
    pub fn data_register_write(&mut self, byte: u8, low: bool) {
        if !self.status_register.drq() {
            log::warn!("Data Register written with DRQ not set");
            return;
        }

        match low {
            true => {
                self.data_register.set_lo(byte);
            }
            false => {
                self.data_register.set_hi(byte);
            }
        }

        if self.data_register.ready() {
            let bytes = self.data_register.get_bytes().unwrap();
            //log::trace!("Data Register write complete: {:X?}", bytes);

            let cursor_pos = self.sector_buffer.stream_position().unwrap();
            if cursor_pos < (DEFAULT_SECTOR_SIZE as u64 - 1) {
                if let Err(e) = self.sector_buffer.write(&bytes) {
                    log::error!("Error writing to sector buffer: {e}");
                }
                if cursor_pos == (DEFAULT_SECTOR_SIZE as u64 - 2) {
                    log::debug!("Sector buffer write complete");
                    self.status_register.set_drq(false);
                }
            }
            else {
                log::warn!("Trying to write to full sector buffer!");
            }
        }
    }

    /// Handle a write to the DMA and interrupt mask register
    pub fn mask_register_write(&mut self, byte: u8) {
        self.irq_enabled = byte & ENABLE_IRQ_MASK != 0;
        self.dma_enabled = byte & ENABLE_DMA_MASK != 0;
        log::trace!(
            "Write to Mask Register. IRQ enabled: {} DMA enabled: {}",
            self.irq_enabled,
            self.dma_enabled
        );

        // Write to mask register puts us in Waiting For Command state
        self.state = AtaState::WaitingForCommand;
    }

    /// Handle a write to the command register
    pub fn handle_command_register_write(&mut self, byte: u8, bus: Option<&mut BusInterface>) {
        log::warn!("Got command byte: {:02X}", byte);
        // Transition from other states. It's possible that we don't check the error code
        // after an operation
        if let AtaState::HaveCommandStatus = self.state {
            log::warn!("Received command with pending unread status register");
            self.state = AtaState::WaitingForCommand;
        }

        match self.state {
            /* Certain commands can be completed instantly - in the absence of emulated delays that the real hardware might have.
            We distinguish between Commands and Operations, whereas some Commands are executed immediately and considered complete
            a Command may initiate an Operation by returning the appropriate enum.

            An Operation is an ongoing command that may take some period of time to complete, such as a DMA transfer.
            Operations are ticked during calls to run() on the XtIdeController device. Operations must be properly
            terminated when complete.

            Here we match a command callback to a specified Command received in a DCB, it will be dispatched when all bytes of the
            command have been received.
            */
            AtaState::WaitingForCommand => {
                if self.interrupt_active {
                    log::warn!(" >>> Received command with interrupt active")
                }

                match byte {
                    0x00 => {
                        log::debug!("NOP command received");
                    }
                    0x20 => {
                        log::debug!("Read Sector(s) (Retry) command received");
                        self.set_command(AtaCommand::ReadSectorRetry, 0, AtaDevice::command_read_sectors_retry);
                        self.process_command_byte(0, bus);
                    }
                    0x21 => {
                        log::debug!("Read Sector(s) command received");
                        self.set_command(AtaCommand::ReadSector, 0, AtaDevice::command_read_sectors);
                        self.process_command_byte(0, bus);
                    }
                    0x30 => {
                        log::debug!("Write Sector(s) command received");
                        self.set_command(AtaCommand::WriteSector, 0, AtaDevice::command_write_sectors);
                        self.process_command_byte(0, bus);
                    }
                    0x40 => {
                        log::debug!("Read Verify Sector(s) command received");
                        self.set_command(AtaCommand::ReadVerifySector, 0, AtaDevice::command_read_verify_sectors);
                        self.process_command_byte(0, bus);
                    }
                    0x70..=0x7F => {
                        log::debug!("Seek command received");
                    }
                    0xE4 => {
                        log::debug!("Read buffer received");
                    }
                    0xEC => {
                        log::debug!("Identify Drive command received");
                        self.set_command(AtaCommand::IdentifyDrive, 0, AtaDevice::command_identify_drive);
                        self.process_command_byte(0, bus);
                    }
                    0xEF => {
                        log::debug!("Set Features command received");
                    }
                    0xC4 => {
                        log::debug!("Read Multiple received");
                        self.set_command(AtaCommand::ReadMultiple, 0, AtaDevice::command_read_multiple);
                        self.process_command_byte(0, bus);
                    }
                    0xC5 => {
                        log::debug!("Write Multiple received");
                        self.set_command(AtaCommand::WriteMultiple, 0, AtaDevice::command_write_multiple);
                        self.process_command_byte(0, bus);
                    }
                    0xC6 => {
                        log::debug!("Set Multiple Mode received");
                        self.set_command(AtaCommand::ReadMultipleMode, 0, AtaDevice::command_set_multiple_mode);
                        self.process_command_byte(0, bus);
                    }
                    0x91 => {
                        log::debug!("Initialize Drive Paramters received");
                    }
                    _ => {
                        log::error!("Unknown command received: {:02X}", byte);
                        // Unknown Command
                    }
                }
            }
            AtaState::ReceivingCommand => {
                // If we are expecting another byte for this command, read it in.
                self.process_command_byte(byte, bus);
            }
            _ => {
                log::error!("Unexpected write to command register in state: {:?}", self.state);
            }
        }
    }

    pub fn process_command_byte(&mut self, byte: u8, bus: Option<&mut BusInterface>) {
        if self.command_byte_n > 0 {
            self.command_queue.push_back(byte);
            //log::trace!("Remaining command bytes: {}", self.command_byte_n );
            self.command_byte_n -= 1;
        }

        if self.command_byte_n == 0 {
            // We read last byte expected for this command, so dispatch to the appropriate command handler
            let mut result = Continuation::CommandComplete;

            match self.command_fn {
                None => {
                    log::error!("No associated method for command: {:?}!", self.command);
                    self.error_register.set_abrt(true);
                }
                Some(command_fn) => {
                    self.error_register.set_abrt(false);
                    result = command_fn(self, bus);
                }
            }

            match result {
                Continuation::CommandComplete => {
                    // Allow commands to ignore unneeded bytes in DCB by clearing it now
                    self.command_queue.clear();

                    self.last_command = self.command;
                    self.command = AtaCommand::None;
                    self.command_fn = None;
                    self.state = AtaState::HaveCommandStatus;
                }
                Continuation::ContinueAsOperation => {
                    log::debug!("Command will continue as operation");
                    self.state = AtaState::ExecutingCommand;
                }
            }
        }
    }

    pub fn status_register_read(&mut self) -> u8 {
        //log::debug!("Status Register read: {:02X}", self.status_register.into_bytes()[0]);
        self.clear_interrupt = true;
        self.status_register.into_bytes()[0]
    }

    fn cylinder_reg(&self) -> u16 {
        (self.cylinder_high_register as u16) << 8 | self.cylinder_low_register as u16
    }

    fn clear_buffer(&mut self) {
        self.sector_buffer.get_mut().fill(0);
        self.sector_buffer.seek(SeekFrom::Start(0)).unwrap();
    }

    pub fn sector_buffer_mark_read(&mut self) {
        // Mark the sector buffer as read
        self.sector_buffer.seek(SeekFrom::End(1)).unwrap();
        self.status_register.set_drq(false);
    }

    pub fn sector_buffer_end(&mut self) -> bool {
        let pos = self.sector_buffer.stream_position().unwrap();
        pos > (DEFAULT_SECTOR_SIZE as u64 - 1)
    }

    pub fn sector_buffer_start(&mut self) -> bool {
        let pos = self.sector_buffer.stream_position().unwrap();
        pos == 0
    }

    pub fn sector_buffer(&self) -> &[u8] {
        self.sector_buffer.get_ref()
    }

    pub fn sector_buffer_mut(&mut self) -> &mut Vec<u8> {
        self.sector_buffer.get_mut()
    }

    /// ATA command: Identify Drive
    fn command_identify_drive(&mut self, _bus: Option<&mut BusInterface>) -> Continuation {
        log::debug!("command_identify_drive()");
        log::debug!("sector buffer size: {}", self.sector_buffer.get_ref().len());
        // Set the DRQ flag

        // Normally the controller would set BSY while processing, but this happens instantaneously
        // here.
        //self.status_register.set_busy(true);

        if let Some(_disk) = self.disk.as_ref() {
            let geometry = self.disk.as_ref().unwrap().geometry();
            log::debug!(
                "Writing Drive Identification block to sector buffer with geometry: {:?}",
                geometry
            );
            let id_blob = AtaDriveIdentification::new(&geometry, DEFAULT_SECTOR_SIZE, self.lba, self.dma);
            self.clear_buffer();
            self.sector_buffer.seek(SeekFrom::Start(0)).unwrap();
            match id_blob.write(&mut self.sector_buffer) {
                Ok(_) => {
                    log::debug!("Drive Identification block written to sector buffer");
                    self.status_register.set_ready(true);
                    self.status_register.set_drq(true);
                    self.sector_buffer.seek(SeekFrom::Start(0)).unwrap();
                }
                Err(e) => {
                    log::error!("Error writing Drive Identification block to sector buffer: {}", e);
                    self.error_register.set_abrt(true);
                }
            }
        }
        else {
            log::warn!("command_identify_drive(): No disk image!");
            self.error_register.set_abrt(true);
        }

        //bus.pic_mut().as_mut().unwrap().request_interrupt(HDC_IRQ);
        Continuation::CommandComplete
    }

    /// ATA command: Set Multiple Mode
    fn command_set_multiple_mode(&mut self, _bus: Option<&mut BusInterface>) -> Continuation {
        log::debug!(
            "command_set_multiple_mode(): sectors_per_block: {} device: {}",
            self.sector_count_register,
            (self.drive_head_register & 0x10) >> 4
        );

        Continuation::CommandComplete
    }

    /// ATA command 0x21: Read Sector(s)
    fn command_read_sectors(&mut self, _bus: Option<&mut BusInterface>) -> Continuation {
        self.set_command_address();
        log::debug!(
            "command_read_sectors(): sector_count: {} chs: {}",
            self.sector_count_register,
            self.command_chs,
        );

        self.disk.as_mut().unwrap().seek(self.command_chs);
        self.read_sector_into_buffer(false);

        if self.sector_count_register > 1 {
            self.operation_status = OperationStatus {
                sectors_complete: 1,
                sectors_left: self.sector_count_register - 1,
                ..Default::default()
            };
            // Continue to transfer other sectors as they are read
            //self.status_register.set_busy(true);
            Continuation::ContinueAsOperation
        }
        else {
            // One sector transferred, command complete
            Continuation::CommandComplete
        }
    }

    /// ATA command 0x20: Read Sector(s) (Retry)
    fn command_read_sectors_retry(&mut self, _bus: Option<&mut BusInterface>) -> Continuation {
        self.set_command_address();

        log::debug!(
            "command_read_sectors_retry(): sector_count: {} chs: {} lba: {}",
            self.sector_count_register,
            self.command_chs,
            self.command_lba
        );

        self.disk.as_mut().unwrap().seek(self.command_chs);
        self.read_sector_into_buffer(true);

        if self.sector_count_register > 1 {
            self.operation_status = OperationStatus {
                sectors_complete: 1,
                sectors_left: self.sector_count_register - 1,
                ..Default::default()
            };
            // Continue to transfer other sectors as they are read
            //self.status_register.set_busy(true);
            Continuation::ContinueAsOperation
        }
        else {
            // One sector transferred, command complete
            Continuation::CommandComplete
        }
    }

    /// ATA command 0x40: Read Verify Sector(s)
    fn command_read_verify_sectors(&mut self, _bus: Option<&mut BusInterface>) -> Continuation {
        self.set_command_chs();
        log::debug!(
            "command_read_sectors_verify(): sector_count: {} chs: {}",
            self.sector_count_register,
            self.command_chs,
        );

        self.disk.as_mut().unwrap().seek(self.command_chs);
        //self.read_sector_into_buffer(self.drive_select, true);

        if self.sector_count_register > 1 {
            self.operation_status = OperationStatus {
                sectors_complete: 1,
                sectors_left: self.sector_count_register - 1,
                ..Default::default()
            };
            // Continue to transfer other sectors as they are read
            //self.status_register.set_busy(true);
            Continuation::ContinueAsOperation
        }
        else {
            // One sector transferred, command complete
            Continuation::CommandComplete
        }
    }

    /// ATA command: Read Multiple
    fn command_read_multiple(&mut self, _bus: Option<&mut BusInterface>) -> Continuation {
        self.set_command_chs();
        log::debug!(
            "command_read_multiple(): sectors: {} lba: {} chs: {}",
            self.sector_count_register,
            (self.drive_head_register & 0x40) >> 6,
            self.command_chs,
        );

        self.disk.as_mut().unwrap().seek(self.command_chs);
        self.read_sector_into_buffer(true);

        if self.sector_count_register > 1 {
            self.operation_status = OperationStatus {
                sectors_complete: 1,
                sectors_left: self.sector_count_register - 1,
                ..Default::default()
            };
            // Continue to transfer other sectors as they are read
            //self.status_register.set_busy(true);
            Continuation::ContinueAsOperation
        }
        else {
            // One sector transferred, command complete
            Continuation::CommandComplete
        }
    }

    /// ATA command 0x30: Write Sector(s)
    fn command_write_sectors(&mut self, _bus: Option<&mut BusInterface>) -> Continuation {
        self.set_command_chs();
        log::debug!(
            "command_write_sectors(): sector_count: {} chs: {}",
            self.sector_count_register,
            self.command_chs,
        );

        self.operation_status = OperationStatus {
            sectors_complete: 0,
            sectors_left: self.sector_count_register,
            ..Default::default()
        };

        self.clear_buffer();
        self.status_register.set_drq(true);
        self.disk.as_mut().unwrap().seek(self.command_chs);
        Continuation::ContinueAsOperation
    }

    /// ATA command 0xC5 Write Multiple
    fn command_write_multiple(&mut self, _bus: Option<&mut BusInterface>) -> Continuation {
        self.set_command_chs();
        log::debug!(
            "command_write_multiple(): sector_count: {} chs: {}",
            self.sector_count_register,
            self.command_chs,
        );

        self.operation_status = OperationStatus {
            sectors_complete: 0,
            sectors_left: self.sector_count_register,
            ..Default::default()
        };

        self.clear_buffer();
        self.status_register.set_drq(true);
        self.disk.as_mut().unwrap().seek(self.command_chs);
        Continuation::ContinueAsOperation
    }

    /// Perform the Sense Status command
    fn command_sense_status(&mut self, _bus: &mut BusInterface) -> Continuation {
        // let dcb = self.read_dcb();
        // self.data_register_in.clear();
        //
        // let byte0 = match self.last_error {
        //     OperationError::NoError => 0,
        //     OperationError::NoReadySignal => ERR_NO_READY_SIGNAL,
        //     OperationError::InvalidCommand => ERR_INVALID_COMMAND,
        //     OperationError::IllegalAccess => ERR_ILLEGAL_ACCESS,
        // };
        //
        // /* The controller BIOS source listing provides the following table for sense byte format
        //     ;---------------------------------------------------;
        //     ;                 SENSE STATUS BYTES                ;
        //     ;                                                   ;
        //     ;       BYTE 0                                      ;
        //     ;           BIT     7   ADDRESS VALID, WHEN SET     ;
        //     ;           BIT     6   SPARE, SET TO ZERO          ;
        //     ;           BITS  5-4   ERROR TYPE                  ;
        //     ;           BITS  3-0   ERROR CODE                  ;
        //     ;                                                   ;
        //     ;      BYTE 1                                       ;
        //     ;           BITS  7-6   ZERO                        ;
        //     ;           BIT     5   DRIVE (0-1)                 ;
        //     ;           BITS  4-0   HEAD NUMBER                 ;
        //     ;                                                   ;
        //     ;      BYTE 2                                       ;
        //     ;           BITS  7-5   CYLINDER HIGH               ;
        //     ;           BITS  4-0   SECTOR NUMBER               ;
        //     ;                                                   ;
        //     ;      BYTE 3                                       ;
        //     ;           BITS  7-0   CYLINDER LOW                ;
        //     ;---------------------------------------------------;
        //
        //     Certain fields like sector number vary in size compared to the equivalent fields in the DCB.
        // */
        // let byte1 = (dcb.drive_select << 5) as u8 | (self.drives[dcb.drive_select].head & 0x1F);
        // let byte2 =
        //     (self.drives[dcb.drive_select].cylinder & 0x700 >> 3) as u8 | self.drives[dcb.drive_select].sector & 0x1F;
        // let byte3 = (self.drives[dcb.drive_select].cylinder & 0xFF) as u8;
        //
        // self.data_register_out.push_back(byte0);
        // self.data_register_out.push_back(byte1);
        // self.data_register_out.push_back(byte2);
        // self.data_register_out.push_back(byte3);
        //
        // self.set_error(OperationError::NoError, dcb.drive_select);
        // self.send_interrupt = true;
        Continuation::CommandComplete
    }

    /// Perform the Read Sector command.
    fn command_read(&mut self, _bus: &mut BusInterface) -> Continuation {
        // let dcb = self.read_dcb();
        // self.data_register_in.clear();
        //
        // let xfer_size = bus.dma_mut().as_mut().unwrap().get_dma_transfer_size(HDC_DMA);
        // log::trace!(
        //     "Command Read: drive: {} c: {} h: {} s: {}, xfer_size:{}",
        //     dcb.drive_select,
        //     dcb.c,
        //     dcb.h,
        //     dcb.s,
        //     xfer_size
        // );
        //
        // // Prime the Sector Buffer with an intitial sector read
        // match &mut self.drives[dcb.drive_select].vhd {
        //     Some(vhd) => {
        //         if let Err(e) = vhd.read_sector(&mut self.drives[dcb.drive_select].sector_buf, dcb.c, dcb.h, dcb.s) {
        //             log::error!(
        //                 "VHD read_sector() failed: c:{} h:{} s:{} Error: {}",
        //                 dcb.c,
        //                 dcb.h,
        //                 dcb.s,
        //                 e
        //             );
        //         }
        //     }
        //     None => {
        //         // No VHD? Handle error stage for read command
        //     }
        // }
        //
        // if xfer_size % SECTOR_SIZE != 0 {
        //     log::warn!("Command Read: DMA word count not multiple of sector size");
        // }
        //
        // self.drive_select = dcb.drive_select;
        //
        // // Check drive status
        // if self.drive_present(dcb.drive_select) {
        //     self.set_error(OperationError::NoError, dcb.drive_select);
        //
        //     // Set up Operation
        //     self.operation_status.buffer_idx = 0;
        //     self.drives[self.drive_select].cylinder = dcb.c;
        //     self.drives[self.drive_select].head = dcb.h;
        //     self.drives[self.drive_select].sector = dcb.s;
        //     //self.command_status.block_ct = block_count;
        //     self.operation_status.block_n = 0;
        //     self.operation_status.dma_bytes_left = xfer_size;
        //     self.operation_status.dma_byte_count = 0;
        //
        //     self.state = State::ExecutingCommand;
        //     self.send_dreq = true;
        //
        //     // Keep running until DMA transfer is complete
        //     Continuation::ContinueAsOperation
        // }
        // else {
        //     // No drive present - Fail immediately
        //     self.set_error(OperationError::NoReadySignal, dcb.drive_select);
        //     self.send_interrupt = true;
        //     Continuation::CommandComplete
        // }

        Continuation::CommandComplete
    }

    /// Perform the Write Sector command.
    fn command_write(&mut self, _bus: &mut BusInterface) -> Continuation {
        // let _cmd_bytes = &self.data_register_in;
        // let dcb = self.read_dcb();
        // self.data_register_in.clear();
        //
        // let xfer_size = bus.dma_mut().as_mut().unwrap().get_dma_transfer_size(HDC_DMA);
        // log::trace!(
        //     "Command Write: drive: {} c: {} h: {} s: {} bc: {}, xfer_size:{}",
        //     dcb.drive_select,
        //     dcb.c,
        //     dcb.h,
        //     dcb.s,
        //     dcb.block_count,
        //     xfer_size
        // );
        //
        // if xfer_size % SECTOR_SIZE != 0 {
        //     log::warn!("Command Write: DMA word count not multiple of sector size");
        // }
        //
        // self.drive_select = dcb.drive_select;
        //
        // // Check drive status
        // if self.drive_present(dcb.drive_select) {
        //     // Set up Operation
        //     self.operation_status.buffer_idx = 0;
        //     self.drives[self.drive_select].cylinder = dcb.c;
        //     self.drives[self.drive_select].head = dcb.h;
        //     self.drives[self.drive_select].sector = dcb.s;
        //
        //     self.operation_status.block_ct = dcb.block_count;
        //     self.operation_status.block_n = 0;
        //
        //     self.operation_status.dma_bytes_left = xfer_size;
        //     self.operation_status.dma_byte_count = 0;
        //
        //     self.state = State::ExecutingCommand;
        //     self.send_dreq = true;
        //
        //     // Keep running until DMA transfer is complete'
        //     Continuation::ContinueAsOperation
        // }
        // else {
        //     // No drive present - Fail immediately
        //     self.set_error(OperationError::NoReadySignal, dcb.drive_select);
        //     self.send_interrupt = true;
        //     Continuation::CommandComplete
        // }

        Continuation::CommandComplete
    }

    /// Perform the Seek command.
    fn command_seek(&mut self, _bus: &mut BusInterface) -> Continuation {
        // let dcb = self.read_dcb();
        // self.data_register_in.clear();
        //
        // log::trace!("Command Seek: drive: {} c: {} h: {}", dcb.drive_select, dcb.c, dcb.h);
        //
        // self.drive_select = dcb.drive_select;
        //
        // // Check drive status
        // if self.drive_present(dcb.drive_select) {
        //     self.drives[self.drive_select].cylinder = dcb.c;
        //     self.drives[self.drive_select].head = dcb.h;
        //     // Seek does not specify a sector - we can only seek to the first sector on a track
        //     self.drives[self.drive_select].sector = 0;
        //
        //     self.set_error(OperationError::NoError, dcb.drive_select);
        // }
        // else {
        //     // No drive present - Fail immediately
        //     self.set_error(OperationError::NoReadySignal, dcb.drive_select);
        // }
        //
        // self.send_interrupt = true;
        Continuation::CommandComplete
    }

    /// End a Command that utilized DMA service.
    fn end_dma_command(&mut self, _drive: u32, error: bool) {
        self.clear_dreq = true;
        self.operation_status.dma_byte_count = 0;
        self.operation_status.dma_bytes_left = 0;

        self.error_flag = error;
        self.send_interrupt = true;
        log::trace!("End of DMA command. Changing state to HaveCommandStatus");
        self.state = AtaState::HaveCommandStatus;
    }

    fn end_operation(&mut self, _error: bool) {
        self.status_register.set_busy(false);
        self.operation_status = OperationStatus::default();
        self.state = AtaState::HaveCommandStatus;
    }

    /// Read a sector from disk into the controller's sector buffer.
    fn read_sector_into_buffer(&mut self, _retry: bool) {
        //self.operation_status[self.drive_select].buffer_idx = 0;

        let pos = self.disk.as_mut().unwrap().position_vhd();

        if let Some(vhd) = self.disk.as_mut().unwrap().vhd_mut() {
            match vhd.read_sector(self.sector_buffer.get_mut(), pos.c, pos.h, pos.s) {
                Ok(_) => {
                    // Sector read successful.
                    // Set sector buffer cursor.
                    log::debug!("Sector read into buffer successfully. Setting sector buffer cursor to 0");
                    // Set DRQ flag to inform host of available data.
                    self.status_register.set_drq(true);
                    self.sector_buffer.seek(SeekFrom::Start(0)).unwrap();
                }
                Err(err) => {
                    log::error!("Sector read failed: {}", err);
                }
            };
        }
    }

    /// Write a sector to disk from the controller's sector buffer.
    fn write_sector_from_buffer(&mut self, _retry: bool) {
        //self.operation_status[self.drive_select].buffer_idx = 0;

        let pos = self.disk.as_mut().unwrap().position_vhd();

        if let Some(vhd) = self.disk.as_mut().unwrap().vhd_mut() {
            match vhd.write_sector(self.sector_buffer.get_ref(), pos.c, pos.h, pos.s) {
                Ok(_) => {
                    // Sector write successful.
                    // Set sector buffer cursor.
                    log::debug!("Sector written from buffer successfully. Setting sector buffer cursor to 0");
                    // Set DRQ flag to inform host we want more data.
                    self.status_register.set_drq(true);
                    self.sector_buffer.seek(SeekFrom::Start(0)).unwrap();
                }
                Err(err) => {
                    log::error!("Sector write failed: {}", err);
                }
            };
        }
    }

    /// Process the Read Sector Buffer operation.
    /// This operation continues until the DMA transfer is complete.
    fn opearation_read_sector_buffer(&mut self, _dma: &mut dma::DMAController, _bus: &mut BusInterface) {
        // if self.dreq_active && dma.read_dma_acknowledge(HDC_DMA) {
        //     if self.operation_status.dma_bytes_left > 0 {
        //         let byte = 0;
        //         //let byte = self.drives[self.drive_select].sector_buf[self.operation_status.buffer_idx & 0x1FF];
        //         //self.operation_status[self.drive_select].buffer_idx += 1;
        //         // Bytes left to transfer
        //         dma.do_dma_write_u8(bus, HDC_DMA, byte);
        //         self.operation_status.dma_byte_count += 1;
        //         self.operation_status.dma_bytes_left -= 1;
        //
        //         // See if we are done based on DMA controller
        //         let tc = dma.check_terminal_count(HDC_DMA);
        //         if tc {
        //             log::trace!("DMA terminal count triggered end of ReadSectorBuffer command.");
        //             if self.operation_status[self.drive_select].dma_bytes_left != 0 {
        //                 log::warn!(
        //                     "Incomplete DMA transfer on terminal count! Bytes remaining: {} count: {}",
        //                     self.operation_status[self.drive_select].dma_bytes_left,
        //                     self.operation_status[self.drive_select].dma_byte_count
        //                 );
        //             }
        //
        //             log::trace!("Completed ReadSectorBuffer command.");
        //             self.end_dma_command(0, false);
        //         }
        //     }
        //     else {
        //         // No more bytes left to transfer. Finalize operation
        //         let tc = dma.check_terminal_count(HDC_DMA);
        //         if !tc {
        //             log::warn!("ReadSectorBuffer complete without DMA terminal count.");
        //         }
        //
        //         log::trace!("Completed ReadSectorBuffer command.");
        //         self.end_dma_command(0, false);
        //     }
        // }
        // else if !self.dreq_active {
        //     log::error!("Error: WriteSectorBuffer command without DMA active!")
        // }
    }

    /// Process the Write Sector Buffer operation.
    /// This operation continues until the DMA transfer is complete.
    fn opearation_write_sector_buffer(&mut self, _dma: &mut dma::DMAController, _bus: &mut BusInterface) {
        // if self.dreq_active && dma.read_dma_acknowledge(HDC_DMA) {
        //     if self.operation_status[self.drive_select].dma_bytes_left > 0 {
        //         // Bytes left to transfer
        //         let _byte = dma.do_dma_read_u8(bus, HDC_DMA);
        //         self.operation_status[self.drive_select].dma_byte_count += 1;
        //         self.operation_status[self.drive_select].dma_bytes_left -= 1;
        //
        //         // See if we are done based on DMA controller
        //         let tc = dma.check_terminal_count(HDC_DMA);
        //         if tc {
        //             log::trace!("DMA terminal count triggered end of WriteSectorBuffer command.");
        //             if self.operation_status[self.drive_select].dma_bytes_left != 0 {
        //                 log::warn!(
        //                     "Incomplete DMA transfer on terminal count! Bytes remaining: {} count: {}",
        //                     self.operation_status[self.drive_select].dma_bytes_left,
        //                     self.operation_status[self.drive_select].dma_byte_count
        //                 );
        //             }
        //
        //             log::trace!("Completed WriteSectorBuffer command.");
        //             self.end_dma_command(0, false);
        //         }
        //     }
        //     else {
        //         // No more bytes left to transfer. Finalize operation
        //         let tc = dma.check_terminal_count(HDC_DMA);
        //         if !tc {
        //             log::warn!("WriteSectorBuffer complete without DMA terminal count.");
        //         }
        //
        //         log::trace!("Completed WriteSectorBuffer command.");
        //         self.end_dma_command(0, false);
        //     }
        // }
        // else if !self.dreq_active {
        //     log::error!("Error: WriteSectorBuffer command without DMA active!")
        // }
    }

    /// Process the Read Sector operation.
    /// This operation continues until the transfer is complete.
    fn operation_read_sector(&mut self, verify: bool, _dma: &mut dma::DMAController, _bus: &mut BusInterface) {
        // log::trace!(
        //     "in operation_read_sector(): drq: {} buffer empty: {}, sectors left: {}",
        //     self.status_register.drq(),
        //     self.sector_buffer_emtpy(),
        //     self.operation_status[drive_select].sectors_left
        // );
        // Wait until sector buffer is exhausted before reading next sector.
        if self.sector_buffer_end() {
            if self.operation_status.sectors_left > 0 {
                // Advance to next sector
                if let Some(new_chs) = self.disk.as_mut().unwrap().next_sector() {
                    log::debug!("operation_read_sector(): Reading sector: {}", new_chs);
                    self.disk.as_mut().unwrap().seek(new_chs);
                }
                else {
                    log::warn!("operation_read_sector(): Failed to get next sector!");
                    self.end_operation(true);
                    return;
                }

                if !verify {
                    self.read_sector_into_buffer(false);
                }
                self.operation_status.sectors_complete += 1;
                self.operation_status.sectors_left -= 1;

                if self.operation_status.sectors_left == 0 {
                    // Last sector read, finalize operation
                    log::debug!(
                        "operation_read_sector(): Completed command. Sectors read: {}",
                        self.operation_status.sectors_complete
                    );
                    self.end_operation(false);
                }
            }
            else {
                // No more sectors left to transfer. Finalize operation
                log::debug!(
                    "operation_read_sector(): Completed command. Sectors read: {}",
                    self.operation_status.sectors_complete
                );
                self.end_operation(false);
            }
        }

        // if self.dreq_active && dma.read_dma_acknowledge(HDC_DMA) {
        //     if self.operation_status.dma_bytes_left > 0 {
        //         // Bytes left to transfer
        //
        //         let byte = self.drives[self.drive_select].sector_buf[self.operation_status.buffer_idx];
        //         dma.do_dma_write_u8(bus, HDC_DMA, byte);
        //         self.operation_status.buffer_idx += 1;
        //         self.operation_status.dma_byte_count += 1;
        //         self.operation_status.dma_bytes_left -= 1;
        //
        //         // Exhausted the sector buffer, read more from disk
        //         if self.operation_status.buffer_idx == SECTOR_SIZE {
        //             // Advance to next sector
        //             //log::trace!("Command Read: Advancing to next sector...");
        //             let (new_c, new_h, new_s) = self.drives[self.drive_select].get_next_sector(
        //                 self.drives[self.drive_select].cylinder,
        //                 self.drives[self.drive_select].head,
        //                 self.drives[self.drive_select].sector,
        //             );
        //
        //             self.drives[self.drive_select].cylinder = new_c;
        //             self.drives[self.drive_select].head = new_h;
        //             self.drives[self.drive_select].sector = new_s;
        //             self.operation_status.buffer_idx = 0;
        //
        //             match &mut self.drives[self.drive_select].vhd {
        //                 Some(vhd) => {
        //                     match vhd.read_sector(
        //                         &mut self.drives[self.drive_select].sector_buf,
        //                         self.drives[self.drive_select].cylinder,
        //                         self.drives[self.drive_select].head,
        //                         self.drives[self.drive_select].sector,
        //                     ) {
        //                         Ok(_) => {
        //                             // Sector read successful
        //                         }
        //                         Err(err) => {
        //                             log::error!("Sector read failed: {}", err);
        //                         }
        //                     };
        //                 }
        //                 None => {
        //                     log::error!("Read operation without VHD mounted.");
        //                 }
        //             }
        //         }
        //
        //         // See if we are done based on DMA controller
        //         let tc = dma.check_terminal_count(HDC_DMA);
        //         if tc {
        //             log::trace!("DMA terminal count triggered end of Read command.");
        //             if self.operation_status.dma_bytes_left != 0 {
        //                 log::warn!(
        //                     "Incomplete DMA transfer on terminal count! Bytes remaining: {} count: {}",
        //                     self.operation_status.dma_bytes_left,
        //                     self.operation_status.dma_byte_count
        //                 );
        //             }
        //
        //             log::trace!("Completed Read Command");
        //             self.end_dma_command(0, false);
        //         }
        //     }
        //     else {
        //         // No more bytes left to transfer. Finalize operation
        //         let tc = dma.check_terminal_count(HDC_DMA);
        //         if !tc {
        //             log::warn!("Command Read complete without DMA terminal count.");
        //         }
        //
        //         log::trace!("Completed Read Command");
        //         self.end_dma_command(0, false);
        //     }
        // }
        // else if !self.dreq_active {
        //     log::error!("Error: Read command without DMA active!")
        // }
    }

    fn operation_write_sector(&mut self, _dma: &mut dma::DMAController, _bus: &mut BusInterface) {
        // Wait until sector buffer is full before reading next sector.
        if self.sector_buffer_end() {
            if self.operation_status.sectors_left > 0 {
                // Write current sector
                log::debug!(
                    "operation_write_sector(): Writing sector: {}",
                    self.disk.as_ref().unwrap().position()
                );
                self.write_sector_from_buffer(false);

                // Advance to next sector
                if let Some(new_chs) = self.disk.as_mut().unwrap().next_sector() {
                    self.disk.as_mut().unwrap().seek(new_chs);
                }
                else {
                    log::warn!("operation_write_sector(): Failed to seek to next sector!");
                    self.end_operation(true);
                    return;
                }

                self.operation_status.sectors_complete += 1;
                self.operation_status.sectors_left -= 1;

                if self.operation_status.sectors_left == 0 {
                    // Last sector read, finalize operation
                    log::debug!(
                        "operation_write_sector(): Completed command. Sectors written: {}",
                        self.operation_status.sectors_complete
                    );
                    self.end_operation(false);
                }
            }
            else {
                // No more sectors left to transfer. Finalize operation
                log::debug!(
                    "operation_write_sector(): Completed command. Sectors written: {}",
                    self.operation_status.sectors_complete
                );
                self.end_operation(false);
            }
        }
    }

    /// Run the ATA Device
    pub fn run(&mut self, dma: &mut dma::DMAController, bus: &mut BusInterface, us: f64) {
        // Handle interrupts
        if self.send_interrupt {
            if self.irq_enabled {
                //log::trace!(">>> Firing HDC IRQ 5");
                if let Some(irq) = self.irq {
                    bus.pic_mut().as_mut().unwrap().request_interrupt(irq);
                }

                self.send_interrupt = false;
                self.interrupt_active = true;
            }
            else {
                //log::trace!(">>> IRQ was masked");
                self.send_interrupt = false;
                self.interrupt_active = false;
            }
        }

        if self.clear_interrupt {
            if let Some(irq) = self.irq {
                bus.pic_mut().as_mut().unwrap().clear_interrupt(irq);
            }
            self.clear_interrupt = false;
            self.interrupt_active = false;
        }

        if self.send_dreq {
            if let Some(dmac) = self.dma_channel {
                dma.request_service(dmac as usize);
            }
            self.send_dreq = false;
            self.dreq_active = true;
        }

        if self.clear_dreq {
            if let Some(dmac) = self.dma_channel {
                dma.clear_service(dmac as usize);
            }
            self.clear_dreq = false;
            self.dreq_active = false;
        }

        self.state_accumulator += us;

        // Process any running Operations
        match self.state {
            AtaState::Reset => {
                // We need to remain in the reset state for a minimum amount of time before moving to
                // WaitingForCommand state. IBM BIOS/DOS does not check for this, but Minix does.
                if self.state_accumulator >= ATA_RESET_DELAY_US {
                    // TODO: We will still move into other states if a command is received. Should we refuse commands
                    //       until reset completes?
                    log::debug!("ATA Reset Complete, moving to WaitingForCommand");
                    self.state = AtaState::WaitingForCommand;
                    self.state_accumulator = 0.0;
                }
            }
            AtaState::ExecutingCommand => match self.command {
                AtaCommand::ReadSector | AtaCommand::ReadSectorRetry | AtaCommand::ReadMultiple => {
                    self.operation_read_sector(false, dma, bus);
                }
                AtaCommand::ReadVerifySector => {
                    self.operation_read_sector(true, dma, bus);
                }
                AtaCommand::WriteSector | AtaCommand::WriteMultiple => {
                    self.operation_write_sector(dma, bus);
                }
                _ => {
                    log::warn!("Unhandled operation: {:?}", self.command);
                }
            },
            _ => {
                // Unhandled state
            }
        }
    }
}

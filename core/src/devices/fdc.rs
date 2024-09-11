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

    devices::fdc.rs

    Implements the NEC µPD765 Floppy Disk Controller
*/

#![allow(dead_code)]

use anyhow::{anyhow, Error};
use std::{collections::VecDeque, default::Default};

use crate::{
    bus::{BusInterface, DeviceRunTimeUnit, IoDevice},
    device_types::fdc::DISK_FORMATS,
    devices::{dma, floppy_drive::FloppyDiskDrive},
    machine_config::FloppyDriveConfig,
    machine_types::FdcType,
};

use crate::device_types::fdc::{DiskFormat, FloppyImageType};
use fluxfox::{DiskCh, DiskChs, DiskChsn};

pub const FDC_IRQ: u8 = 0x06;
pub const FDC_DMA: usize = 2;
pub const FDC_MAX_DRIVES: usize = 4;
pub const FORMAT_BUFFER_SIZE: usize = 4;
//pub const SECTOR_SIZE: usize = 512;

pub const PCXT_IO_BASE: u16 = 0x03F0;
pub const PCJR_IO_BASE: u16 = 0x00F0;

pub const FDC_DIGITAL_OUTPUT_REGISTER: u16 = 0x02;
pub const FDC_STATUS_REGISTER: u16 = 0x04;
pub const FDC_DATA_REGISTER: u16 = 0x05;

// Main Status Register Bit Definitions
// --------------------------------------------------------------------------------
// The first four bits encode which drives are in 'positioning' mode, ie whether
// they are moving their heads or being calibrated
pub const FDC_STATUS_FDD_A_BUSY: u8 = 0b0000_0001;
pub const FDC_STATUS_FDD_B_BUSY: u8 = 0b0000_0010;
pub const FDC_STATUS_FDD_C_BUSY: u8 = 0b0000_0100;
pub const FDC_STATUS_FDD_D_BUSY: u8 = 0b0000_1000;

// Busy bit seems to be on while there are bytes remaining to be read from
// the Data register. The BIOS checks this bit to tell when it is done reading
// from the FDC data register.
pub const FDC_STATUS_FDC_BUSY: u8 = 0b0001_0000;
pub const FDC_STATUS_NON_DMA_MODE: u8 = 0b0010_0000;

// Direction bit is checked by BIOS to tell it if the FDC is expecting a read
// or a write to the Data register.  If this bit is set wrong the BIOS will
// timeout waiting for it.
pub const FDC_STATUS_DIO: u8 = 0b0100_0000;

// MRQ (Main Request) is also used to determine if the data port is ready to be
// written to or read. If this bit is not set the BIOS will timeout waiting for it.
pub const FDC_STATUS_MRQ: u8 = 0b1000_0000;

pub const DOR_DRIVE_SELECT_MASK: u8 = 0b0000_0001;
pub const DOR_DRIVE_SELECT_0: u8 = 0b0000_0000;
pub const DOR_DRIVE_SELECT_1: u8 = 0b0000_0001;
pub const DOR_DRIVE_SELECT_2: u8 = 0b0000_0010;
pub const DOR_DRIVE_SELECT_3: u8 = 0b0000_0011;
pub const DOR_FDC_RESET: u8 = 0b0000_0100;
pub const DOR_DMA_ENABLED: u8 = 0b0000_1000;
pub const DOR_MOTOR_FDD_A: u8 = 0b0001_0000;
pub const DOR_MOTOR_FDD_B: u8 = 0b0010_0000;
pub const DOR_MOTOR_FDD_C: u8 = 0b0100_0000;
pub const DOR_MOTOR_FDD_D: u8 = 0b1000_0000;
// PCJr specific DOR flags
pub const DOR_JRFDC_MOTOR: u8 = 0b0000_0001;
pub const DOR_JRFDC_RESET: u8 = 0b1000_0000;
pub const DOR_JRFDC_WATCHDOG_ENABLE: u8 = 0b0010_0000;
pub const DOR_JRFDC_WATCHDOG_TRIGGER: u8 = 0b0100_0000;

pub const WATCHDOG_TIMEOUT: f64 = 3_000_000.0; // 3 seconds in microseconds

pub const COMMAND_MASK: u8 = 0b0001_1111;
pub const COMMAND_SKIP_BIT: u8 = 0b0010_0000;
pub const COMMAND_READ_TRACK: u8 = 0x02;
pub const COMMAND_WRITE_DATA: u8 = 0x05;
pub const COMMAND_READ_DATA: u8 = 0x06;
pub const COMMAND_WRITE_DELETED_DATA: u8 = 0x09;
pub const COMMAND_READ_DELETED_DATA: u8 = 0x0C;
pub const COMMAND_FORMAT_TRACK: u8 = 0x0D;

pub const COMMAND_FIX_DRIVE_DATA: u8 = 0x03;
pub const COMMAND_CHECK_DRIVE_STATUS: u8 = 0x04;
pub const COMMAND_CALIBRATE_DRIVE: u8 = 0x07;
pub const COMMAND_SENSE_INT_STATUS: u8 = 0x08;
pub const COMMAND_READ_SECTOR_ID: u8 = 0x0A;
pub const COMMAND_SEEK_HEAD: u8 = 0x0F;

pub const ST0_HEAD_ACTIVE: u8 = 0b0000_0100;
pub const ST0_NOT_READY: u8 = 0b0000_1000;
pub const ST0_UNIT_CHECK: u8 = 0b0001_0000;
pub const ST0_SEEK_END: u8 = 0b0010_0000;
pub const ST0_ABNORMAL_TERMINATION: u8 = 0b0100_0000;
pub const ST0_INVALID_OPCODE: u8 = 0b1000_0000;
pub const ST0_ABNORMAL_POLLING: u8 = 0b1100_0000;
pub const ST0_RESET: u8 = 0b1100_0000;

pub const ST1_NO_ID: u8 = 0b0000_0001;
pub const ST1_WRITE_PROTECT: u8 = 0b0000_0010;
pub const ST1_NODATA: u8 = 0b0000_0100;
pub const ST1_CRC_ERROR: u8 = 0b0010_0000;
pub const ST2_DATA_CRC_ERROR: u8 = 0b00010_0000;
pub const ST2_DAD_MARK: u8 = 0b0100_0000;

pub const ST3_ESIG: u8 = 0b1000_0000;
pub const ST3_WRITE_PROTECT: u8 = 0b0100_0000;
pub const ST3_READY: u8 = 0b0010_0000;
pub const ST3_TRACK0: u8 = 0b0001_0000;
pub const ST3_DOUBLESIDED: u8 = 0b0000_1000;
pub const ST3_HEAD: u8 = 0b0000_0100;

/// Represent the state of the DIO bit of the Main Status Register in a readable way.
pub enum IoMode {
    ToCpu,
    FromCpu,
}

/// Represent the various commands that the NEC FDC knows how to handle.
#[derive(Clone, Copy, Debug)]
pub enum Command {
    NoCommand,
    ReadTrack,
    WriteSector,
    ReadSector,
    WriteDeletedSector,
    ReadDeletedSector,
    FormatTrack,
    FixDriveData,
    CheckDriveStatus,
    CalibrateDrive,
    SenseIntStatus,
    ReadSectorID,
    SeekParkHead,
    Invalid,
}

/// Encapsulates a result from a command or operation execution and used to build a
/// status response.
pub enum ControllerResult {
    Success(InterruptCode),
    GeneralFailure(InterruptCode),
    WriteProtectFailure,
}

/// Represents the possible values of the Interrupt Code field in Status Register 0.
/// Returning 'AbnormalTermination' may result in a General Failure reading drive
/// message in DOS.
/// InvalidCommand should be returned for any command not handled by the FDC - later
/// controller models added more commands.
#[derive(Debug)]
pub enum InterruptCode {
    NormalTermination,
    AbnormalTermination,
    InvalidCommand,
    AbnormalPolling,
}

/// Attempt to classify every general error condition a virtual disk drive may experience.
/// These states are used to build the status bytes presented after a command has been
/// executed. The exact mapping between error conditions and status flags is uncertain...
#[derive(Clone, Copy, Debug)]
pub enum DriveError {
    NoError,
    NoMedia,
    BadSeek,
    BadRead,
    BadWrite,
    WriteProtect,
    DMAError,
}

pub struct OperationSpecifier {
    pub chs: DiskChs,
    pub sector_size: u8,
    pub track_len: u8,
    pub gap3_len: u8,
    pub data_len: u8,
}

/// Classify operations - an Operation is intiated by any Command that does not immediately
/// terminate, and is called on a repeated basis by the run() method until complete.
///
/// Operations usually involve DMA transfers.
#[derive(Debug)]
pub enum Operation {
    NoOperation,
    ReadData(DiskChs, u8, u8, u8, u8), // CHS, sector_size, track_len, gap3_len, data_len
    ReadTrack(DiskChs, u8, u8, u8, u8), // CHS, sector_size, track_len, gap3_len, data_len
    WriteData(u8, u8, u8, u8, u8, u8, u8), // cylinder, head, sector, sector_size, track_len, gap3_len, data_len
    FormatTrack(u8, u8, u8, u8),
}

type CommandDispatchFn = fn(&mut FloppyController) -> Continuation;
pub enum Continuation {
    CommandComplete,
    ContinueAsOperation,
}

pub struct FloppyController {
    us_accumulator: f64,
    watchdog_accumulator: f64,
    fdc_type: FdcType,
    status_byte: u8,
    reset_flag: bool,
    reset_sense_count: u8,
    mrq: bool,

    data_register: u8,
    dma: bool,
    dor: u8,
    busy: bool,
    dio: IoMode,
    reading_command: bool,
    command: Command,
    command_fn: Option<CommandDispatchFn>,
    last_command: Command,
    receiving_command: bool,
    command_byte_n: u32,
    command_skip: bool,
    operation: Operation,
    operation_init: bool,
    operation_final_sid: u8,
    send_interrupt: bool,
    pending_interrupt: bool,
    end_interrupt: bool,
    watchdog_enabled: bool,     // IBM PCJr only.  Watchdog timer enabled.
    watchdog_trigger_bit: bool, // IBM PCJr only.  Watchdog timer trigger bit status.
    watchdog_triggered: bool,   // IBM PCJr only.  Watchdog timer triggered.

    last_error: DriveError,

    data_register_out: VecDeque<u8>,
    data_register_in: VecDeque<u8>,
    format_buffer: VecDeque<u8>,

    drives: [FloppyDiskDrive; FDC_MAX_DRIVES],
    drive_ct: usize,
    drive_select: usize,

    in_dma: bool,
    dma_byte_count: usize,
    dma_bytes_left: usize,
    pio_byte_count: usize,
    pio_sector_byte_count: usize,
    pio_bytes_left: usize,
    xfer_size_sectors: usize,
    xfer_size_bytes: usize,
    xfer_completed_sectors: usize,
    xfer_buffer: Vec<u8>,
}

/// IO Port handlers for the FDC
impl IoDevice for FloppyController {
    fn read_u8(&mut self, port: u16, _delta: DeviceRunTimeUnit) -> u8 {
        let base = match self.fdc_type {
            FdcType::IbmNec => PCXT_IO_BASE,
            FdcType::IbmPCJrNec => PCJR_IO_BASE,
        };

        match port - base {
            FDC_DIGITAL_OUTPUT_REGISTER => {
                log::warn!("Read from Write-only DOR register");
                0
            }
            FDC_STATUS_REGISTER => self.handle_status_register_read(),
            FDC_DATA_REGISTER => self.handle_data_register_read(),
            _ => unreachable!("FLOPPY: Bad port #"),
        }
    }

    fn write_u8(&mut self, port: u16, data: u8, _bus: Option<&mut BusInterface>, _delta: DeviceRunTimeUnit) {
        let base = match self.fdc_type {
            FdcType::IbmNec => PCXT_IO_BASE,
            FdcType::IbmPCJrNec => PCJR_IO_BASE,
        };

        match port - base {
            FDC_DIGITAL_OUTPUT_REGISTER => match self.fdc_type {
                FdcType::IbmNec => self.handle_dor_write(data),
                FdcType::IbmPCJrNec => self.handle_dor_write_jr(data),
            },
            FDC_STATUS_REGISTER => {
                log::warn!("Write to Read-only status register");
            }
            FDC_DATA_REGISTER => {
                self.handle_data_register_write(data);
            }
            _ => unreachable!("FLOPPY: Bad port #"),
        }
    }

    fn port_list(&self) -> Vec<(String, u16)> {
        let base = match self.fdc_type {
            FdcType::IbmNec => PCXT_IO_BASE,
            FdcType::IbmPCJrNec => PCJR_IO_BASE,
        };

        vec![
            (
                String::from("FDC Digital Output Register"),
                base + FDC_DIGITAL_OUTPUT_REGISTER,
            ),
            (String::from("FDC Status Register"), base + FDC_STATUS_REGISTER),
            (String::from("FDC Data Register"), base + FDC_DATA_REGISTER),
        ]
    }
}

impl Default for FloppyController {
    fn default() -> Self {
        Self {
            us_accumulator: 0.0,
            watchdog_accumulator: 0.0,
            fdc_type: FdcType::IbmNec,
            status_byte: 0,
            reset_flag: false,
            reset_sense_count: 0,
            mrq: true,
            data_register: 0,
            dma: true,
            dor: 0,
            busy: false,
            dio: IoMode::FromCpu,
            reading_command: false,
            command: Command::NoCommand,
            command_fn: None,
            last_command: Command::NoCommand,
            command_byte_n: 0,
            receiving_command: false,
            command_skip: false,
            operation: Operation::NoOperation,
            operation_init: false,
            operation_final_sid: 1,

            last_error: DriveError::NoError,

            send_interrupt: false,
            pending_interrupt: false,
            end_interrupt: false,
            watchdog_enabled: false,
            watchdog_trigger_bit: false,
            watchdog_triggered: false,

            data_register_out: VecDeque::new(),
            data_register_in: VecDeque::new(),
            format_buffer: VecDeque::new(),

            drives: [
                FloppyDiskDrive::default(),
                FloppyDiskDrive::default(),
                FloppyDiskDrive::default(),
                FloppyDiskDrive::default(),
            ],
            drive_ct: 0,
            drive_select: 0,

            in_dma: false,
            dma_byte_count: 0,
            dma_bytes_left: 0,
            pio_byte_count: 0,
            pio_sector_byte_count: 0,
            pio_bytes_left: 0,
            xfer_size_sectors: 0,
            xfer_size_bytes: 0,
            xfer_completed_sectors: 0,
            xfer_buffer: Vec::new(),
        }
    }
}

impl FloppyController {
    pub fn new(fdc_type: FdcType, drives: Vec<FloppyDriveConfig>) -> Self {
        // PCJr has a maximum of one floppy drive, so ignore drive count.
        let drive_ct = if matches!(fdc_type, FdcType::IbmPCJrNec) {
            1
        }
        else {
            drives.len()
        };

        let mut fdc = FloppyController {
            fdc_type,
            drive_ct,
            ..Default::default()
        };

        for (i, drive) in drives.iter().take(FDC_MAX_DRIVES).enumerate() {
            fdc.drives[i] = FloppyDiskDrive::new(i, drive.fd_type);
        }

        fdc
    }

    /// Reset the Floppy Drive Controller
    pub fn reset(&mut self) {
        // TODO: Implement in terms of Default
        self.status_byte = 0;
        self.drive_select = 0;
        self.reset_flag = true;
        self.reset_sense_count = 0;

        self.data_register_out.clear();
        self.data_register_in.clear();
        self.format_buffer.clear();

        self.mrq = true;
        self.dio = IoMode::FromCpu;

        // Reset all drives.
        for drive in &mut self.drives.iter_mut() {
            drive.reset();
        }

        self.last_error = DriveError::NoError;
        self.receiving_command = false;
        self.command = Command::NoCommand;
        self.command_fn = None;
        self.command_byte_n = 0;

        self.send_interrupt = false;
        self.pending_interrupt = false;
        self.end_interrupt = false;

        self.in_dma = false;
        self.dma_byte_count = 0;
        self.dma_bytes_left = 0;
    }

    pub fn decode_sector_size(code: u8) -> usize {
        match code {
            0x00 => 128,
            0x01 => 256,
            0x02 => 512,
            0x03 => 1024,
            0x04 => 2048,
            0x05 => 4096,
            0x06 => 8192,
            0x07 => 16384,
            _ => 32768,
        }
    }

    pub fn drive_ct(&self) -> usize {
        self.drive_ct
    }

    pub fn drive(&self, idx: usize) -> &FloppyDiskDrive {
        if idx >= self.drive_ct {
            panic!("Invalid drive index");
        }
        &self.drives[self.drive_select]
    }

    /// Load a disk into the specified drive
    pub fn load_image_from(&mut self, drive_select: usize, src_vec: Vec<u8>, write_protect: bool) -> Result<(), Error> {
        if drive_select >= self.drive_ct {
            return Err(anyhow!("Invalid drive selection"));
        }

        self.drives[drive_select].load_image_from(src_vec, write_protect)?;
        Ok(())
    }

    pub fn get_image_data(&self, drive_select: usize) -> Option<&[u8]> {
        if self.drives[drive_select].disk_image.is_some() {
            // We have at least some kind of disk image, return it
            Some(&[])
        }
        else {
            None
        }
    }

    /// Unload (eject) the disk in the specified drive
    pub fn unload_image(&mut self, drive_select: usize) {
        let drive = &mut self.drives[drive_select];

        drive.unload_image();
    }

    pub fn patch_image_bpb(&mut self, drive_select: usize, image_type: Option<FloppyImageType>) -> Result<(), Error> {
        let drive = &mut self.drives[drive_select];

        if let Some(image_type) = image_type {
            if let Ok(standard_disk_format) = image_type.try_into() {
                drive.patch_image_bpb(standard_disk_format)?;
            }
            else {
                return Err(anyhow!("Invalid disk format"));
            }
        }
        else {
            return Err(anyhow!("Invalid disk format"));
        }
        Ok(())
    }

    pub fn handle_status_register_read(&mut self) -> u8 {
        let mut msr_byte = 0;
        for (i, drive) in self.drives.iter().enumerate() {
            if drive.positioning {
                msr_byte |= 0x01 << i;
            }
        }

        if self.busy {
            msr_byte |= FDC_STATUS_FDC_BUSY;
        }

        // The NDMA bit is sort of an PIO operation status bit. It is cleared when the drive is no
        // longer busy with the operation.
        if !self.dma && !matches!(self.operation, Operation::NoOperation) {
            msr_byte |= FDC_STATUS_NON_DMA_MODE;
        }

        // DIO bit => 0=FDC Receiving 1=FDC Sending
        if let IoMode::ToCpu = self.dio {
            msr_byte |= FDC_STATUS_DIO;
        }

        // MRQ => Ready to receive or send data or commands via the data register
        if self.mrq {
            msr_byte |= FDC_STATUS_MRQ;
        }

        //log::trace!("Status Register Read: Drive select:{}, Value: {:02X}", self.drive_select, msr_byte);
        msr_byte
    }

    pub fn motor_on(&mut self, drive_select: usize) {
        self.drives[drive_select].motor_on();
    }

    pub fn motor_off(&mut self, drive_select: usize) {
        if self.drives[drive_select].motor_on {
            log::trace!("Drive {}: turning motor off.", drive_select)
        }
        self.drives[drive_select].motor_on = false;
        //self.drives[drive_select].ready = false;    // Breaks booting(?)
    }

    pub fn write_protect(&mut self, drive_select: usize, write_protected: bool) {
        self.drives[drive_select].write_protected = write_protected;
    }

    pub fn handle_dor_write(&mut self, data: u8) {
        if data & DOR_FDC_RESET == 0 {
            // Reset the FDC when the reset bit is *not* set
            // Ignore all other commands
            log::debug!("FDC Reset requested: {:02X}", data);
            self.reset();
            self.send_interrupt = true;
        }
        else {
            // Not reset. Turn drive motors on or off based on the MOTx bits in the DOR byte.
            let disk_n = data & 0x03;
            if data & DOR_MOTOR_FDD_A != 0 {
                self.motor_on(0);
            }
            else {
                self.motor_off(0);
            }
            if data & DOR_MOTOR_FDD_B != 0 {
                self.motor_on(1);
            }
            else {
                self.motor_off(1);
            }
            if data & DOR_MOTOR_FDD_C != 0 {
                self.motor_on(2);
            }
            else {
                self.motor_off(2);
            }
            if data & DOR_MOTOR_FDD_D != 0 {
                self.motor_on(3);
            }
            else {
                self.motor_off(3);
            }

            if data & DOR_DMA_ENABLED != 0 {
                self.dma = true;
            }
            else {
                self.dma = false;
            }

            // Select drive from DRx bits.
            if self.drives[disk_n as usize].motor_on {
                log::debug!("Drive {} selected, motor on", disk_n);
                self.drive_select = disk_n as usize;
                self.drives[disk_n as usize].motor_on = true;
            }
            else {
                // It's valid to write to the dor without turning a motor on.
                // In this case the FDC can be re-enabled, but with no drive selected.
            }
        }
        self.dor = data;
    }

    pub fn handle_dor_write_jr(&mut self, data: u8) {
        if data & DOR_JRFDC_RESET == 0 {
            // Reset the FDC when the reset bit is *not* set
            // Ignore all other commands
            log::debug!("PCJr FDC Reset requested: {:02X}", data);
            self.reset();
            self.send_interrupt = true;
        }
        else {
            // Not reset. Turn drive motors on or off based on the drive enable bit.
            if data & DOR_JRFDC_MOTOR != 0 {
                self.motor_on(0);
            }
            else {
                self.motor_off(0);
            }

            if data & DOR_DMA_ENABLED != 0 {
                log::error!("PCJr FDC DMA was erroneously enabled");
                self.dma = true;
            }
            else {
                self.dma = false;
            }

            if data & DOR_JRFDC_WATCHDOG_ENABLE != 0 {
                log::debug!("PCJr FDC Watchdog enabled");
                self.watchdog_enabled = true;
            }
            else {
                self.watchdog_enabled = false;
                self.watchdog_triggered = false;
                self.watchdog_accumulator = 0.0;
                self.end_interrupt = true;
            }

            // Watchdog trigger is set on falling edge of trigger bit.
            if data & DOR_JRFDC_WATCHDOG_TRIGGER != 0 {
                self.watchdog_trigger_bit = true;
            }
            else {
                if self.watchdog_trigger_bit {
                    log::debug!("PCJr FDC Watchdog triggered");
                    self.watchdog_triggered = true;
                }
                self.watchdog_trigger_bit = false;
            }
        }
        self.dor = data;
    }

    /// Create the ST0 status register bitfield with the given parameters.
    ///
    /// Note: returning an Interrupt Code of Abnormal Termination will result in a "General failure reading drive"
    ///
    pub fn make_st0_byte(&self, interrupt_code: InterruptCode, drive_select: usize, seek_end: bool) -> u8 {
        let mut st0: u8 = 0;

        // Set selected drive bits
        st0 |= (drive_select as u8) & 0x03;

        // Set active head bit
        if self.drives[drive_select].chs.h() == 1 {
            st0 |= ST0_HEAD_ACTIVE;
        }

        // Set ready bit
        if !self.drives[drive_select].ready || !self.drives[drive_select].disk_present {
            st0 |= ST0_NOT_READY;
        }

        // Set seek bit
        if seek_end {
            st0 |= ST0_SEEK_END;
        }

        let status = self.drives[drive_select].get_operation_status();
        if status.address_crc_error | status.data_crc_error {
            st0 |= ST0_ABNORMAL_TERMINATION;
        }
        else {
            log::trace!("ST0: interrupt code: {:?}", interrupt_code);
            // Set interrupt code
            st0 |= match interrupt_code {
                InterruptCode::NormalTermination => 0,
                InterruptCode::AbnormalTermination => ST0_ABNORMAL_TERMINATION,
                InterruptCode::InvalidCommand => ST0_INVALID_OPCODE,
                InterruptCode::AbnormalPolling => ST0_ABNORMAL_POLLING,
            };
        }

        log::trace!("ST0 byte: {:08b}", st0);
        st0
    }

    /// Generate the value of the ST1 Status Register in response to a command
    pub fn make_st1_byte(&self, drive_select: usize) -> u8 {
        // The ST1 status register contains mostly error codes
        let mut st1_byte = 0;

        // Set the "No Data" bit if we received an invalid request
        st1_byte |= match self.last_error {
            DriveError::BadRead | DriveError::BadWrite | DriveError::BadSeek => ST1_NODATA,
            DriveError::WriteProtect => ST1_WRITE_PROTECT | ST1_NO_ID,
            _ => 0,
        };

        // Based on DOS's behavior regarding the "Not ready error" it appears that
        // operations without a disk timeout instead of returning a particular error
        // flag. Need to verify this on real hardware if possible.
        if !self.drives[drive_select].disk_present {
            st1_byte |= ST1_NODATA | ST1_NO_ID;
        }

        // If the last read produced a crc error, then set the data error bit.
        // The CRC error bit is also set in the ST2 register.
        let status = self.drives[drive_select].get_operation_status();
        if status.sector_not_found {
            st1_byte |= ST1_NODATA;
        }
        if status.address_crc_error | status.data_crc_error {
            st1_byte |= ST1_CRC_ERROR;
        }

        log::trace!("ST1 byte: {:08b}", st1_byte);
        st1_byte
    }

    /// Generate the value of the ST2 Status Register in response to a command
    pub fn make_st2_byte(&self, drive_select: usize) -> u8 {
        // The ST2 status register contains mostly error codes. CRC errors are reported here.
        let mut st2 = 0;
        let status = self.drives[drive_select].get_operation_status();

        if !status.address_crc_error && status.data_crc_error {
            // Set the data CRC error bit - this cannot be set of an address crc error occurred,
            // as we should not have read any data.
            st2 |= ST2_DATA_CRC_ERROR;
        }
        if status.deleted_mark {
            st2 |= ST2_DAD_MARK;
        }

        log::trace!("ST2 byte: {:08b}", st2);
        st2
    }

    /// Generate the value of the ST3 Status Register in response to a command
    pub fn make_st3_byte(&self, drive_select: usize) -> u8 {
        // Set drive select bits DS0 & DS1
        let mut st3_byte = (drive_select & 0x03) as u8;

        // HDSEL signal: 1 == head 1 active
        if self.drives[drive_select].chs.h() == 1 {
            st3_byte |= ST3_HEAD;
        }

        // DSDR signal - Is this active for a double-sided drive, or only when a double-sided disk is present?
        st3_byte |= ST3_DOUBLESIDED;

        if self.drives[drive_select].chs.c() == 0 {
            st3_byte |= ST3_TRACK0;
        }

        // Drive ready - Should drive be ready when no disk is present?
        if self.drives[drive_select].ready {
            st3_byte |= ST3_READY;
        }

        // Write protect status
        if self.drives[drive_select].write_protected {
            st3_byte |= ST3_WRITE_PROTECT;
        }

        // Error signal - (What conditions cause ESIG to assert?)
        if self.drives[drive_select].error_signal {
            st3_byte |= ST3_ESIG;
        }

        st3_byte
    }

    pub fn handle_data_register_read(&mut self) -> u8 {
        let mut out_byte = 0;

        if self.data_register_out.len() > 0 {
            out_byte = self.data_register_out.pop_front().unwrap();
            if self.data_register_out.len() == 0 {
                //log::trace!("Popped last byte, clearing busy flag");
                // CPU has read all available bytes
                self.busy = false;
                self.dio = IoMode::FromCpu;
            }
        }

        //log::trace!("Data Register Read: {:02X}", out_byte );
        out_byte
    }

    pub fn set_command(&mut self, command: Command, n_bytes: u32, command_fn: CommandDispatchFn) {
        // Since we are entering a new command, clear the previous error status
        self.last_error = DriveError::NoError;
        self.receiving_command = true;
        self.command = command;
        self.command_fn = Some(command_fn);
        self.command_byte_n = n_bytes;
    }

    pub fn send_data_register(&mut self) {
        self.busy = true;
        self.dio = IoMode::ToCpu;
        self.mrq = true;
    }

    pub fn select_drive(&mut self, drive_select: usize) -> Option<&FloppyDiskDrive> {
        if drive_select >= self.drive_ct {
            return None;
        }
        self.drive_select = drive_select;
        Some(&self.drives[drive_select])
    }

    pub fn select_drive_mut(&mut self, drive_select: usize) -> Option<&mut FloppyDiskDrive> {
        if drive_select >= self.drive_ct {
            return None;
        }
        self.drive_select = drive_select;
        Some(&mut self.drives[drive_select])
    }

    pub fn selected_drive(&self) -> &FloppyDiskDrive {
        &self.drives[self.drive_select]
    }

    pub fn selected_drive_mut(&mut self) -> &mut FloppyDiskDrive {
        &mut self.drives[self.drive_select]
    }

    /// Handle a write to the Data Register, 0x3F5.
    ///
    /// This register receives various commands which may be up to 8 bytes long.
    ///
    /// We register both the size of the command and the callback function to call once all bytes for the command
    /// have been read in.
    /// A command can return CommandComplete if it is finished immediately, or ContinueAsOperation to keep running
    /// during calls to the fdc run() method during ticks. This is to support operations that take some period of
    /// time like DMA transfers.
    pub fn handle_data_register_write(&mut self, data: u8) {
        //log::trace!("Data Register Write");
        if !self.receiving_command {
            let command = data & COMMAND_MASK;
            self.command_skip = data & COMMAND_SKIP_BIT != 0;
            match command {
                COMMAND_READ_TRACK => {
                    log::trace!("Received Read Track command: {:02}", command);
                    self.set_command(Command::ReadTrack, 8, FloppyController::command_read_track);
                }
                COMMAND_WRITE_DATA => {
                    log::trace!("Received Write Sector command: {:02}", command);
                    self.set_command(Command::WriteSector, 8, FloppyController::command_write_data);
                }
                COMMAND_READ_DATA => {
                    log::trace!("Received Read Sector command: {:02X} {:02}", data, command);
                    self.set_command(Command::ReadSector, 8, FloppyController::command_read_data);
                }
                COMMAND_WRITE_DELETED_DATA => {
                    log::trace!("Received Write Deleted Sector command: {:02}", command);
                    log::error!("Command unimplemented");
                }
                COMMAND_READ_DELETED_DATA => {
                    log::trace!("Received Read Deleted Sector command: {:02}", command);
                    log::error!("Command unimplemented");
                }
                COMMAND_FORMAT_TRACK => {
                    log::trace!("Received Format Track command: {:02}", command);
                    self.set_command(Command::FormatTrack, 5, FloppyController::command_format_track);
                }
                COMMAND_FIX_DRIVE_DATA => {
                    log::trace!("Received Fix Drive Data command: {:02}", command);
                    self.set_command(Command::FixDriveData, 2, FloppyController::command_fix_drive_data);
                }
                COMMAND_CHECK_DRIVE_STATUS => {
                    log::trace!("Received Check Drive Status command: {:02}", command);
                    self.set_command(
                        Command::CheckDriveStatus,
                        1,
                        FloppyController::command_check_drive_status,
                    );
                }
                COMMAND_CALIBRATE_DRIVE => {
                    log::trace!("Received Calibrate Drive command: {:02}", command);
                    self.set_command(Command::CalibrateDrive, 1, FloppyController::command_calibrate_drive);
                }
                COMMAND_SENSE_INT_STATUS => {
                    log::trace!("Received Sense Interrupt Status command: {:02}", command);
                    // Sense Interrupt command has no input bytes, so execute directly
                    self.command_sense_interrupt();
                }
                COMMAND_READ_SECTOR_ID => {
                    log::trace!("Received Read Sector ID command: {:02}", command);
                    self.set_command(Command::ReadSectorID, 1, FloppyController::command_read_sector_id);
                }
                COMMAND_SEEK_HEAD => {
                    log::trace!("Received Seek/Park Head command: {:02}", command);
                    self.set_command(Command::SeekParkHead, 2, FloppyController::command_seek_head);
                }
                _ => {
                    log::warn!("Received invalid command byte: {:02}", command);
                }
            }
        }
        else {
            // Read in command bytes
            if self.command_byte_n > 0 {
                self.data_register_in.push_back(data);
                self.command_byte_n -= 1;
                if self.command_byte_n == 0 {
                    // We read last byte expected for this command, so dispatch to the appropriate command handler
                    let mut result = Continuation::CommandComplete;

                    match self.command_fn {
                        Some(command_fn) => {
                            // Execute the command.
                            result = command_fn(self);
                        }
                        None => {
                            log::error!("No associated method for command: {:?}!", self.command)
                        }
                    }

                    // Clear command if complete
                    if let Continuation::CommandComplete = result {
                        self.last_command = self.command;
                        self.command = Command::NoCommand;
                        self.command_fn = None;
                    }

                    // Clear command vec
                    self.data_register_in.clear();
                    self.receiving_command = false;
                }
            }
        }
    }

    pub fn command_sense_interrupt(&mut self) {
        /* The 5160 BIOS performs four sense interrupts after a reset of the fdc, presumably one for each of
           the possible drives. The BIOS expects to see drive select bits 00 to 11 in the resulting st0 bytes,
           even if no such drives are present.

           In theory the FDC issues interrupts when drive status changes between READY and NOT READY states,
           so a reset would cause all four drives to transition from NOT READY to READY, thus four interrupts.

           But there's no real explanation for interrupts from non-existent drives, or whether the interrupt
           line is per drive or for the entire controller. The documentation for the Sense Interrupt command
           seems to indicate that the interrupt flag is cleared immediately, so it wouldn't leave three more
           interrupts remaining to be serviced.

           Puzzling.

           The 5150 BIOS doesn't do this, and there's no explanation as to why this changed.

            Sense Interrupt returns the Invalid Opcode interrupt code if an interrupt was not in progress.
        */

        let mut st0_byte = ST0_INVALID_OPCODE;

        if self.reset_flag {
            // FDC was just reset, answer with an ST0 for the first drive, but prepare to send up
            // to three more ST0 responses
            st0_byte |= ST0_RESET;
            self.reset_sense_count = 1;
            self.reset_flag = false;
        }
        else if let Command::SenseIntStatus = self.last_command {
            // This Sense Interrupt command was preceded by another.
            // Advance the reset sense count to clear all drives assuming the calling code is doing
            // a four sense-interrupt sequence.
            if self.reset_sense_count < 4 {
                st0_byte |= ST0_RESET;
                st0_byte |= self.reset_sense_count & 0x03;
                self.reset_sense_count += 1;
            }
            else {
                // More than four sense interrupts in a row shouldn't happen
                st0_byte = ST0_INVALID_OPCODE;
                self.reset_flag = false;
                self.reset_sense_count = 0;
            }
        }
        else {
            // Sense interrupt in response to some other command
            if self.pending_interrupt {
                let seek_flag = match self.last_command {
                    Command::CalibrateDrive => true,
                    Command::SeekParkHead => true,
                    _ => false,
                };

                let code = match self.last_error {
                    DriveError::BadRead | DriveError::BadWrite | DriveError::BadSeek => {
                        InterruptCode::AbnormalTermination
                    }
                    _ => InterruptCode::NormalTermination,
                };

                st0_byte = self.make_st0_byte(code, self.drive_select, seek_flag);
            }
            else {
                // Sense Interrupt without pending interrupt is invalid
                st0_byte = ST0_INVALID_OPCODE;
            }
        }

        // Send ST0 register to FIFO
        let cb0 = st0_byte;
        self.data_register_out.push_back(cb0);

        // Send Current Cylinder to FIFO
        let cb1 = self.drives[self.drive_select].chs.c();
        self.data_register_out.push_back(cb1 as u8);

        // We have data for CPU to read
        self.send_data_register();
        // Deassert interrupt
        self.end_interrupt = true;

        self.last_command = Command::SenseIntStatus;
        self.command = Command::NoCommand;
        log::trace!("command_sense_interrupt completed.");
    }

    /// Perform the Fix Drive Data command.
    /// We don't do anything currently with the provided values which are only useful for real drive timings.
    pub fn command_fix_drive_data(&mut self) -> Continuation {
        let steprate_unload = self.data_register_in.pop_front().unwrap();
        let headload_ndm = self.data_register_in.pop_front().unwrap();

        log::trace!(
            "command_fix_drive_data completed: {:08b},{:08b}",
            steprate_unload,
            headload_ndm
        );

        Continuation::CommandComplete
    }

    /// Perform the Check Drive Status command.
    /// This command returns the ST3 status register.
    pub fn command_check_drive_status(&mut self) -> Continuation {
        let drive_select: usize = (self.data_register_in.pop_front().unwrap() & 0x03) as usize;

        let st3 = self.make_st3_byte(drive_select);
        self.data_register_out.push_back(st3);

        // We have data for the CPU to read
        self.send_data_register();

        log::trace!("command_check_drive_status completed: {}", drive_select);

        Continuation::CommandComplete
    }

    /// Perform the Calibrate Drive command (0x07)
    ///
    /// Resets the drive specified drive head to cylinder 0.
    pub fn command_calibrate_drive(&mut self) -> Continuation {
        // A real floppy drive might fail to seek completely to cylinder 0 with one calibrate command.
        // Any point to emulating this behavior?
        let drive_head_select = self.data_register_in.pop_front().unwrap();
        let drive_select = (drive_head_select & 0x03) as usize;
        let head_select = drive_head_select >> 2 & 0x01;

        // Set drive select?
        self.drive_select = drive_select;

        // Set CHS
        self.drives[drive_select].chs.seek(0, head_select, 1);

        log::trace!("command_calibrate_drive completed: {}", drive_select);

        // Calibrate command sends interrupt when complete
        self.send_interrupt = true;
        Continuation::CommandComplete
    }

    /// Performs a Seek for the specified drive to the specified cylinder and head.
    ///
    /// This command has no result phase. The status of the command is checked via Sense Interrupt.
    pub fn command_seek_head(&mut self) -> Continuation {
        // A real floppy drive would take some time to seek
        // Not sure how to go about determining proper timings. For now, seek instantly

        let drive_head_select = self.data_register_in.pop_front().unwrap();
        let cylinder = self.data_register_in.pop_front().unwrap();
        let drive_select = (drive_head_select & 0x03) as usize;
        let head_select = (drive_head_select >> 2) & 0x01;

        // Is this seek out of bounds?
        let drive = self.select_drive(drive_select);
        if drive.is_none() {
            self.last_error = DriveError::BadSeek;
            self.send_interrupt = true;
            log::warn!(
                "command_seek_head: invalid drive: drive:{} c: {} h: {}",
                drive_head_select,
                cylinder,
                head_select
            );
            return Continuation::CommandComplete;
        }

        if !drive
            .unwrap()
            .is_seek_valid(DiskChs::from((cylinder as u16, head_select, 1)))
        {
            self.last_error = DriveError::BadSeek;
            self.send_interrupt = true;
            log::warn!(
                "command_seek_head: invalid seek: drive:{} c: {} h: {}",
                drive_head_select,
                cylinder,
                head_select
            );
            return Continuation::CommandComplete;
        }

        // Seek to values given in command
        self.drives[drive_select].chs.seek(cylinder as u16, head_select, 1);

        log::trace!(
            "command_seek_head completed: {} new chs: {}",
            drive_head_select,
            self.drives[drive_select].chs
        );

        self.last_error = DriveError::NoError;
        self.send_interrupt = true;
        Continuation::CommandComplete
    }

    /// Perform the Read Data Command
    pub fn command_read_track(&mut self) -> Continuation {
        let drive_head_select = self.data_register_in.pop_front().unwrap();
        let cylinder = self.data_register_in.pop_front().unwrap();
        let head = self.data_register_in.pop_front().unwrap();
        let sector = self.data_register_in.pop_front().unwrap();
        let sector_size = self.data_register_in.pop_front().unwrap();
        let track_len = self.data_register_in.pop_front().unwrap();
        let gap3_len = self.data_register_in.pop_front().unwrap();
        let data_len = self.data_register_in.pop_front().unwrap();

        let drive_select = (drive_head_select & 0x03) as usize;
        let head_select = (drive_head_select >> 2) & 0x01;

        let chs = DiskChs::from((cylinder as u16, head, sector));

        if head != head_select {
            // Head and head_select should always match. Seems redundant
            log::warn!("command_read_track(): non-matching head specifiers");
        }

        // Set drive_select for status register reads
        let drive_opt = self.select_drive_mut(drive_select);

        if self.select_drive_mut(drive_select).is_some() {
            // Is there no disk in the drive?
            //
            // Initially I had this command send an interrupt and try to return some error code in the
            // sense bytes. However, that would give inconsistent results in DOS like garbled directory
            // listings, or produce a "General error" reading drive instead of "Not Ready".
            // Also, returning error codes would cause the BIOS to issue an error 601.
            // So, we just let this operation time out if no disk is present, and that seems to work.
            if !self.selected_drive().disk_present() {
                return Continuation::CommandComplete;
            }

            // Start read operation
            self.operation = Operation::ReadTrack(chs, sector_size, track_len, gap3_len, data_len);

            if self.dma {
                // Clear MRQ until operation completion so there is no attempt to read result values
                self.mrq = false;

                // DMA now in progress
                self.in_dma = true;
            }
            else {
                // When not in DMA mode, we can leave MRQ high and let the CPU poll for completion
                log::error!("command_read_track(): In PIO mode");
                self.mrq = true;
                self.in_dma = false;
            }

            // The IBM PC BIOS only seems to ever set a track_len of 8. How do we support 9 sector (365k) floppies?
            // Answer: DOS seems to know to request sector #9 and the BIOS doesn't complain

            log::trace!("command_read_track(): dhs:{:02X} drive:{} cyl:{} head:{} sector:{} sector_size:{} track_len:{} gap3_len:{} data_len:{} skip:{}",
            drive_head_select, drive_select, cylinder, head, sector, sector_size, track_len, gap3_len, data_len, self.command_skip);
            //log::trace!("command_read_sector: may operate on maximum of {} sectors", max_sectors);

            // Flag to set up transfer size later
            self.operation_init = false;

            // Keep running command until DMA transfer completes
            Continuation::ContinueAsOperation
        }
        else {
            self.last_error = DriveError::BadRead;
            self.send_interrupt = true;
            log::warn!(
                "command_read_track(): invalid drive: drive:{} c:{} h:{} s:{}",
                drive_select,
                cylinder,
                head,
                sector
            );
            Continuation::CommandComplete
        }
    }

    /// Perform the Read Data Command
    pub fn command_read_data(&mut self) -> Continuation {
        let drive_head_select = self.data_register_in.pop_front().unwrap();
        let cylinder = self.data_register_in.pop_front().unwrap();
        let head = self.data_register_in.pop_front().unwrap();
        let sector = self.data_register_in.pop_front().unwrap();
        let sector_size = self.data_register_in.pop_front().unwrap();
        let track_len = self.data_register_in.pop_front().unwrap();
        let gap3_len = self.data_register_in.pop_front().unwrap();
        let data_len = self.data_register_in.pop_front().unwrap();

        let drive_select = (drive_head_select & 0x03) as usize;
        let head_select = (drive_head_select >> 2) & 0x01;

        let chs = DiskChs::from((cylinder as u16, head, sector));

        if head != head_select {
            // Head and head_select should always match. Seems redundant
            log::warn!("command_read_sector: non-matching head specifiers");
        }

        // Set drive_select for status register reads
        let drive_opt = self.select_drive_mut(drive_select);

        if self.select_drive_mut(drive_select).is_some() {
            // Is there no disk in the drive?
            //
            // Initially I had this command send an interrupt and try to return some error code in the
            // sense bytes. However, that would give inconsistent results in DOS like garbled directory
            // listings, or produce a "General error" reading drive instead of "Not Ready".
            // Also, returning error codes would cause the BIOS to issue an error 601.
            // So, we just let this operation time out if no disk is present, and that seems to work.
            if !self.selected_drive().disk_present() {
                return Continuation::CommandComplete;
            }

            // Is this read out of bounds?
            if !self.selected_drive().is_id_valid(chs) {
                self.last_error = DriveError::BadRead;
                self.send_interrupt = true;
                log::warn!(
                    "command_read_sector: invalid chs: drive:{}, c:{} h:{} s:{}",
                    drive_select,
                    cylinder,
                    head,
                    sector
                );
                return Continuation::CommandComplete;
            }

            // Seek to values given in command
            self.selected_drive_mut().seek(chs);

            // Start read operation
            self.operation = Operation::ReadData(chs, sector_size, track_len, gap3_len, data_len);

            if self.dma {
                // Clear MRQ until operation completion so there is no attempt to read result values
                self.mrq = false;

                // DMA now in progress
                self.in_dma = true;
            }
            else {
                // When not in DMA mode, we can leave MRQ high and let the CPU poll for completion
                log::error!("command_read_sector: ########## IN PIO MODE ############");
                self.mrq = true;
                self.in_dma = false;
            }

            // The IBM PC BIOS only seems to ever set a track_len of 8. How do we support 9 sector (365k) floppies?
            // Answer: DOS seems to know to request sector #9 and the BIOS doesn't complain

            log::trace!("command_read_sector: dhs:{:02X} drive:{} cyl:{} head:{} sector:{} sector_size:{} track_len:{} gap3_len:{} data_len:{} skip:{}",
            drive_head_select, drive_select, cylinder, head, sector, sector_size, track_len, gap3_len, data_len, self.command_skip);
            //log::trace!("command_read_sector: may operate on maximum of {} sectors", max_sectors);

            // Flag to set up transfer size later
            self.operation_init = false;

            // Keep running command until DMA transfer completes
            Continuation::ContinueAsOperation
        }
        else {
            self.last_error = DriveError::BadRead;
            self.send_interrupt = true;
            log::warn!(
                "command_read_sector: invalid drive: drive:{} c:{} h:{} s:{}",
                drive_select,
                cylinder,
                head,
                sector
            );
            Continuation::CommandComplete
        }
    }

    /// Perform the Write Data Command
    pub fn command_write_data(&mut self) -> Continuation {
        let drive_head_select = self.data_register_in.pop_front().unwrap();
        let cylinder = self.data_register_in.pop_front().unwrap();
        let head = self.data_register_in.pop_front().unwrap();
        let sector = self.data_register_in.pop_front().unwrap();
        let sector_size = self.data_register_in.pop_front().unwrap();
        let track_len = self.data_register_in.pop_front().unwrap();
        let gap3_len = self.data_register_in.pop_front().unwrap();
        let data_len = self.data_register_in.pop_front().unwrap();

        let drive_select = (drive_head_select & 0x03) as usize;
        let head_select = (drive_head_select >> 2) & 0x01;

        let chs = DiskChs::from((cylinder as u16, head, sector));

        if head != head_select {
            log::warn!("command_write_sector: non-matching head specifiers");
        }

        let drive_opt = self.select_drive_mut(drive_select);

        if self.select_drive(drive_select).is_some() {
            // Seek to values given in command
            self.selected_drive_mut().seek(chs);

            // Start write operation
            self.operation = Operation::WriteData(cylinder, head, sector, sector_size, track_len, gap3_len, data_len);

            if self.dma {
                // Clear MRQ until operation completion so there is no attempt to read result values
                self.mrq = false;

                // DMA now in progress
                self.in_dma = true;
            }
            else {
                // When not in DMA mode, we can leave MRQ high and let the CPU poll for completion
                self.mrq = true;
                self.in_dma = false;
            }

            log::trace!(
                "command_write_sector: cyl:{} head:{} sector:{} sector_size:{} track_len:{} gap3_len:{} data_len:{}",
                cylinder,
                head,
                sector,
                sector_size,
                track_len,
                gap3_len,
                data_len
            );
            //log::trace!("command_read_sector: may operate on maximum of {} sectors", max_sectors);

            // Flag to set up transfer size later
            self.operation_init = false;

            // Keep running command until DMA transfer completes
            Continuation::ContinueAsOperation
        }
        else {
            self.last_error = DriveError::BadWrite;
            self.send_interrupt = true;
            log::warn!(
                "command_write_sector: invalid drive: drive:{} c:{} h:{} s:{}",
                drive_select,
                cylinder,
                head,
                sector
            );
            return Continuation::CommandComplete;
        }
    }

    /// Perform the Write Sector Command
    pub fn command_format_track(&mut self) -> Continuation {
        let drive_head_select = self.data_register_in.pop_front().unwrap();
        let sector_size = self.data_register_in.pop_front().unwrap();
        let track_len = self.data_register_in.pop_front().unwrap();
        let gap3_len = self.data_register_in.pop_front().unwrap();
        let fill_byte = self.data_register_in.pop_front().unwrap();

        let _drive_select = (drive_head_select & 0x03) as usize;
        let _head_select = (drive_head_select >> 2) & 0x01;

        // Start format operation
        self.operation_init = false;
        self.operation = Operation::FormatTrack(sector_size, track_len, gap3_len, fill_byte);

        if self.dma {
            // Clear MRQ until operation completion so there is no attempt to read result values
            self.mrq = false;

            // DMA now in progress
            self.in_dma = true;
        }
        else {
            // When not in DMA mode, we can leave MRQ high and let the CPU poll for completion
            self.mrq = true;
            self.in_dma = false;
        }

        log::trace!(
            "command_format_track: sector_size:{} track_len:{} gap3_len:{} fill_byte:{:02X}",
            sector_size,
            track_len,
            gap3_len,
            fill_byte
        );

        // Keep running command until DMA transfer completes
        Continuation::ContinueAsOperation
    }

    /// Perform the Read Sector ID Command
    pub fn command_read_sector_id(&mut self) -> Continuation {
        let drive_head_select = self.data_register_in.pop_front().unwrap();

        let drive_select = (drive_head_select & 0x03) as usize;
        let _head_select = (drive_head_select >> 2) & 0x01;

        self.send_results_phase(
            InterruptCode::NormalTermination,
            drive_select,
            self.drives[drive_select].chs,
            0x02,
        );

        self.send_interrupt = true;
        Continuation::CommandComplete
    }

    fn send_results_phase(&mut self, result: InterruptCode, drive_select: usize, chs: DiskChs, sector_size: u8) {
        /*
        let (ir_result, wp_flag) = match result {
            ControllerResult::Success(code) => (code, 0),
            ControllerResult::GeneralFailure(code) => (code, 0),
            ControllerResult::WriteProtectFailure => (InterruptCode::AbnormalTermination, 1),
        };*/

        // Create the 3 status bytes. Most of these are error flags of some sort
        let st0_byte = self.make_st0_byte(result, drive_select, false);
        let st1_byte = self.make_st1_byte(drive_select);
        let st2_byte = self.make_st2_byte(drive_select);

        // Push result codes into FIFO
        self.data_register_out.clear();
        self.data_register_out.push_back(st0_byte);
        self.data_register_out.push_back(st1_byte);
        self.data_register_out.push_back(st2_byte);

        self.data_register_out.push_back(chs.c() as u8);
        self.data_register_out.push_back(chs.h());
        self.data_register_out.push_back(chs.s());
        self.data_register_out.push_back(sector_size);

        self.send_data_register();

        // Clear error state
        self.last_error = DriveError::NoError;
    }

    fn operation_read_data_pio(&mut self, chs: DiskChs, sector_size: u8, track_len: u8) {
        if !self.operation_init {
            self.xfer_size_sectors = (track_len.saturating_sub(chs.s())) as usize + 1;
            self.xfer_completed_sectors = 0;
            // TODO: fixme for sector size
            self.xfer_size_bytes = self.xfer_size_sectors as usize * 512;

            self.pio_bytes_left = self.xfer_size_bytes;
            self.pio_byte_count = 0;
            self.pio_sector_byte_count = 0;
            self.operation_init = true;
        }

        if self.pio_bytes_left > 0 {
            // Calculate how many sectors we've done
            // TODO: fix me for sector size
            if (self.pio_bytes_left < self.xfer_size_bytes)
                && (self.pio_bytes_left % 512 == 0)
                && self.data_register_out.is_empty()
            {
                // Completed one sector
                self.xfer_completed_sectors += 1;
                self.pio_sector_byte_count = 0;
                log::trace!(
                    "operation_read_sector_pio: Transferred {}/{} sectors, {}/{} bytes ({} left)",
                    self.xfer_completed_sectors,
                    self.xfer_size_sectors,
                    self.pio_byte_count,
                    self.xfer_size_bytes,
                    self.pio_bytes_left
                );
            }

            if self.data_register_out.is_empty() {
                let byte = self.drives[self.drive_select].read_operation_buf();
                log::trace!(
                    "Read byte: {:02X}, bytes remaining: {} DR: {}",
                    byte,
                    self.pio_bytes_left,
                    self.data_register_out.len()
                );

                self.data_register_out.push_back(byte);
                self.pio_byte_count += 1;
                self.pio_sector_byte_count += 1;
                self.pio_bytes_left -= 1;
            }
        }
        else if self.data_register_out.is_empty() {
            // No more bytes left to transfer. Finalize operation
            self.pio_bytes_left = 0;
            self.pio_byte_count = 0;

            let (new_c, new_h, new_s) = self
                .selected_drive()
                .get_chs_sector_offset(self.xfer_size_sectors, chs)
                .into();
            //let (new_c, new_h, new_s) = self.get_next_sector(self.drive_select, cylinder, head, sector);

            let new_chs = DiskChs::new(new_c, new_h, new_s);

            // Terminate normally by sending results registers
            self.send_results_phase(
                InterruptCode::NormalTermination,
                self.drive_select,
                new_chs,
                sector_size,
            );

            // Seek to new CHS
            self.selected_drive_mut().seek(new_chs);

            log::trace!(
                "operation_read_sector_pio completed ({} bytes transferred): new chs: {} drive: {}",
                self.xfer_size_bytes - self.pio_bytes_left,
                &self.drives[self.drive_select].chs,
                self.drive_select
            );
            // Finalize operation
            self.operation = Operation::NoOperation;
            //self.send_interrupt = true;
        }
    }

    fn operation_read_data(
        &mut self,
        dma: &mut dma::DMAController,
        bus: &mut BusInterface,
        chs: DiskChs,
        n: u8,
        _track_len: u8,
    ) {
        if !self.in_dma {
            log::error!("FDC in invalid state: ReadSector operation without DMA! Aborting.");
            self.operation = Operation::NoOperation;
            return;
        }

        let sector_size_decoded = FloppyController::decode_sector_size(n);

        if !self.operation_init {
            let xfer_size = dma.get_dma_transfer_size(FDC_DMA);
            if xfer_size % sector_size_decoded != 0 {
                log::warn!("DMA word count not multiple of sector size");
            }

            let xfer_sectors = xfer_size / sector_size_decoded;
            log::trace!("DMA programmed for transfer of {} sectors", xfer_sectors);

            let dst_address = dma.get_dma_transfer_address(FDC_DMA);
            log::trace!("DMA destination address: {:05X}", dst_address);

            let skip_flag = self.command_skip;
            match self
                .selected_drive_mut()
                .command_read_data(chs, xfer_sectors, n, 0, 0, 0, skip_flag)
            {
                Ok(read_result) => {
                    log::trace!("Read sector command accepted, new sid: {}", read_result.new_sid);
                    self.operation_final_sid = read_result.new_sid;

                    if read_result.not_found {
                        self.send_results_phase(InterruptCode::AbnormalTermination, self.drive_select, chs, n);
                        self.operation = Operation::NoOperation;
                        self.send_interrupt = true;
                        return;
                    }
                }
                Err(e) => {
                    log::warn!("Read sector command failed: {:?}", e);
                    self.send_results_phase(InterruptCode::AbnormalTermination, self.drive_select, chs, n);
                    self.operation = Operation::NoOperation;
                    self.send_interrupt = true;
                    return;
                }
            }

            self.xfer_size_sectors = xfer_sectors;
            self.xfer_completed_sectors = 0;
            self.xfer_size_bytes = xfer_sectors * sector_size_decoded;
            self.dma_bytes_left = xfer_sectors * sector_size_decoded;
            self.operation_init = true;
        }

        if self.dma_bytes_left > 0 {
            // Bytes left to transfer

            // Calculate how many sectors we've done
            if (self.dma_bytes_left < self.xfer_size_bytes) && (self.dma_bytes_left % sector_size_decoded == 0) {
                // Completed one sector

                self.xfer_completed_sectors += 1;
                log::trace!(
                    "operation_read_sector: Transferred {} sectors.",
                    self.xfer_completed_sectors
                );
            }

            // Check if DMA is ready
            if dma.check_dma_ready(FDC_DMA) {
                let byte = self.drives[self.drive_select].read_operation_buf();

                dma.do_dma_write_u8(bus, FDC_DMA, byte);
                self.dma_byte_count += 1;
                self.dma_bytes_left -= 1;

                // See if we are done
                let tc = dma.check_terminal_count(FDC_DMA);
                if tc {
                    log::trace!(
                        "DMA terminal count triggered end of Sector Read operation, {} bytes read.",
                        self.dma_byte_count
                    );
                    self.dma_bytes_left = 0;
                }
            }
        }
        else {
            // No more bytes left to transfer. Finalize operation

            let tc = dma.check_terminal_count(FDC_DMA);
            if !tc {
                log::warn!("FDC sector read complete without DMA terminal count.");
            }

            self.dma_byte_count = 0;
            self.dma_bytes_left = 0;

            let new_chs = DiskChs::new(
                self.selected_drive().chs.c(),
                self.selected_drive().chs.h(),
                self.operation_final_sid,
            );
            log::debug!("Read operation completed: new chs: {}", new_chs);

            // Terminate normally by sending results registers
            self.send_results_phase(InterruptCode::NormalTermination, self.drive_select, new_chs, n);

            // Seek to new CHS
            self.drives[self.drive_select].chs.seek_to(&new_chs);

            log::trace!(
                "operation_read_sector completed: new chs: {}",
                &self.drives[self.drive_select].chs
            );
            // Finalize operation
            self.operation = Operation::NoOperation;
            self.send_interrupt = true;
        }
    }

    fn operation_write_data(
        &mut self,
        dma: &mut dma::DMAController,
        bus: &mut BusInterface,
        chs: DiskChs,
        sector_size: u8,
        _track_len: u8,
    ) {
        if !self.in_dma {
            log::error!("Error: WriteSector operation without DMA!");
            self.operation = Operation::NoOperation;
            return;
        }

        // Fail operation if disk is write protected
        if self.drives[self.drive_select].write_protected {
            log::warn!("WriteSector operation on write protected disk!");

            // Terminate with WriteProtect error.
            self.last_error = DriveError::WriteProtect;
            self.send_results_phase(InterruptCode::AbnormalPolling, self.drive_select, chs, sector_size);

            self.send_interrupt = true;
            self.operation = Operation::NoOperation;
            return;
        }

        let sector_size_bytes = DiskChsn::n_to_bytes(sector_size);

        if !self.operation_init {
            let xfer_size = dma.get_dma_transfer_size(FDC_DMA);

            if xfer_size % sector_size_bytes != 0 {
                log::warn!(
                    "DMA word count {} not multiple of sector size: {}",
                    xfer_size,
                    sector_size
                );
            }

            self.xfer_size_sectors = xfer_size / sector_size_bytes;
            log::trace!("DMA programmed for transfer of {} sectors", self.xfer_size_sectors);

            self.xfer_buffer = Vec::with_capacity(xfer_size);
            self.xfer_size_bytes = self.xfer_size_sectors * sector_size_bytes;
            self.dma_bytes_left = self.xfer_size_bytes;
            self.operation_init = true;
        }

        if self.dma_bytes_left == sector_size_bytes {
            let dst_address = dma.get_dma_transfer_address(FDC_DMA);
            log::trace!("DMA source address: {:05X}", dst_address)
        }

        if self.dma_bytes_left > 0 {
            // Bytes left to transfer

            // Check if DMA is ready
            if dma.check_dma_ready(FDC_DMA) {
                let byte = dma.do_dma_read_u8(bus, FDC_DMA);

                // TODO: Write byte to disk image
                //self.drives[self.drive_select].disk_image[byte_address] = byte;
                self.xfer_buffer.push(byte);

                self.dma_byte_count += 1;
                self.dma_bytes_left -= 1;

                // See if we are done
                let tc = dma.check_terminal_count(FDC_DMA);
                if tc {
                    log::trace!(
                        "DMA terminal count triggered end of Sector Write operation, {} byte(s) written.",
                        self.dma_byte_count
                    );
                    self.dma_bytes_left = 0;
                }
            }
        }
        else {
            // No more bytes left to transfer. Finalize operation

            let tc = dma.check_terminal_count(FDC_DMA);
            if !tc {
                log::warn!("FDC sector write complete without DMA terminal count.");
            }

            // Xfer buffer is full, write sectors to disk

            let ct = self.xfer_size_sectors;
            let write_result =
                self.drives[self.drive_select].command_write_data(chs, ct, sector_size, &self.xfer_buffer, false);

            match write_result {
                Ok(write_result) => {
                    self.dma_byte_count = 0;
                    self.dma_bytes_left = 0;

                    let (new_c, new_h, new_s) = self
                        .selected_drive()
                        .get_chs_sector_offset(self.xfer_completed_sectors + 1, chs)
                        .into();

                    let new_chs = DiskChs::new(
                        self.selected_drive().chs.c(),
                        self.selected_drive().chs.h(),
                        write_result.new_sid,
                    );

                    //let (new_c, new_h, new_s) = self.get_next_sector(self.drive_select, chs.c(), chs.h(), chs.s());
                    let new_chs = DiskChs::new(new_c, new_h, new_s);

                    // Terminate normally by sending results registers
                    self.send_results_phase(
                        InterruptCode::NormalTermination,
                        self.drive_select,
                        new_chs,
                        sector_size,
                    );

                    // Set new CHS
                    self.drives[self.drive_select].chs.seek_to(&new_chs);

                    // Finalize operation
                    self.operation = Operation::NoOperation;
                    self.send_interrupt = true;
                }
                Err(e) => {
                    log::error!("Drive reported write data command failed: {:?}", e);
                    self.send_results_phase(InterruptCode::AbnormalTermination, self.drive_select, chs, sector_size);
                    self.operation = Operation::NoOperation;
                    self.send_interrupt = true;
                    return;
                }
            }
        }
    }

    /// Run the Read Track operation
    fn operation_read_track(
        &mut self,
        dma: &mut dma::DMAController,
        bus: &mut BusInterface,
        ch: DiskCh,
        n: u8,
        eot: u8,
    ) {
        if !self.in_dma {
            log::error!("FDC in invalid state: ReadTrack operation without DMA! Aborting.");
            self.operation = Operation::NoOperation;
            return;
        }

        let sector_size_decoded = FloppyController::decode_sector_size(n);

        if !self.operation_init {
            let xfer_size = dma.get_dma_transfer_size(FDC_DMA);
            if xfer_size % sector_size_decoded != 0 {
                log::warn!(
                    "operation_read_track(): DMA word count {} not multiple of sector size ({})",
                    xfer_size,
                    sector_size_decoded
                );
            }

            let xfer_sectors = xfer_size / sector_size_decoded;
            log::trace!(
                "operation_read_track(): DMA programmed for transfer of {} sectors",
                xfer_sectors
            );

            let dst_address = dma.get_dma_transfer_address(FDC_DMA);
            log::trace!("operation_read_track(): DMA destination address: {:05X}", dst_address);

            match self
                .selected_drive_mut()
                .command_read_track(ch, n, eot, Some(xfer_size))
            {
                Ok(read_result) => {
                    log::trace!(
                        "operation_read_track(): Read track command accepted, new sid: {}",
                        read_result.new_sid
                    );
                    self.operation_final_sid = read_result.new_sid;

                    if read_result.not_found {
                        self.send_results_phase(
                            InterruptCode::AbnormalTermination,
                            self.drive_select,
                            DiskChs::from((ch, read_result.new_sid)),
                            n,
                        );
                        self.operation = Operation::NoOperation;
                        self.send_interrupt = true;
                        return;
                    }
                }
                Err(e) => {
                    log::error!("operation_read_track(): Read track command failed: {:?}", e);
                    self.send_results_phase(
                        InterruptCode::AbnormalTermination,
                        self.drive_select,
                        DiskChs::from((ch, 1)),
                        n,
                    );
                    self.operation = Operation::NoOperation;
                    self.send_interrupt = true;
                    return;
                }
            }

            self.xfer_size_sectors = xfer_sectors;
            self.xfer_completed_sectors = 0;
            self.xfer_size_bytes = xfer_sectors * sector_size_decoded;
            self.dma_bytes_left = xfer_sectors * sector_size_decoded;
            self.operation_init = true;
        }

        if self.dma_bytes_left > 0 {
            // Bytes left to transfer

            // Calculate how many sectors we've done
            if (self.dma_bytes_left < self.xfer_size_bytes) && (self.dma_bytes_left % sector_size_decoded == 0) {
                // Completed one sector

                self.xfer_completed_sectors += 1;
                log::trace!(
                    "operation_read_track():  Transferred {} sectors.",
                    self.xfer_completed_sectors
                );
            }

            // Check if DMA is ready
            if dma.check_dma_ready(FDC_DMA) {
                let byte = self.drives[self.drive_select].read_operation_buf();

                dma.do_dma_write_u8(bus, FDC_DMA, byte);
                self.dma_byte_count += 1;
                self.dma_bytes_left -= 1;

                // See if we are done
                let tc = dma.check_terminal_count(FDC_DMA);
                if tc {
                    log::trace!(
                        "operation_read_track(): DMA terminal count triggered end of Sector Read operation, {} bytes read.",
                        self.dma_byte_count
                    );
                    self.dma_bytes_left = 0;
                }
            }
        }
        else {
            // No more bytes left to transfer. Finalize operation

            let tc = dma.check_terminal_count(FDC_DMA);
            if !tc {
                log::warn!("operation_read_track(): Read Track complete without DMA terminal count.");
            }

            self.dma_byte_count = 0;
            self.dma_bytes_left = 0;

            let new_chs = DiskChs::new(
                self.selected_drive().chs.c(),
                self.selected_drive().chs.h(),
                self.operation_final_sid,
            );
            log::debug!("operation_read_track(): operation completed: new chs: {}", new_chs);

            // Terminate normally by sending results registers
            self.send_results_phase(InterruptCode::NormalTermination, self.drive_select, new_chs, n);

            // Seek to new CHS and finalize operation
            self.drives[self.drive_select].chs.seek_to(&new_chs);
            self.operation = Operation::NoOperation;
            self.send_interrupt = true;
        }
    }

    /// Run the Format Track Operation
    ///
    /// DOS will program DMA for the entire track length, but we only read track_len * 4 bytes from DMA
    /// to read in the format buffers for each sector
    fn operation_format_track(
        &mut self,
        dma: &mut dma::DMAController,
        bus: &mut BusInterface,
        sector_size: u8,
        track_len: u8,
        _gap3_len: u8,
        fill_byte: u8,
    ) {
        if !self.in_dma {
            log::error!("Error: Format Track operation without DMA!");
            self.operation = Operation::NoOperation;
            return;
        }

        // Fail operation if disk is write protected
        if self.drives[self.drive_select].write_protected {
            log::warn!("FormatTrack operation on write protected disk!");

            // Terminate with WriteProtect error.
            self.last_error = DriveError::WriteProtect;
            self.send_results_phase(
                InterruptCode::AbnormalPolling,
                self.drive_select,
                Default::default(),
                sector_size,
            );

            self.send_interrupt = true;
            self.operation = Operation::NoOperation;
            return;
        }

        if !self.operation_init {
            let xfer_size = dma.get_dma_transfer_size(FDC_DMA);

            if xfer_size < (track_len as usize * FORMAT_BUFFER_SIZE) {
                log::error!(
                    "Format Track: DMA word count too small for track_len({:02}) format buffers.",
                    track_len
                );
                self.operation = Operation::NoOperation;
                return;
            }

            // TODO: fix me for sector size
            let xfer_sectors = xfer_size / 512;
            log::trace!("Format Track: DMA programmed for transfer of {} sectors", xfer_sectors);

            self.dma_bytes_left = track_len as usize * FORMAT_BUFFER_SIZE;
            self.operation_init = true;
        }

        if self.dma_bytes_left > 0 {
            // Bytes left to transfer

            // Check if DMA is ready
            if dma.check_dma_ready(FDC_DMA) {
                let byte = dma.do_dma_read_u8(bus, FDC_DMA);
                self.format_buffer.push_back(byte);
                self.dma_bytes_left = self.dma_bytes_left.saturating_sub(1);
            }

            // Have we read in all 4 bytes of a format buffer? Format the sector specified by the buffer.
            if self.format_buffer.len() == FORMAT_BUFFER_SIZE {
                let f_cylinder = self.format_buffer.pop_front().unwrap();
                let f_head = self.format_buffer.pop_front().unwrap();
                let f_sector = self.format_buffer.pop_front().unwrap();
                let f_sector_size = self.format_buffer.pop_front().unwrap();

                log::trace!(
                    "Formatting cylinder: {} head: {} sector: {} size: {} with byte: {:02X}",
                    f_cylinder,
                    f_head,
                    f_sector,
                    f_sector_size,
                    fill_byte
                );

                self.format_sector(f_cylinder, f_head, f_sector, fill_byte);
                self.send_interrupt = true;

                // Clear for next 4 bytes
                self.format_buffer.clear();
            }
        }
        else {
            // No more bytes left to transfer. Finalize operation

            //let tc = dma.check_terminal_count(FDC_DMA);
            //if !tc {
            //    log::warn!("FDC Format Track complete without DMA terminal count.");
            //}

            self.dma_byte_count = 0;
            self.dma_bytes_left = 0;

            //let (new_c, new_h, new_s) = self.get_next_sector(self.drive_select, cylinder, head, sector);

            // Terminate normally by sending results registers

            // Note the u765a whitepaper says this about the result codes of the Format Track command:
            // "In this case, the ID information has no meaning"
            self.send_results_phase(
                InterruptCode::NormalTermination,
                self.drive_select,
                Default::default(), // Default CHS
                sector_size,
            );

            // Set new CHS
            //self.drives[self.drive_select].cylinder = new_c;
            //self.drives[self.drive_select].head = new_h;
            //self.drives[self.drive_select].sector = new_s;

            // Finalize operation
            self.operation = Operation::NoOperation;
            self.send_interrupt = true;
        }
    }

    pub fn format_sector(&mut self, _cylinder: u8, _head: u8, _sector: u8, _fill_byte: u8) {}

    /// Run the Floppy Drive Controller. Process running Operations.
    pub fn run(&mut self, dma: &mut dma::DMAController, bus: &mut BusInterface, us: f64) {
        self.us_accumulator += us;

        if self.watchdog_triggered {
            self.watchdog_accumulator += us;
            if self.watchdog_enabled && self.watchdog_accumulator > WATCHDOG_TIMEOUT {
                log::warn!("FDC watchdog timeout!");
                self.watchdog_triggered = false;
                self.watchdog_accumulator = 0.0;
                self.operation = Operation::NoOperation;
                self.send_interrupt = true;
            }
        }

        // Send an interrupt if one is queued
        if self.send_interrupt {
            bus.pic_mut().as_mut().unwrap().request_interrupt(FDC_IRQ);
            self.pending_interrupt = true;
            self.send_interrupt = false;
        }

        // End an interrupt if one was handled
        if self.end_interrupt {
            bus.pic_mut().as_mut().unwrap().clear_interrupt(FDC_IRQ);
            self.pending_interrupt = false;
            self.end_interrupt = false;
        }

        // Run operation
        #[allow(unreachable_patterns)]
        match self.operation {
            Operation::NoOperation => {
                // Do nothing
            }
            Operation::ReadData(chs, sector_size, track_len, _gap3_len, _data_len) => match self.dma {
                true => self.operation_read_data(dma, bus, chs, sector_size, track_len),
                false => self.operation_read_data_pio(chs, sector_size, track_len),
            },
            Operation::WriteData(cylinder, head, sector, sector_size, track_len, _gap3_len, _data_len) => self
                .operation_write_data(
                    dma,
                    bus,
                    DiskChs::from((cylinder as u16, head, sector)),
                    sector_size,
                    track_len,
                ),
            Operation::ReadTrack(chs, sector_size, track_len, _gap3_len, _data_len) => {
                self.operation_read_track(dma, bus, chs.into(), sector_size, track_len)
            }
            Operation::FormatTrack(sector_size, track_len, gap3_len, fill_byte) => {
                self.operation_format_track(dma, bus, sector_size, track_len, gap3_len, fill_byte)
            }
            _ => {
                log::error!("Invalid FDC operation: {:?}", self.operation)
            }
        }
    }
}

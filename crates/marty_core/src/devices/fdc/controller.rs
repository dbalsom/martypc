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

    devices::fdc.rs

    Implements the NEC µPD765 Floppy Disk Controller
*/

#![allow(dead_code)]

use std::{
    collections::VecDeque,
    default::Default,
    fmt::Display,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use crate::{
    bus::{BusInterface, DeviceRunTimeUnit, IoDevice},
    cpu_common::LogicAnalyzer,
    device_types::fdc::FloppyImageType,
    devices::{
        dma,
        floppy_drive::{FloppyDiskDrive, FloppyImageState},
    },
    machine_config::FloppyDriveConfig,
    machine_types::FdcType,
};

use marty_common::types::history_buffer::HistoryBuffer;

use anyhow::{anyhow, Error};
use fluxfox::prelude::*;
use modular_bitfield::{bitfield, prelude::*};

pub const FDC_LOG_LEN: usize = 1000;

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

pub const FDC_RESET_TIME: f64 = 1000.0; // 1ms in microseconds
pub const FDC_SEEK_TIME: f64 = 10000.0; // 10ms in microseconds

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
// time out waiting for it.
pub const FDC_STATUS_DIO: u8 = 0b0100_0000;

// MRQ (Main Request) is also used to determine if the data port is ready to be
// written to or read. If this bit is not set the BIOS will time out waiting for it.
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
pub const COMMAND_FIX_DRIVE_DATA: u8 = 0x03;
pub const COMMAND_CHECK_DRIVE_STATUS: u8 = 0x04;
pub const COMMAND_WRITE_DATA: u8 = 0x05;
pub const COMMAND_READ_DATA: u8 = 0x06;
pub const COMMAND_CALIBRATE_DRIVE: u8 = 0x07;
pub const COMMAND_SENSE_INT_STATUS: u8 = 0x08;
pub const COMMAND_WRITE_DELETED_DATA: u8 = 0x09;
pub const COMMAND_READ_SECTOR_ID: u8 = 0x0A;
pub const COMMAND_READ_DELETED_DATA: u8 = 0x0C;
pub const COMMAND_FORMAT_TRACK: u8 = 0x0D;
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

pub const ST2_NO_DAM: u8 = 0b0000_0001;
//pub const ST2_BAD_CYLINDER: u8 = 0b0000_0010;
pub const ST2_WRONG_CYLINDER: u8 = 0b0001_0000;
pub const ST2_DATA_CRC_ERROR: u8 = 0b0010_0000;
pub const ST2_DAD_MARK: u8 = 0b0100_0000;

pub const ST3_ESIG: u8 = 0b1000_0000;
pub const ST3_WRITE_PROTECT: u8 = 0b0100_0000;
pub const ST3_READY: u8 = 0b0010_0000;
pub const ST3_TRACK0: u8 = 0b0001_0000;
pub const ST3_DOUBLESIDED: u8 = 0b0000_1000;
pub const ST3_HEAD: u8 = 0b0000_0100;

/// Represent the state of the DIO bit of the Main Status Register in a readable way.
#[derive(Copy, Clone, Debug, Default)]
pub enum IoMode {
    #[default]
    ToCpu,
    FromCpu,
}

/// Represent the various commands that the NEC FDC knows how to handle.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default)]
pub enum Command {
    #[default]
    NoCommand = 0x00,
    ReadTrack = 0x02,
    WriteData = 0x05,
    ReadData = 0x06,
    WriteDeletedSector = 0x09,
    ReadDeletedSector = 0x0c,
    FormatTrack = 0x0d,
    FixDriveData = 0x03,
    CheckDriveStatus = 0x04,
    CalibrateDrive = 0x07,
    SenseIntStatus = 0x08,
    ReadSectorID = 0x0a,
    SeekParkHead = 0x0f,
    Invalid = 0xff,
}

/// Represents the current phase of the controller operation.
#[derive(Copy, Clone, Debug)]
pub enum ControllerPhase {
    CommandPhase,
    ExecutionPhase,
    ResultPhase,
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

#[derive(Copy, Clone, Debug, Default)]
pub enum DataMode {
    #[default]
    Pio,
    Dma,
}

/// An [Operation] is initiated by any controller command that does not immediately terminate.
/// The operation handler is called on a repeated basis by the fdc's run() method until the
/// operation is complete or the controller is reset.
#[derive(Copy, Clone, Debug, Default)]
pub enum Operation {
    #[default]
    NoOperation,
    Reset,
    Calibrate,
    Seek,
    ReadData(u8, DiskChs, u8, u8, u8, u8), // Physical head, id CHS, sector_size, track_len, gap3_len, data_len
    ReadTrack(u8, DiskChs, u8, u8, u8, u8), // Physical head, CHS, sector_size, track_len, gap3_len, data_len
    WriteData(u8, DiskChs, u8, u8, u8, u8, bool), // Physical head, id CHS, sector_size, track_len, gap3_len, data_len, deleted_data
    FormatTrack(u8, u8, u8, u8, u8),              // head_select, sector_size, track_len, gap3_len, fill_byte
}

impl Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Operation::NoOperation => write!(f, "No Operation"),
            Operation::Reset => write!(f, "Reset"),
            Operation::Calibrate => write!(f, "Calibrate"),
            Operation::Seek => write!(f, "Seek"),
            Operation::ReadData(_, _chs, _, _, _, _) => write!(f, "Read Data"),
            Operation::ReadTrack(_, _chs, _, _, _, _) => write!(f, "Read Track"),
            Operation::WriteData(_, _chs, _, _, _, _, _) => write!(f, "Write Data"),
            Operation::FormatTrack(_, _, _, _, _) => write!(f, "Format Track"),
        }
    }
}

type CommandDispatchFn = fn(&mut FloppyController) -> Continuation;
pub enum Continuation {
    CommandComplete,
    ContinueAsOperation,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct CommandByte {
    pub command: B5,
    pub skip: bool,
    pub mfm: bool,
    pub mt: bool,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct DriveHeadSelect {
    pub drive: B2,
    pub head:  B1,
    #[skip]
    unused:    B5,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct StepRateHeadUnload {
    pub head_unload: B4,
    pub step_rate:   B4,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct HeadLoadDma {
    pub non_dma:   bool,
    pub head_load: B7,
}

#[derive(Default)]
pub struct FdcDebugState {
    pub intr: bool,
    pub dor: u8,
    pub data_mode: DataMode,
    pub operation: Operation,
    pub last_cmd: Command,
    pub last_status: Vec<u8>,
    pub drive_select: usize,
    pub status_register: u8,
    pub data_register_in: Vec<u8>,
    pub data_register_out: Vec<u8>,
    pub last_data_read: u8,
    pub last_data_written: u8,
    pub dio: IoMode,
    pub st3: u8,
    pub cmd_log: Vec<String>,
}

pub struct FloppyController {
    phase: ControllerPhase,
    us_accumulator: f64,
    watchdog_accumulator: f64,
    operation_accumulator: f64,
    fdc_type: FdcType,
    status_byte: u8,
    reset_flag: bool,
    reset_sense_count: u8,
    mrq: bool,

    data_register: u8,
    dor: u8,
    dor_dma: bool,
    dor_disabled: bool,
    dma: bool,
    busy: bool,
    dio: IoMode,
    mt: bool,
    reading_command: bool,
    command: Command,
    command_fn: Option<CommandDispatchFn>,
    last_command: Command,
    receiving_command: bool,
    command_byte_n: u32,
    command_skip: bool,
    command_deleted: bool,
    operation: Operation,
    operation_init: bool,
    operation_final_chs: DiskChs,
    send_interrupt: bool,
    pending_interrupt: bool,
    end_interrupt: bool,
    watchdog_enabled: bool,     // IBM PCJr only.  Watchdog timer enabled.
    watchdog_trigger_bit: bool, // IBM PCJr only.  Watchdog timer trigger bit status.
    watchdog_triggered: bool,   // IBM PCJr only.  Watchdog timer triggered.

    last_error: DriveError,
    last_status_bytes: Vec<u8>,

    data_register_out: VecDeque<u8>,
    data_register_in: VecDeque<u8>,
    last_data_read: u8,
    last_data_written: u8,
    last_st3: u8,
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

    cmd_log: HistoryBuffer<String>,
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

    fn write_u8(
        &mut self,
        port: u16,
        data: u8,
        _bus: Option<&mut BusInterface>,
        _delta: DeviceRunTimeUnit,
        _analyzer: Option<&mut LogicAnalyzer>,
    ) {
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
            phase: ControllerPhase::CommandPhase,
            us_accumulator: 0.0,
            watchdog_accumulator: 0.0,
            operation_accumulator: 0.0,
            fdc_type: FdcType::IbmNec,
            status_byte: 0,
            reset_flag: false,
            reset_sense_count: 0,
            mrq: true,
            data_register: 0,
            dor_dma: true,
            dor: 0,
            dor_disabled: false,
            dma: true,
            busy: false,
            dio: IoMode::FromCpu,
            mt: false,
            reading_command: false,
            command: Command::NoCommand,
            command_fn: None,
            last_command: Command::NoCommand,
            command_byte_n: 0,
            receiving_command: false,
            command_skip: false,
            command_deleted: false,
            operation: Operation::NoOperation,
            operation_init: false,
            operation_final_chs: DiskChs::default(),

            last_error: DriveError::NoError,
            last_status_bytes: vec![0; 3],

            send_interrupt: false,
            pending_interrupt: false,
            end_interrupt: false,
            watchdog_enabled: false,
            watchdog_trigger_bit: false,
            watchdog_triggered: false,

            data_register_out: VecDeque::new(),
            data_register_in: VecDeque::new(),
            last_data_read: 0,
            last_data_written: 0,
            last_st3: 0,
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

            cmd_log: HistoryBuffer::new(FDC_LOG_LEN),
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

    pub fn reset(&mut self) {
        self.reset_internal(false);
    }

    /// Reset the Floppy Drive Controller
    pub fn reset_internal(&mut self, internal: bool) {
        // TODO: Implement in terms of Default
        self.phase = ControllerPhase::CommandPhase;
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
        self.last_status_bytes = vec![0; 3];
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

        if !internal {
            self.cmd_log.clear();
        }

        self.log_str("FDC Reset!");
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
    pub fn load_image_from(
        &mut self,
        drive_select: usize,
        src_vec: Vec<u8>,
        path: Option<&Path>,
        write_protect: bool,
    ) -> Result<Arc<RwLock<DiskImage>>, Error> {
        if drive_select >= self.drive_ct {
            return Err(anyhow!("Invalid drive selection"));
        }

        self.drives[drive_select].load_image_from(src_vec, path, write_protect)
    }

    pub fn attach_image(
        &mut self,
        drive_select: usize,
        image: DiskImage,
        path: Option<PathBuf>,
        write_protect: bool,
    ) -> Result<Arc<RwLock<DiskImage>>, Error> {
        if drive_select >= self.drive_ct {
            return Err(anyhow!("Invalid drive selection"));
        }
        let drive = &mut self.drives[drive_select];
        drive.attach_image(image, path, write_protect)
    }

    pub fn get_image(&mut self, drive_select: usize) -> (Option<Arc<RwLock<DiskImage>>>, u64) {
        self.drives[drive_select].get_image()
    }

    /// Unload (eject) the disk in the specified drive
    pub fn unload_image(&mut self, drive_select: usize) {
        let drive = &mut self.drives[drive_select];

        drive.unload_image();
    }

    pub fn create_new_image(
        &mut self,
        drive_select: usize,
        format: StandardFormat,
        formatted: bool,
    ) -> Result<Arc<RwLock<DiskImage>>, Error> {
        let drive = &mut self.drives[drive_select];

        drive.create_new_image(format, formatted)
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

    pub fn handle_status_register_read(&self) -> u8 {
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

    pub fn set_phase(&mut self, new_phase: ControllerPhase) {
        use ControllerPhase::*;
        match (&self.phase, &new_phase) {
            (CommandPhase, ExecutionPhase) => {
                self.phase = ExecutionPhase;
            }
            (ExecutionPhase, ResultPhase) => {
                self.phase = ResultPhase;
            }
            (ResultPhase, CommandPhase) => {
                self.phase = CommandPhase;
            }
            _ => {
                log::error!("set_phase(): Bad phase transition: {:?}->{:?}", self.phase, new_phase);
            }
        }
    }

    pub fn handle_dor_write(&mut self, data: u8) {
        // Handle controller enable bit
        if data & DOR_FDC_RESET == 0 {
            self.mrq = false;
            self.dor_disabled = true;
            self.log_str(&format!("FDC Disabled via DOR write: {:02X}", data));
        }
        else if self.dor_disabled {
            // Reset the FDC when the reset bit is *not* set
            // Ignore all other commands
            self.log_str(&format!("FDC Reset requested via DOR write: {:02X}", data));
            self.operation_accumulator = 0.0;
            self.operation = Operation::Reset;
            self.mrq = false;
            self.dor_disabled = false;
        }

        // Turn drive motors on or off based on the MOTx bits in the DOR byte.
        for i in 0..4 {
            if data & (0x10 << i) != 0 {
                self.motor_on(i);
            }
            else {
                self.motor_off(i);
            }
        }

        self.dor_dma = data & DOR_DMA_ENABLED != 0;

        // Select drive from DRx bits.
        let disk_n = data & 0x03;
        self.drive_select = disk_n as usize;
        if self.drives[disk_n as usize].motor_on {
            log::debug!("Drive {} selected, motor on", disk_n);
            self.drives[disk_n as usize].motor_on = true;
        }
        else {
            log::debug!("Drive {} selected, motor off", disk_n);
        }

        self.dor = data;
    }

    pub fn handle_dor_write_jr(&mut self, data: u8) {
        // Handle controller enable bit
        if data & DOR_JRFDC_RESET == 0 {
            // Reset the FDC when the reset bit is *not* set
            // Ignore all other commands
            self.mrq = false;
            self.dor_disabled = true;
            self.log_str(&format!("PCjr FDC Disabled via DOR write: {:02X}", data));
        }
        else if self.dor_disabled {
            // Reset the FDC when the reset bit is *not* set
            // Ignore all other commands
            self.log_str(&format!("PCjr FDC Reset requested via DOR write: {:02X}", data));
            self.operation_accumulator = 0.0;
            self.operation = Operation::Reset;
            self.mrq = false;
            self.dor_disabled = false;
        }

        // Not reset. Turn drive motors on or off based on the drive enable bit.
        if data & DOR_JRFDC_MOTOR != 0 {
            self.motor_on(0);
        }
        else {
            self.motor_off(0);
        }

        if data & DOR_DMA_ENABLED != 0 {
            log::error!("PCJr FDC DMA was erroneously enabled");
            self.dor_dma = true;
        }
        else {
            self.dor_dma = false;
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
        if self.drives[drive_select].chsn.h() == 1 {
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
        if status.address_crc_error | status.data_crc_error | status.no_dam {
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

        //log::trace!("ST0 byte: {:08b}", st0);
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
        if status.no_dam {
            st1_byte |= ST1_NO_ID;
        }
        if status.sector_not_found {
            st1_byte |= ST1_NODATA;
        }
        if status.address_crc_error | status.data_crc_error {
            st1_byte |= ST1_CRC_ERROR;
        }

        //log::trace!("ST1 byte: {:08b}", st1_byte);
        st1_byte
    }

    /// Generate the value of the ST2 Status Register in response to a command
    pub fn make_st2_byte(&self, drive_select: usize) -> u8 {
        // The ST2 status register contains mostly error codes. CRC errors are reported here.
        let mut st2 = 0;
        let status = self.drives[drive_select].get_operation_status();

        if !status.address_crc_error && status.data_crc_error {
            // Set the data CRC error bit - this cannot be set of if address crc error occurred,
            // as we should not have read any data.
            st2 |= ST2_DATA_CRC_ERROR;
        }
        if status.wrong_cylinder {
            // IDAM scan found a sector with the correct ID field except for cylinder
            st2 |= ST2_WRONG_CYLINDER;
        }
        if status.deleted_mark {
            st2 |= ST2_DAD_MARK;
        }
        if status.no_dam {
            st2 |= ST2_NO_DAM;
        }

        //log::trace!("ST2 byte: {:08b}", st2);
        st2
    }

    /// Generate the value of the ST3 Status Register
    /// ST3 is typically sent in response to Check Drive Status.
    pub fn make_st3_byte(&mut self, drive_select: usize) -> u8 {
        // Set drive select bits DS0 & DS1
        let mut st3_byte = (drive_select & 0x03) as u8;

        // HDSEL signal: 1 == head 1 active
        if self.drives[drive_select].chsn.h() == 1 {
            st3_byte |= ST3_HEAD;
        }

        // DSDR signal - Is this active for a double-sided drive, or only when a double-sided disk is present?
        st3_byte |= ST3_DOUBLESIDED;

        if self.drives[drive_select].chsn.c() == 0 {
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

        //log::trace!("make_st3_byte(): byte is {:02X}", st3_byte);
        self.last_st3 = st3_byte;
        st3_byte
    }

    pub fn handle_data_register_read(&mut self) -> u8 {
        let mut out_byte = 0;

        if !self.data_register_out.is_empty() {
            out_byte = self.data_register_out.pop_front().unwrap();
            if self.data_register_out.is_empty() {
                log::trace!("handle_data_register_read(): Popped last byte, clearing busy flag");
                // CPU has read all available bytes
                self.busy = false;
                self.dio = IoMode::FromCpu;
            }
        }

        //log::trace!("Data Register Read: {:02X}", out_byte );
        self.last_data_read = out_byte;
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
        self.last_data_written = data;
        //log::trace!("Data Register Write");
        if !self.receiving_command {
            let command_byte = CommandByte::from_bytes([data]);
            let command = command_byte.command();
            self.mt = command_byte.mt();
            self.command_skip = command_byte.skip();
            self.command_deleted = false;
            match command {
                COMMAND_READ_TRACK => {
                    log::trace!("Received Read Track command: {:02}", command);
                    self.set_command(Command::ReadTrack, 8, FloppyController::command_read_track);
                }
                COMMAND_WRITE_DATA => {
                    log::trace!("Received Write Data command: {:02}", command);
                    self.set_command(Command::WriteData, 8, FloppyController::command_write_data);
                }
                COMMAND_READ_DATA => {
                    log::trace!("Received Read Data command: {:02X} {:02}", data, command);
                    self.set_command(Command::ReadData, 8, FloppyController::command_read_data);
                }
                COMMAND_WRITE_DELETED_DATA => {
                    self.command_deleted = true;
                    log::warn!("Received Write Deleted Data command: {:02}", command);
                    self.set_command(Command::WriteData, 8, FloppyController::command_write_data);
                }
                COMMAND_READ_DELETED_DATA => {
                    self.command_deleted = true;
                    log::trace!("Received Read Deleted Data command: {:02}", command);
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
        let mut send_cylinder = true;

        let log_str = format!(
            "Last command: {:?}, Last error: {:?}, reset flag: {}, pending interrupt: {}",
            self.last_command, self.last_error, self.reset_flag, self.pending_interrupt
        );
        self.log_cmd(Command::SenseIntStatus, "command_sense_interrupt", &log_str);

        if self.reset_flag {
            // FDC was just reset, answer with an ST0 for the first drive, but prepare to send up
            // to three more ST0 responses
            st0_byte |= ST0_RESET;
            self.reset_sense_count = 1;
            self.reset_flag = false;
        }
        else if matches!(self.last_command, Command::SenseIntStatus) {
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
                log::warn!("Exceeded four sense interrupts");
                st0_byte = ST0_INVALID_OPCODE;
                self.reset_sense_count = 0;
                send_cylinder = false;
            }
        }
        else {
            // Sense interrupt in response to some other command
            if self.pending_interrupt {
                log::trace!(
                    "command_sense_interrupt(): Last command: {:?}, Last error: {:?}, pending interrupt: {}",
                    self.last_command,
                    self.last_error,
                    self.pending_interrupt
                );

                let seek_flag = matches!(self.last_command, Command::CalibrateDrive | Command::SeekParkHead);

                let code = match self.last_error {
                    DriveError::BadRead | DriveError::BadWrite | DriveError::BadSeek => {
                        InterruptCode::AbnormalTermination
                    }
                    _ => InterruptCode::NormalTermination,
                };
                st0_byte = self.make_st0_byte(code, self.drive_select, seek_flag);
            }
            else {
                log::warn!("Sense interrupt received without pending interrupt");
                // Sense Interrupt without pending interrupt is invalid
                st0_byte = ST0_INVALID_OPCODE;
                send_cylinder = false;
            }
        }

        // Send ST0 register to FIFO
        let cb0 = st0_byte;
        self.data_register_out.push_back(cb0);

        if send_cylinder {
            // Send Current Cylinder to FIFO
            let cb1 = self.drives[self.drive_select].chsn.c();
            self.data_register_out.push_back(cb1 as u8);
        }

        self.last_command = Command::SenseIntStatus;
        // We have data for CPU to read
        self.last_status_bytes[0] = st0_byte;
        self.command = Command::NoCommand;
        self.end_interrupt = true;
        self.send_data_register();

        log::trace!(
            "command_sense_interrupt() completed. Pushed {} bytes to data register.",
            self.data_register_out.len()
        );
    }

    /// Perform the Fix Drive Data command.
    /// We don't do anything currently with the provided values which are only useful for real drive timings,
    /// except for the ndma bit which controls DMA/PIO mode.
    pub fn command_fix_drive_data(&mut self) -> Continuation {
        let steprate_unload = StepRateHeadUnload::from_bytes([self.data_register_in.pop_front().unwrap()]);
        let headload_ndm = HeadLoadDma::from_bytes([self.data_register_in.pop_front().unwrap()]);

        let log_str = format!(
            "step rate: {:04b} unload_time: {:04b}, head_load: {:07b} pio_mode: {}",
            steprate_unload.step_rate(),
            steprate_unload.head_unload(),
            headload_ndm.head_load(),
            headload_ndm.non_dma(),
        );

        self.dma = !headload_ndm.non_dma();
        self.log_cmd(Command::FixDriveData, "command_fix_drive_data", &log_str);

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

        let log_str = format!("drive_select: {}", drive_select);
        self.log_cmd(Command::CheckDriveStatus, "command_check_drive_status", &log_str);

        Continuation::CommandComplete
    }

    /// Perform the Calibrate Drive command (0x07)
    ///
    /// Resets the drive specified drive head to cylinder 0.
    pub fn command_calibrate_drive(&mut self) -> Continuation {
        // A real floppy drive might fail to seek completely to cylinder 0 with one calibrate command.
        // Any point to emulating this behavior?
        let dhs = DriveHeadSelect::from_bytes([self.data_register_in.pop_front().unwrap()]);

        // Set drive select and seek to cylinder 0
        self.drive_select = dhs.drive() as usize;
        self.drives[dhs.drive() as usize].seek(0);

        let log_str = format!("drive_select: {}", self.drive_select);
        self.log_cmd(Command::CalibrateDrive, "command_calibrate_drive", &log_str);

        // Set up Calibrate operation
        self.send_interrupt = true;
        self.operation = Operation::Calibrate;
        self.last_command = Command::CalibrateDrive;
        Continuation::ContinueAsOperation
    }

    /// Performs a Seek for the specified drive to the specified cylinder and head.
    ///
    /// This command has no result phase. The status of the command is checked via Sense Interrupt.
    pub fn command_seek_head(&mut self) -> Continuation {
        // A real floppy drive would take some time to seek
        // Not sure how to go about determining proper timings. For now, seek instantly

        let dhs = DriveHeadSelect::from_bytes([self.data_register_in.pop_front().unwrap()]);
        let cylinder = self.data_register_in.pop_front().unwrap();
        let drive = self.select_drive(dhs.drive() as usize);

        // Is this seek out of bounds?
        if drive.is_none() || !drive.unwrap().is_seek_valid(cylinder as u16) {
            self.last_error = DriveError::BadSeek;
            self.send_interrupt = true;
            log::warn!(
                "command_seek_head(): invalid seek: drive:{} c: {} h: {}",
                dhs.drive(),
                cylinder,
                dhs.head()
            );
            return Continuation::CommandComplete;
        }

        // Seek to cylinder given in command
        self.drives[self.drive_select].seek(cylinder as u16);

        let log_str = format!(
            "drive:{} head:{} cylinder: {} new chs: {}",
            dhs.drive(),
            dhs.head(),
            cylinder,
            self.drives[self.drive_select].chsn
        );
        self.log_cmd(Command::SeekParkHead, "command_seek_head", &log_str);

        self.last_error = DriveError::NoError;
        self.send_interrupt = true;
        Continuation::CommandComplete
    }

    /// Perform the Read Data Command
    pub fn command_read_track(&mut self) -> Continuation {
        let func = "command_read_track";
        let dhs = DriveHeadSelect::from_bytes([self.data_register_in.pop_front().unwrap()]);
        let cylinder = self.data_register_in.pop_front().unwrap();
        let head = self.data_register_in.pop_front().unwrap();
        let sector = self.data_register_in.pop_front().unwrap();
        let sector_size = self.data_register_in.pop_front().unwrap();
        let track_len = self.data_register_in.pop_front().unwrap();
        let gap3_len = self.data_register_in.pop_front().unwrap();
        let data_len = self.data_register_in.pop_front().unwrap();

        let chs = DiskChs::from((cylinder as u16, head, sector));

        if head != dhs.head() {
            // Head and head_select should usually match. May differ in some copy-protection schemes.
            log::warn!("command_read_track(): non-matching head specifiers");
        }

        if self.select_drive_mut(dhs.drive() as usize).is_some() {
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
            self.operation = Operation::ReadTrack(dhs.head(), chs, sector_size, track_len, gap3_len, data_len);

            if self.dor_dma {
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

            let log_str = format!(
                "dhs:{:02X}[drive:{} head:{}] [c:{} h:{} s:{} n:{}] track_len:{} gap3_len:{} data_len:{} skip:{}",
                dhs.into_bytes()[0],
                dhs.drive(),
                dhs.head(),
                cylinder,
                head,
                sector,
                sector_size,
                track_len,
                gap3_len,
                data_len,
                self.command_skip
            );
            self.log_cmd(Command::ReadTrack, func, &log_str);
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
                "command_read_track(): invalid dhs:[drive:{} head:{}] c:{} h:{} s:{}",
                dhs.drive(),
                dhs.head(),
                cylinder,
                head,
                sector
            );
            Continuation::CommandComplete
        }
    }

    /// Perform the Read Data Command
    pub fn command_read_data(&mut self) -> Continuation {
        let func = "command_read_data";
        let dhs = DriveHeadSelect::from_bytes([self.data_register_in.pop_front().unwrap()]);
        let cylinder = self.data_register_in.pop_front().unwrap();
        let head = self.data_register_in.pop_front().unwrap();
        let sector = self.data_register_in.pop_front().unwrap();
        let sector_size = self.data_register_in.pop_front().unwrap();
        let eot = self.data_register_in.pop_front().unwrap();
        let gap3_len = self.data_register_in.pop_front().unwrap();
        let data_len = self.data_register_in.pop_front().unwrap();

        let chs = DiskChs::from((cylinder as u16, head, sector));

        if head != dhs.head() {
            // Head select and head id should usually match, but don't have to
            log::warn!("command_read_data(): non-matching head specifiers");
        }

        if self.select_drive_mut(dhs.drive() as usize).is_some() {
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
            self.operation = Operation::ReadData(dhs.head(), chs, sector_size, eot, gap3_len, data_len);

            if self.dor_dma {
                // Clear MRQ until operation completion so there is no attempt to read result values
                self.mrq = false;
                // DMA now in progress
                self.in_dma = true;
            }
            else {
                // When not in DMA mode, we can leave MRQ high and let the CPU poll for completion
                log::warn!("command_read_data(): ########## IN PIO MODE ############");
                self.mrq = true;
                self.in_dma = false;
            }

            // The IBM PC BIOS only seems to ever set a track_len of 8. How do we support 9 sector (365k) floppies?
            // Answer: DOS seems to know to request sector #9 and the BIOS doesn't complain

            let log_str = format!(
                "dhs:{:02X} [drive:{} head:{}] chs:{} n:{} eot:{} gap3_len:{} data_len:{} skip:{}",
                dhs.into_bytes()[0],
                dhs.drive(),
                dhs.head(),
                chs,
                sector_size,
                eot,
                gap3_len,
                data_len,
                self.command_skip
            );
            self.log_cmd(Command::ReadData, func, &log_str);

            //log::trace!("command_read_sector: may operate on maximum of {} sectors", max_sectors);

            // Flag to set up transfer size later
            self.operation_init = false;

            // Keep running command until DMA transfer completes
            Continuation::ContinueAsOperation
        }
        else {
            self.last_error = DriveError::BadRead;
            self.send_interrupt = true;
            log::warn!("command_read_data(): invalid drive: drive:{} chs:{}", dhs.drive(), chs);
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
        let eot = self.data_register_in.pop_front().unwrap();
        let gap3_len = self.data_register_in.pop_front().unwrap();
        let data_len = self.data_register_in.pop_front().unwrap();

        let drive_select = (drive_head_select & 0x03) as usize;
        let head_select = (drive_head_select >> 2) & 0x01;

        let chs = DiskChs::from((cylinder as u16, head, sector));

        if head != head_select {
            log::warn!("command_write_data(): non-matching head specifiers");
        }

        if self.select_drive(drive_select).is_some() {
            // Start write operation
            self.operation = Operation::WriteData(
                head_select,
                chs,
                sector_size,
                eot,
                gap3_len,
                data_len,
                self.command_deleted,
            );

            if self.dor_dma {
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

            let log_str = format!(
                "dhs:{:02X} drive:{} cyl:{} head:{} sector:{} sector_size:{} eot:{}",
                drive_head_select, drive_select, cylinder, head, sector, sector_size, eot
            );
            self.log_cmd(Command::WriteData, "command_write_data", &log_str);
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
                "command_write_data(): invalid drive: drive:{} c:{} h:{} s:{}",
                drive_select,
                cylinder,
                head,
                sector
            );
            Continuation::CommandComplete
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
        let head_select = (drive_head_select >> 2) & 0x01;

        // Start format operation
        self.operation_init = false;
        self.operation = Operation::FormatTrack(head_select, sector_size, track_len, gap3_len, fill_byte);

        if self.dor_dma {
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

        let log_str = format!(
            "dhs:{:02X} sector_size:{} track_len:{} gap3_len:{} fill_byte:{:02X}",
            drive_head_select, sector_size, track_len, gap3_len, fill_byte
        );
        self.log_cmd(Command::FormatTrack, "command_format_track", &log_str);

        // Keep running command until DMA transfer completes
        Continuation::ContinueAsOperation
    }

    /// Perform the Read Sector ID Command
    pub fn command_read_sector_id(&mut self) -> Continuation {
        let drive_head_select = self.data_register_in.pop_front().unwrap();

        let drive_select = (drive_head_select & 0x03) as usize;
        let _head_select = (drive_head_select >> 2) & 0x01;

        let chsn = self.selected_drive().chsn;

        let log_str = format!("drive_select: {} chsn: {}", drive_head_select, chsn);
        self.log_cmd(Command::ReadSectorID, "command_read_sector_id", &log_str);

        self.send_results_phase(InterruptCode::NormalTermination, drive_select, chsn.into(), chsn.n());

        self.drives[drive_select].advance_sector();

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

        self.last_status_bytes[0] = st0_byte;
        self.last_status_bytes[1] = st1_byte;
        self.last_status_bytes[2] = st2_byte;

        let log_str = format!(
            "Result Phase: ST0: {:08b}[{:02X}] ST1: {:08b}[{:02X}] ST2: {:08b}[{:02X}] c:{} h:{} s:{}",
            st0_byte,
            st0_byte,
            st1_byte,
            st1_byte,
            st2_byte,
            st2_byte,
            chs.c(),
            chs.h(),
            chs.s(),
        );
        self.log_str(&log_str);

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

    fn operation_read_data_pio(&mut self, _bus: &mut BusInterface, h: u8, chs: DiskChs, n: u8, eot: u8) {
        if !self.operation_init {
            self.xfer_size_sectors = (eot.saturating_sub(chs.s())) as usize + 1;
            self.xfer_completed_sectors = 0;
            // TODO: fixme for sector size
            self.xfer_size_bytes = self.xfer_size_sectors * 512;

            self.pio_bytes_left = self.xfer_size_bytes;
            self.pio_byte_count = 0;
            self.pio_sector_byte_count = 0;
            self.operation_init = true;

            let mt = self.mt;
            let skip_flag = self.command_skip;
            let xfer_sectors = self.xfer_size_sectors;
            match self
                .selected_drive_mut()
                .command_read_data(mt, h, chs, xfer_sectors, n, 0, 0, 0, skip_flag)
            {
                Ok(read_result) => {
                    log::trace!("Read sector command accepted, new chs: {}", read_result.new_chs);
                    self.operation_final_chs = read_result.new_chs;
                    if self.operation_final_chs.s() > eot {
                        // Don't exceed EOT parameter for returned CHS.
                        self.operation_final_chs.set_s(eot);
                    }

                    if read_result.not_found {
                        self.send_results_phase(InterruptCode::AbnormalTermination, self.drive_select, chs, n);
                        self.operation = Operation::NoOperation;
                        self.send_interrupt = true;
                        return;
                    }

                    // We can read 0 sectors if there is bad IDAM CRC.
                    if read_result.sectors_read == 0 {
                        self.pio_bytes_left = 0;
                        self.xfer_size_sectors = 0;
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

            // let (new_c, new_h, new_s) = self
            //     .selected_drive()
            //     .get_chs_sector_offset(self.xfer_size_sectors, chs)
            //     .into();
            //let (new_c, new_h, new_s) = self.get_next_sector(self.drive_select, cylinder, head, sector);

            let new_chs = self.operation_final_chs;

            // Terminate normally by sending results registers
            self.send_results_phase(InterruptCode::NormalTermination, self.drive_select, new_chs, n);

            log::trace!(
                "operation_read_sector_pio completed ({} bytes transferred): new chsn: {} drive: {}",
                self.xfer_size_bytes - self.pio_bytes_left,
                &self.drives[self.drive_select].chsn,
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
        h: u8,
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
                log::warn!(
                    "DMA word count {} not multiple of sector size ({})",
                    xfer_size,
                    sector_size_decoded
                );
            }

            let mut xfer_sectors = xfer_size / sector_size_decoded;
            if xfer_sectors > 0 {
                log::trace!(
                    "DMA programmed for transfer of {} sectors: ({} bytes)",
                    xfer_sectors,
                    xfer_size
                );
            }
            else if xfer_sectors == 0 && xfer_size > 0 {
                log::trace!("DMA programmed for transfer of partial sector: ({} bytes)", xfer_size);
                xfer_sectors = 1;
            }
            else {
                log::warn!("DMA not programmed for transfer!");
            }

            let dst_address = dma.get_dma_transfer_address(FDC_DMA);
            log::trace!("DMA destination address: {:05X}", dst_address);

            let skip_flag = self.command_skip;
            let mt = self.mt;
            match self
                .selected_drive_mut()
                .command_read_data(mt, h, chs, xfer_sectors, n, 0, 0, 0, skip_flag)
            {
                Ok(read_result) => {
                    log::trace!("Read sector command accepted, new chs: {}", read_result.new_chs);
                    self.operation_final_chs = read_result.new_chs;

                    if read_result.not_found {
                        self.send_results_phase(InterruptCode::AbnormalTermination, self.drive_select, chs, n);
                        self.operation = Operation::NoOperation;
                        self.send_interrupt = true;
                        return;
                    }

                    // We can read 0 sectors if there is bad IDAM CRC.
                    if read_result.sectors_read == 0 {
                        xfer_sectors = 0;
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
                // log::trace!(
                //     "operation_read_sector: Transferred {} sectors.",
                //     self.xfer_completed_sectors
                // );
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

            let new_chs = self.operation_final_chs;
            log::trace!(
                "Read operation completed. Transferred {} bytes, new chs: {}",
                self.dma_byte_count,
                new_chs
            );

            self.dma_byte_count = 0;
            self.dma_bytes_left = 0;

            // Terminate normally by sending results registers
            self.send_results_phase(InterruptCode::NormalTermination, self.drive_select, new_chs, n);

            // Seek to new CHS
            self.drives[self.drive_select].chsn.set_chs(new_chs);

            // Finalize operation
            self.operation = Operation::NoOperation;
            self.send_interrupt = true;
        }
    }

    fn operation_write_data(
        &mut self,
        dma: &mut dma::DMAController,
        bus: &mut BusInterface,
        h: u8,
        chs: DiskChs,
        sector_size: u8,
        _track_len: u8,
        deleted: bool,
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
            let write_result = self.drives[self.drive_select].command_write_data(
                h,
                chs,
                ct,
                sector_size,
                &self.xfer_buffer,
                false,
                deleted,
            );

            match write_result {
                Ok(write_result) => {
                    self.dma_byte_count = 0;
                    self.dma_bytes_left = 0;

                    if write_result.not_found {
                        log::warn!(
                            "operation_write_data(): Drive reported write data command failed: sector ID not found"
                        );
                        self.send_results_phase(
                            InterruptCode::AbnormalTermination,
                            self.drive_select,
                            chs,
                            sector_size,
                        );
                        self.operation = Operation::NoOperation;
                        self.send_interrupt = true;
                        return;
                    }

                    let (new_c, new_h, new_s) = self
                        .selected_drive()
                        .get_chs_sector_offset(self.xfer_completed_sectors + 1, chs)
                        .into();

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
                    self.drives[self.drive_select].chsn.set_chs(new_chs);

                    // Finalize operation
                    self.operation = Operation::NoOperation;
                    self.send_interrupt = true;
                }
                Err(e) => {
                    log::warn!("Drive reported write data command failed: {:?}", e);
                    self.send_results_phase(InterruptCode::AbnormalTermination, self.drive_select, chs, sector_size);
                    self.operation = Operation::NoOperation;
                    self.send_interrupt = true;
                }
            }
        }
    }

    /// Run the Read Track operation
    fn operation_read_track(
        &mut self,
        dma: &mut dma::DMAController,
        bus: &mut BusInterface,
        h: u8,
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
                .command_read_track(h, ch, n, eot, Some(xfer_size))
            {
                Ok(read_result) => {
                    log::trace!(
                        "operation_read_track(): Read track command accepted, new chs: {}",
                        read_result.new_chs
                    );
                    self.operation_final_chs = read_result.new_chs;

                    if read_result.not_found {
                        self.send_results_phase(
                            InterruptCode::AbnormalTermination,
                            self.drive_select,
                            read_result.new_chs,
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

            let new_chs = self.operation_final_chs;
            log::debug!("operation_read_track(): operation completed: new chs: {}", new_chs);

            // Terminate normally by sending results registers
            self.send_results_phase(InterruptCode::NormalTermination, self.drive_select, new_chs, n);

            // Seek to new CHS and finalize operation
            self.drives[self.drive_select].chsn.set_chs(new_chs);
            self.operation = Operation::NoOperation;
            self.send_interrupt = true;
        }
    }

    fn operation_reset(&mut self, delta_us: f64) {
        self.operation_accumulator += delta_us;
        if self.operation_accumulator > FDC_RESET_TIME {
            log::trace!("FDC Operation Reset complete.");
            self.operation_accumulator = 0.0;
            self.reset_internal(true);
            self.operation = Operation::NoOperation;
            self.send_interrupt = true;
        }
    }

    fn operation_seek(&mut self, delta_us: f64) {
        self.operation_accumulator += delta_us;
        if self.operation_accumulator > FDC_SEEK_TIME {
            log::trace!("FDC Operation Seek/Calibrate complete");
            self.operation_accumulator = 0.0;
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
        h: u8,
        n: u8,
        track_len: u8,
        gap3_len: u8,
        fill_byte: u8,
    ) {
        if !self.in_dma {
            log::error!("operation_format_track(): Format Track operation without DMA!");
            self.operation = Operation::NoOperation;
            return;
        }

        let sector_size_decoded = FloppyController::decode_sector_size(n);

        // Fail operation if disk is write protected
        if self.drives[self.drive_select].write_protected {
            log::warn!("operation_format_track(): operation on write protected disk!");

            // Terminate with WriteProtect error.
            self.last_error = DriveError::WriteProtect;
            self.send_results_phase(InterruptCode::AbnormalPolling, self.drive_select, Default::default(), n);

            self.send_interrupt = true;
            self.operation = Operation::NoOperation;
            return;
        }

        if !self.operation_init {
            let xfer_size = dma.get_dma_transfer_size(FDC_DMA);

            if xfer_size < (track_len as usize * FORMAT_BUFFER_SIZE) {
                log::error!(
                    "operation_format_track(): DMA word count too small for track_len({:02}) format buffers.",
                    track_len
                );
                self.operation = Operation::NoOperation;
                return;
            }

            let xfer_sectors = xfer_size / sector_size_decoded;
            self.dma_bytes_left = track_len as usize * FORMAT_BUFFER_SIZE;

            log::trace!(
                "operation_format_track(): DMA programmed for transfer of {} sectors, bytes_left: {}",
                xfer_sectors,
                self.dma_bytes_left
            );

            if self.dma_bytes_left == 0 {
                log::warn!("operation_format_track(): No format buffer bytes to transfer.");
            }
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
            else {
                log::warn!("operation_format_track(): DMA not ready for transfer.");
            }

            // Have we read in the entire format buffer?
            if self.format_buffer.len() == FORMAT_BUFFER_SIZE * track_len as usize {
                // let f_cylinder = self.format_buffer.pop_front().unwrap();
                // let f_head = self.format_buffer.pop_front().unwrap();
                // let f_sector = self.format_buffer.pop_front().unwrap();
                // let f_sector_size = self.format_buffer.pop_front().unwrap();

                // log::trace!(
                //     "Formatting track: {} head: {} size: {} with byte: {:02X}",
                //     f_cylinder,
                //     f_head,
                //     f_sector,
                //     f_sector_size,
                //     fill_byte
                // );

                // self.format_sector(f_cylinder, f_head, f_sector, fill_byte);

                let ch: DiskCh = DiskCh::new(self.drives[self.drive_select].chsn.c(), h);

                match self.drives[self.drive_select].command_format_track(
                    ch,
                    self.format_buffer.make_contiguous(),
                    gap3_len,
                    fill_byte,
                ) {
                    Ok(read_result) => {
                        log::trace!(
                            "operation_format_track(): Command successful, new sid: {}",
                            read_result.new_sid
                        );
                        self.operation_final_chs = DiskChs::from((ch, read_result.new_sid));
                    }
                    Err(e) => {
                        log::error!("operation_format_track(): Format track command failed: {:?}", e);
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
                self.send_interrupt = true;

                // Clear format buffer for next operation
                self.format_buffer.clear();
            }
        }
        else {
            // No more bytes left to transfer. Finalize operation

            //let tc = dma.check_terminal_count(FDC_DMA);
            //if !tc {
            //    log::warn!("FDC Format Track complete without DMA terminal count.");
            //}

            log::trace!("operation_format_track(): Format track operation completed.");

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
                n,
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

    pub fn log_cmd(&mut self, cmd: Command, func: &str, s: &str) {
        self.cmd_log.push(format!("{:?}: {}", cmd, s));
        log::trace!("{}(): {}", func, s);
    }

    pub fn log_str(&mut self, s: &str) {
        self.cmd_log.push(s.to_string());
        log::trace!("{}", s);
    }

    pub fn get_debug_state(&self) -> FdcDebugState {
        FdcDebugState {
            intr: self.pending_interrupt,
            dor: self.dor,
            data_mode: match self.dor & 0x08 != 0 {
                true => DataMode::Dma,
                false => DataMode::Pio,
            },
            operation: self.operation,
            last_cmd: self.last_command,
            last_status: self.last_status_bytes.clone(),
            drive_select: self.drive_select,
            status_register: self.handle_status_register_read(),
            data_register_in: self.data_register_in.clone().make_contiguous().to_vec(),
            data_register_out: self.data_register_out.clone().make_contiguous().to_vec(),
            last_data_read: self.last_data_read,
            last_data_written: self.last_data_written,
            dio: self.dio,
            st3: self.last_st3,
            cmd_log: self.cmd_log.as_vec(),
        }
    }

    pub fn get_image_state(&self) -> Vec<Option<FloppyImageState>> {
        self.drives.iter().map(|d| d.image_state()).collect()
    }

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
            Operation::Reset => {
                // Perform reset
                self.operation_reset(us);
            }
            Operation::Calibrate | Operation::Seek => {
                self.operation_seek(us);
            }
            Operation::ReadData(h, chs, sector_size, track_len, _gap3_len, _data_len) => match self.dma {
                true => self.operation_read_data(dma, bus, h, chs, sector_size, track_len),
                false => self.operation_read_data_pio(bus, h, chs, sector_size, track_len),
            },
            Operation::WriteData(h, chs, sector_size, track_len, _gap3_len, _data_len, deleted) => {
                self.operation_write_data(dma, bus, h, chs, sector_size, track_len, deleted)
            }
            Operation::ReadTrack(h, chs, sector_size, track_len, _gap3_len, _data_len) => {
                self.operation_read_track(dma, bus, h, chs.into(), sector_size, track_len)
            }
            Operation::FormatTrack(head, sector_size, track_len, gap3_len, fill_byte) => {
                self.operation_format_track(dma, bus, head, sector_size, track_len, gap3_len, fill_byte)
            }
            _ => {
                log::error!("Invalid FDC operation: {:?}", self.operation)
            }
        }
    }
}

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

use super::{
    data_interface::{FdcDataDirection, FdcDataInterface, FdcDataMode},
    debug_log::tokenize_log_entry,
    operation::{
        FdcOperation,
        FormatTrackOperation,
        ReadDataOperation,
        ReadTrackOperation,
        ResetOperation,
        SeekOperation,
        WriteDataOperation,
    },
};

use crate::{
    bus::{BusInterface, DeviceRunTimeUnit, IoDevice},
    cpu_common::LogicAnalyzer,
    device_types::fdc::{CoreFloppyImageType, FloppyImageType},
    devices::{
        dma,
        floppy_drive::{FloppyDiskDrive, FloppyImageState, OperationStatus},
    },
    machine_config::FloppyDriveConfig,
    machine_types::FdcType,
};

use marty_common::{syntax_token::SyntaxTokenStream, types::history_buffer::HistoryBuffer};

use anyhow::{anyhow, Error};
use bitflags::bitflags;
use crossbeam_channel::Sender;
use fluxfox::prelude::*;
use marty_common::{FloppyDriveEvent, PresentableDeviceEvent};
use modular_bitfield::{bitfield, prelude::*};

pub const FDC_LOG_LEN: usize = 1000;

pub const FDC_IRQ: u8 = 0x06;
pub const FDC_DMA: usize = 2;
pub const FDC_MAX_DRIVES: usize = 4;
//pub const SECTOR_SIZE: usize = 512;

pub const PCXT_IO_BASE: u16 = 0x03F0;
pub const PCJR_IO_BASE: u16 = 0x00F0;

pub const FDC_DIGITAL_OUTPUT_REGISTER: u16 = 0x02;
pub const FDC_STATUS_REGISTER: u16 = 0x04;
pub const FDC_DATA_REGISTER: u16 = 0x05;

pub const FDC_RESET_TIME: f64 = 1000.0; // 1ms in microseconds

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DataRate {
    Rate250Kbps,
}

impl DataRate {
    pub fn bits_per_second(self) -> u32 {
        match self {
            Self::Rate250Kbps => 250_000,
        }
    }

    pub fn byte_period_us(self) -> f64 {
        8_000_000.0 / self.bits_per_second() as f64
    }
}

// Main Status Register Bit Definitions
// --------------------------------------------------------------------------------
// The first four bits encode drive-busy state. A drive remains busy from the
// beginning of a seek/recalibrate until Sense Interrupt acknowledges its completion.
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

pub const WATCHDOG_TIMEOUT: f64 = 1_000_000.0; // microseconds

pub const COMMAND_MASK: u8 = 0b0001_1111;
pub const COMMAND_SKIP_BIT: u8 = 0b0010_0000;

bitflags! {
    #[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
    pub struct CommandFlags: u8 {
        const CMD_FLAG_DELETED_DATA = 0b0000_0001;
        const CMD_FLAG_CALIBRATE = 0b0000_0010;
    }
}

bitflags! {
    #[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
    struct InterruptSources: u8 {
        const SENSE = 0b0000_0001;
        const RESULT_PHASE = 0b0000_0010;
        const NON_DMA_SERVICE = 0b0000_0100;
    }
}

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
pub const ST0_READY_CHANGED: u8 = 0b1100_0000;

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
    FormatTrack = 0x0d,
    Specify = 0x03,
    CheckDriveStatus = 0x04,
    CalibrateDrive = 0x07,
    SenseIntStatus = 0x08,
    ReadSectorID = 0x0a,
    Seek = 0x0f,
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
#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum InterruptCode {
    NormalTermination = 0,
    AbnormalTermination = ST0_ABNORMAL_TERMINATION,
    InvalidCommand = ST0_INVALID_OPCODE,
    ReadyChanged = ST0_READY_CHANGED,
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

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct DataOperationParameters {
    pub drive: usize,
    pub physical_head: u8,
    pub id_chs: DiskChs,
    pub sector_size: u8,
    pub eot: u8,
    pub gap3_len: u8,
    pub data_len: u8,
    pub multi_track: bool,
    pub mfm: bool,
    pub skip: bool,
    pub deleted_data: bool,
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
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum Operation {
    #[default]
    NoOperation,
    Reset,
    Calibrate,
    Seek,
    ReadData(DataOperationParameters),
    ReadTrack(DataOperationParameters),
    WriteData(DataOperationParameters),
    FormatTrack(u8, u8, u8, u8, u8), // head_select, sector_size, track_len, gap3_len, fill_byte
}

impl Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Operation::NoOperation => write!(f, "No Operation"),
            Operation::Reset => write!(f, "Reset"),
            Operation::Calibrate => write!(f, "Calibrate"),
            Operation::Seek => write!(f, "Seek"),
            Operation::ReadData(_) => write!(f, "Read Data"),
            Operation::ReadTrack(_) => write!(f, "Read Track"),
            Operation::WriteData(_) => write!(f, "Write Data"),
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
    pub cmd_log_tokens: Vec<SyntaxTokenStream>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum AckCondition {
    ReadyPoll,
    Seek,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct InternalDriveStatus {
    pub(super) pcn: u8,
    pub(super) seeking: bool,
    pub(super) unack_condition: Option<AckCondition>,
}

impl InternalDriveStatus {
    fn has_seek_condition(&self) -> bool {
        self.seeking || matches!(self.unack_condition, Some(AckCondition::Seek))
    }

    fn has_unacknowledged_condition(&self) -> bool {
        self.unack_condition.is_some()
    }
}

pub struct FloppyController {
    phase: ControllerPhase,
    us_accumulator: f64,
    watchdog_accumulator: f64,
    fdc_type: FdcType,
    pub(super) data_rate: DataRate,
    status_byte: u8,
    mrq: bool,

    data_register: u8,
    dor: u8,
    last_dor: u8,
    dor_dma: bool,
    dor_disabled: bool,
    pub(super) step_rate: u8,
    head_unload: u8,
    head_load: u8,
    non_dma: bool,
    busy: bool,
    dio: IoMode,
    pub(super) mt: bool,
    command_mfm: bool,
    reading_command: bool,
    command: Command,
    command_fn: Option<CommandDispatchFn>,
    last_command: Command,
    receiving_command: bool,
    command_byte_n: u32,
    pub(super) command_skip: bool,
    command_flags: CommandFlags,
    pub(super) operation: Operation,
    active_operation: Option<Box<dyn FdcOperation>>,
    pub(super) operation_init: bool,
    operation_current_chs: DiskChs,
    operation_current_phys_head: u8,
    pub(super) operation_status: OperationStatus,
    pub(super) operation_status_valid: bool,
    pub(super) read_sector_buf: Vec<u8>,
    pub(super) read_sector_buf_idx: usize,
    read_terminate_after_sector: bool,
    interrupt_sources: InterruptSources,
    irq_asserted: bool,
    watchdog_enabled: bool,           // IBM PCJr only.  Watchdog timer enabled.
    watchdog_trigger_bit: bool,       // IBM PCJr only.  Watchdog timer trigger bit status.
    watchdog_triggered: bool,         // IBM PCJr only.  Watchdog timer triggered.
    watchdog_interrupt_pending: bool, // IBM PCJr only. Watchdog is requesting IRQ6.

    pub(super) last_error: DriveError,
    last_status_bytes: Vec<u8>,

    data_register_out: VecDeque<u8>,
    data_register_in: VecDeque<u8>,
    last_data_read: u8,
    last_data_written: u8,
    last_st3: u8,

    pub(super) drives: [FloppyDiskDrive; FDC_MAX_DRIVES],
    drive_ct: usize,
    pub(super) drive_select: usize,
    pub(super) drive_status: [InternalDriveStatus; FDC_MAX_DRIVES],
    presentable_controller_id: u8,
    presentable_event_sender: Option<Sender<PresentableDeviceEvent>>,

    pub(super) in_dma: bool,
    pub(super) data_interface: FdcDataInterface,
    pub(super) dma_byte_count: usize,
    pub(super) dma_bytes_left: usize,
    pub(super) pio_byte_count: usize,
    pub(super) pio_sector_byte_count: usize,
    pub(super) pio_bytes_left: usize,
    pub(super) xfer_size_bytes: usize,

    cmd_log: HistoryBuffer<String>,
    cmd_log_tokens: HistoryBuffer<SyntaxTokenStream>,
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
            FDC_STATUS_REGISTER => self.read_main_status_register(),
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
            FDC_DATA_REGISTER => self.handle_data_register_write(data),
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
            fdc_type: FdcType::IbmNec,
            data_rate: DataRate::Rate250Kbps,
            status_byte: 0,
            mrq: true,
            data_register: 0,
            dor_dma: true,
            dor: 0,
            last_dor: 0,
            dor_disabled: false,
            step_rate: 0,
            head_unload: 0,
            head_load: 0,
            non_dma: false,
            busy: false,
            dio: IoMode::FromCpu,
            mt: false,
            command_mfm: false,
            reading_command: false,
            command: Command::NoCommand,
            command_fn: None,
            last_command: Command::NoCommand,
            command_byte_n: 0,
            receiving_command: false,
            command_skip: false,
            command_flags: CommandFlags::empty(),
            operation: Operation::NoOperation,
            active_operation: None,
            operation_init: false,
            operation_current_chs: DiskChs::default(),
            operation_current_phys_head: 0,
            operation_status: OperationStatus::default(),
            operation_status_valid: false,
            read_sector_buf: Vec::new(),
            read_sector_buf_idx: 0,
            read_terminate_after_sector: false,

            last_error: DriveError::NoError,
            last_status_bytes: vec![0; 3],

            interrupt_sources: InterruptSources::empty(),
            irq_asserted: false,
            watchdog_enabled: false,
            watchdog_trigger_bit: false,
            watchdog_triggered: false,
            watchdog_interrupt_pending: false,

            data_register_out: VecDeque::new(),
            data_register_in: VecDeque::new(),
            last_data_read: 0,
            last_data_written: 0,
            last_st3: 0,

            drives: [
                FloppyDiskDrive::default(),
                FloppyDiskDrive::default(),
                FloppyDiskDrive::default(),
                FloppyDiskDrive::default(),
            ],
            drive_ct: 0,
            drive_select: 0,
            drive_status: [InternalDriveStatus::default(); FDC_MAX_DRIVES],
            presentable_controller_id: 0,
            presentable_event_sender: None,

            in_dma: false,
            data_interface: FdcDataInterface::default(),
            dma_byte_count: 0,
            dma_bytes_left: 0,
            pio_byte_count: 0,
            pio_sector_byte_count: 0,
            pio_bytes_left: 0,
            xfer_size_bytes: 0,

            cmd_log: HistoryBuffer::new(FDC_LOG_LEN),
            cmd_log_tokens: HistoryBuffer::new(FDC_LOG_LEN),
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

        if matches!(fdc_type, FdcType::IbmPCJrNec) {
            // The PCjr does not wire the 765's execution-phase interrupt.
            // Its BIOS polls MRQ, so only collision with the next byte period
            // constitutes a PIO overrun.
            fdc.data_interface.set_pio_service_timeout(None);
        }

        for (i, drive) in drives.iter().take(FDC_MAX_DRIVES).enumerate() {
            fdc.drives[i] = FloppyDiskDrive::new(i, drive.fd_type);
        }

        fdc
    }

    pub fn reset(&mut self) {
        self.reset_internal(false);
    }

    pub fn set_presentable_event_sender(&mut self, controller_id: u8, sender: Sender<PresentableDeviceEvent>) {
        self.presentable_controller_id = controller_id;
        self.presentable_event_sender = Some(sender);
    }

    /// Return whether the system-specific FDC interrupt source is connected to IRQ6.
    ///
    /// On the PC/XT controller, DOR bit 3 gates the 765's interrupt and DMA
    /// outputs. The PCjr does not connect the 765 interrupt output; instead,
    /// enabling the watchdog connects the watchdog timeout to IRQ6.
    fn interrupts_enabled(&self) -> bool {
        match self.fdc_type {
            FdcType::IbmNec => self.dor & DOR_DMA_ENABLED != 0,
            FdcType::IbmPCJrNec => self.watchdog_enabled,
        }
    }

    fn has_unacknowledged_drive_condition(&self) -> bool {
        self.drive_status
            .iter()
            .any(InternalDriveStatus::has_unacknowledged_condition)
    }

    pub(super) fn has_internal_interrupt_pending(&self) -> bool {
        !self.interrupt_sources.is_empty()
    }

    pub(super) fn raise_sense_interrupt(&mut self) {
        self.interrupt_sources.insert(InterruptSources::SENSE);
    }

    pub(super) fn raise_result_interrupt(&mut self) {
        self.interrupt_sources.insert(InterruptSources::RESULT_PHASE);
    }

    fn acknowledge_sense_interrupt(&mut self) {
        self.interrupt_sources.remove(InterruptSources::SENSE);
    }

    fn acknowledge_result_interrupt(&mut self) {
        self.interrupt_sources.remove(InterruptSources::RESULT_PHASE);
    }

    fn update_non_dma_service_interrupt(&mut self) {
        let status = self.data_interface.status();
        let service_requested =
            self.data_interface.active() && self.data_interface.mode() == FdcDataMode::Pio && status.mrq;
        self.interrupt_sources
            .set(InterruptSources::NON_DMA_SERVICE, service_requested);
    }

    /// Return whether this system's interrupt source currently requests IRQ6.
    ///
    /// The 765's internal interrupt condition is tracked on the PCjr so that
    /// Sense Interrupt Status can observe it, but it is not wired to the PIC - only watchdog
    /// expiry fires IRQ6 on PCjr.
    fn has_external_interrupt_pending(&self) -> bool {
        // The controller cannot drive IRQ6 while reset is asserted or while
        // the reset operation is still completing. Preserve the underlying
        // interrupt conditions so they can become visible after reset exits.
        if self.dor_disabled || matches!(self.operation, Operation::Reset) {
            return false;
        }

        let source_pending = match self.fdc_type {
            FdcType::IbmNec => self.has_internal_interrupt_pending(),
            FdcType::IbmPCJrNec => self.watchdog_interrupt_pending,
        };

        source_pending && self.interrupts_enabled()
    }

    /// Reflect the controller's current interrupt output onto IRQ6.
    fn update_pic_interrupt(&mut self, bus: &mut BusInterface) {
        let interrupt_pending = self.has_external_interrupt_pending();
        let Some(pic) = bus.pic_mut().as_mut()
        else {
            return;
        };

        if interrupt_pending && !self.irq_asserted {
            pic.request_interrupt(FDC_IRQ);
            self.irq_asserted = true;
        }
        else if !interrupt_pending && self.irq_asserted {
            pic.clear_interrupt(FDC_IRQ);
            self.irq_asserted = false;
        }
    }

    pub(super) fn emit_presentable_event(&self, drive: usize, event: FloppyDriveEvent) {
        let Some(sender) = &self.presentable_event_sender
        else {
            return;
        };

        let presentable_event = PresentableDeviceEvent::FloppyDrive {
            controller: self.presentable_controller_id,
            drive: drive as u8,
            event,
        };

        if let Err(err) = sender.try_send(presentable_event) {
            log::warn!("Unable to send presentable FDC event: {}", err);
        }
    }

    /// Reset the Floppy Drive Controller
    pub fn reset_internal(&mut self, internal: bool) {
        // TODO: Implement in terms of Default
        if !internal {
            // A full device reset clears the external DOR.
            match self.fdc_type {
                FdcType::IbmNec => self.handle_dor_write(0),
                FdcType::IbmPCJrNec => self.handle_dor_write_jr(0),
            }

            self.cmd_log.clear();
            self.cmd_log_tokens.clear();
        }

        self.phase = ControllerPhase::CommandPhase;
        self.status_byte = 0;
        self.drive_select = 0;
        self.drive_status.fill(InternalDriveStatus::default());

        self.data_register_out.clear();
        self.data_register_in.clear();

        // A full machine reset leaves the controller held in reset by the
        // cleared DOR. MRQ becomes ready only when ResetOperation completes
        // after the guest releases reset.
        self.mrq = internal;
        self.dio = IoMode::FromCpu;

        // A reset of the 765's internal state does not change the external DOR
        // or its motor-enable lines.
        for drive_index in 0..self.drives.len() {
            self.drives[drive_index].reset();
        }

        self.last_error = DriveError::NoError;
        self.last_status_bytes = vec![0; 3];
        self.operation_current_chs = DiskChs::default();
        self.operation_current_phys_head = 0;
        self.operation_status = OperationStatus::default();
        self.operation_status_valid = false;
        self.read_sector_buf.clear();
        self.read_sector_buf_idx = 0;
        self.read_terminate_after_sector = false;
        self.receiving_command = false;
        self.command = Command::NoCommand;
        self.last_command = Command::NoCommand;
        self.command_fn = None;
        self.command_byte_n = 0;
        self.mt = false;
        self.command_mfm = false;
        self.command_skip = false;
        self.command_flags = CommandFlags::empty();

        self.interrupt_sources = InterruptSources::empty();
        self.operation = Operation::NoOperation;
        self.operation_init = false;
        self.active_operation = None;
        self.in_dma = false;
        self.data_interface.end();
        self.dma_byte_count = 0;
        self.dma_bytes_left = 0;

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

        let was_present = self.drives[drive_select].disk_present();
        let result = self.drives[drive_select].load_image_from(src_vec, path, write_protect);
        if result.is_ok() {
            if was_present {
                self.emit_presentable_event(drive_select, FloppyDriveEvent::MediaEjected);
            }
            self.emit_presentable_event(drive_select, FloppyDriveEvent::MediaInserted);
        }
        result
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
        let was_present = self.drives[drive_select].disk_present();
        let result = self.drives[drive_select].attach_image(image, path, write_protect);
        if result.is_ok() {
            if was_present {
                self.emit_presentable_event(drive_select, FloppyDriveEvent::MediaEjected);
            }
            self.emit_presentable_event(drive_select, FloppyDriveEvent::MediaInserted);
        }
        result
    }

    pub fn get_image(&mut self, drive_select: usize) -> (Option<Arc<RwLock<DiskImage>>>, u64) {
        self.drives[drive_select].get_image()
    }

    /// Unload (eject) the disk in the specified drive
    pub fn unload_image(&mut self, drive_select: usize) {
        if drive_select >= self.drive_ct {
            log::warn!("Ignoring eject for invalid drive selection: {}", drive_select);
            return;
        }

        let was_present = self.drives[drive_select].disk_present();
        self.drives[drive_select].unload_image();
        if was_present {
            self.emit_presentable_event(drive_select, FloppyDriveEvent::MediaEjected);
        }
    }

    pub fn create_new_image(
        &mut self,
        drive_select: usize,
        format: StandardFormat,
        formatted: bool,
    ) -> Result<Arc<RwLock<DiskImage>>, Error> {
        if drive_select >= self.drive_ct {
            return Err(anyhow!("Invalid drive selection"));
        }

        let was_present = self.drives[drive_select].disk_present();
        let result = self.drives[drive_select].create_new_image(format, formatted);
        if result.is_ok() {
            if was_present {
                self.emit_presentable_event(drive_select, FloppyDriveEvent::MediaEjected);
            }
            self.emit_presentable_event(drive_select, FloppyDriveEvent::MediaInserted);
        }
        else if was_present && !self.drives[drive_select].disk_present() {
            self.emit_presentable_event(drive_select, FloppyDriveEvent::MediaEjected);
        }
        result
    }

    pub fn patch_image_bpb(&mut self, drive_select: usize, image_type: Option<FloppyImageType>) -> Result<(), Error> {
        let drive = &mut self.drives[drive_select];

        if let Some(image_type) = image_type {
            if let Ok(standard_disk_format) = CoreFloppyImageType::from(image_type).try_into() {
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

    pub fn read_main_status_register(&self) -> u8 {
        let mut msr_byte = 0;
        // Set the four 'drive status' bits. These are set to 1 when seeking and remain 1 until
        // cleared by a Sense Interrupt.
        for (i, drive_status) in self.drive_status.iter().enumerate() {
            if drive_status.has_seek_condition() {
                msr_byte |= 0x01 << i;
            }
        }

        let data_status = self.data_interface.status();
        if self.busy || data_status.busy {
            msr_byte |= FDC_STATUS_FDC_BUSY;
        }

        // The NDMA bit is sort of an PIO operation status bit. It is cleared when the drive is no
        // longer busy with the operation.
        if self.non_dma && !matches!(self.operation, Operation::NoOperation) {
            msr_byte |= FDC_STATUS_NON_DMA_MODE;
        }

        // DIO bit => 0=FDC Receiving 1=FDC Sending
        if let IoMode::ToCpu = self.dio {
            msr_byte |= FDC_STATUS_DIO;
        }
        if data_status.dio_to_cpu {
            msr_byte |= FDC_STATUS_DIO;
        }

        // MRQ => Ready to receive or send data or commands via the data register
        if self.mrq {
            msr_byte |= FDC_STATUS_MRQ;
        }
        if data_status.mrq {
            msr_byte |= FDC_STATUS_MRQ;
        }

        //log::trace!("Status Register Read: Drive select:{}, Value: {:02X}", self.drive_select, msr_byte);
        msr_byte
    }

    fn motor_on(&mut self, drive_select: usize) {
        let was_on = self.drives[drive_select].is_motor_on();
        self.drives[drive_select].motor_on();
        if !was_on && self.drives[drive_select].is_motor_on() {
            self.emit_presentable_event(
                drive_select,
                FloppyDriveEvent::MotorStarted {
                    media_present: self.drives[drive_select].disk_present(),
                },
            );
        }
    }

    fn motor_off(&mut self, drive_select: usize) {
        let was_on = self.drives[drive_select].is_motor_on();
        if was_on {
            log::trace!("Drive {}: turning motor off.", drive_select)
        }
        self.drives[drive_select].motor_off();
        if was_on {
            self.emit_presentable_event(
                drive_select,
                FloppyDriveEvent::MotorStopped {
                    media_present: self.drives[drive_select].disk_present(),
                },
            );
        }
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
            self.operation = Operation::Reset;
            self.active_operation = Some(Box::new(ResetOperation::new()));
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
        if self.drives[disk_n as usize].is_motor_on() {
            log::trace!("Drive {} selected, motor on", disk_n);
        }
        else {
            log::trace!("Drive {} selected, motor off", disk_n);
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
            self.operation = Operation::Reset;
            self.active_operation = Some(Box::new(ResetOperation::new()));
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

        if (data & DOR_DMA_ENABLED != 0) && !self.dor_dma {
            log::error!("PCJr FDC DMA was erroneously enabled");
            self.dor_dma = true;
        }
        else {
            self.dor_dma = false;
        }

        if (data & DOR_JRFDC_WATCHDOG_ENABLE != 0) && !self.watchdog_enabled {
            log::debug!("PCJr FDC watchdog enabled");
            self.watchdog_enabled = true;
        }
        else if (data & DOR_JRFDC_WATCHDOG_ENABLE == 0) && self.watchdog_enabled {
            log::debug!("PCJr FDC watchdog disabled");
            self.watchdog_enabled = false;
            self.watchdog_triggered = false;
            self.watchdog_accumulator = 0.0;
            self.watchdog_interrupt_pending = false;
        }

        // Watchdog trigger is set on falling edge of trigger bit.
        if data & DOR_JRFDC_WATCHDOG_TRIGGER != 0 {
            self.watchdog_trigger_bit = true;
        }
        else {
            if self.watchdog_trigger_bit {
                log::debug!("PCJr FDC watchdog trigger set");
                self.watchdog_triggered = true;
                self.watchdog_accumulator = 0.0;
            }
            self.watchdog_trigger_bit = false;
        }

        self.last_dor = self.dor;
        self.dor = data;
    }

    fn result_operation_status(&self, drive_select: usize) -> OperationStatus {
        if self.operation_status_valid {
            self.operation_status
        }
        else {
            self.drives[drive_select].get_operation_status()
        }
    }

    /// Create the ST0 status register bitfield with the given parameters.
    ///
    /// Note: returning an Interrupt Code of Abnormal Termination will result in a "General failure reading drive"
    ///
    pub fn make_st0_byte(&self, interrupt_code: InterruptCode, drive_select: usize, seek_end: bool) -> u8 {
        let mut st0: u8 = 0;

        st0 |= interrupt_code as u8;

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
        let status = self.result_operation_status(drive_select);
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
        let status = self.result_operation_status(drive_select);

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

        if let Some(byte) = self.data_interface.cpu_read() {
            self.interrupt_sources.remove(InterruptSources::NON_DMA_SERVICE);
            self.last_data_read = byte;
            return byte;
        }

        if !self.data_register_out.is_empty() {
            // The 8272A drops a result-phase interrupt when the CPU reads the
            // first result byte. Other interrupt causes remain latched.
            self.acknowledge_result_interrupt();
            out_byte = self.data_register_out.pop_front().unwrap();
            if self.data_register_out.is_empty() && self.pio_bytes_left == 0 {
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

    pub fn set_command(&mut self, command: Command, n_bytes: u32, command_fn: CommandDispatchFn, flags: CommandFlags) {
        // Since we are entering a new command, clear the previous error status
        self.last_error = DriveError::NoError;
        self.receiving_command = true;
        self.command = command;
        self.command_fn = Some(command_fn);
        self.command_byte_n = n_bytes;
        self.command_flags = flags;
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

        if self.data_interface.active()
            && self.data_interface.mode() == FdcDataMode::Pio
            && self.data_interface.direction() == FdcDataDirection::FromHost
        {
            if let Err(err) = self.data_interface.cpu_write(data) {
                log::warn!("FDC PIO data interface write failed: {:?}", err);
            }
            else {
                self.interrupt_sources.remove(InterruptSources::NON_DMA_SERVICE);
            }
            return;
        }

        if !self.receiving_command {
            let command_byte = CommandByte::from_bytes([data]);
            let command = command_byte.command();
            self.mt = command_byte.mt();
            self.command_mfm = command_byte.mfm();
            self.command_skip = command_byte.skip();
            self.command_flags = CommandFlags::empty();
            match command {
                COMMAND_READ_TRACK => {
                    log::trace!("Received Read Track command: {:02}", command);
                    self.set_command(
                        Command::ReadTrack,
                        8,
                        FloppyController::command_read_track,
                        CommandFlags::empty(),
                    );
                }
                COMMAND_WRITE_DATA => {
                    log::trace!("Received Write Data command: {:02}", command);
                    self.set_command(
                        Command::WriteData,
                        8,
                        FloppyController::command_write_data,
                        CommandFlags::empty(),
                    );
                }
                COMMAND_READ_DATA => {
                    log::trace!("Received Read Data command: {:02X} {:02}", data, command);
                    self.set_command(
                        Command::ReadData,
                        8,
                        FloppyController::command_read_data,
                        CommandFlags::empty(),
                    );
                }
                COMMAND_WRITE_DELETED_DATA => {
                    log::warn!("Received Write Deleted Data command: {:02}", command);
                    self.set_command(
                        Command::WriteData,
                        8,
                        FloppyController::command_write_data,
                        CommandFlags::CMD_FLAG_DELETED_DATA,
                    );
                }
                COMMAND_READ_DELETED_DATA => {
                    log::trace!("Received Read Deleted Data command: {:02}", command);
                    self.set_command(
                        Command::ReadData,
                        8,
                        FloppyController::command_read_data,
                        CommandFlags::CMD_FLAG_DELETED_DATA,
                    );
                }
                COMMAND_FORMAT_TRACK => {
                    log::trace!("Received Format Track command: {:02}", command);
                    self.set_command(
                        Command::FormatTrack,
                        5,
                        FloppyController::command_format_track,
                        CommandFlags::empty(),
                    );
                }
                COMMAND_FIX_DRIVE_DATA => {
                    log::trace!("Received Specify command: {:02}", command);
                    self.set_command(
                        Command::Specify,
                        2,
                        FloppyController::command_specify,
                        CommandFlags::empty(),
                    );
                }
                COMMAND_CHECK_DRIVE_STATUS => {
                    log::trace!("Received Check Drive Status command: {:02}", command);
                    self.set_command(
                        Command::CheckDriveStatus,
                        1,
                        FloppyController::command_check_drive_status,
                        CommandFlags::empty(),
                    );
                }
                COMMAND_CALIBRATE_DRIVE => {
                    log::trace!("Received Calibrate Drive command: {:02}", command);
                    self.set_command(
                        Command::CalibrateDrive,
                        1,
                        FloppyController::command_seek,
                        CommandFlags::CMD_FLAG_CALIBRATE,
                    );
                }
                COMMAND_SENSE_INT_STATUS => {
                    log::trace!("Received Sense Interrupt Status command: {:02}", command);
                    // Sense Interrupt command has no input bytes, so execute directly
                    self.command_sense_interrupt();
                }
                COMMAND_READ_SECTOR_ID => {
                    log::trace!("Received Read Sector ID command: {:02}", command);
                    self.set_command(
                        Command::ReadSectorID,
                        1,
                        FloppyController::command_read_sector_id,
                        CommandFlags::empty(),
                    );
                }
                COMMAND_SEEK_HEAD => {
                    log::trace!("Received Seek/Park Head command: {:02}", command);
                    self.set_command(Command::Seek, 2, FloppyController::command_seek, CommandFlags::empty());
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
        /* The 5160 BIOS performs four sense interrupts after a reset of the fdc, presumably one for
           each of the possible drives. The BIOS expects to see drive select bits 00 to 11 in the
           resulting st0 bytes, even if no such drives are present.

           In theory the FDC issues interrupts when drive status changes between READY and NOT READY
           states, but IBM ties the READY line high on the 765.

           My theory is that during reset not ready transition is effectively internally generated.
           So resetting causes four drives to transition from NOT READY to READY, thus four
           interrupts. It doesn't matter that there aren't actually four drives, since the 765's
           Unit Select pins aren't connected either. It just keeps seeing the same +5v input on the
           ready pin.

           We model a single unacknowledged condition per drive to account for situations where a
           Sense Interrupt can be issued to clear polling and seek-end conditions that would
           otherwise cause Sense Interrupt to return Invalid Opcode.

           Sense Interrupt returns the Invalid Opcode interrupt code if an interrupt was not in
           progress or one of the conditions described above is not pending.
        */

        let st0_byte;
        let mut send_cylinder = true;

        let ack_bitfield = |condition| {
            self.drive_status.iter().enumerate().fold(0u8, |bits, (drive, status)| {
                bits | (u8::from(status.unack_condition == Some(condition)) << drive)
            })
        };
        let log_str = format!(
            "Last command: {:?}, Last error: {:?}, pending interrupt: {}, poll: {:04b}, seek: {:04b}",
            self.last_command,
            self.last_error,
            self.interrupt_sources.contains(InterruptSources::SENSE),
            ack_bitfield(AckCondition::ReadyPoll),
            ack_bitfield(AckCondition::Seek),
        );
        self.log_cmd(Command::SenseIntStatus, "command_sense_interrupt", &log_str);

        if let Some(drive) = self
            .drive_status
            .iter()
            .position(|status| matches!(status.unack_condition, Some(AckCondition::ReadyPoll)))
        {
            st0_byte = ST0_READY_CHANGED | drive as u8;
            self.drive_status[drive].unack_condition = None;
        }
        else if let Some(drive) = self
            .drive_status
            .iter()
            .position(|status| matches!(status.unack_condition, Some(AckCondition::Seek)))
        {
            let code = match self.last_error {
                DriveError::BadRead | DriveError::BadWrite | DriveError::BadSeek => InterruptCode::AbnormalTermination,
                _ => InterruptCode::NormalTermination,
            };
            st0_byte = self.make_st0_byte(code, drive, true);
            self.drive_status[drive].unack_condition = None;
        }
        else if self.interrupt_sources.contains(InterruptSources::SENSE) {
            log::trace!(
                "command_sense_interrupt(): Last command: {:?}, Last error: {:?}, pending interrupt: {}",
                self.last_command,
                self.last_error,
                self.interrupt_sources.contains(InterruptSources::SENSE)
            );

            let code = match self.last_error {
                DriveError::BadRead | DriveError::BadWrite | DriveError::BadSeek => InterruptCode::AbnormalTermination,
                _ => InterruptCode::NormalTermination,
            };
            let seek_flag = matches!(self.last_command, Command::CalibrateDrive | Command::Seek);
            st0_byte = self.make_st0_byte(code, self.drive_select, seek_flag);
        }
        else {
            if !matches!(self.fdc_type, FdcType::IbmPCJrNec) {
                log::warn!("Unexpected Sense Interrupt without an interrupt condition");
            }
            st0_byte = ST0_INVALID_OPCODE;
            send_cylinder = false;
        }

        if send_cylinder {
            // Sense Interrupt acknowledges the interrupt latch once. Any
            // remaining per-drive conditions remain available to subsequent
            // Sense Interrupt commands without keeping the interrupt high.
            self.acknowledge_sense_interrupt();
        }

        // Send ST0 register to FIFO
        self.data_register_out.push_back(st0_byte);

        if send_cylinder {
            // ST0 identifies which drive's Present Cylinder Number is being reported.
            let pcn_drive = (st0_byte & 0x03) as usize;
            self.data_register_out.push_back(self.drive_status[pcn_drive].pcn);
        }

        self.last_command = Command::SenseIntStatus;
        // We have data for CPU to read
        self.last_status_bytes[0] = st0_byte;
        self.command = Command::NoCommand;
        self.send_data_register();

        log::trace!(
            "command_sense_interrupt() completed. Pushed {} bytes to data register.",
            self.data_register_out.len()
        );
    }

    /// Perform the Specify command.
    /// The timing values are retained even though physical drive timings are not currently modeled.
    pub fn command_specify(&mut self) -> Continuation {
        let steprate_unload = StepRateHeadUnload::from_bytes([self.data_register_in.pop_front().unwrap()]);
        let headload_ndm = HeadLoadDma::from_bytes([self.data_register_in.pop_front().unwrap()]);

        self.step_rate = steprate_unload.step_rate();
        self.head_unload = steprate_unload.head_unload();
        self.head_load = headload_ndm.head_load();
        self.non_dma = headload_ndm.non_dma();

        let log_str = format!(
            "step rate: {:04b} unload_time: {:04b}, head_load: {:07b} pio_mode: {}",
            self.step_rate, self.head_unload, self.head_load, self.non_dma,
        );

        self.log_cmd(Command::Specify, "command_specify", &log_str);

        Continuation::CommandComplete
    }

    /// Return the transfer mode selected internally by the 765 through
    /// SPECIFY's ND bit. The adapter's DOR DMA-enable bit gates the external
    /// DMA/interrupt connection, but does not change the 765's transfer mode.
    fn dma_mode_enabled(&self) -> bool {
        !self.non_dma
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

    /// Performs a Seek or Calibrate for the specified drive.
    ///
    /// Calibrate uses the same iterative seek operation with cylinder 0 as its target.
    /// These commands have no result phase. Their status is checked via Sense Interrupt.
    pub fn command_seek(&mut self) -> Continuation {
        let dhs = DriveHeadSelect::from_bytes([self.data_register_in.pop_front().unwrap()]);
        let calibrate = self.command_flags.contains(CommandFlags::CMD_FLAG_CALIBRATE);
        let cylinder = if calibrate {
            0
        }
        else {
            self.data_register_in.pop_front().unwrap()
        };

        // Issuing either Seek or Calibrate discards any previously
        // unacknowledged per-drive conditions.
        for drive_status in &mut self.drive_status {
            drive_status.unack_condition = None;
        }

        let drive = self.select_drive(dhs.drive() as usize);

        // Is this seek out of bounds?
        if drive.is_none() || !drive.unwrap().is_seek_valid(cylinder as u16) {
            self.drive_select = dhs.drive() as usize;
            self.last_error = DriveError::BadSeek;
            self.raise_sense_interrupt();
            log::warn!(
                "command_seek(): invalid seek: drive:{} c: {} h: {}",
                dhs.drive(),
                cylinder,
                dhs.head()
            );
            return Continuation::CommandComplete;
        }

        let command = if calibrate {
            Command::CalibrateDrive
        }
        else {
            Command::Seek
        };
        let log_str = format!("drive:{} head:{} target cylinder:{}", dhs.drive(), dhs.head(), cylinder);
        self.log_cmd(command, "command_seek", &log_str);

        self.last_error = DriveError::NoError;
        self.drive_status[self.drive_select].seeking = true;
        let operation = if calibrate {
            Operation::Calibrate
        }
        else {
            Operation::Seek
        };
        self.operation = operation;
        self.active_operation = Some(Box::new(SeekOperation::new(
            operation,
            self.drive_select,
            cylinder as u16,
        )));
        self.last_command = command;
        Continuation::ContinueAsOperation
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

            let params = DataOperationParameters {
                drive: dhs.drive() as usize,
                physical_head: dhs.head(),
                id_chs: chs,
                sector_size,
                eot: track_len,
                gap3_len,
                data_len,
                multi_track: self.mt,
                mfm: self.command_mfm,
                skip: self.command_skip,
                deleted_data: false,
            };

            // Start read operation
            self.operation = Operation::ReadTrack(params);
            self.active_operation = Some(Box::new(ReadTrackOperation::new(params)));

            if self.dma_mode_enabled() {
                // Clear MRQ until operation completion so there is no attempt to read result values
                self.mrq = false;

                // DMA now in progress
                self.in_dma = true;
            }
            else {
                // In PIO mode the data interface asserts MRQ when a byte is latched.
                self.mrq = false;
                self.in_dma = false;
            }

            // The IBM PC BIOS only seems to ever set a track_len of 8. How do we support 9 sector (365k) floppies?
            // Answer: DOS seems to know to request sector #9 and the BIOS doesn't complain

            let log_str = format!(
                "mt:{} mf:{} sk:{} dhs:{:02X} [drive:{} head:{}] [c:{} h:{} s:{} n:{}] track_len:{} gap3_len:{} data_len:{}",
                self.mt as u8,
                self.command_mfm as u8,
                self.command_skip as u8,
                dhs.into_bytes()[0],
                dhs.drive(),
                dhs.head(),
                cylinder,
                head,
                sector,
                sector_size,
                track_len,
                gap3_len,
                data_len
            );
            self.log_cmd(Command::ReadTrack, func, &log_str);
            //log::trace!("command_read_sector: may operate on maximum of {} sectors", max_sectors);

            // Flag to set up transfer size on the first operation tick.
            self.operation_init = false;

            // Keep running command until the data transfer completes.
            Continuation::ContinueAsOperation
        }
        else {
            self.last_error = DriveError::BadRead;
            log::warn!(
                "command_read_track(): invalid dhs:[drive:{} head:{}] c:{} h:{} s:{}",
                dhs.drive(),
                dhs.head(),
                cylinder,
                head,
                sector
            );
            self.send_results_phase(
                InterruptCode::AbnormalTermination,
                dhs.drive() as usize,
                chs,
                sector_size,
                true,
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

            let params = DataOperationParameters {
                drive: dhs.drive() as usize,
                physical_head: dhs.head(),
                id_chs: chs,
                sector_size,
                eot,
                gap3_len,
                data_len,
                multi_track: self.mt,
                mfm: self.command_mfm,
                skip: self.command_skip,
                deleted_data: self.command_flags.contains(CommandFlags::CMD_FLAG_DELETED_DATA),
            };

            // Start read operation
            self.operation = Operation::ReadData(params);
            self.active_operation = Some(Box::new(ReadDataOperation::new(params)));

            if self.dma_mode_enabled() {
                // Clear MRQ until operation completion so there is no attempt to read result values
                self.mrq = false;
                // DMA now in progress
                self.in_dma = true;
            }
            else {
                // In PIO mode the data interface asserts MRQ only when a byte is latched.
                log::warn!("command_read_data(): ########## IN PIO MODE ############");
                self.mrq = false;
                self.in_dma = false;
            }

            // The IBM PC BIOS only seems to ever set a track_len of 8. How do we support 9 sector (365k) floppies?
            // Answer: DOS seems to know to request sector #9 and the BIOS doesn't complain

            let log_str = format!(
                "mt:{} mf:{} sk:{} dhs:{:02X} [drive:{} head:{}] chs:{} n:{} eot:{} gap3_len:{} data_len:{}",
                self.mt as u8,
                self.command_mfm as u8,
                self.command_skip as u8,
                dhs.into_bytes()[0],
                dhs.drive(),
                dhs.head(),
                chs,
                sector_size,
                eot,
                gap3_len,
                data_len
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
            log::warn!("command_read_data(): invalid drive: drive:{} chs:{}", dhs.drive(), chs);
            self.send_results_phase(
                InterruptCode::AbnormalTermination,
                dhs.drive() as usize,
                chs,
                sector_size,
                true,
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
            let params = DataOperationParameters {
                drive: drive_select,
                physical_head: head_select,
                id_chs: chs,
                sector_size,
                eot,
                gap3_len,
                data_len,
                multi_track: self.mt,
                mfm: self.command_mfm,
                skip: self.command_skip,
                deleted_data: self.command_flags.contains(CommandFlags::CMD_FLAG_DELETED_DATA),
            };

            // Start write operation
            self.operation = Operation::WriteData(params);
            self.active_operation = Some(Box::new(WriteDataOperation::new(params)));

            if self.dma_mode_enabled() {
                // Clear MRQ until operation completion so there is no attempt to read result values
                self.mrq = false;

                // DMA now in progress
                self.in_dma = true;
            }
            else {
                // PIO write: hold MRQ low until WriteDataOperation's first tick has
                // initialized xfer_size_bytes. Otherwise the CPU could push a data byte that
                // would be misinterpreted as a new FDC command.
                self.mrq = false;
                self.in_dma = false;
            }

            let log_str = format!(
                "mt:{} mf:{} sk:{} dhs:{:02X} drive:{} cyl:{} head:{} sector:{} sector_size:{} eot:{}",
                self.mt as u8,
                self.command_mfm as u8,
                self.command_skip as u8,
                drive_head_select,
                drive_select,
                cylinder,
                head,
                sector,
                sector_size,
                eot
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
            log::warn!(
                "command_write_data(): invalid drive: drive:{} c:{} h:{} s:{}",
                drive_select,
                cylinder,
                head,
                sector
            );
            self.send_results_phase(InterruptCode::AbnormalTermination, drive_select, chs, sector_size, true);
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

        let drive_select = (drive_head_select & 0x03) as usize;
        let head_select = (drive_head_select >> 2) & 0x01;

        if self.select_drive(drive_select).is_none() {
            self.last_error = DriveError::BadWrite;
            log::warn!("command_format_track(): invalid drive: {}", drive_select);
            self.send_results_phase(
                InterruptCode::AbnormalTermination,
                drive_select,
                DiskChs::default(),
                sector_size,
                true,
            );
            return Continuation::CommandComplete;
        }

        // Start format operation
        self.operation_init = false;
        self.operation = Operation::FormatTrack(head_select, sector_size, track_len, gap3_len, fill_byte);
        self.active_operation = Some(Box::new(FormatTrackOperation::new(
            head_select,
            sector_size,
            track_len,
            gap3_len,
            fill_byte,
        )));

        if self.dma_mode_enabled() {
            // Clear MRQ until operation completion so there is no attempt to read result values
            self.mrq = false;

            // DMA now in progress
            self.in_dma = true;
        }
        else {
            // The data interface will assert MRQ when it is ready for the first CHRN byte.
            self.mrq = false;
            self.in_dma = false;
        }

        let log_str = format!(
            "mf: {} dhs:{:02X} sector_size:{} track_len:{} gap3_len:{} fill_byte:{:02X}",
            self.command_mfm as u8, drive_head_select, sector_size, track_len, gap3_len, fill_byte
        );
        self.log_cmd(Command::FormatTrack, "command_format_track", &log_str);

        // Keep running until all CHRN descriptors have been transferred.
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

        self.send_results_phase(
            InterruptCode::NormalTermination,
            drive_select,
            chsn.into(),
            chsn.n(),
            true,
        );

        self.drives[drive_select].advance_sector();

        Continuation::CommandComplete
    }

    pub(super) fn send_results_phase(
        &mut self,
        result: InterruptCode,
        drive_select: usize,
        chs: DiskChs,
        sector_size: u8,
        raise_interrupt: bool,
    ) {
        /*
        let (ir_result, wp_flag) = match result {
            ControllerResult::Success(code) => (code, 0),
            ControllerResult::GeneralFailure(code) => (code, 0),
            ControllerResult::WriteProtectFailure => (InterruptCode::AbnormalTermination, 1),
        };*/

        // Create the 3 status bytes. Most of these are error flags of some sort
        let mut st0_byte = self.make_st0_byte(result, drive_select, false);
        if chs.h() == 0 {
            st0_byte &= !ST0_HEAD_ACTIVE;
        }
        else {
            st0_byte |= ST0_HEAD_ACTIVE;
        }
        let st1_byte = self.make_st1_byte(drive_select);
        let st2_byte = self.make_st2_byte(drive_select);

        self.last_status_bytes[0] = st0_byte;
        self.last_status_bytes[1] = st1_byte;
        self.last_status_bytes[2] = st2_byte;

        let log_str = format!(
            "Result Phase: Result: {:?} ST0: {:08b}[{:02X}] ST1: {:08b}[{:02X}] ST2: {:08b}[{:02X}] c:{} h:{} s:{}",
            result,
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
        if raise_interrupt {
            self.raise_result_interrupt();
        }

        // Clear error state
        self.last_error = DriveError::NoError;
    }

    pub(super) fn merge_operation_status(&mut self, status: OperationStatus) {
        self.operation_status.sector_not_found |= status.sector_not_found;
        self.operation_status.address_crc_error |= status.address_crc_error;
        self.operation_status.data_crc_error |= status.data_crc_error;
        self.operation_status.deleted_mark |= status.deleted_mark;
        self.operation_status.no_dam |= status.no_dam;
        self.operation_status.wrong_cylinder |= status.wrong_cylinder;
        self.operation_status.wrong_head |= status.wrong_head;
    }

    pub fn log_cmd(&mut self, cmd: Command, func: &str, s: &str) {
        let msg = format!("{:?}: {}", cmd, s);
        self.cmd_log_tokens.push(tokenize_log_entry(&msg));
        self.cmd_log.push(msg);
        log::trace!("{}(): {}", func, s);
    }

    pub fn log_str(&mut self, s: &str) {
        self.cmd_log_tokens.push(tokenize_log_entry(s));
        self.cmd_log.push(s.to_string());
        log::trace!("{}", s);
    }

    pub fn get_debug_state(&self) -> FdcDebugState {
        FdcDebugState {
            intr: self.has_external_interrupt_pending(),
            dor: self.dor,
            data_mode: match self.dor & 0x08 != 0 {
                true => DataMode::Dma,
                false => DataMode::Pio,
            },
            operation: self
                .active_operation
                .as_ref()
                .map(|operation| operation.operation_type())
                .unwrap_or(self.operation),
            last_cmd: self.last_command,
            last_status: self.last_status_bytes.clone(),
            drive_select: self.drive_select,
            status_register: self.read_main_status_register(),
            data_register_in: self.data_register_in.clone().make_contiguous().to_vec(),
            data_register_out: self.data_register_out.clone().make_contiguous().to_vec(),
            last_data_read: self.last_data_read,
            last_data_written: self.last_data_written,
            dio: self.dio,
            st3: self.last_st3,
            cmd_log: self.cmd_log.as_vec(),
            cmd_log_tokens: self.cmd_log_tokens.as_vec(),
        }
    }

    pub fn get_image_state(&self) -> Vec<Option<FloppyImageState>> {
        self.drives.iter().map(|d| d.image_state()).collect()
    }

    /// Run the Floppy Drive Controller. Process running Operations.
    pub fn run(&mut self, dma: &mut dma::DMAController, bus: &mut BusInterface, us: f64) {
        self.us_accumulator += us;
        self.data_interface.run(us, dma, bus);
        if self.in_dma && self.data_interface.active() {
            self.dma_byte_count = self.data_interface.byte_count();
            self.dma_bytes_left = self.xfer_size_bytes.saturating_sub(self.dma_byte_count);
        }

        if self.watchdog_triggered {
            self.watchdog_accumulator += us;
            if self.watchdog_enabled && self.watchdog_accumulator > WATCHDOG_TIMEOUT {
                log::warn!("FDC watchdog timeout!");
                self.watchdog_triggered = false;
                self.watchdog_accumulator = 0.0;
                self.operation = Operation::NoOperation;
                self.watchdog_interrupt_pending = true;
            }
        }

        if let Some(mut operation) = self.active_operation.take() {
            if !matches!(self.operation, Operation::NoOperation) {
                operation.run(self, dma, bus, us);
            }

            if !operation.is_complete() && !matches!(self.operation, Operation::NoOperation) {
                self.active_operation = Some(operation);
            }
        }
        else {
            // Run operation
            #[allow(unreachable_patterns)]
            match self.operation {
                Operation::NoOperation => {
                    // Do nothing
                }
                Operation::Reset => unreachable!("Reset operation missing active operation"),
                Operation::Calibrate | Operation::Seek => unreachable!("Seek operation missing active operation"),
                Operation::WriteData(..) => unreachable!("Write Data operation missing active operation"),
                Operation::ReadTrack(..) => unreachable!("Read Track operation missing active operation"),
                Operation::FormatTrack(..) => unreachable!("Format Track operation missing active operation"),
                _ => {}
            }
        }

        // Non-DMA byte service requests follow the data interface's MRQ level.
        self.update_non_dma_service_interrupt();

        // Reflect the system-specific interrupt source onto the physical IRQ6
        // line once per controller tick, after all state transitions.
        self.update_pic_interrupt(bus);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{devices::pic::Pic, machine_types::FloppyDriveType};

    #[test]
    fn rate_250_kbps_has_32_us_byte_period() {
        let rate = DataRate::Rate250Kbps;

        assert_eq!(rate.bits_per_second(), 250_000);
        assert_eq!(rate.byte_period_us(), 32.0);
    }

    #[test]
    fn specify_stores_all_command_fields() {
        let mut fdc = FloppyController::default();
        fdc.data_register_in.extend([0xA5, 0x6B]);

        assert!(matches!(fdc.command_specify(), Continuation::CommandComplete));
        assert_eq!(fdc.step_rate, 0x0A);
        assert_eq!(fdc.head_unload, 0x05);
        assert_eq!(fdc.head_load, 0x35);
        assert!(fdc.non_dma);

        fdc.operation = Operation::Seek;
        assert_ne!(fdc.read_main_status_register() & FDC_STATUS_NON_DMA_MODE, 0);
    }

    #[test]
    fn command_transfer_mode_uses_specify_nd_independently_of_dor() {
        let mut dma_mode_fdc = FloppyController::default();
        dma_mode_fdc.drive_ct = 1;
        dma_mode_fdc.drives[0] = FloppyDiskDrive::new(0, FloppyDriveType::Floppy360K);
        dma_mode_fdc.non_dma = false;
        dma_mode_fdc.dor_dma = false;
        dma_mode_fdc.data_register_in.extend([0, 2, 9, 0x2A, 0xF6]);

        assert!(matches!(
            dma_mode_fdc.command_format_track(),
            Continuation::ContinueAsOperation
        ));
        assert!(dma_mode_fdc.in_dma);

        let mut non_dma_mode_fdc = FloppyController::default();
        non_dma_mode_fdc.drive_ct = 1;
        non_dma_mode_fdc.drives[0] = FloppyDiskDrive::new(0, FloppyDriveType::Floppy360K);
        non_dma_mode_fdc.non_dma = true;
        non_dma_mode_fdc.dor_dma = true;
        non_dma_mode_fdc.data_register_in.extend([0, 2, 9, 0x2A, 0xF6]);

        assert!(matches!(
            non_dma_mode_fdc.command_format_track(),
            Continuation::ContinueAsOperation
        ));
        assert!(!non_dma_mode_fdc.in_dma);
    }

    #[test]
    fn dor_reset_schedules_reset_operation() {
        let mut fdc = FloppyController::default();

        fdc.handle_dor_write(0);
        fdc.handle_dor_write(DOR_FDC_RESET);

        assert!(matches!(fdc.operation, Operation::Reset));
        assert!(fdc.active_operation.is_some());
    }

    #[test]
    fn seek_command_begins_operation_and_defers_position_update() {
        let mut fdc = FloppyController::default();
        fdc.drive_ct = FDC_MAX_DRIVES;
        fdc.drives[2] = FloppyDiskDrive::new(2, FloppyDriveType::Floppy360K);
        fdc.command_flags = CommandFlags::empty();
        for (drive, status) in fdc.drive_status.iter_mut().enumerate() {
            status.unack_condition = Some(if drive % 2 == 0 {
                AckCondition::ReadyPoll
            }
            else {
                AckCondition::Seek
            });
        }

        fdc.data_register_in.extend([2, 37]);
        assert!(matches!(fdc.command_seek(), Continuation::ContinueAsOperation));
        assert!(matches!(fdc.operation, Operation::Seek));
        assert!(matches!(
            fdc.active_operation
                .as_ref()
                .map(|operation| operation.operation_type()),
            Some(Operation::Seek)
        ));
        assert_eq!(fdc.drive_status.map(|status| status.pcn), [0; FDC_MAX_DRIVES]);
        assert_eq!(fdc.drives[2].chsn.c(), 0);
        assert!(fdc.drive_status[2].seeking);
        assert!(fdc.drive_status.iter().all(|status| status.unack_condition.is_none()));
        assert_ne!(fdc.read_main_status_register() & FDC_STATUS_FDD_C_BUSY, 0);
        assert!(!fdc.has_internal_interrupt_pending());
    }

    #[test]
    fn calibrate_uses_iterative_seek_to_cylinder_zero() {
        let mut fdc = FloppyController::default();
        fdc.drive_ct = FDC_MAX_DRIVES;
        fdc.drives[2] = FloppyDiskDrive::new(2, FloppyDriveType::Floppy360K);
        fdc.drives[2].seek(37);
        fdc.drive_status[2].pcn = 37;
        fdc.command_flags = CommandFlags::CMD_FLAG_CALIBRATE;
        for status in &mut fdc.drive_status {
            status.unack_condition = Some(AckCondition::ReadyPoll);
        }

        fdc.data_register_in.push_back(2);
        assert!(matches!(fdc.command_seek(), Continuation::ContinueAsOperation));
        assert!(matches!(fdc.operation, Operation::Calibrate));
        assert!(matches!(
            fdc.active_operation
                .as_ref()
                .map(|operation| operation.operation_type()),
            Some(Operation::Calibrate)
        ));
        assert_eq!(fdc.drive_status[2].pcn, 37);
        assert!(fdc.drive_status[2].seeking);
        assert!(fdc.drive_status.iter().all(|status| status.unack_condition.is_none()));
        assert!(!fdc.has_internal_interrupt_pending());

        let mut dma = dma::DMAController::new();
        let mut bus = BusInterface::default();
        fdc.run(&mut dma, &mut bus, 37_000.0);

        assert_eq!(fdc.drive_status.map(|status| status.pcn), [0; FDC_MAX_DRIVES]);
        assert_eq!(fdc.drives[2].chsn.c(), 0);
        assert!(!fdc.drive_status[2].seeking);
        assert_eq!(fdc.drive_status[2].unack_condition, Some(AckCondition::Seek));
        assert_ne!(fdc.read_main_status_register() & FDC_STATUS_FDD_C_BUSY, 0);
        assert!(fdc.interrupt_sources.contains(InterruptSources::SENSE));
    }

    #[test]
    fn deleted_data_commands_share_handlers_via_command_flags() {
        let mut read_fdc = FloppyController::default();
        read_fdc.handle_data_register_write(COMMAND_READ_DELETED_DATA);

        assert!(matches!(read_fdc.command, Command::ReadData));
        assert_eq!(read_fdc.command_byte_n, 8);
        assert!(read_fdc.command_flags.contains(CommandFlags::CMD_FLAG_DELETED_DATA));

        let mut write_fdc = FloppyController::default();
        write_fdc.handle_data_register_write(COMMAND_WRITE_DELETED_DATA);

        assert!(matches!(write_fdc.command, Command::WriteData));
        assert_eq!(write_fdc.command_byte_n, 8);
        assert!(write_fdc.command_flags.contains(CommandFlags::CMD_FLAG_DELETED_DATA));
    }

    #[test]
    fn write_data_command_starts_the_requested_data_mark_variant() {
        for deleted_data in [false, true] {
            let mut fdc = FloppyController::default();
            fdc.drive_ct = 1;
            fdc.drives[0] = FloppyDiskDrive::new(0, FloppyDriveType::Floppy360K);
            fdc.command_flags = if deleted_data {
                CommandFlags::CMD_FLAG_DELETED_DATA
            }
            else {
                CommandFlags::empty()
            };
            fdc.data_register_in.extend([0, 0, 0, 1, 2, 1, 0x2A, 0xFF]);

            assert!(matches!(fdc.command_write_data(), Continuation::ContinueAsOperation));
            assert!(matches!(
                fdc.active_operation
                    .as_ref()
                    .map(|operation| operation.operation_type()),
                Some(Operation::WriteData(params)) if params.deleted_data == deleted_data
            ));
        }
    }

    #[test]
    fn write_data_command_log_includes_transfer_flags() {
        let mut fdc = FloppyController::default();
        fdc.drive_ct = 1;
        fdc.drives[0] = FloppyDiskDrive::new(0, FloppyDriveType::Floppy360K);
        fdc.mt = true;
        fdc.command_mfm = true;
        fdc.command_skip = true;
        fdc.data_register_in.extend([0, 0, 0, 1, 2, 1, 0x2A, 0xFF]);

        assert!(matches!(fdc.command_write_data(), Continuation::ContinueAsOperation));
        assert_eq!(
            fdc.operation,
            Operation::WriteData(DataOperationParameters {
                drive: 0,
                physical_head: 0,
                id_chs: DiskChs::from((0, 0, 1)),
                sector_size: 2,
                eot: 1,
                gap3_len: 0x2A,
                data_len: 0xFF,
                multi_track: true,
                mfm: true,
                skip: true,
                deleted_data: false,
            })
        );
        assert!(fdc
            .get_debug_state()
            .cmd_log
            .iter()
            .any(|entry| entry.starts_with("WriteData: mt:1 mf:1 sk:1")));
    }

    #[test]
    fn read_data_command_captures_operation_parameters() {
        let mut fdc = FloppyController::default();
        fdc.drive_ct = 1;
        fdc.drives[0] = FloppyDiskDrive::new(0, FloppyDriveType::Floppy360K);
        fdc.drives[0].disk_present = true;
        fdc.mt = true;
        fdc.command_mfm = true;
        fdc.command_skip = true;
        fdc.command_flags = CommandFlags::CMD_FLAG_DELETED_DATA;
        fdc.data_register_in.extend([0x04, 38, 0, 8, 2, 9, 0x2A, 0xFF]);

        assert!(matches!(fdc.command_read_data(), Continuation::ContinueAsOperation));
        assert_eq!(
            fdc.operation,
            Operation::ReadData(DataOperationParameters {
                drive: 0,
                physical_head: 1,
                id_chs: DiskChs::from((38, 0, 8)),
                sector_size: 2,
                eot: 9,
                gap3_len: 0x2A,
                data_len: 0xFF,
                multi_track: true,
                mfm: true,
                skip: true,
                deleted_data: true,
            })
        );
    }

    #[test]
    fn read_track_command_captures_operation_parameters() {
        let mut fdc = FloppyController::default();
        fdc.drive_ct = 1;
        fdc.drives[0] = FloppyDiskDrive::new(0, FloppyDriveType::Floppy360K);
        fdc.drives[0].disk_present = true;
        fdc.mt = true;
        fdc.command_mfm = true;
        fdc.command_skip = true;
        fdc.data_register_in.extend([0x04, 38, 0, 8, 2, 9, 0x2A, 0xFF]);

        assert!(matches!(fdc.command_read_track(), Continuation::ContinueAsOperation));
        let params = DataOperationParameters {
            drive: 0,
            physical_head: 1,
            id_chs: DiskChs::from((38, 0, 8)),
            sector_size: 2,
            eot: 9,
            gap3_len: 0x2A,
            data_len: 0xFF,
            multi_track: true,
            mfm: true,
            skip: true,
            deleted_data: false,
        };
        assert_eq!(fdc.operation, Operation::ReadTrack(params));
        assert!(matches!(
            fdc.active_operation
                .as_ref()
                .map(|operation| operation.operation_type()),
            Some(Operation::ReadTrack(operation_params)) if operation_params == params
        ));
    }

    #[test]
    fn calibrate_command_shares_seek_handler_via_command_flags() {
        let mut fdc = FloppyController::default();
        fdc.handle_data_register_write(COMMAND_CALIBRATE_DRIVE);

        assert!(matches!(fdc.command, Command::CalibrateDrive));
        assert_eq!(fdc.command_byte_n, 1);
        assert!(fdc.command_flags.contains(CommandFlags::CMD_FLAG_CALIBRATE));
    }

    #[test]
    fn sense_interrupt_reports_pcn_for_each_reset_drive() {
        let mut fdc = FloppyController::default();
        let mut dma = dma::DMAController::new();
        let mut bus = BusInterface::default();
        fdc.handle_dor_write(0);
        fdc.handle_dor_write(DOR_FDC_RESET | DOR_DMA_ENABLED);
        fdc.run(&mut dma, &mut bus, FDC_RESET_TIME);

        for (status, pcn) in fdc.drive_status.iter_mut().zip([3, 7, 11, 19]) {
            status.pcn = pcn;
        }

        assert!(fdc.has_internal_interrupt_pending());
        for (drive, expected_pcn) in [3, 7, 11, 19].into_iter().enumerate() {
            fdc.command_sense_interrupt();
            let st0 = fdc.handle_data_register_read();
            let pcn = fdc.handle_data_register_read();

            assert_eq!(st0 & 0x03, drive as u8);
            assert_eq!(pcn, expected_pcn);
            assert!(!fdc.has_internal_interrupt_pending());
        }

        assert!(fdc.drive_status.iter().all(|status| status.unack_condition.is_none()));
    }

    #[test]
    fn sense_interrupt_updates_irq6_on_next_run() {
        let mut fdc = FloppyController::default();
        let mut dma = dma::DMAController::new();
        let mut bus = BusInterface::default();
        *bus.pic_mut() = Some(Box::new(Pic::new()));

        fdc.dor = DOR_FDC_RESET | DOR_DMA_ENABLED;
        fdc.last_command = Command::Seek;
        fdc.raise_sense_interrupt();
        fdc.update_pic_interrupt(&mut bus);

        assert!(fdc.irq_asserted);
        assert_ne!(
            bus.pic_mut().as_mut().unwrap().handle_command_register_read() & (1 << FDC_IRQ),
            0
        );

        fdc.write_u8(
            PCXT_IO_BASE + FDC_DATA_REGISTER,
            COMMAND_SENSE_INT_STATUS,
            Some(&mut bus),
            DeviceRunTimeUnit::Microseconds(0.0),
            None,
        );

        assert!(!fdc.has_internal_interrupt_pending());
        assert!(fdc.irq_asserted);
        assert_ne!(
            bus.pic_mut().as_mut().unwrap().handle_command_register_read() & (1 << FDC_IRQ),
            0
        );

        fdc.run(&mut dma, &mut bus, 0.0);

        assert!(!fdc.irq_asserted);
        assert_eq!(
            bus.pic_mut().as_mut().unwrap().handle_command_register_read() & (1 << FDC_IRQ),
            0
        );
    }

    #[test]
    fn sense_interrupt_acknowledges_only_the_sense_source() {
        let mut fdc = FloppyController::default();
        fdc.drive_status[0].unack_condition = Some(AckCondition::Seek);
        fdc.raise_sense_interrupt();
        fdc.raise_result_interrupt();

        fdc.command_sense_interrupt();

        assert!(!fdc.interrupt_sources.contains(InterruptSources::SENSE));
        assert!(fdc.interrupt_sources.contains(InterruptSources::RESULT_PHASE));
    }

    #[test]
    fn first_result_read_acknowledges_only_the_result_source() {
        let mut fdc = FloppyController::default();
        fdc.raise_sense_interrupt();
        fdc.send_results_phase(InterruptCode::NormalTermination, 0, DiskChs::new(0, 0, 1), 2, true);

        assert!(fdc.interrupt_sources.contains(InterruptSources::SENSE));
        assert!(fdc.interrupt_sources.contains(InterruptSources::RESULT_PHASE));

        let _st0 = fdc.handle_data_register_read();

        assert!(fdc.interrupt_sources.contains(InterruptSources::SENSE));
        assert!(!fdc.interrupt_sources.contains(InterruptSources::RESULT_PHASE));
    }

    #[test]
    fn non_dma_service_interrupt_tracks_mrq_until_cpu_service() {
        let mut fdc = FloppyController::default();
        let mut dma = dma::DMAController::new();
        let mut bus = BusInterface::default();
        *bus.pic_mut() = Some(Box::new(Pic::new()));
        fdc.dor = DOR_FDC_RESET | DOR_DMA_ENABLED;
        fdc.data_interface.begin(FdcDataMode::Pio, FdcDataDirection::ToHost);
        fdc.data_interface.send(0x5A, &mut dma, &mut bus).unwrap();

        fdc.run(&mut dma, &mut bus, 0.0);

        assert!(fdc.interrupt_sources.contains(InterruptSources::NON_DMA_SERVICE));
        assert!(fdc.irq_asserted);
        assert_eq!(fdc.handle_data_register_read(), 0x5A);
        assert!(!fdc.interrupt_sources.contains(InterruptSources::NON_DMA_SERVICE));
        assert!(fdc.irq_asserted);

        fdc.run(&mut dma, &mut bus, 0.0);

        assert!(!fdc.irq_asserted);
    }

    #[test]
    fn sense_interrupt_acknowledges_each_completed_seek() {
        let mut fdc = FloppyController::default();
        fdc.drive_status[0] = InternalDriveStatus {
            pcn: 12,
            unack_condition: Some(AckCondition::Seek),
            ..InternalDriveStatus::default()
        };
        fdc.drive_status[2] = InternalDriveStatus {
            pcn: 37,
            unack_condition: Some(AckCondition::Seek),
            ..InternalDriveStatus::default()
        };
        fdc.last_command = Command::Seek;
        fdc.raise_sense_interrupt();

        assert_eq!(
            fdc.read_main_status_register() & 0x0F,
            FDC_STATUS_FDD_A_BUSY | FDC_STATUS_FDD_C_BUSY
        );

        fdc.command_sense_interrupt();
        assert_eq!(fdc.handle_data_register_read() & 0x03, 0);
        assert_eq!(fdc.handle_data_register_read(), 12);
        assert_eq!(fdc.drive_status[0].unack_condition, None);
        assert_eq!(fdc.drive_status[2].unack_condition, Some(AckCondition::Seek));
        assert!(!fdc.has_internal_interrupt_pending());
        assert!(fdc.has_unacknowledged_drive_condition());
        assert_eq!(fdc.read_main_status_register() & 0x0F, FDC_STATUS_FDD_C_BUSY);

        fdc.command_sense_interrupt();
        assert_eq!(fdc.handle_data_register_read() & 0x03, 2);
        assert_eq!(fdc.handle_data_register_read(), 37);
        assert_eq!(fdc.drive_status[2].unack_condition, None);
        assert!(!fdc.has_internal_interrupt_pending());
        assert!(!fdc.has_unacknowledged_drive_condition());
        assert_eq!(fdc.read_main_status_register() & 0x0F, 0);
    }

    #[test]
    fn sense_interrupt_prioritizes_poll_then_seek_conditions() {
        let mut fdc = FloppyController::default();
        fdc.drive_status[0].unack_condition = Some(AckCondition::ReadyPoll);
        fdc.drive_status[2].unack_condition = Some(AckCondition::Seek);
        fdc.drive_status[2].pcn = 23;
        fdc.drive_select = 3;
        fdc.raise_sense_interrupt();

        fdc.command_sense_interrupt();
        assert_eq!(fdc.handle_data_register_read() & 0x03, 0);
        assert_eq!(fdc.handle_data_register_read(), 0);
        assert_eq!(fdc.drive_status[0].unack_condition, None);
        assert_eq!(fdc.drive_status[2].unack_condition, Some(AckCondition::Seek));
        assert!(!fdc.has_internal_interrupt_pending());

        fdc.command_sense_interrupt();
        let seek_st0 = fdc.handle_data_register_read();
        assert_eq!(seek_st0 & 0x03, 2);
        assert_ne!(seek_st0 & ST0_SEEK_END, 0);
        assert_eq!(fdc.handle_data_register_read(), 23);
        assert_eq!(fdc.drive_status[2].unack_condition, None);
        assert!(!fdc.has_internal_interrupt_pending());

        fdc.command_sense_interrupt();
        assert_eq!(fdc.handle_data_register_read(), 0x80);
        assert_eq!(fdc.data_register_out.len(), 0);
    }

    #[test]
    fn machine_reset_waits_for_reset_operation_before_creating_poll_conditions() {
        let mut fdc = FloppyController::default();
        let mut dma = dma::DMAController::new();
        let mut bus = BusInterface::default();
        for (status, pcn) in fdc.drive_status.iter_mut().zip([1, 2, 3, 4]) {
            status.pcn = pcn;
            status.seeking = true;
            status.unack_condition = Some(AckCondition::Seek);
        }

        fdc.reset();

        assert_eq!(fdc.dor, 0);
        assert!(fdc.dor_disabled);
        assert!(!fdc.mrq);
        assert_eq!(fdc.drive_status, [InternalDriveStatus::default(); FDC_MAX_DRIVES]);
        assert!(!fdc.has_internal_interrupt_pending());
        assert!(!fdc.has_external_interrupt_pending());

        fdc.handle_dor_write(DOR_FDC_RESET | DOR_DMA_ENABLED);

        assert!(!fdc.dor_disabled);
        assert!(matches!(fdc.operation, Operation::Reset));
        assert!(!fdc.mrq);
        assert_eq!(fdc.drive_status, [InternalDriveStatus::default(); FDC_MAX_DRIVES]);
        assert!(!fdc.has_internal_interrupt_pending());
        assert!(!fdc.has_external_interrupt_pending());

        fdc.run(&mut dma, &mut bus, FDC_RESET_TIME);

        assert_eq!(
            fdc.drive_status,
            [InternalDriveStatus {
                unack_condition: Some(AckCondition::ReadyPoll),
                ..InternalDriveStatus::default()
            }; FDC_MAX_DRIVES]
        );
        assert!(matches!(fdc.operation, Operation::NoOperation));
        assert!(fdc.mrq);
        assert!(fdc.has_internal_interrupt_pending());
        assert!(fdc.has_external_interrupt_pending());
    }

    #[test]
    fn motor_state_transitions_emit_presentable_events_once() {
        let mut fdc = FloppyController::default();
        let (sender, receiver) = crossbeam_channel::bounded(4);
        fdc.set_presentable_event_sender(2, sender);

        fdc.handle_dor_write(DOR_FDC_RESET);
        fdc.handle_dor_write(DOR_FDC_RESET | DOR_MOTOR_FDD_A);
        fdc.handle_dor_write(DOR_FDC_RESET | DOR_MOTOR_FDD_A);
        fdc.handle_dor_write(DOR_FDC_RESET);
        fdc.handle_dor_write(DOR_FDC_RESET);

        assert_eq!(
            receiver.try_iter().collect::<Vec<_>>(),
            vec![
                PresentableDeviceEvent::FloppyDrive {
                    controller: 2,
                    drive: 0,
                    event: FloppyDriveEvent::MotorStarted { media_present: false },
                },
                PresentableDeviceEvent::FloppyDrive {
                    controller: 2,
                    drive: 0,
                    event: FloppyDriveEvent::MotorStopped { media_present: false },
                },
            ]
        );
    }

    #[test]
    fn device_reset_emits_motor_stopped_for_a_running_motor() {
        let mut fdc = FloppyController::default();
        let (sender, receiver) = crossbeam_channel::bounded(2);
        fdc.set_presentable_event_sender(2, sender);
        fdc.drives[0].disk_present = true;

        fdc.handle_dor_write(DOR_FDC_RESET | DOR_MOTOR_FDD_A);
        assert!(fdc.drives[0].is_motor_on());
        assert!(matches!(
            receiver.try_recv().unwrap(),
            PresentableDeviceEvent::FloppyDrive {
                controller: 2,
                drive: 0,
                event: FloppyDriveEvent::MotorStarted { media_present: true },
            }
        ));

        fdc.reset_internal(false);
        assert_eq!(fdc.dor, 0);
        assert!(!fdc.drives[0].is_motor_on());
        assert!(matches!(
            receiver.try_recv().unwrap(),
            PresentableDeviceEvent::FloppyDrive {
                controller: 2,
                drive: 0,
                event: FloppyDriveEvent::MotorStopped { media_present: true },
            }
        ));
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn internal_reset_preserves_a_running_motor_without_an_event() {
        let mut fdc = FloppyController::default();
        let (sender, receiver) = crossbeam_channel::bounded(2);
        fdc.set_presentable_event_sender(2, sender);
        fdc.drives[0].disk_present = true;

        fdc.handle_dor_write(DOR_FDC_RESET | DOR_MOTOR_FDD_A);
        assert!(fdc.drives[0].is_motor_on());
        receiver.try_recv().unwrap();

        let dor = fdc.dor;
        fdc.reset_internal(true);
        assert_eq!(fdc.dor, dor);
        assert!(fdc.drives[0].is_motor_on());
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn successful_media_state_changes_emit_presentable_events() {
        let mut fdc = FloppyController::default();
        let (sender, receiver) = crossbeam_channel::bounded(4);
        fdc.set_presentable_event_sender(3, sender);
        fdc.drive_ct = 1;
        fdc.drives[0] = FloppyDiskDrive::new(0, FloppyDriveType::Floppy360K);

        fdc.create_new_image(0, StandardFormat::PcFloppy360, false).unwrap();
        fdc.create_new_image(0, StandardFormat::PcFloppy360, false).unwrap();
        fdc.unload_image(0);
        fdc.unload_image(0);

        assert_eq!(
            receiver.try_iter().collect::<Vec<_>>(),
            vec![
                PresentableDeviceEvent::FloppyDrive {
                    controller: 3,
                    drive: 0,
                    event: FloppyDriveEvent::MediaInserted,
                },
                PresentableDeviceEvent::FloppyDrive {
                    controller: 3,
                    drive: 0,
                    event: FloppyDriveEvent::MediaEjected,
                },
                PresentableDeviceEvent::FloppyDrive {
                    controller: 3,
                    drive: 0,
                    event: FloppyDriveEvent::MediaInserted,
                },
                PresentableDeviceEvent::FloppyDrive {
                    controller: 3,
                    drive: 0,
                    event: FloppyDriveEvent::MediaEjected,
                },
            ]
        );
    }

    #[test]
    fn failed_media_mount_emits_no_presentable_event() {
        let mut fdc = FloppyController::default();
        let (sender, receiver) = crossbeam_channel::bounded(1);
        fdc.set_presentable_event_sender(3, sender);
        fdc.drive_ct = 1;
        fdc.drives[0] = FloppyDiskDrive::new(0, FloppyDriveType::Floppy360K);

        assert!(fdc.load_image_from(0, Vec::new(), None, false).is_err());
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn interrupt_enable_is_controller_specific() {
        let mut pcxt_fdc = FloppyController::default();
        pcxt_fdc.dor = DOR_FDC_RESET;
        assert!(!pcxt_fdc.interrupts_enabled());

        pcxt_fdc.dor |= DOR_DMA_ENABLED;
        assert!(pcxt_fdc.interrupts_enabled());

        let mut pcjr_fdc = FloppyController::new(FdcType::IbmPCJrNec, Vec::new());
        assert!(!pcjr_fdc.interrupts_enabled());

        pcjr_fdc.watchdog_enabled = true;
        assert!(pcjr_fdc.interrupts_enabled());
    }

    #[test]
    fn pcjr_routes_only_watchdog_interrupts_to_pic() {
        let mut fdc = FloppyController::new(FdcType::IbmPCJrNec, Vec::new());
        fdc.watchdog_enabled = true;
        fdc.raise_sense_interrupt();

        assert!(!fdc.has_external_interrupt_pending());

        fdc.watchdog_interrupt_pending = true;
        assert!(fdc.has_external_interrupt_pending());
    }

    #[test]
    fn pcjr_watchdog_disable_deasserts_irq6_on_next_run() {
        let mut fdc = FloppyController::new(FdcType::IbmPCJrNec, Vec::new());
        let mut dma = dma::DMAController::new();
        let mut bus = BusInterface::default();
        *bus.pic_mut() = Some(Box::new(Pic::new()));

        fdc.watchdog_enabled = true;
        fdc.watchdog_interrupt_pending = true;
        fdc.update_pic_interrupt(&mut bus);

        assert!(fdc.irq_asserted);
        assert_ne!(
            bus.pic_mut().as_mut().unwrap().handle_command_register_read() & (1 << FDC_IRQ),
            0
        );

        fdc.write_u8(
            PCJR_IO_BASE + FDC_DIGITAL_OUTPUT_REGISTER,
            DOR_JRFDC_RESET,
            Some(&mut bus),
            DeviceRunTimeUnit::Microseconds(0.0),
            None,
        );

        assert!(!fdc.watchdog_interrupt_pending);
        assert!(fdc.irq_asserted);
        assert_ne!(
            bus.pic_mut().as_mut().unwrap().handle_command_register_read() & (1 << FDC_IRQ),
            0
        );

        fdc.run(&mut dma, &mut bus, 0.0);

        assert!(!fdc.irq_asserted);
        assert_eq!(
            bus.pic_mut().as_mut().unwrap().handle_command_register_read() & (1 << FDC_IRQ),
            0
        );
    }

    #[test]
    fn pcxt_dor_gates_pic_without_discarding_765_interrupt() {
        let mut fdc = FloppyController::default();
        fdc.raise_sense_interrupt();
        fdc.dor = DOR_FDC_RESET;

        assert!(!fdc.has_external_interrupt_pending());
        assert!(fdc.has_internal_interrupt_pending());

        fdc.dor |= DOR_DMA_ENABLED;
        assert!(fdc.has_external_interrupt_pending());
        assert!(fdc.has_internal_interrupt_pending());
    }

    #[test]
    fn reset_state_suppresses_irq6_without_discarding_interrupt_conditions() {
        let mut fdc = FloppyController::default();
        fdc.dor = DOR_FDC_RESET | DOR_DMA_ENABLED;
        fdc.drive_status[0].unack_condition = Some(AckCondition::ReadyPoll);
        fdc.raise_sense_interrupt();

        fdc.dor_disabled = true;
        assert!(!fdc.has_external_interrupt_pending());
        assert_eq!(fdc.drive_status[0].unack_condition, Some(AckCondition::ReadyPoll));

        fdc.dor_disabled = false;
        fdc.operation = Operation::Reset;
        assert!(!fdc.has_external_interrupt_pending());
        assert_eq!(fdc.drive_status[0].unack_condition, Some(AckCondition::ReadyPoll));

        fdc.operation = Operation::NoOperation;
        assert!(fdc.has_external_interrupt_pending());
        assert_eq!(fdc.drive_status[0].unack_condition, Some(AckCondition::ReadyPoll));
    }

    #[test]
    fn pcjr_watchdog_retrigger_restarts_timer() {
        let mut fdc = FloppyController::new(FdcType::IbmPCJrNec, Vec::new());

        fdc.handle_dor_write_jr(DOR_JRFDC_RESET | DOR_JRFDC_WATCHDOG_ENABLE | DOR_JRFDC_WATCHDOG_TRIGGER);
        fdc.watchdog_accumulator = 1234.0;
        fdc.handle_dor_write_jr(DOR_JRFDC_RESET | DOR_JRFDC_WATCHDOG_ENABLE);

        assert!(fdc.watchdog_enabled);
        assert!(fdc.watchdog_triggered);
        assert_eq!(fdc.watchdog_accumulator, 0.0);
    }
}

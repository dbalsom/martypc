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

    Implements the NEC µPD764 Floppy Disk Controller
*/
#![allow(dead_code)]
use lazy_static::lazy_static;
use std::collections::{HashMap, VecDeque};

use crate::{
    bus::{BusInterface, DeviceRunTimeUnit, IoDevice},
    devices::implementations::dma,
    machine_config::FloppyControllerConfig,
};

pub const FDC_IRQ: u8 = 0x06;
pub const FDC_DMA: usize = 2;
pub const FDC_MAX_DRIVES: usize = 4;
pub const FORMAT_BUFFER_SIZE: usize = 4;
pub const SECTOR_SIZE: usize = 512;

pub const FDC_DIGITAL_OUTPUT_REGISTER: u16 = 0x3F2;
pub const FDC_STATUS_REGISTER: u16 = 0x3F4;
pub const FDC_DATA_REGISTER: u16 = 0x3F5;

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

pub const COMMAND_MASK: u8 = 0b0001_1111;
pub const COMMAND_READ_TRACK: u8 = 0x02;
pub const COMMAND_WRITE_SECTOR: u8 = 0x05;
pub const COMMAND_READ_SECTOR: u8 = 0x06;
pub const COMMAND_WRITE_DELETED_SECTOR: u8 = 0x09;
pub const COMMAND_READ_DELETED_SECTOR: u8 = 0x0C;
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
pub const ST0_ABNORMAL_TERMINATION: u8 = 0b01000_0000;
pub const ST0_INVALID_OPCODE: u8 = 0b1000_0000;
pub const ST0_ABNORMAL_POLLING: u8 = 0b1100_0000;
pub const ST0_RESET: u8 = 0b1100_0000;

pub const ST1_NO_ID: u8 = 0b0000_0001;
pub const ST1_WRITE_PROTECT: u8 = 0b0000_0010;
pub const ST1_NODATA: u8 = 0b0000_0100;

pub const ST3_ESIG: u8 = 0b1000_0000;
pub const ST3_WRITE_PROTECT: u8 = 0b0100_0000;
pub const ST3_READY: u8 = 0b0010_0000;
pub const ST3_TRACK0: u8 = 0b0001_0000;
pub const ST3_DOUBLESIDED: u8 = 0b0000_1000;
pub const ST3_HEAD: u8 = 0b0000_0100;

pub struct DiskFormat {
    pub cylinders: u8,
    pub heads: u8,
    pub sectors: u8,
}

lazy_static! {
    static ref DISK_FORMATS: HashMap<usize, DiskFormat> = {
        let map = HashMap::from([
            (
                163_840,
                DiskFormat {
                    cylinders: 40,
                    heads: 1,
                    sectors: 8,
                },
            ),
            (
                184_320,
                DiskFormat {
                    cylinders: 40,
                    heads: 1,
                    sectors: 9,
                },
            ),
            (
                327_680,
                DiskFormat {
                    cylinders: 40,
                    heads: 2,
                    sectors: 8,
                },
            ),
            (
                368_640,
                DiskFormat {
                    cylinders: 40,
                    heads: 2,
                    sectors: 9,
                },
            ),
            (
                737_280,
                DiskFormat {
                    cylinders: 80,
                    heads: 2,
                    sectors: 9,
                },
            ),
            (
                1_228_800,
                DiskFormat {
                    cylinders: 80,
                    heads: 2,
                    sectors: 15,
                },
            ),
            (
                1_474_560,
                DiskFormat {
                    cylinders: 80,
                    heads: 2,
                    sectors: 18,
                },
            ),
        ]);
        map
    };
}

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

/// Represents the possible values of the Interrupt Code field in Status Register 0.
/// Returning 'AbnormalTermination' may result in a General Failure reading drive
/// message in DOS.
/// InvalidCommand should be returned for any command not handled by the FDC - later
/// controller models added more commands.
pub enum InterruptCode {
    NormalTermination,
    AbnormalTermination,
    InvalidCommand,
    AbnormalPolling,
}

/// Attempt to classify every general error condition a virtual disk drive may experience.
/// These states are used to build the status bytes presented afer a command has been
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

/// Classify operations - an Operation is intiated by any Command that does not immediately
/// terminate, and is called on a repeated basis by the run() method until complete.
///
/// Operations usually involve DMA transfers.
#[derive(Debug)]
pub enum Operation {
    NoOperation,
    ReadSector(u8, u8, u8, u8, u8, u8, u8), // cylinder, head, sector, sector_size, track_len, gap3_len, data_len
    WriteSector(u8, u8, u8, u8, u8, u8, u8), // cylinder, head, sector, sector_size, track_len, gap3_len, data_len
    FormatTrack(u8, u8, u8, u8),
}

pub struct DiskDrive {
    error_signal: bool,
    cylinder: u8,
    head: u8,
    sector: u8,
    max_cylinders: u8,
    max_heads: u8,
    max_sectors: u8,
    ready: bool,
    motor_on: bool,
    positioning: bool,
    have_disk: bool,
    write_protected: bool,
    disk_image: Vec<u8>,
}

impl DiskDrive {
    pub fn new() -> Self {
        Self {
            error_signal: false,
            cylinder: 0,
            head: 0,
            sector: 0,
            max_cylinders: 0,
            max_heads: 0,
            max_sectors: 0,
            ready: false,
            motor_on: false,
            positioning: false,
            have_disk: false,
            write_protected: false,
            disk_image: Vec::new(),
        }
    }
}

type CommandDispatchFn = fn(&mut FloppyController) -> Continuation;
pub enum Continuation {
    CommandComplete,
    ContinueAsOperation,
}

pub struct FloppyController {
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
    operation: Operation,
    operation_init: bool,
    send_interrupt: bool,
    pending_interrupt: bool,
    end_interrupt: bool,

    last_error: DriveError,

    data_register_out: VecDeque<u8>,
    data_register_in: VecDeque<u8>,
    format_buffer: VecDeque<u8>,

    drives: [DiskDrive; 4],
    drive_ct: usize,
    drive_select: usize,

    in_dma: bool,
    dma_byte_count: usize,
    dma_bytes_left: usize,
    xfer_size_sectors: u32,
    xfer_size_bytes: usize,
    xfer_completed_sectors: u32,
}

/// IO Port handlers for the FDC
impl IoDevice for FloppyController {
    fn read_u8(&mut self, port: u16, _delta: DeviceRunTimeUnit) -> u8 {
        match port {
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
        match port {
            FDC_DIGITAL_OUTPUT_REGISTER => {
                self.handle_dor_write(data);
            }
            FDC_STATUS_REGISTER => {
                log::warn!("Write to Read-only status register");
            }
            FDC_DATA_REGISTER => {
                self.handle_data_register_write(data);
            }
            _ => unreachable!("FLOPPY: Bad port #"),
        }
    }

    fn port_list(&self) -> Vec<u16> {
        vec![FDC_DIGITAL_OUTPUT_REGISTER, FDC_STATUS_REGISTER, FDC_DATA_REGISTER]
    }
}

impl Default for FloppyController {
    fn default() -> Self {
        Self {
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
            operation: Operation::NoOperation,
            operation_init: false,

            last_error: DriveError::NoError,

            send_interrupt: false,
            pending_interrupt: false,
            end_interrupt: false,

            data_register_out: VecDeque::new(),
            data_register_in: VecDeque::new(),
            format_buffer: VecDeque::new(),

            drives: [DiskDrive::new(), DiskDrive::new(), DiskDrive::new(), DiskDrive::new()],
            drive_ct: 0,
            drive_select: 0,

            in_dma: false,
            dma_byte_count: 0,
            dma_bytes_left: 0,
            xfer_size_sectors: 0,
            xfer_size_bytes: 0,
            xfer_completed_sectors: 0,
        }
    }
}

impl FloppyController {
    pub fn new(drive_ct: usize) -> Self {
        Self {
            drive_ct,
            ..Default::default()
        }
    }

    /// Reset the Floppy Drive Controller
    pub fn reset(&mut self) {
        self.status_byte = 0;
        self.drive_select = 0;
        self.reset_flag = true;
        self.reset_sense_count = 0;

        self.data_register_out.clear();
        self.data_register_in.clear();
        self.format_buffer.clear();

        self.mrq = true;
        self.dio = IoMode::FromCpu;

        // Seek to first sector for each drive, but keep the currently loaded floppy image(s).
        // After all, a reboot wouldn't eject your disks.
        for drive in &mut self.drives.iter_mut() {
            drive.head = 0;
            drive.cylinder = 0;
            drive.sector = 1;

            drive.ready = drive.have_disk;
            drive.motor_on = false;
            drive.positioning = false;
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

    pub fn drive_ct(&self) -> usize {
        self.drive_ct
    }

    /// Load a disk into the specified drive
    pub fn load_image_from(&mut self, drive_select: usize, src_vec: Vec<u8>) -> Result<(), &'static str> {
        if drive_select >= FDC_MAX_DRIVES {
            return Err("Invalid drive selection");
        }

        let image_len: usize = src_vec.len();

        // Disk images must contain whole sectors
        if image_len % SECTOR_SIZE > 0 {
            return Err("Invalid image length");
        }

        // Look up disk parameters based on image size
        if let Some(fmt) = DISK_FORMATS.get(&image_len) {
            self.drives[drive_select].max_cylinders = fmt.cylinders;
            self.drives[drive_select].max_heads = fmt.heads;
            self.drives[drive_select].max_sectors = fmt.sectors;
        }
        else {
            // No image format found.
            if image_len < 163_840 {
                // If image is smaller than single sided disk, assume single sided disk, 8 sectors per track
                // This is useful for loading things like boot sector images without having to copy them to
                // a full disk image
                self.drives[drive_select].max_cylinders = 40;
                self.drives[drive_select].max_heads = 1;
                self.drives[drive_select].max_sectors = 8;
            }
            else {
                return Err("Invalid image length");
            }
        }

        self.drives[drive_select].have_disk = true;
        self.drives[drive_select].disk_image = src_vec;
        log::debug!(
            "Loaded floppy image, size: {} c: {} h: {} s: {}",
            self.drives[drive_select].disk_image.len(),
            self.drives[drive_select].max_cylinders,
            self.drives[drive_select].max_heads,
            self.drives[drive_select].max_sectors
        );

        Ok(())
    }

    pub fn get_image_data(&self, drive_select: usize) -> Option<&[u8]> {
        if self.drives[drive_select].disk_image.len() > 0 {
            // We have at least some kind of disk image, return it
            Some(&self.drives[drive_select].disk_image)
        }
        else {
            None
        }
    }

    /// Unload (eject) the disk in the specified drive
    pub fn unload_image(&mut self, drive_select: usize) {
        let drive = &mut self.drives[drive_select];

        drive.cylinder = 0;
        drive.head = 0;
        drive.sector = 1;
        drive.max_cylinders = 40;
        drive.max_heads = 1;
        drive.max_sectors = 8;
        drive.have_disk = false;
        drive.disk_image.clear();
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

        if !self.dma {
            msr_byte |= FDC_STATUS_NON_DMA_MODE;
        }

        // DIO bit => 0=FDC Receiving 1=FDC Sending
        if let IoMode::ToCpu = self.dio {
            msr_byte |= FDC_STATUS_DIO;
        }

        // MRQ => Ready to receive or send data or commands via the data register
        // set this always on for now
        if self.mrq {
            msr_byte |= FDC_STATUS_MRQ;
        }

        //log::trace!("Status Register Read: Drive select:{}, Value: {:02X}", self.drive_select, msr_byte);
        msr_byte
    }

    pub fn motor_on(&mut self, drive_select: usize) {
        if self.drives[drive_select].have_disk {
            self.drives[drive_select].motor_on = true;
            self.drives[drive_select].ready = true;
        }
    }

    pub fn motor_off(&mut self, drive_select: usize) {
        if self.drives[drive_select].motor_on {
            log::trace!("Drive {}: turning motor off.", drive_select)
        }
        self.drives[drive_select].motor_on = false;
        //self.drives[drive_select].ready = false;    // Breaks booting(?)
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

    /// Create the ST0 status register bitfield with the given parameters.
    ///
    /// Note: returning an Interrupt Code of Abnormal Termination will result in a "General failure reading drive"
    ///
    pub fn make_st0_byte(&self, interrupt_code: InterruptCode, drive_select: usize, seek_end: bool) -> u8 {
        let mut st0: u8 = 0;

        // Set selected drive bits
        st0 |= (drive_select as u8) & 0x03;

        // Set active head bit
        if self.drives[drive_select].head == 1 {
            st0 |= ST0_HEAD_ACTIVE;
        }

        // Set ready bit
        if !self.drives[drive_select].ready || !self.drives[drive_select].have_disk {
            st0 |= ST0_NOT_READY;
        }

        // Set seek bit
        if seek_end {
            st0 |= ST0_SEEK_END;
        }

        // Set interrupt code
        st0 |= match interrupt_code {
            InterruptCode::NormalTermination => 0,
            InterruptCode::AbnormalTermination => ST0_ABNORMAL_TERMINATION,
            InterruptCode::InvalidCommand => ST0_INVALID_OPCODE,
            InterruptCode::AbnormalPolling => ST0_ABNORMAL_POLLING,
        };

        st0
    }

    /// Generate the value of the ST1 Status Register in response to a command
    pub fn make_st1_byte(&self, drive_select: usize) -> u8 {
        // The ST1 status register contains mostly error codes
        let mut st1_byte = 0;

        // Set the "No Data" bit if we received an invalid request
        match self.last_error {
            DriveError::BadRead | DriveError::BadWrite | DriveError::BadSeek => st1_byte |= ST1_NODATA,
            _ => {}
        }

        // Based on DOS's behavior regarding the "Not ready error" it appears that
        // operations without a disk timeout instead of returning a particular error
        // flag. Need to verify this on real hardware if possible.
        if !self.drives[drive_select].have_disk {
            st1_byte |= ST1_NODATA | ST1_NO_ID;
        }
        st1_byte
    }

    /// Generate the value of the ST2 Status Register in response to a command
    pub fn make_st2_byte(&self, _drive_select: usize) -> u8 {
        // The ST2 status register contains mostly error codes, so for now we can just always return success
        // by returning 0 until we handle possible errors.
        0
    }

    /// Generate the value of the ST3 Status Register in response to a command
    pub fn make_st3_byte(&self, drive_select: usize) -> u8 {
        // Set drive select bits DS0 & DS1
        let mut st3_byte = (drive_select & 0x03) as u8;

        // HDSEL signal: 1 == head 1 active
        if self.drives[drive_select].head == 1 {
            st3_byte |= ST3_HEAD;
        }

        // DSDR signal - Is this active for a double sided drive, or only when a double-sided disk is present?
        st3_byte |= ST3_DOUBLESIDED;

        if self.drives[drive_select].cylinder == 0 {
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

    /// Returns whether the CHS address is valid for the specified drive
    pub fn is_id_valid(&self, drive_select: usize, c: u8, h: u8, s: u8) -> bool {
        if !self.drives[drive_select].have_disk {
            return false;
        }

        // Sectors are 1-indexed
        if c < self.drives[drive_select].max_cylinders
            && h < self.drives[drive_select].max_heads
            && s <= self.drives[drive_select].max_sectors
        {
            return true;
        }

        return false;
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
            match command {
                COMMAND_READ_TRACK => {
                    log::trace!("Received Read Track command: {:02}", command);
                }
                COMMAND_WRITE_SECTOR => {
                    log::trace!("Received Write Sector command: {:02}", command);
                    self.set_command(Command::WriteSector, 8, FloppyController::command_write_sector);
                }
                COMMAND_READ_SECTOR => {
                    log::trace!("Received Read Sector command: {:02}", command);
                    self.set_command(Command::ReadSector, 8, FloppyController::command_read_sector);
                }
                COMMAND_WRITE_DELETED_SECTOR => {
                    log::trace!("Received Write Deleted Sector command: {:02}", command);
                }
                COMMAND_READ_DELETED_SECTOR => {
                    log::trace!("Received Read Deleted Sector command: {:02}", command);
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
                    // We read last byte expected for this command, so dispatch to the appropriate command handler
                    let mut result = Continuation::CommandComplete;

                    match self.command_fn {
                        None => {
                            log::error!("No associated method for command: {:?}!", self.command)
                        }
                        Some(command_fn) => {
                            result = command_fn(self);
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
           the possible drives. The BIOS expects to to see drive select bits 00 to 11 in the resuling st0 bytes,
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
        let cb1 = self.drives[self.drive_select].cylinder;
        self.data_register_out.push_back(cb1);

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
        self.drives[drive_select].cylinder = 0;
        self.drives[drive_select].head = head_select;
        self.drives[drive_select].sector = 1;

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
        if !self.is_id_valid(drive_select, cylinder, head_select, 1) {
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

        // Set CHS to new seeked values
        self.drives[drive_select].cylinder = cylinder;
        self.drives[drive_select].head = head_select;
        self.drives[drive_select].sector = 1;

        log::trace!(
            "command_seek_head completed: {} cylinder: {}",
            drive_head_select,
            cylinder
        );

        self.last_error = DriveError::NoError;
        self.send_interrupt = true;
        Continuation::CommandComplete
    }

    /// Perform the Read Sector Command
    pub fn command_read_sector(&mut self) -> Continuation {
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

        if head != head_select {
            // Head and head_select should always match. Seems redundant
            log::warn!("command_read_sector: non-matching head specifiers");
        }

        // Set drive_select for status register reads
        self.drive_select = drive_select;

        // Is there no disk in the drive?
        //
        // Initially I had this command send an interrupt and try to return some error code in the
        // sense bytes. However that would give inconsistent results in DOS like garbled directory
        // listings, or produce a "General error" reading drive instead of "Not Ready".
        // Also, returning error codes would cause the BIOS to issue an error 601.
        // So, we just let this operation time out if no disk is present, and that seems to work.
        if !self.drives[drive_select].have_disk {
            return Continuation::CommandComplete;
        }

        // Is this read out of bounds?
        if !self.is_id_valid(drive_select, cylinder, head, sector) {
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

        // "Seek" to values given in command
        self.drives[drive_select].cylinder = cylinder;
        self.drives[drive_select].head = head;
        self.drives[drive_select].sector = sector;

        // Start read operation
        self.operation = Operation::ReadSector(cylinder, head, sector, sector_size, track_len, gap3_len, data_len);

        // Clear MRQ until operation completion so there is no attempt to read result values
        self.mrq = false;

        // DMA now in progress (TODO: Support PIO mode?)
        self.in_dma = true;

        // The IBM PC BIOS only seems to ever set a track_len of 8. How do we support 9 sector (365k) floppies?
        // Answer: DOS seems to know to request sector #9 and the BIOS doesn't complain

        // Maximum size of DMA transfer

        //let max_sectors;
        //if track_len > 0 {
        //    max_sectors = track_len - sector + 1;
        //}
        //else {
        //    max_sectors = 1;
        //}
        //self.dma_bytes_left = max_sectors as usize * SECTOR_SIZE;

        log::trace!("command_read_sector: drive: {} cyl:{} head:{} sector:{} sector_size:{} track_len:{} gap3_len:{} data_len:{}",
            drive_select, cylinder, head, sector, sector_size, track_len, gap3_len, data_len);
        //log::trace!("command_read_sector: may operate on maximum of {} sectors", max_sectors);

        let base_address = self.get_image_address(self.drive_select, cylinder, head, sector);
        log::trace!("command_read_sector: base address of image read: {:06X}", base_address);

        // Flag to set up transfer size later
        self.operation_init = false;

        // Keep running command until DMA transfer completes
        Continuation::ContinueAsOperation
    }

    /// Perform the Write Sector Command
    pub fn command_write_sector(&mut self) -> Continuation {
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

        if head != head_select {
            log::warn!("command_write_sector: non-matching head specifiers");
        }

        // Set CHS
        self.drives[drive_select].cylinder = cylinder;
        self.drives[drive_select].head = head;
        self.drives[drive_select].sector = sector;

        // Start write operation
        self.operation = Operation::WriteSector(cylinder, head, sector, sector_size, track_len, gap3_len, data_len);

        // Clear MRQ until operation completion so there is no attempt to read result values
        self.mrq = false;

        // DMA now in progress (TODO: Support PIO mode?)
        self.in_dma = true;

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

        let base_address = self.get_image_address(self.drive_select, cylinder, head, sector);
        log::trace!(
            "command_write_sector: base address of image write: {:06X}",
            base_address
        );

        // Flag to set up transfer size later
        self.operation_init = false;

        // Keep running command until DMA transfer completes
        Continuation::ContinueAsOperation
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

        // Clear MRQ until operation completion so there is no attempt to read result values
        self.mrq = false;

        // DMA now in progress (TODO: Support PIO mode?)
        self.in_dma = true;

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

    /// Return a byte offset given a CHS (Cylinder, Head, Sector) address
    pub fn get_image_address(&self, drive_select: usize, cylinder: u8, head: u8, sector: u8) -> usize {
        if sector == 0 {
            log::warn!("Invalid sector == 0");
            return 0;
        }
        let hpc = self.drives[drive_select].max_heads as usize;
        let spt = self.drives[drive_select].max_sectors as usize;
        let lba: usize = (cylinder as usize * hpc + (head as usize)) * spt + (sector as usize - 1);
        lba * SECTOR_SIZE
    }

    pub fn get_chs_sector_offset(
        &self,
        drive_select: usize,
        sector_offset: u32,
        cylinder: u8,
        head: u8,
        sector: u8,
    ) -> (u8, u8, u8) {
        let mut c = cylinder;
        let mut h = head;
        let mut s = sector;

        for _ in 0..sector_offset {
            (c, h, s) = self.get_next_sector(drive_select, c, h, s);
        }

        (c, h, s)
    }

    pub fn get_next_sector(&self, drive_select: usize, cylinder: u8, head: u8, sector: u8) -> (u8, u8, u8) {
        if sector < self.drives[drive_select].max_sectors {
            // Not at last sector, just return next sector
            (cylinder, head, sector + 1)
        }
        else if head < self.drives[drive_select].max_heads - 1 {
            // At last sector, but not at last head, go to next head, same cylinder, sector 1
            (cylinder, head + 1, 1)
        }
        else if cylinder < self.drives[drive_select].max_cylinders - 1 {
            // At last sector and last head, go to next cylinder, head 0, sector 1
            (cylinder + 1, 0, 1)
        }
        else {
            // Return end of drive? What does this do on real hardware
            (self.drives[drive_select].max_cylinders, 0, 1)
        }
    }

    fn send_results_phase(&mut self, result: InterruptCode, drive_select: usize, c: u8, h: u8, s: u8, sector_size: u8) {
        // Create the 3 status bytes. Most of these are error flags of some sort
        let st0_byte = self.make_st0_byte(result, drive_select, false);
        let st1_byte = self.make_st1_byte(drive_select);
        let st2_byte = self.make_st2_byte(drive_select);

        // Push result codes into FIFO
        self.data_register_out.clear();
        self.data_register_out.push_back(st0_byte);
        self.data_register_out.push_back(st1_byte);
        self.data_register_out.push_back(st2_byte);

        self.data_register_out.push_back(c);
        self.data_register_out.push_back(h);
        self.data_register_out.push_back(s);
        self.data_register_out.push_back(sector_size);

        self.send_data_register();
        // Clear error state
        self.last_error = DriveError::NoError;
    }

    fn operation_read_sector(
        &mut self,
        dma: &mut dma::DMAController,
        bus: &mut BusInterface,
        cylinder: u8,
        head: u8,
        sector: u8,
        sector_size: u8,
        _track_len: u8,
    ) {
        if !self.in_dma {
            log::error!("FDC in invalid state: ReadSector operation without DMA! Aborting.");
            self.operation = Operation::NoOperation;
            return;
        }

        // Is read valid?

        if !self.operation_init {
            let xfer_size = dma.get_dma_transfer_size(FDC_DMA);

            if xfer_size % SECTOR_SIZE != 0 {
                log::warn!("DMA word count not multiple of sector size");
            }

            let xfer_sectors = xfer_size / SECTOR_SIZE;
            log::trace!("DMA programmed for transfer of {} sectors", xfer_sectors);

            let dst_address = dma.get_dma_transfer_address(FDC_DMA);
            log::trace!("DMA destination address: {:05X}", dst_address);

            self.xfer_size_sectors = xfer_sectors as u32;
            self.xfer_completed_sectors = 0;
            self.xfer_size_bytes = xfer_sectors * SECTOR_SIZE;
            self.dma_bytes_left = xfer_sectors * SECTOR_SIZE;
            self.operation_init = true;
        }

        if self.dma_bytes_left > 0 {
            // Bytes left to transfer

            // Calculate how many sectors we've done
            if (self.dma_bytes_left < self.xfer_size_bytes) && (self.dma_bytes_left % SECTOR_SIZE == 0) {
                // Completed one sector

                self.xfer_completed_sectors += 1;
                log::trace!(
                    "operation_read_sector: Transferred {} sectors.",
                    self.xfer_completed_sectors
                );
            }

            // Check if DMA is ready
            if dma.check_dma_ready(FDC_DMA) {
                let base_address = self.get_image_address(self.drive_select, cylinder, head, sector);
                let byte_address = base_address + self.dma_byte_count;

                //log::trace!("Byte address for FDC read: {:04X}", byte_address);
                if byte_address >= self.drives[self.drive_select].disk_image.len() {
                    log::error!(
                        "Read past end of disk image: {}/{}!",
                        byte_address,
                        self.drives[self.drive_select].disk_image.len()
                    );
                    self.dma_bytes_left = 0;
                }
                else {
                    let byte = self.drives[self.drive_select].disk_image[byte_address];

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
        }
        else {
            // No more bytes left to transfer. Finalize operation

            let tc = dma.check_terminal_count(FDC_DMA);
            if !tc {
                log::warn!("FDC sector read complete without DMA terminal count.");
            }

            self.dma_byte_count = 0;
            self.dma_bytes_left = 0;

            let (new_c, new_h, new_s) = self.get_chs_sector_offset(
                self.drive_select,
                self.xfer_completed_sectors + 1,
                cylinder,
                head,
                sector,
            );
            //let (new_c, new_h, new_s) = self.get_next_sector(self.drive_select, cylinder, head, sector);

            // Terminate normally by sending results registers
            self.send_results_phase(
                InterruptCode::NormalTermination,
                self.drive_select,
                new_c,
                new_h,
                new_s,
                sector_size,
            );

            // Set new CHS
            self.drives[self.drive_select].cylinder = new_c;
            self.drives[self.drive_select].head = new_h;
            self.drives[self.drive_select].sector = new_s;

            log::trace!(
                "operation_read_sector completed: new cylinder: {} head: {} sector: {}",
                new_c,
                new_h,
                new_s
            );
            // Finalize operation

            self.operation = Operation::NoOperation;
            self.send_interrupt = true;
        }
    }

    fn operation_write_sector(
        &mut self,
        dma: &mut dma::DMAController,
        bus: &mut BusInterface,
        cylinder: u8,
        head: u8,
        sector: u8,
        sector_size: u8,
        _track_len: u8,
    ) {
        if !self.in_dma {
            log::error!("Error: WriteSector operation without DMA!");
            self.operation = Operation::NoOperation;
            return;
        }

        if !self.operation_init {
            let xfer_size = dma.get_dma_transfer_size(FDC_DMA);

            if xfer_size % SECTOR_SIZE != 0 {
                log::warn!("DMA word count not multiple of sector size");
            }

            let xfer_sectors = xfer_size / SECTOR_SIZE;
            log::trace!("DMA programmed for transfer of {} sectors", xfer_sectors);

            self.dma_bytes_left = xfer_sectors * SECTOR_SIZE;
            self.operation_init = true;
        }

        if self.dma_bytes_left == SECTOR_SIZE {
            let dst_address = dma.get_dma_transfer_address(FDC_DMA);
            log::trace!("DMA source address: {:05X}", dst_address)
        }

        if self.dma_bytes_left > 0 {
            // Bytes left to transfer

            // Check if DMA is ready
            if dma.check_dma_ready(FDC_DMA) {
                let base_address = self.get_image_address(self.drive_select, cylinder, head, sector);
                let byte_address = base_address + self.dma_byte_count;

                //log::trace!("Byte address for FDC write: {:04X}", byte_address);
                if byte_address >= self.drives[self.drive_select].disk_image.len() {
                    log::error!(
                        "Write past end of disk image: {}/{}!",
                        byte_address,
                        self.drives[self.drive_select].disk_image.len()
                    );
                    self.dma_bytes_left = 0;
                    // cleanup ?
                }
                else {
                    let byte = dma.do_dma_read_u8(bus, FDC_DMA);
                    self.drives[self.drive_select].disk_image[byte_address] = byte;
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
        }
        else {
            // No more bytes left to transfer. Finalize operation

            let tc = dma.check_terminal_count(FDC_DMA);
            if !tc {
                log::warn!("FDC sector write complete without DMA terminal count.");
            }

            self.dma_byte_count = 0;
            self.dma_bytes_left = 0;

            let (new_c, new_h, new_s) = self.get_next_sector(self.drive_select, cylinder, head, sector);

            // Terminate normally by sending results registers
            self.send_results_phase(
                InterruptCode::NormalTermination,
                self.drive_select,
                new_c,
                new_h,
                new_s,
                sector_size,
            );

            // Set new CHS
            self.drives[self.drive_select].cylinder = new_c;
            self.drives[self.drive_select].head = new_h;
            self.drives[self.drive_select].sector = new_s;

            // Finalize operation
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

            let xfer_sectors = xfer_size / SECTOR_SIZE;
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
                0,
                0,
                0,
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
    pub fn run(&mut self, dma: &mut dma::DMAController, bus: &mut BusInterface, _us: f64) {
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
            Operation::ReadSector(cylinder, head, sector, sector_size, track_len, _gap3_len, _data_len) => {
                self.operation_read_sector(dma, bus, cylinder, head, sector, sector_size, track_len)
            }
            Operation::WriteSector(cylinder, head, sector, sector_size, track_len, _gap3_len, _data_len) => {
                self.operation_write_sector(dma, bus, cylinder, head, sector, sector_size, track_len)
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

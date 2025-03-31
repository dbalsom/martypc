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

    devices::hdc.rs

    Implements the IBM/Xebec 20MB Fixed Disk Adapter

*/

#![allow(dead_code)]

use crate::{
    bus::{BusInterface, DeviceRunTimeUnit, IoDevice},
    cpu_common::LogicAnalyzer,
    device_types::{chs::DiskChs, geometry::DriveGeometry, hdc::HardDiskFormat},
    devices::{
        dma,
        hdc::{at_formats::AtFormats, DEFAULT_SECTOR_SIZE},
    },
    vhd::{VHDGeometry, VirtualHardDisk},
};
use binrw::{binrw, BinWrite};
use core::{fmt, fmt::Display};
use fluxfox::io::ReadBytesExt;
use modular_bitfield::bitfield;
use std::{
    collections::VecDeque,
    error::Error,
    fmt::Debug,
    io::{Cursor, Seek, SeekFrom, Write},
};

pub const DRIVE_CT: usize = 2;

// Public consts
pub const DEFAULT_IO_BASE: u16 = 0x300;

pub const HDC_IRQ: u8 = 0x05;
pub const HDC_DMA: usize = 0x03;
pub const SECTOR_SIZE: usize = 512;

pub const REG_SHIFT: u16 = 1;

macro_rules! mod_swap {
    ($val:expr) => {{
        let b0 = ($val & 0x01) << 3; // old bit 0
        let b3 = ($val >> 3) & 0x01; // old bit 3
        ($val & !0x09) | b3 | b0
    }};
}

pub const HDC_DATA_REGISTER0: u16 = 0x0;
pub const HDC_DATA_REGISTER1: u16 = 0x1;
pub const HDC_ERROR_REGISTER: u16 = mod_swap!(0x01);
pub const HDC_SECTOR_COUNT_REGISTER: u16 = mod_swap!(0x02);
pub const HDC_SECTOR_NUMBER_REGISTER: u16 = mod_swap!(0x03);
pub const HDC_CYLINDER_LOW_REGISTER: u16 = mod_swap!(0x04);
pub const HDC_CYLINDER_HIGH_REGISTER: u16 = mod_swap!(0x05);
pub const HDC_DRIVE_HEAD_REGISTER: u16 = mod_swap!(0x06);
pub const HDC_STATUS_REGISTER: u16 = mod_swap!(0x07);

// Private consts
const DBC_LEN: u32 = 5; // Length of Device Control Block, the 5 bytes that are sent after a command opcode
const IDC_LEN: u32 = 8; // The Initialize Drive Characteristics command is followed by 8 bytes after DCB

const ENABLE_DMA_MASK: u8 = 0x01;
const ENABLE_IRQ_MASK: u8 = 0x02;

// Controller error codes
const NO_ERROR_CODE: u8 = 0;
const ERR_NO_INDEX_SIGNAL: u8 = 0b00_0010;
const ERR_WRITE_FAULT: u8 = 0b00_0011;
const ERR_NO_READY_SIGNAL: u8 = 0b00_0100;
const ERR_SECTOR_NOT_FOUND: u8 = 0b01_0100;
const ERR_SEEK_ERROR: u8 = 0b01_0101;
const ERR_INVALID_COMMAND: u8 = 0b10_0000;
const ERR_ILLEGAL_ACCESS: u8 = 0b10_0001;

const RESET_DELAY_US: f64 = 200_000.0; // 200ms

#[binrw]
#[derive(Debug, Default)]
pub struct AtaString<const N: usize> {
    // On read, pick up N bytes...
    #[br(count = N)]
    // On write, you might do something else, like explicitly check length:
    #[bw(assert(raw.len() == N, "raw length must be N"))]
    raw: Vec<u8>,
}

impl<const N: usize> AtaString<N> {
    // Provide a way to create the wrapper from a normal ASCII string.
    pub fn from_str(s: &str) -> Self {
        // We want exactly N bytes.
        // Typically, N is a multiple of 2 (since these are 16-bit words).
        let mut buf = vec![b' '; N];
        let bytes = s.as_bytes();
        let len = bytes.len().min(N);
        buf[..len].clone_from_slice(&bytes[..len]);

        // Now swap each pair in place to match ATA’s weird
        // big-endian-within-each-16-bit-word requirement.
        for chunk in buf.chunks_mut(2) {
            chunk.swap(0, 1);
        }

        Self { raw: buf }
    }

    pub fn as_str(&self) -> String {
        // Reverse the swapping to get a normal ASCII string
        // if you ever want to read it back out.
        let mut unwrapped = self.raw.clone();
        for chunk in unwrapped.chunks_mut(2) {
            chunk.swap(0, 1);
        }
        // Then we can convert it to a Rust string (losing trailing spaces, etc.).
        String::from_utf8_lossy(&unwrapped).to_string()
    }
}

/// An implementation of a 16-bit register.
#[derive(Default, Debug)]
pub struct AtaRegister16 {
    pub bytes: [Option<u8>; 2],
}

impl AtaRegister16 {
    pub fn new() -> Self {
        Self::default()
    }
    /// Set the entire register to a 16-bit value.
    #[inline]
    pub fn set_16(&mut self, value: u16) {
        self.bytes[0] = Some((value & 0xFF) as u8);
        self.bytes[1] = Some((value >> 8) as u8);
    }
    /// Set the high byte of the register.
    #[inline]
    pub fn set_hi(&mut self, byte: u8) {
        self.bytes[1] = Some(byte);
    }
    /// Set the low byte of the register.
    #[inline]
    pub fn set_lo(&mut self, byte: u8) {
        self.bytes[0] = Some(byte);
    }
    /// Return a bool representing whether the register has been fully written to.
    #[inline]
    pub fn ready(&self) -> bool {
        self.bytes[0].is_some() && self.bytes[1].is_some()
    }
    /// If a full value has been written, return Some(value) and clear the register. Otherwise, return None.
    #[inline]
    pub fn get(&mut self) -> Option<u16> {
        if self.ready() {
            let value = Some(u16::from_le_bytes([self.bytes[0].unwrap(), self.bytes[1].unwrap()]));
            *self = Self::default();
            value
        }
        else {
            None
        }
    }
    #[inline]
    pub fn get_bytes(&mut self) -> Option<[u8; 2]> {
        if self.ready() {
            let bytes = [self.bytes[0].unwrap(), self.bytes[1].unwrap()];
            *self = Self::default();
            Some(bytes)
        }
        else {
            None
        }
    }
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub enum OperationError {
    NoError,
    NoReadySignal,
    InvalidCommand,
    IllegalAccess,
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum ControllerError {
    NoError,
    InvalidDevice,
    UnsupportedVHD,
}
impl Error for ControllerError {}
impl Display for ControllerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            ControllerError::NoError => write!(f, "No error."),
            ControllerError::InvalidDevice => {
                write!(f, "The specified Device ID was out of range [0..1]")
            }
            ControllerError::UnsupportedVHD => {
                write!(f, "The VHD file did not match the list of supported drive types.")
            }
        }
    }
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub enum State {
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
pub enum Command {
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

type CommandDispatchFn = fn(&mut XtIdeController, &mut BusInterface) -> Continuation;

#[bitfield]
#[derive(Copy, Clone)]
pub struct StatusRegister {
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
#[derive(Copy, Clone)]
pub struct ErrorRegister {
    pub amnf: bool, // Address Mark Not Found
    pub tk0:  bool, // Track 0 Not Found
    pub abrt: bool, // Command Aborted
    pub mcr:  bool, // Media Change Request
    pub idnf: bool, // ID Not Found
    pub mc:   bool, // Media changed
    pub unc:  bool, // Unrecoverable
    pub bbk:  bool, // Bad Block
}

#[binrw]
#[derive(Default)]
#[brw(little)]
pub struct DriveIdentification {
    pub general: u16,
    pub cylinders: u16,
    pub specific_configuration: u16,
    pub num_heads: u16,
    pub unformatted_bytes_per_track: u16,
    pub unformatted_bytes_per_sector: u16,
    pub sectors_per_track: u16,
    pub vendor_unique: [u16; 3],
    pub serial_no: [u8; 20],
    pub buffer_type: u16,
    pub buffer_size: u16,
    pub long_cmd_bytes: u16,
    pub firmware_revision: [u8; 8],
    pub model_number: AtaString<40>,
    pub maximum_block_transfer: u8,
    pub vendor_unique2: u8,
    pub double_word_io: u16,
    pub capabilities: u16,
    pub reserved: u16,
    pub pio_timing: u16,
    pub dma_timing: u16,
    pub field_validity: u16,
    pub current_cylinders: u16,
    pub current_heads: u16,
    pub current_sectors_per_track: u16,
    pub current_capacity_low: u16,
    pub current_capacity_high: u16,
    pub multiple_sector: u16,
    pub user_addressable_sectors: u32,
    pub single_word_dma: u16,
    pub multi_word_dma: u16,
}

impl DriveIdentification {
    pub fn new(vhdgeometry: &VHDGeometry) -> Self {
        let current_capacity: u32 = vhdgeometry.c as u32 * vhdgeometry.h as u32 * vhdgeometry.s as u32;

        DriveIdentification {
            general: 0b0000_0000_0100_0000, // Fixed Disk
            cylinders: vhdgeometry.c,
            num_heads: vhdgeometry.h as u16,
            unformatted_bytes_per_track: SECTOR_SIZE as u16 * vhdgeometry.s as u16,
            unformatted_bytes_per_sector: SECTOR_SIZE as u16,
            sectors_per_track: vhdgeometry.s as u16,
            current_cylinders: vhdgeometry.c,
            current_heads: vhdgeometry.h as u16,
            current_sectors_per_track: vhdgeometry.s as u16,
            serial_no: "0000000000000MARTYPC".as_bytes().try_into().unwrap(),
            model_number: AtaString::from_str("MartyPC Virtual Drive"),
            firmware_revision: "1.2.3.4 ".as_bytes().try_into().unwrap(),
            maximum_block_transfer: 1,
            field_validity: 1,
            current_capacity_low: current_capacity as u16,
            current_capacity_high: (current_capacity >> 16) as u16,
            user_addressable_sectors: current_capacity,
            ..DriveIdentification::default()
        }
    }
}

impl IoDevice for XtIdeController {
    fn read_u8(&mut self, port: u16, _delta: DeviceRunTimeUnit) -> u8 {
        match port - self.io_base {
            HDC_DATA_REGISTER0 | HDC_DATA_REGISTER1 => {
                
                // let pos = self.sector_buffer.stream_position().unwrap();
                // if pos == (SECTOR_SIZE as u64 - 1) {
                //     log::debug!("{port:03X}: Sector buffer read complete #[{pos}]: {byte:02X}");
                // }
                self.handle_data_register_read()
            }
            HDC_ERROR_REGISTER => self.error_register.into_bytes()[0],
            HDC_SECTOR_COUNT_REGISTER => self.sector_count_register,
            HDC_SECTOR_NUMBER_REGISTER => self.sector_number_register,
            HDC_CYLINDER_LOW_REGISTER => self.cylinder_low_register,
            HDC_CYLINDER_HIGH_REGISTER => self.cylinder_high_register,
            HDC_DRIVE_HEAD_REGISTER => self.drive_head_register,
            HDC_STATUS_REGISTER => self.handle_status_register_read(),
            _ => {
                log::error!("Read from invalid port: {:03X}", port);
                0
            }
        }
    }

    #[rustfmt::skip]
    fn write_u8(
        &mut self,
        port: u16,
        data: u8,
        bus: Option<&mut BusInterface>,
        _delta: DeviceRunTimeUnit,
        _analyzer: Option<&mut LogicAnalyzer>,
    ) {
        match port - self.io_base {
            HDC_DATA_REGISTER0 => {
                //log::trace!("{:03X}: Data register (low) write: {:02X}", port, data);
                self.handle_data_register_write(data, port & 0x01 == 0);
            }
            HDC_DATA_REGISTER1 => {
                //log::trace!("{:03X}: Data register (high) write: {:02X}", port, data);
                self.handle_data_register_write(data, port & 0x01 == 0);
            }
            HDC_ERROR_REGISTER => {
                log::warn!("{:03X}: Feature register write: {:02X}", port, data);
            }
            HDC_SECTOR_COUNT_REGISTER => {
                log::debug!("{:03X}: Sector count register write: {:02X}", port, data);
                self.sector_count_register = data;
            }
            HDC_SECTOR_NUMBER_REGISTER => {
                log::debug!("{:03X}: Sector number register write: {:02X}", port, data);
                self.sector_number_register = data;
            }
            HDC_CYLINDER_LOW_REGISTER => {
                log::debug!("{:03X}: Cylinder low register write: {:02X}", port, data);
                self.cylinder_low_register = data;
            }
            HDC_CYLINDER_HIGH_REGISTER => {
                log::debug!("{:03X}: Cylinder high register write: {:02X}", port, data);
                self.cylinder_high_register = data;
            }
            HDC_DRIVE_HEAD_REGISTER => {
                log::debug!("{:03X}: Drive/head register write: {:02X}", port, data);
                self.drive_head_register = data;
            }
            HDC_STATUS_REGISTER => {
                self.handle_command_register_write(data, bus.unwrap());
            }
            _ => {
                log::error!("Write to invalid port");
            }
        }
    }

    #[rustfmt::skip]
    fn port_list(&self) -> Vec<(String, u16)> {
        vec![
            (String::from("XTIDE Data Register"), self.io_base + HDC_DATA_REGISTER0),
            (String::from("XTIDE Data Register"), self.io_base + HDC_DATA_REGISTER1),
            (String::from("XTIDE Error Register"), self.io_base + HDC_ERROR_REGISTER),
            (String::from("XTIDE Sector Count Register"), self.io_base + HDC_SECTOR_COUNT_REGISTER),
            (String::from("XTIDE Sector Number Register"), self.io_base + HDC_SECTOR_NUMBER_REGISTER),
            (String::from("XTIDE Cylinder Low Register"), self.io_base + HDC_CYLINDER_LOW_REGISTER),
            (String::from("XTIDE Cylinder High Register"), self.io_base + HDC_CYLINDER_HIGH_REGISTER),
            (String::from("XTIDE Drive/Head Register"), self.io_base + HDC_DRIVE_HEAD_REGISTER),
            (String::from("XTIDE Status Register"), self.io_base + HDC_STATUS_REGISTER),
        ]
    }
}

pub struct HardDisk {
    position: DiskChs,
    geometry: DriveGeometry,
    sector_buf: Vec<u8>,
    vhd: Option<VirtualHardDisk>,
}

impl Debug for HardDisk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HardDisk")
            .field("position", &self.position)
            .field("geometry", &self.geometry)
            .finish()
    }
}

impl HardDisk {
    pub fn new(geometry: DriveGeometry) -> Self {
        Self {
            position: DiskChs::new(0, 0, 1),
            geometry,
            sector_buf: vec![0; SECTOR_SIZE],
            vhd: None,
        }
    }

    pub fn set_vhd(&mut self, vhd: VirtualHardDisk) {
        self.vhd = Some(vhd);
    }

    pub fn geometry(&self) -> DriveGeometry {
        self.geometry
    }

    pub fn set_geometry(&mut self, geometry: DriveGeometry) {
        self.geometry = geometry;
    }

    pub fn position(&self) -> DiskChs {
        self.position
    }

    /// Returns the current position in VHD format CHS (0-indexed)
    pub fn position_vhd(&self) -> DiskChs {
        DiskChs::new(self.position.c, self.position.h, self.position.s.saturating_sub(1))
    }

    pub fn seek(&mut self, chs: DiskChs) {
        if self.geometry.contains(chs) {
            log::debug!("HardDisk::seek(): Seeking to CHS: {}", chs);
            self.position = chs;
        }
        else {
            log::error!(
                "HardDisk::seek(): Attempted to seek to invalid CHS: {} for geometry: {}",
                chs,
                self.geometry
            );
        }
    }

    pub fn next_sector(&mut self) -> Option<DiskChs> {
        self.position.next_sector(&self.geometry)
    }
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

pub enum Continuation {
    CommandComplete,
    ContinueAsOperation,
}

#[allow(dead_code)]
pub struct DeviceControlBlock {
    drive_select: usize,
    c: u16,
    h: u8,
    s: u8,
    interleave: u8,
    block_count: u8,
    step: u8,
    retry_on_ecc: bool,
    disable_retry: bool,
}

#[allow(dead_code)]
pub struct XtIdeController {
    io_base: u16,
    drives: Box<[HardDisk; DRIVE_CT]>,
    drive_ct: usize,
    drive_select: usize,

    supported_formats: Vec<HardDiskFormat>,
    drive_type_dip: u8,
    state: State,
    last_error: OperationError,
    last_error_drive: usize,
    error_flag: bool,
    receiving_dcb: bool,
    command: Command,
    command_chs: DiskChs,
    command_fn: Option<CommandDispatchFn>,
    last_command: Command,
    command_byte_n: u32,
    command_queue: VecDeque<u8>,
    command_result_pending: bool,

    sector_buffer_idx: usize,
    sector_buffer: Cursor<Vec<u8>>,
    status_register: StatusRegister,
    error_register: ErrorRegister,
    sector_count_register: u8,
    sector_number_register: u8,
    cylinder_low_register: u8,
    cylinder_high_register: u8,
    drive_head_register: u8,

    status_reads:  u64,
    data_reads:    u64,
    data_writes:   u64,
    data_register: AtaRegister16,

    operation_status: Box<[OperationStatus; DRIVE_CT]>,

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

impl Default for XtIdeController {
    fn default() -> Self {
        let mut status_register = StatusRegister::from_bytes([0]);
        status_register.set_ready(true);
        let error_register = ErrorRegister::from_bytes([0]);
        let mut default_disks = Vec::new();

        // Loop because VHD isn't Clone
        for _ in 0..DRIVE_CT {
            default_disks.push(HardDisk::new(DriveGeometry::default()));
        }

        let disk_box = default_disks.into_boxed_slice();
        Self {
            io_base: DEFAULT_IO_BASE,
            drives: disk_box.try_into().unwrap(),
            drive_ct: 1,
            drive_select: 0,
            supported_formats: AtFormats::vec(),
            drive_type_dip: 0,
            state: State::Reset,
            last_error: OperationError::NoError,
            last_error_drive: 0,
            error_flag: false,
            receiving_dcb: false,
            command: Command::None,
            command_chs: DiskChs::new(0, 0, 1),
            command_fn: None,
            last_command: Command::None,
            command_byte_n: 0,
            command_queue: VecDeque::new(),
            command_result_pending: false,
            sector_buffer_idx: SECTOR_SIZE,
            sector_buffer: Cursor::new(vec![0; SECTOR_SIZE]),
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
            operation_status: vec![Default::default(); DRIVE_CT]
                .into_boxed_slice()
                .try_into()
                .unwrap(),
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

impl XtIdeController {
    pub fn new(io_base: Option<u16>, drive_ct: usize) -> Self {
        Self {
            io_base: io_base.unwrap_or(DEFAULT_IO_BASE),
            drive_ct,
            ..Default::default()
        }
    }

    pub fn reset(&mut self) {
        log::trace!("Resetting Hard Disk Controller...");
        self.command_queue.clear();
        self.command_result_pending = false;
        self.command_byte_n = 0;

        self.interrupt_active = false;
        self.send_interrupt = false;
        self.send_dreq = false;
        self.state = State::Reset;
        self.state_accumulator = 0.0;

        self.sector_count_register = 1;
        self.sector_number_register = 1;
        self.cylinder_low_register = 0;
        self.cylinder_high_register = 0;

        self.receiving_dcb = false;
        self.command = Command::None;
        self.command_fn = None;
        self.command_byte_n = 0;

        self.status_reads = 0;
        self.data_reads = 0;
        self.data_writes = 0;
        self.data_register = AtaRegister16::new();
    }

    pub fn drive_ct(&self) -> usize {
        self.drive_ct
    }

    pub fn get_supported_formats(&self) -> Vec<HardDiskFormat> {
        self.supported_formats.clone()
    }

    pub fn set_vhd(&mut self, device_id: usize, vhd: VirtualHardDisk) -> Result<(), ControllerError> {
        if device_id > 1 {
            return Err(ControllerError::InvalidDevice);
        }

        // Check that the VHD geometry is in the list of supported formats
        // (Currently there is only one supported format but that might change)
        let mut supported = false;
        for format in &self.supported_formats {
            if vhd.max_cylinders as u16 == format.geometry.c
                && vhd.max_heads as u8 == format.geometry.h
                && vhd.max_sectors as u8 == format.geometry.s
            {
                supported = true;
                break;
            }
        }

        if supported {
            // Note: The Xebec controller used 0-indexed sectors. ATA uses 1-indexed sectors.
            self.drives[device_id].set_geometry(DriveGeometry::new(
                vhd.max_cylinders as u16,
                vhd.max_heads as u8,
                vhd.max_sectors as u8,
                1,
                DEFAULT_SECTOR_SIZE,
            ));
            log::debug!(
                "Set drive {} VHD geometry of {}",
                device_id,
                self.drives[device_id].geometry()
            );
            self.drives[device_id].set_vhd(vhd);
        }
        else {
            return Err(ControllerError::UnsupportedVHD);
        }

        Ok(())
    }

    pub fn set_command(&mut self, command: Command, n_bytes: u32, command_fn: CommandDispatchFn) {
        self.state = State::ReceivingCommand;
        self.receiving_dcb = true;
        self.command = command;
        self.command_fn = Some(command_fn);
        self.command_byte_n = n_bytes;
    }

    fn drive_select(&self) -> usize {
        ((self.drive_head_register & 0x10) >> 4) as usize
    }

    fn set_command_chs(&mut self) {
        self.drive_select = self.drive_select();
        self.command_chs = DiskChs::new(
            self.cylinder_reg(),
            self.drive_head_register & 0x0F,
            self.sector_number_register,
        );
    }

    pub fn set_error(&mut self, error: OperationError, drive_select: usize) {
        self.last_error = error;
        self.last_error_drive = drive_select;

        match error {
            OperationError::NoError => self.error_flag = false,
            _ => self.error_flag = true,
        }
    }

    /// Handle a write to the Controller Select Pulse register
    pub fn handle_controller_select(&self, byte: u8) {
        // Byte written to pulse register ignored?
        // Not entirely sure the purpose of this register, but it may be used to coordinate multiple disk controllers
        log::trace!("Controller select: {:02X}", byte);
    }

    /// Read from the Data Register
    ///
    /// Sense Bytes can be read after a Request Sense command, or the Status Byte otherwise
    pub fn handle_data_register_read(&mut self) -> u8 {
        self.data_reads += 1;
        let mut byte = 0;

        if !self.status_register.drq() {
            log::warn!("Data Register read with DRQ not set");
            return 0;
        }

        let cursor_pos = self.sector_buffer.stream_position().unwrap();
        if cursor_pos < (SECTOR_SIZE as u64) {
            byte = self.sector_buffer.read_u8().unwrap_or_else(|e| {
                log::error!("Error reading from sector buffer: {e}");
                0
            });
            if cursor_pos == (SECTOR_SIZE as u64 - 1) {
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
    pub fn handle_data_register_write(&mut self, byte: u8, low: bool) {
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
            if cursor_pos < (SECTOR_SIZE as u64 - 1) {
                if let Err(e) = self.sector_buffer.write(&bytes) {
                    log::error!("Error writing to sector buffer: {e}");
                }
                if cursor_pos == (SECTOR_SIZE as u64 - 2) {
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
    pub fn handle_mask_register_write(&mut self, byte: u8) {
        self.irq_enabled = byte & ENABLE_IRQ_MASK != 0;
        self.dma_enabled = byte & ENABLE_DMA_MASK != 0;
        log::trace!(
            "Write to Mask Register. IRQ enabled: {} DMA enabled: {}",
            self.irq_enabled,
            self.dma_enabled
        );

        // Write to mask register puts us in Waiting For Command state
        self.state = State::WaitingForCommand;
    }

    /// Handle a write to the command register
    pub fn handle_command_register_write(&mut self, byte: u8, bus: &mut BusInterface) {
        log::warn!("Got command byte: {:02X}", byte);
        // Transition from other states. It's possible that we don't check the error code
        // after an operation
        if let State::HaveCommandStatus = self.state {
            log::warn!("Received command with pending unread status register");
            self.state = State::WaitingForCommand;
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
            State::WaitingForCommand => {
                if self.interrupt_active {
                    log::warn!(" >>> Received command with interrupt active")
                }

                match byte {
                    0x00 => {
                        log::debug!("NOP command received");
                    }
                    0x20 => {
                        log::debug!("Read Sector(s) (Retry) command received");
                        self.set_command(Command::ReadSectorRetry, 0, XtIdeController::command_read_sectors_retry);
                        self.process_command_byte(0, bus);
                    }
                    0x21 => {
                        log::debug!("Read Sector(s) command received");
                        self.set_command(Command::ReadSector, 0, XtIdeController::command_read_sectors);
                        self.process_command_byte(0, bus);
                    }
                    0x30 => {
                        log::debug!("Write Sector(s) command received");
                        self.set_command(Command::WriteSector, 0, XtIdeController::command_write_sectors);
                        self.process_command_byte(0, bus);
                    }
                    0x40 => {
                        log::debug!("Read Verify Sector(s) command received");
                        self.set_command(
                            Command::ReadVerifySector,
                            0,
                            XtIdeController::command_read_verify_sectors,
                        );
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
                        self.set_command(Command::IdentifyDrive, 0, XtIdeController::command_identify_drive);
                        self.process_command_byte(0, bus);
                    }
                    0xEF => {
                        log::debug!("Set Features command received");
                    }
                    0xC4 => {
                        log::debug!("Read Multiple received");
                        self.set_command(Command::ReadMultiple, 0, XtIdeController::command_read_multiple);
                        self.process_command_byte(0, bus);
                    }
                    0xC5 => {
                        log::debug!("Read Multiple received");
                        self.set_command(Command::WriteMultiple, 0, XtIdeController::command_read_multiple);
                        self.process_command_byte(0, bus);
                    }
                    0xC6 => {
                        log::debug!("Set Multiple Mode received");
                        self.set_command(Command::ReadMultipleMode, 0, XtIdeController::command_set_multiple_mode);
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
            State::ReceivingCommand => {
                // If we are expecting another byte for this command, read it in.
                self.process_command_byte(byte, bus);
            }
            _ => {
                log::error!("Unexpected write to command register.");
            }
        }
    }

    pub fn process_command_byte(&mut self, byte: u8, bus: &mut BusInterface) {
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
                    self.command = Command::None;
                    self.command_fn = None;
                    self.state = State::HaveCommandStatus;
                }
                Continuation::ContinueAsOperation => {
                    log::debug!("Command will continue as operation");
                    self.state = State::ExecutingCommand;
                }
            }
        }
    }

    pub fn handle_status_register_read(&mut self) -> u8 {
        //log::debug!("Status Register read: {:02X}", self.status_register.into_bytes()[0]);
        self.clear_interrupt = true;
        self.status_register.into_bytes()[0]
    }

    /// Return a boolean representing whether a virtual drive is mounted for the specified drive number
    fn drive_present(&mut self, drive_n: usize) -> bool {
        self.drives[drive_n].vhd.is_some()
    }

    fn cylinder_reg(&self) -> u16 {
        (self.cylinder_high_register as u16) << 8 | self.cylinder_low_register as u16
    }

    fn clear_buffer(&mut self) {
        self.sector_buffer.get_mut().fill(0);
        self.sector_buffer.seek(SeekFrom::Start(0)).unwrap();
    }

    pub fn sector_buffer_end(&mut self) -> bool {
        let pos = self.sector_buffer.stream_position().unwrap();
        pos > (SECTOR_SIZE as u64 - 1)
    }

    pub fn sector_buffer_start(&mut self) -> bool {
        let pos = self.sector_buffer.stream_position().unwrap();
        pos == 0
    }

    /// ATA command: Identify Drive
    fn command_identify_drive(&mut self, _bus: &mut BusInterface) -> Continuation {
        self.drive_select = self.drive_select();
        log::debug!("command_identify_drive()");
        log::debug!("sector buffer size: {}", self.sector_buffer.get_ref().len());
        // Set the DRQ flag

        // Normally the controller would set BSY while processing, but this happens instantaneously
        // here.
        //self.status_register.set_busy(true);
        if let Some(vhd) = &self.drives[self.drive_select].vhd {
            log::debug!(
                "Writing Drive Identification block to sector buffer with geometry: {:?}",
                vhd.geometry()
            );
            let id_blob = DriveIdentification::new(&vhd.geometry());
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
        //bus.pic_mut().as_mut().unwrap().request_interrupt(HDC_IRQ);
        Continuation::CommandComplete
    }

    /// ATA command: Set Multiple Mode
    fn command_set_multiple_mode(&mut self, _bus: &mut BusInterface) -> Continuation {
        log::debug!(
            "command_set_multiple_mode(): sectors_per_block: {} device: {}",
            self.sector_count_register,
            (self.drive_head_register & 0x10) >> 4
        );

        Continuation::CommandComplete
    }

    /// ATA command 0x21: Read Sector(s)
    fn command_read_sectors(&mut self, _bus: &mut BusInterface) -> Continuation {
        self.set_command_chs();
        log::debug!(
            "command_read_sectors(): drive: {}, sector_count: {} chs: {}",
            self.drive_select(),
            self.sector_count_register,
            self.command_chs,
        );

        self.drives[self.drive_select].seek(self.command_chs);
        self.read_sector_into_buffer(self.drive_select, false);

        if self.sector_count_register > 1 {
            self.operation_status[self.drive_select] = OperationStatus {
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
    fn command_read_sectors_retry(&mut self, _bus: &mut BusInterface) -> Continuation {
        self.set_command_chs();
        log::debug!(
            "command_read_sectors_retry(): drive: {}, sector_count: {} chs: {}",
            self.drive_select(),
            self.sector_count_register,
            self.command_chs,
        );

        self.drives[self.drive_select].seek(self.command_chs);
        self.read_sector_into_buffer(self.drive_select, true);

        if self.sector_count_register > 1 {
            self.operation_status[self.drive_select] = OperationStatus {
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
    fn command_read_verify_sectors(&mut self, _bus: &mut BusInterface) -> Continuation {
        self.set_command_chs();
        log::debug!(
            "command_read_sectors_verify(): drive: {}, sector_count: {} chs: {}",
            self.drive_select(),
            self.sector_count_register,
            self.command_chs,
        );

        self.drives[self.drive_select].seek(self.command_chs);
        //self.read_sector_into_buffer(self.drive_select, true);

        if self.sector_count_register > 1 {
            self.operation_status[self.drive_select] = OperationStatus {
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
    fn command_read_multiple(&mut self, _bus: &mut BusInterface) -> Continuation {
        self.set_command_chs();
        log::debug!(
            "command_read_multiple(): drive: {} sectors: {} lba: {} chs: {}",
            self.drive_select(),
            self.sector_count_register,
            (self.drive_head_register & 0x40) >> 6,
            self.command_chs,
        );

        self.drives[self.drive_select].seek(self.command_chs);
        self.read_sector_into_buffer(self.drive_select, true);

        if self.sector_count_register > 1 {
            self.operation_status[self.drive_select] = OperationStatus {
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
    fn command_write_sectors(&mut self, _bus: &mut BusInterface) -> Continuation {
        self.set_command_chs();
        log::debug!(
            "command_write_sectors(): drive: {}, sector_count: {} chs: {}",
            self.drive_select(),
            self.sector_count_register,
            self.command_chs,
        );

        self.operation_status[self.drive_select] = OperationStatus {
            sectors_complete: 0,
            sectors_left: self.sector_count_register,
            ..Default::default()
        };

        self.clear_buffer();
        self.status_register.set_drq(true);
        self.drives[self.drive_select].seek(self.command_chs);
        Continuation::ContinueAsOperation
    }

    /// ATA command 0xC5 Write Multiple
    fn command_write_multiple(&mut self, _bus: &mut BusInterface) -> Continuation {
        self.set_command_chs();
        log::debug!(
            "command_write_multiple(): drive: {}, sector_count: {} chs: {}",
            self.drive_select(),
            self.sector_count_register,
            self.command_chs,
        );

        self.operation_status[self.drive_select] = OperationStatus {
            sectors_complete: 0,
            sectors_left: self.sector_count_register,
            ..Default::default()
        };

        self.clear_buffer();
        self.status_register.set_drq(true);
        self.drives[self.drive_select].seek(self.command_chs);
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

    /// Perform the Read Sector Buffer command.
    ///
    fn command_read_sector_buffer(&mut self, bus: &mut BusInterface) -> Continuation {
        // Don't care about DBC bytes

        let xfer_size = bus.dma_mut().as_mut().unwrap().get_dma_transfer_size(HDC_DMA);
        if xfer_size != SECTOR_SIZE {
            log::warn!("Command ReadSectorBuffer: DMA word count != sector size");
        }
        self.operation_status[self.drive_select].dma_bytes_left = xfer_size;
        self.operation_status[self.drive_select].dma_byte_count = 0;

        log::trace!("Command ReadSectorBuffer: DMA xfer size: {}", xfer_size);

        self.state = State::ExecutingCommand;
        self.send_dreq = true;

        // Keep running until DMA transfer is complete
        Continuation::ContinueAsOperation
    }

    /// Perform the Write Sector Buffer command.
    ///
    fn command_write_sector_buffer(&mut self, bus: &mut BusInterface) -> Continuation {
        // Don't care about DBC bytes

        let xfer_size = bus.dma_mut().as_mut().unwrap().get_dma_transfer_size(HDC_DMA);
        if xfer_size != SECTOR_SIZE {
            log::warn!("Command WriteSectorBuffer: DMA word count != sector size");
        }
        self.operation_status[self.drive_select].dma_bytes_left = xfer_size;
        self.operation_status[self.drive_select].dma_byte_count = 0;

        log::trace!("Command WriteSectorBuffer: DMA xfer size: {}", xfer_size);

        self.state = State::ExecutingCommand;
        self.send_dreq = true;

        // Keep running until DMA transfer is complete
        Continuation::ContinueAsOperation
    }

    /// End a Command that utilized DMA service.
    fn end_dma_command(&mut self, _drive: u32, error: bool) {
        self.clear_dreq = true;
        self.operation_status[self.drive_select].dma_byte_count = 0;
        self.operation_status[self.drive_select].dma_bytes_left = 0;

        self.error_flag = error;
        self.send_interrupt = true;
        log::trace!("End of DMA command. Changing state to HaveCommandStatus");
        self.state = State::HaveCommandStatus;
    }

    fn end_operation(&mut self, drive_select: usize, _error: bool) {
        self.status_register.set_busy(false);
        self.operation_status[drive_select] = OperationStatus::default();
        self.state = State::HaveCommandStatus;
    }

    /// Read a sector from disk into the controller's sector buffer.
    fn read_sector_into_buffer(&mut self, drive_select: usize, _retry: bool) {
        //self.operation_status[self.drive_select].buffer_idx = 0;

        let pos = self.drives[drive_select].position_vhd();

        if let Some(vhd) = &mut self.drives[drive_select].vhd {
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
        else {
            log::error!("No VHD mounted for drive {}", drive_select);
        }
    }

    /// Write a sector to disk from the controller's sector buffer.
    fn write_sector_from_buffer(&mut self, drive_select: usize, _retry: bool) {
        //self.operation_status[self.drive_select].buffer_idx = 0;

        let pos = self.drives[drive_select].position_vhd();

        if let Some(vhd) = &mut self.drives[drive_select].vhd {
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
        else {
            log::error!("No VHD mounted for drive {}", drive_select);
        }
    }

    /// Process the Read Sector Buffer operation.
    /// This operation continues until the DMA transfer is complete.
    fn opearation_read_sector_buffer(&mut self, dma: &mut dma::DMAController, bus: &mut BusInterface) {
        if self.dreq_active && dma.read_dma_acknowledge(HDC_DMA) {
            if self.operation_status[self.drive_select].dma_bytes_left > 0 {
                let byte = 0;
                //let byte = self.drives[self.drive_select].sector_buf[self.operation_status.buffer_idx & 0x1FF];
                //self.operation_status[self.drive_select].buffer_idx += 1;
                // Bytes left to transfer
                dma.do_dma_write_u8(bus, HDC_DMA, byte);
                self.operation_status[self.drive_select].dma_byte_count += 1;
                self.operation_status[self.drive_select].dma_bytes_left -= 1;

                // See if we are done based on DMA controller
                let tc = dma.check_terminal_count(HDC_DMA);
                if tc {
                    log::trace!("DMA terminal count triggered end of ReadSectorBuffer command.");
                    if self.operation_status[self.drive_select].dma_bytes_left != 0 {
                        log::warn!(
                            "Incomplete DMA transfer on terminal count! Bytes remaining: {} count: {}",
                            self.operation_status[self.drive_select].dma_bytes_left,
                            self.operation_status[self.drive_select].dma_byte_count
                        );
                    }

                    log::trace!("Completed ReadSectorBuffer command.");
                    self.end_dma_command(0, false);
                }
            }
            else {
                // No more bytes left to transfer. Finalize operation
                let tc = dma.check_terminal_count(HDC_DMA);
                if !tc {
                    log::warn!("ReadSectorBuffer complete without DMA terminal count.");
                }

                log::trace!("Completed ReadSectorBuffer command.");
                self.end_dma_command(0, false);
            }
        }
        else if !self.dreq_active {
            log::error!("Error: WriteSectorBuffer command without DMA active!")
        }
    }

    /// Process the Write Sector Buffer operation.
    /// This operation continues until the DMA transfer is complete.
    fn opearation_write_sector_buffer(&mut self, dma: &mut dma::DMAController, bus: &mut BusInterface) {
        if self.dreq_active && dma.read_dma_acknowledge(HDC_DMA) {
            if self.operation_status[self.drive_select].dma_bytes_left > 0 {
                // Bytes left to transfer
                let _byte = dma.do_dma_read_u8(bus, HDC_DMA);
                self.operation_status[self.drive_select].dma_byte_count += 1;
                self.operation_status[self.drive_select].dma_bytes_left -= 1;

                // See if we are done based on DMA controller
                let tc = dma.check_terminal_count(HDC_DMA);
                if tc {
                    log::trace!("DMA terminal count triggered end of WriteSectorBuffer command.");
                    if self.operation_status[self.drive_select].dma_bytes_left != 0 {
                        log::warn!(
                            "Incomplete DMA transfer on terminal count! Bytes remaining: {} count: {}",
                            self.operation_status[self.drive_select].dma_bytes_left,
                            self.operation_status[self.drive_select].dma_byte_count
                        );
                    }

                    log::trace!("Completed WriteSectorBuffer command.");
                    self.end_dma_command(0, false);
                }
            }
            else {
                // No more bytes left to transfer. Finalize operation
                let tc = dma.check_terminal_count(HDC_DMA);
                if !tc {
                    log::warn!("WriteSectorBuffer complete without DMA terminal count.");
                }

                log::trace!("Completed WriteSectorBuffer command.");
                self.end_dma_command(0, false);
            }
        }
        else if !self.dreq_active {
            log::error!("Error: WriteSectorBuffer command without DMA active!")
        }
    }

    /// Process the Read Sector operation.
    /// This operation continues until the transfer is complete.
    fn operation_read_sector(
        &mut self,
        drive_select: usize,
        verify: bool,
        _dma: &mut dma::DMAController,
        _bus: &mut BusInterface,
    ) {
        // log::trace!(
        //     "in operation_read_sector(): drq: {} buffer empty: {}, sectors left: {}",
        //     self.status_register.drq(),
        //     self.sector_buffer_emtpy(),
        //     self.operation_status[drive_select].sectors_left
        // );
        // Wait until sector buffer is exhausted before reading next sector.
        if self.sector_buffer_end() {
            if self.operation_status[drive_select].sectors_left > 0 {
                // Advance to next sector
                if let Some(new_chs) = self.drives[drive_select].next_sector() {
                    log::debug!("operation_read_sector(): Reading sector: {}", new_chs);
                    self.drives[drive_select].seek(new_chs);
                }
                else {
                    log::warn!("operation_read_sector(): Failed to get next sector!");
                    self.end_operation(drive_select, true);
                    return;
                }

                if !verify {
                    self.read_sector_into_buffer(drive_select, false);
                }
                self.operation_status[drive_select].sectors_complete += 1;
                self.operation_status[drive_select].sectors_left -= 1;

                if self.operation_status[drive_select].sectors_left == 0 {
                    // Last sector read, finalize operation
                    log::debug!(
                        "operation_read_sector(): Completed command. Sectors read: {}",
                        self.operation_status[drive_select].sectors_complete
                    );
                    self.end_operation(drive_select, false);
                }
            }
            else {
                // No more sectors left to transfer. Finalize operation
                log::debug!(
                    "operation_read_sector(): Completed command. Sectors read: {}",
                    self.operation_status[drive_select].sectors_complete
                );
                self.end_operation(drive_select, false);
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

    fn operation_write_sector(&mut self, drive_select: usize, _dma: &mut dma::DMAController, _bus: &mut BusInterface) {
        // Wait until sector buffer is full before reading next sector.
        if self.sector_buffer_end() {
            if self.operation_status[drive_select].sectors_left > 0 {
                // Write current sector
                log::debug!(
                    "operation_write_sector(): Writing sector: {}",
                    self.drives[drive_select].position()
                );
                self.write_sector_from_buffer(drive_select, false);

                // Advance to next sector
                if let Some(new_chs) = self.drives[drive_select].next_sector() {
                    self.drives[drive_select].seek(new_chs);
                }
                else {
                    log::warn!("operation_write_sector(): Failed to seek to next sector!");
                    self.end_operation(drive_select, true);
                    return;
                }

                self.operation_status[drive_select].sectors_complete += 1;
                self.operation_status[drive_select].sectors_left -= 1;

                if self.operation_status[drive_select].sectors_left == 0 {
                    // Last sector read, finalize operation
                    log::debug!(
                        "operation_write_sector(): Completed command. Sectors written: {}",
                        self.operation_status[drive_select].sectors_complete
                    );
                    self.end_operation(drive_select, false);
                }
            }
            else {
                // No more sectors left to transfer. Finalize operation
                log::debug!(
                    "operation_write_sector(): Completed command. Sectors written: {}",
                    self.operation_status[drive_select].sectors_complete
                );
                self.end_operation(drive_select, false);
            }
        }
    }

    /// Run the HDC device.
    pub fn run(&mut self, dma: &mut dma::DMAController, bus: &mut BusInterface, us: f64) {
        // Handle interrupts
        if self.send_interrupt {
            if self.irq_enabled {
                //log::trace!(">>> Firing HDC IRQ 5");
                bus.pic_mut().as_mut().unwrap().request_interrupt(HDC_IRQ);
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
            bus.pic_mut().as_mut().unwrap().clear_interrupt(HDC_IRQ);
            self.clear_interrupt = false;
            self.interrupt_active = false;
        }

        if self.send_dreq {
            dma.request_service(HDC_DMA);
            self.send_dreq = false;
            self.dreq_active = true;
        }

        if self.clear_dreq {
            dma.clear_service(HDC_DMA);
            self.clear_dreq = false;
            self.dreq_active = false;
        }

        self.state_accumulator += us;

        // Process any running Operations
        match self.state {
            State::Reset => {
                // We need to remain in the reset state for a minimum amount of time before moving to
                // WaitingForCommand state. IBM BIOS/DOS does not check for this, but Minix does.
                if self.state_accumulator >= RESET_DELAY_US {
                    // TODO: We will still move into other states if a command is received. Should we refuse commands
                    //       until reset completes?
                    log::debug!("HDC Reset Complete, moving to WaitingForCommand");
                    self.state = State::WaitingForCommand;
                    self.state_accumulator = 0.0;
                }
            }
            State::ExecutingCommand => {
                match self.command {
                    Command::ReadSector | Command::ReadSectorRetry | Command::ReadMultiple => {
                        self.operation_read_sector(self.drive_select, false, dma, bus);
                    }
                    Command::ReadVerifySector => {
                        self.operation_read_sector(self.drive_select, true, dma, bus);
                    }
                    Command::WriteSector | Command::WriteMultiple => {
                        self.operation_write_sector(self.drive_select, dma, bus);
                    }
                    _ => {
                        log::warn!("Unhandled operation: {:?}", self.command);
                    }
                }
                // match self.command {
                //     Command::ReadSectorBuffer => {
                //         self.opearation_read_sector_buffer(dma, bus);
                //     }
                //     Command::WriteSectorBuffer => {
                //         self.opearation_write_sector_buffer(dma, bus);
                //     }
                //     Command::Read => {
                //         self.operation_read_sector(dma, bus);
                //     }
                //     Command::Write => {
                //         self.operation_write_sector(dma, bus);
                //     }
                //     _ => panic!("Unexpected command: {:?}", self.command),
                // }
            }
            _ => {
                // Unhandled state
            }
        }
    }
}

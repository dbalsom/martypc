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

//! An implementation of an JR-IDE controller for the IBM PCjr.

#![allow(dead_code)]

use std::{error::Error, fmt::Debug};

use crate::{
    bus::{BusInterface, DeviceRunTimeUnit, IoDevice},
    cpu_common::LogicAnalyzer,
    device_types::{geometry::DriveGeometry, hdc::HardDiskFormat},
    devices::{ata::ata_device::AtaDevice, hdc::at_formats::AtFormats},
    vhd::VirtualHardDisk,
};

use crate::{
    bus::{MemRangeDescriptor, MemoryMappedDevice},
    device_types::disk::Disk,
    devices::{ata::ata_error::AtaError, dma},
};
use core::fmt::Display;

pub const DRIVE_CT: usize = 2;

// Public consts
pub const DEFAULT_IO_BASE: u16 = 0x300;
pub const DEFAULT_ADDRESS_BASE: usize = 0xC0000;

pub const DATA_WINDOW_OFFSET: usize = 0x3A00;
pub const DATA_WINDOW_SIZE: usize = 0x200;
pub const CS0_REGISTER_FILE_OFFSET: usize = 0x3C00;
pub const CS1_REGISTER_FILE_OFFSET: usize = 0x3C08;
pub const FILL_ENABLE_REGISTER: usize = 0x3C10;
pub const WINDOW_ENABLE_REGISTER: usize = 0x3C11;
pub const SCRATCH_RAM: usize = 0x3C12;
pub const SCRATCH_RAM_SIZE: usize = 1006; // 3C12-3FFF
pub const MAP_SIZE: usize = 0x600;
pub const MAP_END_OFFSET: usize = 0x4000;

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

#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub enum OperationError {
    NoError,
    NoReadySignal,
    InvalidCommand,
    IllegalAccess,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub enum ControllerError {
    NoError,
    InvalidDevice,
    UnsupportedVHD,
    AtaError(AtaError),
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
            ControllerError::AtaError(ref err) => {
                write!(f, "ATA error: {}", err)
            }
        }
    }
}

impl IoDevice for JrIdeController {
    fn read_u8(&mut self, port: u16, _delta: DeviceRunTimeUnit) -> u8 {
        match port - self.io_base {
            HDC_DATA_REGISTER0 | HDC_DATA_REGISTER1 => {
                // let pos = self.sector_buffer.stream_position().unwrap();
                // if pos == (SECTOR_SIZE as u64 - 1) {
                //     log::debug!("{port:03X}: Sector buffer read complete #[{pos}]: {byte:02X}");
                // }
                self.data_register_read()
            }
            HDC_ERROR_REGISTER => self.error_register_read(),
            HDC_SECTOR_COUNT_REGISTER => self.sector_count_register_read(),
            HDC_SECTOR_NUMBER_REGISTER => self.sector_number_register_read(),
            HDC_CYLINDER_LOW_REGISTER => self.cylinder_low_register_read(),
            HDC_CYLINDER_HIGH_REGISTER => self.cylinder_high_register_read(),
            HDC_DRIVE_HEAD_REGISTER => self.drive_head_register_read(),
            HDC_STATUS_REGISTER => self.status_register_read(),
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
                self.data_register_write(data, port & 0x01 == 0);
            }
            HDC_DATA_REGISTER1 => {
                //log::trace!("{:03X}: Data register (high) write: {:02X}", port, data);
                self.data_register_write(data, port & 0x01 == 0);
            }
            HDC_ERROR_REGISTER => {
                log::warn!("{:03X}: Feature register write: {:02X}", port, data);
            }
            HDC_SECTOR_COUNT_REGISTER => {
                log::debug!("{:03X}: Sector count register write: {:02X}", port, data);
                self.sector_count_register_write(data);
            }
            HDC_SECTOR_NUMBER_REGISTER => {
                log::debug!("{:03X}: Sector number register write: {:02X}", port, data);
                self.sector_number_register_write(data);
            }
            HDC_CYLINDER_LOW_REGISTER => {
                log::debug!("{:03X}: Cylinder low register write: {:02X}", port, data);
                self.cylinder_low_register_write(data);
            }
            HDC_CYLINDER_HIGH_REGISTER => {
                log::debug!("{:03X}: Cylinder high register write: {:02X}", port, data);
                self.cylinder_high_register_write(data);
            }
            HDC_DRIVE_HEAD_REGISTER => {
                log::debug!("{:03X}: Drive/head register write: {:02X}", port, data);
                self.drive_head_register_write(data);
            }
            HDC_STATUS_REGISTER => {
                self.command_register_write(data, bus);
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
pub struct JrIdeController {
    io_base: u16,
    base_address: usize,
    drives: Box<[AtaDevice; DRIVE_CT]>,
    drive_ct: usize,
    drive_select: usize,
    supported_formats: Vec<HardDiskFormat>,
    drive_type_dip: u8,

    drive_head_register: u8,
    last_error: ControllerError,
    last_error_drive: usize,
    error_flag: bool,

    scratch_ram: Vec<u8>,
}

impl Default for JrIdeController {
    fn default() -> Self {
        let mut default_disks = Vec::new();
        // Loop because VHD isn't Clone
        for d_idx in 0..DRIVE_CT {
            default_disks.push(AtaDevice::new(
                d_idx,
                Disk::new(DriveGeometry::default()),
                None,
                true,
                None,
            ));
        }
        let disk_box = default_disks.into_boxed_slice();
        Self {
            io_base: DEFAULT_IO_BASE,
            base_address: 0xC0000,
            drives: disk_box.try_into().unwrap(),
            drive_ct: 1,
            drive_select: 0,
            supported_formats: AtFormats::vec(),
            drive_type_dip: 0,

            drive_head_register: 0,
            last_error: ControllerError::NoError,
            last_error_drive: 0,
            error_flag: false,

            scratch_ram: vec![0; SCRATCH_RAM_SIZE],
        }
    }
}

impl JrIdeController {
    pub fn new(io_base: Option<u16>, base_address: Option<usize>, drive_ct: usize) -> Self {
        Self {
            io_base: io_base.unwrap_or(DEFAULT_IO_BASE),
            base_address: base_address.unwrap_or(DEFAULT_ADDRESS_BASE),
            drive_ct,
            ..Default::default()
        }
    }

    pub fn reset(&mut self) {
        log::trace!("Resetting Hard Disk Controller...");
        for drive in self.drives.as_mut() {
            drive.reset();
        }
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
            self.drives[device_id].set_vhd(vhd).map_err(|e| {
                log::error!("Error setting VHD: {}", e);
                ControllerError::AtaError(e)
            })?;
            log::debug!("Set VHD!");
        }
        else {
            return Err(ControllerError::UnsupportedVHD);
        }

        Ok(())
    }

    fn unload_vhd(&mut self, device_id: usize) -> Result<(), ControllerError> {
        if device_id > 1 {
            return Err(ControllerError::InvalidDevice);
        }

        self.drives[device_id].unload_vhd();
        Ok(())
    }

    fn drive_head_register_write(&mut self, data: u8) {
        self.drive_head_register = data;
        let new_drive_select = ((data & 0x10) >> 4) as usize;

        if new_drive_select < DRIVE_CT {
            self.drive_select = new_drive_select;
        }
        else {
            log::error!("Drive select out of range: {new_drive_select}");
        }

        self.drives[self.drive_select].drive_head_register_write(data);
    }

    pub fn set_error(&mut self, error: ControllerError, drive_select: usize) {
        self.last_error = error;
        self.last_error_drive = drive_select;

        match error {
            ControllerError::NoError => self.error_flag = false,
            _ => self.error_flag = true,
        }
    }

    /// Handle a write to the Controller Select Pulse register
    pub fn controller_select(&self, byte: u8) {
        // Byte written to pulse register ignored?
        // Not entirely sure the purpose of this register, but it may be used to coordinate multiple disk controllers
        log::trace!("Controller select: {:02X}", byte);
    }

    fn data_register_read(&mut self) -> u8 {
        self.drives[self.drive_select].data_register_read()
    }
    fn error_register_read(&self) -> u8 {
        self.drives[self.drive_select].error_register_read()
    }
    fn sector_count_register_read(&self) -> u8 {
        self.drives[self.drive_select].sector_count_register_read()
    }
    fn sector_number_register_read(&self) -> u8 {
        self.drives[self.drive_select].sector_number_register_read()
    }
    fn cylinder_low_register_read(&self) -> u8 {
        self.drives[self.drive_select].cylinder_low_register_read()
    }
    fn cylinder_high_register_read(&self) -> u8 {
        self.drives[self.drive_select].cylinder_high_register_read()
    }
    fn drive_head_register_read(&self) -> u8 {
        self.drive_head_register
    }
    fn status_register_read(&mut self) -> u8 {
        self.drives[self.drive_select].status_register_read()
    }

    fn data_register_write(&mut self, byte: u8, low: bool) {
        self.drives[self.drive_select].data_register_write(byte, low);
    }
    fn sector_count_register_write(&mut self, byte: u8) {
        self.drives[self.drive_select].sector_count_register_write(byte)
    }
    fn sector_number_register_write(&mut self, byte: u8) {
        self.drives[self.drive_select].sector_number_register_write(byte)
    }
    fn cylinder_low_register_write(&mut self, byte: u8) {
        self.drives[self.drive_select].cylinder_low_register_write(byte)
    }
    fn cylinder_high_register_write(&mut self, byte: u8) {
        self.drives[self.drive_select].cylinder_high_register_write(byte)
    }

    fn command_register_write(&mut self, byte: u8, bus: Option<&mut BusInterface>) {
        self.drives[self.drive_select].handle_command_register_write(byte, bus)
    }

    /// Handle a write to the DMA and interrupt mask register
    pub fn mask_register_write(&mut self, byte: u8) {
        self.drives[self.drive_select].mask_register_write(byte)
    }

    /// Return a boolean representing whether a virtual drive is mounted for the specified drive number
    fn drive_present(&mut self, drive_n: usize) -> bool {
        self.drives[drive_n].disk().is_some()
    }

    /// Run the XT-IDE controller device.
    pub fn run(&mut self, dma: &mut dma::DMAController, bus: &mut BusInterface, us: f64) {
        for drive in self.drives.iter_mut() {
            drive.run(dma, bus, us);
        }
    }
}

impl MemoryMappedDevice for JrIdeController {
    fn get_read_wait(&mut self, _address: usize, _cycles: u32) -> u32 {
        0
    }

    fn mmio_read_u8(&mut self, address: usize, _cycles: u32, cpumem: Option<&[u8]>) -> (u8, u32) {
        let offset = address - self.base_address;
        match offset {
            0..DATA_WINDOW_OFFSET => (cpumem.unwrap()[address], 0),
            DATA_WINDOW_OFFSET..CS0_REGISTER_FILE_OFFSET => {
                let buffer_offset = offset - DATA_WINDOW_OFFSET;
                if buffer_offset < DATA_WINDOW_SIZE {
                    let byte = self.drives[0].sector_buffer()[buffer_offset];
                    log::trace!("Read from sector buffer: {offset:04X} [{byte:02X}]");
                    if buffer_offset == (DATA_WINDOW_SIZE - 1) {
                        log::debug!("Sector buffer read complete!");
                        self.drives[0].sector_buffer_mark_read();
                    }
                    (byte, 0)
                }
                else {
                    log::error!("Read from sector buffer out of range: {offset:04X}");
                    (0, 0)
                }
            }
            CS0_REGISTER_FILE_OFFSET..CS1_REGISTER_FILE_OFFSET => {
                let reg = offset & 0x07;
                let byte = self.drives[0].register_read(reg as u8);
                log::debug!("Read from CS0 register [{reg:X}]: {:02X}", byte);
                (byte, 0)
            }
            CS1_REGISTER_FILE_OFFSET..FILL_ENABLE_REGISTER => {
                let reg = offset & 0x07;
                log::debug!("Read from CS1 register [{reg:X}]");
                (0, 0)
            }
            FILL_ENABLE_REGISTER => {
                log::debug!("Read from fill enable register: {offset:04X}");
                (0, 0)
            }
            WINDOW_ENABLE_REGISTER => {
                log::debug!("Read from window enable register: {offset:04X}");
                (0, 0)
            }
            SCRATCH_RAM..MAP_END_OFFSET => {
                let ram_offset = offset - SCRATCH_RAM;
                if ram_offset < SCRATCH_RAM_SIZE {
                    let byte = self.scratch_ram[ram_offset];
                    log::debug!("Read from scratch RAM: {offset:04X} [{byte:02X}]",);
                    (byte, 0)
                }
                else {
                    log::error!("Read from scratch RAM out of range: {offset:04X}");
                    (0, 0)
                }
            }
            _ => {
                log::error!("Read from invalid address: {offset:04X}");
                (0, 0)
            }
        }
    }

    fn mmio_read_u16(&mut self, address: usize, _cycles: u32, _cpumem: Option<&[u8]>) -> (u16, u32) {
        log::warn!("Unsupported 16-bit read from address {address:04X}");
        (0, 0)
    }

    fn mmio_peek_u8(&self, address: usize, cpumem: Option<&[u8]>) -> u8 {
        let offset = address - self.base_address;
        match offset {
            ..DATA_WINDOW_OFFSET => cpumem.unwrap()[address],
            DATA_WINDOW_OFFSET..CS0_REGISTER_FILE_OFFSET => {
                let buffer_offset = offset - DATA_WINDOW_OFFSET;
                if buffer_offset < DATA_WINDOW_SIZE {
                    let byte = self.drives[0].sector_buffer()[buffer_offset];
                    byte
                }
                else {
                    log::error!("Read from sector buffer out of range: {offset:04X}");
                    0
                }
            }
            CS0_REGISTER_FILE_OFFSET..CS1_REGISTER_FILE_OFFSET => {
                let reg = offset & 0x07;
                log::debug!("Peek from CS0 register [{reg:X}]");
                0
            }
            CS1_REGISTER_FILE_OFFSET..FILL_ENABLE_REGISTER => {
                log::debug!("Peek from CS1 register file: {offset:04X}");
                0
            }
            FILL_ENABLE_REGISTER => {
                log::debug!("Peek from fill enable register: {offset:04X}");
                0
            }
            WINDOW_ENABLE_REGISTER => {
                log::debug!("Peek from window enable register: {offset:04X}");
                0
            }
            SCRATCH_RAM..MAP_END_OFFSET => {
                log::debug!("Peek from scratch RAM: {offset:04X}");
                0
            }
            _ => {
                log::error!("Peek from invalid address: {offset:04X}");
                0
            }
        }
    }

    fn mmio_peek_u16(&self, _address: usize, _cpumem: Option<&[u8]>) -> u16 {
        0
    }

    fn get_write_wait(&mut self, _address: usize, _cycles: u32) -> u32 {
        0
    }

    fn mmio_write_u8(&mut self, address: usize, data: u8, _cycles: u32, _cpumem: Option<&mut [u8]>) -> u32 {
        let offset = address - self.base_address;
        match offset {
            ..DATA_WINDOW_OFFSET => 0,
            DATA_WINDOW_OFFSET..CS0_REGISTER_FILE_OFFSET => {
                log::trace!("Write to sector buffer: {offset:04X}");

                let buffer_offset = offset - DATA_WINDOW_OFFSET;
                if buffer_offset < DATA_WINDOW_SIZE {
                    self.drives[0].sector_buffer_mut()[buffer_offset] = data;
                    log::trace!("Write to sector buffer: {offset:04X} [{data:02X}]");
                    if buffer_offset == (DATA_WINDOW_SIZE - 1) {
                        log::debug!("Sector buffer write complete!");
                        self.drives[0].sector_buffer_mark_written();
                    }
                    0
                }
                else {
                    log::error!("Write to sector buffer out of range: {offset:04X}");
                    0
                }
            }
            CS0_REGISTER_FILE_OFFSET..CS1_REGISTER_FILE_OFFSET => {
                let reg = offset & 0x07;
                log::debug!("Write to CS0 register {reg:X}: [{data:02X}]");
                self.drives[0].register_write(reg as u8, data, None);
                0
            }
            CS1_REGISTER_FILE_OFFSET..FILL_ENABLE_REGISTER => {
                log::debug!("Write to CS1 register file: {offset:04X}");
                0
            }
            FILL_ENABLE_REGISTER => {
                log::debug!("Write to fill enable register: {offset:04X}");
                0
            }
            WINDOW_ENABLE_REGISTER => {
                log::debug!("Write to window enable register: {offset:04X}");
                0
            }
            SCRATCH_RAM..MAP_END_OFFSET => {
                let ram_offset = offset - SCRATCH_RAM;
                if ram_offset < SCRATCH_RAM_SIZE {
                    log::debug!("Write to scratch RAM: {offset:04X} [{:02X}]", data);
                    self.scratch_ram[ram_offset] = data;
                }
                else {
                    log::error!("Write to scratch RAM out of range: {offset:04X}");
                }
                0
            }
            _ => {
                log::error!("Write to invalid address: {offset:04X}");
                0
            }
        }
    }

    fn mmio_write_u16(&mut self, _address: usize, _data: u16, _cycles: u32, _cpumem: Option<&mut [u8]>) -> u32 {
        0
    }

    fn get_mapping(&self) -> Vec<MemRangeDescriptor> {
        vec![MemRangeDescriptor {
            address: self.base_address,
            size: MAP_END_OFFSET,
            cycle_cost: 0,
            read_only: false,
            priority: 0,
        }]
    }
}

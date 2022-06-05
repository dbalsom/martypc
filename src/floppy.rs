/*
    floppy.rc
    Implement the NEC µPD764 Floppy Disk Controller

*/
#![allow(dead_code)]
use std::collections::{VecDeque, HashMap};
use lazy_static::lazy_static;

use crate::io::{IoDevice};
use crate::dma;
use crate::bus::{BusInterface};
use crate::pic;

pub const FDC_IRQ: u8 = 0x06;
pub const FDC_DMA: usize = 2;
pub const FDC_MAX_DRIVES: usize = 4;

pub const SECTOR_SIZE: usize = 512;

pub const FDC_DIGITAL_OUTPUT_REGISTER: u16 = 0x3F2;
pub const FDC_STATUS_REGISTER: u16 = 0x3F4;
pub const FDC_DATA_REGISTER: u16 = 0x3F5;

// Main Status Register Bit Definitions
// --------------------------------------------------------------------------------
// The first four bits encode which drives are in 'positioning' mode, ie whether
// they are moving their heads or being calibrated
pub const FDC_STATUS_FDD_A_BUSY: u8     = 0b0000_0001;
pub const FDC_STATUS_FDD_B_BUSY: u8     = 0b0000_0010;
pub const FDC_STATUS_FDD_C_BUSY: u8     = 0b0000_0100;
pub const FDC_STATUS_FDD_D_BUSY: u8     = 0b0000_1000;

// Busy bit seems to be on while there are bytes remaining to be read from 
// the Data register. The BIOS checks this bit to tell when it is done reading
// from the FDC data register.
pub const FDC_STATUS_FDC_BUSY: u8       = 0b0001_0000;
pub const FDC_STATUS_NON_DMA_MODE: u8   = 0b0010_0000;

// Direction bit is checked by BIOS to tell it if the FDC is expecting a read
// or a write to the Data register.  If this bit is set wrong the BIOS will 
// timeout waiting for it.
pub const FDC_STATUS_DIO: u8            = 0b0100_0000;

// MRQ (Main Request) is also used to determine if the data port is ready to be 
// written to or read. If this bit is not set the BIOS will timeout waiting for it.
pub const FDC_STATUS_MRQ: u8            = 0b1000_0000;

pub const DOR_DRIVE_SELECT_MASK: u8     = 0b0000_0001;
pub const DOR_DRIVE_SELECT_0: u8        = 0b0000_0000;
pub const DOR_DRIVE_SELECT_1: u8        = 0b0000_0001;
pub const DOR_DRIVE_SELECT_2: u8        = 0b0000_0010;
pub const DOR_DRIVE_SELECT_3: u8        = 0b0000_0011;
pub const DOR_FDC_RESET: u8             = 0b0000_0100;
pub const DOR_DMA_ENABLED: u8           = 0b0000_1000;
pub const DOR_MOTOR_FDD_A: u8           = 0b0001_0000;
pub const DOR_MOTOR_FDD_B: u8           = 0b0010_0000;
pub const DOR_MOTOR_FDD_C: u8           = 0b0100_0000;
pub const DOR_MOTOR_FDD_D: u8           = 0b1000_0000;

pub const COMMAND_MASK: u8                  = 0b0001_1111;
pub const COMMAND_READ_TRACK: u8            = 0x02;
pub const COMMAND_WRITE_SECTOR: u8          = 0x05;
pub const COMMAND_READ_SECTOR: u8           = 0x06;
pub const COMMAND_WRITE_DELETED_SECTOR: u8  = 0x09;
pub const COMMAND_READ_DELETED_SECTOR: u8   = 0x0C;
pub const COMMAND_FORMAT_TRACK: u8          = 0x0D;

pub const COMMAND_FIX_DRIVE_DATA: u8        = 0x03;
pub const COMMAND_CHECK_DRIVE_STATUS: u8    = 0x04;
pub const COMMAND_CALIBRATE_DRIVE: u8       = 0x07;
pub const COMMAND_CHECK_INT_STATUS: u8      = 0x08;
pub const COMMAND_READ_SECTOR_ID: u8        = 0x0A;
pub const COMMAND_SEEK_HEAD: u8             = 0x0F;

pub const ST0_HEAD_ACTIVE: u8   = 0b0000_0100;
pub const ST0_NOT_READY: u8     = 0b0000_1000;
pub const ST0_UNIT_CHECK: u8    = 0b0001_0000;
pub const ST0_SEEK_END: u8      = 0b0010_0000;
pub const ST0_RESET: u8         = 0b1100_0000;

pub struct DiskFormat {
    cylinders: u8,
    heads: u8,
    sectors: u8
}

lazy_static! {
    static ref DISK_FORMATS: HashMap<usize, DiskFormat> = {
        let map = HashMap::from([
            (
                163_840, 
                DiskFormat{
                    cylinders: 40,
                    heads: 1,
                    sectors: 8
                }
            ),(
                184_320,
                DiskFormat{
                    cylinders: 40,
                    heads: 1,
                    sectors: 9
                }
            ),(
                327_680,
                DiskFormat{
                    cylinders: 40,
                    heads: 2,
                    sectors: 8
                }
            ),( 
                368_640,
                DiskFormat{
                    cylinders: 40,
                    heads: 2,
                    sectors: 9
                }
            ),            
        ]);
        map
    };
}

pub enum IoMode {
    ToCpu,
    FromCpu
}

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
    CheckIntStatus,
    ReadSectorID,
    SeekParkHead,
    Invalid
}

pub enum Operation {
    NoOperation,
    ReadSector(u8, u8, u8, u8, u8, u8, u8) // cylinder, head, sector, sector_size, track_len, gap3_len, data_len
}
pub struct DiskDrive {
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
    disk_image: Vec<u8>
}
pub struct FloppyController {

    status_byte: u8,
    reset_flag: bool,
    mrq: bool,

    data_register: u8,
    dma: bool,
    dor: u8,
    busy: bool,
    dio: IoMode,
    reading_command: bool,
    command: Command,
    command_byte_n: u32,
    operation: Operation,
    send_interrupt: bool,
    end_interrupt: bool,
    in_command: bool,
    command_init: bool,

    data_register_out: VecDeque<u8>,
    data_register_in: VecDeque<u8>,

    drives: [DiskDrive; 4],    
    drive_select: usize,

    in_dma: bool,
    dma_byte_count: usize,
    dma_bytes_left: usize
}

impl IoDevice for FloppyController {

    fn read_u8(&mut self, port: u16) -> u8 {
        match port {
            FDC_DIGITAL_OUTPUT_REGISTER => {
                log::warn!("Read from Write-only DOR register");
                0
            },
            FDC_STATUS_REGISTER => {
                self.handle_status_register_read()
            },
            FDC_DATA_REGISTER => {
                self.handle_data_register_read()
            },
            _ => unreachable!("FLOPPY: Bad port #")
        }        
    }
    fn write_u8(&mut self, port: u16, data: u8) {
        match port {
            FDC_DIGITAL_OUTPUT_REGISTER => {
                self.handle_dor_write(data);
            },
            FDC_STATUS_REGISTER => {
                log::warn!("Write to Read-only status register");
            },
            FDC_DATA_REGISTER => {
                self.handle_data_register_write(data);
            },
            _ => unreachable!("FLOPPY: Bad port #")
        }    
    }    
    fn read_u16(&mut self, port: u16) -> u16 {
        match port {
            _ => unreachable!("FLOPPY: Bad port read")
        }
    }
    fn write_u16(&mut self, port: u16, data: u16) {
        match port {
            _ => unreachable!("FLOPPY: Bad port write")
        }
    }
}

impl FloppyController {
    pub fn new() -> Self {
        Self {
            status_byte: 0,
            reset_flag: false,
            mrq: true,
            data_register: 0,
            dma: true,
            dor: 0,
            busy: false,
            dio: IoMode::FromCpu,
            reading_command: false,
            command: Command::NoCommand,
            command_byte_n: 0,
            operation: Operation::NoOperation,
            send_interrupt: false,
            end_interrupt: false,
            in_command: false,
            command_init: false,

            data_register_out: VecDeque::new(),
            data_register_in: VecDeque::new(),
            drives: [
                DiskDrive {
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
                    disk_image: Vec::new(),
                },
                DiskDrive {
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
                    disk_image: Vec::new(),
                },
                DiskDrive {
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
                    disk_image: Vec::new(),
                },
                DiskDrive {
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
                    disk_image: Vec::new(),
                }
            ],
            drive_select: 0,
            
            in_dma: false,
            dma_byte_count: 0,
            dma_bytes_left: 0,
        }
    }

    /// Reset the Floppy Drive Controller
    pub fn reset(&mut self) {

        // We keep the currently loaded floppy image(s). After all, a reboot wouldn't 
        // eject your disks.
        self.status_byte = 0;
        self.drive_select = 0;
        self.reset_flag = true;

        self.data_register_out.clear();
        self.data_register_in.clear();

        for drive in &mut self.drives.iter_mut() {
            drive.head = 0;
            drive.cylinder = 0;
            drive.sector = 1;

            drive.ready = drive.have_disk;
            drive.motor_on = false;
            drive.positioning = false;
        }

        self.send_interrupt = false;
        self.end_interrupt = false;

        self.in_dma = false;
        self.dma_byte_count = 0;
        self.dma_bytes_left = 0;
    }

    /// Load a disk into the specified drive
    pub fn load_image_from(&mut self, drive_select: usize, src_vec: Vec<u8>) -> Result<(), &'static str>  {
        
        if drive_select >= FDC_MAX_DRIVES {
            return Err("Invalid drive selection");
        }

        let image_len: usize = src_vec.len();

        // Disk images must contain whole sectors
        if image_len % SECTOR_SIZE > 0 {
            return Err("Invalid image length")
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
                return Err("Invalid image length")
            }
        }

        self.drives[drive_select].have_disk = true;
        self.drives[drive_select].disk_image = src_vec;
        log::debug!("Loaded floppy image, size: {} c: {} h: {} s: {}", 
            self.drives[drive_select].disk_image.len(),
            self.drives[drive_select].max_cylinders,
            self.drives[drive_select].max_heads,
            self.drives[drive_select].max_sectors
        );

        Ok(())
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
        msr_byte |= FDC_STATUS_MRQ;

        //log::trace!("Status Register Read: {:02X}", msr_byte);
        msr_byte
    }

    pub fn motor_on(&mut self, drive_select: usize) {
        if self.drives[drive_select].have_disk {
            self.drives[drive_select].motor_on = true;
            self.drives[drive_select].ready = true;
        }
    }

    pub fn handle_dor_write(&mut self, data: u8) {

        if data & DOR_FDC_RESET == 0 {
            // Reset when reset bit is *not* set
            // ignore all other commands
            log::debug!("FDC Reset requested: {:02X}", data);
            self.reset();
            self.send_interrupt = true;
            return
        }

        let disk_n = data & 0x03;
        if data & DOR_MOTOR_FDD_A != 0 {
            self.motor_on(0);
        }
        if data & DOR_MOTOR_FDD_B != 0 {
            self.motor_on(1);
        }
        if data & DOR_MOTOR_FDD_C != 0 {
            self.motor_on(2);
        }
        if data & DOR_MOTOR_FDD_D != 0 {
            self.motor_on(3);
        }

        if !self.drives[disk_n as usize].motor_on {
            // It's valid to issue this command without turning a motor on. In this case the FDC can
            // be enabled, but no drive is selected.
        }
        else {
            log::debug!("Drive {} selected, motor on", disk_n);
            self.drive_select = disk_n as usize;
            self.drives[disk_n as usize].motor_on = true;
        }

        self.dor = data;
    }

    pub fn make_st0_byte(&mut self, seek_end: bool) -> u8 {

        let mut st0: u8 = 0;

        // Set selected drive bits
        st0 |= (self.drive_select as u8) & 0x03;

        // Set active head bit
        if self.drives[self.drive_select].head == 1 {
            st0 |= ST0_HEAD_ACTIVE;
        }

        // Set ready bit
        if !self.drives[self.drive_select].ready {
            st0 |= ST0_NOT_READY;
        }

        // Set seek bit
        if seek_end {
            st0 |= ST0_SEEK_END;
        }

        // Highest two bits are set after a reset procedure
        if self.reset_flag {
            st0 |= ST0_RESET;
            self.reset_flag = false;
        }

        st0
    }

    pub fn make_st1_byte(&mut self) -> u8 {
        // The ST1 status register contains mostly error codes, so for now we can just always return success
        // by returning 0, until we handle possible errors.
        0
    }

    pub fn make_st2_byte(&mut self) -> u8 {
        // The ST2 status register contains mostly error codes, so for now we can just always return success
        // by returning 0 until we handle possible errors.
        0
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

    pub fn set_command(&mut self, command: Command, n_bytes: u32) {
        self.in_command = true;
        self.command = command;
        self.command_byte_n = n_bytes;
    }

    pub fn send_data_register(&mut self) {
        self.busy = true;
        self.dio = IoMode::ToCpu;
        self.mrq = true;
    }

    pub fn handle_data_register_write(&mut self, data: u8) {
        //log::trace!("Data Register Write");
        if !self.in_command { 
            let command = data & COMMAND_MASK;

            match command {
                COMMAND_READ_TRACK => {
                    log::trace!("Received Read Track command: {:02}", command);
                }
                COMMAND_WRITE_SECTOR => {
                    log::trace!("Received Write Sector command: {:02}", command);
                }
                COMMAND_READ_SECTOR => {
                    log::trace!("Received Read Sector command: {:02}", command);
                    self.set_command(Command::ReadSector, 8);
                }
                COMMAND_WRITE_DELETED_SECTOR => {
                    log::trace!("Received Write Deleted Sector command: {:02}", command);
                }
                COMMAND_READ_DELETED_SECTOR => {
                    log::trace!("Received Read Deleted Sector command: {:02}", command);
                }
                COMMAND_FORMAT_TRACK => {
                    log::trace!("Received Format Track command: {:02}", command);
                }
                COMMAND_FIX_DRIVE_DATA => {
                    log::trace!("Received Fix Drive Data command: {:02}", command);
                    self.set_command(Command::FixDriveData, 2);
                }
                COMMAND_CHECK_DRIVE_STATUS => {
                    log::trace!("Received Check Drive Status command: {:02}", command);
                }
                COMMAND_CALIBRATE_DRIVE => {
                    log::trace!("Received Calibrate Drive command: {:02}", command);
                    self.set_command(Command::CalibrateDrive, 1);
                }
                COMMAND_CHECK_INT_STATUS => {
                    log::trace!("Received Check Interrupt Status command: {:02}", command);
                    // Queue response bytes
                    
                    self.do_sense_interrupt();

                }
                COMMAND_READ_SECTOR_ID => {
                    log::trace!("Received Read Sector ID command: {:02}", command);
                }
                COMMAND_SEEK_HEAD => {
                    log::trace!("Received Seek/Park Head command: {:02}", command);
                    self.set_command(Command::SeekParkHead, 2);
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
                    let mut result = false;
                    match self.command {
                        Command::ReadTrack => {
                        }
                        Command::WriteSector => {
                        }
                        Command::ReadSector => {
                            result = self.command_read_sector();
                        }
                        Command::WriteDeletedSector => {
                        }
                        Command::ReadDeletedSector => {
                        }
                        Command::FormatTrack => {
                        }
                        Command::FixDriveData => {
                            result = self.command_fix_drive_data();
                        }
                        Command::CheckDriveStatus => {
                        }
                        Command::CalibrateDrive => {
                            result = self.command_calibrate_drive();
                        }
                        Command::CheckIntStatus => {
                        }
                        Command::ReadSectorID => {
                            result = self.command_fix_drive_data();
                        }
                        Command::SeekParkHead => {
                            result = self.command_seek_head();
                        }    
                        _ => {
                            log::error!("FDC in invalid state!");
                        }  
                    }

                    // Clear command vec 
                    self.data_register_in.clear();
                    self.in_command = false;

                    // Clear command if complete
                    if result {
                        self.command = Command::NoCommand;
                    }
                }
            }
        }
    }    

    pub fn do_sense_interrupt(&mut self) {
        
        // Release IR line
        self.end_interrupt = true;

        // Send ST0 register to FIFO
        let cb0 = self.make_st0_byte(false);
        self.data_register_out.push_back(cb0);

        // Send Current Cylinder to FIFO
        let cb1 = self.drives[self.drive_select].cylinder;
        self.data_register_out.push_back(cb1);
        
        // We have data for CPU to read
        self.send_data_register();
    }

    pub fn command_fix_drive_data(&mut self) -> bool {
        // We don't really do anything with these values
        let steprate_unload = self.data_register_in.pop_front().unwrap();
        let headload_ndm = self.data_register_in.pop_front().unwrap();

        log::trace!("command_fix_drive_data completed: {:08b},{:08b}", steprate_unload, headload_ndm);
        return true
    }
    
    /// Resets the drive head to cylinder 0. 
    pub fn command_calibrate_drive(&mut self) -> bool{
        
        // A real floppy drive might fail to seek completely to cylinder 0 with one calibrate command. 
        // Any point to emulating this behavior?
        let drive_head_select = self.data_register_in.pop_front().unwrap();
        let drive_select = (drive_head_select & 0x03) as usize;
        let head_select = drive_head_select >> 2 & 0x01;

        // Set CHS
        self.drives[drive_select].cylinder = 0;
        self.drives[drive_select].head = head_select;
        self.drives[drive_select].sector = 1;
        
        // Calibrate command sends interrupt when complete
        self.send_interrupt = true;

        log::trace!("command_calibrate_drive completed: {:02b}", drive_select);
        return true
    }

    pub fn command_seek_head(&mut self) -> bool {
        // A real floppy drive would take some time to seek
        // Not sure how to go about determining proper timings. For now, seek instantly

        let drive_head_select = self.data_register_in.pop_front().unwrap();
        let cylinder = self.data_register_in.pop_front().unwrap();
        let drive_select = (drive_head_select & 0x03) as usize;
        let head_select = (drive_head_select >> 2) & 0x01;

        // Set CHS
        self.drives[drive_select].cylinder = cylinder;
        self.drives[drive_select].head = head_select;
        // We can only seek to the start of a cylinder, so set to first sector
        self.drives[drive_select].sector = 1;

        // Seek command sends interrupt when complete
        self.send_interrupt = true;

        log::trace!("command_seek_head completed: {:03b} cylinder: {:02X}", drive_head_select, cylinder);
        return true
    }

    pub fn command_read_sector(&mut self) -> bool {
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
            log::warn!("command_read_sector: non-matching head specifiers");
        }

        // Set CHS
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

        // Maximum size of DMA transfer

        let max_sectors;
        if track_len > 0 {
            max_sectors = track_len - sector + 1;
        }
        else {
            max_sectors = 1;
        }
        self.dma_bytes_left = max_sectors as usize * SECTOR_SIZE;

        log::trace!("command_read_sector: cyl:{:01X} head:{:01X} sector:{:02X} sector_size:{:02X} track_len:{:02X} gap3_len:{:02X} data_len:{:02X}",
            cylinder, head, sector, sector_size, track_len, gap3_len, data_len);
        log::trace!("command_read_sector: may operate on maximum of {} sectors", max_sectors);

        let base_address = self.get_image_address(self.drive_select, cylinder, head, sector);
        log::trace!("command_read_sector: base address of image read: {:06X}", base_address);

        // Flag to set up transfer size later
        self.command_init = false;
        // Keep running command until DMA transfer completes
        return false
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

    pub fn get_next_sector(&self, cylinder: u8, head: u8, sector: u8) -> (u8, u8, u8) {
        if sector < 8 {
            return (cylinder, head, sector + 1)
        }
        else if cylinder < 39 {
            return (cylinder + 1, head, 0)
        }
        else if head < 1 {
            return (0, 1, 0)
        }
        else {
            // Return end of drive? What does this do on real hardware
            return (39, 1, 8)
        }
    }

    /// Run the Floppy Drive Controller. Process running Operations.
    pub fn run(&mut self, pic: &mut pic::Pic, dma: &mut dma::DMAController, bus: &mut BusInterface, cpu_cycles: u32 ) {

        // Send an interrupt if one is queued
        if self.send_interrupt {
            pic.request_interrupt(FDC_IRQ);
            self.send_interrupt = false;
        }

        // End an interrupt if one was handled
        if self.end_interrupt {
            pic.clear_interrupt(FDC_IRQ);
            self.end_interrupt = false;
        }

        // Run operation
        match self.operation {
            Operation::ReadSector(cylinder, head, sector, sector_size, track_len, gap3_len, data_len) => {
                if !self.in_dma {
                    log::error!("FDC in invalid state: ReadSector operation without DMA! Aborting.");
                    self.operation = Operation::NoOperation;
                    return
                }

                if !self.command_init {
                    let xfer_size = dma.get_dma_transfer_size(FDC_DMA);

                    if xfer_size % SECTOR_SIZE != 0 {
                        log::warn!("DMA word count not multiple of sector size");
                    }

                    let xfer_sectors = xfer_size / SECTOR_SIZE;
                    log::trace!("DMA programmed for transfer of {} sectors", xfer_sectors);

                    self.dma_bytes_left = xfer_sectors * SECTOR_SIZE;
                    self.command_init = true;
                }

                if self.dma_bytes_left == SECTOR_SIZE {
                    let dst_address = dma.get_dma_transfer_address(FDC_DMA);
                    log::trace!("DMA destination address: {:05X}", dst_address)
                }

                if self.dma_bytes_left > 0 {
                    // Bytes left to transfer

                    // Check if DMA is ready
                    if dma.check_dma_ready(FDC_DMA) {
                        let base_address = self.get_image_address(self.drive_select, cylinder, head, sector);
                        let byte_address = base_address + self.dma_byte_count;

                        //log::trace!("Byte address for FDC read: {:04X}", byte_address);
                        if byte_address >= self.drives[self.drive_select].disk_image.len() {
                            log::error!("Read past end of disk image: {}/{}!", byte_address, self.drives[self.drive_select].disk_image.len() );
                            self.dma_bytes_left = 0;
                        }
                        else {
                            let byte = self.drives[self.drive_select].disk_image[byte_address];

                            dma.do_dma_transfer_u8(bus, FDC_DMA, byte);
                            self.dma_byte_count += 1;
                            self.dma_bytes_left -= 1;

                            // See if we are done
                            let tc = dma.check_terminal_count(FDC_DMA);
                            if tc {
                                log::trace!("DMA terminal count triggered end of Sector Read operation.");
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

                    let (c, h, s) = self.get_next_sector(cylinder, head, sector);

                    // Set CHS
                    self.drives[self.drive_select].cylinder = cylinder;
                    self.drives[self.drive_select].head = head;
                    self.drives[self.drive_select].sector = sector;

                    let st0_byte = self.make_st0_byte(false);
                    let st1_byte = self.make_st1_byte();
                    let st2_byte = self.make_st2_byte();

                    // Push result codes into FIFO
                    self.data_register_out.clear();
                    self.data_register_out.push_back(st0_byte);
                    self.data_register_out.push_back(st1_byte);
                    self.data_register_out.push_back(st2_byte);

                    self.data_register_out.push_back(c);
                    self.data_register_out.push_back(h);
                    self.data_register_out.push_back(s);
                    self.data_register_out.push_back(sector_size);
                
                    // Finalize operation
                    self.send_data_register();
                    self.operation = Operation::NoOperation;
                    pic.request_interrupt(FDC_IRQ);
                }
            }
            _ => {
                // Do nothing
            }
        }
    }
}
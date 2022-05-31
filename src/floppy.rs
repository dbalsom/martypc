/*
    floppy.rc
    Implement the NEC µPD764 Floppy Disk Controller

*/

use crate::io::{IoDevice};

pub const FDC_DIGITAL_OUTPUT_REGISTER: u16 = 0x3F2;
pub const FDC_STATUS_REGISTER: u16 = 0x3F4;
pub const FDC_DATA_REGISTER: u16 = 0x3F5;

pub const FDC_STATUS_FDD_A_BUSY: u8     = 0b0000_0001;
pub const FDC_STATUS_FDD_B_BUSY: u8     = 0b0000_0010;
pub const FDC_STATUS_FDD_C_BUSY: u8     = 0b0000_0100;
pub const FDC_STATUS_FDD_D_BUSY: u8     = 0b0000_1000;
pub const FDC_STATUS_FDC_BUSY: u8       = 0b0001_0000;
pub const FDC_STATUS_NON_DMA_MODE: u8   = 0b0010_0000;
pub const FDC_STATUS_DIO: u8            = 0b0100_0000;
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
pub struct DiskDrive {
    motor_on: bool,
    positioning: bool,
}
pub struct FloppyController {

    status_byte: u8,
    drive_select: usize,
    data_register: u8,
    dma: bool,
    dor: u8,
    busy: bool,
    dio: IoMode,
    doing_command: bool,
    command: Command,
    command_byte_n: u32,
    drives: [DiskDrive; 4],

    
    disk_image: Vec<u8>
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
            data_register: 0,
            dma: true,
            dor: 0,
            busy: false,
            dio: IoMode::ToCpu,
            doing_command: false,
            command: Command::NoCommand,
            command_byte_n: 0,
            drives: [
                DiskDrive {
                    motor_on: false,
                    positioning: false,
                },
                DiskDrive {
                    motor_on: false,
                    positioning: false,
                },
                DiskDrive {
                    motor_on: false,
                    positioning: false,
                },
                DiskDrive {
                    motor_on: false,
                    positioning: false,
                }
            ],
            drive_select: 0,
            disk_image: Vec::new()
        }
    }

    pub fn reset(&mut self) {
        self.status_byte = 0;
        self.drive_select = 0;
    }

    pub fn load_image_from(&mut self, src_vec: Vec<u8>) {
        self.disk_image = src_vec;
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
        
        // DIO bit => 0=CPU->FDC 1=FDC->CPU
        msr_byte |= match self.dio {
            IoMode::ToCpu => 0,
            IoMode::FromCpu => 1
        };

        // MRQ => Ready to receive or send data or commands via the data register
        // set this always on for now
        msr_byte |= FDC_STATUS_MRQ;

        log::trace!("Status Register Read: {:02X}", msr_byte);
        msr_byte
    }

    pub fn handle_data_register_read(&mut self) -> u8 {
        log::trace!("FLOPPY: Data Register Read");
        0
    }

    pub fn handle_dor_write(&mut self, data: u8) {

        if data & DOR_FDC_RESET == 0 {
            // Reset when reset bit is *not* set
            // ignore all other commands
            log::debug!("FDC Reset requested: {:02X}", data);
            self.reset();
            return
        }

        let disk_n = data & 0x03;
        self.drives[0].motor_on = data & DOR_MOTOR_FDD_A != 0;
        self.drives[1].motor_on = data & DOR_MOTOR_FDD_B != 0;
        self.drives[2].motor_on = data & DOR_MOTOR_FDD_C != 0;
        self.drives[3].motor_on = data & DOR_MOTOR_FDD_D != 0;    

        if !self.drives[disk_n as usize].motor_on {
            //log::warn!("FDD selected without motor on: {:02X}", data);
        }
        else {
            log::debug!("Drive {} selected, motor on", disk_n);
            self.drive_select = disk_n as usize;
        }

        self.dor = data;
    }

    pub fn handle_data_register_write(&mut self, data: u8) {
        log::trace!("FLOPPY: Data Register Write");
    }    
}
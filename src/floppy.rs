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
pub const FDC_STATUS_RQM: u8            = 0b1000_0000;

pub const DOR_DRIVE_SELECT_MASK: u8     = 0b0000_0001;
pub const DOR_DRIVE_SELECT_0: u8        = 0b0000_0000;
pub const DOR_DRIVE_SELECT_1: u8        = 0b0000_0001;
pub const DOR_DRIVE_SELECT_2: u8        = 0b0000_0010;
pub const DOR_DRIVE_SELECT_3: u8        = 0b0000_0011;

pub struct FloppyController {

    status_byte: u8
}

impl IoDevice for FloppyController {

    fn read_u8(&mut self, port: u16) -> u8 {
        match port {
            FDC_DIGITAL_OUTPUT_REGISTER => {
                self.handle_digital_register_read()
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
                self.handle_digital_register_write(data);
            },
            FDC_STATUS_REGISTER => {
                self.handle_status_register_write(data);
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
            status_byte: 0
        }
    }

    pub fn handle_digital_register_read(&mut self) -> u8 {
        log::trace!("FLOPPY: Digital Register Read");
        0
    }
    pub fn handle_status_register_read(&mut self) -> u8 {
        log::trace!("FLOPPY: Status Register Read");
        0
    }
    pub fn handle_data_register_read(&mut self) -> u8 {
        log::trace!("FLOPPY: Data Register Read");
        0
    }
    pub fn handle_digital_register_write(&mut self, data: u8) {
        log::trace!("FLOPPY: Digitial Register Write");

    }
    pub fn handle_status_register_write(&mut self, data: u8) {
        log::trace!("FLOPPY: Status Register Write");

    }
    pub fn handle_data_register_write(&mut self, data: u8) {
        log::trace!("FLOPPY: Data Register Write");
    }    
}
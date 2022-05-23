/*

    Implements the Intel 8237 DMA Controller


*/


use crate::io::{IoBusInterface, IoDevice};
use log::debug;

pub const DMA_CONTROL_PORT: u16 = 0x08;

// Control byte bit fields
pub const DMA_CONTROL_DISABLE: u8 = 0x04;

pub enum ServiceMode {
    SingleTransfer,
    BlockTransfer,
    DemandTransfer,
    Cascade
}

pub enum TransferType {
    Read,
    Write,
    Verify
}

pub struct DMAChannel {
    current_address_reg: u16,
    current_word_count_reg: u16,
    base_address_reg: u16,
    base_word_count: u16,
    mode_reg: u8,
}

pub struct DMAController {
    enabled: bool,
    channel: [DMAChannel; 4],
    
    command_register: u8,
    request_reg: u8,
    mask_reg: u8,
    status_reg: u8,
    temp_reg: u8

}

impl IoDevice for DMAController {
    fn read_u8(&mut self, port: u16) -> u8 {
        0
    }
    fn write_u8(&mut self, port: u16, data: u8) {
        if port == DMA_CONTROL_PORT {
            // Write to DMA Control port
            let control_byte = data;
            if (control_byte & DMA_CONTROL_DISABLE != 0) && self.enabled {
                debug!("DMA: Disabling DMA controller");
                self.enabled = false
            }            
        }
    }
    fn read_u16(&mut self, port: u16) -> u16 {
        0
    }
    fn write_u16(&mut self, port: u16, data: u16) {

    }
}

impl DMAController {
    pub fn new() -> Self {

        Self {
            enabled: true
        }
    }

    pub fn run(&mut self, io_bus: &mut IoBusInterface) {


    }
}
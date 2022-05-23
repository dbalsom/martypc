use crate::io::{IoBusInterface, IoDevice};
use log::debug;

pub const DMA_CONTROL_PORT: u16 = 0x08;

// Control byte bit fields
pub const DMA_CONTROL_DISABLE: u8 = 0x04;

pub struct DMAController {
    enabled: bool
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
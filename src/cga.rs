#![allow(dead_code)]
use log;
use crate::io::{IoBusInterface, IoDevice};

pub const CGA_MODE_CONTROL_REGISTER: u16 = 0x3D8;

const MODE_HIRES_TEXT: u8       = 0b0000_0001;
const MODE_GRAPHICS: u8         = 0b0000_0010;
const MODE_BW: u8               = 0b0000_0100;
const MODE_ENABLE: u8           = 0b0000_1000;
const MODE_HIRES_GRAPHICS: u8   = 0b0001_0000;
const MODE_BLINKING: u8         = 0b0010_0000;

pub enum Resolution {
    Res640by200,
    Res320by200
}

pub enum BitDepth {
    Depth1,
    Depth2,
    Depth4,
}

pub struct CGACard {

    mode_byte: u8,
    mode_enable: bool,
    mode_graphics: bool,
    mode_bw: bool,
    mode_hires: bool,
    mode_blinking: bool,
}


impl IoDevice for CGACard {
    fn read_u8(&mut self, port: u16) -> u8 {
        0
    }
    fn write_u8(&mut self, port: u16, data: u8) {
        if let CGA_MODE_CONTROL_REGISTER = port {
            self.handle_mode_register(data);
        }
    }
    fn read_u16(&mut self, port: u16) -> u16 {
        log::error!("Invalid 16-bit read from CGA");
        0   
    }
    fn write_u16(&mut self, port: u16, data: u16) {
        log::error!("Invalid 16-bit write to CGA");
    }
}

impl CGACard {

    pub fn new() -> Self {
        Self {
            mode_byte: 0,
            mode_enable: true,
            mode_graphics: false,
            mode_bw: false,
            mode_hires: true,
            mode_blinking: true
        }
    }

    pub fn handle_mode_register(&mut self, mode_byte: u8) {

        self.mode_hires = mode_byte & MODE_HIRES_TEXT != 0;
        self.mode_graphics = mode_byte & MODE_GRAPHICS != 0;
        self.mode_bw = mode_byte & MODE_BW != 0;
        self.mode_enable = mode_byte & MODE_ENABLE != 0;
        self.mode_hires = mode_byte & MODE_HIRES_GRAPHICS != 0;
        self.mode_blinking = mode_byte & MODE_BLINKING != 0;
        self.mode_byte = mode_byte;
        log::debug!("CGA: Mode Selected ({:02X}) Enabled: {} Graphics mode: {} HiRes: {}", 
            mode_byte, 
            self.mode_enable,
            self.mode_graphics, 
            self.mode_hires );
    }
}
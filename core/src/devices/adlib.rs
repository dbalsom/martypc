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

    devices::adlib.rs

    Implement the AdLib sound card.

*/

pub const REGS: [[u8; 2]; 166] = [
    [0x04, 0x60],
    [0x04, 0x80],
    [0x02, 0xFF],
    [0x04, 0x21],
    [0x04, 0x60],
    [0x04, 0x80],
    [0x01, 0x00],
    [0x02, 0x00],
    [0x03, 0x00],
    [0x04, 0x00],
    [0x05, 0x00],
    [0x06, 0x00],
    [0x07, 0x00],
    [0x08, 0x00],
    [0x09, 0x00],
    [0x0A, 0x00],
    [0x0B, 0x00],
    [0x0C, 0x00],
    [0x0D, 0x00],
    [0x0E, 0x00],
    [0x0F, 0x00],
    [0x10, 0x00],
    [0x11, 0x00],
    [0x12, 0x00],
    [0x13, 0x00],
    [0x14, 0x00],
    [0x15, 0x00],
    [0x16, 0x00],
    [0x17, 0x00],
    [0x18, 0x00],
    [0x19, 0x00],
    [0x1A, 0x00],
    [0x1B, 0x00],
    [0x1C, 0x00],
    [0x1D, 0x00],
    [0x1E, 0x00],
    [0x1F, 0x00],
    [0x20, 0x00],
    [0x21, 0x00],
    [0x22, 0x00],
    [0x23, 0x00],
    [0x24, 0x00],
    [0x25, 0x00],
    [0x26, 0x00],
    [0x27, 0x00],
    [0x28, 0x00],
    [0x29, 0x00],
    [0x2A, 0x00],
    [0x2B, 0x00],
    [0x2C, 0x00],
    [0x2D, 0x00],
    [0x2E, 0x00],
    [0x2F, 0x00],
    [0x30, 0x00],
    [0x31, 0x00],
    [0x32, 0x00],
    [0x33, 0x00],
    [0x34, 0x00],
    [0x35, 0x00],
    [0x36, 0x00],
    [0x37, 0x00],
    [0x38, 0x00],
    [0x39, 0x00],
    [0x3A, 0x00],
    [0x3B, 0x00],
    [0x3C, 0x00],
    [0x3D, 0x00],
    [0x3E, 0x00],
    [0x3F, 0x00],
    [0x40, 0x00],
    [0x41, 0x00],
    [0x42, 0x00],
    [0x43, 0x00],
    [0x44, 0x00],
    [0x45, 0x00],
    [0x46, 0x00],
    [0x47, 0x00],
    [0x48, 0x00],
    [0x49, 0x00],
    [0x4A, 0x00],
    [0x4B, 0x00],
    [0x4C, 0x00],
    [0x4D, 0x00],
    [0x4E, 0x00],
    [0x4F, 0x00],
    [0x50, 0x00],
    [0x51, 0x00],
    [0x52, 0x00],
    [0x53, 0x00],
    [0x54, 0x00],
    [0x55, 0x00],
    [0x56, 0x00],
    [0x57, 0x00],
    [0x58, 0x00],
    [0x59, 0x00],
    [0x5A, 0x00],
    [0x5B, 0x00],
    [0x5C, 0x00],
    [0x5D, 0x00],
    [0x5E, 0x00],
    [0x5F, 0x00],
    [0x60, 0x00],
    [0x61, 0x00],
    [0x62, 0x00],
    [0x63, 0x00],
    [0x64, 0x00],
    [0x65, 0x00],
    [0x66, 0x00],
    [0x67, 0x00],
    [0x68, 0x00],
    [0x69, 0x00],
    [0x6A, 0x00],
    [0x6B, 0x00],
    [0x6C, 0x00],
    [0x6D, 0x00],
    [0x6E, 0x00],
    [0x6F, 0x00],
    [0x70, 0x00],
    [0x71, 0x00],
    [0x72, 0x00],
    [0x73, 0x00],
    [0x74, 0x00],
    [0x75, 0x00],
    [0x76, 0x00],
    [0x77, 0x00],
    [0x78, 0x00],
    [0x79, 0x00],
    [0x7A, 0x00],
    [0x7B, 0x00],
    [0x7C, 0x00],
    [0x7D, 0x00],
    [0x7E, 0x00],
    [0x7F, 0x00],
    [0x80, 0x00],
    [0x81, 0x00],
    [0x82, 0x00],
    [0x83, 0x00],
    [0x84, 0x00],
    [0x85, 0x00],
    [0x86, 0x00],
    [0x87, 0x00],
    [0x88, 0x00],
    [0x89, 0x00],
    [0x8A, 0x00],
    [0x8B, 0x00],
    [0x8C, 0x00],
    [0x8D, 0x00],
    [0x8E, 0x00],
    [0x8F, 0x00],
    [0x90, 0x00],
    [0x91, 0x00],
    [0x92, 0x00],
    [0x93, 0x00],
    [0x94, 0x00],
    [0x95, 0x00],
    [0x96, 0x00],
    [0x97, 0x00],
    [0x98, 0x00],
    [0x99, 0x00],
    [0x9A, 0x00],
    [0x9B, 0x00],
    [0x9C, 0x00],
    [0x9D, 0x00],
    [0x9E, 0x00],
    [0x9F, 0x00],
    [0xA0, 0x00],
];

pub const DEFAULT_ADLIB_BASE: u16 = 0x388;
pub const SAMPLE_BUF_LEN: usize = 800;

use crate::{
    bus::{BusInterface, DeviceRunTimeUnit, IoDevice},
    device_traits::sounddevice::{AudioSample, SoundDevice},
};
use crossbeam_channel::Sender;
use opl3_rs::{Opl3Device, OplRegisterFile};

pub struct AdLibCard {
    pub io_base: u16,
    pub opl3: Opl3Device,
    pub sender: Sender<AudioSample>,
    pub in_buf: [i16; 2],
    pub out_buf: [i16; SAMPLE_BUF_LEN * 2],
    pub sample_accum: usize,
    pub usec_accum: f64,
    pub addr: u8,
}

impl AdLibCard {
    pub fn new(io_base: u16, sample_rate: u32, sender: Sender<AudioSample>) -> Self {
        log::debug!("Creating Opl3Device with sample rate {}", sample_rate);
        let opl3 = Opl3Device::new(sample_rate);
        AdLibCard {
            io_base,
            opl3,
            sender,
            in_buf: [0; 2],
            out_buf: [0; SAMPLE_BUF_LEN * 2],
            sample_accum: 0,
            usec_accum: 0.0,
            addr: 0,
        }
    }

    pub fn test(&mut self) {
        let mut buf: [i16; 1024 * 2] = [0; 1024 * 2];
        for i in 0..1 {
            for regpair in REGS {
                println!("Writing register: {:02X} = {:02X}", regpair[0], regpair[1]);
                self.opl3
                    .write_register(regpair[0], regpair[1], OplRegisterFile::Primary, false);
                self.opl3.run(30.0);
            }
            _ = self.opl3.generate_samples(&mut buf);
        }
    }
}

impl SoundDevice for AdLibCard {
    fn run(&mut self, usec: f64) {
        self.sample_accum += self.opl3.run(usec);
        self.usec_accum += usec;

        while self.sample_accum >= SAMPLE_BUF_LEN {
            //log::debug!("Reached {} samples in {}", self.sample_accum, self.usec_accum);
            self.sample_accum -= SAMPLE_BUF_LEN;
            self.usec_accum = 0.0;

            //_ = self.opl3.generate_samples(&mut self.out_buf);

            for s in self.out_buf.chunks_exact(2) {
                let samp0 = if s[0] == -1 { 0 } else { s[0] };
                let samp1 = if s[1] == -1 { 0 } else { s[1] };
                self.sender.send(samp0 as f32 / i16::MAX as f32).unwrap();
                self.sender.send(samp1 as f32 / i16::MAX as f32).unwrap();
            }
        }

        //self.sender.send(self.in_buf[0] as f32 / i16::MAX as f32).unwrap();
        //self.sender.send(self.in_buf[1] as f32 / i16::MAX as f32).unwrap();
    }
}

impl IoDevice for AdLibCard {
    fn read_u8(&mut self, port: u16, _delta: DeviceRunTimeUnit) -> u8 {
        //log::debug!("Read from Adlib port {:04X}", port - self.io_base);
        match port - self.io_base {
            0 => self.opl3.read_status(),
            1 => 0x1F,
            _ => 0xFF,
        }
    }

    fn write_u8(&mut self, port: u16, data: u8, _bus: Option<&mut BusInterface>, _delta: DeviceRunTimeUnit) {
        match port - self.io_base {
            0 => {
                _ = {
                    //log::debug!("Write to Adlib address port {:02X}", data);
                    _ = self.opl3.write_address(data, OplRegisterFile::Primary);
                    self.addr = data;
                }
            }
            1 => {
                _ = {
                    if self.addr < 0xB0 || self.addr >= 0xC0 {
                        log::debug!("Write to Adlib data port {:02X}:{:02X}", self.addr, data);
                        //self.opl3.write_data(data, OplRegisterFile::Primary, true)
                        self.opl3
                            .write_register(self.addr, data, OplRegisterFile::Primary, false);
                    }
                    //log::debug!("Write to Adlib data port {:02X}", data);
                    //self.opl3.write_data(data, OplRegisterFile::Primary, true)
                    //self.opl3.write_register(self.addr, data, OplRegisterFile::Primary, true);
                }
            }
            _ => {}
        }
    }

    #[rustfmt::skip]
    fn port_list(&self) -> Vec<(String, u16)> {
        let ports = vec![
            (String::from("Adlib Address"), self.io_base),
            (String::from("Adlib Data"), self.io_base + 1),
        ];
        ports
    }
}

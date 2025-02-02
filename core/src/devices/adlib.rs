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

pub const DEFAULT_ADLIB_BASE: u16 = 0x388;
pub const SAMPLE_BUF_LEN: usize = 800;

use crate::{
    bus::{BusInterface, DeviceRunTimeUnit, IoDevice},
    cpu_common::LogicAnalyzer,
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
}

impl SoundDevice for AdLibCard {
    fn run(&mut self, usec: f64) {
        self.sample_accum += self.opl3.run(usec);
        self.usec_accum += usec;

        while self.sample_accum >= SAMPLE_BUF_LEN {
            //log::debug!("Reached {} samples in {}", self.sample_accum, self.usec_accum);
            self.sample_accum -= SAMPLE_BUF_LEN;
            self.usec_accum = 0.0;

            _ = self.opl3.generate_samples(&mut self.out_buf);

            for s in self.out_buf.chunks_exact(2) {
                let samp0 = if s[0] == -1 { 0 } else { s[0] };
                let samp1 = if s[1] == -1 { 0 } else { s[1] };
                self.sender.send(samp0 as f32 / i16::MAX as f32).unwrap();
                self.sender.send(samp1 as f32 / i16::MAX as f32).unwrap();
            }
        }
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

    fn write_u8(
        &mut self,
        port: u16,
        data: u8,
        _bus: Option<&mut BusInterface>,
        _delta: DeviceRunTimeUnit,
        _analyzer: Option<&mut LogicAnalyzer>,
    ) {
        match port - self.io_base {
            0 => {
                //log::debug!("Write to Adlib address port {:02X}", data);
                _ = self.opl3.write_address(data, OplRegisterFile::Primary);
                self.addr = data;
            }
            1 => {
                _ = self.opl3.write_data(data, OplRegisterFile::Primary, false);
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

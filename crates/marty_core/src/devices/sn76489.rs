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

//! Implementation of Texas Instruments SN76489 3-voice sound chip.
//! This chip was used in the IBM PCjr and Tandy 1000 series of computers.

use crate::{
    channel::BidirectionalChannel,
    device_traits::sounddevice::AudioSample,
    devices::lpt_port::{ParallelControl, ParallelMessage, ParallelStatus},
};

use crate::{
    bus::{BusInterface, ClockFactor, DeviceRunTimeUnit, IoDevice, NO_IO_BYTE},
    cpu_common::LogicAnalyzer,
};
use crossbeam_channel::Sender;

pub const SAMPLE_DECAY_RATE: f32 = 0.01; // 1% decay per sample
pub const SAMPLE_RATE: f64 = 48000.0;
pub const SAMPLE_TIME: f64 = 1_000_000.0 / SAMPLE_RATE;
pub const SAMPLE_BUFFER_SIZE: usize = SAMPLE_RATE as usize / 60;
pub const SN_PORT_MASK: u16 = 0x0007;
pub const SN_DEFAULT_PORT: u16 = 0x00C0;
pub const SN_COUNTER_MAX: u16 = 0x03FF;
pub const SN_INTERNAL_DIVISOR: u32 = 16;

pub const SN_MHZ: f64 = 3.579545;
pub const SN_TICK_US: f64 = 1.0 / SN_MHZ;

const ATTENUATION_TABLE: [f32; 16] = [
    1.000, // 0 dB
    0.794, // 2 dB
    0.630, // 4 dB
    0.500, // 6 dB
    0.398, // 8 dB
    0.316, // 10 dB
    0.251, // 12 dB
    0.200, // 14 dB
    0.158, // 16 dB
    0.126, // 18 dB
    0.100, // 20 dB
    0.079, // 22 dB
    0.063, // 24 dB
    0.050, // 26 dB
    0.040, // 28 dB
    0.0,   // off
];

pub struct Sn76489 {
    io_base: u16,
    clock_divisor: u32,
    internal_divisor: u32,
    sample_sender: Sender<AudioSample>,
    sys_tick_accumulator: u32,
    sn_tick_accumulator: f64,
    ticks_per_sample: f64,

    selected_channel: usize,
    channels: [SoundChannel; 3],
}

#[derive(Default)]
struct SoundChannel {
    idx: usize,
    running: bool,
    frequency: u16,
    freq_counter: u16,
    attenuation: usize,
    output: bool,
}

impl SoundChannel {
    pub fn new(index: usize) -> SoundChannel {
        SoundChannel {
            idx: index,
            ..Default::default()
        }
    }

    #[inline]
    pub fn set_freq_1st(&mut self, data: u8) {
        // First byte contains the lower 4 bits of frequency.
        self.frequency = (self.frequency & 0xFFF0) | (data & 0x0F) as u16;
        log::debug!(
            "[{}]: Setting frequency 1st byte: {:02X}, new freq {}",
            self.idx,
            data,
            self.frequency
        );
    }
    #[inline]
    pub fn set_freq_2nd(&mut self, data: u8) {
        // Second byte contains the upper 6 bits of frequency.
        self.frequency = (self.frequency & 0x000F) | (((data & 0x3F) as u16) << 4);
        if self.frequency == 0 {
            self.frequency = SN_COUNTER_MAX;
        }
        log::debug!(
            "[{}]: Setting frequency 2st byte: {:02X}, new freq {}",
            self.idx,
            data,
            self.frequency
        );
        self.running = true;
    }
    #[inline]
    pub fn set_attenuation(&mut self, data: u8) {
        log::debug!("[{}]: Setting attenuation: {:02X}", self.idx, data);
        self.attenuation = (data & 0x0F) as usize;
    }
}

impl Sn76489 {
    pub fn new(io_base: u16, crystal: f64, clock_divisor: ClockFactor, sample_sender: Sender<AudioSample>) -> Self {
        let clock_divisor = if let ClockFactor::Divisor(divisor) = clock_divisor {
            if divisor == 0 {
                panic!("Clock divisor cannot be zero");
            }
            // Internal divisor of 16.
            divisor as u32
        }
        else {
            panic!("Sn76489 clock multiplier unimplemented");
        };

        // Internal divisor of 16.
        let internal_divisor = clock_divisor * SN_INTERNAL_DIVISOR;
        let ticks_per_sample = ((crystal * 1_000_000.0) / clock_divisor as f64) / SAMPLE_RATE;

        log::debug!(
            "SN76489: crystal={}MHz, clock_divisor={}, internal_divisor={}, ticks_per_sample={}",
            crystal,
            clock_divisor,
            internal_divisor,
            ticks_per_sample
        );

        Sn76489 {
            io_base,
            clock_divisor,
            internal_divisor,
            sample_sender,
            sys_tick_accumulator: 0,
            sn_tick_accumulator: 0.0,
            ticks_per_sample,

            selected_channel: 0,
            channels: [SoundChannel::new(0), SoundChannel::new(1), SoundChannel::new(2)],
        }
    }

    pub fn sample_rate(&self) -> u32 {
        SAMPLE_RATE as u32
    }

    pub fn run(&mut self, run_unit: DeviceRunTimeUnit) {
        if let DeviceRunTimeUnit::SystemTicks(ticks) = run_unit {
            self.sys_tick_accumulator += ticks;
        }
        else {
            panic!("Free-running SN76489 devices not supported");
        }

        while self.sys_tick_accumulator >= self.internal_divisor {
            self.sys_tick_accumulator -= self.internal_divisor;
            self.tick();
            self.sn_tick_accumulator += SN_INTERNAL_DIVISOR as f64;
        }

        while self.sn_tick_accumulator >= self.ticks_per_sample {
            self.sn_tick_accumulator -= self.ticks_per_sample;
            let sample = self.sample();
            self.sample_sender.send(sample.into()).unwrap();
        }
    }

    pub fn tick(&mut self) {
        for channel in self.channels.iter_mut() {
            channel.freq_counter = channel.freq_counter.wrapping_sub(1);
            if channel.freq_counter == 0 {
                channel.freq_counter = channel.frequency;
                channel.output = !channel.output;
            }
        }
    }

    pub fn sample(&mut self) -> f32 {
        let mut sample = 0.0;
        for channel in self.channels.iter() {
            if channel.running && channel.output {
                //sample += 1.0 - (channel.attenuation as f32) / 15.0;
                sample += ATTENUATION_TABLE[channel.attenuation];
            }
        }
        (sample / 1.5) - 1.0 // Normalize to -1.0 - 1.0 range
    }
}

impl IoDevice for Sn76489 {
    fn read_u8(&mut self, _port: u16, _delta: DeviceRunTimeUnit) -> u8 {
        // No readable ports
        NO_IO_BYTE
    }

    fn write_u8(
        &mut self,
        _port: u16,
        data: u8,
        _bus: Option<&mut BusInterface>,
        _delta: DeviceRunTimeUnit,
        _analyzer: Option<&mut LogicAnalyzer>,
    ) {
        if data & 0x80 != 0 {
            let reg = (data >> 4) & 0x07;
            log::debug!("Write to reg: {} data: {:02X}", reg, data);
            if reg < 6 {
                // One of the tone channels is selected.
                self.selected_channel = (reg >> 1) as usize;
                if reg & 0x01 == 0 {
                    // 1st write to frequency Register
                    self.channels[self.selected_channel].set_freq_1st(data);
                }
                else {
                    // Write to attenuation Register
                    self.channels[self.selected_channel].set_attenuation(data);
                }
            }
        }
        else {
            // Write the upper 6 bits of frequency data to the selected channel
            self.channels[self.selected_channel].set_freq_2nd(data);
        }
    }

    fn port_list(&self) -> Vec<(String, u16)> {
        vec![("SN76489".to_string(), self.io_base)]
    }
}

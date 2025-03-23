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
//!
//! TI documentation typically refers to the tone channels as "tone 1", "tone 2", and "tone 3".
//! We will reuse those names here for clarity.

use crate::{channel::BidirectionalChannel, device_traits::sounddevice::AudioSample};

use crate::{
    bus::{BusInterface, ClockFactor, DeviceRunTimeUnit, IoDevice},
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
pub const SN_LFSR_INIT: u16 = 0x7FFF;
pub const SN_WRITE_WAIT_TICKS: u32 = 32;
pub const SN_MHZ: f64 = 3.579545;
pub const SN_TICK_US: f64 = 1.0 / SN_MHZ;

pub const SN_NOISE_DIVIDER_1: u16 = (512 / SN_INTERNAL_DIVISOR) as u16;
pub const SN_NOISE_DIVIDER_2: u16 = (1024 / SN_INTERNAL_DIVISOR) as u16;
pub const SN_NOISE_DIVIDER_3: u16 = (2048 / SN_INTERNAL_DIVISOR) as u16;

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
    write_wait: u32,
    sample_sender: Sender<AudioSample>,
    sys_tick_accumulator: u32,
    sn_tick_accumulator: f64,
    ticks_per_sample: f64,
    selected_channel: usize,
    tone_channels: [SoundChannel; 3],
    noise_channel: NoiseChannel,
    attenuation_registers: [ChannelAttenuation; 4],
}

#[derive(Default)]
struct ChannelAttenuation {
    idx: usize,
    attenuation: usize,
}

impl ChannelAttenuation {
    pub fn new(index: usize) -> ChannelAttenuation {
        ChannelAttenuation {
            idx: index,
            ..Default::default()
        }
    }
    #[inline]
    pub fn set(&mut self, data: u8) {
        log::debug!("[{}]: Setting attenuation: {:02X}", self.idx, data);
        self.attenuation = (data & 0x0F) as usize;
    }
    #[inline(always)]
    pub fn get(&self) -> f32 {
        ATTENUATION_TABLE[self.attenuation]
    }
}

#[derive(Default)]
struct SoundChannel {
    idx: usize,
    running: bool,
    period: u16,
    freq_counter: u16,
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
        self.period = (self.period & 0xFFF0) | (data & 0x0F) as u16;
        if self.period == 0 {
            self.period = SN_COUNTER_MAX + 1;
        }
        log::debug!(
            "[{}]: Setting frequency 1st byte: {:02X}, new period {}",
            self.idx,
            data,
            self.period
        );
        self.running = true;
    }
    #[inline]
    pub fn set_freq_2nd(&mut self, data: u8) {
        // Second byte contains the upper 6 bits of frequency.
        self.period = (self.period & 0x000F) | (((data & 0x3F) as u16) << 4);
        if self.period == 0 {
            self.period = SN_COUNTER_MAX + 1;
        }
        log::debug!(
            "[{}]: Setting frequency 2st byte: {:02X}, new period {}",
            self.idx,
            data,
            self.period
        );
        self.running = true;
    }

    /// Tick the tone channel by decrementing the frequency counter.
    /// If the frequency counter reaches zero, toggle the output and reset the counter.
    /// Returns true if the output was toggled.
    #[inline(always)]
    pub fn tick(&mut self) -> bool {
        self.freq_counter = self.freq_counter.wrapping_sub(1);
        if self.freq_counter == 0 {
            self.freq_counter = self.period;
            self.output = !self.output;
            true
        }
        else {
            false
        }
    }

    #[inline(always)]
    fn output(&self) -> bool {
        self.running && self.output
    }
}

#[derive(Copy, Clone, Default, Debug)]
pub enum FeedbackType {
    #[default]
    Periodic,
    WhiteNoise,
}

#[derive(Default)]
struct NoiseChannel {
    running: bool,
    feedback: FeedbackType,
    shift_rate: u8,
    counter: u16,
    lfsr: u16,
    output: bool,
}

impl NoiseChannel {
    pub fn new() -> NoiseChannel {
        NoiseChannel {
            lfsr: SN_LFSR_INIT,
            ..Default::default()
        }
    }

    #[inline]
    pub fn set(&mut self, data: u8) {
        // First byte contains the lower 4 bits of frequency.
        self.feedback = match data & 0x04 {
            0 => FeedbackType::Periodic,
            _ => FeedbackType::WhiteNoise,
        };
        self.shift_rate = data & 0x03;
        log::debug!(
            "[Noise]: Setting parameter byte: {:02X}, feedback {:?}, shift rate {}",
            data,
            self.feedback,
            self.shift_rate
        );
        self.running = true;
    }

    /// Shift the LFSR once and calculate the new output status.
    fn shift(&mut self) -> bool {
        let feedback_bit = match self.feedback {
            FeedbackType::WhiteNoise => {
                let bit0 = self.lfsr & 0x0001;
                let bit3 = (self.lfsr >> 3) & 0x0001;
                bit0 ^ bit3
            }
            FeedbackType::Periodic => {
                // Just bit 0
                self.lfsr & 0x0001
            }
        };

        // Shift right by 1
        self.lfsr >>= 1;

        // Inject the feedback bit into bit 14
        if feedback_bit == 1 {
            self.lfsr |= 0x4000;
        }

        // The new output bit is typically the LSB after shifting,
        // but some references do it before. The SN doc is ambiguous,
        // but either approach yields similar "randomness."
        let new_bit0 = self.lfsr & 0x0001;
        self.output = new_bit0 != 0;

        self.output
    }

    fn tick(&mut self, tone3_edge: bool) {
        // If shift_rate == 0, we shift the LFSR every time tone #3 output changes state.
        // 'tone3_edge' is a boolean representing that tone #3 output changed on this tick.
        if self.shift_rate == 0 {
            if tone3_edge {
                self.shift();
            }
            return;
        }

        // If shift_rate != 0, we have a fixed rate => some precomputed reload
        let reload = match self.shift_rate {
            1 => SN_NOISE_DIVIDER_1,
            2 => SN_NOISE_DIVIDER_2,
            3 => SN_NOISE_DIVIDER_3,
            _ => panic!("shift_rate out of range"),
        };

        // Decrement the counter and shift the LFSR when it reaches zero.
        if self.counter > 0 {
            self.counter -= 1;
        }
        else {
            self.counter = reload;
            self.shift();
        }
    }

    #[inline(always)]
    fn output(&self) -> bool {
        self.running && self.output
    }
}

impl Sn76489 {
    pub fn new(io_base: u16, crystal: f64, clock_divisor: ClockFactor, sample_sender: Sender<AudioSample>) -> Self {
        let clock_divisor = if let ClockFactor::Divisor(divisor) = clock_divisor {
            if divisor == 0 {
                panic!("Clock divisor cannot be zero");
            }
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
            write_wait: 0,
            sample_sender,
            sys_tick_accumulator: 0,
            sn_tick_accumulator: 0.0,
            ticks_per_sample,

            selected_channel: 0,
            tone_channels: [SoundChannel::new(0), SoundChannel::new(1), SoundChannel::new(2)],
            noise_channel: NoiseChannel::new(),
            attenuation_registers: [
                ChannelAttenuation::new(0),
                ChannelAttenuation::new(1),
                ChannelAttenuation::new(2),
                ChannelAttenuation::new(3),
            ],
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

        // We get ticks at 14.3181818Mhz, we have an external divisor of 4,
        // for an external clock of 3.579545Mhz.  There is an internal divisor of 16,
        // so we have a final clock of 223.72156Khz.
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
        self.tone_channels[0].tick();
        self.tone_channels[1].tick();
        // Pass any edge of tone #3 to the noise channel
        self.noise_channel.tick(self.tone_channels[2].tick());
        // Decrement the write wait counter.
        self.write_wait = self.write_wait.saturating_sub(1);
    }

    /// Produce a single, f32 sample from the contribution of all channels, normalized to -1.0 - 1.0 range.
    pub fn sample(&mut self) -> f32 {
        let mut sample = 0.0;
        for (c_idx, channel) in self.tone_channels.iter().enumerate() {
            // If channel output is high, add its attenuated contribution to the current sample.
            if channel.output() {
                sample += self.attenuation_registers[c_idx].get();
            }
        }
        if self.noise_channel.output() {
            sample += self.attenuation_registers[3].get()
        }

        // Normalize [0..4.0] sample range to [-1.0..1.0]
        (sample / 2.0) - 1.0
    }
}

impl IoDevice for Sn76489 {
    /// Handle a write to the SN76489.
    /// The SN76489 has a single 8-bit register, but 8 internal registers which are decoded
    /// from 3 bits of the written byte.
    ///
    /// Register map:
    ///
    /// | Register   | Description         |
    /// |------------|---------------------|
    /// | Register 0 | Tone 1 Frequency    |
    /// | Register 1 | Tone 1 Attenuation  |
    /// | Register 2 | Tone 2 Frequency    |
    /// | Register 3 | Tone 2 Attenuation  |
    /// | Register 4 | Tone 3 Frequency    |
    /// | Register 5 | Tone 3 Attenuation  |
    /// | Register 6 | Noise Control       |
    /// | Register 7 | Noise Attenuation   |
    ///
    /// The MSB of the byte written indicates that the three register selection bits are present.
    /// The only case where this should not be set is when writing the upper 6 bits of tone channel
    /// frequency data, which is sent as a second byte; the first byte having both selected the
    /// register and written the lower 4 bits.
    /// The selected register value remains latched, so the upper 6 bits of a tone channel can be
    /// repeatedly rewritten by writing bytes with the MSB cleared.
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

            self.selected_channel = (reg >> 1) as usize;

            // Every odd register represents an attenuation register.
            if reg & 0x01 != 0 {
                // LSB is set, this is a write to an attenuation register.
                self.attenuation_registers[self.selected_channel].set(data);
            }
            else {
                if self.selected_channel < 3 {
                    // One of the tone channels is selected.
                    // 1st write to frequency Register
                    self.tone_channels[self.selected_channel].set_freq_1st(data);
                }
                else {
                    // The noise channel is selected.
                    self.noise_channel.set(data);
                }
            }
        }
        else {
            // Write the upper 6 bits of frequency data to the previously selected channel
            if self.selected_channel < 3 {
                self.tone_channels[self.selected_channel].set_freq_2nd(data);
            }
        }

        // Reset write wait counter.
        self.write_wait = SN_WRITE_WAIT_TICKS;
    }

    /// Return the number of system tick waits required for the device to be ready for a new write.
    /// This is approximately 32 clock ticks after the last write.
    fn write_wait(&mut self, _port: u16, _delta: DeviceRunTimeUnit) -> u32 {
        let sys_tick_waits = self.write_wait * self.internal_divisor;
        log::debug!("SN76489 write_wait: {} sys_tick_waits", sys_tick_waits);
        0
    }

    /// Provide a list of I/O ports used by this device.
    /// The SN76489 is typically decoded at 0xC0-0xC7 in Tandy systems. This is important, as
    /// some titles will use 16-bit OUT instructions to make two writes at once.
    fn port_list(&self) -> Vec<(String, u16)> {
        let mut ports = Vec::new();
        for i in 0..8 {
            ports.push((format!("SN76489 Reg {}", i), self.io_base + i as u16));
        }
        ports
    }
}

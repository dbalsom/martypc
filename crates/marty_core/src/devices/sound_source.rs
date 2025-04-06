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

//! Implementation of the Disney Sound Source

use std::collections::VecDeque;

use crate::{
    channel::BidirectionalChannel,
    device_traits::sounddevice::AudioSample,
    devices::lpt_port::{ParallelControl, ParallelMessage, ParallelStatus},
};

use crossbeam_channel::Sender;

pub const POWER_STROBE_LIMIT: f64 = 125.0; // maximum strobe time before power-off
pub const SAMPLE_DECAY_RATE: f32 = 0.01; // 1% decay per sample
pub const SAMPLE_RATE: f64 = 7000.0; // 7 kHz
pub const SAMPLE_TIME: f64 = 1_000_000.0 / SAMPLE_RATE;
pub const SAMPLE_BUFFER_SIZE: usize = SAMPLE_RATE as usize / 60;
pub const FIFO_LEN: usize = 16;
pub const ACK_CLEAR: usize = 3; // This many samples free clears ack
pub const POLL_TIME: f64 = 50.0; // 100 microseconds

pub struct DSoundSource {
    pub poll_accum: f64,
    pub sample_accum: f64,
    pub strobe_accum: f64,
    pub power: bool,
    pub channel: BidirectionalChannel<ParallelMessage>,
    pub data: u8,
    pub status: ParallelStatus,
    pub control: ParallelControl,
    pub fifo: VecDeque<u8>,
    pub sample_sender: Sender<AudioSample>,
    pub last_sample: f32,
}

impl DSoundSource {
    pub fn new(channel: BidirectionalChannel<ParallelMessage>, sample_sender: Sender<AudioSample>) -> Self {
        DSoundSource {
            poll_accum: 0.0,
            sample_accum: 0.0,
            strobe_accum: 0.0,
            power: false,
            channel,
            data: 0,
            control: ParallelControl::default(),
            status: ParallelStatus::default(),
            fifo: VecDeque::new(),
            sample_sender,
            last_sample: 0.0,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        SAMPLE_RATE as u32 * 7
    }

    pub fn receive(&mut self, message: ParallelMessage) {
        match message {
            ParallelMessage::Data(data) => {
                if !self.power {
                    return;
                }
                self.data = data;
            }
            ParallelMessage::Control(control) => {
                if !control.select_in() {
                    if !self.power {
                        log::debug!("SoundSource::receive(): !SELECT_IN low: Power on");
                        self.power = true;
                    }
                }
                else if !self.control.select_in() && self.power {
                    // Rising edge of !SELECT_IN, clock data into FIFO
                    if self.fifo.len() < FIFO_LEN {
                        self.fifo.push_back(self.data);

                        if self.fifo.len() == FIFO_LEN && !self.status.ack() {
                            self.status.set_ack(true);
                            let _ = self.channel.send(ParallelMessage::Status(self.status));
                        }
                        else if self.fifo.len() < (FIFO_LEN - ACK_CLEAR) && self.status.ack() {
                            self.status.set_ack(false);
                            let _ = self.channel.send(ParallelMessage::Status(self.status));
                        }
                    }
                    else {
                        log::warn!("SoundSource::receive(): FIFO overflow");

                        if !self.status.ack() {
                            self.status.set_ack(true);
                            let _ = self.channel.send(ParallelMessage::Status(self.status));
                        }
                    }
                }

                self.control = control;
            }
            ParallelMessage::Status(_status) => {
                //self.status = status;
            }
        }
    }

    pub fn update(&mut self, _usec: f64) {
        while let Ok(message) = self.channel.try_recv() {
            self.receive(message);
        }

        if self.power {
            // Do something with the sound source
        }
    }

    pub fn run(&mut self, usec: f64) {
        self.poll_accum += usec;
        if self.poll_accum >= POLL_TIME {
            self.poll_accum -= POLL_TIME;
            self.update(POLL_TIME);
        }

        if self.power && self.control.select_in() {
            self.strobe_accum += usec;
            if self.strobe_accum >= POWER_STROBE_LIMIT {
                log::debug!(
                    "SoundSource::run(): Strobe timeout ({:.2}us) with !SELECT_IN high, power off.",
                    self.strobe_accum
                );
                self.power = false;
                self.strobe_accum = 0.0;
                self.status.set_ack(false);
                let _ = self.channel.send(ParallelMessage::Status(self.status));
            }
        }
        else {
            self.strobe_accum = 0.0;
        }

        self.sample_accum += usec;
        if self.sample_accum >= SAMPLE_TIME {
            self.sample_accum -= SAMPLE_TIME;

            if let Some(sample) = self.fifo.pop_front() {
                if self.status.ack() && self.fifo.len() < (FIFO_LEN - ACK_CLEAR) {
                    self.status.set_ack(false);
                    let _ = self.channel.send(ParallelMessage::Status(self.status));
                }

                let sample_f32 = 255.0 / (sample as f32);
                self.last_sample = sample_f32;
                for _ in 0..7 {
                    self.sample_sender.send(sample_f32).unwrap();
                }
            }
            else {
                for _ in 0..7 {
                    self.sample_sender.send(self.last_sample).unwrap();
                }
                self.last_sample -= SAMPLE_DECAY_RATE;
                if self.last_sample < 0.0 {
                    self.last_sample = 0.0;
                }
            }
        }
    }
}

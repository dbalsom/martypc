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

    sound.rs

    Implement interfaces for generating sound output.

*/

use crate::device_traits::sounddevice::AudioSample;
use crossbeam_channel::Receiver;

pub const DEFAULT_SAMPLE_RATE: u32 = 44100;

#[derive(Debug)]
pub struct SoundOutputConfig {
    pub enabled: bool,
    pub sample_rate: u32,
    pub channels: usize,
    pub buffer_size: usize,
}

impl Default for SoundOutputConfig {
    fn default() -> Self {
        SoundOutputConfig {
            enabled: true,
            sample_rate: DEFAULT_SAMPLE_RATE,
            channels: 2,
            buffer_size: 1024,
        }
    }
}

#[derive(Default)]
pub struct SoundOutput {
    sources: Vec<SoundSourceDescriptor>,
}

pub struct SoundSourceDescriptor {
    pub name: String,
    pub sample_rate: u32,
    pub channels: usize,
    pub receiver: Receiver<AudioSample>,
}

impl SoundSourceDescriptor {
    pub fn new(
        name: &str,
        sample_rate: u32,
        channels: usize,
        receiver: Receiver<AudioSample>,
    ) -> SoundSourceDescriptor {
        SoundSourceDescriptor {
            name: name.to_string(),
            sample_rate,
            channels,
            receiver,
        }
    }
}

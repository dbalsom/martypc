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

    sound_player.rs

    Implement the sound player interface.

*/

use anyhow::{anyhow, Error};
use frontend_common::types::sound::SoundSourceStats;

// Stub in missing types that won't be present in the core with sound disabled
#[derive(Default)]
pub struct SoundSource {}
#[derive(Default)]
pub struct SoundOutputConfig {}
#[derive(Default)]
pub struct SoundSourceDescriptor {}

pub struct SoundInterface {
    enabled: bool,
    device_name: String,
    sample_rate: u32,
    sample_format: String, // We don't really need this, so I am not converting it to an enum.
    channels: usize,
    sources: Vec<SoundSource>,
}

impl Default for crate::sound::SoundInterface {
    fn default() -> Self {
        SoundInterface {
            enabled: false,
            device_name: String::new(),
            sample_rate: 0,
            sample_format: String::new(),
            channels: 0,
            sources: Vec::new(),
        }
    }
}

impl SoundInterface {
    pub fn new(enabled: bool) -> SoundInterface {
        SoundInterface {
            enabled,
            ..Default::default()
        }
    }

    pub fn open_device(&mut self) -> Result<(), Error> {
        *self = {
            SoundInterface {
                enabled: self.enabled,
                device_name: String::from("Null Sound Device"),
                sample_rate: self.sample_rate,
                sample_format: self.sample_format.clone(),
                channels: self.channels,
                sources: Vec::new(),
            }
        };

        Ok(())
    }

    pub fn add_source(&mut self, _source: &SoundSourceDescriptor) -> Result<(), Error> {
        Ok(())
    }

    pub fn run(&mut self) {}

    pub fn open_stream(&mut self) -> Result<(), Error> {
        Ok(())
    }

    pub fn device_name(&self) -> String {
        self.device_name.clone()
    }

    pub fn set_volume(&mut self, s_idx: usize, volume: f32) {}

    pub fn config(&self) -> SoundOutputConfig {
        Default::default()
    }

    pub fn get_stats(&self) -> Vec<SoundSourceStats> {
        Vec::new()
    }
}

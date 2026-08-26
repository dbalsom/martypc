/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

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
#![allow(dead_code)]

//! The null sound interface.  A stub of the sound API for when the sound
//! feature is not enabled.

use marty_core::sound::{SoundOutputConfig, SoundSourceDescriptor};
use marty_frontend_common::{
    sound_file_manager::{PresentableSoundKey, SoundEffect},
    types::sound::SoundSourceInfo,
};

use anyhow::Error;
use web_time::Duration;

pub struct NullSoundInterface {
    enabled: bool,
    device_name: String,
    sample_rate: u32,
    channels: usize,
}

impl Default for NullSoundInterface {
    fn default() -> Self {
        NullSoundInterface {
            enabled: false,
            device_name: String::new(),
            sample_rate: 0,
            channels: 0,
        }
    }
}

impl NullSoundInterface {
    pub fn new(enabled: bool) -> NullSoundInterface {
        NullSoundInterface {
            enabled,
            ..Default::default()
        }
    }

    pub fn open_device(&mut self) -> Result<(), Error> {
        self.device_name = String::from("Null Sound Device");

        Ok(())
    }

    pub fn add_source(&mut self, _source: &SoundSourceDescriptor) -> Result<(), Error> {
        Ok(())
    }

    pub fn run(&mut self, _duration: Duration) {}

    pub fn open_stream(&mut self) -> Result<(), Error> {
        Ok(())
    }

    pub fn device_name(&self) -> String {
        self.device_name.clone()
    }

    pub fn set_volume(&mut self, _s_idx: usize, _volume: Option<f32>, _muted: Option<bool>) {}

    pub fn set_master_speed(&mut self, _speed: f32) {}

    pub fn config(&self) -> SoundOutputConfig {
        SoundOutputConfig {
            enabled: self.enabled,
            sample_rate: self.sample_rate,
            channels: self.channels,
            buffer_size: 1024,
        }
    }

    pub fn info(&self) -> Vec<SoundSourceInfo> {
        Vec::new()
    }

    pub fn play_sound(&mut self, _samples: &[f32], _sample_rate: u32, _stereo: bool) -> Result<(), Error> {
        Ok(())
    }

    pub fn device_reset(&mut self) {}

    pub fn set_looping_sounds_paused(&mut self, _paused: bool) {}

    pub fn start_loop(
        &mut self,
        _key: PresentableSoundKey,
        _intro: &SoundEffect,
        _looping: &SoundEffect,
    ) -> Result<(), Error> {
        Ok(())
    }

    pub fn stop_loop(&mut self, _key: PresentableSoundKey, _outro: &SoundEffect) -> Result<(), Error> {
        Ok(())
    }
}

impl super::SoundInterfaceBackend for NullSoundInterface {
    fn new(enabled: bool) -> Self {
        NullSoundInterface::new(enabled)
    }

    fn open_device(&mut self) -> Result<(), Error> {
        NullSoundInterface::open_device(self)
    }

    fn open_stream(&mut self) -> Result<(), Error> {
        NullSoundInterface::open_stream(self)
    }

    fn device_name(&self) -> String {
        NullSoundInterface::device_name(self)
    }

    fn add_source(&mut self, source: &SoundSourceDescriptor) -> Result<(), Error> {
        NullSoundInterface::add_source(self, source)
    }

    fn run(&mut self, duration: Duration) {
        NullSoundInterface::run(self, duration)
    }

    fn set_master_speed(&mut self, speed: f32) {
        NullSoundInterface::set_master_speed(self, speed)
    }

    fn set_volume(&mut self, source_index: usize, volume: Option<f32>, muted: Option<bool>) {
        NullSoundInterface::set_volume(self, source_index, volume, muted)
    }

    fn config(&self) -> SoundOutputConfig {
        NullSoundInterface::config(self)
    }

    fn info(&self) -> Vec<SoundSourceInfo> {
        NullSoundInterface::info(self)
    }

    fn play_sound(&mut self, samples: &[f32], sample_rate: u32, stereo: bool) -> Result<(), Error> {
        NullSoundInterface::play_sound(self, samples, sample_rate, stereo)
    }

    fn device_reset(&mut self) {
        NullSoundInterface::device_reset(self)
    }

    fn set_looping_sounds_paused(&mut self, paused: bool) {
        NullSoundInterface::set_looping_sounds_paused(self, paused)
    }

    fn start_loop(
        &mut self,
        key: PresentableSoundKey,
        intro: &SoundEffect,
        looping: &SoundEffect,
    ) -> Result<(), Error> {
        NullSoundInterface::start_loop(self, key, intro, looping)
    }

    fn stop_loop(&mut self, key: PresentableSoundKey, outro: &SoundEffect) -> Result<(), Error> {
        NullSoundInterface::stop_loop(self, key, outro)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_interface_exposes_no_sound_sources() {
        let mut interface = NullSoundInterface::default();

        interface.set_volume(0, Some(0.6), Some(true));

        assert!(interface.info().is_empty());
    }
}

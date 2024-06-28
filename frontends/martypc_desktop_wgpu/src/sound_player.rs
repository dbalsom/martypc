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
use marty_core::sound::SoundOutputConfig;
use rodio::{cpal::traits::HostTrait, DeviceTrait};

pub struct SoundInterface {
    enabled: bool,
    device_name: String,
    sample_rate: u32,
    sample_format: String, // We don't really need this, so I am not converting it to an enum.
    channels: usize,
    cpal_device: Option<rodio::cpal::Device>,
}

impl Default for SoundInterface {
    fn default() -> Self {
        SoundInterface {
            enabled: false,
            device_name: String::new(),
            sample_rate: 0,
            sample_format: String::new(),
            channels: 0,
            cpal_device: None,
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
        //let audio_device = rodio::cpal::default_host().default_output_device()?;
        let audio_device = rodio::cpal::default_host()
            .default_output_device()
            .ok_or(anyhow!("No audio device found."))?;
        let device_name = audio_device.name()?;
        let config = audio_device.default_output_config()?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;
        let sample_format = config.sample_format().to_string();

        *self = {
            SoundInterface {
                enabled: self.enabled,
                device_name,
                sample_rate,
                sample_format,
                channels,
                cpal_device: Some(audio_device),
            }
        };

        Ok(())
    }

    pub fn open_stream(&mut self) -> Result<(), Error> {
        if self.cpal_device.is_none() {
            return Err(anyhow!("No audio device open."));
        }

        let stream = rodio::OutputStream::try_from_device(self.cpal_device.as_ref().unwrap())?;
        log::debug!("Rodio stream successfully opened.");
        Ok(())
    }

    pub fn device_name(&self) -> String {
        self.device_name.clone()
    }

    pub fn config(&self) -> SoundOutputConfig {
        SoundOutputConfig {
            enabled: self.enabled,
            sample_rate: self.sample_rate,
            channels: self.channels,
            buffer_size: 1024,
        }
    }
}

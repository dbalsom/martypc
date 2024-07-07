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
use crossbeam_channel::Receiver;
use marty_core::{
    device_traits::sounddevice::AudioSample,
    sound::{SoundOutputConfig, SoundSourceDescriptor},
};
use rodio::{cpal::traits::HostTrait, DeviceTrait, Sink, Source};

pub struct SoundSource {
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub receiver: Receiver<AudioSample>,
    pub volume: f32,
    pub sink: Sink,
}

pub struct SoundInterface {
    enabled: bool,
    device_name: String,
    sample_rate: u32,
    sample_format: String, // We don't really need this, so I am not converting it to an enum.
    channels: usize,
    device: Option<rodio::cpal::Device>,
    stream: Option<rodio::OutputStream>,
    stream_handle: Option<rodio::OutputStreamHandle>,
    sources: Vec<SoundSource>,
}

impl Default for SoundInterface {
    fn default() -> Self {
        SoundInterface {
            enabled: false,
            device_name: String::new(),
            sample_rate: 0,
            sample_format: String::new(),
            channels: 0,
            device: None,
            stream: None,
            stream_handle: None,
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
        //let audio_device = rodio::cpal::default_host().default_output_device()?;
        let audio_device = rodio::cpal::default_host()
            .default_output_device()
            .ok_or(anyhow!("No audio device found."))?;
        let device_name = audio_device.name()?;
        let config = audio_device.default_output_config()?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;
        let sample_format = config.sample_format().to_string();

        let (stream, stream_handle) = rodio::OutputStream::try_from_device(&audio_device)?;

        *self = {
            SoundInterface {
                enabled: self.enabled,
                device_name,
                sample_rate,
                sample_format,
                channels,
                device: Some(audio_device),
                stream: Some(stream),
                stream_handle: Some(stream_handle),
                sources: Vec::new(),
            }
        };

        Ok(())
    }

    pub fn add_source(&mut self, source: &SoundSourceDescriptor) -> Result<(), Error> {
        let stream_handle = self.stream_handle.as_ref().unwrap();
        let sink = Sink::try_new(stream_handle)?;

        self.sources.push(SoundSource {
            name: source.name.clone(),
            sample_rate: source.sample_rate,
            channels: source.channels as u16,
            receiver: source.receiver.clone(),
            sink,
            volume: 1.0,
        });

        Ok(())
    }

    pub fn run(&mut self) {
        for source in self.sources.iter_mut() {
            let samples_in = source.receiver.try_iter().collect::<Vec<f32>>();
            //log::debug!("received {} samples from channel {}", samples_in.len(), source.name);
            let sink_buffer = rodio::buffer::SamplesBuffer::new(source.channels, source.sample_rate, samples_in);
            source.sink.append(sink_buffer);
        }
    }

    pub fn open_stream(&mut self) -> Result<(), Error> {
        if self.device.is_none() {
            return Err(anyhow!("No audio device open."));
        }

        let stream = rodio::OutputStream::try_from_device(self.device.as_ref().unwrap())?;
        log::debug!("Rodio stream successfully opened.");
        Ok(())
    }

    pub fn device_name(&self) -> String {
        self.device_name.clone()
    }

    pub fn set_volume(&mut self, s_idx: usize, volume: f32) {
        if s_idx < self.sources.len() {
            let source = &mut self.sources[s_idx];
            source.volume = volume;
            source.sink.set_volume(source.volume);
        }
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

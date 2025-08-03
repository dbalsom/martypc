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

    sound_player.rs

    Implement the sound player interface.

*/
const MAX_BUFFER_SIZE: u32 = 100;
const DEFAULT_VOLUME: f32 = 0.25;

const MAX_LATENCY: f32 = 150.0; // Maximum latency in milliseconds

use anyhow::{anyhow, Error};
use crossbeam_channel::Receiver;
use marty_core::{
    device_traits::sounddevice::AudioSample,
    sound::{SoundOutputConfig, SoundSourceDescriptor},
};
use marty_frontend_common::types::sound::SoundSourceInfo;
use rodio::{
    cpal::{traits::HostTrait, SupportedBufferSize},
    DeviceTrait,
    Sink,
    SupportedStreamConfig,
};
use web_time::{Duration, Instant};

pub struct SoundSource {
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub receiver: Receiver<AudioSample>,
    pub sample_ct: u64,
    pub latency_ms: f32,
    #[allow(unused)]
    pub buffer_ct: u64,
    pub first_buffer: Option<Instant>,
    pub muted: bool,
    pub volume: f32,
    pub sink: Sink,
    pub last_block_received: Instant,
    pub controller: AudioLatencyController,
}

impl SoundSource {
    pub fn info(&self) -> SoundSourceInfo {
        SoundSourceInfo {
            name: self.name.clone(),
            sample_rate: self.sample_rate,
            channels: self.channels,
            sample_ct: self.sample_ct,
            latency_ms: self.latency_ms,
            muted: self.muted,
            volume: self.volume,
            len: self.sink.len(),
        }
    }
}

#[allow(unused)]
pub struct AudioLatencyController {
    target_latency: f32, // Target latency in milliseconds
    tolerance: f32,      // Tolerance in milliseconds
    playback_speed: f32, // Current playback speed (1.0 = normal)
    kp: f32,             // Proportional gain
    ki: f32,             // Integral gain (optional)
    integral: f32,       // Accumulated integral term
    min_speed: f32,      // Lower bound for playback speed
    max_speed: f32,      // Upper bound for playback speed
}

impl Default for AudioLatencyController {
    fn default() -> Self {
        AudioLatencyController::new(
            50.0,   // Target latency in ms
            20.0,   // Tolerance in ms
            0.001,  // Proportional gain
            0.0001, // Integral gain
            0.90,   // Min playback speed
            1.1,    // Max playback speed
        )
    }
}

impl AudioLatencyController {
    fn new(target_latency: f32, tolerance: f32, kp: f32, ki: f32, min_speed: f32, max_speed: f32) -> Self {
        Self {
            target_latency,
            tolerance,
            playback_speed: 1.0,
            kp,
            ki,
            integral: 0.0,
            min_speed,
            max_speed,
        }
    }

    #[allow(dead_code)]
    fn speed(&self) -> f32 {
        self.playback_speed
    }

    fn update(&mut self, measured_latency: f32, _dt: f32) -> f32 {
        let error = measured_latency - self.target_latency;

        let lower_bound = self.target_latency - self.tolerance;
        let upper_bound = self.target_latency + self.tolerance;
        if measured_latency < lower_bound || measured_latency > upper_bound {
            // Proportional term
            let p_term = self.kp * error;
            //log::trace!("Error: {:.2} P-term: {}", error, p_term);

            // Integral term (accumulates over time)
            //self.integral += error * dt;
            // let i_term = self.ki * self.integral;
            let i_term = 0.0;

            // Compute new playback speed
            self.playback_speed += p_term + i_term;

            // Clamp playback speed within safe bounds
            self.playback_speed = self.playback_speed.clamp(self.min_speed, self.max_speed);
        }
        else {
            self.playback_speed = 1.0;
        }

        self.playback_speed
    }
}

#[allow(unused)]
pub struct SoundInterface {
    enabled: bool,
    device_name: String,
    master_speed: f32,
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
            master_speed: 1.0,
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
        let default_config = audio_device.default_output_config()?;

        let new_max = match default_config.buffer_size() {
            SupportedBufferSize::Range { min, .. } => {
                if *min > MAX_BUFFER_SIZE {
                    *min
                }
                else {
                    MAX_BUFFER_SIZE
                }
            }
            _ => MAX_BUFFER_SIZE,
        };
        log::debug!(
            "Device buffer size: {:?} Overriding max buffer size to: {}",
            default_config.buffer_size(),
            new_max
        );

        let config = SupportedStreamConfig::new(
            default_config.channels(),
            default_config.sample_rate(),
            SupportedBufferSize::Range { min: 0, max: new_max },
            default_config.sample_format(),
        );

        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;
        let sample_format = config.sample_format().to_string();

        let (stream, stream_handle) = rodio::OutputStream::try_from_device_config(&audio_device, config)?;

        *self = {
            SoundInterface {
                enabled: self.enabled,
                device_name,
                master_speed: 1.0,
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

    pub fn set_master_speed(&mut self, speed: f32) {
        self.master_speed = speed;

        for source in self.sources.iter_mut() {
            source.sink.set_speed(speed);
        }
    }

    pub fn add_source(&mut self, source: &SoundSourceDescriptor) -> Result<(), Error> {
        let stream_handle = self.stream_handle.as_ref().unwrap();
        let sink = Sink::try_new(stream_handle)?;
        let volume = DEFAULT_VOLUME;
        sink.set_volume(volume);

        self.sources.push(SoundSource {
            name: source.name.clone(),
            sample_rate: source.sample_rate,
            channels: source.channels as u16,
            receiver: source.receiver.clone(),
            sample_ct: 0,
            latency_ms: 0.0,
            buffer_ct: 0,
            first_buffer: None,
            sink,
            muted: false,
            volume,
            last_block_received: Instant::now(),
            controller: Default::default(),
        });

        Ok(())
    }

    pub fn run(&mut self, _duration: Duration) {
        for source in self.sources.iter_mut() {
            let samples_in = source.receiver.try_iter().collect::<Vec<f32>>();
            //log::debug!("received {} samples from channel {}", samples_in.len(), source.name);

            // Do not append an empty buffer.
            if samples_in.len() > 0 {
                let now = Instant::now();
                if source.first_buffer.is_none() {
                    source.first_buffer = Some(now);
                }
                let last_block_duration = now - source.last_block_received;
                source.last_block_received = now;
                let block_len = samples_in.len() / source.channels as usize;

                let block_duration = Duration::from_secs_f64(block_len as f64 / source.sample_rate as f64);
                // How far along is the current block?
                let mut sink_pos = source.sink.get_pos();

                if sink_pos > block_duration {
                    sink_pos = block_duration;
                }

                // Calculate the latency of the audio queue, by combining the current source position with the
                // number of buffers in the queue
                let latency = (block_duration - sink_pos)
                    + Duration::from_secs_f64(source.sink.len() as f64 * block_duration.as_secs_f64());
                let dt = last_block_duration.as_secs_f32();
                let new_speed = source.controller.update((latency.as_nanos() as f32) / 1_000_000.0, dt);

                //let effective_sample_rate = block_len as f32 / block_duration.as_secs_f32();
                let _average_sample_rate =
                    source.sample_ct as f64 / source.first_buffer.unwrap().elapsed().as_secs_f64();

                source.latency_ms = latency.as_millis() as f32;
                // log::debug!(
                //     "{}: Average sample rate: {} Latency: {}ms Speed: {:.2}",
                //     source.name,
                //     average_sample_rate,
                //     latency.as_millis(),
                //     new_speed,
                // );

                // Only push more samples if the latency is below the maximum. Latency can "run away" if the window is minimized
                if source.latency_ms < MAX_LATENCY {
                    source.sample_ct += block_len as u64;
                    let sink_buffer =
                        rodio::buffer::SamplesBuffer::new(source.channels, source.sample_rate, samples_in);
                    source.sink.append(sink_buffer);
                }
                source.sink.set_speed(new_speed * self.master_speed);
            }
        }
    }

    pub fn open_stream(&mut self) -> Result<(), Error> {
        if self.device.is_none() {
            return Err(anyhow!("No audio device open."));
        }

        let _stream = rodio::OutputStream::try_from_device(self.device.as_ref().unwrap())?;
        log::debug!("Rodio stream successfully opened.");
        Ok(())
    }

    pub fn device_name(&self) -> String {
        self.device_name.clone()
    }

    pub fn set_volume(&mut self, s_idx: usize, volume: Option<f32>, muted: Option<bool>) {
        if s_idx < self.sources.len() {
            let source = &mut self.sources[s_idx];
            let new_volume = volume.unwrap_or(source.volume);
            let mut new_sink_volume = new_volume;

            if let Some(mute_state) = muted {
                source.muted = mute_state;
                new_sink_volume = match mute_state {
                    true => 0.0,
                    false => new_volume,
                }
            }

            source.volume = new_volume;
            source.sink.set_volume(new_sink_volume);
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

    pub fn info(&self) -> Vec<SoundSourceInfo> {
        self.sources.iter().map(|s| s.info()).collect()
    }
}

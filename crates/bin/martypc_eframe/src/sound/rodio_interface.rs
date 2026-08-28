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

//! The rodio-backed sound interface used when the 'sound' feature is enabled.
#[cfg(not(target_arch = "wasm32"))]
const MAX_BUFFER_SIZE: u32 = 100;
#[cfg(target_arch = "wasm32")]
const MAX_BUFFER_SIZE: u32 = 1024;
const DEFAULT_VOLUME: f32 = 0.25;

const MAX_LATENCY: f32 = 150.0; // Maximum latency in milliseconds

use std::num::NonZero;

use marty_common::MartyHashMap;
use marty_core::{
    device_traits::sounddevice::AudioSample,
    sound::{SoundOutputConfig, SoundSourceDescriptor},
};
use marty_frontend_common::{
    sound_file_manager::{PresentableSoundKey, SoundEffect},
    types::sound::{SoundSourceInfo, SoundSourceKind},
};
use rodio::{
    buffer::SamplesBuffer,
    cpal::{traits::HostTrait, BufferSize, SupportedBufferSize},
    mixer::{self, Mixer},
    source::Zero,
    DeviceSinkBuilder,
    DeviceTrait,
    MixerDeviceSink,
    Player,
    Source,
};

use anyhow::{anyhow, Error};
use crossbeam_channel::Receiver;
use web_time::{Duration, Instant};

pub struct SoundSource {
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_rate_nz: NonZero<u32>,
    pub channels_nz: NonZero<u16>,
    pub receiver: Receiver<AudioSample>,
    pub sample_ct: u64,
    pub latency_ms: f32,
    #[allow(unused)]
    pub buffer_ct: u64,
    pub first_buffer: Option<Instant>,
    pub muted: bool,
    pub volume: f32,
    pub player: Player,
    pub last_block_received: Instant,
    pub controller: AudioLatencyController,
}

struct KeyedPresentableSound {
    player:  Player,
    looping: bool,
}

impl SoundSource {
    pub fn info(&self) -> SoundSourceInfo {
        SoundSourceInfo {
            kind: SoundSourceKind::Emulated,
            name: self.name.clone(),
            sample_rate: self.sample_rate,
            channels: self.channels,
            sample_ct: self.sample_ct,
            latency_ms: self.latency_ms,
            muted: self.muted,
            volume: self.volume,
            len: self.player.len(),
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
pub struct RodioSoundInterface {
    enabled: bool,
    device_name: String,
    master_speed: f32,
    sample_rate: u32,
    sample_format: String, // We don't really need this, so I am not converting it to an enum.
    channels: usize,
    device: Option<rodio::cpal::Device>,
    stream: Option<MixerDeviceSink>,
    // Presentable events are things like floppy drive sounds.
    presentable_event_mixer: Option<Mixer>,
    // Master output for all presentable event sounds.
    presentable_event_output: Option<Player>,
    machine_sounds_volume: f32,
    machine_sounds_muted: bool,

    looping_sounds_paused: bool,
    presentable_event_players: MartyHashMap<PresentableSoundKey, KeyedPresentableSound>,
    presentable_event_one_shots: Vec<Player>,
    sources: Vec<SoundSource>,
}

impl Default for RodioSoundInterface {
    fn default() -> Self {
        RodioSoundInterface {
            enabled: false,
            device_name: String::new(),
            master_speed: 1.0,
            sample_rate: 0,
            sample_format: String::new(),
            channels: 0,
            device: None,
            stream: None,
            presentable_event_mixer: None,
            presentable_event_output: None,
            machine_sounds_volume: super::MACHINE_SOUNDS_DEFAULT_VOLUME,
            machine_sounds_muted: false,
            looping_sounds_paused: false,
            presentable_event_players: MartyHashMap::default(),
            presentable_event_one_shots: Vec::new(),
            sources: Vec::new(),
        }
    }
}

impl RodioSoundInterface {
    pub fn new(enabled: bool) -> RodioSoundInterface {
        RodioSoundInterface {
            enabled,
            ..Default::default()
        }
    }

    pub fn open_device(&mut self) -> Result<(), Error> {
        //let audio_device = rodio::cpal::default_host().default_output_device()?;
        let audio_device = rodio::cpal::default_host()
            .default_output_device()
            .ok_or(anyhow!("No audio device found."))?;

        let device_name = audio_device.description()?.name().to_owned();
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

        let sample_rate = default_config.sample_rate();
        let channels = default_config.channels() as usize;
        let sample_format = default_config.sample_format().to_string();

        let mut stream = DeviceSinkBuilder::from_device(audio_device.clone())?
            .with_supported_config(&default_config)
            .with_buffer_size(BufferSize::Fixed(new_max))
            .open_stream()?;
        // Suppress stdout log on drop from rodio
        stream.log_on_drop(false);

        let channels_nz = NonZero::new(default_config.channels()).ok_or(anyhow!("Audio device has zero channels."))?;
        let sample_rate_nz = NonZero::new(sample_rate).ok_or(anyhow!("Audio device has a zero sample rate."))?;

        // Create a mixer for presentable events - floppy drive sounds, etc.
        let (presentable_event_mixer, presentable_event_mixer_source) = mixer::mixer(channels_nz, sample_rate_nz);
        presentable_event_mixer.add(Zero::new(channels_nz, sample_rate_nz));
        let presentable_event_output = Player::connect_new(stream.mixer());
        presentable_event_output.set_volume(if self.machine_sounds_muted {
            0.0
        }
        else {
            self.machine_sounds_volume
        });
        presentable_event_output.append(presentable_event_mixer_source);

        *self = {
            RodioSoundInterface {
                enabled: self.enabled,
                device_name,
                master_speed: 1.0,
                sample_rate,
                sample_format,
                channels,
                device: Some(audio_device),
                stream: Some(stream),
                presentable_event_mixer: Some(presentable_event_mixer),
                presentable_event_output: Some(presentable_event_output),
                machine_sounds_volume: self.machine_sounds_volume,
                machine_sounds_muted: self.machine_sounds_muted,
                looping_sounds_paused: self.looping_sounds_paused,
                presentable_event_players: MartyHashMap::default(),
                presentable_event_one_shots: Vec::new(),
                sources: Vec::new(),
            }
        };

        Ok(())
    }

    fn samples_buffer(samples: &[f32], sample_rate: u32, stereo: bool) -> Result<SamplesBuffer, Error> {
        if stereo && !samples.len().is_multiple_of(2) {
            return Err(anyhow!("Stereo sound contains an odd number of samples."));
        }

        let channels = NonZero::new(if stereo { 2 } else { 1 }).ok_or(anyhow!("Sound has zero channels."))?;
        let sample_rate = NonZero::new(sample_rate).ok_or(anyhow!("Sound has a zero sample rate."))?;

        Ok(SamplesBuffer::new(channels, sample_rate, samples.to_vec()))
    }

    fn cancel_presentable_sound(&mut self, key: PresentableSoundKey) {
        if let Some(sound) = self.presentable_event_players.remove(&key) {
            sound.player.stop();
        }
    }

    /// Plays samples through the mixer reserved for presentable frontend events.
    ///
    /// Stereo samples must be interleaved left/right pairs. Rodio resamples
    /// the source to the output device's sample rate when necessary.
    pub fn play_sound(&mut self, samples: &[f32], sample_rate: u32, stereo: bool) -> Result<(), Error> {
        if samples.is_empty() {
            return Ok(());
        }

        let mixer = self
            .presentable_event_mixer
            .as_ref()
            .ok_or(anyhow!("No audio stream open."))?;
        let source = Self::samples_buffer(samples, sample_rate, stereo)?;
        let player = Player::connect_new(mixer);
        player.append(source);
        self.presentable_event_one_shots.push(player);

        Ok(())
    }

    pub fn device_reset(&mut self) {
        for (_, sound) in self.presentable_event_players.drain() {
            sound.player.stop();
        }
        for player in self.presentable_event_one_shots.drain(..) {
            player.stop();
        }
    }

    pub fn set_looping_sounds_paused(&mut self, paused: bool) {
        if self.looping_sounds_paused == paused {
            return;
        }

        self.looping_sounds_paused = paused;
        for sound in self.presentable_event_players.values() {
            if sound.looping {
                if paused {
                    sound.player.pause();
                }
                else {
                    sound.player.play();
                }
            }
        }
    }

    pub fn start_loop(
        &mut self,
        key: PresentableSoundKey,
        intro: &SoundEffect,
        looping: &SoundEffect,
    ) -> Result<(), Error> {
        if looping.samples.is_empty() {
            return Err(anyhow!("Looping sound '{}' contains no samples.", looping.base_name));
        }

        let mixer = self
            .presentable_event_mixer
            .clone()
            .ok_or(anyhow!("No audio stream open."))?;
        // This also cancels a queued or currently playing motor-stop sound when
        // the motor is turned back on before the stop sound has completed.
        self.cancel_presentable_sound(key);

        let player = Player::connect_new(&mixer);
        if !intro.samples.is_empty() {
            player.append(Self::samples_buffer(&intro.samples, intro.sample_rate, intro.stereo)?);
        }
        player.append(Self::samples_buffer(&looping.samples, looping.sample_rate, looping.stereo)?.repeat_infinite());
        if self.looping_sounds_paused {
            player.pause();
        }
        self.presentable_event_players
            .insert(key, KeyedPresentableSound { player, looping: true });

        Ok(())
    }

    pub fn stop_loop(&mut self, key: PresentableSoundKey, outro: &SoundEffect) -> Result<(), Error> {
        self.cancel_presentable_sound(key);

        if outro.samples.is_empty() {
            return Ok(());
        }

        let mixer = self
            .presentable_event_mixer
            .clone()
            .ok_or(anyhow!("No audio stream open."))?;
        let player = Player::connect_new(&mixer);
        player.append(Self::samples_buffer(&outro.samples, outro.sample_rate, outro.stereo)?);
        self.presentable_event_players
            .insert(key, KeyedPresentableSound { player, looping: false });

        Ok(())
    }

    pub fn set_master_speed(&mut self, speed: f32) {
        self.master_speed = speed;

        for source in self.sources.iter_mut() {
            source.player.set_speed(speed);
        }
    }

    pub fn add_source(&mut self, source: &SoundSourceDescriptor) -> Result<(), Error> {
        let stream = self.stream.as_ref().ok_or(anyhow!("No audio stream open."))?;
        let player = Player::connect_new(stream.mixer());
        let volume = DEFAULT_VOLUME;
        player.set_volume(if source.initially_muted { 0.0 } else { volume });

        let channels = u16::try_from(source.channels)?;
        let channels_nz = NonZero::new(channels).ok_or(anyhow!("Sound source has zero channels."))?;
        let sample_rate_nz = NonZero::new(source.sample_rate).ok_or(anyhow!("Sound source has zero sample rate."))?;

        self.sources.push(SoundSource {
            name: source.name.clone(),
            sample_rate: source.sample_rate,
            channels,
            sample_rate_nz,
            channels_nz,
            receiver: source.receiver.clone(),
            sample_ct: 0,
            latency_ms: 0.0,
            buffer_ct: 0,
            first_buffer: None,
            player,
            muted: source.initially_muted,
            volume,
            last_block_received: Instant::now(),
            controller: Default::default(),
        });

        Ok(())
    }

    pub fn run(&mut self, _duration: Duration) {
        self.presentable_event_players.retain(|_, sound| !sound.player.empty());
        self.presentable_event_one_shots.retain(|player| !player.empty());

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
                let mut sink_pos = source.player.get_pos();

                if sink_pos > block_duration {
                    sink_pos = block_duration;
                }

                // Calculate the latency of the audio queue, by combining the current source position with the
                // number of buffers in the queue
                let latency = (block_duration - sink_pos)
                    + Duration::from_secs_f64(source.player.len() as f64 * block_duration.as_secs_f64());
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
                    let sink_buffer = SamplesBuffer::new(source.channels_nz, source.sample_rate_nz, samples_in);
                    source.player.append(sink_buffer);
                }
                source.player.set_speed(new_speed * self.master_speed);
            }
        }
    }

    pub fn open_stream(&mut self) -> Result<(), Error> {
        if self.stream.is_none() {
            return Err(anyhow!("No audio stream open."));
        }

        log::debug!("Rodio stream successfully opened.");
        Ok(())
    }

    pub fn device_name(&self) -> String {
        self.device_name.clone()
    }

    pub fn set_volume(&mut self, s_idx: usize, volume: Option<f32>, muted: Option<bool>) {
        if s_idx == 0 {
            self.machine_sounds_volume = volume.unwrap_or(self.machine_sounds_volume);
            self.machine_sounds_muted = muted.unwrap_or(self.machine_sounds_muted);

            if let Some(output) = self.presentable_event_output.as_ref() {
                output.set_volume(if self.machine_sounds_muted {
                    0.0
                }
                else {
                    self.machine_sounds_volume
                });
            }
        }
        else if let Some(source) = self.sources.get_mut(s_idx - 1) {
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
            source.player.set_volume(new_sink_volume);
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
        let mut info = vec![super::machine_sounds_info(
            self.sample_rate,
            self.channels,
            self.machine_sounds_volume,
            self.machine_sounds_muted,
            self.presentable_event_players.len() + self.presentable_event_one_shots.len(),
        )];
        info.extend(self.sources.iter().map(|s| s.info()));
        info
    }
}

impl super::SoundInterfaceBackend for RodioSoundInterface {
    fn new(enabled: bool) -> Self {
        RodioSoundInterface::new(enabled)
    }

    fn open_device(&mut self) -> Result<(), Error> {
        RodioSoundInterface::open_device(self)
    }

    fn open_stream(&mut self) -> Result<(), Error> {
        RodioSoundInterface::open_stream(self)
    }

    fn device_name(&self) -> String {
        RodioSoundInterface::device_name(self)
    }

    fn add_source(&mut self, source: &SoundSourceDescriptor) -> Result<(), Error> {
        RodioSoundInterface::add_source(self, source)
    }

    fn run(&mut self, duration: Duration) {
        RodioSoundInterface::run(self, duration)
    }

    fn set_master_speed(&mut self, speed: f32) {
        RodioSoundInterface::set_master_speed(self, speed)
    }

    fn set_volume(&mut self, source_index: usize, volume: Option<f32>, muted: Option<bool>) {
        RodioSoundInterface::set_volume(self, source_index, volume, muted)
    }

    fn config(&self) -> SoundOutputConfig {
        RodioSoundInterface::config(self)
    }

    fn info(&self) -> Vec<SoundSourceInfo> {
        RodioSoundInterface::info(self)
    }

    fn play_sound(&mut self, samples: &[f32], sample_rate: u32, stereo: bool) -> Result<(), Error> {
        RodioSoundInterface::play_sound(self, samples, sample_rate, stereo)
    }

    fn device_reset(&mut self) {
        RodioSoundInterface::device_reset(self)
    }

    fn set_looping_sounds_paused(&mut self, paused: bool) {
        RodioSoundInterface::set_looping_sounds_paused(self, paused)
    }

    fn start_loop(
        &mut self,
        key: PresentableSoundKey,
        intro: &SoundEffect,
        looping: &SoundEffect,
    ) -> Result<(), Error> {
        RodioSoundInterface::start_loop(self, key, intro, looping)
    }

    fn stop_loop(&mut self, key: PresentableSoundKey, outro: &SoundEffect) -> Result<(), Error> {
        RodioSoundInterface::stop_loop(self, key, outro)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound::SoundInterfaceBackend;

    fn effect(base_name: &str, samples: Vec<f32>) -> SoundEffect {
        SoundEffect {
            base_name: base_name.to_string(),
            samples,
            stereo: false,
            sample_rate: 44_100,
        }
    }

    #[test]
    fn motor_restart_replaces_queued_stop_sound() {
        let (mixer, _mixer_source) =
            mixer::mixer(NonZero::<u16>::new(2).unwrap(), NonZero::<u32>::new(44_100).unwrap());
        let mut interface = RodioSoundInterface {
            presentable_event_mixer: Some(mixer),
            ..Default::default()
        };
        let key = PresentableSoundKey::FloppyDriveMotor {
            controller: 0,
            drive: 0,
        };
        let stop = effect("drive_motor_stop", vec![-1.0; 16]);
        let start = effect("drive_motor_start", vec![0.5; 16]);
        let looping = effect("drive_motor_on", vec![0.25; 16]);

        interface.stop_loop(key, &stop).unwrap();
        assert_eq!(interface.presentable_event_players[&key].player.len(), 1);

        interface.start_loop(key, &start, &looping).unwrap();
        assert_eq!(interface.presentable_event_players.len(), 1);
        assert_eq!(interface.presentable_event_players[&key].player.len(), 2);
    }

    #[test]
    fn execution_pause_only_affects_looping_presentable_sounds() {
        let (mixer, _mixer_source) =
            mixer::mixer(NonZero::<u16>::new(2).unwrap(), NonZero::<u32>::new(44_100).unwrap());
        let mut interface = RodioSoundInterface {
            presentable_event_mixer: Some(mixer),
            ..Default::default()
        };
        let key = PresentableSoundKey::FloppyDriveMotor {
            controller: 0,
            drive: 0,
        };
        let start = effect("drive_motor_start", vec![0.5; 16]);
        let looping = effect("drive_motor_on", vec![0.25; 16]);
        let stop = effect("drive_motor_stop", vec![-1.0; 16]);

        interface.play_sound(&[1.0; 16], 44_100, false).unwrap();
        interface.start_loop(key, &start, &looping).unwrap();
        interface.set_looping_sounds_paused(true);

        assert!(interface.presentable_event_players[&key].player.is_paused());
        assert!(!interface.presentable_event_one_shots[0].is_paused());

        interface.set_looping_sounds_paused(false);
        assert!(!interface.presentable_event_players[&key].player.is_paused());

        interface.set_looping_sounds_paused(true);
        interface.start_loop(key, &start, &looping).unwrap();
        assert!(interface.presentable_event_players[&key].player.is_paused());

        interface.stop_loop(key, &stop).unwrap();
        assert!(!interface.presentable_event_players[&key].looping);
        assert!(!interface.presentable_event_players[&key].player.is_paused());
    }

    #[test]
    fn device_reset_stops_all_presentable_event_players() {
        let (mixer, _mixer_source) =
            mixer::mixer(NonZero::<u16>::new(2).unwrap(), NonZero::<u32>::new(44_100).unwrap());
        let mut interface = RodioSoundInterface {
            presentable_event_mixer: Some(mixer),
            ..Default::default()
        };
        let key = PresentableSoundKey::FloppyDriveMotor {
            controller: 0,
            drive: 0,
        };
        let start = effect("drive_motor_start", vec![0.5; 16]);
        let looping = effect("drive_motor_on", vec![0.25; 16]);

        interface.play_sound(&[1.0; 16], 44_100, false).unwrap();
        interface.start_loop(key, &start, &looping).unwrap();
        interface.set_looping_sounds_paused(true);
        assert_eq!(interface.presentable_event_one_shots.len(), 1);
        assert_eq!(interface.presentable_event_players.len(), 1);

        interface.device_reset();
        assert!(interface.presentable_event_one_shots.is_empty());
        assert!(interface.presentable_event_players.is_empty());
    }

    #[test]
    fn machine_sounds_are_listed_first_and_control_presentable_event_output() {
        let (mixer, _mixer_source) =
            mixer::mixer(NonZero::<u16>::new(2).unwrap(), NonZero::<u32>::new(44_100).unwrap());
        let output = Player::connect_new(&mixer);
        let mut interface = RodioSoundInterface {
            sample_rate: 44_100,
            channels: 2,
            presentable_event_output: Some(output),
            ..Default::default()
        };

        let machine_sounds_index = 0;
        interface.set_volume(machine_sounds_index, Some(0.7), Some(true));

        let info = interface.info();
        let machine_sounds = info.first().unwrap();
        assert_eq!(machine_sounds.kind, SoundSourceKind::MachineSounds);
        assert_eq!(machine_sounds.name, super::super::MACHINE_SOUNDS_NAME);
        assert_eq!(machine_sounds.sample_rate, 44_100);
        assert_eq!(machine_sounds.channels, 2);
        assert_eq!(machine_sounds.volume, 0.7);
        assert!(machine_sounds.muted);
        assert_eq!(interface.presentable_event_output.as_ref().unwrap().volume(), 0.0);
        assert_eq!(
            interface
                .source_by_name(super::super::MACHINE_SOUNDS_NAME)
                .map(|(source_index, _)| source_index),
            Some(machine_sounds_index)
        );
        assert!(interface.source_by_name("Missing Source").is_none());

        interface.set_volume(machine_sounds_index, None, Some(false));
        assert_eq!(interface.presentable_event_output.as_ref().unwrap().volume(), 0.7);
    }
}

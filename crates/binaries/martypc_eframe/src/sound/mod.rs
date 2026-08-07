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

    sound_player.rs

    Implement the sound player interface.

*/

mod null_interface;
#[cfg(feature = "sound")]
mod rodio_interface;

use anyhow::Error;
use marty_core::sound::SoundOutputConfig;
use marty_frontend_common::{
    sound_file_manager::{PresentableSoundKey, SoundEffect},
    types::sound::{SoundSourceInfo, SoundSourceKind},
};
use web_time::Duration;

const MACHINE_SOUNDS_NAME: &str = "Machine Sounds";
const MACHINE_SOUNDS_DEFAULT_VOLUME: f32 = 0.5;

fn machine_sounds_info(sample_rate: u32, channels: usize, volume: f32, muted: bool, len: usize) -> SoundSourceInfo {
    SoundSourceInfo {
        kind: SoundSourceKind::MachineSounds,
        name: MACHINE_SOUNDS_NAME.to_string(),
        sample_rate,
        channels: u16::try_from(channels).unwrap_or(u16::MAX),
        sample_ct: 0,
        latency_ms: 0.0,
        volume,
        muted,
        len,
    }
}

#[cfg(not(feature = "sound"))]
pub use null_interface::NullSoundInterface as SoundInterface;

pub use marty_core::sound::SoundSourceDescriptor;
#[cfg(feature = "sound")]
pub use rodio_interface::RodioSoundInterface as SoundInterface;

pub trait SoundInterfaceBackend: Default {
    fn new(enabled: bool) -> Self
    where
        Self: Sized;

    fn open_device(&mut self) -> Result<(), Error>;
    fn open_stream(&mut self) -> Result<(), Error>;
    fn device_name(&self) -> String;
    fn add_source(&mut self, source: &SoundSourceDescriptor) -> Result<(), Error>;
    fn run(&mut self, duration: Duration);
    fn set_master_speed(&mut self, speed: f32);
    fn set_volume(&mut self, source_index: usize, volume: Option<f32>, muted: Option<bool>);
    fn config(&self) -> SoundOutputConfig;
    fn info(&self) -> Vec<SoundSourceInfo>;
    fn play_sound(&mut self, samples: &[f32], sample_rate: u32, stereo: bool) -> Result<(), Error>;
    fn device_reset(&mut self);
    fn set_looping_sounds_paused(&mut self, paused: bool);
    fn start_loop(&mut self, key: PresentableSoundKey, intro: &SoundEffect, looping: &SoundEffect)
        -> Result<(), Error>;
    fn stop_loop(&mut self, key: PresentableSoundKey, outro: &SoundEffect) -> Result<(), Error>;
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "sound")]
    use super::rodio_interface;
    use super::{null_interface, SoundInterfaceBackend};

    fn assert_sound_interface<T: SoundInterfaceBackend>() {}

    #[test]
    fn all_backends_implement_sound_interface() {
        assert_sound_interface::<null_interface::NullSoundInterface>();
        #[cfg(feature = "sound")]
        assert_sound_interface::<rodio_interface::RodioSoundInterface>();
    }
}

/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the "Software"),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.

    --------------------------------------------------------------------------
*/

//! The SoundFileManager is responsible for loading and resolution of sound effects used by
//! frontend device events.
//!
//! At the moment this is just floppy drive sound effects, but more are possible in the future.

use std::{
    fmt,
    io::{Cursor, Read},
    path::PathBuf,
};

use anyhow::{anyhow, Context, Error};
use hound::{SampleFormat, WavReader};
use marty_common::{FloppyDriveEvent, MartyHashMap, PresentableDeviceEvent};
use zip::ZipArchive;

use crate::{
    asset_manager::{AssetManager, SoundLibrary},
    resource_manager::ResourceManager,
};

pub const FLOPPY_DRIVE_SOUNDS_SUBTYPE: &str = "FloppyDriveSounds";
const DRIVE_MOTOR_ON_SOUND: &str = "drive_motor_on";
const DRIVE_MOTOR_START_SOUND: &str = "drive_motor_start";
const DRIVE_MOTOR_STOP_SOUND: &str = "drive_motor_stop";
const EMPTY_DRIVE_MOTOR_ON_SOUND: &str = "empty_drive_motor_on";
const EMPTY_DRIVE_MOTOR_START_SOUND: &str = "empty_drive_motor_start";
const EMPTY_DRIVE_MOTOR_OFF_SOUND: &str = "empty_drive_motor_off";
const DISK_INSERT_SOUND: &str = "insert_disk";
const DISK_EJECT_SOUND: &str = "eject_disk";

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct SoundFileManagerOptions {
    pub floppy_sounds: bool,
}

#[derive(Clone, PartialEq)]
pub struct SoundEffect {
    pub base_name: String,
    pub samples: Vec<f32>,
    pub stereo: bool,
    pub sample_rate: u32,
}

impl fmt::Debug for SoundEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SoundEffect")
            .field("base_name", &self.base_name)
            .field("sample_count", &self.samples.len())
            .field("stereo", &self.stereo)
            .field("sample_rate", &self.sample_rate)
            .finish()
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum PresentableSoundKey {
    FloppyDriveMotor { controller: u8, drive: u8 },
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PresentableSoundAction<'a> {
    OneShot(&'a SoundEffect),
    StartLoop {
        key: PresentableSoundKey,
        intro: &'a SoundEffect,
        looping: &'a SoundEffect,
    },
    StopLoop {
        key:   PresentableSoundKey,
        outro: &'a SoundEffect,
    },
}

pub struct SoundFileManager {
    options: SoundFileManagerOptions,
    effects: MartyHashMap<String, SoundEffect>,
    sound_library: Option<SoundLibrary>,
}

impl SoundFileManager {
    pub fn new(options: SoundFileManagerOptions) -> Self {
        Self {
            options,
            effects: MartyHashMap::default(),
            sound_library: None,
        }
    }

    pub async fn load_assets(
        &mut self,
        asset_manager: &AssetManager,
        rm: &mut ResourceManager,
    ) -> Result<usize, Error> {
        self.effects.clear();
        self.sound_library = None;

        if !self.options.floppy_sounds {
            return Ok(0);
        }

        let Some(library) = asset_manager
            .sound_libraries()
            .find(|library| library.manifest.asset_subtype == FLOPPY_DRIVE_SOUNDS_SUBTYPE)
            .cloned()
        else {
            log::debug!("No {} sound library was discovered.", FLOPPY_DRIVE_SOUNDS_SUBTYPE);
            return Ok(0);
        };

        let archive_data = rm
            .read_resource_from_path(&library.path)
            .await
            .with_context(|| format!("Failed to read sound library {}", library.path.display()))?;
        let effect_count = self.load_sound_library(&library, archive_data)?;
        log::debug!(
            "Loaded {} sound effect(s) from '{}' ({})",
            effect_count,
            library.manifest.asset_name,
            library.path.display()
        );
        self.sound_library = Some(library);

        Ok(effect_count)
    }

    fn load_sound_library(&mut self, library: &SoundLibrary, archive_data: Vec<u8>) -> Result<usize, Error> {
        let mut archive = ZipArchive::new(Cursor::new(archive_data))
            .with_context(|| format!("Failed to open sound library {}", library.path.display()))?;

        for index in 0..archive.len() {
            let mut file = archive
                .by_index(index)
                .with_context(|| format!("Failed to read entry {} from {}", index, library.path.display()))?;
            if file.is_dir() {
                continue;
            }

            let entry_path = PathBuf::from(file.name());
            let is_wav = entry_path
                .extension()
                .is_some_and(|extension| extension.to_string_lossy().eq_ignore_ascii_case("wav"));
            if !is_wav {
                continue;
            }

            let Some(base_filename) = entry_path
                .file_stem()
                .and_then(|filename| filename.to_str())
                .map(str::to_owned)
            else {
                log::warn!(
                    "Skipping sound with invalid filename '{}' in {}",
                    entry_path.display(),
                    library.path.display()
                );
                continue;
            };

            let mut wav_data = Vec::with_capacity(file.size() as usize);
            if let Err(err) = file.read_to_end(&mut wav_data) {
                log::warn!(
                    "Failed to read sound '{}' from {}: {}",
                    entry_path.display(),
                    library.path.display(),
                    err
                );
                continue;
            }

            match Self::decode_wav(&wav_data, base_filename.clone()) {
                Ok(effect) => {
                    if self.effects.insert(base_filename.clone(), effect).is_some() {
                        log::warn!(
                            "Sound library '{}' contains duplicate base filename '{}'; using the last entry.",
                            library.manifest.asset_name,
                            base_filename
                        );
                    }
                }
                Err(err) => {
                    log::warn!(
                        "Failed to decode sound '{}' from {}: {}",
                        entry_path.display(),
                        library.path.display(),
                        err
                    );
                }
            }
        }

        Ok(self.effects.len())
    }

    fn decode_wav(data: &[u8], base_name: String) -> Result<SoundEffect, Error> {
        let mut reader = WavReader::new(Cursor::new(data)).context("Failed to open WAV data")?;
        let spec = reader.spec();
        if !matches!(spec.channels, 1 | 2) {
            return Err(anyhow!("Unsupported WAV channel count: {}", spec.channels));
        }

        let samples = match spec.sample_format {
            SampleFormat::Float => {
                if spec.bits_per_sample != 32 {
                    return Err(anyhow!(
                        "Unsupported floating-point WAV sample width: {}",
                        spec.bits_per_sample
                    ));
                }
                reader
                    .samples::<f32>()
                    .collect::<Result<Vec<_>, _>>()
                    .context("Failed to decode floating-point WAV samples")?
            }
            SampleFormat::Int => match spec.bits_per_sample {
                1..=8 => {
                    let scale = (1u32 << (spec.bits_per_sample - 1)) as f32;
                    reader
                        .samples::<i8>()
                        .map(|sample| sample.map(|sample| sample as f32 / scale))
                        .collect::<Result<Vec<_>, _>>()
                        .context("Failed to decode 8-bit WAV samples")?
                }
                9..=16 => {
                    let scale = (1u32 << (spec.bits_per_sample - 1)) as f32;
                    reader
                        .samples::<i16>()
                        .map(|sample| sample.map(|sample| sample as f32 / scale))
                        .collect::<Result<Vec<_>, _>>()
                        .context("Failed to decode 16-bit WAV samples")?
                }
                17..=32 => {
                    let scale = (1u64 << (spec.bits_per_sample - 1)) as f32;
                    reader
                        .samples::<i32>()
                        .map(|sample| sample.map(|sample| sample as f32 / scale))
                        .collect::<Result<Vec<_>, _>>()
                        .context("Failed to decode 24/32-bit WAV samples")?
                }
                bits => return Err(anyhow!("Unsupported integer WAV sample width: {}", bits)),
            },
        };

        Ok(SoundEffect {
            base_name,
            samples,
            stereo: spec.channels == 2,
            sample_rate: spec.sample_rate,
        })
    }

    pub fn options(&self) -> SoundFileManagerOptions {
        self.options
    }

    pub fn effects(&self) -> &MartyHashMap<String, SoundEffect> {
        &self.effects
    }

    pub fn effect(&self, base_filename: &str) -> Option<&SoundEffect> {
        self.effects.get(base_filename)
    }

    fn resolve_seek_effect(&self, direction: &str, step_count: u8) -> Option<&SoundEffect> {
        let exact_name = format!("seek_{direction}_{step_count:02}");
        if let Some(effect) = self.effect(&exact_name) {
            return Some(effect);
        }

        let prefix = format!("seek_{direction}_");
        let (max_step_count, fallback) = self
            .effects
            .iter()
            .filter_map(|(name, effect)| {
                let sample_step_count = name.strip_prefix(&prefix)?.parse::<u8>().ok()?;
                Some((sample_step_count, effect))
            })
            .max_by_key(|(sample_step_count, _)| *sample_step_count)?;

        if step_count > max_step_count {
            log::trace!(
                "Seek sound '{}' not found; using largest available {} seek sample '{}'.",
                exact_name,
                direction,
                fallback.base_name
            );
            Some(fallback)
        }
        else {
            None
        }
    }

    pub fn sound_library(&self) -> Option<&SoundLibrary> {
        self.sound_library.as_ref()
    }

    /// resolve_presentable_event() receives PresentableDeviceEvents and returns
    /// Option<PresentableAction>, ie, what should be done, if anything, in response to the
    /// presentable event.
    /// In the initial implementation, the only PresentableDeviceEvent is of type FloppyDrive and
    /// is used to generate sound effects.
    pub fn resolve_presentable_event(&self, event: PresentableDeviceEvent) -> Option<PresentableSoundAction<'_>> {
        match event {
            PresentableDeviceEvent::FloppyDrive {
                event:
                    FloppyDriveEvent::HeadStep {
                        from_cylinder,
                        to_cylinder,
                    },
                ..
            } => {
                let direction = match to_cylinder.cmp(&from_cylinder) {
                    std::cmp::Ordering::Greater => "in",
                    std::cmp::Ordering::Less => "out",
                    std::cmp::Ordering::Equal => return None,
                };
                let step_ct = from_cylinder.abs_diff(to_cylinder);

                self.resolve_seek_effect(direction, step_ct)
                    .map(PresentableSoundAction::OneShot)
            }
            PresentableDeviceEvent::FloppyDrive {
                controller,
                drive,
                event: FloppyDriveEvent::MotorStarted { media_present },
            } => {
                let (intro_name, loop_name) = if media_present {
                    (DRIVE_MOTOR_START_SOUND, DRIVE_MOTOR_ON_SOUND)
                }
                else {
                    (EMPTY_DRIVE_MOTOR_START_SOUND, EMPTY_DRIVE_MOTOR_ON_SOUND)
                };

                Some(PresentableSoundAction::StartLoop {
                    key: PresentableSoundKey::FloppyDriveMotor { controller, drive },
                    intro: self.effect(intro_name)?,
                    looping: self.effect(loop_name)?,
                })
            }
            PresentableDeviceEvent::FloppyDrive {
                controller,
                drive,
                event: FloppyDriveEvent::MotorStopped { media_present },
            } => Some(PresentableSoundAction::StopLoop {
                key:   PresentableSoundKey::FloppyDriveMotor { controller, drive },
                outro: self.effect(if media_present {
                    DRIVE_MOTOR_STOP_SOUND
                }
                else {
                    EMPTY_DRIVE_MOTOR_OFF_SOUND
                })?,
            }),
            PresentableDeviceEvent::FloppyDrive {
                event: FloppyDriveEvent::MediaInserted,
                ..
            } => self.effect(DISK_INSERT_SOUND).map(PresentableSoundAction::OneShot),
            PresentableDeviceEvent::FloppyDrive {
                event: FloppyDriveEvent::MediaEjected,
                ..
            } => self.effect(DISK_EJECT_SOUND).map(PresentableSoundAction::OneShot),
            PresentableDeviceEvent::FloppyDrive { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::asset_manager::{AssetManifest, AssetType};
    use zip::{write::SimpleFileOptions, ZipWriter};

    fn pcm16_wav(channels: u16, sample_rate: u32, samples: &[i16]) -> Vec<u8> {
        let data_len = std::mem::size_of_val(samples) as u32;
        let byte_rate = sample_rate * channels as u32 * size_of::<i16>() as u32;
        let block_align = channels * size_of::<i16>() as u16;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        wav
    }

    fn sound_library() -> SoundLibrary {
        SoundLibrary {
            path: PathBuf::from("fd55.zip"),
            manifest: AssetManifest {
                asset_type: AssetType::SoundLibrary,
                asset_subtype: FLOPPY_DRIVE_SOUNDS_SUBTYPE.to_string(),
                asset_name: "TEAC FD-55 Sound Effects".to_string(),
                asset_specifier: "TEAC FD-55".to_string(),
            },
        }
    }

    #[test]
    fn loads_wav_files_by_base_filename() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("effects/seek_step.wav", SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(&pcm16_wav(1, 22_050, &[i16::MIN, 0, i16::MAX]))
            .unwrap();
        let archive_data = writer.finish().unwrap().into_inner();

        let mut manager = SoundFileManager::new(SoundFileManagerOptions { floppy_sounds: true });
        assert_eq!(manager.load_sound_library(&sound_library(), archive_data).unwrap(), 1);

        let effect = manager.effect("seek_step").unwrap();
        assert_eq!(effect.base_name, "seek_step");
        assert!(!effect.stereo);
        assert_eq!(effect.sample_rate, 22_050);
        assert_eq!(effect.samples.len(), 3);
        assert_eq!(effect.samples[0], -1.0);
        assert_eq!(effect.samples[1], 0.0);
        assert!(effect.samples[2] < 1.0);
    }

    fn test_effect(base_name: &str) -> SoundEffect {
        SoundEffect {
            base_name: base_name.to_string(),
            samples: vec![0.0],
            stereo: false,
            sample_rate: 22_050,
        }
    }

    fn head_step(from_cylinder: u8, to_cylinder: u8) -> PresentableDeviceEvent {
        PresentableDeviceEvent::FloppyDrive {
            controller: 0,
            drive: 0,
            event: FloppyDriveEvent::HeadStep {
                from_cylinder,
                to_cylinder,
            },
        }
    }

    fn floppy_event(controller: u8, drive: u8, event: FloppyDriveEvent) -> PresentableDeviceEvent {
        PresentableDeviceEvent::FloppyDrive {
            controller,
            drive,
            event,
        }
    }

    #[test]
    fn sound_effect_debug_output_omits_samples() {
        let effect = test_effect("seek_in_01");

        assert_eq!(
            format!("{effect:?}"),
            "SoundEffect { base_name: \"seek_in_01\", sample_count: 1, stereo: false, sample_rate: 22050 }"
        );
    }

    #[test]
    fn resolves_inward_seek_by_step_count() {
        let mut manager = SoundFileManager::new(SoundFileManagerOptions::default());
        manager
            .effects
            .insert("seek_in_05".to_string(), test_effect("seek_in_05"));

        let Some(PresentableSoundAction::OneShot(effect)) = manager.resolve_presentable_event(head_step(4, 9))
        else {
            panic!("Expected an inward seek one-shot");
        };
        assert_eq!(effect.base_name, "seek_in_05");
    }

    #[test]
    fn resolves_outward_seek_by_step_count() {
        let mut manager = SoundFileManager::new(SoundFileManagerOptions::default());
        manager
            .effects
            .insert("seek_out_12".to_string(), test_effect("seek_out_12"));
        manager
            .effects
            .insert("seek_out_39".to_string(), test_effect("seek_out_39"));

        let Some(PresentableSoundAction::OneShot(effect)) = manager.resolve_presentable_event(head_step(20, 8))
        else {
            panic!("Expected an outward seek one-shot");
        };
        assert_eq!(effect.base_name, "seek_out_12");
    }

    #[test]
    fn outward_seek_overflow_uses_largest_outward_sample() {
        let mut manager = SoundFileManager::new(SoundFileManagerOptions::default());
        for name in ["seek_out_12", "seek_out_39", "seek_in_40"] {
            manager.effects.insert(name.to_string(), test_effect(name));
        }

        let Some(PresentableSoundAction::OneShot(effect)) = manager.resolve_presentable_event(head_step(54, 12))
        else {
            panic!("Expected an outward seek overflow one-shot");
        };
        assert_eq!(effect.base_name, "seek_out_39");
    }

    #[test]
    fn inward_seek_overflow_uses_largest_inward_sample() {
        let mut manager = SoundFileManager::new(SoundFileManagerOptions::default());
        for name in ["seek_in_31", "seek_out_39"] {
            manager.effects.insert(name.to_string(), test_effect(name));
        }

        let Some(PresentableSoundAction::OneShot(effect)) = manager.resolve_presentable_event(head_step(12, 54))
        else {
            panic!("Expected an inward seek overflow one-shot");
        };
        assert_eq!(effect.base_name, "seek_in_31");
    }

    #[test]
    fn missing_in_range_seek_sample_does_not_use_overflow_fallback() {
        let mut manager = SoundFileManager::new(SoundFileManagerOptions::default());
        for name in ["seek_out_04", "seek_out_12"] {
            manager.effects.insert(name.to_string(), test_effect(name));
        }

        assert!(manager.resolve_presentable_event(head_step(10, 5)).is_none());
    }

    #[test]
    fn seek_without_directional_samples_does_not_resolve() {
        let manager = SoundFileManager::new(SoundFileManagerOptions::default());

        assert!(manager.resolve_presentable_event(head_step(54, 12)).is_none());
    }

    #[test]
    fn ignores_head_step_without_movement() {
        let mut manager = SoundFileManager::new(SoundFileManagerOptions::default());
        manager
            .effects
            .insert("seek_in_00".to_string(), test_effect("seek_in_00"));

        assert!(manager.resolve_presentable_event(head_step(8, 8)).is_none());
    }

    #[test]
    fn resolves_motor_start_to_intro_and_loop() {
        let mut manager = SoundFileManager::new(SoundFileManagerOptions::default());
        manager.effects.insert(
            DRIVE_MOTOR_START_SOUND.to_string(),
            test_effect(DRIVE_MOTOR_START_SOUND),
        );
        manager
            .effects
            .insert(DRIVE_MOTOR_ON_SOUND.to_string(), test_effect(DRIVE_MOTOR_ON_SOUND));

        let Some(PresentableSoundAction::StartLoop { key, intro, looping }) = manager.resolve_presentable_event(
            floppy_event(1, 2, FloppyDriveEvent::MotorStarted { media_present: true }),
        )
        else {
            panic!("Expected a motor start loop");
        };

        assert_eq!(
            key,
            PresentableSoundKey::FloppyDriveMotor {
                controller: 1,
                drive: 2,
            }
        );
        assert_eq!(intro.base_name, DRIVE_MOTOR_START_SOUND);
        assert_eq!(looping.base_name, DRIVE_MOTOR_ON_SOUND);
    }

    #[test]
    fn resolves_motor_stop_to_loop_stop_and_outro() {
        let mut manager = SoundFileManager::new(SoundFileManagerOptions::default());
        manager
            .effects
            .insert(DRIVE_MOTOR_STOP_SOUND.to_string(), test_effect(DRIVE_MOTOR_STOP_SOUND));

        let Some(PresentableSoundAction::StopLoop { key, outro }) = manager.resolve_presentable_event(floppy_event(
            3,
            1,
            FloppyDriveEvent::MotorStopped { media_present: true },
        ))
        else {
            panic!("Expected a motor stop action");
        };

        assert_eq!(
            key,
            PresentableSoundKey::FloppyDriveMotor {
                controller: 3,
                drive: 1,
            }
        );
        assert_eq!(outro.base_name, DRIVE_MOTOR_STOP_SOUND);
    }

    #[test]
    fn resolves_empty_drive_motor_samples() {
        let mut manager = SoundFileManager::new(SoundFileManagerOptions::default());
        for name in [
            EMPTY_DRIVE_MOTOR_START_SOUND,
            EMPTY_DRIVE_MOTOR_ON_SOUND,
            EMPTY_DRIVE_MOTOR_OFF_SOUND,
        ] {
            manager.effects.insert(name.to_string(), test_effect(name));
        }

        let Some(PresentableSoundAction::StartLoop { key, intro, looping }) = manager.resolve_presentable_event(
            floppy_event(2, 3, FloppyDriveEvent::MotorStarted { media_present: false }),
        )
        else {
            panic!("Expected an empty-drive motor start loop");
        };
        assert_eq!(
            key,
            PresentableSoundKey::FloppyDriveMotor {
                controller: 2,
                drive: 3,
            }
        );
        assert_eq!(intro.base_name, EMPTY_DRIVE_MOTOR_START_SOUND);
        assert_eq!(looping.base_name, EMPTY_DRIVE_MOTOR_ON_SOUND);

        let Some(PresentableSoundAction::StopLoop { key: stop_key, outro }) = manager.resolve_presentable_event(
            floppy_event(2, 3, FloppyDriveEvent::MotorStopped { media_present: false }),
        )
        else {
            panic!("Expected an empty-drive motor stop action");
        };
        assert_eq!(stop_key, key);
        assert_eq!(outro.base_name, EMPTY_DRIVE_MOTOR_OFF_SOUND);
    }

    #[test]
    fn resolves_media_inserted_to_one_shot() {
        let mut manager = SoundFileManager::new(SoundFileManagerOptions::default());
        manager
            .effects
            .insert(DISK_INSERT_SOUND.to_string(), test_effect(DISK_INSERT_SOUND));

        let Some(PresentableSoundAction::OneShot(effect)) =
            manager.resolve_presentable_event(floppy_event(0, 2, FloppyDriveEvent::MediaInserted))
        else {
            panic!("Expected a media insertion one-shot");
        };
        assert_eq!(effect.base_name, DISK_INSERT_SOUND);
    }

    #[test]
    fn resolves_media_ejected_to_one_shot() {
        let mut manager = SoundFileManager::new(SoundFileManagerOptions::default());
        manager
            .effects
            .insert(DISK_EJECT_SOUND.to_string(), test_effect(DISK_EJECT_SOUND));

        let Some(PresentableSoundAction::OneShot(effect)) =
            manager.resolve_presentable_event(floppy_event(0, 2, FloppyDriveEvent::MediaEjected))
        else {
            panic!("Expected a media ejection one-shot");
        };
        assert_eq!(effect.base_name, DISK_EJECT_SOUND);
    }
}

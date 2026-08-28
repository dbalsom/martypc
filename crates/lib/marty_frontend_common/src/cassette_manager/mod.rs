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
*/

use std::{ffi::OsString, io::Cursor, path::PathBuf};

use marty_common::types::cassette::CassetteTapeType;
use marty_core::devices::cassette_deck::CASSETTE_SAMPLE_RATE;

use crate::resource_manager::{PathTreeNode, ResourceItem, ResourceManager};

use anyhow::{anyhow, Context, Error};
use hound::{SampleFormat, WavReader};

const CASSETTE_RESOURCE: &str = "cassette";

#[derive(Debug)]
pub struct CassetteMedia {
    pub name: OsString,
    pub path: PathBuf,
    pub samples: Vec<f32>,
    pub tape_type: CassetteTapeType,
}

pub struct CassetteManager {
    files: Vec<ResourceItem>,
}

impl CassetteManager {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    pub fn scan_resource(&mut self, rm: &mut ResourceManager) -> Result<bool, Error> {
        self.files = rm.enumerate_items(CASSETTE_RESOURCE, None, true, true, Some(vec![OsString::from("wav")]))?;
        Ok(true)
    }

    pub fn make_tree(&self, rm: &ResourceManager) -> Result<PathTreeNode, Error> {
        rm.items_to_tree(CASSETTE_RESOURCE, &self.files)
    }

    pub fn load_resource(&self, idx: usize, rm: &mut ResourceManager) -> Result<CassetteMedia, Error> {
        let path = self
            .files
            .get(idx)
            .ok_or_else(|| anyhow!("Cassette WAV index {idx} was not found"))?
            .location
            .clone();
        let data = rm.read_resource_from_path_blocking(&path)?;
        self.load_data(path, data)
    }

    pub fn load_data(&self, path: PathBuf, data: Vec<u8>) -> Result<CassetteMedia, Error> {
        let name = path.file_name().unwrap_or(path.as_os_str()).to_os_string();
        let samples = decode_wav(&data)?;
        let tape_type = CassetteTapeType::smallest_for_samples(samples.len(), CASSETTE_SAMPLE_RATE)
            .ok_or_else(|| anyhow!("Cassette WAV is longer than the maximum C60 tape length"))?;
        Ok(CassetteMedia {
            name,
            path,
            samples,
            tape_type,
        })
    }
}

fn decode_wav(data: &[u8]) -> Result<Vec<f32>, Error> {
    let mut reader = WavReader::new(Cursor::new(data)).context("Failed to open cassette WAV data")?;
    let spec = reader.spec();
    if spec.channels == 0 {
        return Err(anyhow!("Cassette WAV has no channels"));
    }

    let interleaved_samples = match spec.sample_format {
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
                .context("Failed to decode floating-point cassette WAV samples")?
        }
        SampleFormat::Int => match spec.bits_per_sample {
            1..=8 => {
                let scale = (1u32 << (spec.bits_per_sample - 1)) as f32;
                reader
                    .samples::<i8>()
                    .map(|sample| sample.map(|sample| sample as f32 / scale))
                    .collect::<Result<Vec<_>, _>>()
                    .context("Failed to decode 8-bit cassette WAV samples")?
            }
            9..=16 => {
                let scale = (1u32 << (spec.bits_per_sample - 1)) as f32;
                reader
                    .samples::<i16>()
                    .map(|sample| sample.map(|sample| sample as f32 / scale))
                    .collect::<Result<Vec<_>, _>>()
                    .context("Failed to decode 16-bit cassette WAV samples")?
            }
            17..=32 => {
                let scale = (1u64 << (spec.bits_per_sample - 1)) as f32;
                reader
                    .samples::<i32>()
                    .map(|sample| sample.map(|sample| sample as f32 / scale))
                    .collect::<Result<Vec<_>, _>>()
                    .context("Failed to decode 32-bit cassette WAV samples")?
            }
            bits => return Err(anyhow!("Unsupported integer WAV sample width: {bits}")),
        },
    };

    let channel_count = spec.channels as usize;
    let mono_samples = if channel_count == 1 {
        interleaved_samples
    }
    else {
        interleaved_samples
            .chunks_exact(channel_count)
            .map(|frame| frame.iter().sum::<f32>() / channel_count as f32)
            .collect()
    };

    resample(&mono_samples, spec.sample_rate, CASSETTE_SAMPLE_RATE)
}

/// perform simple resampling with linear interpolation
/// TODO: Use Rubato?
fn resample(samples: &[f32], source_rate: u32, target_rate: u32) -> Result<Vec<f32>, Error> {
    if source_rate == 0 || target_rate == 0 {
        return Err(anyhow!("Cassette WAV has an invalid sample rate"));
    }
    if samples.is_empty() || source_rate == target_rate {
        return Ok(samples.to_vec());
    }

    let output_len =
        ((samples.len() as u64 * target_rate as u64 + source_rate as u64 / 2) / source_rate as u64) as usize;
    let mut output = Vec::with_capacity(output_len);

    for output_idx in 0..output_len {
        let source_position = output_idx as f64 * source_rate as f64 / target_rate as f64;
        let left_idx = (source_position.floor() as usize).min(samples.len() - 1);
        let right_idx = (left_idx + 1).min(samples.len() - 1);
        let fraction = (source_position - left_idx as f64) as f32;
        output.push(samples[left_idx] + (samples[right_idx] - samples[left_idx]) * fraction);
    }

    Ok(output)
}

impl Default for CassetteManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::{WavSpec, WavWriter};

    #[test]
    fn wav_is_downmixed_and_resampled() {
        let mut wav = Cursor::new(Vec::new());
        {
            let spec = WavSpec {
                channels: 2,
                sample_rate: CASSETTE_SAMPLE_RATE / 2,
                bits_per_sample: 16,
                sample_format: SampleFormat::Int,
            };
            let mut writer = WavWriter::new(&mut wav, spec).unwrap();
            for sample in [i16::MAX, i16::MAX, i16::MIN, i16::MIN] {
                writer.write_sample(sample).unwrap();
            }
            writer.finalize().unwrap();
        }

        let media = CassetteManager::new()
            .load_data(PathBuf::from("test.wav"), wav.into_inner())
            .unwrap();
        assert_eq!(media.tape_type, CassetteTapeType::C5);
        assert_eq!(media.samples.len(), 4);
        assert!(media.samples[0] > 0.99);
        assert!(media.samples[1].abs() < 0.001);
        assert_eq!(media.samples[2], -1.0);
        assert_eq!(media.samples[3], -1.0);
    }

    #[test]
    fn invalid_wav_data_is_rejected() {
        assert!(decode_wav(b"not a wav").is_err());
    }

    #[test]
    fn smallest_tape_is_selected() {
        let c5_samples = CassetteTapeType::C5.capacity_samples(CASSETTE_SAMPLE_RATE);
        let c10_samples = CassetteTapeType::C10.capacity_samples(CASSETTE_SAMPLE_RATE);

        assert_eq!(
            CassetteTapeType::smallest_for_samples(c5_samples, CASSETTE_SAMPLE_RATE),
            Some(CassetteTapeType::C5)
        );
        assert_eq!(
            CassetteTapeType::smallest_for_samples(c5_samples + 1, CASSETTE_SAMPLE_RATE),
            Some(CassetteTapeType::C10)
        );
        assert_eq!(
            CassetteTapeType::smallest_for_samples(c10_samples + 1, CASSETTE_SAMPLE_RATE),
            Some(CassetteTapeType::C15)
        );
    }
}

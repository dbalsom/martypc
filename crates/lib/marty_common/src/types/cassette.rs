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

use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Copy, Clone, Debug, Default, EnumIter, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum CassetteTapeType {
    C5,
    C10,
    C15,
    C30,
    C45,
    #[default]
    C60,
}

impl CassetteTapeType {
    pub const fn duration_minutes(self) -> u32 {
        match self {
            Self::C5 => 5,
            Self::C10 => 10,
            Self::C15 => 15,
            Self::C30 => 30,
            Self::C45 => 45,
            Self::C60 => 60,
        }
    }

    pub const fn duration_seconds(self) -> u32 {
        self.duration_minutes() * 60
    }

    pub const fn length_in_m(self) -> f64 {
        85.7 * self.duration_minutes() as f64 / Self::C60.duration_minutes() as f64
    }

    pub const fn thickness_in_m(self) -> f64 {
        match self {
            Self::C5 | Self::C10 | Self::C15 | Self::C30 | Self::C45 | Self::C60 => 16.7e-6,
        }
    }

    /// Return the capapcity of the [CassetteTapeType] in samples given the rate
    pub fn capacity_samples(self, sample_rate: u32) -> usize {
        (sample_rate as usize).saturating_mul(self.duration_seconds() as usize)
    }

    /// Return the smallest [CassetteTapeType] that first the specified sample count & rate
    pub fn smallest_for_samples(sample_len: usize, sample_rate: u32) -> Option<Self> {
        Self::iter().find(|tape_type| sample_len <= tape_type.capacity_samples(sample_rate))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tape_length_scales_with_playing_time() {
        // Shorter tape types should contain the corresponding fraction of a C60's tape.
        assert_eq!(CassetteTapeType::C60.length_in_m(), 85.7);
        assert!((CassetteTapeType::C5.length_in_m() - 85.7 / 12.0).abs() < f64::EPSILON);
    }

    #[test]
    fn all_tape_types_use_the_same_thickness() {
        // Every tape type currently uses the same physical tape thickness.
        for tape_type in CassetteTapeType::iter() {
            assert_eq!(tape_type.thickness_in_m(), 16.7e-6);
        }
    }

    #[test]
    fn selects_the_smallest_tape_that_fits() {
        let sample_rate = 100u32;

        assert_eq!(
            CassetteTapeType::smallest_for_samples(300 * sample_rate as usize, sample_rate),
            Some(CassetteTapeType::C5)
        );
        assert_eq!(
            CassetteTapeType::smallest_for_samples(300 * sample_rate as usize + 1, sample_rate),
            Some(CassetteTapeType::C10)
        );
        assert_eq!(
            CassetteTapeType::smallest_for_samples(1_800 * sample_rate as usize + 1, sample_rate),
            Some(CassetteTapeType::C45)
        );
        assert_eq!(
            CassetteTapeType::smallest_for_samples(2_700 * sample_rate as usize + 1, sample_rate),
            Some(CassetteTapeType::C60)
        );
        assert_eq!(
            CassetteTapeType::smallest_for_samples(3_601 * sample_rate as usize, sample_rate),
            None
        );
    }
}

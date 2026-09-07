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

//! # Motherboard cassette-input comparator.
//!
//! The IBM 5150 uses a `MC1741` op-amp, the IBM PCjr uses a `LM358` for a similar
//! purpose. These op-amps are wired as *Schmitt triggerred voltage comparators*.

const DEFAULT_LOW_THRESHOLD: f32 = -0.05;
const DEFAULT_HIGH_THRESHOLD: f32 = 0.05;

/// Converts the cassette deck's analog samples into the digital signal read
/// through PPI port C. Accepted transitions are stored as sample-count deltas.
pub struct CassetteComparator {
    low_threshold: f32,
    high_threshold: f32,
    output_level: bool,
    initial_level: bool,
    segment_start: Option<usize>,
    last_sample_position: Option<usize>,
    last_edge_position: Option<usize>,
    edge_deltas: Vec<u32>,
}

impl CassetteComparator {
    pub fn new(low_threshold: f32, high_threshold: f32) -> Self {
        assert!(low_threshold.is_finite());
        assert!(high_threshold.is_finite());
        assert!(low_threshold < high_threshold);

        Self {
            low_threshold,
            high_threshold,
            output_level: false,
            initial_level: false,
            segment_start: None,
            last_sample_position: None,
            last_edge_position: None,
            edge_deltas: Vec::new(),
        }
    }

    /// Process one sample at its logical tape position.
    /// Returns the delta from the previous edge, or from the start of a new continuous segment for
    /// the first edge in that segment.
    pub fn process_sample(&mut self, position: usize, sample: f32) -> Option<u32> {
        let is_contiguous = self
            .last_sample_position
            .and_then(|last_position| last_position.checked_add(1))
            == Some(position);

        if self.last_sample_position.is_none() || !is_contiguous {
            self.start_segment(position, sample);
            return None;
        }

        self.last_sample_position = Some(position);
        if !sample.is_finite() {
            return None;
        }

        let next_level = match self.output_level {
            false if sample >= self.high_threshold => true,
            true if sample <= self.low_threshold => false,
            _ => return None,
        };

        let previous_position = self.last_edge_position.or(self.segment_start)?;
        let delta = u32::try_from(position - previous_position).unwrap_or(u32::MAX);

        self.output_level = next_level;
        self.last_edge_position = Some(position);
        self.edge_deltas.push(delta);

        Some(delta)
    }

    /// start a new segment. A segment represents a continuous segment of tape. A new segment
    /// may be created by seeking, fast-forwarding, rewinding, or inserting a new tape.
    fn start_segment(&mut self, position: usize, sample: f32) {
        self.output_level = sample >= self.high_threshold;
        self.initial_level = self.output_level;
        self.segment_start = Some(position);
        self.last_sample_position = Some(position);
        self.last_edge_position = None;
        self.edge_deltas.clear();
    }

    /// Reset the comparator.
    pub fn reset(&mut self) {
        self.output_level = false;
        self.initial_level = false;
        self.segment_start = None;
        self.last_sample_position = None;
        self.last_edge_position = None;
        self.edge_deltas.clear();
    }

    pub fn output_level(&self) -> bool {
        self.output_level
    }

    pub fn initial_level(&self) -> bool {
        self.initial_level
    }

    pub fn edge_deltas(&self) -> &[u32] {
        &self.edge_deltas
    }

    pub fn take_edge_deltas(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.edge_deltas)
    }
}

impl Default for CassetteComparator {
    fn default() -> Self {
        Self::new(DEFAULT_LOW_THRESHOLD, DEFAULT_HIGH_THRESHOLD)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_time_between_edges() {
        // Each accepted transition should report the sample count since the previous edge.
        let mut comparator = CassetteComparator::default();

        assert_eq!(comparator.process_sample(0, 0.0), None);
        assert_eq!(comparator.process_sample(1, 0.06), Some(1));
        assert!(comparator.output_level());

        assert_eq!(comparator.process_sample(2, 0.01), None);
        assert!(comparator.output_level());

        assert_eq!(comparator.process_sample(3, -0.06), Some(2));
        assert!(!comparator.output_level());
        assert_eq!(comparator.edge_deltas(), &[1, 2]);
    }

    #[test]
    fn a_gap_starts_a_new_segment() {
        // A gap in tape positions should discard timing from the previous continuous segment.
        let mut comparator = CassetteComparator::default();

        comparator.process_sample(10, 0.1);
        assert!(comparator.initial_level());
        assert_eq!(comparator.process_sample(11, -0.1), Some(1));
        assert_eq!(comparator.edge_deltas(), &[1]);

        assert_eq!(comparator.process_sample(20, 0.1), None);
        assert!(comparator.initial_level());
        assert!(comparator.output_level());
        assert!(comparator.edge_deltas().is_empty());
    }

    #[test]
    fn no_edges_inside_hysteresis_band() {
        // Noise between the two thresholds should neither change the output nor create edges.
        let mut comparator = CassetteComparator::default();

        comparator.process_sample(0, 0.0);
        for (position, sample) in [0.01, -0.01, 0.049, -0.049].into_iter().enumerate() {
            assert_eq!(comparator.process_sample(position + 1, sample), None);
        }

        assert!(!comparator.output_level());
        assert!(comparator.edge_deltas().is_empty());
    }

    #[test]
    fn taking_edge_deltas_preserves_timing() {
        // Draining recorded deltas should not reset the position used to time the next edge.
        let mut comparator = CassetteComparator::default();

        comparator.process_sample(0, 0.0);
        comparator.process_sample(1, 0.1);
        assert_eq!(comparator.take_edge_deltas(), vec![1]);
        assert!(comparator.edge_deltas().is_empty());

        assert_eq!(comparator.process_sample(2, -0.1), Some(1));
        assert_eq!(comparator.edge_deltas(), &[1]);
    }
}

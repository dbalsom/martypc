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

//! # Implement the IBM 5150 and PCjr cassette tape interface.
//!
//! This module provides a state machine via [CassetteDeckTransportState].
//!
//! The state of the cassette relay gates motor control in in the
//! [CassetteDeckTransportState::Playing] and [CassetteDeckTransportState::Recording]
//! states.
//!
//! The [CassetteDeckTransportState] can be set directly by the *Cassette Deck* GUI
//! window, as well as the overall tape position.

use crossbeam_channel::Sender;
use marty_common::types::cassette::CassetteTapeType;

use crate::device_traits::sounddevice::AudioSample;

pub const CASSETTE_SAMPLE_RATE: u32 = 44_100;
pub const CASSETTE_MONITOR_SOURCE_NAME: &str = "Cassette Deck Monitor";

const MICROSECONDS_PER_SECOND: f64 = 1_000_000.0;
const SAMPLE_PERIOD_US: f64 = MICROSECONDS_PER_SECOND / CASSETTE_SAMPLE_RATE as f64;
const FAST_WIND_MULTIPLIER: f64 = 10.0;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum CassetteDeckTransportState {
    #[default]
    Stopped,
    Paused,
    Playing,
    FastForwarding,
    Rewinding,
    Recording,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct CassetteDeckState {
    pub transport_state: CassetteDeckTransportState,
    pub relay_state: bool,
    pub tape_inserted: bool,
    pub tape_type: CassetteTapeType,
    pub sample_position: usize,
    pub sample_length: usize,
}

pub struct CassetteDeck {
    image: Option<Vec<f32>>,
    tape_type: CassetteTapeType,
    position: usize,
    state: CassetteDeckTransportState,
    relay_state: bool,
    sample_accum: f64,
    sample_sender: Option<Sender<AudioSample>>,
}

impl CassetteDeck {
    pub fn new(sample_sender: Option<Sender<AudioSample>>) -> Self {
        Self {
            image: None,
            tape_type: CassetteTapeType::C60,
            position: 0,
            state: CassetteDeckTransportState::Stopped,
            relay_state: false,
            sample_accum: 0.0,
            sample_sender,
        }
    }

    /// Insert a cassette image, turning the referenced slice into an owned vector.
    /// Arm playback while the relay is open so the emulated machine can start the tape by closing it.
    pub fn insert_image(&mut self, samples: &[f32], tape_type: CassetteTapeType) {
        self.image = Some(samples.to_vec());
        self.tape_type = tape_type;
        self.position = 0;
        self.state = if self.relay_state {
            CassetteDeckTransportState::Stopped
        }
        else {
            CassetteDeckTransportState::Playing
        };
        self.sample_accum = 0.0;
    }

    /// Eject the cassette image.
    pub fn eject_image(&mut self) {
        self.image = None;
        self.position = 0;
        self.state = CassetteDeckTransportState::Stopped;
        self.sample_accum = 0.0;
    }

    pub fn play(&mut self) {
        if self.has_remaining_tape() {
            self.set_transport_state(CassetteDeckTransportState::Playing);
        }
    }

    pub fn stop(&mut self) {
        self.set_transport_state(CassetteDeckTransportState::Stopped);
    }

    pub fn pause(&mut self) {
        if !matches!(
            self.state,
            CassetteDeckTransportState::Stopped | CassetteDeckTransportState::Paused
        ) {
            self.set_transport_state(CassetteDeckTransportState::Paused);
        }
    }

    pub fn fast_forward(&mut self) {
        if self.has_remaining_tape() {
            self.set_transport_state(CassetteDeckTransportState::FastForwarding);
        }
    }

    pub fn rewind(&mut self) {
        if self.has_image() && self.position > 0 {
            self.set_transport_state(CassetteDeckTransportState::Rewinding);
        }
    }

    pub fn record(&mut self) {
        if self.has_remaining_tape() {
            self.set_transport_state(CassetteDeckTransportState::Recording);
        }
    }

    fn set_transport_state(&mut self, state: CassetteDeckTransportState) {
        self.state = state;
        self.sample_accum = 0.0;
    }

    pub fn run(&mut self, us: f64) {
        self.run_with_sample_output(us, |_, _| {});
    }

    pub fn run_with_sample_output<F>(&mut self, us: f64, mut emit_sample: F)
    where
        F: FnMut(usize, AudioSample),
    {
        if !us.is_finite() || us <= 0.0 || !self.transport_can_advance() {
            return;
        }

        let speed = match self.state {
            CassetteDeckTransportState::FastForwarding | CassetteDeckTransportState::Rewinding => FAST_WIND_MULTIPLIER,
            _ => 1.0,
        };
        self.sample_accum += us * speed;
        while self.sample_accum >= SAMPLE_PERIOD_US {
            self.sample_accum -= SAMPLE_PERIOD_US;

            match self.state {
                CassetteDeckTransportState::Playing => {
                    let Some(sample) = self.current_sample()
                    else {
                        self.stop();
                        break;
                    };

                    emit_sample(self.position, sample);
                    if let Some(sender) = &self.sample_sender {
                        let _ = sender.send(sample);
                    }
                    self.advance();
                }
                CassetteDeckTransportState::Recording => {
                    // Audio capture is not connected yet. Recording still advances
                    // the transport at normal tape speed without altering the image.
                    self.advance();
                }
                CassetteDeckTransportState::FastForwarding => self.advance(),
                CassetteDeckTransportState::Rewinding => {
                    if self.position == 0 {
                        self.stop();
                        break;
                    }
                    self.position -= 1;
                    if self.position == 0 {
                        self.stop();
                        break;
                    }
                }
                CassetteDeckTransportState::Stopped | CassetteDeckTransportState::Paused => break,
            }
        }
    }

    fn transport_can_advance(&self) -> bool {
        match self.state {
            CassetteDeckTransportState::Stopped | CassetteDeckTransportState::Paused => false,
            CassetteDeckTransportState::Playing | CassetteDeckTransportState::Recording => self.relay_state,
            CassetteDeckTransportState::FastForwarding | CassetteDeckTransportState::Rewinding => true,
        }
    }

    fn current_sample(&self) -> Option<f32> {
        if !self.has_remaining_tape() {
            return None;
        }

        Some(self.image.as_ref()?.get(self.position).copied().unwrap_or(0.0))
    }

    fn advance(&mut self) {
        if self.position < self.tape_len() {
            self.position += 1;
        }
        if self.position >= self.tape_len() {
            self.stop();
        }
    }

    fn has_remaining_tape(&self) -> bool {
        self.position < self.tape_len()
    }

    pub fn has_image(&self) -> bool {
        self.image.is_some()
    }

    pub fn is_playing(&self) -> bool {
        self.state == CassetteDeckTransportState::Playing
    }

    pub fn state(&self) -> CassetteDeckState {
        CassetteDeckState {
            transport_state: self.state,
            relay_state: self.relay_state,
            tape_inserted: self.has_image(),
            tape_type: self.tape_type,
            sample_position: self.position,
            sample_length: self.tape_len(),
        }
    }

    pub fn transport_state(&self) -> CassetteDeckTransportState {
        self.state
    }

    pub fn relay_state(&self) -> bool {
        self.relay_state
    }

    pub fn set_relay_state(&mut self, state: bool) {
        self.relay_state = state;
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn seek(&mut self, sample_position: usize) {
        self.position = sample_position.min(self.tape_len());
        self.sample_accum = 0.0;
    }

    pub fn image_len(&self) -> usize {
        self.image.as_ref().map_or(0, Vec::len)
    }

    pub fn tape_type(&self) -> CassetteTapeType {
        self.tape_type
    }

    pub fn tape_len(&self) -> usize {
        if self.has_image() {
            self.tape_type.capacity_samples(CASSETTE_SAMPLE_RATE)
        }
        else {
            0
        }
    }
}

impl Default for CassetteDeck {
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;

    #[test]
    fn a_new_tape_is_ready_to_play() {
        // Inserting with the relay open should select Play without moving the tape yet.
        let mut deck = CassetteDeck::default();

        deck.insert_image(&[0.25, -0.5], CassetteTapeType::C5);
        assert!(deck.has_image());
        assert_eq!(deck.image_len(), 2);
        assert_eq!(deck.tape_type(), CassetteTapeType::C5);
        assert_eq!(
            deck.tape_len(),
            CassetteTapeType::C5.capacity_samples(CASSETTE_SAMPLE_RATE)
        );
        assert_eq!(deck.position(), 0);
        assert_eq!(deck.transport_state(), CassetteDeckTransportState::Playing);

        deck.run(SAMPLE_PERIOD_US);
        assert_eq!(deck.position(), 0);

        deck.set_relay_state(true);
        deck.run(SAMPLE_PERIOD_US);
        assert_eq!(deck.position(), 1);
    }

    #[test]
    fn ejecting_a_tape_resets_the_deck() {
        // Ejecting should remove the image, rewind the position, and stop the transport.
        let mut deck = CassetteDeck::default();
        deck.insert_image(&[0.25, -0.5], CassetteTapeType::C5);
        deck.set_relay_state(true);
        deck.run(SAMPLE_PERIOD_US);

        deck.eject_image();

        assert!(!deck.has_image());
        assert_eq!(deck.image_len(), 0);
        assert_eq!(deck.tape_len(), 0);
        assert_eq!(deck.position(), 0);
        assert_eq!(deck.transport_state(), CassetteDeckTransportState::Stopped);
    }

    #[test]
    fn a_tape_inserted_with_the_relay_closed_stays_stopped() {
        // A closed relay should prevent insertion from automatically selecting Play.
        let mut deck = CassetteDeck::default();
        deck.set_relay_state(true);

        deck.insert_image(&[0.25, -0.5], CassetteTapeType::C5);

        assert_eq!(deck.transport_state(), CassetteDeckTransportState::Stopped);
        deck.run(SAMPLE_PERIOD_US);
        assert_eq!(deck.position(), 0);
    }

    #[test]
    fn playback_sends_one_sample_each_period() {
        // Two elapsed sample periods should send the first two samples to the monitor.
        let (sender, receiver) = unbounded();
        let mut deck = CassetteDeck::new(Some(sender));
        deck.insert_image(&[0.25, -0.5], CassetteTapeType::C5);
        deck.set_relay_state(true);
        deck.play();

        deck.run(SAMPLE_PERIOD_US * 2.0);

        assert_eq!(receiver.try_iter().collect::<Vec<_>>(), vec![0.25, -0.5]);
        assert_eq!(deck.position(), 2);
        assert_eq!(deck.transport_state(), CassetteDeckTransportState::Playing);
    }

    #[test]
    fn the_sample_callback_includes_the_tape_position() {
        // The callback should pair each sample with the position it came from.
        let mut deck = CassetteDeck::default();
        let mut emitted_samples = Vec::new();
        deck.insert_image(&[0.25, -0.5], CassetteTapeType::C5);
        deck.set_relay_state(true);
        deck.play();

        for _ in 0..3 {
            deck.run_with_sample_output(SAMPLE_PERIOD_US, |position, sample| {
                emitted_samples.push((position, sample));
            });
        }

        assert_eq!(emitted_samples, vec![(0, 0.25), (1, -0.5), (2, 0.0)]);
    }

    #[test]
    fn a_short_image_is_padded_with_silence() {
        // Playback should output silence after the image ends but before the tape ends.
        let (sender, receiver) = unbounded();
        let mut deck = CassetteDeck::new(Some(sender));
        deck.insert_image(&[0.25, -0.5], CassetteTapeType::C5);
        deck.set_relay_state(true);
        deck.play();

        for _ in 0..4 {
            deck.run(SAMPLE_PERIOD_US);
        }

        assert_eq!(receiver.try_iter().collect::<Vec<_>>(), vec![0.25, -0.5, 0.0, 0.0]);
        assert_eq!(deck.position(), 4);
        assert_eq!(deck.transport_state(), CassetteDeckTransportState::Playing);
    }

    #[test]
    fn playback_stops_at_the_end_of_the_tape() {
        // Consuming the final position should stop the transport at the tape boundary.
        let mut deck = CassetteDeck::default();
        deck.insert_image(&[0.25], CassetteTapeType::C5);
        deck.position = deck.tape_len() - 1;
        deck.set_relay_state(true);
        deck.play();

        deck.run(SAMPLE_PERIOD_US);

        assert_eq!(
            deck.position(),
            CassetteTapeType::C5.capacity_samples(CASSETTE_SAMPLE_RATE)
        );
        assert_eq!(deck.transport_state(), CassetteDeckTransportState::Stopped);
    }

    #[test]
    fn the_transport_controls_select_the_requested_mode() {
        // Each valid transport command should leave the deck in its requested mode.
        let mut deck = CassetteDeck::default();
        deck.insert_image(&[0.0; 32], CassetteTapeType::C5);

        deck.play();
        assert_eq!(deck.transport_state(), CassetteDeckTransportState::Playing);
        deck.pause();
        assert_eq!(deck.transport_state(), CassetteDeckTransportState::Paused);
        deck.fast_forward();
        assert_eq!(deck.transport_state(), CassetteDeckTransportState::FastForwarding);
        deck.record();
        assert_eq!(deck.transport_state(), CassetteDeckTransportState::Recording);
        deck.stop();
        assert_eq!(deck.transport_state(), CassetteDeckTransportState::Stopped);
    }

    #[test]
    fn fast_forward_and_rewind_run_at_winding_speed() {
        // Winding should skip samples quickly and stop when it reaches the start.
        let mut deck = CassetteDeck::default();
        deck.insert_image(&[0.0; 20], CassetteTapeType::C5);
        deck.set_relay_state(true);

        deck.fast_forward();
        deck.run(SAMPLE_PERIOD_US);
        assert_eq!(deck.position(), FAST_WIND_MULTIPLIER as usize);

        deck.rewind();
        deck.run(SAMPLE_PERIOD_US);
        assert_eq!(deck.position(), 0);
        assert_eq!(deck.transport_state(), CassetteDeckTransportState::Stopped);
    }

    #[test]
    fn seeking_resets_timing_and_stays_within_the_tape() {
        // Seeking should discard partial timing and clamp positions beyond the tape.
        let mut deck = CassetteDeck::default();
        deck.insert_image(&[0.0; 32], CassetteTapeType::C5);
        deck.sample_accum = SAMPLE_PERIOD_US / 2.0;

        deck.seek(123_456);
        assert_eq!(deck.position(), 123_456);
        assert_eq!(deck.sample_accum, 0.0);

        deck.seek(usize::MAX);
        assert_eq!(deck.position(), deck.tape_len());
    }

    #[test]
    fn the_motor_relay_only_controls_play_and_record() {
        // With the relay open, play and record should wait while winding still works.
        let (sender, receiver) = unbounded();
        let mut deck = CassetteDeck::new(Some(sender));
        deck.insert_image(&[0.25, -0.5], CassetteTapeType::C5);

        deck.play();
        deck.run(SAMPLE_PERIOD_US);
        assert_eq!(deck.transport_state(), CassetteDeckTransportState::Playing);
        assert_eq!(deck.position(), 0);
        assert!(receiver.is_empty());

        deck.record();
        deck.run(SAMPLE_PERIOD_US);
        assert_eq!(deck.transport_state(), CassetteDeckTransportState::Recording);
        assert_eq!(deck.position(), 0);

        deck.fast_forward();
        deck.run(SAMPLE_PERIOD_US);
        assert_eq!(deck.transport_state(), CassetteDeckTransportState::FastForwarding);
        assert_eq!(deck.position(), FAST_WIND_MULTIPLIER as usize);

        deck.rewind();
        deck.run(SAMPLE_PERIOD_US);
        assert_eq!(deck.position(), 0);
        assert_eq!(deck.transport_state(), CassetteDeckTransportState::Stopped);

        deck.set_relay_state(true);
        deck.play();
        deck.run(SAMPLE_PERIOD_US);
        assert_eq!(deck.position(), 1);
        assert_eq!(receiver.try_recv(), Ok(0.25));
    }
}

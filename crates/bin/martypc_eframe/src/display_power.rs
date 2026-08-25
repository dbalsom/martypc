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
*/

//! Create a power-off animation for the display with the Machine is powered off via the Machine
//! menu. It's not quite 100% realistic to how a 5153 specifically powers off, but it's recognizable
//! and it looks kinda cool.

use web_time::{Duration, Instant};

const POWER_OFF_DURATION: Duration = Duration::from_millis(700);

#[derive(Clone, Debug)]
enum DisplayPowerState {
    On,
    PoweringOff { started: Instant },
    Off,
}

/// Tracks display presentation separately from the machine's electrical state.
#[derive(Clone, Debug)]
pub struct DisplayPowerEffect {
    state: DisplayPowerState,
}

impl Default for DisplayPowerEffect {
    fn default() -> Self {
        Self {
            state: DisplayPowerState::On,
        }
    }
}

impl DisplayPowerEffect {
    pub fn begin_power_off(&mut self) {
        if matches!(self.state, DisplayPowerState::On) {
            self.state = DisplayPowerState::PoweringOff {
                started: Instant::now(),
            };
        }
    }

    pub fn power_on(&mut self) {
        self.state = DisplayPowerState::On;
    }

    pub fn power_off_immediately(&mut self) {
        self.state = DisplayPowerState::Off;
    }

    /// The backing frame must remain untouched throughout the animation and while powered off.
    pub fn frame_frozen(&self) -> bool {
        !matches!(self.state, DisplayPowerState::On)
    }

    /// Return normalized shader progress and complete the state transition when its timer expires.
    pub fn progress(&mut self) -> f32 {
        match self.state {
            DisplayPowerState::On => 0.0,
            DisplayPowerState::Off => 1.0,
            DisplayPowerState::PoweringOff { started } => {
                let progress = normalized_progress(started.elapsed());
                if progress >= 1.0 {
                    self.state = DisplayPowerState::Off;
                }
                progress
            }
        }
    }
}

fn normalized_progress(elapsed: Duration) -> f32 {
    (elapsed.as_secs_f32() / POWER_OFF_DURATION.as_secs_f32()).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_off_progress_is_normalized_and_clamped() {
        assert_eq!(normalized_progress(Duration::ZERO), 0.0);
        assert!((normalized_progress(POWER_OFF_DURATION / 2) - 0.5).abs() < f32::EPSILON);
        assert_eq!(normalized_progress(POWER_OFF_DURATION), 1.0);
        assert_eq!(normalized_progress(POWER_OFF_DURATION * 2), 1.0);
    }

    #[test]
    fn powered_off_displays_remain_frozen_until_power_on() {
        let mut effect = DisplayPowerEffect::default();
        assert!(!effect.frame_frozen());

        effect.begin_power_off();
        assert!(effect.frame_frozen());

        effect.power_on();
        assert!(!effect.frame_frozen());
        assert_eq!(effect.progress(), 0.0);
    }
}

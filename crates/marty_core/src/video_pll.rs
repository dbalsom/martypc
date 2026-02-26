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

//! A simple Phase-Locked Loop (PLL) implementation for simulating monitor sync behavior.
//! This can be used for either the vertical or horizontal sync signals.

/// Sync pulse polarity initialization parameter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SyncPolarity {
    #[default]
    Positive, // Idle Low, Pulse High
    Negative, // Idle High, Pulse Low
    Auto,
}

/// Active pulse polarity when 'Auto' was selected for SyncPolarity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActiveSyncPolarity {
    #[default]
    Positive, // Idle Low, Pulse High
    Negative, // Idle High, Pulse Low
}

/// Parameters for configuring the VideoHoldPll.
pub struct VideoPllParams {
    /// Maximum frequency range adjustment
    pub range: f64,
    /// Proportional gain
    pub kp: f64,
    /// Integral gain
    pub ki: f64,
    pub max_error: f64,
    pub free_drift_term: f64,
    /// Size of the sync window (in phase units)
    pub window_size: f64,
    /// Sync pulse polarity
    pub polarity: SyncPolarity,
}

impl Default for VideoPllParams {
    fn default() -> Self {
        // The default values are calibrated for a 14MHz clock.
        Self {
            range: 0.1,
            kp: 0.5,
            ki: 0.0000005,
            max_error: 0.05,
            free_drift_term: 0.15,
            window_size: 0.2, // ~16 scanlines
            polarity: SyncPolarity::Negative,
        }
    }
}

pub struct VideoHoldPllDebug {
    pub vco_phase: f64,
    pub is_locked: bool,
}

pub struct VideoHoldPll {
    enabled: bool,
    /// Master pixel clock frequency.
    /// - 16.25 Mhz for MDA
    /// - 14.31818 Mhz for CGA and 320 column EGA modes
    ticks_per_second: f64,
    target_period_ticks: f64,
    last_period_ticks: f64,
    base_phase_step: f64, // Phase increment per clock tick
    max_drift: f64,       // Upper bound of the integrator
    min_drift: f64,       // Lower bound of the integrator
    max_error: f64,

    // Loop Filter gain values. There is no rigorous derivation here, just trial and error.
    kp: f64, // Proportional term: Directly corrects phase
    ki: f64, // Integral term: Corrects frequency drift (dampening factor)

    free_drift_term: f64,
    /// Size of the sync window (in phase units)
    window_size: f64,
    /// Progress through current frame [0.0, 1.0]
    vco_phase: f64,
    debug_phase: f64,
    last_error: f64,
    /// The "V-Hold" adjustment to the base frequency
    drift_offset: f64,
    last_sync_active: bool,

    // Polarity Detection
    high_count: u64,
    low_count: u64,
    polarity: SyncPolarity,
    active_polarity: ActiveSyncPolarity,

    /// Whether the PLL is currently locked to the input signal.
    is_locked: bool,
    ticks_since_last_sync: u64,
}

impl VideoHoldPll {
    pub fn new(clock_base: f64, ref_clock: f64, terms: VideoPllParams) -> Self {
        Self {
            enabled: true,
            // The clock base is typically the pixel clock of the card.
            // This can change depending on the video mode, so cards will need
            // to reconfigure the PLL when switching modes, or use different PLLs per mode.
            ticks_per_second: clock_base,
            // Compute the master clock period as a tick count of the base block.
            target_period_ticks: clock_base / ref_clock,
            last_period_ticks: 0.0,
            base_phase_step: ref_clock / clock_base,
            max_drift: (ref_clock / clock_base) * terms.range,
            min_drift: -((ref_clock / clock_base) * terms.range),
            max_error: terms.max_error,
            kp: terms.kp,
            ki: terms.ki,
            free_drift_term: terms.free_drift_term,
            window_size: terms.window_size,
            vco_phase: 0.0,
            debug_phase: 0.0,
            last_error: 0.0,
            drift_offset: 0.0,
            last_sync_active: false,
            is_locked: false,
            high_count: 0,
            low_count: 0,
            polarity: terms.polarity,
            active_polarity: if matches!(terms.polarity, SyncPolarity::Auto) {
                ActiveSyncPolarity::Positive
            }
            else {
                match terms.polarity {
                    SyncPolarity::Positive => ActiveSyncPolarity::Positive,
                    SyncPolarity::Negative => ActiveSyncPolarity::Negative,
                    SyncPolarity::Auto => unreachable!(),
                }
            },
            ticks_since_last_sync: u64::MAX,
        }
    }

    pub fn enable(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Run the PLL.
    /// `ticks_elapsed`: Number of `clock_base` cycles since last update.
    /// `raw_sync`: Current state of the sync pulse.
    /// Returns `true` if the sync pulse was detected within the PLL sync window.
    pub fn run(&mut self, ticks_elapsed: u32, raw_sync: bool) -> bool {
        // Flyback Trigger
        let mut triggered = false;
        self.ticks_since_last_sync = self.ticks_since_last_sync.saturating_add(ticks_elapsed as u64);
        let dt = ticks_elapsed as f64;

        let sync_active = match self.active_polarity {
            ActiveSyncPolarity::Positive => raw_sync,
            ActiveSyncPolarity::Negative => !raw_sync,
        };

        // Early 80's PLL implementations often consisted of a phase comparator, loop filter, and
        // voltage controlled oscillator (VC0). We attempt a simple model of what is a complex
        // analog feedback system.

        // Phase Comparator - the phase comparator measures the difference in phase between a
        // reference clock (the master_clock defined in the PLL) and an observed clock

        // Trigger on edge of sync pulse
        if sync_active {
            if !self.enabled {
                // PLL is disabled, just honor the input sync pulse.
                return true;
            }
            else if !self.last_sync_active && self.ticks_since_last_sync > (self.target_period_ticks * 0.5) as u64 {
                self.ticks_since_last_sync = 0;
                self.debug_phase = self.vco_phase;

                // We expect the pulse at phase 0.0 (the start of a new frame).
                // Calculate how far off we are (Phase Error).

                //let raw_error = self.calculate_phase_error();
                //let error = raw_error.clamp(-self.max_error, self.max_error);

                let error = self.calculate_phase_error();
                self.last_error = error;

                // Apply Integral (Drift/Hold)
                self.drift_offset = (self.drift_offset + error * self.ki).clamp(self.min_drift, self.max_drift);
                // Apply Proportional (Phase Snap)
                self.vco_phase += error * self.kp;

                // Only adjust if the pulse is within the "Sync Window"
                if error.abs() < (self.window_size / 2.0) {
                    triggered = true;
                    self.is_locked = true;
                }
                else {
                    // Pulse fell outside the window. This represents loss sync and the picture
                    // will begin to roll as the monitor will perform flyback out of phase.

                    // We don't apply full correction, but we will slowly drift toward the sync.
                    //self.drift_offset += error * (self.ki * self.free_drift_term);
                    self.is_locked = false;
                }
            }
        }

        self.last_sync_active = sync_active;

        // Voltage Controlled Oscillator (VCO)
        // The VCO speeds up or slows down based on the provided voltage from the phase comparator + loop filter.
        // A negative voltage makes the VCO run slower, a positive voltage makes it run faster.

        self.vco_phase += (self.base_phase_step + self.drift_offset) * dt;

        if self.vco_phase >= 1.0 {
            self.vco_phase -= 1.0;
            //triggered |= true;
        }

        triggered
    }

    /// Returns the error between current phase and the ideal sync point (0.0/1.0).
    /// Positive error means the pulse arrived "early" (we need to speed up).
    /// Negative error means the pulse arrived "late" (we need to slow down).
    fn calculate_phase_error(&self) -> f64 {
        if self.vco_phase > 0.5 {
            1.0 - self.vco_phase // Example: phase 0.98 -> error +0.02
        }
        else {
            0.0 - self.vco_phase // Example: phase 0.02 -> error -0.02
        }
    }

    /// Simulates the user turning the "hold" knob on the back of the monitor.
    /// A positive value makes the monitor's internal oscillator run faster.
    pub fn adjust_hold(&mut self, adjustment: f64) {
        self.drift_offset += adjustment;
    }

    pub fn current_freq(&self) -> f64 {
        let step_per_tick = (1.0 / self.target_period_ticks) + self.drift_offset;
        step_per_tick * self.ticks_per_second
    }

    pub fn is_locked(&self) -> bool {
        self.is_locked
    }

    pub fn is_in_window(&self) -> bool {
        let error = self.calculate_phase_error();
        error.abs() < (self.window_size / 2.0)
    }

    pub fn phase(&self) -> f64 {
        self.vco_phase
    }
    pub fn sync_phase(&self) -> f64 {
        self.debug_phase
    }
    pub fn error(&self) -> f64 {
        self.last_error
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_pll() -> VideoHoldPll {
        let params = VideoPllParams {
            kp: 0.5,
            ki: 0.1,
            free_drift_term: 0.0,
            window_size: 0.2,
            ..Default::default()
        };
        // Base clock 1000 Hz, Target 10 Hz -> 100 ticks per period
        VideoHoldPll::new(1000.0, 10.0, params)
    }

    #[test]
    fn initialization_state() {
        let pll = create_test_pll();
        assert_eq!(pll.vco_phase, 0.0);
        assert_eq!(pll.drift_offset, 0.0);
        assert!(!pll.is_locked);
        assert!(!pll.last_sync_active);
    }

    #[test]
    fn free_running_oscillator_triggers() {
        let mut pll = create_test_pll();

        // Run for 99 ticks (target is 100)
        let triggered = pll.run(99, false);
        assert!(!triggered, "Should not trigger before period is complete");

        // Run for 1 more tick
        let triggered = pll.run(1, false);
        assert!(triggered, "Should trigger when period is complete");
    }

    #[test]
    fn sync_pulse_locks_pll() {
        let mut pll = create_test_pll();

        // Simulate a sync pulse arriving exactly at the start (phase 0.0)
        // We need to transition sync_active from false to true
        pll.run(0, false);
        pll.run(0, true);

        assert!(pll.is_locked);
    }

    #[test]
    fn sync_pulse_outside_window_unlocks() {
        let mut pll = create_test_pll();

        // Advance phase to 0.5 (middle of cycle)
        // Target is 100 ticks, so run 50 ticks
        pll.run(50, false);

        // Send sync pulse (rising edge)
        pll.run(0, true);

        // Error is 0.5, window is 0.2. Should fail to lock.
        assert!(!pll.is_locked);
    }

    #[test]
    fn phase_error_calculation_early() {
        let mut pll = create_test_pll();
        // Set phase to 0.9 (pulse arrived "early" relative to end of cycle)
        pll.vco_phase = 0.9;

        let error = pll.calculate_phase_error();
        // Expected: 1.0 - 0.9 = 0.1 (positive error)
        assert!((error - 0.1).abs() < 1e-9);
    }

    #[test]
    fn phase_error_calculation_late() {
        let mut pll = create_test_pll();
        // Set phase to 0.1 (pulse arrived "late" relative to start of cycle)
        pll.vco_phase = 0.1;

        let error = pll.calculate_phase_error();
        // Expected: 0.0 - 0.1 = -0.1 (negative error)
        assert!((error - -0.1).abs() < 1e-9);
    }

    #[test]
    fn adjust_hold_changes_fps() {
        let mut pll = create_test_pll();
        let initial_freq = pll.current_freq();

        // Adjust knob to speed up (positive)
        pll.adjust_hold(0.01);

        let new_freq = pll.current_freq();
        assert!(new_freq > initial_freq);
    }

    #[test]
    fn proportional_correction_applied() {
        let mut pll = create_test_pll();

        // Set phase slightly off (0.05), but within window (0.2)
        pll.vco_phase = 0.05;

        // Trigger sync
        pll.run(0, true);

        // Error was -0.05. kp is 0.5.
        // Correction = -0.05 * 0.5 = -0.025.
        // New phase should be approx 0.05 - 0.025 = 0.025 (plus integral term)
        // We just check that phase moved towards 0
        assert!(pll.vco_phase < 0.05);
    }
}

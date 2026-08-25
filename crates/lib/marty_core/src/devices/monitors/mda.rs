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

//! Monitor module for Monochrome displays (IBM 5151, etc).

use crate::{
    device_traits::{monitor::Monitor, videocard::VideoCardStateEntry},
    devices::mda::{MDA_CLOCK, MDA_HORIZ_REFRESH, MDA_VERT_REFRESH},
    video_pll::{SyncPolarity, VideoHoldPll, VideoPllParams},
};

pub struct MdaMonitor {
    emulate_vsync:  bool,
    emulate_hsync:  bool,
    horizontal_pll: VideoHoldPll,
    vertical_pll:   VideoHoldPll,
    out_of_sync:    bool,
}

impl Default for MdaMonitor {
    fn default() -> Self {
        Self {
            emulate_vsync:  true,
            emulate_hsync:  true,
            horizontal_pll: VideoHoldPll::new(
                MDA_CLOCK * 1_000_000.0,
                MDA_HORIZ_REFRESH, // ~15.699Khz
                VideoPllParams {
                    range: 0.10,
                    kp: 0.5,
                    ki: 1.0e-6,
                    max_error: 0.05,
                    free_drift_term: 0.15,
                    window_size: 0.20,
                    polarity: SyncPolarity::Positive,
                },
            ),
            vertical_pll:   VideoHoldPll::new(
                MDA_CLOCK * 1_000_000.0,
                MDA_VERT_REFRESH,
                VideoPllParams {
                    polarity: SyncPolarity::Negative,
                    ..Default::default()
                },
            ),
            out_of_sync:    false,
        }
    }
}

impl Monitor for MdaMonitor {
    fn run(
        &mut self,
        ticks_elapsed: u32,
        hsync: bool,
        vsync: bool,
        h_callback: &mut dyn FnMut(),
        v_callback: &mut dyn FnMut(),
    ) {
        if self.horizontal_pll.run(ticks_elapsed, hsync) {
            h_callback();
        }
        if self.vertical_pll.run(ticks_elapsed, vsync) {
            v_callback();
        }
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.horizontal_pll.enable(enabled);
        self.vertical_pll.enable(enabled);
    }

    fn set_sync_polarities(&mut self, hsync: SyncPolarity, vsync: SyncPolarity) {
        self.horizontal_pll.set_polarity(hsync);
        self.vertical_pll.set_polarity(vsync);
    }

    fn hsync_frequency(&self) -> Option<f64> {
        self.horizontal_pll
            .is_locked()
            .then(|| self.horizontal_pll.observed_freq())
            .flatten()
    }

    fn debug_state(&self) -> Vec<(String, VideoCardStateEntry)> {
        vec![
            (
                String::from("hhold:"),
                VideoCardStateEntry::String(format!("{}", self.horizontal_pll.is_locked())),
            ),
            (
                String::from("hsync polarity:"),
                VideoCardStateEntry::String(format!("{:?}", self.horizontal_pll.polarity())),
            ),
            (
                String::from("h_sync_freq:"),
                VideoCardStateEntry::String(
                    self.horizontal_pll
                        .observed_freq()
                        .map(|freq| format!("{:.2}Hz", freq))
                        .unwrap_or_else(|| String::from("None")),
                ),
            ),
            (
                String::from("h_pll_freq:"),
                VideoCardStateEntry::String(format!("{:.2}Hz", self.horizontal_pll.current_freq())),
            ),
            (
                String::from("h_pll_phase:"),
                VideoCardStateEntry::String(format!("{:.3}", self.horizontal_pll.sync_phase())),
            ),
            (
                String::from("h_pll_error:"),
                VideoCardStateEntry::String(format!("{:.3}", self.horizontal_pll.error())),
            ),
            (
                String::from("vhold:"),
                VideoCardStateEntry::String(format!("{}", self.vertical_pll.is_locked())),
            ),
            (
                String::from("vsync polarity:"),
                VideoCardStateEntry::String(format!("{:?}", self.vertical_pll.polarity())),
            ),
            (
                String::from("v_pll_freq:"),
                VideoCardStateEntry::String(format!("{:.2}Hz", self.vertical_pll.current_freq())),
            ),
            (
                String::from("v_pll_phase:"),
                VideoCardStateEntry::String(format!("{:.3}", self.vertical_pll.sync_phase())),
            ),
        ]
    }
}

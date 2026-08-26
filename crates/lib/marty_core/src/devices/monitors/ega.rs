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

//! Monitor module for IBM EGA dual-sync displays.

use crate::{
    device_traits::{monitor::Monitor, videocard::VideoCardStateEntry},
    device_types::video::{NTSC_CLOCK, NTSC_HORIZ_REFRESH, NTSC_VERT_REFRESH},
    devices::ega::EGA_CLOCK1,
    video_pll::{SyncPolarity, VideoHoldPll, VideoPllParams},
};

const EGA_MODE2_SCANLINE_DOTS: f64 = 744.0;
const EGA_MODE2_SCANLINES: f64 = 364.0;
const EGA_MODE2_HORIZ_REFRESH: f64 = EGA_CLOCK1 / EGA_MODE2_SCANLINE_DOTS * 1_000_000.0;
const EGA_MODE2_VERT_REFRESH: f64 = EGA_CLOCK1 / (EGA_MODE2_SCANLINE_DOTS * EGA_MODE2_SCANLINES) * 1_000_000.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EgaMonitorMode {
    #[default]
    /// EGA monitor mode 1: 15.7 kHz compatible scan rate.
    Mode1,
    /// EGA monitor mode 2: 21.8 kHz enhanced scan rate.
    Mode2,
}

impl From<(SyncPolarity, SyncPolarity)> for EgaMonitorMode {
    fn from((hsync, vsync): (SyncPolarity, SyncPolarity)) -> Self {
        match (hsync, vsync) {
            (SyncPolarity::Positive, SyncPolarity::Positive) => Self::Mode1,
            (SyncPolarity::Positive, SyncPolarity::Negative) => Self::Mode2,
            (SyncPolarity::Negative, SyncPolarity::Positive) => Self::Mode1,
            // Negative/Negative is not a valid EGA monitor mode; keep it in the enhanced scan-rate bucket.
            (SyncPolarity::Negative, SyncPolarity::Negative) => Self::Mode2,
        }
    }
}

impl EgaMonitorMode {
    fn clock_base(self) -> f64 {
        match self {
            Self::Mode1 => NTSC_CLOCK * 1_000_000.0,
            Self::Mode2 => EGA_CLOCK1 * 1_000_000.0,
        }
    }

    fn horizontal_refresh(self) -> f64 {
        match self {
            Self::Mode1 => NTSC_HORIZ_REFRESH,
            Self::Mode2 => EGA_MODE2_HORIZ_REFRESH,
        }
    }

    fn vertical_refresh(self) -> f64 {
        match self {
            Self::Mode1 => NTSC_VERT_REFRESH,
            Self::Mode2 => EGA_MODE2_VERT_REFRESH,
        }
    }

    fn new_horizontal_pll(self, polarity: SyncPolarity, input_clock_base: f64) -> VideoHoldPll {
        let range = match self {
            Self::Mode1 => 0.10,
            Self::Mode2 => 0.21, // Approximate a multisync monitor for Rambo
        };

        VideoHoldPll::new(
            input_clock_base,
            self.horizontal_refresh(),
            VideoPllParams {
                range,
                kp: 0.5,
                ki: 1.0e-6,
                max_error: 0.05,
                free_drift_term: 0.15,
                window_size: 0.20,
                polarity,
            },
        )
    }

    fn new_vertical_pll(self, polarity: SyncPolarity, input_clock_base: f64) -> VideoHoldPll {
        VideoHoldPll::new(
            input_clock_base,
            self.vertical_refresh(),
            VideoPllParams {
                polarity,
                ..Default::default()
            },
        )
    }
}

pub struct EgaMonitor {
    enabled: bool,
    mode: EgaMonitorMode,
    input_clock_base: f64,
    hsync_polarity: SyncPolarity,
    vsync_polarity: SyncPolarity,
    horizontal_pll: VideoHoldPll,
    vertical_pll: VideoHoldPll,
}

impl EgaMonitor {
    pub fn new(mode: EgaMonitorMode) -> Self {
        Self::new_with_clock(mode, mode.clock_base())
    }

    pub fn new_with_clock(mode: EgaMonitorMode, input_clock_base: f64) -> Self {
        Self {
            enabled: true,
            mode,
            input_clock_base,
            hsync_polarity: SyncPolarity::Positive,
            vsync_polarity: SyncPolarity::Positive,
            horizontal_pll: mode.new_horizontal_pll(SyncPolarity::Positive, input_clock_base),
            vertical_pll: mode.new_vertical_pll(SyncPolarity::Positive, input_clock_base),
        }
    }

    pub fn mode(&self) -> EgaMonitorMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: EgaMonitorMode) {
        if self.mode == mode {
            return;
        }

        self.mode = mode;
        self.rebuild_plls();
    }

    pub fn set_input_clock_base(&mut self, input_clock_base: f64) {
        if (self.input_clock_base - input_clock_base).abs() < f64::EPSILON {
            return;
        }

        self.input_clock_base = input_clock_base;
        self.rebuild_plls();
    }

    fn rebuild_plls(&mut self) {
        self.horizontal_pll = self.mode.new_horizontal_pll(self.hsync_polarity, self.input_clock_base);
        self.vertical_pll = self.mode.new_vertical_pll(self.vsync_polarity, self.input_clock_base);
        self.horizontal_pll.enable(self.enabled);
        self.vertical_pll.enable(self.enabled);
    }
}

impl Default for EgaMonitor {
    fn default() -> Self {
        Self::new(EgaMonitorMode::default())
    }
}

impl Monitor for EgaMonitor {
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
        self.enabled = enabled;
        self.horizontal_pll.enable(enabled);
        self.vertical_pll.enable(enabled);
    }

    fn set_sync_polarities(&mut self, hsync: SyncPolarity, vsync: SyncPolarity) {
        self.hsync_polarity = hsync;
        self.vsync_polarity = vsync;

        let mode = EgaMonitorMode::from((hsync, vsync));
        if self.mode != mode {
            self.set_mode(mode);
        }
        else {
            self.horizontal_pll.set_polarity(hsync);
            self.vertical_pll.set_polarity(vsync);
        }
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
                String::from("mode:"),
                VideoCardStateEntry::String(format!("{:?}", self.mode)),
            ),
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

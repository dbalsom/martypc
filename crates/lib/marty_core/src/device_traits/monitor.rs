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
use enum_dispatch::enum_dispatch;

use crate::{
    device_traits::videocard::VideoCardStateEntry,
    devices::monitors::{fifteen_hertz::FifteenHertzMonitor, mda::MdaMonitor},
    video_pll::SyncPolarity,
};

#[cfg(feature = "ega")]
use crate::devices::monitors::ega::EgaMonitor;

#[enum_dispatch]
pub enum MonitorDispatch {
    MdaMonitor,
    FifteenHertzMonitor,
    #[cfg(feature = "ega")]
    EgaMonitor,
}

pub type MonitorSyncCallback<'a> = &'a mut dyn FnMut();

/// Trait for monitor / video PLL emulation.
#[enum_dispatch(MonitorDispatch)]
pub trait Monitor {
    fn run(
        &mut self,
        ticks_elapsed: u32,
        hsync: bool,
        vsync: bool,
        h_callback: MonitorSyncCallback<'_>,
        v_callback: MonitorSyncCallback<'_>,
    );

    fn set_enabled(&mut self, enabled: bool);

    fn set_sync_polarities(&mut self, hsync: SyncPolarity, vsync: SyncPolarity);

    fn hsync_frequency(&self) -> Option<f64>;

    fn debug_state(&self) -> Vec<(String, VideoCardStateEntry)>;
}

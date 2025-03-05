/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2025 Daniel Balsom

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

    frontend_common::timestep_manager::mod.rs

    Timestep and rendering statistics manager.

*/

use marty_common::types::history_buffer::HistoryBuffer;
use std::{cell::Cell, default::Default, thread};
use web_time::{Duration, Instant};

const SECOND: Duration = Duration::from_secs(1);

const UPS_CAP: u64 = 1000; // Maximum number of window manager updates per second
const UPS_MIN_DURATION: Duration = Duration::from_millis(1000 / UPS_CAP as u64); // Minimum duration between window manager updates
const DEFAULT_EMU_FPS_TARGET: f32 = 60.0; // Default rendering FPS for the emulator
const FRAME_HISTORY_LEN: usize = 60; // Number of frames of history to keep

#[derive(Copy, Clone, Default)]
pub struct FrameEntry {
    pub emu_time:   Duration, // Time spent in the emulator core per frame
    pub frame_time: Duration, // All time spent rendering the frame
}

#[derive(Copy, Clone, Default)]
pub struct PerfCounter {
    pub accum: u32, // Count accumulator
    pub total: u32, // Total count this timespan
    pub last:  u32, // Total count for last timespan
}

impl PerfCounter {
    #[inline]
    pub fn tick(&mut self) {
        self.accum += 1;
    }
    #[inline]
    pub fn mark_interval(&mut self) {
        self.last = self.total; // Save last frame's count
        self.total = self.accum; // Save this frame's count
        self.accum = 0; // Reset accumulator
    }
}

/// A counter for tracking clock cycles / system ticks per frame.
/// 'update' should be called once per frame and returns the number of cycles
/// that elapsed since the previous frame.
#[derive(Copy, Clone, Default)]
pub struct CycleFrameCounter {
    pub last:    u64, // Cycle count at last update
    pub current: u64, // Current cycle count
}

impl CycleFrameCounter {
    pub fn update(&mut self, current: u64) -> u64 {
        self.last = self.current;
        self.current = current;
        self.current.saturating_sub(self.last)
    }
    pub fn cycles_per(&self) -> u64 {
        self.current.saturating_sub(self.last)
    }
}

#[derive(Copy, Clone, Default)]
pub struct HertzEvent {
    rate:   f32,
    target: Duration,
    accum:  Duration,
}

impl HertzEvent {
    pub fn new(rate: f32) -> Self {
        Self {
            rate,
            target: Duration::from_secs_f64(SECOND.as_secs_f64() / rate as f64),
            accum: Duration::from_secs(0),
        }
    }
    pub fn set(&mut self, rate: f32) {
        self.rate = rate;
        self.target = Duration::from_secs_f64(SECOND.as_secs_f64() / rate as f64);
    }
    pub fn get(&self) -> f32 {
        self.rate
    }
    #[inline]
    pub fn tick(&mut self, elapsed: Duration) -> bool {
        self.accum += elapsed;
        if self.accum >= self.target {
            self.accum -= self.target;
            true
        }
        else {
            false
        }
    }
}

#[derive(Copy, Clone, Default)]
pub struct PerfStats {
    pub wm_ups: PerfCounter,  // Number of updates per second from the window manager
    pub wm_fps: PerfCounter,  // Number of frames per second calculated from wm updates
    pub emu_ups: PerfCounter, // Number of updates per second performed by emulator core
    pub cpu_cycles: CycleFrameCounter,
    pub cpu_instructions: CycleFrameCounter,
    pub sys_ticks: CycleFrameCounter,
    pub emu_frames: CycleFrameCounter, // Frames reported rendered by the core
    pub emu_frame_time: Duration,      // Time spent in the emulator core per frame
    pub machine_time: Duration,        // Elapsed time from emulated machine perspective
    pub render_time: Duration,
    pub gui_time: Duration,
    pub frame_time: Duration,
}

#[derive(Copy, Clone, Default)]
pub struct PerfSnapshot {
    pub wm_ups: u32,
    pub wm_fps: u32,
    pub emu_ups: u32,
    pub cpu_cycles: u32,
    pub cpu_instructions: u32,
    pub sys_ticks: u32,
    pub emu_frames: u32,
    pub emu_frame_time: Duration,
    pub render_time: Duration,
    pub gui_time: Duration,
    pub frame_time: Duration,
    pub cpu_cycle_update_target: u32,
}

impl PerfStats {
    pub fn snapshot(&self, cpu_cycle_update_target: u32) -> PerfSnapshot {
        PerfSnapshot {
            wm_ups: self.wm_ups.total,
            wm_fps: self.wm_fps.total,
            emu_ups: self.emu_ups.total,
            cpu_cycles: self.cpu_cycles.cycles_per() as u32,
            cpu_instructions: self.cpu_instructions.cycles_per() as u32,
            sys_ticks: self.sys_ticks.cycles_per() as u32,
            emu_frames: self.emu_frames.cycles_per() as u32,
            emu_frame_time: self.emu_frame_time,
            render_time: self.render_time,
            gui_time: self.gui_time,
            frame_time: self.frame_time,
            cpu_cycle_update_target,
        }
    }
}

#[derive(Copy, Clone, Default)]
pub struct MachinePerfStats {
    pub cpu_mhz: f64,
    pub cpu_cycles: u64,
    pub cpu_instructions: u64,
    pub system_ticks: u64,
    pub emu_frames: Option<u64>,
}

#[derive(Default)]
pub struct TimestepUpdate {
    pub new_throttle_factor: Option<f64>,
}

pub struct TimestepManager {
    init: bool, // Has the timestep manager been initialized?
    second_rate: HertzEvent,
    emu_render_rate: HertzEvent, // Desired rendering FPS for the emulator. This can change depending on the video card.
    emu_update_rate: HertzEvent, // Desired update rate for the emulator.
    gui_render_rate: HertzEvent, // Desired rendering FPS for the GUI. Should be at least emu_render_rate.
    gui_update_rate: HertzEvent, // Desired update rate for the GUI. May be less than emu_fps_target.
    last_instant: Instant,
    last_frame_instant: Instant,
    current_instant: Instant,
    last_processed_wm_update: Instant, // The last time the window manager update was processed instead of sleeping

    cpu_mhz: f64,                 // Mhz of the primary emulated CPU (drives sys ticks)
    cpu_cycle_update_target: u32, // Number of CPU cycles to execute per emulator update
    frame_target: Duration,       // Target frame time in microseconds
    throttle_factor: Cell<f64>,   // Factor to adjust CPU cycle target by to keep up with emu_render_rate

    frame_history: HistoryBuffer<FrameEntry>,
    perf_stats: PerfStats,
    total_running_time: Duration,
    frame_due: bool,
}

impl Default for TimestepManager {
    fn default() -> Self {
        Self {
            init: false,
            second_rate: HertzEvent::new(1.0),
            emu_render_rate: HertzEvent::new(DEFAULT_EMU_FPS_TARGET),
            emu_update_rate: HertzEvent::new(DEFAULT_EMU_FPS_TARGET),
            gui_render_rate: HertzEvent::new(DEFAULT_EMU_FPS_TARGET),
            gui_update_rate: HertzEvent::new(DEFAULT_EMU_FPS_TARGET),
            last_instant: Instant::now(),
            last_frame_instant: Instant::now(),
            current_instant: Instant::now(),
            last_processed_wm_update: Instant::now(),

            cpu_mhz: 1.0,
            cpu_cycle_update_target: (1_000_000.0 / DEFAULT_EMU_FPS_TARGET) as u32,
            frame_target: Duration::from_secs_f64(SECOND.as_secs_f64() / DEFAULT_EMU_FPS_TARGET as f64),
            throttle_factor: Cell::new(1.0),

            frame_history: HistoryBuffer::new(FRAME_HISTORY_LEN),
            total_running_time: Duration::from_secs(0),
            perf_stats: PerfStats::default(),

            frame_due: false,
        }
    }
}

impl TimestepManager {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn start(&mut self) {
        self.init = true;
        self.last_instant = Instant::now();
        self.last_processed_wm_update = Instant::now();
        self.total_running_time = Duration::from_secs(0);
    }

    pub fn throttle_factor(&self) -> f64 {
        self.throttle_factor.get()
    }

    pub fn set_throttle_factor(&self, factor: f64) {
        self.throttle_factor.set(factor);
    }

    /// Process a window manager update.
    /// In winit 0.29.4+, this should be called in response to WindowEvent::RedrawRequested
    /// When a second has elapsed, the 'machine_callback' is called to retrieve the current
    /// CPU cycle count, system tick count, instruction count, and optionally the number of
    /// rendered frames from the primary video card (if present).
    pub fn wm_update<E, D, F, G, H>(
        &mut self,
        emu: &mut E,
        dm: &mut D,
        second_callback: F,
        mut emu_update_callback: G,
        mut emu_render_callback: H,
    ) where
        F: FnOnce(&mut E) -> MachinePerfStats,
        G: FnMut(&mut E, u32),
        H: FnMut(&mut E, &mut D, &TimestepManager, &PerfSnapshot, Duration, &mut TimestepUpdate),
    {
        if !self.init {
            self.start();
            return;
        }

        self.current_instant = Instant::now();
        let elapsed = self.last_instant.elapsed();
        self.last_instant = self.current_instant;

        // Ignore deltas that are too big (updates may have stopped due to drawing window, etc.
        // honoring large deltas will lead to audio queue backup
        if elapsed > self.frame_target * 2 {
            #[cfg(not(debug_assertions))]
            log::debug!("Ignoring oversized timestep: {:?}", elapsed);
            //self.last_instant = self.current_instant;
            return;
        }

        self.total_running_time += elapsed;
        self.perf_stats.wm_ups.tick();

        // Handle seconds
        if self.second_rate.tick(elapsed) {
            self.handle_second(emu, second_callback);
        }

        // Handle emu updates
        if self.emu_update_rate.tick(elapsed) {
            self.last_frame_instant = Instant::now();
            let emu_start = Instant::now();
            emu_update_callback(emu, self.cpu_cycle_update_target);
            self.perf_stats.emu_ups.tick();
            self.perf_stats.emu_frame_time = emu_start.elapsed();
        }

        // Handle emu frame render
        if self.emu_render_rate.tick(elapsed) {
            let snapshot = self.perf_stats.snapshot(self.cpu_cycle_update_target);

            // TODO: We can't give the callback mutable access to the timestep manager,
            //       but we could give it a struct it can update with new values.
            //       new_factor should probably be moved in there if we need anything else.
            let mut update_me = TimestepUpdate::default();
            emu_render_callback(emu, dm, &self, &snapshot, elapsed, &mut update_me);

            if let Some(factor) = update_me.new_throttle_factor {
                self.throttle_factor.set(factor);
                self.recalculate_target();
            }

            self.perf_stats.wm_fps.tick();
            self.perf_stats.frame_time = self.last_frame_instant.elapsed();

            self.frame_history.push(FrameEntry {
                emu_time:   self.perf_stats.emu_frame_time,
                frame_time: self.perf_stats.frame_time,
            });
        }

        thread::yield_now();
    }

    pub fn handle_second<E, F>(&mut self, emu: &mut E, second_callback: F)
    where
        F: FnOnce(&mut E) -> MachinePerfStats,
    {
        // One second has elapsed. Get the current ticks from the core and update counters.
        let MachinePerfStats {
            cpu_mhz,
            cpu_cycles,
            cpu_instructions,
            system_ticks,
            emu_frames,
        } = second_callback(emu);

        self.perf_stats.cpu_cycles.update(cpu_cycles);
        self.perf_stats.sys_ticks.update(system_ticks);
        self.perf_stats.cpu_instructions.update(cpu_instructions);
        if let Some(frames) = emu_frames {
            self.perf_stats.emu_frames.update(frames);
        }
        // Mark the end of the second for other counters
        self.perf_stats.wm_ups.mark_interval();
        self.perf_stats.wm_fps.mark_interval();
        self.perf_stats.emu_ups.mark_interval();
        //self.perf_stats.emu_fps.mark_interval();

        // If the CPU Mhz has changed, update the cycle target
        if cpu_mhz != self.cpu_mhz {
            self.set_cpu_mhz(cpu_mhz);
        }
    }

    pub fn set_emu_render_rate(&mut self, fps: f32) {
        self.emu_render_rate.set(fps);
        self.frame_target = Duration::from_micros(1_000_000 / self.emu_render_rate.get() as u64);
        log::info!(
            "Emulator render rate has changed to {} FPS, new frame target: {:.2}ms",
            self.emu_render_rate.get(),
            self.frame_target.as_secs_f64() * 1000.0,
        );
    }

    pub fn set_cpu_mhz(&mut self, mhz: f64) {
        self.cpu_mhz = mhz;
        self.recalculate_target();
    }

    fn recalculate_target(&mut self) {
        self.cpu_cycle_update_target =
            ((self.cpu_mhz * 1_000_000.0 / self.emu_update_rate.get() as f64) * self.throttle_factor.get()) as u32;
        log::info!(
            "CPU clock has changed to {:.4}Mhz, speed factor: {}, new cycle target: {}",
            self.cpu_mhz,
            self.throttle_factor.get(),
            self.cpu_cycle_update_target,
        );
    }

    pub fn set_emu_update_rate(&mut self, fps: f32) {
        self.emu_update_rate.set(fps);
    }

    pub fn set_gui_render_rate(&mut self, fps: f32) {
        self.gui_render_rate.set(fps);
    }

    pub fn set_gui_update_rate(&mut self, fps: f32) {
        self.gui_update_rate.set(fps);
    }

    pub fn get_perf_stats(&self) -> (&PerfStats, Vec<FrameEntry>) {
        (&self.perf_stats, self.frame_history.as_vec())
    }
}

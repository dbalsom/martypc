/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2024 Daniel Balsom

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

    ---------------------------------------------------------------------------

    egui::memory.rs

    Implements a memory viewer control.
    The control is a virtual window that can be scrolled over the entire
    address space. The virtual machine is polled for the contents of the
    active display as it is scrolled by sending GuiEvent::MemoryUpdate
    events.

*/

use crate::*;
use core::fmt;
use egui::CollapsingHeader;
use egui_plot::{GridMark, Line, Plot, PlotPoints};
use frontend_common::timestep_manager::{FrameEntry, PerfSnapshot};
use marty_common::util::format_duration;
use videocard_renderer::VideoParams;

pub struct PerformanceViewerControl {
    dti: Vec<DisplayInfo>,
    perf: PerfSnapshot,
    video_data: VideoParams,
    frame_history: Vec<FrameEntry>,
}

struct DisplayOption<T>(Option<T>);

impl<T: fmt::Debug> fmt::Debug for DisplayOption<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some(value) => write!(f, "{:?}", value),
            None => write!(f, "None"),
        }
    }
}

pub fn format_freq_counter(ct: u32) -> String {
    let mut ct = ct as f64;
    let suffix;
    if ct > 1_000_000.0 {
        ct /= 1_000_000.0;
        suffix = "MHz";
    }
    else if ct > 1_000.0 {
        ct /= 1_000.0;
        suffix = "KHz";
    }
    else {
        suffix = "Hz";
    }
    format!("{:.2}{}", ct, suffix)
}

impl PerformanceViewerControl {
    pub fn new() -> Self {
        Self {
            dti: Vec::new(),
            perf: Default::default(),
            video_data: Default::default(),
            frame_history: Vec::new(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
        egui::Grid::new("perf")
            .striped(true)
            .min_col_width(100.0)
            .show(ui, |ui| {
                for (i, dt) in self.dti.iter().enumerate() {
                    CollapsingHeader::new(&format!("Display {}: {} ({})", i, dt.name, dt.dtype))
                        .default_open(true)
                        .show(ui, |ui| {
                            egui::Grid::new("displays").striped(false).show(ui, |ui| {
                                ui.label("Backend: ");
                                ui.label(egui::RichText::new(dt.backend_name.clone()));
                                ui.end_row();

                                ui.label("SW Render Time: ");
                                ui.label(egui::RichText::new(format_duration(dt.render_time)));
                                ui.end_row();
                                ui.label("GUI Render Time: ");
                                ui.label(egui::RichText::new(format_duration(dt.gui_render_time)));
                                ui.end_row();
                            })
                        });
                    ui.end_row();
                }

                ui.label("Build: ");
                #[cfg(debug_assertions)]
                ui.label(egui::RichText::new("DEBUG".to_string()));
                #[cfg(not(debug_assertions))]
                ui.label(egui::RichText::new("Release".to_string()));
                ui.end_row();

                ui.label("Internal resolution: ");
                ui.label(egui::RichText::new(format!(
                    "{}, {}",
                    self.video_data.render.w, self.video_data.render.h
                )));
                ui.end_row();
                ui.label("Target resolution: ");
                ui.label(egui::RichText::new(format!(
                    "{}, {}",
                    self.video_data.aspect_corrected.w, self.video_data.aspect_corrected.h
                )));
                ui.end_row();

                ui.label("Window Manager UPS: ");
                ui.label(egui::RichText::new(format!("{}", self.perf.wm_ups)));
                ui.end_row();
                ui.label("Window Manager FPS: ");
                ui.label(egui::RichText::new(format!("{}", self.perf.wm_fps)));
                ui.end_row();
                ui.label("Emulated FPS: ");
                ui.label(egui::RichText::new(format!("{}", self.perf.emu_frames)));
                ui.end_row();
                ui.label("Effective CPU Freq: ");
                ui.label(egui::RichText::new(format_freq_counter(self.perf.cpu_cycles)));
                ui.end_row();
                ui.label("Effective Sys Freq: ");
                ui.label(egui::RichText::new(format_freq_counter(self.perf.sys_ticks)));
                ui.end_row();
                ui.label("IPS: ");
                ui.label(egui::RichText::new(format!("{}", self.perf.cpu_instructions)));
                ui.end_row();

                ui.label("Cycle Target: ");
                ui.label(egui::RichText::new(format!("{}", self.perf.cpu_cycle_update_target)));
                ui.end_row();

                ui.label("Emulation time: ");
                ui.label(egui::RichText::new(format_duration(self.perf.emu_time)));
                ui.end_row();

                ui.label("Total Frame time: ");
                ui.label(egui::RichText::new(format_duration(self.perf.frame_time)));
                ui.end_row();
            });

        ui.end_row();
        ui.horizontal(|ui| {
            let points: PlotPoints = self
                .frame_history
                .iter()
                .enumerate()
                .map(|(i, fe)| [i as f64, fe.frame_time.as_secs_f64() * 1000.0])
                .collect();

            let line = Line::new(points);
            let _x_mag = self.frame_history.len();
            Plot::new("frame_time_plot")
                .height(96.0)
                .allow_scroll(false)
                .allow_drag(false)
                .allow_zoom(false)
                .y_axis_width(2)
                .y_grid_spacer(|_spacer| {
                    vec![
                        // 100s
                        GridMark {
                            value: 16.7,
                            step_size: 16.7,
                        },
                    ]
                })
                .x_axis_formatter(|x, _, range| format!("{:.0}", range.end() - x))
                .show(ui, |plot_ui| {
                    plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max([0.0, 0.0], [60.0, 20.0]));

                    //plot_ui.set_auto_bounds(egui::Vec2b::new(true, false));
                    plot_ui.line(line);
                });
        });
    }

    pub fn update_video_data(&mut self, video_data: &VideoParams) {
        self.video_data = video_data.clone();
    }

    pub fn update(&mut self, dti: Vec<DisplayInfo>, perf: &PerfSnapshot, frame_history: Vec<FrameEntry>) {
        self.dti = dti;
        self.perf = *perf;
        self.frame_history = frame_history;
    }
}

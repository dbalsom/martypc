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
use frontend_common::timestep_manager::PerfSnapshot;
use marty_common::util::format_duration;
use videocard_renderer::VideoParams;

pub struct PerformanceViewerControl {
    adapter: String,
    backend: String,
    dti: Vec<DisplayInfo>,
    perf: PerfSnapshot,
    video_data: VideoParams,
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
    let mut suffix = "";
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
            adapter: String::new(),
            backend: String::new(),
            dti: Vec::new(),
            perf: Default::default(),
            video_data: Default::default(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
        egui::Grid::new("perf")
            .striped(true)
            .min_col_width(100.0)
            .show(ui, |ui| {
                ui.label("Adapter: ");
                ui.label(egui::RichText::new(format!("{}", self.adapter)));
                ui.end_row();

                ui.label("Backend: ");
                ui.label(egui::RichText::new(format!("{}", self.backend)));
                ui.end_row();

                for (i, dt) in self.dti.iter().enumerate() {
                    CollapsingHeader::new(&format!("Display {}: {} ({})", i, dt.name, dt.dtype))
                        .default_open(true)
                        .show(ui, |ui| {
                            egui::Grid::new("displays").striped(false).show(ui, |ui| {
                                // ui.label("Type: ");
                                // ui.label(egui::RichText::new(format!("{}", dt.dtype)));
                                // ui.end_row();
                                // ui.label("Video Type: ");
                                // ui.label(egui::RichText::new(format!("{:?}", DisplayOption(dt.vtype))));
                                // ui.end_row();
                                // ui.label("Card ID: ");
                                // ui.label(egui::RichText::new(format!(
                                //     "{:?}",
                                //     DisplayOption(dt.vid.and_then(|vid| { Some(vid.idx) }))
                                // )));
                                // ui.end_row();
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
                ui.label(egui::RichText::new(format!("{}", self.perf.emu_fps)));
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
                ui.label(egui::RichText::new(format!("{}", 0)));
                ui.end_row();

                ui.label("Emulation time: ");
                ui.label(egui::RichText::new(format_duration(self.perf.emu_time)));
                ui.end_row();
                ui.label("SW Render time: ");
                ui.label(egui::RichText::new(format_duration(self.perf.render_time)));
                ui.end_row();
                ui.label("Gui Render time: ");
                ui.label(egui::RichText::new(format_duration(self.perf.gui_time)));
                ui.end_row();
                ui.label("Total Frame time: ");
                ui.label(egui::RichText::new(format_duration(self.perf.frame_time)));
                ui.end_row();
            });
    }

    pub fn update_video_data(&mut self, video_data: &VideoParams) {
        self.video_data = video_data.clone();
    }

    pub fn update(&mut self, adapter: String, backend: String, dti: Vec<DisplayInfo>, perf: &PerfSnapshot) {
        self.adapter = adapter;
        self.backend = backend;
        self.dti = dti;
        self.perf = *perf;
    }
}

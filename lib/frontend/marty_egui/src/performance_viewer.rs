/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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
use videocard_renderer::VideoParams;

pub struct PerformanceViewerControl {
    stats: PerformanceStats,
    video_data: VideoParams,
}

impl PerformanceViewerControl {
    pub fn new() -> Self {
        Self {
            stats: Default::default(),
            video_data: Default::default(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
        egui::Grid::new("perf")
            .striped(true)
            .min_col_width(100.0)
            .show(ui, |ui| {
                ui.label("Adapter: ");
                ui.label(egui::RichText::new(format!("{}", self.stats.adapter)));
                ui.end_row();

                ui.label("Backend: ");
                ui.label(egui::RichText::new(format!("{}", self.stats.backend)));
                ui.end_row();

                ui.label("Build: ");
                #[cfg(debug_assertions)]
                ui.label(egui::RichText::new(format!("DEBUG")));
                #[cfg(not(debug_assertions))]
                ui.label(egui::RichText::new(format!("Release")));
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

                ui.label("UPS: ");
                ui.label(egui::RichText::new(format!("{}", self.stats.current_ups)));
                ui.end_row();
                ui.label("FPS: ");
                ui.label(egui::RichText::new(format!("{}", self.stats.current_fps)));
                ui.end_row();
                ui.label("Emulated FPS: ");
                ui.label(egui::RichText::new(format!("{}", self.stats.emulated_fps)));
                ui.end_row();
                ui.label("IPS: ");
                ui.label(egui::RichText::new(format!("{}", self.stats.current_ips)));
                ui.end_row();
                ui.label("Cycle Target: ");
                ui.label(egui::RichText::new(format!("{}", self.stats.cycle_target)));
                ui.end_row();
                ui.label("CPS: ");
                ui.label(egui::RichText::new(format!("{}", self.stats.current_cps)));
                ui.end_row();
                ui.label("TPS: ");
                ui.label(egui::RichText::new(format!("{}", self.stats.current_tps)));
                ui.end_row();
                ui.label("Emulation time: ");
                ui.label(egui::RichText::new(format!(
                    "{}",
                    ((self.stats.emulation_time.as_micros() as f64) / 1000.0)
                )));
                ui.end_row();
                ui.label("Framebuffer time: ");
                ui.label(egui::RichText::new(format!(
                    "{}",
                    ((self.stats.render_time.as_micros() as f64) / 1000.0)
                )));
                ui.end_row();
                ui.label("Gui Render time: ");
                ui.label(egui::RichText::new(format!(
                    "{}",
                    ((self.stats.gui_time.as_micros() as f64) / 1000.0)
                )));
                ui.end_row();
            });
    }

    pub fn update_video_data(&mut self, video_data: &VideoParams) {
        self.video_data = video_data.clone();
    }

    pub fn update_stats(&mut self, stats: &PerformanceStats) {
        let save_gui_time = self.stats.gui_time;
        self.stats = stats.clone();
        self.stats.gui_time = save_gui_time;
    }
}

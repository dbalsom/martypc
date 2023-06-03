/*
    Marty PC Emulator 
    (C)2023 Daniel Balsom
    https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

    ---------------------------------------------------------------------------

    egui::memory.rs

    Implements a memory viewer control.
    The control is a virtual window that can be scrolled over the entire 
    address space. The virtual machine is polled for the contents of the 
    active display as it is scrolled by sending GuiEvent::MemoryUpdate
    events.

*/

use std::collections::VecDeque;

use crate::egui::*;

pub struct PerformanceViewerControl {
    stats: PerformanceStats,
    video_data: VideoData,
}


impl PerformanceViewerControl {
    
    pub fn new() -> Self {
        Self {
            stats: Default::default(),
            video_data: Default::default()
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut VecDeque<GuiEvent> ) {
      
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

            ui.label("Internal resolution: ");
            ui.label(egui::RichText::new(format!("{}, {}", 
                self.video_data.render_w, 
                self.video_data.render_h))
                );
            ui.end_row();
            ui.label("Display buffer resolution: ");
            ui.label(egui::RichText::new(format!("{}, {}", 
                self.video_data.aspect_w, 
                self.video_data.aspect_h))
                );
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
            ui.label(egui::RichText::new(format!("{}", ((self.stats.emulation_time.as_micros() as f64) / 1000.0))));
            ui.end_row();
            ui.label("Render time: ");
            ui.label(egui::RichText::new(format!("{}", ((self.stats.render_time.as_micros() as f64) / 1000.0))));
            ui.end_row();
            ui.label("Gui Render time: ");
            ui.label(egui::RichText::new(format!("{}", ((self.stats.gui_time.as_micros() as f64) / 1000.0))));
            ui.end_row();                        
        });          
    }

    pub fn update_video_data(&mut self, video_data: VideoData ) {
        self.video_data = video_data;
    }

    pub fn update_stats(&mut self, stats: &PerformanceStats) {
        let save_gui_time = self.stats.gui_time;
        self.stats = stats.clone();
        self.stats.gui_time = save_gui_time;
    }
}
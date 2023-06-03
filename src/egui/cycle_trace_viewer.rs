/*
    MartyPC Emulator 
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
    
    egui::cycle_trace_viewer.rs

    Implements a viewer for the cycle trace of the last instruction.
    This is a simple edit control for now. Tokenizing with syntaxtokens
    may be interesting, but a bit complex for how niche this feature is.

*/

use std::collections::VecDeque;

use crate::egui::*;

pub struct CycleTraceViewerControl {

    pub content_str: String,
    pub instr_len: usize
}

impl CycleTraceViewerControl {
    pub fn new() -> Self {
        Self {
            content_str: String::new(),
            instr_len: 0,
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, events: &mut VecDeque<GuiEvent> ) {
        ui.horizontal(|ui| {
            ui.add_sized(ui.available_size(), 
                egui::TextEdit::multiline(&mut self.content_str)
                    .font(egui::TextStyle::Monospace));
            ui.end_row()
        });

        ui.separator();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Cycles:").text_style(egui::TextStyle::Monospace));
            ui.label(egui::RichText::new(format!("{}", self.instr_len)).text_style(egui::TextStyle::Monospace));
        }); 

    }

    pub fn update(&mut self, trace_vec: &Vec<String>) {

        self.instr_len = trace_vec.len();
        self.content_str = trace_vec.join("\n");
    }

}
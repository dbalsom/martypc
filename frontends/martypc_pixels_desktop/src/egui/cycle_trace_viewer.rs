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

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue ) {
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
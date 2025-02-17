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

    egui::cycle_trace_viewer.rs

    Implements a viewer for the cycle trace of the last instruction.
    This is a simple edit control for now. Tokenizing with syntaxtokens
    may be interesting, but a bit complex for how niche this feature is.

*/
use crate::*;
use egui_extras::{Column, TableBuilder};
use marty_core::{cpu_common::TraceMode, syntax_token::SyntaxToken};

pub struct CycleTraceViewerControl {
    pub mode: TraceMode,
    pub content_str: String,
    pub instr_len: usize,

    pub header_vec: Vec<String>,
    pub text_content: String,
    pub content: Vec<Vec<SyntaxToken>>,
    pub col_sizes: Vec<u32>,
    pub col_states: Vec<bool>,
}

impl CycleTraceViewerControl {
    pub fn new() -> Self {
        Self {
            mode: TraceMode::None,
            content_str: String::new(),
            instr_len: 0,
            header_vec: Vec::new(),
            text_content: String::new(),
            content: vec![vec![]],
            col_sizes: Vec::new(),
            col_states: Vec::new(),
        }
    }

    pub fn set_mode(&mut self, mode: TraceMode) {
        self.mode = mode;
    }

    pub fn set_header(&mut self, header_vec: Vec<String>) {
        self.header_vec = header_vec;
        self.col_sizes.clear();
        self.col_states.clear();
        for header_str in &self.header_vec {
            self.col_sizes.push(header_str.len() as u32);
            self.col_states.push(true);
        }
    }

    pub fn pad_header(&mut self, header_idx: usize) -> String {
        let mut header_str = self.header_vec[header_idx].clone();
        let mut pad_len = self.col_sizes[header_idx] - header_str.len() as u32;
        while pad_len > 0 {
            header_str.push(' ');
            pad_len -= 1;
        }
        header_str
    }

    pub fn col_select_menu(&mut self, _ui: &mut egui::Ui) {}

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Cycles:").text_style(egui::TextStyle::Monospace));
            ui.label(egui::RichText::new(format!("{}", self.instr_len)).text_style(egui::TextStyle::Monospace));
        });

        match self.mode {
            TraceMode::None => {
                ui.label("CPU tracing not enabled.");
                return;
            }
            TraceMode::CycleText => {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        ui.available_size(),
                        egui::TextEdit::multiline(&mut self.content_str).font(egui::TextStyle::Monospace),
                    );
                    ui.end_row()
                });
            }
            TraceMode::CycleCsv => {
                let mut table = TableBuilder::new(ui);

                for _ in self.header_vec.iter().rev().skip(1).rev() {
                    table = table.column(Column::auto().clip(true).resizable(true));
                }

                table
                    .auto_shrink(true)
                    .column(Column::auto().clip(true).resizable(false))
                    .header(20.0, |mut header| {
                        for (i, header_str) in self.header_vec.iter().enumerate() {
                            if !self.col_states[i] {
                                continue;
                            }
                            header
                                .col(|ui| {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(header_str)
                                                .text_style(egui::TextStyle::Monospace)
                                                .strong(),
                                        )
                                        .wrap(),
                                    );
                                })
                                .1
                                .context_menu(|ui| {
                                    for (idx, state) in self.col_states.iter_mut().enumerate() {
                                        ui.add(egui::Checkbox::new(state, self.header_vec[idx].clone()));
                                    }
                                });
                        }
                    })
                    .body(|mut body| {
                        for trace_row in &self.content {
                            body.row(20.0, |mut row| {
                                for (i, token) in trace_row.iter().enumerate() {
                                    if !self.col_states[i] {
                                        continue;
                                    }
                                    row.col(|ui| {
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(token.to_string())
                                                    .text_style(egui::TextStyle::Monospace),
                                            )
                                            .wrap(),
                                        );
                                    });
                                }
                            });
                        }
                    });
            }
            TraceMode::CycleSigrok => {
                ui.label("Cycle tracing in sigrok mode. No display available.");
            }
            TraceMode::Instruction => {
                ui.label("CPU tracing in instruction mode. No cycle tracing available.");
            }
        }

        //
    }

    pub fn update(&mut self, trace_vec: &Vec<String>) {
        self.instr_len = trace_vec.len();
        self.content_str = trace_vec.join("\n");
    }

    pub fn update_tokens(&mut self, trace_vec: &Vec<Vec<SyntaxToken>>) {
        self.instr_len = trace_vec.len();

        if trace_vec.len() > 0 && trace_vec[0].len() != self.header_vec.len() {
            log::warn!(
                "Cycle trace header length mismatch. Expected {}, got {}",
                self.header_vec.len(),
                trace_vec.len()
            );
        }

        self.content = trace_vec.clone();
    }
}

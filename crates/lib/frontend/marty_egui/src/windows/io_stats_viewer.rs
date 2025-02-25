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

    ---------------------------------------------------------------------------

    egui::io_stats_viewer.rs

    Implements a viewer for device IO statistics (reads and writes)

*/

use crate::{token_listview::*, *};
use marty_core::syntax_token::*;

const DEFAULT_ROWS: usize = 24;

pub struct IoStatsViewerControl {
    tlv: TokenListView,
    row: usize,
    content: Vec<Vec<SyntaxToken>>,
    scrolling: bool,
}

impl IoStatsViewerControl {
    pub fn new() -> Self {
        let mut tlv = TokenListView::new();
        tlv.set_capacity(DEFAULT_ROWS);
        tlv.set_visible(DEFAULT_ROWS);

        Self {
            tlv,
            row: 0,
            content: Vec::new(),
            scrolling: false,
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, events: &mut GuiEventQueue) {
        ui.horizontal(|ui| {
            if ui.button("Reset").on_hover_text("Reset statistics to 0").clicked() {
                events.send(GuiEvent::ResetIOStats);
            }
        });

        let mut new_row = self.row;
        ui.horizontal(|ui| {
            self.tlv
                .draw(ui, events, &mut new_row, &mut |_scrolled_to, _sevents| {});
        });

        // TLV viewport was scrolled, update address
        if self.row != new_row {
            //log::debug!("update row to: {}", new_row);
            self.row = new_row;
            self.scrolling = true;
        }
    }

    pub fn set_content(&mut self, ivt: Vec<Vec<SyntaxToken>>) {
        self.content = ivt;
        if !self.content.is_empty() {
            self.tlv.set_capacity(self.content.len());

            // Check if row is out of range first
            if self.row >= self.content.len() {
                self.row = 0;
            }
            self.tlv.set_contents(
                self.content[self.row..std::cmp::min(self.content.len(), self.row + DEFAULT_ROWS)].to_vec(),
                self.scrolling,
            );
        }
        else {
            self.row = 0;
        }
        self.scrolling = false;
    }

    pub fn reset(&mut self) {
        self.scrolling = false;
        self.row = 0;
        self.set_content(Vec::new());
    }
}

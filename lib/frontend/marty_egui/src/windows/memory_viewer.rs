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

use crate::{token_listview::*, *};
use marty_core::syntax_token::*;

pub struct MemoryViewerControl {
    pub address: String,
    pub row: usize,
    pub lastrow: usize,
    pub mem: Vec<String>,
    //update_scroll_pos: bool,
    tlv: TokenListView,
}

impl MemoryViewerControl {
    pub fn new() -> Self {
        Self {
            address: format!("{:05X}", 0),
            row: 0,
            lastrow: 0,
            mem: Vec::new(),
            //update_scroll_pos: false,
            tlv: TokenListView::new(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, events: &mut GuiEventQueue) {
        ui.horizontal(|ui| {
            ui.label("Address: ");
            if ui.text_edit_singleline(&mut self.address).changed() {
                events.send(GuiEvent::MemoryUpdate);
            }
        });
        ui.separator();

        self.tlv.set_capacity(0xFFFFF);
        self.tlv.set_visible(16);

        let mut new_row = self.row;
        ui.horizontal(|ui| {
            self.tlv.draw(ui, events, &mut new_row);
        });

        // TLV viewport was scrolled, update address
        if self.row != new_row {
            log::debug!("update address to: {:05X}", new_row);
            self.address = format!("{:05X}", new_row);
            self.row = new_row;
        }
    }

    #[allow(dead_code)]
    fn update_addr_from_row(&mut self) {
        self.address = format!("{:05X}", self.row);
    }

    pub fn set_row(&mut self, row: usize) {
        //log::warn!("Set row to {}", row & !0x0F);
        self.row = row & !0x0F;
    }

    #[allow(dead_code)]
    pub fn set_address(&mut self, address: String) {
        self.address = address;
    }

    pub fn get_address(&mut self) -> String {
        self.address.clone()
    }

    pub fn set_memory(&mut self, mem: Vec<Vec<SyntaxToken>>) {
        self.tlv.set_contents(mem);
    }

    pub fn set_hover_text(&mut self, text: String) {
        self.tlv.set_hover_text(text);
    }
}

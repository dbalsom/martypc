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
    pub address_input: String,
    pub address: String,
    pub address_source: InputFieldChangeSource,
    pub row: usize,
    pub row_span: usize,
    pub prev_row: usize,
    pub mem: Vec<String>,
    //update_scroll_pos: bool,
    tlv: TokenListView,
}

impl MemoryViewerControl {
    pub fn new() -> Self {
        Self {
            address_input: format!("{:05X}", 0),
            address: format!("{:05X}", 0),
            address_source: InputFieldChangeSource::None,
            row: 0,
            row_span: 16,
            prev_row: 0,
            mem: Vec::new(),
            //update_scroll_pos: false,
            tlv: TokenListView::new(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, events: &mut GuiEventQueue) {
        ui.horizontal(|ui| {
            ui.label("Address: ");
            if ui.text_edit_singleline(&mut self.address_input).lost_focus() {
                self.address = self.address_input.clone();
                self.address_source = InputFieldChangeSource::UserInput;
            }
            // if ui.text_edit_singleline(&mut self.address_input).lost_focus() {
            //     log::debug!("text edit changed to {}", self.address_input);
            //     let new_address_res = usize::from_str_radix(&self.address_input, 16);
            //     if let Ok(new_address) = new_address_res {
            //         log::debug!(
            //             "text edit is valid hex, {:05X}, row: {} [{:05X}]",
            //             new_address,
            //             new_address / self.row_span,
            //             new_address / self.row_span
            //         );
            //         self.address = format!("{:05X}", new_address);
            //         self.
            //         self.tlv.set_scroll_pos(new_address / self.row_span);
            //         events.send(GuiEvent::MemoryUpdate);
            //     }
            // }
        });
        ui.separator();

        self.tlv.set_capacity(0x10000);
        self.tlv.set_visible(16);

        let mut new_row = self.row;
        let mut scrolled_to_opt = None;
        ui.horizontal(|ui| {
            self.tlv.draw(ui, events, &mut new_row, &mut |scrolled_to, sevents| {
                scrolled_to_opt = Some(scrolled_to);
                sevents.send(GuiEvent::MemoryUpdate);
            });
        });

        // TLV viewport was scrolled, update address
        // if self.row != new_row {
        //     let new_address = new_row * self.row_span;
        //     log::debug!("update address to: {:05X}", new_address);
        //     self.address_input = format!("{:05X}", new_address);
        //     self.address = self.address_input.clone();
        //     self.address_source = InputFieldChangeSource::ScrollEvent;
        //     self.row = new_row;
        // }

        if let Some(scrolled_to) = scrolled_to_opt {
            self.update_addr_from_scroll(scrolled_to);
            self.row = scrolled_to;
        }

        self.prev_row = self.row;
    }

    #[allow(dead_code)]
    fn update_addr_from_row(&mut self) {
        self.address_input = format!("{:05X}", self.row * self.row_span);
    }

    fn update_addr_from_scroll(&mut self, new_pos: usize) {
        self.address_input = format!("{:05X}", new_pos * self.row_span);
        self.address = self.address_input.clone();
        self.address_source = InputFieldChangeSource::ScrollEvent;
    }

    pub fn set_address(&mut self, addr: usize) {
        let new_addr = addr & 0xFFFFF;
        self.row = new_addr & !(self.row_span - 1);

        if self.row != self.prev_row {
            log::debug!("set_address: {:04X}, row {}", addr, self.row);
            self.tlv.set_scroll_pos(self.row / self.row_span);
        }
    }

    // #[allow(dead_code)]
    // pub fn set_address(&mut self, address: String) {
    //     self.address_input = address;
    // }

    pub fn get_address(&mut self) -> (&str, InputFieldChangeSource) {
        (&self.address, self.address_source)
    }

    pub fn set_memory(&mut self, mem: Vec<Vec<SyntaxToken>>) {
        self.tlv.set_contents(mem, false);
    }

    pub fn set_hover_text(&mut self, text: String) {
        self.tlv.set_hover_text(text);
    }
}

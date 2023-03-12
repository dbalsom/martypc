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


    egui::memory.rs

    Implements a memory viewer control.
    The control is a virtual window that can be scrolled over the entire 
    address space. The virtual machine is polled for the contents of the 
    active display as it is scrolled by sending GuiEvent::MemoryUpdate
    events.

*/

use std::collections::VecDeque;

use crate::egui::*;
use crate::egui::token_listview::*;
use crate::syntax_token::*;

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
            tlv: TokenListView::new()
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, events: &mut VecDeque<GuiEvent> ) {

        ui.horizontal(|ui| {
            ui.label("Address: ");
            if ui.text_edit_singleline(&mut self.address).changed() {
                events.push_back(GuiEvent::MemoryUpdate);
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

    #[allow (dead_code)]
    fn update_addr_from_row(&mut self) {
        self.address = format!("{:05X}", self.row);
    }

    pub fn set_row(&mut self, row: usize) {
        //log::warn!("Set row to {}", row & !0x0F);
        self.row = row & !0x0F;
    }

    #[allow (dead_code)]
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
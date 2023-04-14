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


    egui::disassembly.rs

    Implements a disassembly viewer control.
    The control is a virtual window that will display the disassembly of 
    the next X instructions from the specified address. This address can
    be an expression, such as 'cs:ip'

*/
use std::collections::VecDeque;

use crate::egui::*;
use crate::egui::token_listview::*;
use crate::syntax_token::*;

pub struct DisassemblyControl {

    pub address: String,
    pub row: usize,
    pub lastrow: usize,
    tlv: TokenListView,
}

impl DisassemblyControl {

    pub fn new() -> Self {
        Self {
            address: "cs:ip".to_string(),
            row: 0,
            lastrow: 0,
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

        self.tlv.set_capacity(24);
        self.tlv.set_visible(24);

        let mut new_row = self.row;
        ui.horizontal(|ui| {
            self.tlv.draw(ui, events, &mut new_row);
        });
    }

    pub fn set_content(&mut self, mem: Vec<Vec<SyntaxToken>>) {
        self.tlv.set_contents(mem);
    }

    pub fn set_address(&mut self, address: String) {
        self.address = address;
    }

    pub fn get_address(&mut self) -> String {
        self.address.clone()
    }
}
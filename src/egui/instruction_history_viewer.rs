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
    
    egui::instruction_history_viewer.rs

    Implements the instruction history viewer control.
    The control is a virtual window that will display the disassembly of 
    the last X executed instructions. 

*/
use std::collections::VecDeque;

use crate::egui::*;
use crate::egui::token_listview::*;
use crate::syntax_token::*;

pub struct InstructionHistoryControl {

    pub address: String,
    pub row: usize,
    pub lastrow: usize,
    tlv: TokenListView,
}

impl InstructionHistoryControl {

    pub fn new() -> Self {
        Self {
            address: "cs:ip".to_string(),
            row: 0,
            lastrow: 0,
            tlv: TokenListView::new()
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, events: &mut VecDeque<GuiEvent> ) {

        self.tlv.set_capacity(32);
        self.tlv.set_visible(32);

        let mut new_row = self.row;
        ui.horizontal(|ui| {
            self.tlv.draw(ui, events, &mut new_row);
        });
    }

    pub fn set_content(&mut self, mem: Vec<Vec<SyntaxToken>>) {
        self.tlv.set_contents(mem);
    }

    #[allow (dead_code)]
    pub fn set_address(&mut self, address: String) {
        self.address = address;
    }
    
    #[allow (dead_code)]
    pub fn get_address(&mut self) -> String {
        self.address.clone()
    }
}
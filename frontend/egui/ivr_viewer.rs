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

    egui::ivt_viewer.rs

    Implements the a viewer for the IVT (Interrupt Vector Table)

*/

use crate::egui::*;
use crate::egui::token_listview::*;
use marty_core::syntax_token::*;

pub struct IvrViewerControl {

    tlv: TokenListView,
    row: usize,
}

impl IvrViewerControl {

    pub fn new() -> Self {
        let mut tlv = TokenListView::new();
        tlv.set_capacity(256);
        tlv.set_visible(32);

        Self {
            tlv,
            row: 0
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, events: &mut VecDeque<GuiEvent> ) {

        let mut new_row = self.row;
        ui.horizontal(|ui| {
            self.tlv.draw(ui, events, &mut new_row);
        });

        // TLV viewport was scrolled, update address
        if self.row != new_row {
            log::debug!("update address to: {:05X}", new_row);
            self.row = new_row;
        }        
    }        

    pub fn set_content(&mut self, mem: Vec<Vec<SyntaxToken>>) {
        self.tlv.set_contents(mem);
    }
}    
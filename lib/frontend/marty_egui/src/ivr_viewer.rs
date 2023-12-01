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

    egui::ivt_viewer.rs

    Implements the a viewer for the IVT (Interrupt Vector Table)

*/

use crate::*;
use crate::token_listview::*;
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

    pub fn draw(&mut self, ui: &mut egui::Ui, events: &mut GuiEventQueue ) {

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
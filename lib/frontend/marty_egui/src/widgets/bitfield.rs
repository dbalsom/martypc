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

    --------------------------------------------------------------------------

    marty_egui::widgets::bitfield.rs

    Implements a custom control that displays a bitfield, with the bit values
    on top and the field descriptors beneath.

*/

use egui::Response;

pub struct BitFieldElement {
    pub label: String,
    pub value: u16,
    pub len:   u16,
}

pub struct BitFieldWidget {
    pub fields: Vec<BitFieldElement>,
}

impl BitFieldWidget {
    pub fn draw(&self, ui: &mut egui::Ui, fields: Vec<BitFieldElement>) -> Response {
        egui::Grid::new("scaler_adjust").striped(false).show(ui, |ui| {
            for field in &fields {
                ui.vertical(|ui| {
                    let mut label_str = String::new();
                    ui.label(write!(&label_str, "{:0width$b}", number, width = field.len as usize));
                    ui.label(egui::RichText::new(field.label.clone()).text_style(egui::TextStyle::Monospace));
                });
            }
            ui.end_row();
        })
    }
}

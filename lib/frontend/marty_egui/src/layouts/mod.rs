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

    marty_egui::layout::mod.rs

    Defines custom layout widgets for use in the MartyPC GUI.

*/

use egui::{style::Spacing, InnerResponse, Response};
use std::sync::atomic::{AtomicUsize, Ordering};

static LAYOUT_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub enum Layout {
    KeyValue,
}

pub struct MartyLayout {
    atomic_id: usize,
    id_str:    String,
    layout:    Layout,
}

impl MartyLayout {
    pub(crate) fn new(layout: Layout, id_str: &str) -> Self {
        let atomic_id = LAYOUT_COUNTER.fetch_add(1, Ordering::SeqCst);
        MartyLayout {
            atomic_id,
            id_str: id_str.to_string(),
            layout,
        }
    }

    pub fn show<F: FnOnce(&mut egui::Ui)>(&self, ui: &mut egui::Ui, content: F) -> Response {
        match self.layout {
            Layout::KeyValue => self.key_value_layout(ui, content).response,
        }
    }

    fn key_value_layout<F: FnOnce(&mut egui::Ui)>(&self, ui: &mut egui::Ui, content: F) -> InnerResponse<()> {
        egui::Grid::new(format!("ml_{}", self.id_str))
            .num_columns(2)
            .striped(false)
            .min_col_width(100.0)
            .spacing(egui::Vec2::from((Spacing::default().item_spacing.x, 6.0f32)))
            .show(ui, content)
    }

    pub fn kv_row<F: FnOnce(&mut egui::Ui)>(ui: &mut egui::Ui, label: &str, _min_width: Option<f32>, content: F) {
        ui.label(label);
        ui.horizontal(|ui| {
            content(ui);
        });
        ui.end_row();
    }

    fn gen_id(&self, id: &str) -> String {
        format!("{}-{}", self.atomic_id, id)
    }
}

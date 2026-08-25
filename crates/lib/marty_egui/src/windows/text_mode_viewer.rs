/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

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

    -------------------------------------------------------------------------

    marty_egui::windows::text_mode_viewer.rs

    A simple viewer for a VideoCard's text mode dumps.
*/

use crate::{layouts, layouts::MartyLayout, GuiEventQueue};
use marty_core::device_traits::videocard::VideoCardId;
use std::collections::HashMap;

pub struct TextModeViewer {
    content_strs: HashMap<VideoCardId, String>,
    cards: Vec<(VideoCardId, String)>,
    card_idx: usize,
    updates: Vec<u64>,
}

impl TextModeViewer {
    pub fn new() -> Self {
        Self {
            content_strs: HashMap::new(),
            cards: Vec::new(),
            card_idx: 0,
            updates: Vec::new(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
        if self.card_idx < self.cards.len() {
            MartyLayout::new(layouts::Layout::KeyValue, "text-mode-card-grid").show(ui, |ui| {
                MartyLayout::kv_row(ui, "Updates", None, |ui| {
                    ui.label(self.updates[self.card_idx].to_string());
                });
                MartyLayout::kv_row(ui, "Card", None, |ui| {
                    egui::ComboBox::from_id_salt("text-mode-card-select")
                        .selected_text(&self.cards[self.card_idx].1)
                        .show_ui(ui, |ui| {
                            for (i, (_, desc)) in self.cards.iter().enumerate() {
                                ui.selectable_value(&mut self.card_idx, i, desc);
                            }
                        });
                });
            });
        }

        if let Some(card) = self.cards.get(self.card_idx).map(|(card, _)| *card) {
            ui.horizontal(|ui| {
                ui.add_sized(
                    ui.available_size(),
                    egui::TextEdit::multiline(self.get_str(card)).font(egui::TextStyle::Monospace),
                );
                ui.end_row()
            });
        }
    }

    fn get_str(&mut self, card_id: VideoCardId) -> &mut String {
        self.content_strs.entry(card_id).or_default()
    }

    pub fn select_card(&mut self, card_id: VideoCardId) {
        if let Some(card_idx) = self.cards.iter().position(|(card, _)| *card == card_id) {
            self.card_idx = card_idx;
        }
    }

    pub fn set_cards(&mut self, cards: Vec<(VideoCardId, String)>) {
        let selected_card = self.cards.get(self.card_idx).map(|(card, _)| *card);
        self.cards = cards;
        self.card_idx = selected_card
            .and_then(|selected| self.cards.iter().position(|(card, _)| *card == selected))
            .unwrap_or_default();
        self.updates.resize(self.cards.len(), 0);
    }

    pub fn set_content(&mut self, card_id: VideoCardId, content: Vec<String>) {
        if let Some(card_idx) = self.cards.iter().position(|(card, _)| *card == card_id) {
            self.content_strs.insert(card_id, content.join("\n"));
            self.updates[card_idx] += 1;
        }
    }
}

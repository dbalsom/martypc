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

    marty_egui::widgets::tab_group.rs

    Implements fake tabs with SelectableLabels
*/

#[allow(dead_code)]
pub struct MartyTabGroup {
    tab_labels:   Vec<String>,
    selected_tab: usize,
}

impl MartyTabGroup {
    pub fn new() -> Self {
        Self {
            tab_labels:   Vec::new(),
            selected_tab: 0,
        }
    }

    pub fn with_tab(mut self, label: &str) -> Self {
        self.add_tab(label);
        self
    }

    pub fn draw(&mut self, ui: &mut egui::Ui) -> egui::InnerResponse<()> {
        let selected_color = ui.visuals().widgets.active.fg_stroke.color;
        let response = ui.horizontal(|ui| {
            for (li, label_text) in self.tab_labels.iter().enumerate() {
                let label_text = if li == self.selected_tab {
                    egui::RichText::new(label_text).color(selected_color)
                }
                else {
                    egui::RichText::new(label_text)
                };

                if ui
                    .add(
                        egui::Label::new(label_text)
                            .selectable(false)
                            .sense(egui::Sense::click()),
                    )
                    .clicked()
                {
                    self.selected_tab = li;
                }
                if li < self.tab_labels.len() - 1 {
                    ui.separator();
                }
            }
        });

        response
    }

    pub fn add_tab(&mut self, label: &str) {
        self.tab_labels.push(label.to_string());
    }

    pub fn select_tab(&mut self, index: usize) {
        if index < self.tab_labels.len() {
            self.selected_tab = index;
        }
    }

    pub fn selected_tab(&self) -> usize {
        self.selected_tab
    }
}

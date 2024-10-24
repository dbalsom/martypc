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

    marty_egui::widgets::file_tree_menu.rs

    Creates a file tree menu from a FileTreeNode.

*/

use egui::RichText;
use frontend_common::FileTreeNode;
use std::collections::HashMap;

pub struct FileTreeMenu {
    root: FileTreeNode,
    selected_idx: HashMap<usize, Option<usize>>,
    file_icon: String,
}

impl FileTreeMenu {
    pub fn new() -> Self {
        Self {
            root: FileTreeNode::default(),
            selected_idx: HashMap::new(),
            file_icon: String::new(),
        }
    }

    pub fn with_file_icon(mut self, icon: &str) -> Self {
        self.file_icon = format!("{} ", icon);
        self
    }

    pub fn draw(&self, ui: &mut egui::Ui, ctx: usize, close: bool, clicked_fn: &mut dyn FnMut(usize) -> ()) {
        self.draw_node(ui, &self.root, ctx, close, clicked_fn);
    }

    pub fn draw_node(
        &self,
        ui: &mut egui::Ui,
        node: &FileTreeNode,
        ctx: usize,
        close: bool,
        clicked_fn: &mut dyn FnMut(usize) -> (),
    ) {
        for child in node.children() {
            if child.is_directory() {
                ui.menu_button(format!("📁 {}", child.name()), |ui| {
                    self.draw_node(ui, child, ctx, close, clicked_fn);
                });
            }
        }

        for child in node.children() {
            if !child.is_directory() {
                ui.horizontal(|ui| {
                    let button_text = if Some(child.idx()) == self.get_selected(ctx) {
                        RichText::new(format!("{}{}", self.file_icon, child.name())).color(egui::Color32::LIGHT_BLUE)
                    }
                    else {
                        RichText::new(format!("{}{}", self.file_icon, child.name()))
                    };
                    if ui.button(button_text).clicked() {
                        log::debug!("clicked on file {}, idx: {}", child.name(), child.idx());
                        clicked_fn(child.idx());
                        ui.close_menu();
                    }
                });
            }
        }
    }

    pub fn set_selected(&mut self, ctx: usize, idx: Option<usize>) {
        self.selected_idx.insert(ctx, idx);
    }

    pub fn get_selected(&self, ctx: usize) -> Option<usize> {
        *self.selected_idx.get(&ctx).unwrap_or(&None)
    }

    pub fn set_root(&mut self, root: FileTreeNode) {
        self.root = root;
    }
}

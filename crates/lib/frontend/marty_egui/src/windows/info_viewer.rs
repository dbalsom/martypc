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
*/
use crate::GuiEventQueue;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

pub struct InfoViewer {
    pub title: String,
    pub md_content: String,
    pub cache: CommonMarkCache,
}

impl Default for InfoViewer {
    fn default() -> Self {
        Self {
            title: "Info".to_string(),
            md_content: "".to_string(),
            cache: CommonMarkCache::default(),
        }
    }
}

impl InfoViewer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    pub fn set_content(&mut self, content: &str) {
        self.md_content = content.to_string();
    }

    pub fn show(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
        CommonMarkViewer::new().show(ui, &mut self.cache, &self.md_content);
    }
}

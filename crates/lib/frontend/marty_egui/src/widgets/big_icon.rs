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

    marty_egui::widgets::big_icon.rs

    Displays a big icon designed to flow in-line with text.
*/

use egui::Response;

#[allow(dead_code)]
pub enum IconType {
    Info,
    Warning,
    Error,
    Floppy,
    HardDisk,
}

impl IconType {
    pub fn draw(&self, ui: &mut egui::Ui, color: Option<egui::Color32>) -> egui::InnerResponse<Response> {
        let response = ui.horizontal(|ui| {
            ui.add_space(6.0);
            let response = ui.horizontal_centered(|ui| self.icon(ui, color)).response;
            ui.add_space(6.0);
            response
        });
        response
    }

    fn icon(&self, ui: &mut egui::Ui, color: Option<egui::Color32>) -> Response {
        match self {
            IconType::Info => ui.label(
                egui::RichText::new("🛈")
                    .color(color.unwrap_or(ui.visuals().text_color()))
                    .font(egui::FontId::proportional(40.0)),
            ),
            IconType::Warning => ui.label(
                egui::RichText::new("⚠")
                    .color(color.unwrap_or(ui.visuals().warn_fg_color))
                    .font(egui::FontId::proportional(40.0)),
            ),
            IconType::Error => ui.label(
                egui::RichText::new("⛔")
                    .color(color.unwrap_or(ui.visuals().error_fg_color))
                    .font(egui::FontId::proportional(40.0)),
            ),
            IconType::Floppy => ui.label(
                egui::RichText::new("💾")
                    .color(color.unwrap_or(ui.visuals().text_color()))
                    .font(egui::FontId::proportional(40.0)),
            ),
            IconType::HardDisk => ui.label(
                egui::RichText::new("🖴")
                    .color(color.unwrap_or(ui.visuals().text_color()))
                    .font(egui::FontId::proportional(40.0)),
            ),
        }
    }
}

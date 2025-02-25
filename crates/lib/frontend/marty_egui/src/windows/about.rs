/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2025 Daniel Balsom

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

    egui::about.rs

    Implements the About dialog box for the emulator.

*/

use crate::*;

pub struct AboutDialog {
    //texture: Option<egui::TextureHandle>,
    _params: bool,
}

impl AboutDialog {
    pub fn new() -> Self {
        Self {
            //texture: None,
            _params: Default::default(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, _ctx: &Context, _events: &mut GuiEventQueue) {
        /*
        let about_texture: &egui::TextureHandle = self.texture.get_or_insert_with(|| {
            ctx.load_texture(
                "logo",
                get_ui_image(UiImage::Logo),
                Default::default()
            )
        });
        */

        //ui.image(about_texture, about_texture.size_vec2());
        ui.add(
            egui::Image::new(egui::include_image!("../../../../../../assets/marty_logo_about.png")), //        .fit_to_original_size(1.0),
        );

        ui.separator();
        ui.vertical(|ui| {
            ui.label(format!("MartyPC Version {}", env!("CARGO_PKG_VERSION")));
            ui.label("MartyPC is free software licensed under the MIT License.");
            ui.label("©2024 Daniel Balsom (GloriousCow)");

            ui.horizontal(|ui| {
                ui.label("Github:");
                ui.hyperlink("https://github.com/dbalsom/martypc");
            });
        });

        ui.separator();
        ui.vertical(|ui| {
            ui.label("Made possible by the work of:");
            ui.label(
                egui::RichText::new("reenigne, Ken Shirriff, modem7, phix")
                    .color(ui.visuals().strong_text_color())
                    .font(egui::FontId::proportional(16.0)),
            );
            ui.label("Special thanks to:");
            ui.label(
                egui::RichText::new(
                    "640KB, BigBass, VileR, Scali, Trixter, UtterChaos, n0p, raphnet, everyone on VOGONS and /r/emudev",
                )
                .color(ui.visuals().strong_text_color())
                .font(egui::FontId::proportional(16.0)),
            );
            ui.label("Dedicated to:");
            ui.label(
                egui::RichText::new("Near")
                    .color(ui.visuals().strong_text_color())
                    .font(egui::FontId::proportional(16.0)),
            );
        });
    }
}

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

    -------------------------------------------------------------------------

    egui::about.rs

    Implements the About dialog box for the emulator.

*/

use crate::*;

pub struct AboutDialog {
    texture: Option<egui::TextureHandle>,
    _params: bool
}


impl AboutDialog {

    pub fn new() -> Self {
        Self {
            texture: None,
            _params: Default::default(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, ctx: &Context, _events: &mut GuiEventQueue ) {

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
        ui.image(egui::include_image!("../../../../assets/marty_logo_about.png"));

        ui.separator();
        ui.vertical(|ui| {
            ui.label(format!("MartyPC Version {}", env!("CARGO_PKG_VERSION")));
            ui.label("MartyPC is free software licensed under the MIT License.");
            ui.label("©2023 Daniel Balsom (GloriousCow)");

            ui.label("Github:");
            ui.hyperlink("https://github.com/dbalsom/martypc");
        });

        ui.separator();
        ui.vertical(|ui| {
            ui.label("Shoutouts to:");
            ui.label(egui::RichText::new(
                "reenigne, modem7, phix, Bigbass, iqon, xkevio, google0101, raphnet, Artlav, cngsoft, 640KB, \
                    i509VCB, qeeg, Kelpsy, phire, VileR, Scali, UtterChaos, Alkaid, kado, peach.bot, Dillon, velocity, \
                    EMMIR, ThomW, Ratalon, Blackcat9, DianeOfTheMoon, Halen, TFO, DigitalSkunk, Heck"
            )
            .color(egui::Color32::WHITE)
            .font(egui::FontId::proportional(20.0)));
        });
        ui.separator();
        ui.label("Dedicated to Near.");

    }
}
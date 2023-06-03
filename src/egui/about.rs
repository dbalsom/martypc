/*
    Marty PC Emulator 
    (C)2023 Daniel Balsom
    https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

    -------------------------------------------------------------------------

    egui::about.rs

    Implements the About dialog box for the emulator.

*/

use crate::egui::*;

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

    pub fn draw(&mut self, ui: &mut egui::Ui, ctx: &Context, _events: &mut VecDeque<GuiEvent> ) {

        let about_texture: &egui::TextureHandle = self.texture.get_or_insert_with(|| {
            ctx.load_texture(
                "logo",
                get_ui_image(UiImage::Logo),
                Default::default()
            )
        });

        ui.image(about_texture, about_texture.size_vec2());

        ui.separator();
        ui.vertical(|ui| {
            ui.label(format!("MartyPC Version {}", env!("CARGO_PKG_VERSION")));
            ui.label("MartyPC is free software licensed under the GPLv3.");
            ui.label("©2023 Daniel Balsom (GloriousCow)");

            ui.label("Github:");
            ui.hyperlink("https://github.com/dbalsom/marty");
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
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

    egui::about.rs

    Implements the About dialog box for the emulator.

*/
use crate::{
    widgets::{about_logo::AboutLogoWidget, greets::GreetsWidget},
    *,
};
use egui::FontId;

const GREETS: &[&str] = &[
    "VileR",
    "Trixter",
    "UtterChaos",
    "phoenix",
    "n0p",
    "640KB",
    "BigBass",
    "Folkert",
    "raphnet",
    "twvd",
    "Smartest Blob",
    "sqpat",
    "modem7",
    "DigitalSkunk",
    "Alice Averlong",
    "Mamoru",
    "Hampa Hug",
    "TubeTime",
    "howprice",
    "DutchMagic",
    "Digitoxin",
    "Disk Blitz",
    "RobSmithDev",
    "eientei",
    "electroly",
    "MicroCoreLabs",
    "google0101",
    "Tape_Worm",
    "DDX",
    "Tom Harte",
    "John Novak",
    "joncampbell123",
    "Ian Scott",
    "Mike Brutman",
    "Lord Nightmare",
    "DonKale",
    "NewRisingSun",
    "Aaron Giles",
    "PickledDog",
    "Nicole Express",
    "VOGONS",
    "VCF",
    "r/emudev",
    "...and all of you!",
    "Thank you for your support!",
    "💾",
    "💾",
    "💾",
];

pub struct AboutDialog {
    greets: GreetsWidget,
    version: String,
    build_id: String,
    logo: AboutLogoWidget,
}

impl AboutDialog {
    pub fn new() -> Self {
        Self {
            greets: GreetsWidget::new(GREETS, FontId::monospace(20.0), 0.5),
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_id: "000000".to_string(),
            logo: AboutLogoWidget::new(),
        }
    }

    pub fn set_build_info(&mut self, version: impl Into<String>, build_id: impl Into<String>) {
        self.version = version.into();
        self.build_id = build_id.into();
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, _ctx: &Context, _events: &mut GuiEventQueue) {
        self.logo.show(ui);

        ui.separator();
        ui.vertical(|ui| {
            ui.label(format!("MartyPC Version {} [{}]", self.version, self.build_id));
            ui.label("MartyPC is free software licensed under the MIT License.");
            ui.label("©2022-2025 Daniel Balsom (GloriousCow)");

            ui.horizontal(|ui| {
                ui.label("Github:");
                ui.hyperlink("https://github.com/dbalsom/martypc");
            });
        });

        ui.separator();
        ui.vertical(|ui| {
            ui.label("Made possible by the work of:");
            ui.label(
                egui::RichText::new("reenigne, Ken Shirriff, Longshot, phix")
                    .color(ui.visuals().strong_text_color())
                    .font(egui::FontId::proportional(16.0)),
            );

            ui.label("Greets to:");
            self.greets.show(ui);

            ui.label("Dedicated to:");
            ui.label(
                egui::RichText::new("Near")
                    .color(ui.visuals().strong_text_color())
                    .font(egui::FontId::proportional(16.0)),
            );
        });
    }
}

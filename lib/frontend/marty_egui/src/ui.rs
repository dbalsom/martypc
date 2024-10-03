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

    egui::src::ui.rs

    Main UI drawing code for EGUI.
*/

use crate::state::GuiState;
use egui::Context;

impl GuiState {
    pub fn menu_ui(&mut self, ctx: &Context) {
        // Draw top menu bar
        egui::TopBottomPanel::top("menubar_container").show(ctx, |ui| {
            self.draw_menu(ui);
        });
    }

    /// Create the UI using egui.
    pub fn ui(&mut self, ctx: &Context) {
        // Init things that need the context
        self.toasts.show(ctx);
        self.data_visualizer.init(ctx.clone());
        self.floppy_viewer.init(ctx.clone());

        egui::Window::new("Warning")
            .open(&mut self.warning_dialog_open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("⚠")
                            .color(egui::Color32::YELLOW)
                            .font(egui::FontId::proportional(40.0)),
                    );
                    ui.label(&self.warning_string);
                });
            });

        egui::Window::new("Error")
            .open(&mut self.error_dialog_open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("❎")
                            .color(egui::Color32::RED)
                            .font(egui::FontId::proportional(40.0)),
                    );
                    ui.label(&self.error_string);
                });
            });

        self.draw_workspace(ctx);
    }
}

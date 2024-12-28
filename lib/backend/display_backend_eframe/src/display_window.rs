use eframe::{egui, egui::Grid};

pub struct DisplayWindow {
    pub open: bool,
}

impl Default for DisplayWindow {
    fn default() -> Self {
        DisplayWindow::new()
    }
}

impl DisplayWindow {
    pub fn new() -> DisplayWindow {
        DisplayWindow { open: true }
    }

    pub fn show(&mut self, ctx: &mut egui::Context, title: &str, texture: &egui::TextureHandle) {
        //log::debug!("DisplayWindow::show: title: {} open: {}", title, self.open);
        egui::Window::new(title).open(&mut self.open).show(ctx, |ui| {
            //ui.label("< INSERT EMULATOR HERE >");
            ui.image(texture);
        });
    }
}

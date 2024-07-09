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

    ---------------------------------------------------------------------------

    egui::data_visualizer.rs

    Implements a data visualizer that interprets data as a bitmap.
    Utilizes the pixel_canvas widget to display the data as a bitmap.

*/

use crate::{
    token_listview::*,
    widgets::pixel_canvas::{PixelCanvas, PixelCanvasDepth},
    *,
};
use egui::ScrollArea;
use marty_core::syntax_token::*;

pub const DEFAULT_WIDTH: u32 = 128;
pub const DEFAULT_HEIGHT: u32 = 128;
pub const MIN_WIDTH: u32 = 4;
pub const MIN_HEIGHT: u32 = 4;
pub const MAX_WIDTH: u32 = 1024;
pub const MAX_HEIGHT: u32 = 1024;

pub const ZOOM_LEVELS: usize = 4;
pub const DEFAULT_ZOOM: usize = 2;
pub const ZOOM_LUT: [f32; ZOOM_LEVELS] = [1.0, 2.0, 4.0, 8.0];
pub const ZOOM_STR_LUT: [&str; ZOOM_LEVELS] = ["1x", "2x", "4x", "8x"];

pub const DEFAULT_BPP: usize = 0;
pub const BPP_LUT: [PixelCanvasDepth; 4] = [
    PixelCanvasDepth::OneBpp,
    PixelCanvasDepth::TwoBpp,
    PixelCanvasDepth::FourBpp,
    PixelCanvasDepth::EightBpp,
];
pub const BPP_STR_LUT: [&str; 4] = ["1bpp", "2bpp", "4bpp", "8bpp"];

pub struct DataVisualizerControl {
    pub address_input: String,
    pub address: String,
    pub zoom_str: String,
    pub zoom_idx: usize,
    pub bpp: PixelCanvasDepth,
    w: u32,
    h: u32,
    use_device_palette: bool,
    canvas: Option<PixelCanvas>,
}

impl DataVisualizerControl {
    pub fn new() -> Self {
        Self {
            address_input: format!("{:05X}", 0),
            address: format!("{:05X}", 0),
            zoom_str: ZOOM_STR_LUT[DEFAULT_ZOOM].to_string(),
            zoom_idx: DEFAULT_ZOOM,
            bpp: BPP_LUT[DEFAULT_BPP],
            w: DEFAULT_WIDTH,
            h: DEFAULT_HEIGHT,
            use_device_palette: true,
            canvas: None,
        }
    }

    pub fn init(&mut self, ctx: egui::Context) {
        if self.canvas.is_none() {
            self.canvas = Some(PixelCanvas::new((DEFAULT_WIDTH, DEFAULT_HEIGHT), ctx));
        }
    }

    pub fn get_address(&self) -> &str {
        &self.address_input
    }

    pub fn get_required_data_size(&self) -> usize {
        if let Some(canvas) = &self.canvas {
            canvas.get_required_data_size()
        }
        else {
            0
        }
    }

    pub fn update_data(&mut self, data: Vec<u8>) {
        if let Some(canvas) = &mut self.canvas {
            //log::debug!("Updating data of length {}...", data.len());
            canvas.update_data(&data);
        }
    }

    pub fn update_palette_u8(&mut self, palette: Vec<[u8; 4]>) {
        let pal = palette
            .iter()
            .map(|p| egui::Color32::from_rgb(p[0], p[1], p[2]))
            .collect();
        if let Some(canvas) = &mut self.canvas {
            canvas.update_device_palette(pal);
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, events: &mut GuiEventQueue) {
        if let Some(canvas) = &mut self.canvas {
            ui.set_width(canvas.get_width());
        }

        ui.horizontal(|ui| {
            ui.label("Address:");
            ui.text_edit_singleline(&mut self.address_input);
            egui::ComboBox::from_label("Zoom")
                .selected_text(ZOOM_STR_LUT[self.zoom_idx])
                .show_ui(ui, |ui| {
                    for i in 0..ZOOM_LUT.len() {
                        if ui.selectable_value(&mut self.zoom_idx, i, ZOOM_STR_LUT[i]).clicked() {
                            if let Some(canvas) = &mut self.canvas {
                                canvas.set_zoom(ZOOM_LUT[self.zoom_idx]);
                            }
                        }
                    }
                });

            egui::ComboBox::from_label("BPP")
                .selected_text(BPP_STR_LUT[self.bpp as usize])
                .show_ui(ui, |ui| {
                    for i in 0..BPP_LUT.len() {
                        if ui.selectable_value(&mut self.bpp, BPP_LUT[i], BPP_STR_LUT[i]).clicked() {
                            if let Some(canvas) = &mut self.canvas {
                                canvas.set_bpp(self.bpp);
                            }
                        }
                    }
                });
        });

        let mut resize = false;
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::DragValue::new(&mut self.w)
                        .clamp_range(MIN_WIDTH..=MAX_WIDTH)
                        .prefix("w:")
                        .suffix("px")
                        .speed(0.25)
                        .update_while_editing(false),
                )
                .changed()
            {
                resize = true;
            }
            if ui
                .add(
                    egui::DragValue::new(&mut self.h)
                        .clamp_range(MIN_HEIGHT..=MAX_HEIGHT)
                        .prefix("h:")
                        .suffix("px")
                        .speed(0.25)
                        .update_while_editing(false),
                )
                .changed()
            {
                resize = true;
            }

            if ui.checkbox(&mut self.use_device_palette, "Device Palette").changed() {
                if let Some(canvas) = &mut self.canvas {
                    canvas.use_device_palette(self.use_device_palette);
                }
            }
        });

        if resize {
            if let Some(canvas) = &mut self.canvas {
                canvas.resize((self.w, self.h));
            }
        }

        if let Some(canvas) = &mut self.canvas {
            ui.separator();
            ui.set_width(canvas.get_width());
            canvas.draw(ui);
        }
    }
}

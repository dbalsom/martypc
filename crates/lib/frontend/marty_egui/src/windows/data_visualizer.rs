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

    ---------------------------------------------------------------------------

    egui::data_visualizer.rs

    Implements a data visualizer that interprets data as a bitmap.
    Utilizes the pixel_canvas widget to display the data as a bitmap.

*/
use crate::{
    glyphs::FontInfo,
    token_listview::*,
    widgets::pixel_canvas::{PixelCanvas, PixelCanvasDepth},
    GuiEventQueue,
};
use egui::ScrollArea;
use image;
use marty_common::find_unique_filename;
use marty_core::syntax_token::*;
use std::{fmt::Display, path::PathBuf};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

pub const DEFAULT_WIDTH: u32 = 128;
pub const DEFAULT_HEIGHT: u32 = 128;
pub const MIN_WIDTH: u32 = 4;
pub const MIN_HEIGHT: u32 = 4;
pub const MAX_WIDTH: u32 = 2048;
pub const MAX_HEIGHT: u32 = 1024;

pub const ZOOM_LEVELS: usize = 4;
pub const DEFAULT_ZOOM: usize = 0;
pub const ZOOM_LUT: [f32; ZOOM_LEVELS] = [1.0, 2.0, 4.0, 8.0];
pub const ZOOM_STR_LUT: [&str; ZOOM_LEVELS] = ["1x", "2x", "4x", "8x"];

pub const DEFAULT_BPP: usize = 1;
pub const BPP_LUT: [PixelCanvasDepth; 5] = [
    PixelCanvasDepth::Text,
    PixelCanvasDepth::OneBpp,
    PixelCanvasDepth::TwoBpp,
    PixelCanvasDepth::FourBpp,
    PixelCanvasDepth::EightBpp,
];
pub const BPP_STR_LUT: [&str; 5] = ["text", "1bpp", "2bpp", "4bpp", "8bpp"];

#[derive(EnumIter, Copy, Clone, PartialEq, Eq, Default)]
pub enum VizPreset {
    FourtyBy25,
    FourtyBy200,
    EightyBy25,
    EightyBy100,
    CgaLowRes320x200,
    CgaLowRes320x800,
    #[default]
    CgaHighRes640x200,
    EgaLowRes320x200,
    EgaLowRes320x1600,
    EgaRes640x350,
    VgaRes640x400,
    VgaRes640x480,
    Mode13h320x200,
}

impl Display for VizPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VizPreset::FourtyBy25 => write!(f, "40x25 Text"),
            VizPreset::FourtyBy200 => write!(f, "40x200 Text"),
            VizPreset::EightyBy25 => write!(f, "80x25 Text"),
            VizPreset::EightyBy100 => write!(f, "80x100 Text"),
            VizPreset::CgaLowRes320x200 => write!(f, "320x200 LowRes 2bpp"),
            VizPreset::CgaLowRes320x800 => write!(f, "320x800 LowRes 2bpp"),
            VizPreset::CgaHighRes640x200 => write!(f, "640x200 HighRes"),
            VizPreset::EgaLowRes320x200 => write!(f, "320x200 EGA"),
            VizPreset::EgaLowRes320x1600 => write!(f, "320x1600 EGA"),
            VizPreset::EgaRes640x350 => write!(f, "640x350 EGA"),
            VizPreset::VgaRes640x400 => write!(f, "640x400 VGA"),
            VizPreset::VgaRes640x480 => write!(f, "640x480 VGA"),
            VizPreset::Mode13h320x200 => write!(f, "320x200 Mode13h"),
        }
    }
}

pub struct PresetParameters {
    pub w:    u32,
    pub h:    u32,
    pub bpp:  PixelCanvasDepth,
    pub zoom: usize,
}

impl VizPreset {
    pub fn params(&self) -> PresetParameters {
        match self {
            VizPreset::FourtyBy25 => PresetParameters {
                w:    40 * 8,
                h:    25 * 8,
                bpp:  PixelCanvasDepth::Text,
                zoom: 1,
            },
            VizPreset::FourtyBy200 => PresetParameters {
                w:    40 * 8,
                h:    200 * 8,
                bpp:  PixelCanvasDepth::Text,
                zoom: 1,
            },
            VizPreset::EightyBy25 => PresetParameters {
                w:    80 * 8,
                h:    25 * 8,
                bpp:  PixelCanvasDepth::Text,
                zoom: 1,
            },
            VizPreset::EightyBy100 => PresetParameters {
                w:    80 * 8,
                h:    100 * 8,
                bpp:  PixelCanvasDepth::Text,
                zoom: 1,
            },
            VizPreset::CgaLowRes320x200 => PresetParameters {
                w:    320,
                h:    200,
                bpp:  PixelCanvasDepth::TwoBpp,
                zoom: 2,
            },
            VizPreset::CgaLowRes320x800 => PresetParameters {
                w:    320,
                h:    800,
                bpp:  PixelCanvasDepth::TwoBpp,
                zoom: 2,
            },
            VizPreset::CgaHighRes640x200 => PresetParameters {
                w:    640,
                h:    200,
                bpp:  PixelCanvasDepth::OneBpp,
                zoom: 0,
            },
            VizPreset::EgaLowRes320x200 => PresetParameters {
                w:    320,
                h:    200,
                bpp:  PixelCanvasDepth::FourBpp,
                zoom: 2,
            },
            VizPreset::EgaLowRes320x1600 => PresetParameters {
                w:    320,
                h:    1600,
                bpp:  PixelCanvasDepth::FourBpp,
                zoom: 2,
            },
            VizPreset::EgaRes640x350 => PresetParameters {
                w:    640,
                h:    350,
                bpp:  PixelCanvasDepth::FourBpp,
                zoom: 0,
            },
            VizPreset::VgaRes640x400 => PresetParameters {
                w:    640,
                h:    400,
                bpp:  PixelCanvasDepth::FourBpp,
                zoom: 0,
            },
            VizPreset::VgaRes640x480 => PresetParameters {
                w:    640,
                h:    480,
                bpp:  PixelCanvasDepth::FourBpp,
                zoom: 0,
            },
            VizPreset::Mode13h320x200 => PresetParameters {
                w:    320,
                h:    200,
                bpp:  PixelCanvasDepth::EightBpp,
                zoom: 2,
            },
        }
    }
}

pub struct DataVisualizerControl {
    pub address_input: String,
    pub address: String,
    pub zoom_str: String,
    pub zoom_idx: usize,
    pub bpp: PixelCanvasDepth,
    w: u32,
    h: u32,
    offset: usize,
    row_offset: usize,
    row_span: usize,
    use_device_palette: bool,
    canvas: Option<PixelCanvas>,
    font: FontInfo,
    dump_path: Option<PathBuf>,
    record: bool,
    active_preset: VizPreset,
}

impl DataVisualizerControl {
    pub fn new() -> Self {
        let active_preset = VizPreset::default();
        let params = active_preset.params();
        Self {
            address_input: format!("{:05X}", 0),
            address: format!("{:05X}", 0),
            zoom_str: ZOOM_STR_LUT[params.zoom].to_string(),
            zoom_idx: params.zoom,
            bpp: params.bpp,
            w: params.w,
            h: params.h,
            offset: 0,
            row_offset: 0,
            row_span: 0,
            use_device_palette: true,
            canvas: None,
            font: FontInfo::default(),
            dump_path: None,
            record: false,
            active_preset,
        }
    }

    pub fn init(&mut self, ctx: egui::Context) {
        if self.canvas.is_none() {
            let mut canvas = PixelCanvas::new((self.w, self.h), ctx);
            canvas.set_bpp(self.bpp);
            canvas.set_zoom(ZOOM_LUT[self.zoom_idx]);
            self.canvas = Some(canvas);
        }
    }

    pub fn get_address(&self) -> (&str, usize) {
        let mut offset = self.offset;

        let row_size_bits;
        let row_size_bytes;
        let offset;
        if let PixelCanvasDepth::Text = self.bpp {
            row_size_bits = ((self.w / self.font.w) as usize) * self.bpp.bits();
            row_size_bytes = row_size_bits / 8;
            // Force offset to even byte boundaries. This keeps us from displaying attributes
            // as glyphs.
            offset = self.row_offset * row_size_bytes + self.offset & !0x01;
        }
        else {
            row_size_bits = self.w as usize * self.bpp.bits();
            row_size_bytes = row_size_bits / 8;
            offset = self.row_offset * row_size_bytes + self.offset;
        }

        // log::warn!(
        //     "Calculated row size: Bitmap w: {}, bits: {}, bytes: {} offset: {:05X}",
        //     self.w,
        //     row_size_bits,
        //     row_size_bytes,
        //     offset
        // );
        (&self.address_input, offset)
    }

    pub fn get_required_data_size(&self) -> usize {
        if let Some(canvas) = &self.canvas {
            canvas.get_required_data_size(Some(&self.font))
        }
        else {
            0
        }
    }

    pub fn update_data(&mut self, data: Vec<u8>) {
        if let Some(canvas) = &mut self.canvas {
            //log::debug!("Updating data of length {}...", data.len());
            canvas.update_data(&data, Some(&self.font));

            if let Some(dump_path) = &self.dump_path {
                if self.record {
                    let filename = find_unique_filename(dump_path, "viz_dump", "png");
                    match canvas.save_buffer(&filename) {
                        Ok(_) => log::info!("Saved visualization to file: {}", filename.display()),
                        Err(e) => log::error!("Error saving visualization to file: {}", e),
                    }
                }
            }
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

    pub fn set_dump_path(&mut self, path: PathBuf) {
        self.dump_path = Some(path);
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
        if let Some(canvas) = &mut self.canvas {
            ui.set_width(canvas.get_width());
        }

        ui.horizontal(|ui| {
            ui.label("Address:");

            ui.add(egui::TextEdit::singleline(&mut self.address_input).desired_width(50.0));

            //ui.text_edit_singleline(&mut self.address_input);

            if ui
                .add(
                    egui::DragValue::new(&mut self.offset)
                        .clamp_range(0..=0xFFFFF)
                        .hexadecimal(5, false, true)
                        .prefix("byte_offs:")
                        .speed(0.5)
                        .update_while_editing(false),
                )
                .changed()
            {
                // Do stuff when offset changes
            }

            if ui
                .add(
                    egui::DragValue::new(&mut self.row_offset)
                        .clamp_range(0..=0xFFFFF)
                        .hexadecimal(5, false, true)
                        .prefix("row_offs:")
                        .speed(0.5)
                        .update_while_editing(false),
                )
                .changed()
            {
                // Do stuff when offset changes
            }

            egui::ComboBox::from_id_source("viz_zoom_combo")
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

            egui::ComboBox::from_id_source("viz_bpp_combo")
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
            ui.label("Preset:");

            egui::ComboBox::from_id_source("viz_preset_combo")
                .selected_text(&self.active_preset.to_string())
                .show_ui(ui, |ui| {
                    for preset in VizPreset::iter() {
                        if ui
                            .selectable_value(&mut self.active_preset, preset, preset.to_string())
                            .clicked()
                        {
                            let params = self.active_preset.params();
                            self.w = params.w;
                            self.h = params.h;
                            self.bpp = params.bpp;
                            if let Some(canvas) = &mut self.canvas {
                                canvas.set_bpp(self.bpp);
                            }
                            resize = true;
                        }
                    }
                });

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

            if let PixelCanvasDepth::Text = self.bpp {
                if ui
                    .add(
                        egui::DragValue::new(&mut self.font.max_scanline)
                            .clamp_range(1..=self.font.h)
                            .prefix("r9:")
                            .speed(0.25)
                            .update_while_editing(false),
                    )
                    .changed()
                {
                    resize = true;
                }
            }

            if ui
                .button("SavePNG")
                .on_hover_text("Save visualization to file.")
                .clicked()
            {
                if let Some(canvas) = self.canvas.as_mut() {
                    if let Some(dump_path) = &self.dump_path {
                        let filename = find_unique_filename(dump_path, "viz_dump", "png");

                        match canvas.save_buffer(&filename) {
                            Ok(_) => log::info!("Saved visualization to file: {}", filename.display()),
                            Err(e) => log::error!("Error saving visualization to file: {}", e),
                        }
                    }
                }
            }

            ui.checkbox(&mut self.record, "Record")
                .on_hover_text("Dump each frame produced.");
        });

        if resize {
            if let Some(canvas) = &mut self.canvas {
                canvas.resize((self.w, self.h), Some(&self.font));
            }
        }

        if let Some(canvas) = &mut self.canvas {
            ui.separator();
            ui.set_width(canvas.get_width());
            canvas.draw(ui);
        }
    }
}

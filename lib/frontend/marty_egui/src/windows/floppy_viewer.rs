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

    egui::floppy_viewer.rs

    Implements a viewer control for a floppy disk image.

*/
use crate::{
    constants::*,
    layouts::{Layout::KeyValue, MartyLayout},
    widgets::{sector_status::sector_status, tab_group::MartyTabGroup},
    *,
};
use crossbeam_channel as channel;
use crossbeam_utils::thread;
use std::sync::{Arc, Mutex};

use egui::{Label, Sense};
use fluxfox::{
    structure_parsers::DiskStructureGenericElement,
    visualization::{render_track_metadata_quadrant, RenderTrackMetadataParams, RotationDirection},
    DiskDataResolution,
    DiskImage,
};
use marty_core::devices::floppy_drive::FloppyImageState;

use crate::{
    widgets::pixel_canvas::{PixelCanvas, PixelCanvasDepth, PixelCanvasZoom},
    windows::data_visualizer::ZOOM_LUT,
};
use tiny_skia::{Color, Pixmap, PixmapPaint, Transform};

pub const SECTOR_ROW_SIZE: usize = 9;
pub const VIZ_RESOLUTION: u32 = 512;

#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum FloppyViewerResolution {
    Disk = 0,
    Head = 1,
    Track = 2,
    Sector = 3,
}

pub struct FloppyViewerControl {
    drive_idx:   usize,
    head_idx:    usize,
    track_idx:   usize,
    sector_idx:  usize,
    image_state: Vec<Option<FloppyImageState>>,
    tab_group:   MartyTabGroup,
    resolution:  FloppyViewerResolution,

    pixmap_pool: Vec<Arc<Mutex<Pixmap>>>,

    palette: HashMap<DiskStructureGenericElement, Color>,
    viz_pixmaps: Vec<Pixmap>,
    viz_unsupported: bool,
    viz_write_cts: Vec<u64>,
    canvas: Option<PixelCanvas>,
    rendered_disk: usize,
    draw_deferred: bool,
    deferred_ct: u32,
}

impl FloppyViewerControl {
    pub fn new() -> Self {
        let mut tab_group = MartyTabGroup::new();
        tab_group.add_tab("Track Layout");
        tab_group.add_tab("Sector Data");
        tab_group.add_tab("Disk View");

        let viz_light_red: Color = Color::from_rgba8(180, 0, 0, 255);

        let viz_orange: Color = Color::from_rgba8(255, 100, 0, 255);
        let vis_purple: Color = Color::from_rgba8(180, 0, 180, 255);
        let viz_cyan: Color = Color::from_rgba8(70, 200, 200, 255);
        let vis_light_purple: Color = Color::from_rgba8(185, 0, 255, 255);

        let pal_medium_green = Color::from_rgba8(0x38, 0xb7, 0x64, 0xff);
        let pal_dark_green = Color::from_rgba8(0x25, 0x71, 0x79, 0xff);
        let pal_dark_blue = Color::from_rgba8(0x29, 0x36, 0x6f, 0xff);
        let pal_medium_blue = Color::from_rgba8(0x3b, 0x5d, 0xc9, 0xff);
        let pal_light_blue = Color::from_rgba8(0x41, 0xa6, 0xf6, 0xff);
        let pal_dark_purple = Color::from_rgba8(0x5d, 0x27, 0x5d, 0xff);
        let pal_orange = Color::from_rgba8(0xef, 0x7d, 0x57, 0xff);
        let pal_dark_red = Color::from_rgba8(0xb1, 0x3e, 0x53, 0xff);

        Self {
            drive_idx: 0,
            head_idx: 0,
            track_idx: 0,
            sector_idx: 1,
            image_state: Vec::new(),
            tab_group,
            resolution: FloppyViewerResolution::Track,

            pixmap_pool: vec![
                Arc::new(Mutex::new(Pixmap::new(VIZ_RESOLUTION / 2, VIZ_RESOLUTION / 2).unwrap())),
                Arc::new(Mutex::new(Pixmap::new(VIZ_RESOLUTION / 2, VIZ_RESOLUTION / 2).unwrap())),
                Arc::new(Mutex::new(Pixmap::new(VIZ_RESOLUTION / 2, VIZ_RESOLUTION / 2).unwrap())),
                Arc::new(Mutex::new(Pixmap::new(VIZ_RESOLUTION / 2, VIZ_RESOLUTION / 2).unwrap())),
            ],

            palette: HashMap::from([
                (DiskStructureGenericElement::SectorData, pal_medium_green),
                (DiskStructureGenericElement::SectorBadData, pal_orange),
                (DiskStructureGenericElement::SectorDeletedData, pal_dark_green),
                (DiskStructureGenericElement::SectorBadDeletedData, viz_light_red),
                (DiskStructureGenericElement::SectorHeader, pal_light_blue),
                (DiskStructureGenericElement::SectorBadHeader, pal_medium_blue),
                (DiskStructureGenericElement::Marker, vis_purple),
            ]),

            viz_unsupported: true,
            viz_pixmaps: vec![Pixmap::new(VIZ_RESOLUTION, VIZ_RESOLUTION).unwrap(); 4],
            viz_write_cts: vec![0; 4],

            canvas: None,
            rendered_disk: 0,
            draw_deferred: false,
            deferred_ct: 0,
        }
    }

    pub fn init(&mut self, ctx: egui::Context) {
        if self.canvas.is_none() {
            let mut canvas = PixelCanvas::new((VIZ_RESOLUTION, VIZ_RESOLUTION), ctx);
            canvas.set_bpp(PixelCanvasDepth::Rgba);
            self.canvas = Some(canvas);
        }
    }

    pub fn reset(&mut self) {
        self.drive_idx = 0;
        self.head_idx = 0;
        self.track_idx = 0;
        self.sector_idx = 1;
        self.viz_write_cts = vec![0; 4];
        self.draw_deferred = false;
        self.deferred_ct = 0;
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
        self.tab_group.draw(ui);

        let tab = self.tab_group.selected_tab();
        self.resolution = match tab {
            0 => FloppyViewerResolution::Track,
            1 => FloppyViewerResolution::Sector,
            _ => FloppyViewerResolution::Head,
        };

        ui.separator();

        if self.drive_idx < self.image_state.len() {
            self.draw_selectors(self.resolution, ui, _events);

            match tab {
                0 => self.draw_track_layout(ui, _events),
                1 => self.draw_sector_data(ui, _events),
                _ => self.draw_disk_data(ui, _events),
            }
        }
    }

    pub fn draw_selectors(
        &mut self,
        resolution: FloppyViewerResolution,
        ui: &mut egui::Ui,
        _events: &mut GuiEventQueue,
    ) {
        MartyLayout::new(KeyValue, "floppy-image-drive-select-grid").show(ui, |ui| {
            MartyLayout::kv_row(ui, "Drive:", None, |ui| {
                egui::ComboBox::from_id_source("floppy-drive-select")
                    .selected_text(format!(
                        "{}/{}",
                        self.drive_idx,
                        self.image_state.len().saturating_sub(1)
                    ))
                    .show_ui(ui, |ui| {
                        for (i, _) in self.image_state.iter().enumerate() {
                            if ui.selectable_value(&mut self.drive_idx, i, format!("{}", i)).clicked() {
                                // Set write counter to 0 if drive is clicked so that we regen the visualization
                                // on the next update.
                                self.viz_write_cts[self.drive_idx] = 0;
                            };
                        }
                    });
            });

            if let Some(state) = &self.image_state[self.drive_idx] {
                if resolution > FloppyViewerResolution::Disk {
                    MartyLayout::kv_row(ui, "Head:", None, |ui| {
                        let _response = egui::ComboBox::from_id_source("head-idx-select")
                            .selected_text(format!("{}/{}", self.head_idx, state.heads.saturating_sub(1)))
                            .show_ui(ui, |ui| {
                                for i in 0..state.heads as usize {
                                    if ui.selectable_value(&mut self.head_idx, i, format!("{}", i)).clicked() {
                                        // Set write counter to 0 if head is clicked so that we regen the visualization
                                        // on the next update.
                                        self.viz_write_cts[self.drive_idx] = 0;
                                    }
                                }
                            })
                            .response;
                    });
                }
                if resolution > FloppyViewerResolution::Head {
                    MartyLayout::kv_row(ui, "Track:", None, |ui| {
                        let _response = egui::ComboBox::from_id_source("track-idx-select")
                            .selected_text(format!(
                                "{}/{}",
                                self.track_idx,
                                state.get_track_ct(self.head_idx).saturating_sub(1)
                            ))
                            .show_ui(ui, |ui| {
                                for i in 0..state.get_track_ct(self.head_idx) {
                                    ui.selectable_value(&mut self.track_idx, i, format!("{}", i));
                                }
                            })
                            .response;
                    });
                }
                if resolution > FloppyViewerResolution::Track {
                    MartyLayout::kv_row(ui, "Sector:", None, |ui| {
                        // Sectors are 1-indexed
                        egui::ComboBox::from_id_source("sector-idx-select")
                            .selected_text(format!(
                                "{}/{}",
                                self.sector_idx,
                                state.get_sector_ct(self.head_idx, self.track_idx)
                            ))
                            .show_ui(ui, |ui| {
                                for i in 0..state.get_sector_ct(self.head_idx, self.track_idx) {
                                    ui.selectable_value(&mut self.sector_idx, i + 1, format!("{}", i + i));
                                }
                            });
                    });
                }
            }
        });
    }

    pub fn draw_track_layout(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
        if let Some(state) = &self.image_state[self.drive_idx] {
            let track_opt = state
                .sector_map
                .get(self.head_idx)
                .and_then(|map| map.get(self.track_idx));
            if let Some(track) = track_opt {
                ui.group(|ui| {
                    if track.len() == 0 {
                        ui.horizontal(|ui| {
                            ui.label("No sectors found on this track.");
                        });
                        return;
                    }

                    let rows = (track.len() + (SECTOR_ROW_SIZE - 1)) / SECTOR_ROW_SIZE;

                    egui::Grid::new("floppy-sector-grid")
                        .striped(false)
                        .spacing([10.0, 10.0])
                        .show(ui, |ui| {
                            for row in 0..rows {
                                for col in 0..SECTOR_ROW_SIZE {
                                    let idx = row * SECTOR_ROW_SIZE + col;
                                    if idx < track.len() {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                egui::RichText::new(format!("s:{:02X}", track[idx].chsn.s()))
                                                    .monospace(),
                                            );
                                            sector_status(ui, &track[idx], true);
                                        });
                                    }
                                }
                                ui.end_row();
                            }
                        });
                });
            }
        }
        else {
            ui.horizontal(|ui| {
                ui.label("No image loaded.");
            });
        }
    }

    pub fn draw_sector_data(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {}

    pub fn draw_disk_data(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
        if let Some(canvas) = &mut self.canvas {
            if self.viz_unsupported {
                ui.horizontal(|ui| {
                    ui.label("Visualization not supported for this image.");
                });
                return;
            }

            if self.rendered_disk != self.drive_idx {
                ui.horizontal(|ui| {
                    ui.label("No disk image.");
                });
                return;
            }
            ui.set_width(canvas.get_width());
            canvas.draw(ui);

            if self.draw_deferred {
                if self.deferred_ct > 3 && canvas.has_texture() {
                    log::debug!("draw_disk_data(): Deferred updates ending.");
                    canvas.update_data(self.viz_pixmaps[self.drive_idx].data(), None);
                    self.draw_deferred = false;
                    self.deferred_ct = 0;
                }
                else {
                    canvas.update_data(self.viz_pixmaps[self.drive_idx].data(), None);
                    self.deferred_ct += 1;
                }
            }
        }
    }

    pub fn clear_visualization(&mut self, drive: usize) {
        self.viz_unsupported = true;
        self.rendered_disk = 0xFF;
        self.viz_write_cts[drive] = 0;
        self.viz_pixmaps[drive].fill(Color::TRANSPARENT);
    }

    pub fn update_visualization(&mut self, drive: usize, image: &DiskImage, write_ct: u64) {
        self.viz_unsupported = !matches!(
            image.resolution(),
            DiskDataResolution::BitStream | DiskDataResolution::FluxStream
        );
        if self.viz_unsupported || ((write_ct <= self.viz_write_cts[drive]) && (write_ct > 0)) {
            return;
        }

        log::debug!(
            "Updating visualization for drive {} (write_ct: {}, viz_write_ct:{})",
            drive,
            write_ct,
            self.viz_write_cts[drive]
        );
        self.viz_write_cts[drive] = write_ct;

        // Clear pixmap
        self.viz_pixmaps[drive].fill(Color::TRANSPARENT);

        let (sender, receiver) = channel::unbounded::<u8>();

        let side = self.head_idx as u8;

        for quadrant in 0..4 {
            //let disk = Arc::clone(&image);
            let pixmap = Arc::clone(&self.pixmap_pool[quadrant as usize]);
            let sender = sender.clone();
            let palette = self.palette.clone();

            let direction = match side {
                0 => RotationDirection::CounterClockwise,
                1 => RotationDirection::Clockwise,
                _ => panic!("Invalid side"),
            };

            let angle = 0.0;

            let min_radius_fraction = 0.333;
            let render_track_gap = 0.10;

            log::debug!("dispatching thread for quadrant {}", quadrant);
            thread::scope(|s| {
                s.spawn(move |_| {
                    let mut pixmap = pixmap.lock().unwrap();
                    //let l_disk = disk.lock().unwrap();
                    let track_ct = image.get_track_ct(side.into());
                    let render_params = RenderTrackMetadataParams {
                        quadrant,
                        head: side,
                        min_radius_fraction,
                        index_angle: angle,
                        track_limit: track_ct,
                        track_gap: render_track_gap,
                        direction,
                        palette,
                        draw_empty_tracks: true,
                        pin_last_standard_track: true,
                        ..Default::default()
                    };
                    match render_track_metadata_quadrant(image, &mut pixmap, &render_params) {
                        Ok(_) => {
                            log::debug!("...Rendered quadrant {}", quadrant);
                        }
                        Err(e) => {
                            log::error!("Error rendering quadrant: {}", e);
                        }
                    }

                    //println!("Sending quadrant over channel...");
                    match sender.send(quadrant) {
                        Ok(_) => {
                            log::debug!("...Sent!");
                        }
                        Err(e) => {
                            log::error!("Error sending quadrant: {}", e);
                        }
                    }
                });
            })
            .unwrap();
        }

        for (q, quadrant) in receiver.iter().enumerate() {
            log::debug!("Received quadrant {}, compositing...", quadrant);
            let (x, y) = match quadrant {
                0 => (0, 0),
                1 => (VIZ_RESOLUTION / 2, 0),
                2 => (0, VIZ_RESOLUTION / 2),
                3 => (VIZ_RESOLUTION / 2, VIZ_RESOLUTION / 2),
                _ => panic!("Invalid quadrant"),
            };

            let paint = PixmapPaint::default();

            self.viz_pixmaps[drive].draw_pixmap(
                x as i32,
                y as i32,
                self.pixmap_pool[quadrant as usize].lock().unwrap().as_ref(),
                &paint,
                Transform::identity(),
                None,
            );

            // Clear pixmap after compositing
            self.pixmap_pool[quadrant as usize]
                .lock()
                .unwrap()
                .as_mut()
                .fill(Color::TRANSPARENT);

            if q == 3 {
                break;
            }
        }

        log::debug!("Done compositing quadrants.");

        if let Some(canvas) = &mut self.canvas {
            if canvas.has_texture() {
                log::debug!("Updating canvas...");
                canvas.update_data(self.viz_pixmaps[drive].data(), None);
            }
            else {
                log::debug!("Canvas not initialized, deferring update...");
                self.draw_deferred = true;
            }
        }

        self.rendered_disk = drive;
    }

    pub fn set_drive_idx(&mut self, idx: usize) {
        self.drive_idx = idx;
    }

    pub fn get_drive_idx(&mut self) -> usize {
        self.drive_idx
    }

    pub fn update_state(&mut self, state: Vec<Option<FloppyImageState>>) {
        self.image_state = state;
        if self.drive_idx >= self.image_state.len() {
            self.drive_idx = self.image_state.len().saturating_sub(1);
        }
    }
}

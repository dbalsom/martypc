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

    egui::floppy_viewer.rs

    Implements a viewer control for a floppy disk image.

*/

use std::sync::{Arc, Mutex, RwLock};

use crate::{
    constants::*,
    layouts::{Layout::KeyValue, MartyLayout},
    widgets::{
        pixel_canvas::{PixelCanvas, PixelCanvasDepth, PixelCanvasZoom},
        sector_status::sector_status,
        tab_group::MartyTabGroup,
    },
    windows::data_visualizer::ZOOM_LUT,
    *,
};

use fluxfox::{prelude::*, track_schema::GenericTrackElement, visualization::prelude::*};
use fluxfox_egui::controls::disk_visualization::{DiskVisualization, VizEvent};

use marty_core::devices::floppy_drive::FloppyImageState;

use crossbeam_channel as channel;
use crossbeam_utils::thread;
use egui::{Label, Sense};
use fluxfox::visualization::rasterize_disk::rasterize_track_metadata_quadrant;
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
    init: bool,
    drive_idx: usize,
    head_idx: usize,
    track_idx: usize,
    sector_idx: usize,
    image_state: Vec<Option<FloppyImageState>>,
    tab_group: MartyTabGroup,
    resolution: FloppyViewerResolution,

    viz: [DiskVisualization; 4],

    palette: HashMap<GenericTrackElement, VizColor>,
    viz_unsupported: bool,
    viz_write_cts: Vec<u64>,

    rendered_disk: usize,
    draw_deferred: bool,
    deferred_ct:   u32,
}

impl FloppyViewerControl {
    pub fn new() -> Self {
        let mut tab_group = MartyTabGroup::new();
        tab_group.add_tab("Track Layout");
        tab_group.add_tab("Sector Data");
        tab_group.add_tab("Disk View");

        let viz_light_red: VizColor = VizColor::from_rgba8(180, 0, 0, 255);
        let vis_purple: VizColor = VizColor::from_rgba8(180, 0, 180, 255);
        let pal_medium_green = VizColor::from_rgba8(0x38, 0xb7, 0x64, 0xff);
        let pal_dark_green = VizColor::from_rgba8(0x25, 0x71, 0x79, 0xff);
        let pal_medium_blue = VizColor::from_rgba8(0x3b, 0x5d, 0xc9, 0xff);
        let pal_light_blue = VizColor::from_rgba8(0x41, 0xa6, 0xf6, 0xff);
        let pal_orange = VizColor::from_rgba8(0xef, 0x7d, 0x57, 0xff);

        Self {
            init: false,
            drive_idx: 0,
            head_idx: 0,
            track_idx: 0,
            sector_idx: 1,
            image_state: Vec::new(),
            tab_group,
            resolution: FloppyViewerResolution::Track,

            viz: [
                DiskVisualization::default(),
                DiskVisualization::default(),
                DiskVisualization::default(),
                DiskVisualization::default(),
            ],

            palette: HashMap::from([
                (GenericTrackElement::SectorData, pal_medium_green),
                (GenericTrackElement::SectorBadData, pal_orange),
                (GenericTrackElement::SectorDeletedData, pal_dark_green),
                (GenericTrackElement::SectorBadDeletedData, viz_light_red),
                (GenericTrackElement::SectorHeader, pal_light_blue),
                (GenericTrackElement::SectorBadHeader, pal_medium_blue),
                (GenericTrackElement::Marker, vis_purple),
            ]),

            viz_unsupported: true,
            viz_write_cts: vec![0; 4],

            rendered_disk: 0,
            draw_deferred: false,
            deferred_ct: 0,
        }
    }

    pub fn init(&mut self, ctx: egui::Context) {
        if !self.init {
            self.viz = [
                DiskVisualization::new(ctx.clone(), VIZ_RESOLUTION),
                DiskVisualization::new(ctx.clone(), VIZ_RESOLUTION),
                DiskVisualization::new(ctx.clone(), VIZ_RESOLUTION),
                DiskVisualization::new(ctx.clone(), VIZ_RESOLUTION),
            ];
            self.init = true;
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
            _ => FloppyViewerResolution::Disk,
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
                egui::ComboBox::from_id_salt("floppy-drive-select")
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
                        let _response = egui::ComboBox::from_id_salt("head-idx-select")
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
                        let _response = egui::ComboBox::from_id_salt("track-idx-select")
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
                        egui::ComboBox::from_id_salt("sector-idx-select")
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

    fn draw_sector_data(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {}

    fn draw_disk_data(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
        if self.viz[self.drive_idx].compatible {
            if let Some(new_event) = self.viz[self.drive_idx].show(ui) {
                match new_event {
                    VizEvent::NewSectorSelected { c, h, s_idx } => {
                        log::debug!("New sector selected: c:{} h:{}, s:{}", c, h, s_idx);

                        self.viz[self.drive_idx].update_selection(c, h, s_idx);
                    }
                    _ => {}
                }
            }
        }
        else {
            ui.label("Current disk image not compatible with visualization.");
            log::error!("Visualization not compatible with current disk image.");
        }
    }

    pub fn clear_visualization(&mut self, drive: usize) {
        self.viz_unsupported = true;
        self.rendered_disk = 0xFF;
        self.viz_write_cts[drive] = 0;
    }

    pub fn set_disk(&mut self, drive: usize, disk_lock: Arc<RwLock<DiskImage>>) {
        self.viz[drive % 4].update_disk(disk_lock);
        self.render(drive);
    }

    fn render(&mut self, drive: usize) {
        _ = self.viz[drive % 4].render_visualization(0);
        _ = self.viz[drive % 4].render_visualization(1);
    }

    pub fn update_visualization(&mut self, drive: usize, write_ct: u64) {
        if self.viz_write_cts[drive] < write_ct {
            log::debug!(
                "Updating visualization for drive {} (write_ct: {}, viz_write_ct:{})",
                drive,
                write_ct,
                self.viz_write_cts[drive]
            );
            self.viz_write_cts[drive] = write_ct;

            // Render here

            self.rendered_disk = drive;
        }
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

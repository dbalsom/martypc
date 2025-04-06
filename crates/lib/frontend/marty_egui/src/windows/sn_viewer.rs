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
*/

//! Implement a debug window for the SN76496 sound chip.

use egui::*;
use egui_plot::{Line, PlotPoints};
use marty_core::devices::sn76489::SnDisplayState;

/*
use egui::plot::{
    Line,
    //Plot,
    PlotPoints,
    //PlotBounds
};*/

use crate::{color::*, constants::*, widgets::vu_meter::VuMeter, *};

pub const COL_SPACING: f32 = 60.0;
pub const ROW_SPACING: f32 = 4.0;

#[allow(dead_code)]
pub struct SnViewerControl {
    sn_state: SnDisplayState,
    vu_enabled: bool,
    scope_enabled: bool,
}

impl SnViewerControl {
    pub fn new() -> Self {
        Self {
            sn_state: Default::default(),
            vu_enabled: false,
            scope_enabled: true,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.vu_enabled, "VU Meters");
                ui.checkbox(&mut self.scope_enabled, "Oscilloscopes");
            });
            for (i, channel) in self.sn_state.tone_channels.iter().enumerate() {
                CollapsingHeader::new(format!("Tone Generator {}", i + 1))
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.set_min_width(SN_VIEWER_WIDTH);
                            ui.group(|ui| {
                                ui.set_min_width(SN_VIEWER_WIDTH);

                                ui.vertical(|ui| {
                                    Grid::new(format!("sn_channel_view{}", i))
                                        .num_columns(2)
                                        .spacing([COL_SPACING, ROW_SPACING])
                                        .striped(true)
                                        .show(ui, |ui| {
                                            ui.label(RichText::new("Period").text_style(TextStyle::Monospace));
                                            ui.label(
                                                RichText::new(channel.period.to_string())
                                                    .text_style(TextStyle::Monospace),
                                            );
                                            ui.label(RichText::new("Counter").text_style(TextStyle::Monospace));
                                            ui.label(
                                                RichText::new(channel.counter.to_string())
                                                    .text_style(TextStyle::Monospace),
                                            );
                                            ui.end_row();

                                            ui.label(RichText::new("Attenuation").text_style(TextStyle::Monospace));
                                            ui.label(
                                                RichText::new(channel.attenuation.to_string())
                                                    .text_style(TextStyle::Monospace),
                                            );
                                            ui.end_row();
                                        });

                                    if self.vu_enabled {
                                        Grid::new(format!("sn_tone{}_vu_grid", i))
                                            .num_columns(2)
                                            .spacing([80.0, 4.0])
                                            .striped(true)
                                            .show(ui, |ui| {
                                                ui.label(RichText::new("Volume").text_style(TextStyle::Monospace));
                                                ui.add(VuMeter::new(16, channel.volume));
                                                ui.end_row();
                                            });
                                    }

                                    //ui.label(format!("{} pts", channel.scope.len()));
                                    if self.scope_enabled {
                                        self.show_scope(i, &channel.scope, ui);
                                    }
                                });
                            });
                        });
                    });
            }

            CollapsingHeader::new("Noise Generator")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.set_min_width(SN_VIEWER_WIDTH);
                        ui.group(|ui| {
                            ui.set_min_width(SN_VIEWER_WIDTH);

                            ui.vertical(|ui| {
                                Grid::new("sn_noise_grid")
                                    .num_columns(2)
                                    .spacing([COL_SPACING, ROW_SPACING])
                                    .striped(true)
                                    .show(ui, |ui| {
                                        ui.label(RichText::new("Feedback Mode").text_style(TextStyle::Monospace));
                                        ui.label(
                                            RichText::new(format!("{:?}", self.sn_state.noise_feedback))
                                                .text_style(TextStyle::Monospace),
                                        );
                                        ui.end_row();

                                        ui.label(RichText::new("Shift Rate").text_style(TextStyle::Monospace));
                                        ui.label(
                                            RichText::new(format!("{:?}", self.sn_state.noise_divider))
                                                .text_style(TextStyle::Monospace),
                                        );
                                        ui.end_row();

                                        ui.label(RichText::new("Attenuation").text_style(TextStyle::Monospace));
                                        ui.label(
                                            RichText::new(self.sn_state.noise_attenuation.to_string())
                                                .text_style(TextStyle::Monospace),
                                        );
                                        ui.end_row();
                                    });

                                if self.vu_enabled {
                                    Grid::new("sn_noise_vu_grid")
                                        .num_columns(2)
                                        .spacing([COL_SPACING, ROW_SPACING])
                                        .striped(true)
                                        .show(ui, |ui| {
                                            ui.label(RichText::new("Volume").text_style(TextStyle::Monospace));
                                            ui.add(VuMeter::new(16, self.sn_state.noise_volume));
                                            ui.end_row();
                                        });
                                }

                                //ui.label(format!("{} pts", self.sn_state.noise_scope.len()));
                                if self.scope_enabled {
                                    self.show_scope(4, &self.sn_state.noise_scope, ui);
                                }
                            });
                        });
                    });
                });
        });
    }

    pub fn show_scope(&self, i: usize, scope: &Vec<(u64, f32)>, ui: &mut Ui) {
        egui_plot::Plot::new(format!("sn_chan_plot{}", i))
            .view_aspect(2.0)
            .width(SN_VIEWER_WIDTH - 10.0)
            .height(75.0)
            .allow_scroll(false)
            .allow_zoom(false)
            .allow_boxed_zoom(false)
            .show_x(true)
            .show_y(true)
            .show_axes(false)
            .show_grid(false)
            .auto_bounds(Vec2b::new(true, false))
            .include_y(-1.125)
            .include_y(1.125)
            .center_y_axis(false)
            .show(ui, |plot_ui| {
                //ui.set_plot_bounds(PlotBounds::from_min_max([0.0, 0.0], [100.0, 1.0]));

                let points: PlotPoints = scope
                    .iter()
                    .map(|pt| {
                        let x = pt.0 as f64;
                        let y = pt.1 as f64;
                        [x, y]
                    })
                    .collect();

                plot_ui.line(Line::new(points).color(Color32::from_rgb(0, 255, 255)));
            });
    }

    pub fn update_state(&mut self, state: SnDisplayState) {
        self.sn_state = state;
    }
}

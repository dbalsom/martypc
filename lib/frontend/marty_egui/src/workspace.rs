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

    egui::src::workspace.rs

    Handles managing, saving and restoring workspace state.

    A workspace conceptually manages the state, position and size of all GUI
    windows.
*/

#![allow(dead_code)]

use crate::{state::GuiState, GuiWindow, WORKSPACE_WINDOWS};
use anyhow::Error;
use egui::{Context, Ui};
use std::collections::HashMap;

pub struct GuiWorkspaceConfig {}

impl GuiState {
    pub fn workspace_window_open_button(&mut self, ui: &mut Ui, win_enum: GuiWindow, close: bool) {
        let win_def = WORKSPACE_WINDOWS
            .get(&win_enum)
            .expect(format!("Invalid window enum: {:?}", win_enum).as_str());

        if ui.button(format!("{}...", win_def.menu)).clicked() {
            self.window_state.entry(win_enum).and_modify(|e| e.open = true);
            if close {
                ui.close_menu();
            }
        }
    }

    pub fn workspace_window_open_button_with(
        &mut self,
        ui: &mut Ui,
        win_enum: GuiWindow,
        close: bool,
        on_click: impl FnOnce(&mut Self) -> (),
    ) {
        let win_def = WORKSPACE_WINDOWS
            .get(&win_enum)
            .expect(format!("Invalid window enum: {:?}", win_enum).as_str());

        if ui.button(format!("{}...", win_def.menu)).clicked() {
            self.window_state.entry(win_enum).and_modify(|e| e.open = true);
            on_click(self);
            if close {
                ui.close_menu();
            }
        }
    }

    pub fn workspace_window_toggle_button(&mut self, ui: &mut Ui, win_enum: GuiWindow, close: bool) {
        let win_def = WORKSPACE_WINDOWS
            .get(&win_enum)
            .expect(format!("Invalid window enum: {:?}", win_enum).as_str());

        if ui.button(format!("{}...", win_def.menu)).clicked() {
            self.window_state.entry(win_enum).and_modify(|e| e.open = !e.open);
            if close {
                ui.close_menu();
            }
        }
    }

    pub fn window_flag(&mut self, window: GuiWindow) -> &mut bool {
        &mut self.window_state.get_mut(&window).unwrap().open
    }

    pub fn is_window_open(&self, window: GuiWindow) -> bool {
        if let Some(state) = self.window_state.get(&window) {
            state.open
        }
        else {
            false
        }
    }

    pub fn set_window_open(&mut self, window: GuiWindow, state: bool) {
        *self.window_flag(window) = state;
    }

    pub fn draw_workspace(&mut self, ctx: &Context) {
        for (win_enum, win_state) in self.window_state.iter_mut() {
            // Get definition of this window from the constant definitions
            let win_def = WORKSPACE_WINDOWS
                .get(win_enum)
                .expect(format!("Invalid window enum: {:?}", win_enum).as_str());

            let mut win = egui::Window::new(win_def.title)
                .open(&mut win_state.open)
                .resizable(win_def.resizable);

            win = win.default_width(win_def.width);

            if let Some(egui::Vec2 { x, .. }) = win_state.initial_size {
                win = win.default_width(x);
            }

            let inner_response_opt = win.show(ctx, |ui| match win_enum {
                GuiWindow::About => {
                    self.about_dialog.draw(ui, ctx, &mut self.event_queue);
                }
                GuiWindow::CpuControl => {
                    self.cpu_control.draw(ui, &mut self.option_flags, &mut self.event_queue);
                }
                GuiWindow::PerfViewer => {
                    self.perf_viewer.draw(ui, &mut self.event_queue);
                }
                GuiWindow::MemoryViewer => {
                    self.memory_viewer.draw(ui, &mut self.event_queue);
                }
                GuiWindow::CompositeAdjust => {
                    self.composite_adjust.draw(ui, &mut self.event_queue);
                }
                GuiWindow::ScalerAdjust => {
                    self.scaler_adjust.draw(ui, &mut self.event_queue);
                }
                GuiWindow::CpuStateViewer => {
                    self.cpu_viewer.draw(ui, &mut self.event_queue);
                }
                GuiWindow::InstructionHistoryViewer => {
                    self.trace_viewer.draw(ui, &mut self.event_queue);
                }
                GuiWindow::IvtViewer => {
                    self.ivt_viewer.draw(ui, &mut self.event_queue);
                }
                GuiWindow::IoStatsViewer => {
                    self.io_stats_viewer.draw(ui, &mut self.event_queue);
                }
                GuiWindow::DelayAdjust => {
                    self.delay_adjust.draw(ui, &mut self.event_queue);
                }
                GuiWindow::DeviceControl => {
                    self.device_control.draw(ui, &mut self.event_queue);
                }
                GuiWindow::DisassemblyViewer => {
                    self.disassembly_viewer.draw(ui, &mut self.event_queue);
                }
                GuiWindow::PitViewer => {
                    self.pit_viewer.draw(ui, &mut self.event_queue);
                }
                GuiWindow::SerialViewer => {
                    self.serial_viewer.draw(ui, &mut self.event_queue);
                }
                GuiWindow::PicViewer => {
                    self.pic_viewer.draw(ui, &mut self.event_queue);
                }
                GuiWindow::PpiViewer => {
                    self.ppi_viewer.draw(ui, &mut self.event_queue);
                }
                GuiWindow::DmaViewer => {
                    self.dma_viewer.draw(ui, &mut self.event_queue);
                }
                GuiWindow::VideoCardViewer => {
                    GuiState::draw_video_card_panel(ui, &self.videocard_state);
                }
                GuiWindow::DataVisualizer => {
                    self.data_visualizer.draw(ui, &mut self.event_queue);
                }
                GuiWindow::CallStack => {
                    self.call_stack_viewer.draw(ui, &mut self.event_queue);
                }
                GuiWindow::VHDCreator => {
                    self.vhd_creator.draw(ui, &mut self.event_queue);
                }
                GuiWindow::CycleTraceViewer => {
                    self.cycle_trace_viewer.draw(ui, &mut self.event_queue);
                }
                GuiWindow::TextModeViewer => {
                    self.text_mode_viewer.draw(ui, &mut self.event_queue);
                }
            });

            match inner_response_opt {
                Some(inner_response) => {
                    let win_pos = inner_response.response.rect.min;
                    win_state.pos = win_pos;
                }
                None => {
                    //log::warn!("Window {:?} returned None from show()", win_enum);
                }
            }
        }
    }

    pub fn get_workspace_config_string(&mut self) -> Result<String, Error> {
        let window_state: HashMap<_, _> = self.window_state.clone().into_iter().collect();
        let window_state_toml = toml::to_string_pretty(&window_state).unwrap_or_else(|_| {
            log::error!("Failed to serialize workspace state");
            return String::new();
        });

        Ok(window_state_toml)
    }
}

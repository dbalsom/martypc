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

use crate::{state::GuiState};
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
        self.toasts.show(ctx);

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

        /*        egui::Window::new("About")
            .open(self.window_open_flags.get_mut(&GuiWindow::About).unwrap())
            .show(ctx, |ui| {
                self.about_dialog.draw(ui, ctx, &mut self.event_queue);
            });

        egui::Window::new("Performance")
            .open(self.window_open_flags.get_mut(&GuiWindow::PerfViewer).unwrap())
            .show(ctx, |ui| {
                self.perf_viewer.draw(ui, &mut self.event_queue);
            });

        let foo = egui::Window::new("CPU Control")
            .open(self.window_open_flags.get_mut(&GuiWindow::CpuControl).unwrap())
            .show(ctx, |ui| {
                self.cpu_control.draw(ui, &mut self.option_flags, &mut self.event_queue);
                let window_position = ui.min_rect().min;
                //log::debug!("CPU Control window position: {:?}", window_position);
            });
        if let Some(ireponse) = foo {
            let min = ireponse.response.rect.min;
            log::debug!("CPU Control window position: {:?}", min);
        }

        egui::Window::new("Memory View")
            .open(self.window_open_flags.get_mut(&GuiWindow::MemoryViewer).unwrap())
            .resizable(true)
            .default_width(540.0)
            .show(ctx, |ui| {
                self.memory_viewer.draw(ui, &mut self.event_queue);
            });

        egui::Window::new("Instruction History")
            .open(
                self.window_open_flags
                    .get_mut(&GuiWindow::InstructionHistoryViewer)
                    .unwrap(),
            )
            .resizable(true)
            .default_width(540.0)
            .show(ctx, |ui| {
                self.trace_viewer.draw(ui, &mut self.event_queue);
            });

        egui::Window::new("Cycle Trace")
            .open(self.window_open_flags.get_mut(&GuiWindow::CycleTraceViewer).unwrap())
            .resizable(true)
            //.default_width(540.0)
            .show(ctx, |ui| {
                self.cycle_trace_viewer.draw(ui, &mut self.event_queue);
            });

        egui::Window::new("Call Stack")
            .open(self.window_open_flags.get_mut(&GuiWindow::CallStack).unwrap())
            .resizable(true)
            .default_width(540.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        ui.available_size(),
                        egui::TextEdit::multiline(&mut self.call_stack_string).font(egui::TextStyle::Monospace),
                    );
                    ui.end_row()
                });
            });

        egui::Window::new("Disassembly View")
            .open(self.window_open_flags.get_mut(&GuiWindow::DisassemblyViewer).unwrap())
            .resizable(true)
            .default_width(540.0)
            .show(ctx, |ui| {
                self.disassembly_viewer.draw(ui, &mut self.event_queue);
            });

        egui::Window::new("IVT Viewer")
            .open(self.window_open_flags.get_mut(&GuiWindow::IvtViewer).unwrap())
            .resizable(true)
            .default_width(400.0)
            .show(ctx, |ui| {
                self.ivt_viewer.draw(ui, &mut self.event_queue);
            });

        egui::Window::new("CPU State")
            .open(self.window_open_flags.get_mut(&GuiWindow::CpuStateViewer).unwrap())
            .resizable(false)
            .default_width(220.0)
            .show(ctx, |ui| {
                self.cpu_viewer.draw(ui, &mut self.event_queue);
            });

        egui::Window::new("Delay Adjust")
            .open(self.window_open_flags.get_mut(&GuiWindow::DelayAdjust).unwrap())
            .resizable(true)
            .default_width(800.0)
            .show(ctx, |ui| {
                self.delay_adjust.draw(ui, &mut self.event_queue);
            });

        egui::Window::new("Device Control")
            .open(self.window_open_flags.get_mut(&GuiWindow::DeviceControl).unwrap())
            .resizable(true)
            .default_width(400.0)
            .show(ctx, |ui| {
                self.device_control.draw(ui, &mut self.event_queue);
            });

        egui::Window::new("PIT View")
            .open(self.window_open_flags.get_mut(&GuiWindow::PitViewer).unwrap())
            .resizable(false)
            .min_width(600.0)
            .default_width(600.0)
            .show(ctx, |ui| {
                self.pit_viewer.draw(ui, &mut self.event_queue);
            });

        egui::Window::new("PIC View")
            .open(self.window_open_flags.get_mut(&GuiWindow::PicViewer).unwrap())
            .resizable(true)
            .default_width(600.0)
            .show(ctx, |ui| {
                self.pic_viewer.draw(ui, &mut self.event_queue);
            });

        egui::Window::new("PPI View")
            .open(self.window_open_flags.get_mut(&GuiWindow::PpiViewer).unwrap())
            .resizable(true)
            .default_width(600.0)
            .show(ctx, |ui| {

            });

        egui::Window::new("DMA View")
            .open(self.window_open_flags.get_mut(&GuiWindow::DmaViewer).unwrap())
            .resizable(false)
            .default_width(200.0)
            .show(ctx, |ui| {
                self.dma_viewer.draw(ui, &mut self.event_queue);
            });

        egui::Window::new("Video Card View")
            .open(self.window_open_flags.get_mut(&GuiWindow::VideoCardViewer).unwrap())
            .resizable(false)
            .default_width(300.0)
            .show(ctx, |ui| {
                GuiState::draw_video_card_panel(ui, &self.videocard_state);
            });

        egui::Window::new("Create VHD")
            .open(self.window_open_flags.get_mut(&GuiWindow::VHDCreator).unwrap())
            .resizable(false)
            .default_width(400.0)
            .show(ctx, |ui| {
                self.vhd_creator.draw(ui, &mut self.event_queue);
            });

        egui::Window::new("Composite Adjustment")
            .open(self.window_open_flags.get_mut(&GuiWindow::CompositeAdjust).unwrap())
            .resizable(false)
            .default_width(300.0)
            .show(ctx, |ui| {
                self.composite_adjust.draw(ui, &mut self.event_queue);
            });

        egui::Window::new("Scaler Adjustment")
            .open(self.window_open_flags.get_mut(&GuiWindow::ScalerAdjust).unwrap())
            .resizable(false)
            .default_width(300.0)
            .show(ctx, |ui| {
                self.scaler_adjust.draw(ui, &mut self.event_queue);
            });

        egui::Window::new("Text Mode Viewer")
            .open(self.window_open_flags.get_mut(&GuiWindow::TextModeViewer).unwrap())
            .resizable(true)
            .default_width(600.0)
            .show(ctx, |ui| {
                self.text_mode_viewer.draw(ui, &mut self.event_queue);
            });*/
    }
}

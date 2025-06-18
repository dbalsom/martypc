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

    --------------------------------------------------------------------------
*/

//! Display the `Debug` sub-menu.
//!

use crate::{state::GuiState, GuiBoolean, GuiEvent, GuiVariable, GuiVariableContext, GuiWindow};
use marty_core::cpu_common::Register16;

impl GuiState {
    pub fn draw_debug_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Debug", |ui| {
            ui.menu_button("CPU", |ui| {
                self.workspace_window_open_button(ui, GuiWindow::CpuControl, true, true);
                self.workspace_window_open_button(ui, GuiWindow::CpuStateViewer, true, true);

                ui.menu_button("CPU Debug Options", |ui| {
                    if ui
                        .checkbox(
                            &mut self.get_option_mut(GuiBoolean::CpuEnableWaitStates),
                            "Enable Wait States",
                        )
                        .clicked()
                    {
                        let new_opt = self.get_option(GuiBoolean::CpuEnableWaitStates).unwrap();

                        self.event_queue.send(GuiEvent::VariableChanged(
                            GuiVariableContext::Global,
                            GuiVariable::Bool(GuiBoolean::CpuEnableWaitStates, new_opt),
                        ));
                    }
                    if ui
                        .checkbox(
                            &mut self.get_option_mut(GuiBoolean::CpuInstructionHistory),
                            "Instruction History",
                        )
                        .clicked()
                    {
                        let new_opt = self.get_option(GuiBoolean::CpuInstructionHistory).unwrap();

                        self.event_queue.send(GuiEvent::VariableChanged(
                            GuiVariableContext::Global,
                            GuiVariable::Bool(GuiBoolean::CpuInstructionHistory, new_opt),
                        ));
                        ui.close_menu();
                    }
                    if ui
                        .checkbox(
                            &mut self.get_option_mut(GuiBoolean::CpuTraceLoggingEnabled),
                            "Trace Logging Enabled",
                        )
                        .clicked()
                    {
                        let new_opt = self.get_option(GuiBoolean::CpuTraceLoggingEnabled).unwrap();

                        self.event_queue.send(GuiEvent::VariableChanged(
                            GuiVariableContext::Global,
                            GuiVariable::Bool(GuiBoolean::CpuTraceLoggingEnabled, new_opt),
                        ));
                        ui.close_menu();
                    }
                    #[cfg(feature = "devtools")]
                    if ui.button("Delays...").clicked() {
                        *self.window_flag(GuiWindow::DelayAdjust) = true;
                        ui.close_menu();
                    }

                    if ui.button("Trigger NMI").clicked() {
                        self.event_queue.send(GuiEvent::SetNMI(true));
                        ui.close_menu();
                    }

                    if ui.button("Clear NMI").clicked() {
                        self.event_queue.send(GuiEvent::SetNMI(false));
                        ui.close_menu();
                    }
                });

                self.workspace_window_open_button(ui, GuiWindow::InstructionHistoryViewer, true, true);
                self.workspace_window_open_button(ui, GuiWindow::CycleTraceViewer, true, true);
                self.workspace_window_open_button(ui, GuiWindow::CallStack, true, true);
                self.workspace_window_open_button(ui, GuiWindow::DisassemblyViewer, true, true);

                // Don't show disassembly listing recording options on web.
                // There's no place for the recording to go...
                #[cfg(not(target_arch = "wasm32"))]
                {
                    ui.menu_button("Disassembly Listing", |ui| {
                        if ui.button("⏺ Start Recording").clicked() {
                            self.event_queue.send(GuiEvent::StartRecordingDisassembly);
                            ui.close_menu();
                        }
                        if ui.button("⏹ Stop Recording and Save").clicked() {
                            self.event_queue.send(GuiEvent::StopRecordingDisassembly);
                            ui.close_menu();
                        }
                    });
                }
            });

            ui.menu_button("Memory", |ui| {
                self.workspace_window_open_button(ui, GuiWindow::MemoryViewer, true, true);
                self.workspace_window_open_button(ui, GuiWindow::DataVisualizer, true, true);
                self.workspace_window_open_button(ui, GuiWindow::IvtViewer, true, true);
                self.workspace_window_open_button(ui, GuiWindow::FantasyEMSStatsViewer, true, true);
                self.workspace_window_open_button(ui, GuiWindow::EMSVirtualMemoryViewer, true, true);

                ui.menu_button("Dump Memory", |ui| {
                    if ui.button("Video Memory").clicked() {
                        self.event_queue.send(GuiEvent::DumpVRAM);
                        ui.close_menu();
                    }
                    if ui.button("Code Segment (CS)").clicked() {
                        self.event_queue.send(GuiEvent::DumpSegment(Register16::CS));
                        ui.close_menu();
                    }
                    if ui.button("Data Segment (DS)").clicked() {
                        self.event_queue.send(GuiEvent::DumpSegment(Register16::DS));
                        ui.close_menu();
                    }
                    if ui.button("Extra Segment (ES)").clicked() {
                        self.event_queue.send(GuiEvent::DumpSegment(Register16::ES));
                        ui.close_menu();
                    }
                    if ui.button("Stack Segment (SS)").clicked() {
                        self.event_queue.send(GuiEvent::DumpSegment(Register16::SS));
                        ui.close_menu();
                    }
                    if ui.button("All Memory").clicked() {
                        self.event_queue.send(GuiEvent::DumpAllMem);
                        ui.close_menu();
                    }
                });
            });

            ui.menu_button("Devices", |ui| {
                #[cfg(feature = "devtools")]
                if ui.button("Device control...").clicked() {
                    *self.window_flag(GuiWindow::DeviceControl) = true;
                    ui.close_menu();
                }
                self.workspace_window_open_button(ui, GuiWindow::IoStatsViewer, true, true);
                self.workspace_window_open_button(ui, GuiWindow::PicViewer, true, true);
                self.workspace_window_open_button(ui, GuiWindow::PitViewer, true, true);
                self.workspace_window_open_button(ui, GuiWindow::PpiViewer, true, true);
                self.workspace_window_open_button(ui, GuiWindow::DmaViewer, true, true);
                self.workspace_window_open_button(ui, GuiWindow::SerialViewer, true, true);
                self.workspace_window_open_button(ui, GuiWindow::FdcViewer, true, true);
                self.workspace_window_open_button(ui, GuiWindow::VideoCardViewer, true, true);
                if self.has_sn76489() {
                    self.workspace_window_open_button(ui, GuiWindow::SnViewer, true, true);
                }
                /*
                if ui
                    .checkbox(
                        &mut self.get_option_mut(GuiBoolean::ShowBackBuffer),
                        "Debug back buffer",
                    )
                    .clicked()
                {
                    let new_opt = self.get_option(GuiBoolean::ShowBackBuffer).unwrap();

                    self.event_queue.send(GuiEvent::VariableChanged(
                        GuiVariableContext::Global,
                        GuiVariable::Bool(GuiBoolean::ShowBackBuffer, new_opt),
                    ));
                    ui.close_menu();
                }
                 */
            });

            if ui
                .checkbox(&mut self.get_option_mut(GuiBoolean::ShowBackBuffer), "Show Back Buffer")
                .clicked()
            {
                let new_opt = self.get_option(GuiBoolean::ShowBackBuffer).unwrap();

                self.event_queue.send(GuiEvent::VariableChanged(
                    GuiVariableContext::Global,
                    GuiVariable::Bool(GuiBoolean::ShowBackBuffer, new_opt),
                ));
            }

            if ui
                .checkbox(
                    &mut self.get_option_mut(GuiBoolean::ShowRasterPosition),
                    "Show Raster Position",
                )
                .clicked()
            {
                let new_opt = self.get_option(GuiBoolean::ShowRasterPosition).unwrap();

                self.event_queue.send(GuiEvent::VariableChanged(
                    GuiVariableContext::Global,
                    GuiVariable::Bool(GuiBoolean::ShowRasterPosition, new_opt),
                ));
            }

            if ui.button("Flush Trace Logs").clicked() {
                self.event_queue.send(GuiEvent::FlushLogs);
                ui.close_menu();
            }
        });
    }
}

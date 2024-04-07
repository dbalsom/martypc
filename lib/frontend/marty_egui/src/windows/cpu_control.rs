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

    -------------------------------------------------------------------------

    egui::cpu_control.rs

    Implements debug controls for the CPU including pause, step, step over,
    restart, breakpoints, etc.

*/
use crate::*;
use marty_core::{
    cpu_808x::CpuAddress,
    machine::{ExecutionControl, ExecutionOperation, ExecutionState},
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub struct CpuControl {
    exec_control: Rc<RefCell<ExecutionControl>>,
    breakpoint: String,
    mem_breakpoint: String,
    int_breakpoint: String,
    step_over_target: Option<CpuAddress>,
}

impl CpuControl {
    pub fn new(exec_control: Rc<RefCell<ExecutionControl>>) -> Self {
        Self {
            exec_control,
            breakpoint: String::new(),
            mem_breakpoint: String::new(),
            int_breakpoint: String::new(),
            step_over_target: None,
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, gui_options: &mut HashMap<GuiBoolean, bool>, events: &mut GuiEventQueue) {
        let mut exec_control = self.exec_control.borrow_mut();

        let (pause_enabled, step_enabled, run_enabled) = match exec_control.state {
            ExecutionState::Paused | ExecutionState::BreakpointHit | ExecutionState::StepOverHit => (false, true, true),
            ExecutionState::Running => (true, false, false),
            ExecutionState::Halted => (false, false, false),
        };

        ui.horizontal(|ui| {
            ui.add_enabled_ui(pause_enabled, |ui| {
                if ui
                    .button(egui::RichText::new("⏸").font(egui::FontId::proportional(20.0)))
                    .clicked()
                {
                    exec_control.set_state(ExecutionState::Paused);
                };
            });

            ui.add_enabled_ui(step_enabled, |ui| {
                if ui
                    .button(egui::RichText::new("⤵").font(egui::FontId::proportional(20.0)))
                    .clicked()
                {
                    exec_control.set_op(ExecutionOperation::StepOver);
                };

                /*                if ui.input(|i| i.key_pressed(egui::Key::F10)) {
                    exec_control.set_op(ExecutionOperation::StepOver);
                };*/
            });

            ui.add_enabled_ui(step_enabled, |ui| {
                if ui
                    .button(egui::RichText::new("➡").font(egui::FontId::proportional(20.0)))
                    .clicked()
                {
                    exec_control.set_op(ExecutionOperation::Step);
                };

                /*                if ui.input(|i| i.key_pressed(egui::Key::F11)) {
                    exec_control.set_op(ExecutionOperation::Step);
                }*/
            });

            ui.add_enabled_ui(run_enabled, |ui| {
                if ui
                    .button(egui::RichText::new("▶").font(egui::FontId::proportional(20.0)))
                    .clicked()
                {
                    exec_control.set_op(ExecutionOperation::Run);
                };

                /*                if ui.input(|i| i.key_pressed(egui::Key::F5)) {
                    exec_control.set_op(ExecutionOperation::Run);
                }*/
            });

            if ui
                .button(egui::RichText::new("⟲").font(egui::FontId::proportional(20.0)))
                .clicked()
            {
                exec_control.set_op(ExecutionOperation::Reset);
            };

            ui.menu_button(egui::RichText::new("⏷").font(egui::FontId::proportional(20.0)), |ui| {
                if ui
                    .checkbox(
                        &mut gui_options.get_mut(&GuiBoolean::CpuEnableWaitStates).unwrap(),
                        "Enable Wait States",
                    )
                    .clicked()
                {
                    let new_opt = gui_options.get(&GuiBoolean::CpuEnableWaitStates).unwrap();

                    events.send(GuiEvent::VariableChanged(
                        GuiVariableContext::Global,
                        GuiVariable::Bool(GuiBoolean::CpuEnableWaitStates, *new_opt),
                    ));
                    ui.close_menu();
                }
                if ui
                    .checkbox(
                        &mut gui_options.get_mut(&GuiBoolean::CpuInstructionHistory).unwrap(),
                        "Instruction History",
                    )
                    .clicked()
                {
                    let new_opt = gui_options.get(&GuiBoolean::CpuInstructionHistory).unwrap();

                    events.send(GuiEvent::VariableChanged(
                        GuiVariableContext::Global,
                        GuiVariable::Bool(GuiBoolean::CpuInstructionHistory, *new_opt),
                    ));
                    ui.close_menu();
                }
                if ui
                    .checkbox(
                        &mut gui_options.get_mut(&GuiBoolean::CpuTraceLoggingEnabled).unwrap(),
                        "Trace Logging Enabled",
                    )
                    .clicked()
                {
                    let new_opt = gui_options.get(&GuiBoolean::CpuTraceLoggingEnabled).unwrap();

                    events.send(GuiEvent::VariableChanged(
                        GuiVariableContext::Global,
                        GuiVariable::Bool(GuiBoolean::CpuTraceLoggingEnabled, *new_opt),
                    ));
                    ui.close_menu();
                }
            });
        });

        let state_str = format!("{:?}", exec_control.get_state());
        ui.separator();

        egui::Grid::new("cpu_control_grid")
            .num_columns(2)
            .striped(false)
            .min_col_width(60.0)
            .show(ui, |ui| {
                ui.label("Run state: ");
                ui.label(&state_str);
                ui.end_row();

                ui.label("StepOver Target: ");
                if let Some(target) = self.step_over_target {
                    ui.label(&format!("{:?}", self.step_over_target));
                }
                else {
                    ui.label("None");
                }
                ui.end_row();

                ui.label("Exec Breakpoint: ");
                if ui.text_edit_singleline(&mut self.breakpoint).changed() {
                    events.send(GuiEvent::EditBreakpoint);
                };
                ui.end_row();

                ui.label("Mem Breakpoint: ");
                if ui.text_edit_singleline(&mut self.mem_breakpoint).changed() {
                    events.send(GuiEvent::EditBreakpoint);
                }
                ui.end_row();

                ui.label("Int Breakpoint: ");
                if ui.text_edit_singleline(&mut self.int_breakpoint).changed() {
                    events.send(GuiEvent::EditBreakpoint);
                }
                ui.end_row();
            });
    }

    pub fn set_step_over_target(&mut self, target: Option<CpuAddress>) {
        self.step_over_target = target;
    }

    pub fn get_breakpoints(&mut self) -> (&str, &str, &str) {
        (&self.breakpoint, &self.mem_breakpoint, &self.int_breakpoint)
    }
}

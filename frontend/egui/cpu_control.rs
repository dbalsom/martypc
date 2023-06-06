/*
    MartyPC Emulator 
    (C)2023 Daniel Balsom
    https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

    -------------------------------------------------------------------------

    egui::cpu_control.rs

    Implements debug controls for the CPU including pause, step, step over,
    restart, breakpoints, etc.

*/
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    rc::Rc,
};
use crate::egui::*;

use marty_core::machine::{ExecutionControl, ExecutionState, ExecutionOperation};
pub struct CpuControl {

    exec_control: Rc<RefCell<ExecutionControl>>,
    breakpoint: String,
    mem_breakpoint: String,
    int_breakpoint: String,
}

impl CpuControl {
    
    pub fn new(exec_control: Rc<RefCell<ExecutionControl>>) -> Self {
        Self {
            exec_control,
            breakpoint: String::new(),
            mem_breakpoint: String::new(),
            int_breakpoint: String::new(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, gui_options: &mut HashMap::<GuiOption, bool>, events: &mut VecDeque<GuiEvent> ) {

        let mut exec_control = self.exec_control.borrow_mut();

        let (pause_enabled, step_enabled, run_enabled) = match exec_control.state {
            ExecutionState::Paused | ExecutionState::BreakpointHit => (false, true, true),
            ExecutionState::Running => (true, false, false),
            ExecutionState::Halted => (false, false, false),
        };

        ui.horizontal(|ui|{

            ui.add_enabled_ui(pause_enabled, |ui| {
                if ui.button(egui::RichText::new("⏸").font(egui::FontId::proportional(20.0))).clicked() {
                    exec_control.set_state(ExecutionState::Paused);
                };
            });

            ui.add_enabled_ui(step_enabled, |ui| {
                if ui.button(egui::RichText::new("⤵").font(egui::FontId::proportional(20.0))).clicked() {
                   exec_control.set_op(ExecutionOperation::StepOver);
                };

                if ui.input(|i| i.key_pressed(egui::Key::F10)) {
                    exec_control.set_op(ExecutionOperation::StepOver);
                };
            });   

            ui.add_enabled_ui(step_enabled, |ui| {
                if ui.button(egui::RichText::new("➡").font(egui::FontId::proportional(20.0))).clicked() {
                   exec_control.set_op(ExecutionOperation::Step);
                };

                if ui.input(|i| i.key_pressed(egui::Key::F11)) {
                    exec_control.set_op(ExecutionOperation::Step);
                }                             
            });                 

            ui.add_enabled_ui(run_enabled, |ui| {
                if ui.button(egui::RichText::new("▶").font(egui::FontId::proportional(20.0))).clicked() {
                    exec_control.set_op(ExecutionOperation::Run);
                };

                if ui.input(|i| i.key_pressed(egui::Key::F5)) {
                    exec_control.set_op(ExecutionOperation::Run);
                }                        
            });

            if ui.button(egui::RichText::new("⟲").font(egui::FontId::proportional(20.0))).clicked() {
                exec_control.set_op(ExecutionOperation::Reset);
            };

            ui.menu_button(egui::RichText::new("⏷").font(egui::FontId::proportional(20.0)), |ui| {
                if ui.checkbox(&mut gui_options.get_mut(&GuiOption::CpuEnableWaitStates).unwrap(), "Enable Wait States").clicked() {

                    let new_opt = gui_options.get(&GuiOption::CpuEnableWaitStates).unwrap();

                    events.push_back(
                        GuiEvent::OptionChanged(
                            GuiOption::CpuEnableWaitStates, 
                            *new_opt 
                        )
                    );
                    ui.close_menu();
                } 
                if ui.checkbox(&mut gui_options.get_mut(&GuiOption::CpuInstructionHistory).unwrap(), "Instruction History").clicked() {

                    let new_opt = gui_options.get(&GuiOption::CpuInstructionHistory).unwrap();

                    events.push_back(
                        GuiEvent::OptionChanged(
                            GuiOption::CpuInstructionHistory, 
                            *new_opt 
                        )
                    );
                    ui.close_menu();
                }   
                if ui.checkbox(&mut gui_options.get_mut(&GuiOption::CpuTraceLoggingEnabled).unwrap(), "Trace Logging Enabled").clicked() {

                    let new_opt = gui_options.get(&GuiOption::CpuTraceLoggingEnabled).unwrap();

                    events.push_back(
                        GuiEvent::OptionChanged(
                            GuiOption::CpuTraceLoggingEnabled, 
                            *new_opt 
                        )
                    );
                    ui.close_menu();
                }                                        
            });
        });

        let state_str = format!("{:?}", exec_control.get_state());
        ui.separator();
        ui.horizontal(|ui|{
            ui.label("Run state: ");
            ui.label(&state_str);
        });
        ui.separator();
        ui.horizontal(|ui|{
            ui.label("Exec Breakpoint: ");
            if ui.text_edit_singleline(&mut self.breakpoint).changed() {
                events.push_back(GuiEvent::EditBreakpoint);
            };
        });
        ui.separator();
        ui.horizontal(|ui|{
            ui.label("Mem Breakpoint: ");
            if ui.text_edit_singleline(&mut self.mem_breakpoint).changed() {
                events.push_back(GuiEvent::EditBreakpoint);
            }
        });
        ui.separator();
        ui.horizontal(|ui|{
            ui.label("Int Breakpoint: ");
            if ui.text_edit_singleline(&mut self.int_breakpoint).changed() {
                events.push_back(GuiEvent::EditBreakpoint);
            }
        });                
    }

    pub fn get_breakpoints(&mut self) -> (&str, &str, &str) {
        (&self.breakpoint, &self.mem_breakpoint, &self.int_breakpoint)
    }


}
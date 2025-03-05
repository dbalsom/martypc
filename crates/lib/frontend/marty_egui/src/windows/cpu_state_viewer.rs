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

    -------------------------------------------------------------------------

    egui::cpu_state_viewer.rs

    Implements a viewer control to display CPU state, including registers,
    flags and cycle information.

*/
use crate::layouts::MartyLayout;
#[allow(dead_code)]
use crate::*;
use egui::TextBuffer;
use marty_core::{
    cpu_common::CpuStringState,
    machine::{ExecutionControl, ExecutionState},
};
use std::{cell::RefCell, rc::Rc};

pub struct CpuViewerControl {
    exec_control: Rc<RefCell<ExecutionControl>>,
    cpu_state: CpuStringState,
    reg_updated: bool,
    flag_updated: bool,
    paused_updates: u32,
}

impl CpuViewerControl {
    pub fn new(exec_control: Rc<RefCell<ExecutionControl>>) -> Self {
        Self {
            exec_control,
            cpu_state: Default::default(),
            reg_updated: false,
            flag_updated: false,
            paused_updates: 0,
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, events: &mut GuiEventQueue) {
        let state = self.exec_control.borrow_mut().get_state();
        let state_str = format!("{:?}", state);

        egui::Grid::new("cpu_state_status_grid")
            .num_columns(2)
            .striped(false)
            .min_col_width(60.0)
            .show(ui, |ui| {
                ui.label("Run state: ");
                ui.label(&state_str);
                ui.end_row();
            });

        let mut paused = false;
        match state {
            ExecutionState::Running => {
                // Show read-only registers.
                self.show_immutable_regs(ui);
            }
            _ => {
                paused = true;
                // Execution state is stopped, show editable registers.
                self.show_mutable_regs(ui, events);
            }
        }

        ui.separator();

        MartyLayout::new(layouts::Layout::KeyValue, "cpu-state-grid").show(ui, |ui| {
            MartyLayout::kv_row(ui, "PIQ", None, |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.cpu_state.piq.as_str()).font(egui::TextStyle::Monospace),
                    );
                    // Show the button to flush the queue if we're paused
                    if paused {
                        if ui
                            .add(egui::Button::new("Flush"))
                            .on_hover_text("Flush the CPU's instruction queue")
                            .clicked()
                        {
                            events.send(GuiEvent::CpuFlushQueue);
                            self.paused_updates = 0;
                        }
                    }
                });
            });
            MartyLayout::kv_row(ui, "Instruction #", None, |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.cpu_state.instruction_count.as_str())
                            .font(egui::TextStyle::Monospace),
                    );
                });
            });
            MartyLayout::kv_row(ui, "Cycle #", None, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.cpu_state.cycle_count.as_str())
                        .font(egui::TextStyle::Monospace),
                );
            });
        });

        ui.separator();

        egui::CollapsingHeader::new("DMA Scheduler")
            .default_open(false)
            .show(ui, |ui| {
                MartyLayout::new(layouts::Layout::KeyValue, "cpu-state-scheduler-grid").show(ui, |ui| {
                    MartyLayout::kv_row(ui, "DMA State", None, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.cpu_state.dma_state.as_str())
                                .font(egui::TextStyle::Monospace),
                        );
                    });
                    MartyLayout::kv_row(ui, "DRAM Refresh period", None, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.cpu_state.dram_refresh_cycle_period.as_str())
                                .font(egui::TextStyle::Monospace),
                        );
                    });
                    MartyLayout::kv_row(ui, "DRAM Refresh cycle number", None, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.cpu_state.dram_refresh_cycle_num.as_str())
                                .font(egui::TextStyle::Monospace),
                        );
                    });
                });
            });
    }

    fn show_reg_mut(
        ui: &mut egui::Ui,
        label: &str,
        value: &mut dyn TextBuffer,
        reg: Register16,
        updated: &mut bool,
        events: &mut GuiEventQueue,
    ) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(label).text_style(egui::TextStyle::Monospace));
            let response = ui.add(
                egui::TextEdit::singleline(value)
                    .char_limit(4)
                    .font(egui::TextStyle::Monospace),
            );

            if response.lost_focus() {
                // TextEdit loses focus on enter or tab. In any case, we'll apply the value if it is valid.
                match u16::from_str_radix(value.as_str(), 16) {
                    Ok(val) => {
                        log::debug!("Register {:?} updated to 0x{:04X}", reg, val);
                        events.send(GuiEvent::Register16Update(reg, val));
                    }
                    Err(_) => {
                        // Invalid value - could change text color to red?
                    }
                }
                *updated = true;
            }
        });
    }

    #[rustfmt::skip]
    fn show_mutable_regs(&mut self, ui: &mut egui::Ui, events: &mut GuiEventQueue) {
        self.flag_updated = false;
        egui::Grid::new("reg_general_grid")
            .striped(true)
            .min_col_width(100.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("AH:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ah.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("AL:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.al.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.horizontal(|ui| {
                    Self::show_reg_mut(ui, "AX:", &mut self.cpu_state.ax, Register16::AX, &mut self.reg_updated, events);
                });
                ui.end_row();

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("BH:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.bh.as_str()).font(egui::TextStyle::Monospace),);
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("BL:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.bl.as_str()).font(egui::TextStyle::Monospace),);
                });
                ui.horizontal(|ui| {
                    Self::show_reg_mut(ui, "BX:", &mut self.cpu_state.bx, Register16::BX, &mut self.reg_updated, events);
                });
                ui.end_row();

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("CH:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ch.as_str()).font(egui::TextStyle::Monospace),);
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("CL:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.cl.as_str()).font(egui::TextStyle::Monospace),);
                });
                ui.horizontal(|ui| {
                    Self::show_reg_mut(ui, "CX:", &mut self.cpu_state.cx, Register16::CX, &mut self.reg_updated, events);
                });
                ui.end_row();

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("DH:").text_style(egui::TextStyle::Monospace));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.cpu_state.dh.as_str()).font(egui::TextStyle::Monospace),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("DL:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.dl.as_str()).font(egui::TextStyle::Monospace),);
                });
                ui.horizontal(|ui| {
                    Self::show_reg_mut(ui, "DX:", &mut self.cpu_state.dx, Register16::DX, &mut self.reg_updated, events);
                });
                ui.end_row();
            });

        ui.separator();

        egui::Grid::new("reg_segment")
            .striped(true)
            .min_col_width(100.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    Self::show_reg_mut(ui, "SP", &mut self.cpu_state.sp, Register16::SP, &mut self.reg_updated, events);
                });
                ui.horizontal(|ui| {
                    Self::show_reg_mut(ui, "ES", &mut self.cpu_state.es, Register16::ES, &mut self.reg_updated, events);
                });
                ui.horizontal(|ui| {
                    Self::show_reg_mut(ui, "PC", &mut self.cpu_state.pc, Register16::PC, &mut self.reg_updated, events);
                });
                ui.end_row();

                ui.horizontal(|ui| {
                    Self::show_reg_mut(ui, "BP", &mut self.cpu_state.bp, Register16::BP, &mut self.reg_updated, events);
                });
                ui.horizontal(|ui| {
                    Self::show_reg_mut(ui, "CS", &mut self.cpu_state.cs, Register16::CS, &mut self.reg_updated, events);
                });
                ui.horizontal(|ui| {
                    // IP is not a real register - don't allow editing (?)
                    ui.label(egui::RichText::new("IP:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ip.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.end_row();

                ui.horizontal(|ui| {
                    Self::show_reg_mut(ui, "SI", &mut self.cpu_state.si, Register16::SI, &mut self.reg_updated, events);
                });
                ui.horizontal(|ui| {
                    Self::show_reg_mut(ui, "SS", &mut self.cpu_state.ss, Register16::SS, &mut self.reg_updated, events);
                });
                ui.end_row();

                ui.horizontal(|ui| {
                    Self::show_reg_mut(ui, "DI", &mut self.cpu_state.di, Register16::DI, &mut self.reg_updated, events);
                });
                ui.horizontal(|ui| {
                    Self::show_reg_mut(ui, "DS", &mut self.cpu_state.ds, Register16::DS, &mut self.reg_updated, events);
                });
                ui.end_row();
            });

        ui.separator();

        egui::Grid::new("reg_flags")
            .striped(true)
            .max_col_width(10.0)
            .show(ui, |ui| {
                Self::show_flagbit_mut(ui, &mut self.cpu_state.o_fl, &mut self.flag_updated,  "O", "Overflow");
                Self::show_flagbit_mut(ui, &mut self.cpu_state.d_fl, &mut self.flag_updated, "D", "Direction");
                Self::show_flagbit_mut(ui, &mut self.cpu_state.i_fl, &mut self.flag_updated, "I", "Interrupt enable");
                Self::show_flagbit_mut(ui, &mut self.cpu_state.t_fl, &mut self.flag_updated, "T", "Trap");
                Self::show_flagbit_mut(ui, &mut self.cpu_state.s_fl, &mut self.flag_updated, "S", "Sign");
                Self::show_flagbit_mut(ui, &mut self.cpu_state.z_fl, &mut self.flag_updated, "Z", "Zero");
                Self::show_flagbit_mut(ui, &mut self.cpu_state.a_fl, &mut self.flag_updated, "A", "Auxiliary carry");
                Self::show_flagbit_mut(ui, &mut self.cpu_state.p_fl, &mut self.flag_updated, "P", "Parity");
                Self::show_flagbit_mut(ui, &mut self.cpu_state.c_fl, &mut self.flag_updated, "C", "Carry");
                ui.end_row();
            });

        if self.reg_updated {
            log::trace!("Clearing paused update count on reg update...");
            self.paused_updates = 0;
        }
        if self.flag_updated {
            events.send(GuiEvent::CpuFlagsUpdate(self.calc_flag_value()));
        }
    }

    #[rustfmt::skip]
    fn show_immutable_regs(&mut self, ui: &mut egui::Ui)  {
        egui::Grid::new("reg_general_grid")
            .striped(true)
            .min_col_width(100.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("AH:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ah.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("AL:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.al.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("AX:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ax.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.end_row();

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("BH:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.bh.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("BL:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.bl.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("BX:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.bx.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.end_row();

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("CH:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ch.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("CL:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.cl.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("CX:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.cx.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.end_row();

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("DH:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.dh.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("DL:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.dl.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("DX:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.dx.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.end_row();
            });

        ui.separator();

        egui::Grid::new("reg_segment_grid")
            .striped(true)
            .min_col_width(100.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    //ui.add(egui::Label::new("SP:"));
                    ui.label(egui::RichText::new("SP:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.sp.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("ES:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.es.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("PC:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.pc.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.end_row();

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("BP:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.bp.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("CS:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.cs.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("IP:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ip.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.end_row();

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("SI:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.si.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("SS:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ss.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.end_row();

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("DI:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.di.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("DS:").text_style(egui::TextStyle::Monospace));
                    ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ds.as_str()).font(egui::TextStyle::Monospace));
                });
                ui.end_row();
            });

        ui.separator();

        egui::Grid::new("reg_flags_grid")
            .striped(true)
            .max_col_width(10.0)
            .show(ui, |ui| {
                Self::show_flagbit(ui, &mut self.cpu_state.o_fl.as_str(),"O", "Overflow");
                Self::show_flagbit(ui, &mut self.cpu_state.d_fl.as_str(), "D", "Direction");
                Self::show_flagbit(ui, &mut self.cpu_state.i_fl.as_str(), "I", "Interrupt enable");
                Self::show_flagbit(ui, &mut self.cpu_state.t_fl.as_str(), "T", "Trap");
                Self::show_flagbit(ui, &mut self.cpu_state.s_fl.as_str(), "S", "Sign");
                Self::show_flagbit(ui, &mut self.cpu_state.z_fl.as_str(), "Z", "Zero");
                Self::show_flagbit(ui, &mut self.cpu_state.a_fl.as_str(), "A", "Auxiliary carry");
                Self::show_flagbit(ui, &mut self.cpu_state.p_fl.as_str(), "P", "Parity");
                Self::show_flagbit(ui, &mut self.cpu_state.c_fl.as_str(), "C", "Carry");

                ui.end_row();
            });

    }

    /// Display a widget for a flag bit. It will show the provided tooltip text on hover.
    fn show_flagbit(ui: &mut egui::Ui, text: &mut dyn TextBuffer, label: &str, tip: &str) {
        ui.vertical(|ui| {
            ui.add(
                egui::TextEdit::singleline(text)
                    .char_limit(1)
                    .horizontal_align(egui::Align::Center)
                    .font(egui::TextStyle::Monospace),
            );
            ui.centered_and_justified(|ui| {
                if ui
                    .add(
                        egui::Label::new(egui::RichText::new(label).text_style(egui::TextStyle::Monospace))
                            .selectable(false),
                    )
                    .hovered()
                {
                    egui::containers::popup::show_tooltip(
                        ui.ctx(),
                        ui.layer_id(),
                        egui::Id::new("flag_tooltip"),
                        |ui| {
                            ui.horizontal(|ui| {
                                ui.label(tip);
                            });
                        },
                    );
                }
            });
        });
    }

    /// Display a widget for an editable flag bit. It will show the provided tooltip text on hover.
    fn show_flagbit_mut(ui: &mut egui::Ui, text: &mut dyn TextBuffer, updated: &mut bool, label: &str, tip: &str) {
        ui.vertical(|ui| {
            let edit_response = ui.add(
                egui::TextEdit::singleline(text)
                    .char_limit(1)
                    .horizontal_align(egui::Align::Center)
                    .char_limit(1)
                    .font(egui::TextStyle::Monospace),
            );

            if edit_response.lost_focus() {
                // TextEdit loses focus on enter or tab. In any case, we'll apply the value if it is valid.
                match u16::from_str_radix(text.as_str(), 16) {
                    Ok(val) if val == 0 || val == 1 => {
                        log::debug!("Flag {} updated to {}", label, val);
                        //events.send(GuiEvent::Register16Update(reg, val));
                    }
                    _ => {
                        // Invalid value - could change text color to red?
                    }
                }
                *updated = true;
            }

            ui.centered_and_justified(|ui| {
                if ui
                    .add(
                        egui::Label::new(egui::RichText::new(label).text_style(egui::TextStyle::Monospace))
                            .selectable(false),
                    )
                    .hovered()
                {
                    egui::containers::popup::show_tooltip(
                        ui.ctx(),
                        ui.layer_id(),
                        egui::Id::new("flag_tooltip"),
                        |ui| {
                            ui.horizontal(|ui| {
                                ui.label(tip);
                            });
                        },
                    );
                }
            });
        });
    }

    /// Calculate the flag value from the current string state.
    /// Note we don't have to account for reserved fields, as the cpu's set_flags method will
    /// enforce the correct bit values.
    #[rustfmt::skip]
    fn calc_flag_value(&self) -> u16 {
        let mut flags = 0u16;
        //const CPU_FLAG_CARRY: u16      = 0b0000_0000_0001;
        //const CPU_FLAG_RESERVED1: u16  = 0b0000_0000_0010;
        //const CPU_FLAG_PARITY: u16     = 0b0000_0000_0100;
        //const CPU_FLAG_AUX_CARRY: u16  = 0b0000_0001_0000;
        //const CPU_FLAG_ZERO: u16       = 0b0000_0100_0000;
        //const CPU_FLAG_SIGN: u16       = 0b0000_1000_0000;
        //const CPU_FLAG_TRAP: u16       = 0b0001_0000_0000;
        //const CPU_FLAG_INT_ENABLE: u16 = 0b0010_0000_0000;
        //const CPU_FLAG_DIRECTION: u16  = 0b0100_0000_0000;
        //const CPU_FLAG_OVERFLOW: u16   = 0b1000_0000_0000;
        flags |= if self.cpu_state.c_fl.as_str() == "1" { 0b0000_0000_0001 } else { 0 };
        flags |= if self.cpu_state.p_fl.as_str() == "1" { 0b0000_0000_0100 } else { 0 };
        flags |= if self.cpu_state.a_fl.as_str() == "1" { 0b0000_0001_0000 } else { 0 };
        flags |= if self.cpu_state.z_fl.as_str() == "1" { 0b0000_0100_0000 } else { 0 };
        flags |= if self.cpu_state.s_fl.as_str() == "1" { 0b0000_1000_0000 } else { 0 };
        flags |= if self.cpu_state.t_fl.as_str() == "1" { 0b0001_0000_0000 } else { 0 };
        flags |= if self.cpu_state.i_fl.as_str() == "1" { 0b0010_0000_0000 } else { 0 };
        flags |= if self.cpu_state.d_fl.as_str() == "1" { 0b0100_0000_0000 } else { 0 };
        flags |= if self.cpu_state.o_fl.as_str() == "1" { 0b1000_0000_0000 } else { 0 };
        flags
    }

    pub fn update_state(&mut self, cpu_state: CpuStringState) {
        let exec_state = self.exec_control.borrow_mut().get_state();
        match exec_state {
            ExecutionState::Running => {
                // Accept updates anytime, when running.
                self.cpu_state = cpu_state;
                self.paused_updates = 0;
            }
            _ => {
                // Accept one update after paused state, then stop updating.
                // This is mainly so we can update the state of dependent registers
                // (editing AX will update AH and AL, PC will update IP).
                if self.paused_updates < 1 {
                    log::trace!("Honoring update while paused!");
                    self.reg_updated = false;
                    self.cpu_state = cpu_state;
                    self.paused_updates += 1;
                }
            }
        }
    }
}

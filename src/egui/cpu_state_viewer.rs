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

    egui::cpu_state_viewer.rs

    Implements a viewer control to display CPU state, including registers,
    flags and cycle information.

*/
#[allow (dead_code)]

use egui::*;

use crate::cpu_808x::CpuStringState;
use crate::syntax_token::*;

use crate::egui::*;
use crate::egui::color::*;
use crate::egui::constants::*;

pub struct CpuViewerControl {

  cpu_state: CpuStringState

}


impl CpuViewerControl {
    
  pub fn new() -> Self {
      Self {
          cpu_state: Default::default(),
      }
  }

  pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut VecDeque<GuiEvent> ) {
      
    egui::Grid::new("reg_general")
      .striped(true)
      .min_col_width(100.0)
      .show(ui, |ui| {
        
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("AH:").text_style(egui::TextStyle::Monospace));
            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ah).font(egui::TextStyle::Monospace));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("AL:").text_style(egui::TextStyle::Monospace));
            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.al).font(egui::TextStyle::Monospace));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("AX:").text_style(egui::TextStyle::Monospace));
            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ax).font(egui::TextStyle::Monospace));
        });
        ui.end_row();
      
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("BH:").text_style(egui::TextStyle::Monospace));
            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.bh).font(egui::TextStyle::Monospace));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("BL:").text_style(egui::TextStyle::Monospace));
            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.bl).font(egui::TextStyle::Monospace));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("BX:").text_style(egui::TextStyle::Monospace));
            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.bx).font(egui::TextStyle::Monospace));
        });
        ui.end_row();
      
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("CH:").text_style(egui::TextStyle::Monospace));
            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ch).font(egui::TextStyle::Monospace));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("CL:").text_style(egui::TextStyle::Monospace));
            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.cl).font(egui::TextStyle::Monospace));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("CX:").text_style(egui::TextStyle::Monospace));
            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.cx).font(egui::TextStyle::Monospace));
        });
        ui.end_row();
      
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("DH:").text_style(egui::TextStyle::Monospace));
            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.dh).font(egui::TextStyle::Monospace));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("DL:").text_style(egui::TextStyle::Monospace));
            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.dl).font(egui::TextStyle::Monospace));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("DX:").text_style(egui::TextStyle::Monospace));
            ui.add(egui::TextEdit::singleline(&mut self.cpu_state.dx).font(egui::TextStyle::Monospace));
        });
        ui.end_row();         
    });
    
    ui.separator();
    
    egui::Grid::new("reg_segment")
        .striped(true)
        .min_col_width(100.0)
        .show(ui, |ui| {
        
            ui.horizontal( |ui| {
                //ui.add(egui::Label::new("SP:"));
                ui.label(egui::RichText::new("SP:").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.cpu_state.sp).font(egui::TextStyle::Monospace));
            });
            ui.horizontal( |ui| {
                ui.label(egui::RichText::new("ES:").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.cpu_state.es).font(egui::TextStyle::Monospace));
            });                        
            ui.end_row();  
            ui.horizontal( |ui| {
                ui.label(egui::RichText::new("BP:").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.cpu_state.bp).font(egui::TextStyle::Monospace));
            });
            ui.horizontal( |ui| {
                ui.label(egui::RichText::new("CS:").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.cpu_state.cs).font(egui::TextStyle::Monospace));
            });                         
            ui.end_row();  
            ui.horizontal( |ui| {
                ui.label(egui::RichText::new("SI:").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.cpu_state.si).font(egui::TextStyle::Monospace));
            });
            ui.horizontal( |ui| {
                ui.label(egui::RichText::new("SS:").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ss).font(egui::TextStyle::Monospace));
            });                         
            ui.end_row();  
            ui.horizontal( |ui| {
                ui.label(egui::RichText::new("DI:").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.cpu_state.di).font(egui::TextStyle::Monospace));
            });
            ui.horizontal( |ui| {
                ui.label(egui::RichText::new("DS:").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ds).font(egui::TextStyle::Monospace));
            });                         
            ui.end_row();  
            ui.label("");
            ui.horizontal( |ui| {
                ui.label(egui::RichText::new("IP:").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.cpu_state.ip).font(egui::TextStyle::Monospace));
                //ui.text_edit_singleline(&mut self.memory_viewer_address);
            }); 
            ui.end_row();  
        });
      
    ui.separator();
      
    egui::Grid::new("reg_flags")
        .striped(true)
        .max_col_width(15.0)
        .show(ui, |ui| {
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
        
            ui.horizontal( |ui| {
                //ui.add(egui::Label::new("SP:"));
                ui.label(egui::RichText::new("O:").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.cpu_state.o_fl).font(egui::TextStyle::Monospace));
                ui.label(egui::RichText::new("D:").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.cpu_state.d_fl).font(egui::TextStyle::Monospace)); 
                ui.label(egui::RichText::new("I:").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.cpu_state.i_fl).font(egui::TextStyle::Monospace));  
                ui.label(egui::RichText::new("T:").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.cpu_state.t_fl).font(egui::TextStyle::Monospace));
                ui.label(egui::RichText::new("S:").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.cpu_state.s_fl).font(egui::TextStyle::Monospace));
                ui.label(egui::RichText::new("Z:").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.cpu_state.z_fl).font(egui::TextStyle::Monospace));      
                ui.label(egui::RichText::new("A:").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.cpu_state.a_fl).font(egui::TextStyle::Monospace));  
                ui.label(egui::RichText::new("P:").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.cpu_state.p_fl).font(egui::TextStyle::Monospace));             
                ui.label(egui::RichText::new("C:").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.cpu_state.c_fl).font(egui::TextStyle::Monospace));                                        
            });
          
            ui.end_row();  
        });
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Instruction #:").text_style(egui::TextStyle::Monospace));
        ui.add(egui::TextEdit::singleline(&mut self.cpu_state.instruction_count).font(egui::TextStyle::Monospace));
    }); 
    ui.horizontal(|ui| {
      ui.label(egui::RichText::new("Cycle #:").text_style(egui::TextStyle::Monospace));
      ui.add(egui::TextEdit::singleline(&mut self.cpu_state.cycle_count).font(egui::TextStyle::Monospace));
  });     
  }
    
  pub fn update_state(&mut self, state: CpuStringState) {
    self.cpu_state = state;
  }
    
}
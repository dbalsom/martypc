
/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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
    
    egui::pic_viewer.rs

    Implements a viewer control for the Programmable Interrupt Controller.
    
    This viewer displays data regarding the Programmable Interrupt 
    Controller's registers as well as statistics regarding the various
    interrupt levels.

*/

use crate::egui::*;

pub struct PicViewerControl {

    state: PicStringState,
}

impl PicViewerControl {

    pub fn new() -> Self {
        Self {
            state: Default::default(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue ) {

        egui::Grid::new("pic_view")
        .striped(true)
        .min_col_width(100.0)
        .show(ui, |ui| {

            //ui.horizontal(|ui| {
                ui.label(egui::RichText::new("IMR Register: ").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.imr).font(egui::TextStyle::Monospace));
            //});
            ui.end_row();
            //ui.horizontal(|ui| {
                ui.label(egui::RichText::new("ISR Register: ").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.isr).font(egui::TextStyle::Monospace));
            //});
            ui.end_row();   
            //ui.horizontal(|ui| {
                ui.label(egui::RichText::new("IRR Register: ").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.irr).font(egui::TextStyle::Monospace));
            //});         
            ui.end_row();
            //ui.horizontal(|ui| {
                ui.label(egui::RichText::new("IR Lines: ").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.ir).font(egui::TextStyle::Monospace));
            //});         
            ui.end_row();            
            //ui.horizontal(|ui| {
                ui.label(egui::RichText::new("INTR Status: ").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.intr).font(egui::TextStyle::Monospace));
            //});
            ui.end_row();                    
            //ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Auto-EOI: ").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.autoeoi).font(egui::TextStyle::Monospace));
            //});
            ui.end_row();
            //ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Trigger Mode: ").text_style(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.trigger_mode).font(egui::TextStyle::Monospace));
            //});
            ui.end_row();                    

            // Add table header
            ui.label(egui::RichText::new("").text_style(egui::TextStyle::Monospace));
            ui.label(egui::RichText::new("IMR Masked").text_style(egui::TextStyle::Monospace));
            ui.label(egui::RichText::new("ISR Masked").text_style(egui::TextStyle::Monospace));
            ui.label(egui::RichText::new("Serviced").text_style(egui::TextStyle::Monospace));
            ui.end_row();

            // Draw table
            for i in 0..self.state.interrupt_stats.len() {
                let label_str = format!("IRQ {}", i );
                ui.label(egui::RichText::new(label_str).text_style(egui::TextStyle::Monospace));

                ui.add(egui::TextEdit::singleline(&mut self.state.interrupt_stats[i].0).font(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.interrupt_stats[i].1).font(egui::TextStyle::Monospace));
                ui.add(egui::TextEdit::singleline(&mut self.state.interrupt_stats[i].2).font(egui::TextStyle::Monospace));
                ui.end_row();                                           
            }
          
        });
    }

    pub fn update_state(&mut self, state: &PicStringState ) {
        self.state = state.clone();
    }
}
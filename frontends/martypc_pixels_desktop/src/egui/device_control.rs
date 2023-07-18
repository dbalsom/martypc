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

    -------------------------------------------------------------------------

    egui::device_control.rs

    Implements debug controls for system devices, allowing them to be 
    ticked independently of the rest of the system.

*/

use crate::egui::*;

pub struct DeviceControl {
    _params: bool
}

impl DeviceControl {
    
    pub fn new() -> Self {
        Self {
            _params: false
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, events: &mut VecDeque<GuiEvent> ) {
        ui.horizontal(|ui|{
            ui.vertical(|ui|{
                ui.label("Tick:");
                ui.group(|ui| {

                        if ui.button(egui::RichText::new("Timer 0").font(egui::FontId::proportional(20.0))).clicked() {
                            events.push_back(GuiEvent::TickDevice(DeviceSelection::Timer(0), 1))
                        };
                        if ui.button(egui::RichText::new("Video + 8").font(egui::FontId::proportional(20.0))).clicked() {
                            events.push_back(GuiEvent::TickDevice(DeviceSelection::VideoCard, 8))
                        };                    
                });
            });

            ui.vertical(|ui|{
                ui.label("Tick 100:");
                ui.group(|ui| {

                        if ui.button(egui::RichText::new("Timer 0").font(egui::FontId::proportional(20.0))).clicked() {
                            events.push_back(GuiEvent::TickDevice(DeviceSelection::Timer(0), 100))
                        };
                        if ui.button(egui::RichText::new("Video +104").font(egui::FontId::proportional(20.0))).clicked() {
                            events.push_back(GuiEvent::TickDevice(DeviceSelection::VideoCard, 104))
                        };                    
                });
            });

            ui.vertical(|ui|{
                ui.label("Special:");
                ui.group(|ui| {

                        if ui.button(egui::RichText::new("Tick CGA -1").font(egui::FontId::proportional(20.0))).clicked() {
                            events.push_back(GuiEvent::TickDevice(DeviceSelection::VideoCard, 238943))
                        };           
                        if ui.button(egui::RichText::new("Tick CGA -100").font(egui::FontId::proportional(20.0))).clicked() {
                            events.push_back(GuiEvent::TickDevice(DeviceSelection::VideoCard, 238844))
                        };       
                        if ui.button(egui::RichText::new("Tick CGA +912").font(egui::FontId::proportional(20.0))).clicked() {
                            events.push_back(GuiEvent::TickDevice(DeviceSelection::VideoCard,  912))
                        };                                   
                        if ui.button(egui::RichText::new("Tick CGA -8 (no vadj)").font(egui::FontId::proportional(20.0))).clicked() {
                            events.push_back(GuiEvent::TickDevice(DeviceSelection::VideoCard, 233464))
                        };           
                        if ui.button(egui::RichText::new("Tick CGA -104 (no vadj)").font(egui::FontId::proportional(20.0))).clicked() {
                            events.push_back(GuiEvent::TickDevice(DeviceSelection::VideoCard,  233638))
                        };     
                        if ui.button(egui::RichText::new("Tick CGA -912 (no vadj").font(egui::FontId::proportional(20.0))).clicked() {
                            events.push_back(GuiEvent::TickDevice(DeviceSelection::VideoCard,  232560))
                        };                               

                                                                  
                });
            });     

        });
    }
}
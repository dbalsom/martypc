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

    egui::device_control.rs

    Implements debug controls for system devices, allowing them to be 
    ticked independently of the rest of the system.

*/

use crate::egui::*;

pub struct DeviceControl {
    params: bool
}

impl DeviceControl {
    
    pub fn new() -> Self {
        Self {
            params: false
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
                        if ui.button(egui::RichText::new("Video").font(egui::FontId::proportional(20.0))).clicked() {
                            events.push_back(GuiEvent::TickDevice(DeviceSelection::VideoCard, 1))
                        };                    
                });
            });

            ui.vertical(|ui|{
                ui.label("Tick 100:");
                ui.group(|ui| {

                        if ui.button(egui::RichText::new("Timer 0").font(egui::FontId::proportional(20.0))).clicked() {
                            events.push_back(GuiEvent::TickDevice(DeviceSelection::Timer(0), 100))
                        };
                        if ui.button(egui::RichText::new("Video").font(egui::FontId::proportional(20.0))).clicked() {
                            events.push_back(GuiEvent::TickDevice(DeviceSelection::VideoCard, 100))
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
                        if ui.button(egui::RichText::new("Tick CGA -1 (no vadj)").font(egui::FontId::proportional(20.0))).clicked() {
                            events.push_back(GuiEvent::TickDevice(DeviceSelection::VideoCard, 233471))
                        };           
                        if ui.button(egui::RichText::new("Tick CGA -100 (no vadj)").font(egui::FontId::proportional(20.0))).clicked() {
                            events.push_back(GuiEvent::TickDevice(DeviceSelection::VideoCard,  233372))
                        };                                                   
                });
            });     

        });
    }
}
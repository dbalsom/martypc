use crate::egui::{GuiState, GuiEvent};


impl GuiState {

    pub fn draw_menu(&mut self, ui: &mut egui::Ui) {

        egui::menu::bar(ui, |ui| {

            ui.menu_button("Emulator", |ui| {
                if ui.button("Performance...").clicked() {
                    self.perf_viewer_open = true;
                    ui.close_menu();
                }
                if ui.button("About...").clicked() {
                    self.about_window_open = true;
                    ui.close_menu();
                }                    
            });
            ui.menu_button("Media", |ui| {
                ui.style_mut().spacing.item_spacing = egui::Vec2{ x: 6.0, y:6.0 };

                ui.menu_button("Load Floppy in Drive A:...", |ui| {
                    for name in &self.floppy_names {
                        if ui.button(name.to_str().unwrap()).clicked() {
                            
                            log::debug!("Selected floppy filename: {:?}", name);
                            
                            self.new_floppy_name0 = Some(name.clone());
                            self.event_queue.push_back(GuiEvent::LoadFloppy(0, name.clone()));
                            ui.close_menu();
                        }
                    }
                });

                ui.menu_button("Load Floppy in Drive B:...", |ui| {
                    for name in &self.floppy_names {
                        if ui.button(name.to_str().unwrap()).clicked() {
                            
                            log::debug!("Selected floppy filename: {:?}", name);
                            
                            self.new_floppy_name1 = Some(name.clone());
                            self.event_queue.push_back(GuiEvent::LoadFloppy(1, name.clone()));
                            ui.close_menu();
                        }
                    }
                });      
                
                if ui.button("Eject Floppy in Drive A:...").clicked() {
                    self.event_queue.push_back(GuiEvent::EjectFloppy(0));
                    ui.close_menu();
                };       
                
                if ui.button("Eject Floppy in Drive B:...").clicked() {
                    self.event_queue.push_back(GuiEvent::EjectFloppy(1));
                    ui.close_menu();
                };                              

                ui.menu_button("Load VHD in Drive 0:...", |ui| {
                    for name in &self.vhd_names {

                        if ui.radio_value(&mut self.vhd_name0, name.clone(), name.to_str().unwrap()).clicked() {

                            log::debug!("Selected VHD filename: {:?}", name);
                            self.new_vhd_name0 = Some(name.clone());
                            ui.close_menu();
                        }
                    }
                });                               

                if ui.button("Create new VHD...").clicked() {
                    self.vhd_creator_open = true;
                    ui.close_menu();
                };
                
            });
            ui.menu_button("Debug", |ui| {
                ui.menu_button("Dump Memory", |ui| {
                    if ui.button("Video Memory").clicked() {
                        self.event_queue.push_back(GuiEvent::DumpVRAM);
                        ui.close_menu();
                    }
                    if ui.button("Code Segment").clicked() {
                        self.event_queue.push_back(GuiEvent::DumpCS);
                        ui.close_menu();
                    }                        
                });
                if ui.button("CPU Control...").clicked() {
                    self.cpu_control_dialog_open = true;
                    ui.close_menu();
                }
                if ui.button("Memory...").clicked() {
                    self.memory_viewer_open = true;
                    ui.close_menu();
                }
                if ui.button("Registers...").clicked() {
                    self.register_viewer_open = true;
                    ui.close_menu();
                }
                if ui.button("Instruction Trace...").clicked() {
                    self.trace_viewer_open = true;
                    ui.close_menu();
                }
                if ui.button("Call Stack...").clicked() {
                    self.call_stack_open = true;
                    ui.close_menu();
                }                    
                if ui.button("Disassembly...").clicked() {
                    self.disassembly_viewer_open = true;
                    ui.close_menu();
                }
                if ui.button("PIC...").clicked() {
                    self.pic_viewer_open = true;
                    ui.close_menu();
                }    
                if ui.button("PIT...").clicked() {
                    self.pit_viewer_open = true;
                    ui.close_menu();
                }
                if ui.button("PPI...").clicked() {
                    self.ppi_viewer_open = true;
                    ui.close_menu();
                }
                if ui.button("DMA...").clicked() {
                    self.dma_viewer_open = true;
                    ui.close_menu();
                }
                if ui.button("Video Card...").clicked() {
                    self.videocard_viewer_open = true;
                    ui.close_menu();
                }
            
            });
            ui.menu_button("Options", |ui| {
                if ui.checkbox(&mut self.aspect_correct, "Correct Aspect Ratio").clicked() {
                    ui.close_menu();
                }
                if ui.checkbox(&mut self.composite, "Composite Monitor").clicked() {
                    ui.close_menu();
                }
                ui.menu_button("Attach COM2: ...", |ui| {
                    for port in &self.serial_ports {

                        if ui.radio_value(&mut self.serial_port_name, port.port_name.clone(), port.port_name.clone()).clicked() {

                            self.event_queue.push_back(GuiEvent::BridgeSerialPort(self.serial_port_name.clone()));
                            ui.close_menu();
                        }
                    }
                });                                
            });
        });

    }
}
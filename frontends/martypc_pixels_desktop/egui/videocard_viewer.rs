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

    --------------------------------------------------------------------------

    egui::videocard_viewer.rs

    Implements a debug display for video card registers and state.

*/

use egui::CollapsingHeader;

use crate::egui::{GuiState};
use marty_core::videocard::{VideoCardState, VideoCardStateEntry};

impl GuiState {

    pub fn draw_video_card_panel(ui: &mut egui::Ui, videocard_state: &VideoCardState) {

        egui::Grid::new("videocard_view1")
        .num_columns(2)
        .striped(true)
        .min_col_width(50.0)
        .show(ui, |ui| {       
            let register_file = videocard_state.get("General");
            match register_file {
                Some(file) => {
                    for register in file {   
                        ui.label(egui::RichText::new(&register.0).text_style(egui::TextStyle::Monospace));

                        match &register.1 {
                            VideoCardStateEntry::String(str) => {
                                ui.label(egui::RichText::new(str).text_style(egui::TextStyle::Monospace));
                            },
                            _=> {
                                ui.label("unsupported entry type");
                            }
                        }

                        ui.end_row();
                    }
                }
                None => {}
            }
        });  


        egui::Grid::new("videocard_view0")
        .num_columns(2)
        .striped(false)
        .show(ui, |ui| {

            if videocard_state.contains_key("CRTC") {
                ui.vertical(|ui| {
                    CollapsingHeader::new("CRTC Registers")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.group(|ui| {
                                    egui::Grid::new("videocard_view2")
                                        .num_columns(2)
                                        .striped(true)
                                        .min_col_width(50.0)
                                        .show(ui, |ui| {                                    
                                        let register_file = videocard_state.get("CRTC");
                                        match register_file {
                                            Some(file) => {
                                                for register in file {   
                                                    ui.label(egui::RichText::new(&register.0).text_style(egui::TextStyle::Monospace));
                                                    match &register.1 {
                                                        VideoCardStateEntry::String(str) => {
                                                            ui.label(egui::RichText::new(str).text_style(egui::TextStyle::Monospace));
                                                        },
                                                        _=> {
                                                            ui.label("unsupported entry type");
                                                        }
                                                    }
                                                    ui.end_row();
                                                }
                                            }
                                            None => {}
                                        }
                                    });
                                });                    
                            });
                        });
                    });
                });
            }

            ui.vertical(|ui| {
                if videocard_state.contains_key("Internal") {
                    CollapsingHeader::new("Internal Registers")
                    .default_open(false)
                    .show(ui,  |ui| {
                        ui.vertical(|ui| {

                            ui.horizontal(|ui| {
                                ui.group(|ui| {
                                    egui::Grid::new("videocard_view14")
                                        .num_columns(2)
                                        .striped(true)
                                        .min_col_width(60.0)
                                        .show(ui, |ui| {                                    
                                        let register_file = videocard_state.get("Internal");
                                        match register_file {
                                            Some(file) => {
                                                for register in file {   
                                                    ui.label(egui::RichText::new(&register.0).text_style(egui::TextStyle::Monospace));
                                                    match &register.1 {
                                                        VideoCardStateEntry::String(str) => {
                                                            ui.label(egui::RichText::new(str).text_style(egui::TextStyle::Monospace));
                                                        },
                                                        _=> {
                                                            ui.label("unsupported entry type");
                                                        }
                                                    }
                                                    ui.end_row();
                                                }
                                            }
                                            None => {}
                                        }
                                    });
                                });                    
                            });
                        });
                    });
                }                  
                if videocard_state.contains_key("External") {
                    CollapsingHeader::new("External Registers")
                    .default_open(false)
                    .show(ui,  |ui| {
                        ui.vertical(|ui| {
                            //ui.label(egui::RichText::new("External Registers").color(egui::Color32::LIGHT_BLUE));
                            ui.horizontal(|ui| {
                                ui.group(|ui| {
                                    egui::Grid::new("videocard_view13")
                                        .num_columns(2)
                                        .striped(true)
                                        .min_col_width(60.0)
                                        .show(ui, |ui| {                                    
                                        let register_file = videocard_state.get("External");
                                        match register_file {
                                            Some(file) => {
                                                for register in file {   
                                                    ui.label(egui::RichText::new(&register.0).text_style(egui::TextStyle::Monospace));
                                                    match &register.1 {
                                                        VideoCardStateEntry::String(str) => {
                                                            ui.label(egui::RichText::new(str).text_style(egui::TextStyle::Monospace));
                                                        },
                                                        _=> {
                                                            ui.label("unsupported entry type");
                                                        }
                                                    }
                                                    ui.end_row();
                                                }
                                            }
                                            None => {}
                                        }
                                    });
                                });                    
                            });
                        });
                    });
                }                        
                if videocard_state.contains_key("Sequencer") {
                    CollapsingHeader::new("Sequencer Registers")
                    .default_open(false)
                    .show(ui,  |ui| {
                        ui.vertical(|ui| {
                            //ui.label(egui::RichText::new("Sequencer Registers").color(egui::Color32::LIGHT_BLUE));
                            ui.horizontal(|ui| {
                                ui.group(|ui| {
                                    egui::Grid::new("videocard_view3")
                                        .num_columns(2)
                                        .striped(true)
                                        .min_col_width(60.0)
                                        .show(ui, |ui| {                                    
                                        let register_file = videocard_state.get("Sequencer");
                                        match register_file {
                                            Some(file) => {
                                                for register in file {   
                                                    ui.label(egui::RichText::new(&register.0).text_style(egui::TextStyle::Monospace));
                                                    match &register.1 {
                                                        VideoCardStateEntry::String(str) => {
                                                            ui.label(egui::RichText::new(str).text_style(egui::TextStyle::Monospace));
                                                        },
                                                        _=> {
                                                            ui.label("unsupported entry type");
                                                        }
                                                    }
                                                    ui.end_row();
                                                }
                                            }
                                            None => {}
                                        }
                                    });
                                });                    
                            });
                        });
                    });
                }
                if videocard_state.contains_key("Graphics") {
                    CollapsingHeader::new("Graphics Registers")
                    .default_open(false)
                    .show(ui,  |ui| {
                        ui.vertical(|ui| {
                            //ui.label(egui::RichText::new("Graphics Registers").color(egui::Color32::LIGHT_BLUE));
                            ui.horizontal(|ui| {
                                ui.group(|ui| {
                                    egui::Grid::new("videocard_view4")
                                        .num_columns(2)
                                        .striped(true)
                                        .min_col_width(50.0)
                                        .show(ui, |ui| {                                    
                                        let register_file = videocard_state.get("Graphics");
                                        match register_file {
                                            Some(file) => {
                                                for register in file {   
                                                    ui.label(egui::RichText::new(&register.0).text_style(egui::TextStyle::Monospace));
                                                    match &register.1 {
                                                        VideoCardStateEntry::String(str) => {
                                                            ui.label(egui::RichText::new(str).text_style(egui::TextStyle::Monospace));
                                                        },
                                                        _=> {
                                                            ui.label("unsupported entry type");
                                                        }
                                                    }
                                                    ui.end_row();
                                                }
                                            }
                                            None => {}
                                        }
                                    });
                                });                    
                            });
                        });
                    });
                }
                if videocard_state.contains_key("AttributePalette") {
                    CollapsingHeader::new("Attribute Palette Registers")
                    .default_open(false)
                    .show(ui,  |ui| {                            
                        ui.vertical(|ui| {
                            //ui.label(egui::RichText::new("Attribute Palette Registers").color(egui::Color32::LIGHT_BLUE));
                            ui.horizontal(|ui| {
                                ui.group(|ui| {
                                    egui::Grid::new("videocard_view6")
                                        .num_columns(2)
                                        .striped(true)
                                        .min_col_width(50.0)
                                        .show(ui, |ui| {                                    
                                        let register_file = videocard_state.get("AttributePalette");
                                        match register_file {
                                            Some(file) => {
                                                for register in file {   
                                                    ui.label(egui::RichText::new(&register.0).text_style(egui::TextStyle::Monospace));
                                                    match &register.1 {
                                                        VideoCardStateEntry::String(str) => {
                                                            ui.label(egui::RichText::new(str).text_style(egui::TextStyle::Monospace));
                                                        },
                                                        VideoCardStateEntry::Color(str, r, g, b) => {
                                                            ui.label(egui::RichText::new(str).text_style(egui::TextStyle::Monospace));
                                                            GuiState::color_swatch(ui, egui::Color32::from_rgb(*r, *g, *b), true);
                                                        }
                                                        _=> {
                                                            ui.label("unsupported entry type");
                                                        }
                                                    }

                                                    
                                                    ui.end_row();
                                                }
                                            }
                                            None => {}
                                        }
                                    });
                                });                    
                            });
                        });
                    });
                }                          
                if videocard_state.contains_key("Attribute") {
                    CollapsingHeader::new("Attribute Registers")
                    .default_open(false)
                    .show(ui,  |ui| {                            
                        ui.vertical(|ui| {
                            //ui.label(egui::RichText::new("Attribute Registers").color(egui::Color32::LIGHT_BLUE));
                            ui.horizontal(|ui| {
                                ui.group(|ui| {
                                    egui::Grid::new("videocard_view7")
                                        .num_columns(2)
                                        .striped(true)
                                        .min_col_width(50.0)
                                        .show(ui, |ui| {                                    
                                        let register_file = videocard_state.get("Attribute");
                                        match register_file {
                                            Some(file) => {
                                                for register in file {   
                                                    ui.label(egui::RichText::new(&register.0).text_style(egui::TextStyle::Monospace));
                                                    match &register.1 {
                                                        VideoCardStateEntry::String(str) => {
                                                            ui.label(egui::RichText::new(str).text_style(egui::TextStyle::Monospace));
                                                        },
                                                        _=> {
                                                            ui.label("unsupported entry type");
                                                        }
                                                    }
                                                    ui.end_row();
                                                }
                                            }
                                            None => {}
                                        }
                                    });
                                });                    
                            });
                        });
                    });
                }

                if videocard_state.contains_key("DACPalette") {
                    CollapsingHeader::new("DAC Palette Registers")
                    .default_open(false)
                    .show(ui,  |ui| {                            
                        ui.vertical(|ui| {
                            //ui.label(egui::RichText::new("Attribute Palette Registers").color(egui::Color32::LIGHT_BLUE));
                            ui.horizontal(|ui| {
                                ui.group(|ui| {
                                    egui::Grid::new("videocard_view6")
                                        .num_columns(16)
                                        .striped(true)
                                        .min_col_width(0.0)
                                        .show(ui, |ui| {                                    
                                            let register_file = videocard_state.get("DACPalette");
                                            match register_file {
                                                Some(file) => {
                                                    let mut reg_ct = 0;
                                                    for register in file {
                                                        if let VideoCardStateEntry::Color(_str, r, g, b) = &register.1 {
                                                            GuiState::color_swatch(ui, egui::Color32::from_rgb(*r, *g, *b), true);
                                                        }
                                                        reg_ct += 1;
                                                        if reg_ct == 16 {
                                                            ui.end_row();
                                                            reg_ct = 0;
                                                        }
                                                    }
                                                }
                                                None => {}
                                            }
                                        });
                                    });                    
                                });
                            });
                        });
                    }                               

                });
        }); 
    }
}
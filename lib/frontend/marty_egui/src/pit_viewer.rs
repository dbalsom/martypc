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
    
    egui::pit_viewer.rs

    Implements a viewer control for the Programmable Interval Timer.
    
    This viewer displays data regarding the Programmable Interval Timer's 
    3 channels, as well as displaying a graph of the timer output.

*/

use egui::*;

/*
use egui::plot::{
    Line, 
    //Plot, 
    PlotPoints, 
    //PlotBounds
};*/

use crate::*;
use crate::color::*;
use crate::constants::*;

use marty_core::devices::pit::PitDisplayState;
use marty_core::syntax_token::*;

#[allow (dead_code)]
pub struct PitViewerControl {

    pit_state: PitDisplayState,
    channel_vecs: [Vec<u8>; 3],
    //channel_data: [PlotPoints; 3],
    //channel_lines: [Line; 3]
}

impl PitViewerControl {

    pub fn new() -> Self {
        Self {
            pit_state: Default::default(),
            channel_vecs: [
                Vec::new(), Vec::new(), Vec::new()
            ],
            /*
            channel_data: [
                PlotPoints::new(Vec::new()),
                PlotPoints::new(Vec::new()),
                PlotPoints::new(Vec::new())
            ],
            channel_lines: [
                Line::new(PlotPoints::new(Vec::new())),
                Line::new(PlotPoints::new(Vec::new())),
                Line::new(PlotPoints::new(Vec::new()))
            ]

             */
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, _events: &mut GuiEventQueue ) {

        for (i, channel) in self.pit_state.iter().enumerate() {

            egui::CollapsingHeader::new(format!("Channel: {}", i))
            .default_open(true)
            .show(ui, |ui| {

                ui.horizontal(|ui| {
                    ui.set_min_width(PIT_VIEWER_WIDTH);
                    ui.group(|ui| {

                        ui.set_min_width(PIT_VIEWER_WIDTH);

                        egui::Grid::new(format!("pit_view{}", i))
                        .num_columns(2)
                        .spacing([40.0, 4.0])
                        .striped(true)

                        .show(ui, |ui| {
                        
                            for (key, value) in channel {

                                if let SyntaxToken::StateString(text, _, age) = value {
                                    ui.label(egui::RichText::new(*key).text_style(egui::TextStyle::Monospace));
                                    ui.label(
                                        egui::RichText::new(text)
                                            .text_style(egui::TextStyle::Monospace)
                                            .color(fade_c32(Color32::GRAY, STATUS_UPDATE_COLOR, 255-*age))
                                        );
                                    //ui.add(egui::TextEdit::singleline(&mut self.pit_state.c0_access_mode).font(egui::TextStyle::Monospace));
                                    ui.end_row();

                                    
                                }
                            }
                        });
                    });
                });

                /*
                Plot::new(format!("pit_plot{}", i))
                .view_aspect(2.0)
                .width(PIT_VIEWER_WIDTH - 10.0)
                .height(75.0)
                .allow_scroll(false)
                .allow_zoom(false)
                .show_x(true)
                .show_y(true)
                .show(ui, |ui| {

                    ui.set_plot_bounds(PlotBounds::from_min_max([0.0,0.0], [100.0,1.0]));

                    let points: PlotPoints = self.channel_vecs[i].iter().enumerate().map(|i| {
                        
                        let x = i.0 as f64;
            
                        // Convert u8 to f32
                        let y = if *i.1 == 1u8 { 1.0 } else { 0.0 };
            
                        [x, y]
                    }).collect();

                    ui.line(Line::new(points));
                });
                */
            });

        }  
    }

    pub fn update_state(&mut self, state: &PitDisplayState ) {


        let mut new_pit_state = state.clone();

        // Update state entry ages
        for (i, channel) in new_pit_state.iter_mut().enumerate() {
            for (key, value) in channel.iter_mut() {

                if let SyntaxToken::StateString(_txt, dirty, age) = value {
                    if *dirty {
                        *age = 0;
                    }
                    else if i < self.pit_state.len() {
                        if let Some(old_tok) = self.pit_state[i].get_mut(key) {
                            if let SyntaxToken::StateString(_,_,old_age) = old_tok {
                                *age = old_age.saturating_add(2);                            
                            }
                        }
                    }
                }
            }
        }

        self.pit_state = new_pit_state;
    }

    pub fn update_channel_data(&mut self, channel: usize, data: &[u8]) {

        self.channel_vecs[channel] = data.to_vec();



        //self.channel_data[channel] = points;
        //self.channel_lines[channel] = Line::new(points);
    }
    

}


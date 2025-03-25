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

    --------------------------------------------------------------------------
*/
use egui::{ecolor::Hsva, *};

pub struct VuMeter {
    pub segments: u8,
    pub level:    f32,
}

impl VuMeter {
    pub fn new(segments: u8, level: f32) -> Self {
        VuMeter { segments, level }
    }
}

impl Widget for VuMeter {
    fn ui(self, ui: &mut Ui) -> Response {
        let segments = self.segments as f32;
        let filled_segments = (self.level * segments).ceil() as u8;

        ui.horizontal(|ui| {
            let mut spacing = ui.spacing().item_spacing;
            spacing.x = spacing.x / 2.0;
            ui.spacing_mut().item_spacing = spacing;
            //ui.spacing_mut().button_padding = vec2(0.0, 0.0);

            // Allocate space
            let size = Vec2 {
                x: ui.spacing().interact_size.y / 2.0,
                y: ui.spacing().interact_size.y,
            };
            let sense = Sense {
                click: false,
                drag: false,
                focusable: false,
            };

            for i in 0..self.segments {
                // Compute color
                let color = if i < filled_segments {
                    color_for_segment(i, self.segments)
                }
                else {
                    Color32::from_gray(20)
                };

                let (rect, response) = ui.allocate_exact_size(size, sense);

                if ui.is_rect_visible(rect) {
                    let visuals = ui.style().interact(&response);

                    //painter.rect_filled(rect, 0.0, color);

                    let rounding = visuals.rounding.at_most(2.0);

                    ui.painter().rect_filled(rect, rounding, color);
                    ui.painter().rect_stroke(rect, rounding, (2.0, visuals.bg_fill));
                    // fill is intentional, because default style has no border
                }
            }
        });

        ui.response()
    }
}

fn color_for_segment(i: u8, total_segments: u8) -> Color32 {
    // Avoid division by zero, just in case
    if total_segments <= 1 {
        return Color32::GREEN;
    }

    // Fraction from 0.0 to 1.0 across the segments
    let mut fraction = i as f32 / (total_segments - 1) as f32;

    // “Log-like” skew: raise to a power > 1.0 to linger in green longer
    fraction = fraction.powf(1.4);

    // We’ll do a piecewise linear interpolation in hue:
    //   0.0 .. 0.5 => green (120 deg) to yellow (60 deg)
    //   0.5 .. 1.0 => yellow (60 deg) to red (0 deg)
    //
    // (These hue values are in degrees on the 0..=360 color wheel, so
    //  120° = green, 60° = yellow, 0° = red.)

    let (hue, saturation, value) = if fraction < 0.5 {
        // Interpolate from hue=120 (green) down to hue=60 (yellow)
        let t = fraction / 0.5; // Remap [0..0.5] => [0..1]
        let hue = 120.0 - 60.0 * t;
        (hue, 1.0, 1.0)
    }
    else {
        // Interpolate from hue=60 (yellow) down to hue=0 (red)
        let t = (fraction - 0.5) / 0.5; // Remap [0.5..1.0] => [0..1]
        let hue = 60.0 - 60.0 * t;
        (hue, 1.0, 1.0)
    };

    // Convert degrees-based HSV to an egui Color32
    let hsv = Hsva::new(hue / 360.0, saturation, value, 1.0);
    Color32::from(hsv)
}

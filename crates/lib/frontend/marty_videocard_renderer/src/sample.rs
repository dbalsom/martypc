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

    ---------------------------------------------------------------------------

    marty_videocard_renderer::sample.rs

    Sampling routines for VideoRenderer (primarily used for light pen emulation)
    At some point this should be moved to monitor emulation in the core.
*/
use crate::VideoRenderer;

impl VideoRenderer {
    pub fn sample_luma(frame: &mut [u8], w: u32, span: u32, h: u32, x: u32, y: u32) -> f32 {
        let mut total_luma = 0.0;

        // Luminance coefficients (BT.709)
        //const R_COEF: f32 = 0.2126;
        //const G_COEF: f32 = 0.7152;
        //const B_COEF: f32 = 0.0722;

        // Custom weights for light pen emulation.
        const R_COEF: f32 = 0.3;
        const G_COEF: f32 = 0.4;
        const B_COEF: f32 = 0.7;

        // Iterate through the 5x5 kernel (-2 to +2)
        for ky in -2..=2 {
            for kx in -2..=2 {
                // Clamp coordinates to image boundaries
                let curr_x = ((x as i32 + kx).max(0).min(w as i32 - 1)) as u32;
                let curr_y = ((y as i32 + ky).max(0).min(h as i32 - 1)) as u32;

                let offset = ((curr_y * span) + curr_x) as usize * 4;

                if offset + 2 < frame.len() {
                    // Draw debug blue rect
                    //frame[offset + 2] = 255;

                    let r = frame[offset] as f32 / 255.0;
                    let g = frame[offset + 1] as f32 / 255.0;
                    let b = frame[offset + 2] as f32 / 255.0;

                    total_luma += (r * R_COEF) + (g * G_COEF) + (b * B_COEF);
                }
            }
        }

        // Return the average luminance across the 25 samples
        total_luma / 25.0
    }
}

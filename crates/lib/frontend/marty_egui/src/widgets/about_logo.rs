/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the "Software"),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.

    --------------------------------------------------------------------------

    marty_egui::widgets::about_logo.rs

    Implements the animated logo widget used by the About dialog.
*/

use egui::{Color32, ColorImage, Response, Sense, TextureHandle, TextureOptions, Ui};
use std::{sync::Arc, time::Duration};

#[derive(Clone, Copy)]
#[allow(dead_code)] // One variant is intentionally inactive based on ABOUT_ASSET_SIZE.
enum AboutAssetSize {
    Small,
    Large,
}

const ABOUT_ASSET_SIZE: AboutAssetSize = AboutAssetSize::Small;
const PLASMA_DIMENSIONS: [usize; 2] = ABOUT_ASSET_SIZE.dimensions();
const PLASMA_WIDTH: usize = PLASMA_DIMENSIONS[0];
const PLASMA_HEIGHT: usize = PLASMA_DIMENSIONS[1];
const PLASMA_UV_HEIGHT: f64 = 100.0;
const PLASMA_UV_WIDTH: f64 = PLASMA_UV_HEIGHT * PLASMA_WIDTH as f64 / PLASMA_HEIGHT as f64;
const PLASMA_FRAME_TIME: f64 = 0.060;
const LOGO_WIDTH: f32 = PLASMA_WIDTH as f32;
const LOGO_HEIGHT: f32 = PLASMA_HEIGHT as f32;

impl AboutAssetSize {
    const fn dimensions(self) -> [usize; 2] {
        match self {
            Self::Small => [378, 135],
            Self::Large => [755, 270],
        }
    }

    fn mask_bytes(self) -> &'static [u8] {
        match self {
            Self::Small => include_bytes!("../../../../../../assets/marty_logo_about_small_mask.png"),
            Self::Large => include_bytes!("../../../../../../assets/marty_logo_about_mask.png"),
        }
    }

    fn logo(self) -> egui::ImageSource<'static> {
        match self {
            Self::Small => egui::include_image!("../../../../../../assets/marty_logo_about_small.png"),
            Self::Large => egui::include_image!("../../../../../../assets/marty_logo_about.png"),
        }
    }
}

/// Renders the MartyPC About logo with a masked plasma effect.
///
/// The plasma is a more-or-less direct port of a Win32 effect I wrote in May of 1999.
pub struct AboutLogoWidget {
    palette: [Color32; 256],
    mask: Vec<u8>,
    image: Arc<ColorImage>,
    texture: Option<TextureHandle>,
    start_time: Option<f64>,
    last_frame: Option<u64>,
}

impl AboutLogoWidget {
    pub fn new() -> Self {
        let palette = std::array::from_fn(|index| {
            let radians = (index as f64).to_radians();
            Color32::from_rgb(
                Self::wrapping_byte(radians.sinh() * 256.0),
                Self::wrapping_byte(radians.sin() * 256.0),
                Self::wrapping_byte(radians.cos() * 256.0),
            )
        });

        let mask_image = ::image::load_from_memory(ABOUT_ASSET_SIZE.mask_bytes())
            .expect("embedded About logo plasma mask should be a valid image")
            .to_luma8();
        assert_eq!(
            mask_image.dimensions(),
            (PLASMA_WIDTH as u32, PLASMA_HEIGHT as u32),
            "About logo plasma mask dimensions must match the logo",
        );

        Self {
            palette,
            mask: mask_image.into_raw(),
            image: Arc::new(ColorImage::new(
                [PLASMA_WIDTH, PLASMA_HEIGHT],
                vec![Color32::TRANSPARENT; PLASMA_WIDTH * PLASMA_HEIGHT],
            )),
            texture: None,
            start_time: None,
            last_frame: None,
        }
    }

    /// The Win32 code assigned floating-point results directly to eight-bit color channels.
    /// This method reproduces low-byte wrapping explicitly instead of Rust's saturating float
    /// conversion.
    fn wrapping_byte(value: f64) -> u8 {
        value.trunc() as i32 as u8
    }

    fn sample(phase: f64) -> f32 {
        phase.rem_euclid(360.0).to_radians().sin() as f32
    }

    fn render_frame(&mut self, alpha: u64) -> Arc<ColorImage> {
        let palette = &self.palette;
        let mask = &self.mask;
        let pixels = &mut Arc::make_mut(&mut self.image).pixels;
        let mut x_results = [0.0; PLASMA_WIDTH];

        for (x, result) in x_results.iter_mut().enumerate() {
            let u = x as f64 / PLASMA_WIDTH as f64;
            let x_phase = u * PLASMA_UV_WIDTH;
            *result = 20.0 * Self::sample(x_phase * 4.0 + alpha as f64)
                + 30.0 * Self::sample(x_phase + (alpha * 4) as f64)
                + 50.0 * Self::sample(x_phase / 4.0 + (alpha / 4) as f64);
        }

        for y in 0..PLASMA_HEIGHT {
            let v = y as f64 / PLASMA_HEIGHT as f64;
            let y_phase = v * PLASMA_UV_HEIGHT;
            let result_y = 40.0 * Self::sample(y_phase * 6.0 + alpha as f64)
                + 40.0 * Self::sample(y_phase + (alpha * 6) as f64)
                + 20.0 * Self::sample(y_phase + (alpha / 6) as f64);

            for x in 0..PLASMA_WIDTH {
                let color_index = (result_y + x_results[x]).trunc() as i32 as u8;

                // The original positive-height DIB section stored its scanlines bottom-up.
                let output_y = PLASMA_HEIGHT - 1 - y;
                let output_index = x + output_y * PLASMA_WIDTH;
                let color = palette[color_index as usize];
                pixels[output_index] =
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), mask[output_index]);
            }
        }

        Arc::clone(&self.image)
    }

    pub fn show(&mut self, ui: &mut Ui) -> Response {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|input| input.time);
        let start_time = *self.start_time.get_or_insert(now);
        let frame = ((now - start_time) / PLASMA_FRAME_TIME).floor() as u64;

        if self.last_frame != Some(frame) {
            let image = self.render_frame(frame);
            if let Some(texture) = self.texture.as_mut() {
                texture.set(image, TextureOptions::NEAREST);
            }
            else {
                self.texture = Some(ctx.load_texture("about_plasma", image, TextureOptions::NEAREST));
            }
            self.last_frame = Some(frame);
        }

        let (rect, response) = ui.allocate_exact_size(egui::vec2(LOGO_WIDTH, LOGO_HEIGHT), Sense::hover());
        egui::Image::new(ABOUT_ASSET_SIZE.logo()).paint_at(ui, rect);

        if let Some(texture) = self.texture.as_ref() {
            ui.painter().image(
                texture.id(),
                rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }

        ctx.request_repaint_after(Duration::from_millis(60));
        response
    }
}

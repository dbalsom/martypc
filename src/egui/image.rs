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

    --------------------------------------------------------------------------

    egui::image.rs

    Routines for loading and manipulating GUI images.
*/

use egui::ColorImage;
//use image;

static LOGO_IMAGE: &[u8] = include_bytes!("../../assets/marty_logo_about.png");

// Support other images later if needed
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum UiImage {
    Logo,
}

pub fn get_ui_image(img_select: UiImage) -> ColorImage {

    match img_select {
        UiImage::Logo => {
            // This shouldn't fail since its all static
            load_image_from_memory(LOGO_IMAGE).unwrap()
        }   
    }
}

fn load_image_from_memory(image_data: &[u8]) -> Result<ColorImage, image::ImageError> {
    let image = image::load_from_memory(image_data)?;
    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();
    Ok(ColorImage::from_rgba_unmultiplied(
        size,
        pixels.as_slice(),
    ))
}



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


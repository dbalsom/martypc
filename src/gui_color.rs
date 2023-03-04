
use egui::Color32;

pub fn darken_c32(color: Color32, percent: f32) -> Color32 {
    /*
    let (r,g,b,_) = color.to_rgba8();

    let dr = (r * percent) as u8;
    let dg = (g * percent) as u8;
    let db = (b * percent) as u8;

    egui::Color32::from_rgb(dr, dg, db)
    */
    color
}
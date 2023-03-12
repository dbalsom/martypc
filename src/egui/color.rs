use egui::{
    Color32, 
};

pub const STATUS_UPDATE_COLOR: Color32 = Color32::from_rgb(0, 255, 255);

pub fn darken_c32(color: Color32, percent: f32) -> Color32 {
    
    let (r,g,b,_) = color.to_tuple();

    let dr = ((r as f32) * (1.0 - percent)) as u8;
    let dg = ((g as f32) * (1.0 - percent)) as u8;
    let db = ((b as f32) * (1.0 - percent)) as u8;

    egui::Color32::from_rgb(dr, dg, db)
}

pub fn lighten_c32(color: Color32, percent: f32) -> Color32 {
    
    let (r,g,b,_) = color.to_tuple();

    let dr = r.saturating_add(((r as f32) * percent) as u8);
    let dg = g.saturating_add(((g as f32) * percent) as u8);
    let db = b.saturating_add(((b as f32) * percent) as u8);

    egui::Color32::from_rgb(dr, dg, db)
}

pub fn add_c32(color: Color32, amount: u8) -> Color32 {
    
    let (r,g,b,_) = color.to_tuple();
    
    let dr = r.saturating_add(amount);
    let dg = g.saturating_add(amount);
    let db = b.saturating_add(amount);

    egui::Color32::from_rgb(dr, dg, db)
}

pub fn fade_c32(color1: Color32, color2: Color32, amount: u8) -> Color32 {

    let c1r: f32 = (color1.r() as f32) / 255.0;
    let c1g: f32 = (color1.g() as f32) / 255.0;
    let c1b: f32 = (color1.b() as f32) / 255.0;

    let c2r: f32 = (color2.r() as f32) / 255.0;
    let c2g: f32 = (color2.g() as f32) / 255.0;
    let c2b: f32 = (color2.b() as f32) / 255.0;

    let percent: f32 = (amount as f32) / 255.0;

    let result_r = c1r + (percent * (c2r - c1r));
    let result_g = c1g + (percent * (c2g - c1g));
    let result_b = c1b + (percent * (c2b - c1b));

    Color32::from_rgb((result_r * 255.0) as u8, (result_g * 255.0) as u8, (result_b * 255.0) as u8)
}
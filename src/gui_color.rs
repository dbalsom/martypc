
use egui::{
    Color32, 
};

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
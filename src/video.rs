// Video module
// This module takes an internal representation from the cga module and actually draws the screen
// It also defines representational details such as colors

extern crate rand; 
use rand::{
    distributions::{Distribution, Standard},
    Rng,
}; 

// Font is encoded as a bit pattern with a span of 256 bits per row
static CGA_FONT: &'static [u8; 2048] = include_bytes!("cga_font.bin");
const FONT_SPAN: u32 = 32;
const FONT_W: u32 = 8;
const FONT_H: u32 = 8;

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone)]
pub enum CGAColor {
    Black,
    Blue,
    Green,
    Cyan,
    Red,
    Magenta,
    Brown,
    White,
    BlackBright,
    BlueBright,
    GreenBright,
    CyanBright,
    RedBright,
    MagentaBright,
    Yellow,
    WhiteBright
}

// Random color generator
impl Distribution<CGAColor> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> CGAColor {
        // match rng.gen_range(0, 3) { // rand 0.5, 0.6, 0.7
        match rng.gen_range(0..=15) { // rand 0.8
            0 => CGAColor::Black,
            1 => CGAColor::Blue,       
            2 => CGAColor::Green,        
            3 => CGAColor::Cyan,       
            4 => CGAColor::Red,        
            5 => CGAColor::Magenta,      
            6 => CGAColor::Brown,      
            7 => CGAColor::White,       
            8 => CGAColor::BlackBright,  
            9 => CGAColor::BlueBright, 
            10 => CGAColor::GreenBright,  
            11 => CGAColor::CyanBright, 
            12 => CGAColor::RedBright,  
            13 => CGAColor::MagentaBright,
            14 => CGAColor::Yellow,
            _ => CGAColor::WhiteBright  
        }
    }
}

// Return a RGBA slice given a CGA color Enum
pub fn color_enum_to_rgba(color: CGAColor) -> &'static [u8; 4] {
    
    match color {
        CGAColor::Black         => &[0x00u8, 0x00u8, 0x00u8, 0xFFu8],
        CGAColor::Blue          => &[0x00u8, 0x00u8, 0xAAu8, 0xFFu8],
        CGAColor::Green         => &[0x00u8, 0xAAu8, 0x00u8, 0xFFu8],
        CGAColor::Cyan          => &[0x00u8, 0xAAu8, 0xAAu8, 0xFFu8],
        CGAColor::Red           => &[0xAAu8, 0x00u8, 0x00u8, 0xFFu8],
        CGAColor::Magenta       => &[0xAAu8, 0x00u8, 0xAAu8, 0xFFu8],
        CGAColor::Brown         => &[0xAAu8, 0x55u8, 0x00u8, 0xFFu8],
        CGAColor::White         => &[0xAAu8, 0xAAu8, 0xAAu8, 0xFFu8],
        CGAColor::BlackBright   => &[0x55u8, 0x55u8, 0x55u8, 0xFFu8],
        CGAColor::BlueBright    => &[0x55u8, 0x55u8, 0xFFu8, 0xFFu8],
        CGAColor::GreenBright   => &[0x55u8, 0xFFu8, 0x55u8, 0xFFu8],
        CGAColor::CyanBright    => &[0x55u8, 0xFFu8, 0xFFu8, 0xFFu8],
        CGAColor::RedBright     => &[0xFFu8, 0x55u8, 0x55u8, 0xFFu8],
        CGAColor::MagentaBright => &[0xFFu8, 0x55u8, 0xFFu8, 0xFFu8],
        CGAColor::Yellow        => &[0xFFu8, 0xFFu8, 0x55u8, 0xFFu8],
        CGAColor::WhiteBright   => &[0xFFu8, 0xFFu8, 0xFFu8, 0xFFu8],
    }
}

pub enum VideoMode {
    Mode0TextBw40,
    Mode1TextCo40,
    Mode2TextBw80,
    Mode3TextCo80,
    Mode4LowResGraphics,
    Mode5LowResAltPalette,
    Mode6HiResGraphics,
    Mode7LowResComposite,
    Mode8LowResTweaked
}

pub struct Video {


    mode: VideoMode,
}

impl Video {
    fn new() -> Self {
        Self {
            mode: VideoMode::Mode3TextCo80
        }
    }
}


// Draw a CGA font glyph at an arbitrary location
pub fn draw_glyph2x( 
    glyph: u8,
    fg_color: CGAColor,
    bg_color: CGAColor,
    frame: &mut [u8], 
    frame_w: u32, 
    frame_h: u32, 
    pos_x: u32, 
    pos_y: u32 ) -> () {

    // Do not draw glyph off screen
    if pos_x + FONT_W > frame_w {
        return
    }
    if pos_y + (FONT_H * 2) > frame_h {
        return
    }

    // Find the source position of the glyph
    let glyph_offset_src_x = glyph as u32 % FONT_SPAN;
    let glyph_offset_src_y = (glyph as u32 / FONT_SPAN) * (FONT_H * FONT_SPAN); 

    for draw_glyph_y in 0..FONT_H {

        let dst_row_offset = frame_w * 4 * ((pos_y * 2) + (draw_glyph_y*2));
        let dst_row_offset2 = dst_row_offset + (frame_w * 4);
        
        let glyph_offset = glyph_offset_src_y + (draw_glyph_y * FONT_SPAN) + glyph_offset_src_x;

        let glyph_byte: u8 = CGA_FONT[glyph_offset as usize];

        for draw_glyph_x in 0..FONT_W {
        
            let test_bit: u8 = 0x80u8 >> draw_glyph_x;

            let color = if test_bit & glyph_byte > 0 {
                color_enum_to_rgba(fg_color)
            }
            else {
                color_enum_to_rgba(bg_color)
            };

            let dst_offset = dst_row_offset + (pos_x + draw_glyph_x) * 4;
            frame[dst_offset as usize] = color[0];
            frame[dst_offset as usize + 1] = color[1];
            frame[dst_offset as usize + 2] = color[2];
            frame[dst_offset as usize + 3] = color[3];

            let dst_offset2 = dst_row_offset2 + (pos_x + draw_glyph_x) * 4;
            frame[dst_offset2 as usize] = color[0];
            frame[dst_offset2 as usize + 1] = color[1];
            frame[dst_offset2 as usize + 2] = color[2];
            frame[dst_offset2 as usize + 3] = color[3];            
        }
    }
     
}


// Draw a CGA font glyph at an arbitrary location
pub fn draw_glyph( 
    glyph: u8,
    fg_color: CGAColor,
    bg_color: CGAColor,
    frame: &mut [u8], 
    frame_w: u32, 
    frame_h: u32, 
    pos_x: u32, 
    pos_y: u32 ) -> () {

    // Do not draw glyph off screen
    if pos_x + FONT_W > frame_w {
        return
    }
    if pos_y + FONT_H > frame_h {
        return
    }

    // Find the source position of the glyph
    let glyph_offset_src_x = glyph as u32 % FONT_SPAN;
    let glyph_offset_src_y = (glyph as u32 / FONT_SPAN) * (FONT_H * FONT_SPAN); 

    for draw_glyph_y in 0..FONT_H {

        let dst_row_offset = frame_w * 4 * (pos_y + draw_glyph_y);
        let glyph_offset = glyph_offset_src_y + (draw_glyph_y * FONT_SPAN) + glyph_offset_src_x;

        let glyph_byte: u8 = CGA_FONT[glyph_offset as usize];

        for draw_glyph_x in 0..FONT_W {
        
            let test_bit: u8 = 0x80u8 >> draw_glyph_x;

            let color = if test_bit & glyph_byte > 0 {
                color_enum_to_rgba(fg_color)
            }
            else {
                color_enum_to_rgba(bg_color)
            };

            let dst_offset = dst_row_offset + (pos_x + draw_glyph_x) * 4;
            frame[dst_offset as usize] = color[0];
            frame[dst_offset as usize + 1] = color[1];
            frame[dst_offset as usize + 2] = color[2];
            frame[dst_offset as usize + 3] = color[3];
        }
    }
     
}

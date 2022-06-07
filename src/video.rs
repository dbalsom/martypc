#![allow(dead_code)]
// Video module
// This module takes an internal representation from the cga module and actually draws the screen
// It also defines representational details such as colors
use std::rc::Rc;
use std::cell::RefCell;

use crate::cga::{self, CGACard, CGAPalette, DisplayMode};
use crate::bus::BusInterface;

extern crate rand; 
use rand::{
    distributions::{Distribution, Standard},
    Rng,
}; 

pub const ATTR_BLUE_FG: u8 = 0b0000_0001;
pub const ATTR_GREEN_FG: u8 = 0b0000_0010;
pub const ATTR_RED_FG: u8 = 0b0000_0100;
pub const ATTR_BRIGHT_FG: u8 = 0b0000_1000;
pub const ATTR_BLUE_BG: u8 = 0b0001_0000;
pub const ATTR_GREEN_BG: u8 = 0b0010_0000;
pub const ATTR_RED_BG: u8 = 0b0100_0000;
pub const ATTR_BRIGHT_BG: u8 = 0b1000_0000;

// Font is encoded as a bit pattern with a span of 256 bits per row
static CGA_FONT: &'static [u8; 2048] = include_bytes!("cga_font.bin");

const CGA_FIELD_OFFSET: u32 = 8192;

const FONT_SPAN: u32 = 32;
const FONT_W: u32 = 8;
const FONT_H: u32 = 8;

const GFX_W: u32 = 320;
const GFX_H: u32 = 200;

const FRAME_W: u32 = 640;
const FRAME_H: u32 = 400;


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

pub fn get_cga_gfx_color(bits: u8, palette: &CGAPalette, intensity: bool) -> &'static [u8; 4] {
    match (bits, palette, intensity) {
        // Palette 0 - Low Intensity
        (0b00, CGAPalette::RedGreenYellow, false) => &[0x00u8, 0x00u8, 0x00u8, 0xFFu8], // Black
        (0b01, CGAPalette::RedGreenYellow, false) => &[0x00u8, 0xAAu8, 0x00u8, 0xFFu8], // Green
        (0b10, CGAPalette::RedGreenYellow, false) => &[0xAAu8, 0x00u8, 0x00u8, 0xFFu8], // Red
        (0b11, CGAPalette::RedGreenYellow, false) => &[0xAAu8, 0x55u8, 0x00u8, 0xFFu8], // Brown
        // Palette 0 - High Intensity
        (0b00, CGAPalette::RedGreenYellow, true) => &[0x00u8, 0x00u8, 0x00u8, 0xFFu8], // Black
        (0b01, CGAPalette::RedGreenYellow, true) => &[0x55u8, 0xFFu8, 0x55u8, 0xFFu8], // GreenBright
        (0b10, CGAPalette::RedGreenYellow, true) => &[0xFFu8, 0x55u8, 0x55u8, 0xFFu8], // RedBright
        (0b11, CGAPalette::RedGreenYellow, true) => &[0xFFu8, 0xFFu8, 0x55u8, 0xFFu8], // Yellow
        // Palette 1 - Low Intensity
        (0b00, CGAPalette::MagentaCyanWhite, false) => &[0x00u8, 0x00u8, 0x00u8, 0xFFu8], // Black
        (0b01, CGAPalette::MagentaCyanWhite, false) => &[0x00u8, 0xAAu8, 0xAAu8, 0xFFu8], // Cyan
        (0b10, CGAPalette::MagentaCyanWhite, false) => &[0xAAu8, 0x00u8, 0x00u8, 0xFFu8], // Magenta
        (0b11, CGAPalette::MagentaCyanWhite, false) => &[0xAAu8, 0x55u8, 0x00u8, 0xFFu8], // Gray
        // Palette 1 - High Intensity
        (0b00, CGAPalette::MagentaCyanWhite, true) => &[0x00u8, 0x00u8, 0x00u8, 0xFFu8], // Black
        (0b01, CGAPalette::MagentaCyanWhite, true) => &[0x55u8, 0xFFu8, 0xFFu8, 0xFFu8], // CyanBright
        (0b10, CGAPalette::MagentaCyanWhite, true) => &[0xFFu8, 0x55u8, 0xFFu8, 0xFFu8], // MagentaBright
        (0b11, CGAPalette::MagentaCyanWhite, true) => &[0xFFu8, 0xFFu8, 0xFFu8, 0xFFu8], // WhiteBright
        // Palette 2 - Low Intensity
        (0b00, CGAPalette::RedCyanWhite, false) => &[0x00u8, 0x00u8, 0x00u8, 0xFFu8], // Black
        (0b01, CGAPalette::RedCyanWhite, false) => &[0x00u8, 0xAAu8, 0xAAu8, 0xFFu8], // Cyan
        (0b10, CGAPalette::RedCyanWhite, false) => &[0xAAu8, 0x00u8, 0x00u8, 0xFFu8], // Red
        (0b11, CGAPalette::RedCyanWhite, false) => &[0xAAu8, 0x55u8, 0x00u8, 0xFFu8], // Gray
        // Palette 2 - High Intensity
        (0b00, CGAPalette::RedCyanWhite, true) => &[0x00u8, 0x00u8, 0x00u8, 0xFFu8], // Black
        (0b01, CGAPalette::RedCyanWhite, true) => &[0x55u8, 0xFFu8, 0xFFu8, 0xFFu8], // CyanBright
        (0b10, CGAPalette::RedCyanWhite, true) => &[0xFFu8, 0x55u8, 0x55u8, 0xFFu8], // RedBright
        (0b11, CGAPalette::RedCyanWhite, true) => &[0xFFu8, 0xFFu8, 0xFFu8, 0xFFu8], // WhiteBright
        _=> &[0x00u8, 0x00u8, 0x00u8, 0xFFu8] // Default Black
    }
}


pub struct Video {
    mode: DisplayMode,
    cols: u32,
    rows: u32
}

impl Video {
    pub fn new() -> Self {
        Self {
            mode: DisplayMode::Mode3TextCo80,
            cols: 80,
            rows: 25
        }
    }

    pub fn draw(&self, frame: &mut [u8], cga: Rc<RefCell<CGACard>>, bus: &BusInterface) {

        let video_mem = bus.get_slice_at(cga::CGA_MEM_ADDRESS, cga::CGA_MEM_SIZE);

        let cga_card = cga.borrow();
        let mode_40_cols = cga_card.is_40_columns();
        if cga_card.is_graphics_mode() {
        
            let (palette, intensity, alt_color) = cga_card.get_palette();
            self.draw_gfx_mode2x(frame, FRAME_W, FRAME_H, video_mem, palette, intensity);
        }
        else {
            self.draw_text_mode(frame, video_mem, mode_40_cols );
        }
    }

    pub fn draw_gfx_mode2x(&self, frame: &mut [u8], frame_w: u32, frame_h: u32, mem: &[u8], pal: CGAPalette, intensity: bool) {
        // First half of graphics memory contains all EVEN rows (0, 2, 4, 6, 8)
        
        let mut field_src_offset = 0;
        let mut field_dst_offset = 0;
        for _field in 0..2 {
            for draw_y in 0..(GFX_H / 2) {

                // CGA gfx mode = 2 bits (4 pixels per byte). Double line count to skip every other line
                let src_y_idx = draw_y * (GFX_W / 4) + field_src_offset; 
                let dst_span = (FRAME_W) * 4;
                let dst1_y_idx = draw_y * (dst_span * 4) + field_dst_offset;  // RBGA = 4 bytes x 2x pixels
                let dst2_y_idx = draw_y * (dst_span * 4) + dst_span + field_dst_offset;  // One scanline down

                // Draw 4 pixels at a time
                for draw_x in 0..(GFX_W / 4) {

                    let dst1_x_idx = (draw_x * 4) * 4 * 2;
                    let dst2_x_idx = dst1_x_idx + 4;

                    let cga_byte: u8 = mem[(src_y_idx + draw_x) as usize];

                    // Four pixels in a byte
                    for pix_n in 0..4 {
                        // Mask the pixel bits, right-to-left
                        let shift_ct = 8 - (pix_n * 2) - 2;
                        let pix_bits = cga_byte >> shift_ct & 0x03;
                        // Get the RGBA for this pixel
                        let color = get_cga_gfx_color(pix_bits, &pal, intensity);
                        // Draw first row of pixel 2x
                        frame[(dst1_y_idx + dst1_x_idx + (pix_n * 8)) as usize]     = color[0];
                        frame[(dst1_y_idx + dst1_x_idx + (pix_n * 8)) as usize + 1] = color[1];
                        frame[(dst1_y_idx + dst1_x_idx + (pix_n * 8)) as usize + 2] = color[2];
                        frame[(dst1_y_idx + dst1_x_idx + (pix_n * 8)) as usize + 3] = color[3];

                        frame[(dst1_y_idx + dst2_x_idx + (pix_n * 8)) as usize]     = color[0];
                        frame[(dst1_y_idx + dst2_x_idx + (pix_n * 8)) as usize + 1] = color[1];
                        frame[(dst1_y_idx + dst2_x_idx + (pix_n * 8)) as usize + 2] = color[2];
                        frame[(dst1_y_idx + dst2_x_idx + (pix_n * 8)) as usize + 3] = color[3];

                        // Draw 2nd row of pixel 2x
                        frame[(dst2_y_idx + dst1_x_idx + (pix_n * 8)) as usize]     = color[0];
                        frame[(dst2_y_idx + dst1_x_idx + (pix_n * 8)) as usize + 1] = color[1];
                        frame[(dst2_y_idx + dst1_x_idx + (pix_n * 8)) as usize + 2] = color[2];
                        frame[(dst2_y_idx + dst1_x_idx + (pix_n * 8)) as usize + 3] = color[3];      

                        frame[(dst2_y_idx + dst2_x_idx + (pix_n * 8)) as usize]     = color[0];
                        frame[(dst2_y_idx + dst2_x_idx + (pix_n * 8)) as usize + 1] = color[1];
                        frame[(dst2_y_idx + dst2_x_idx + (pix_n * 8)) as usize + 2] = color[2];
                        frame[(dst2_y_idx + dst2_x_idx + (pix_n * 8)) as usize + 3] = color[3];                                    
                    }
                }
            }
            field_src_offset += CGA_FIELD_OFFSET;
            field_dst_offset += (FRAME_W) * 4 * 2;
        }
    }

    pub fn draw_text_mode(&self, frame: &mut [u8], mem: &[u8], lowres: bool) {

        let mem_span = match lowres {
            true => 40,
            false => 80
        };

        for (i, char) in mem.chunks_exact(2).enumerate() {
            let x = (i % mem_span as usize) as u32;
            let y = (i / mem_span as usize) as u32;
            
            //println!("x: {} y: {}", x, y);
            //pixel.copy_from_slice(&rgba);
            if y > 24 {
                break;
            }

            let (fg_color, bg_color) = get_colors_from_attr_byte(char[1]);

            match lowres {
                true => draw_glyph4x(char[0], fg_color, bg_color, frame, 640, 400, x * 8, y * 8),
                false => draw_glyph2x(char[0], fg_color, bg_color, frame, 640, 400, x * 8, y * 8)
            }

        }
    }

}

pub fn get_colors_from_attr_byte(byte: u8) -> (CGAColor, CGAColor) {

    let fg_nibble = byte & 0x0F;
    let bg_nibble = (byte >> 4 ) & 0x0F;

    let bg_color = get_colors_from_attr_nibble(bg_nibble);
    let fg_color = get_colors_from_attr_nibble(fg_nibble);

    (fg_color, bg_color)
}

pub fn get_colors_from_attr_nibble(byte: u8) -> CGAColor {

    match byte {
        0b0000 => CGAColor::Black,
        0b0001 => CGAColor::Blue,
        0b0010 => CGAColor::Green,
        0b0100 => CGAColor::Red,
        0b0011 => CGAColor::Cyan,
        0b0101 => CGAColor::Magenta,
        0b0110 => CGAColor::Brown,
        0b0111 => CGAColor::White,
        0b1000 => CGAColor::BlackBright,
        0b1001 => CGAColor::BlueBright,
        0b1010 => CGAColor::GreenBright,
        0b1100 => CGAColor::RedBright,
        0b1011 => CGAColor::CyanBright,
        0b1101 => CGAColor::MagentaBright,
        0b1110 => CGAColor::Yellow,
        0b1111 => CGAColor::WhiteBright,
        _=> CGAColor::Black
    }
}

// Draw a CGA font glyph in 80 column mode at an arbitrary location
pub fn draw_glyph4x( 
    glyph: u8,
    fg_color: CGAColor,
    bg_color: CGAColor,
    frame: &mut [u8], 
    frame_w: u32, 
    frame_h: u32, 
    pos_x: u32, 
    pos_y: u32 ) -> () {

    // Do not draw glyph off screen
    if (pos_x + (FONT_W * 2) > frame_w) || (pos_y + (FONT_H * 2 ) > frame_h) {
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

            let dst_offset = dst_row_offset + ((pos_x * 2) + (draw_glyph_x*2)) * 4;
            frame[dst_offset as usize] = color[0];
            frame[dst_offset as usize + 1] = color[1];
            frame[dst_offset as usize + 2] = color[2];
            frame[dst_offset as usize + 3] = color[3];

            frame[(dst_offset + 4) as usize] = color[0];
            frame[(dst_offset + 4) as usize + 1] = color[1];
            frame[(dst_offset + 4) as usize + 2] = color[2];
            frame[(dst_offset + 4) as usize + 3] = color[3];


            let dst_offset2 = dst_row_offset2 + ((pos_x * 2) + (draw_glyph_x*2)) * 4;
            frame[dst_offset2 as usize] = color[0];
            frame[dst_offset2 as usize + 1] = color[1];
            frame[dst_offset2 as usize + 2] = color[2];
            frame[dst_offset2 as usize + 3] = color[3];   

            frame[(dst_offset2 + 4 ) as usize] = color[0];
            frame[(dst_offset2 + 4) as usize + 1] = color[1];
            frame[(dst_offset2 + 4) as usize + 2] = color[2];
            frame[(dst_offset2 + 4) as usize + 3] = color[3];    
        }
    }     
}

// Draw a CGA font glyph in 80 column mode at an arbitrary location
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
    if pos_y + (FONT_H * 2 ) > frame_h {
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

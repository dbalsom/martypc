use marty_frontend_common::color::cga::CGAColor;

const CGA_FONT: &'static [u8] = include_bytes!("../../../../../assets/cga_8by8.bin");

pub struct FontInfo {
    pub w: u32,
    pub h: u32,
    pub max_scanline: u32,
    pub font_data: Vec<u8>,
}

impl Default for FontInfo {
    fn default() -> Self {
        Self {
            w: 8,
            h: 8,
            max_scanline: 8,
            font_data: CGA_FONT.to_vec(),
        }
    }
}

// Draw a font glyph at an arbitrary location at normal resolution
pub fn draw_glyph(
    glyph: u8,
    fg_color: CGAColor,
    bg_color: CGAColor,
    frame: &mut [u8],
    frame_w: u32,
    frame_span: u32,
    frame_h: u32,
    char_height: u32,
    pos_x: u32,
    pos_y: u32,
    font: &FontInfo,
) {
    // Do not draw glyph off screen
    if pos_x + font.w > frame_w {
        return;
    }
    if pos_y + font.h > frame_h {
        return;
    }

    // Find the source position of the glyph
    //let glyph_offset_src_x = glyph as u32 % FONT_SPAN;
    //let glyph_offset_src_y = (glyph as u32 / FONT_SPAN) * (FONT_H * FONT_SPAN);
    let glyph_offset_src_x = glyph as u32;
    let glyph_offset_src_y = 0;

    let max_char_height = std::cmp::min(font.h, char_height);
    for draw_glyph_y in 0..max_char_height {
        let dst_row_offset = (frame_span * 4) * (pos_y + draw_glyph_y);
        //let glyph_offset = glyph_offset_src_y + (draw_glyph_y * FONT_SPAN) + glyph_offset_src_x;
        let glyph_offset = glyph_offset_src_y + (draw_glyph_y * 256) + glyph_offset_src_x;

        let glyph_byte: u8 = font.font_data[glyph_offset as usize];

        for draw_glyph_x in 0..font.w {
            let test_bit: u8 = 0x80u8 >> draw_glyph_x;

            let color = if test_bit & glyph_byte > 0 {
                fg_color.to_rgba()
            }
            else {
                bg_color.to_rgba()
            };

            let dst_offset = dst_row_offset + (pos_x + draw_glyph_x) * 4;
            frame[dst_offset as usize] = color[0];
            frame[dst_offset as usize + 1] = color[1];
            frame[dst_offset as usize + 2] = color[2];
            frame[dst_offset as usize + 3] = color[3];
        }
    }
}

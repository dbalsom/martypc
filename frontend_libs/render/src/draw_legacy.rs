/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

    render::draw_legacy.rs

    Legacy drawing routines for VideoRenderer. Just keeping them around for 
    reference
*/

impl VideoRenderer {
    pub fn draw(&self, frame: &mut [u8], video_card: Box<&dyn VideoCard>, bus: &BusInterface, composite: bool) {

        //let video_card = video.borrow();        
        let start_address = video_card.get_start_address() as usize;
        let mode_40_cols = video_card.is_40_columns();

        let (frame_w, frame_h) = video_card.get_display_size();

        match video_card.get_display_mode() {
            DisplayMode::Disabled => {
                // Blank screen here?
                return
            }
            DisplayMode::Mode0TextBw40 | DisplayMode::Mode1TextCo40 | DisplayMode::Mode2TextBw80 | DisplayMode::Mode3TextCo80 => {
                let video_type = video_card.get_video_type();
                let cursor = video_card.get_cursor_info();
                let char_height = video_card.get_character_height();
    
                // Start address is multiplied by two due to 2 bytes per character (char + attr)

                let video_mem = match video_type {
                    VideoType::MDA | VideoType::CGA | VideoType::EGA => {
                        bus.get_slice_at(cga::CGA_MEM_ADDRESS + start_address * 2, cga::CGA_MEM_SIZE)
                    }
                    VideoType::VGA => {
                        bus.get_slice_at(cga::CGA_MEM_ADDRESS + start_address * 2, cga::CGA_MEM_SIZE)
                        //video_mem = video_card.get_vram();
                    }
                };
                
                // Get font info from adapter
                let font_info = video_card.get_current_font();

                self.draw_text_mode(
                    video_type, 
                    cursor, 
                    frame, 
                    frame_w, 
                    frame_h, 
                    video_mem, 
                    char_height, 
                    mode_40_cols, 
                    &font_info );
            }
            DisplayMode::Mode4LowResGraphics | DisplayMode::Mode5LowResAltPalette => {
                let (palette, intensity) = video_card.get_cga_palette();

                let video_mem = bus.get_slice_at(cga::CGA_MEM_ADDRESS, cga::CGA_MEM_SIZE);
                if !composite {
                    //draw_cga_gfx_mode2x(frame, frame_w, frame_h, video_mem, palette, intensity);
                    draw_cga_gfx_mode(frame, frame_w, frame_h, video_mem, palette, intensity);
                }
                else {
                    //draw_gfx_mode2x_composite(frame, frame_w, frame_h, video_mem, palette, intensity);
                }
            }
            DisplayMode::Mode6HiResGraphics => {
                let (palette, _intensity) = video_card.get_cga_palette();

                let video_mem = bus.get_slice_at(cga::CGA_MEM_ADDRESS, cga::CGA_MEM_SIZE);
                if !composite {
                    //draw_cga_gfx_mode_highres2x(frame, frame_w, frame_h, video_mem, palette);
                    draw_cga_gfx_mode_highres(frame, frame_w, frame_h, video_mem, palette);
                }
                else {
                    //draw_gfx_mode2x_composite(frame, frame_w, frame_h, video_mem, palette, intensity);
                }
                
            }
            DisplayMode::Mode7LowResComposite => {
                let (palette, _intensity) = video_card.get_cga_palette();

                let video_mem = bus.get_slice_at(cga::CGA_MEM_ADDRESS, cga::CGA_MEM_SIZE);
                if !composite {
                    //draw_cga_gfx_mode_highres2x(frame, frame_w, frame_h, video_mem, palette);
                    draw_cga_gfx_mode_highres(frame, frame_w, frame_h, video_mem, palette);
                }
                else {
                    //draw_gfx_mode2x_composite(frame, frame_w, frame_h, video_mem, palette, intensity);
                }                
            }
            DisplayMode::ModeDEGALowResGraphics => {
                draw_ega_lowres_gfx_mode(video_card, frame, frame_w, frame_h);
            }
            DisplayMode::Mode10EGAHiResGraphics => {
                draw_ega_hires_gfx_mode(video_card, frame, frame_w, frame_h);
            }
            DisplayMode::Mode12VGAHiResGraphics => {
                draw_vga_hires_gfx_mode(video_card, frame, frame_w, frame_h)
            }            
            DisplayMode::Mode13VGALowRes256 => {
                draw_vga_mode13h(video_card, frame, frame_w, frame_h);
            }

            _ => {
                // blank screen here?
            }
        }
    }

    pub fn draw_text_mode(
        &self, 
        video_type: VideoType,
        cursor: CursorInfo, 
        frame: &mut [u8], 
        frame_w: u32, 
        frame_h: u32, 
        mem: &[u8], 
        char_height: u8, 
        lowres: bool,
        font: &FontInfo ) 
    {

        let mem_span = match lowres {
            true => 40,
            false => 80
        };

        // Avoid drawing weird sizes during BIOS setup
        if frame_h < 200 {
            return
        }

        if char_height < 2 {
            return
        }

        let char_height = char_height as u32;

        let max_y = frame_h / char_height - 1;

        for (i, char) in mem.chunks_exact(2).enumerate() {
            let x = (i % mem_span as usize) as u32;
            let y = (i / mem_span as usize) as u32;
            
            //println!("x: {} y: {}", x, y);
            //pixel.copy_from_slice(&rgba);
            if y > max_y {
                break;
            }

            let (fg_color, bg_color) = get_colors_from_attr_byte(char[1]);

            match (video_type, lowres) {
                (VideoType::CGA, true) => {
                    draw_glyph4x(char[0], fg_color, bg_color, frame, frame_w, frame_h, char_height, x * 8, y * char_height, font)
                }
                (VideoType::CGA, false) => {
                    //draw_glyph2x(char[0], fg_color, bg_color, frame, frame_w, frame_h, char_height, x * 8, y * char_height, font)
                    draw_glyph1x1(char[0], fg_color, bg_color, frame, frame_w, frame_h, char_height, x * 8, y * char_height, font)
                }
                (VideoType::EGA, true) => {
                    draw_glyph2x1(
                        char[0], 
                        fg_color, 
                        bg_color, 
                        frame, 
                        frame_w, 
                        frame_h, 
                        char_height, 
                        x * 8 * 2, 
                        y * char_height, 
                        font)
                }
                (VideoType::EGA, false) => {
                    draw_glyph1x1(
                        char[0], 
                        fg_color, 
                        bg_color, 
                        frame, 
                        frame_w, 
                        frame_h, 
                        char_height, 
                        x * 8, 
                        y * char_height, 
                        font)                    
                }
                (VideoType::VGA, false) => {
                    draw_glyph1x1(
                        char[0], 
                        fg_color, 
                        bg_color, 
                        frame, 
                        frame_w, 
                        frame_h, 
                        char_height, 
                        x * 8, 
                        y * char_height, 
                        font)                    
                }
                _=> {}
            }

        }

        match (video_type, lowres) {
            (VideoType::CGA, true) => draw_cursor4x(cursor, frame, frame_w, frame_h, mem, font ),
            (VideoType::CGA, false) => {
                //draw_cursor2x(cursor, frame, frame_w, frame_h, mem, font ),
                draw_cursor(cursor, frame, frame_w, frame_h, mem, font )
            }
            (VideoType::EGA, true) | (VideoType::EGA, false) => {
                draw_cursor(cursor, frame, frame_w, frame_h, mem, font )
            }
            _=> {}
        }
    }    
}

pub fn draw_cga_gfx_mode(frame: &mut [u8], frame_w: u32, _frame_h: u32, mem: &[u8], pal: CGAPalette, intensity: bool) {
    // First half of graphics memory contains all EVEN rows (0, 2, 4, 6, 8)
    let mut field_src_offset = 0;
    let mut field_dst_offset = 0;
    for _field in 0..2 {
        for draw_y in 0..(CGA_GFX_H / 2) {

            // CGA gfx mode = 2 bits (4 pixels per byte). Double line count to skip every other line
            let src_y_idx = draw_y * (CGA_GFX_W / 4) + field_src_offset; 
            let dst_span = frame_w * 4;
            let dst1_y_idx = draw_y * dst_span * 2 + field_dst_offset;  // RBGA = 4 bytes

            // Draw 4 pixels at a time
            for draw_x in 0..(CGA_GFX_W / 4) {

                let dst1_x_idx = (draw_x * 4) * 4;
                //let dst2_x_idx = dst1_x_idx + 4;

                let cga_byte: u8 = mem[(src_y_idx + draw_x) as usize];

                // Four pixels in a byte
                for pix_n in 0..4 {
                    // Mask the pixel bits, right-to-left
                    let shift_ct = 8 - (pix_n * 2) - 2;
                    let pix_bits = cga_byte >> shift_ct & 0x03;
                    // Get the RGBA for this pixel
                    let color = get_cga_gfx_color(pix_bits, &pal, intensity);

                    let draw_offset = (dst1_y_idx + dst1_x_idx + (pix_n * 4)) as usize;
                    if draw_offset + 3 < frame.len() {
                        frame[draw_offset]     = color[0];
                        frame[draw_offset + 1] = color[1];
                        frame[draw_offset + 2] = color[2];
                        frame[draw_offset + 3] = color[3];
                    }                       
                }
            }
        }
        // Switch fields
        field_src_offset += CGA_FIELD_OFFSET;
        field_dst_offset += frame_w * 4;
    }
}

pub fn draw_cga_gfx_mode2x(frame: &mut [u8], frame_w: u32, _frame_h: u32, mem: &[u8], pal: CGAPalette, intensity: bool) {
    // First half of graphics memory contains all EVEN rows (0, 2, 4, 6, 8)

    let mut field_src_offset = 0;
    let mut field_dst_offset = 0;
    for _field in 0..2 {
        for draw_y in 0..(CGA_GFX_H / 2) {

            // CGA gfx mode = 2 bits (4 pixels per byte). Double line count to skip every other line
            let src_y_idx = draw_y * (CGA_GFX_W / 4) + field_src_offset; 
            let dst_span = (frame_w) * 4;
            let dst1_y_idx = draw_y * (dst_span * 4) + field_dst_offset;  // RBGA = 4 bytes x 2x pixels
            let dst2_y_idx = draw_y * (dst_span * 4) + dst_span + field_dst_offset;  // One scanline down

            // Draw 4 pixels at a time
            for draw_x in 0..(CGA_GFX_W / 4) {

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
        field_dst_offset += (frame_w) * 4 * 2;
    }
}

pub fn draw_cga_gfx_mode_highres(frame: &mut [u8], frame_w: u32, _frame_h: u32, mem: &[u8], pal: CGAPalette) {
    // First half of graphics memory contains all EVEN rows (0, 2, 4, 6, 8)

    let mut field_src_offset = 0;
    let mut field_dst_offset = 0;
    for _field in 0..2 {
        for draw_y in 0..(CGA_HIRES_GFX_H / 2) {

            // CGA hi-res gfx mode = 1 bpp (8 pixels per byte).
            let src_y_idx = draw_y * (CGA_HIRES_GFX_W / 8) + field_src_offset; 
            let dst_span = frame_w * 4;
            let dst1_y_idx = draw_y * dst_span * 2 + field_dst_offset;  // RBGA = 4 bytes
            //let dst2_y_idx = draw_y * (dst_span * 4) + dst_span + field_dst_offset;  // One scanline down

            // Draw 8 pixels at a time
            for draw_x in 0..(CGA_HIRES_GFX_W / 8) {

                let dst1_x_idx = (draw_x * 8) * 4;

                let cga_byte: u8 = mem[(src_y_idx + draw_x) as usize];

                // Eight pixels in a byte
                for pix_n in 0..8 {
                    // Mask the pixel bits, right-to-left
                    let shift_ct = 8 - pix_n - 1;
                    let pix_bit = cga_byte >> shift_ct & 0x01;
                    // Get the RGBA for this pixel
                    let color = get_cga_gfx_color(pix_bit, &pal, false);
                    // Draw first row of pixel
                    let draw_offset = (dst1_y_idx + dst1_x_idx + (pix_n * 4)) as usize;
                    if draw_offset + 3 < frame.len() {
                        frame[draw_offset + 0] = color[0];
                        frame[draw_offset + 1] = color[1];
                        frame[draw_offset + 2] = color[2];
                        frame[draw_offset + 3] = color[3];
                    }     
                }
            }
        }
        field_src_offset += CGA_FIELD_OFFSET;
        field_dst_offset += frame_w * 4;
    }
}

pub fn draw_cga_gfx_mode_highres2x(frame: &mut [u8], frame_w: u32, _frame_h: u32, mem: &[u8], pal: CGAPalette) {
    // First half of graphics memory contains all EVEN rows (0, 2, 4, 6, 8)

    let mut field_src_offset = 0;
    let mut field_dst_offset = 0;
    for _field in 0..2 {
        for draw_y in 0..(CGA_HIRES_GFX_H / 2) {

            // CGA hi-res gfx mode = 1 bpp (8 pixels per byte).

            let src_y_idx = draw_y * (CGA_HIRES_GFX_W / 8) + field_src_offset; 

            let dst_span = frame_w * 4;
            let dst1_y_idx = draw_y * (dst_span * 4) + field_dst_offset;  // RBGA = 4 bytes x 2x pixels
            let dst2_y_idx = draw_y * (dst_span * 4) + dst_span + field_dst_offset;  // One scanline down

            // Draw 8 pixels at a time
            for draw_x in 0..(CGA_HIRES_GFX_W / 8) {

                let dst1_x_idx = (draw_x * 8) * 4;

                let cga_byte: u8 = mem[(src_y_idx + draw_x) as usize];

                // Eight pixels in a byte
                for pix_n in 0..8 {
                    // Mask the pixel bits, right-to-left
                    let shift_ct = 8 - pix_n - 1;
                    let pix_bit = cga_byte >> shift_ct & 0x01;
                    // Get the RGBA for this pixel
                    let color = get_cga_gfx_color(pix_bit, &pal, false);
                    // Draw first row of pixel
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 4)) as usize]     = color[0];
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 4)) as usize + 1] = color[1];
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 4)) as usize + 2] = color[2];
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 4)) as usize + 3] = color[3];

                    // Draw 2nd row of pixel
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 4)) as usize]     = color[0];
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 4)) as usize + 1] = color[1];
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 4)) as usize + 2] = color[2];
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 4)) as usize + 3] = color[3];      
                }
            }
        }
        field_src_offset += CGA_FIELD_OFFSET;
        field_dst_offset += (frame_w) * 4 * 2;
    }
}


pub fn draw_gfx_mode2x_composite(
    frame: &mut [u8], 
    frame_w: u32, 
    _frame_h: u32, 
    mem: &[u8], 
    pal: CGAPalette, 
    _intensity: bool
    ) {
    // First half of graphics memory contains all EVEN rows (0, 2, 4, 6, 8)

    let mut field_src_offset = 0;
    let mut field_dst_offset = 0;
    for _field in 0..2 {
        for draw_y in 0..(CGA_GFX_H / 2) {

            // CGA gfx mode = 2 bits (4 pixels per byte). Double line count to skip every other line
            let src_y_idx = draw_y * (CGA_GFX_W / 4) + field_src_offset; 
            let dst_span = (frame_w) * 4;
            let dst1_y_idx = draw_y * (dst_span * 4) + field_dst_offset;  // RBGA = 4 bytes x 2x pixels
            let dst2_y_idx = draw_y * (dst_span * 4) + dst_span + field_dst_offset;  // One scanline down

            // Draw 4 pixels at a time
            for draw_x in 0..(CGA_GFX_W / 4) {

                let dst1_x_idx = (draw_x * 4) * 4 * 2;
                let dst2_x_idx = dst1_x_idx + 4;
                let dst3_x_idx = dst1_x_idx + 8;
                let dst4_x_idx = dst1_x_idx + 12;

                let cga_byte: u8 = mem[(src_y_idx + draw_x) as usize];

                // Two composite 'pixels' in a byte
                for pix_n in 0..2 {
                    // Mask the pixel bits, right-to-left
                    let shift_ct = 8 - (pix_n * 4) - 4;
                    let pix_bits = cga_byte >> shift_ct & 0x0F;
                    // Get the RGBA for this pixel
                    let color = get_cga_composite_color(pix_bits, &pal);
                    // Draw first row of pixel 4x
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 16)) as usize]     = color[0];
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 16)) as usize + 1] = color[1];
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 16)) as usize + 2] = color[2];
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 16)) as usize + 3] = color[3];

                    frame[(dst1_y_idx + dst2_x_idx + (pix_n * 16)) as usize]     = color[0];
                    frame[(dst1_y_idx + dst2_x_idx + (pix_n * 16)) as usize + 1] = color[1];
                    frame[(dst1_y_idx + dst2_x_idx + (pix_n * 16)) as usize + 2] = color[2];
                    frame[(dst1_y_idx + dst2_x_idx + (pix_n * 16)) as usize + 3] = color[3];

                    frame[(dst1_y_idx + dst3_x_idx + (pix_n * 16)) as usize]     = color[0];
                    frame[(dst1_y_idx + dst3_x_idx + (pix_n * 16)) as usize + 1] = color[1];
                    frame[(dst1_y_idx + dst3_x_idx + (pix_n * 16)) as usize + 2] = color[2];
                    frame[(dst1_y_idx + dst3_x_idx + (pix_n * 16)) as usize + 3] = color[3];
                    
                    frame[(dst1_y_idx + dst4_x_idx + (pix_n * 16)) as usize]     = color[0];
                    frame[(dst1_y_idx + dst4_x_idx + (pix_n * 16)) as usize + 1] = color[1];
                    frame[(dst1_y_idx + dst4_x_idx + (pix_n * 16)) as usize + 2] = color[2];
                    frame[(dst1_y_idx + dst4_x_idx + (pix_n * 16)) as usize + 3] = color[3];                    

                    // Draw 2nd row of pixel 4x
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 16)) as usize]     = color[0];
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 16)) as usize + 1] = color[1];
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 16)) as usize + 2] = color[2];
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 16)) as usize + 3] = color[3];      

                    frame[(dst2_y_idx + dst2_x_idx + (pix_n * 16)) as usize]     = color[0];
                    frame[(dst2_y_idx + dst2_x_idx + (pix_n * 16)) as usize + 1] = color[1];
                    frame[(dst2_y_idx + dst2_x_idx + (pix_n * 16)) as usize + 2] = color[2];
                    frame[(dst2_y_idx + dst2_x_idx + (pix_n * 16)) as usize + 3] = color[3];      

                    frame[(dst2_y_idx + dst3_x_idx + (pix_n * 16)) as usize]     = color[0];
                    frame[(dst2_y_idx + dst3_x_idx + (pix_n * 16)) as usize + 1] = color[1];
                    frame[(dst2_y_idx + dst3_x_idx + (pix_n * 16)) as usize + 2] = color[2];
                    frame[(dst2_y_idx + dst3_x_idx + (pix_n * 16)) as usize + 3] = color[3];    

                    frame[(dst2_y_idx + dst4_x_idx + (pix_n * 16)) as usize]     = color[0];
                    frame[(dst2_y_idx + dst4_x_idx + (pix_n * 16)) as usize + 1] = color[1];
                    frame[(dst2_y_idx + dst4_x_idx + (pix_n * 16)) as usize + 2] = color[2];
                    frame[(dst2_y_idx + dst4_x_idx + (pix_n * 16)) as usize + 3] = color[3];    
                }
            }
        }
        field_src_offset += CGA_FIELD_OFFSET;
        field_dst_offset += (frame_w) * 4 * 2;
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

// Draw a CGA font glyph in 40 column mode at an arbitrary location
pub fn draw_glyph4x( 
    glyph: u8,
    fg_color: CGAColor,
    bg_color: CGAColor,
    frame: &mut [u8], 
    frame_w: u32, 
    frame_h: u32, 
    char_height: u32,
    pos_x: u32, 
    pos_y: u32,
    font: &FontInfo )
    {

    // Do not draw glyph off screen
    if (pos_x + (font.w * 2) > frame_w) || (pos_y * 2 + (font.h * 2 ) > frame_h) {
        return
    }

    // Find the source position of the glyph
    //let glyph_offset_src_x = glyph as u32 % FONT_SPAN;
    //let glyph_offset_src_y = (glyph as u32 / FONT_SPAN) * (FONT_H * FONT_SPAN); 
    let glyph_offset_src_x = glyph as u32;
    let glyph_offset_src_y = 0;

    let max_char_height = std::cmp::min(font.h, char_height);
    for draw_glyph_y in 0..max_char_height {

        let dst_row_offset = frame_w * 4 * ((pos_y * 2) + (draw_glyph_y*2));
        let dst_row_offset2 = dst_row_offset + (frame_w * 4);
        
        let glyph_offset = glyph_offset_src_y + (draw_glyph_y * 256) + glyph_offset_src_x;

        let glyph_byte: u8 = font.font_data[glyph_offset as usize];

        for draw_glyph_x in 0..font.w {
        
            let test_bit: u8 = 0x80u8 >> draw_glyph_x;

            let color = if test_bit & glyph_byte > 0 {
                color_enum_to_rgba(&fg_color)
            }
            else {
                color_enum_to_rgba(&bg_color)
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
    char_height: u32,
    pos_x: u32, 
    pos_y: u32,
    font: &FontInfo ) 
    {

    // Do not draw glyph off screen
    if pos_x + font.w > frame_w {
        return
    }
    if pos_y * 2 + (font.h * 2 ) > frame_h {
        return
    }

    // Find the source position of the glyph

    //let glyph_offset_src_x = glyph as u32 % FONT_SPAN;
    //let glyph_offset_src_y = (glyph as u32 / FONT_SPAN) * (FONT_H * FONT_SPAN); 
    let glyph_offset_src_x = glyph as u32;
    let glyph_offset_src_y = 0;

    let max_char_height = std::cmp::min(font.h, char_height);
    for draw_glyph_y in 0..max_char_height {

        let dst_row_offset = frame_w * 4 * ((pos_y * 2) + (draw_glyph_y*2));
        let dst_row_offset2 = dst_row_offset + (frame_w * 4);
        
        let glyph_offset = glyph_offset_src_y + (draw_glyph_y * 256) + glyph_offset_src_x;

        let glyph_byte: u8 = font.font_data[glyph_offset as usize];

        for draw_glyph_x in 0..font.w {
        
            let test_bit: u8 = 0x80u8 >> draw_glyph_x;

            let color = if test_bit & glyph_byte > 0 {
                color_enum_to_rgba(&fg_color)
            }
            else {
                color_enum_to_rgba(&bg_color)
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

pub fn draw_cursor4x(cursor: CursorInfo, frame: &mut [u8], frame_w: u32, frame_h: u32, mem: &[u8], font: &FontInfo ) {
    
    // First off, is cursor even visible?
    if !cursor.visible {
        return
    }

    // Do not draw cursor off screen
    let pos_x = cursor.pos_x * font.w;
    let pos_y = cursor.pos_y * font.h;
    if (pos_x + (font.w * 2) > frame_w) || (pos_y * 2 + (font.h * 2 ) > frame_h) {
        return
    }

    // Cursor start register can be greater than end register, in this case no cursor is shown
    if cursor.line_start > cursor.line_end {
        return
    }

    let line_start = cursor.line_start as u32;
    let mut line_end = cursor.line_end as u32;

    // Clip cursor if at bottom of screen and cursor.line_end > FONT_H
    if pos_y * 2 + line_end * 2 >= frame_h {
        line_end -= frame_h - (pos_y * 2 + line_end * 2) + 1;
    }        

    // Is character attr in mem range?
    let attr_addr = (cursor.addr * 2 + 1) as usize;
    if attr_addr > mem.len() {
        return
    }
    let cursor_attr: u8 = mem[attr_addr];
    let (fg_color, _bg_color) = get_colors_from_attr_byte(cursor_attr);
    let color = color_enum_to_rgba(&fg_color);

    for draw_glyph_y in line_start..line_end {

        let dst_row_offset = frame_w * 4 * ((pos_y * 2) + (draw_glyph_y*2));
        let dst_row_offset2 = dst_row_offset + (frame_w * 4);
        
        for draw_glyph_x in 0..font.w {
        
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

/// Draw the cursor as a character cell into the specified framebuffer with 2x height
pub fn draw_cursor2x(cursor: CursorInfo, frame: &mut [u8], frame_w: u32, frame_h: u32, mem: &[u8] , font: &FontInfo ) {

    // First off, is cursor even visible?
    if !cursor.visible {
        return
    }

    // Do not draw cursor off screen
    let pos_x = cursor.pos_x * font.w;
    let pos_y = cursor.pos_y * font.h;

    let max_pos_x = pos_x + font.w; 
    let max_pos_y = pos_y * 2 + (font.h * 2);  
    if max_pos_x > frame_w || max_pos_y > frame_h {
        return
    }

    // Cursor start register can be greater than end register, in this case no cursor is shown
    if cursor.line_start > cursor.line_end {
        return
    }

    let line_start = cursor.line_start as u32;
    let mut line_end = cursor.line_end as u32;

    // Clip cursor if at bottom of screen and cursor.line_end > FONT_H
    if pos_y * 2 + line_end * 2 >= frame_h {
        line_end -= frame_h - (pos_y * 2 + line_end * 2) + 1;
    }

    // Is character attr in mem range?
    let attr_addr = (cursor.addr * 2 + 1) as usize;
    if attr_addr > mem.len() {
        return
    }
    let cursor_attr: u8 = mem[attr_addr];
    let (fg_color, _bg_color) = get_colors_from_attr_byte(cursor_attr);
    let color = color_enum_to_rgba(&fg_color);

    for draw_glyph_y in line_start..=line_end {

        let dst_row_offset = frame_w * 4 * ((pos_y * 2) + (draw_glyph_y*2));
        let dst_row_offset2 = dst_row_offset + (frame_w * 4);
                                    
        for draw_glyph_x in 0..font.w {
        
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

/// Draw the cursor as a character cell into the specified framebuffer at native height
pub fn draw_cursor(cursor: CursorInfo, frame: &mut [u8], frame_w: u32, frame_h: u32, mem: &[u8] , font: &FontInfo ) {

    // First off, is cursor even visible?
    if !cursor.visible {
        return
    }

    // Do not draw cursor off screen
    let pos_x = cursor.pos_x * font.w;
    let pos_y = cursor.pos_y * font.h;

    let max_pos_x = pos_x + font.w; 
    let max_pos_y = pos_y + font.h;  
    if max_pos_x > frame_w || max_pos_y > frame_h {
        return
    }

    // Cursor start register can be greater than end register, in this case no cursor is shown
    if cursor.line_start > cursor.line_end {
        return
    }

    let line_start = cursor.line_start as u32;
    let mut line_end = cursor.line_end as u32;

    // Clip cursor if at bottom of screen and cursor.line_end > FONT_H
    if pos_y + line_end >= frame_h {
        line_end -= frame_h - (pos_y + line_end) + 1;
    }

    // Is character attr in mem range?
    let attr_addr = (cursor.addr * 2 + 1) as usize;
    if attr_addr > mem.len() {
        return
    }
    let cursor_attr: u8 = mem[attr_addr];
    let (fg_color, _bg_color) = get_colors_from_attr_byte(cursor_attr);
    let color = color_enum_to_rgba(&fg_color);

    for draw_glyph_y in line_start..=line_end {

        let dst_row_offset = frame_w * 4 * (pos_y + draw_glyph_y);
        for draw_glyph_x in 0..font.w {
        
            let dst_offset = dst_row_offset + (pos_x + draw_glyph_x) * 4;
            frame[dst_offset as usize] = color[0];
            frame[dst_offset as usize + 1] = color[1];
            frame[dst_offset as usize + 2] = color[2];
            frame[dst_offset as usize + 3] = color[3];
        }
    }                 
}

// Draw a font glyph at an arbitrary location at 2x horizontal resolution
pub fn draw_glyph2x1( 
    glyph: u8,
    fg_color: CGAColor,
    bg_color: CGAColor,
    frame: &mut [u8], 
    frame_w: u32, 
    frame_h: u32, 
    char_height: u32,
    pos_x: u32, 
    pos_y: u32,
    font: &FontInfo )
    {

    // Do not draw a glyph off screen
    if pos_x + (font.w * 2) > frame_w {
        return
    }
    if pos_y + font.h > frame_h {
        return
    }

    // Find the source position of the glyph
    //let glyph_offset_src_x = glyph as u32 % FONT_SPAN;
    //let glyph_offset_src_y = (glyph as u32 / FONT_SPAN) * (FONT_H * FONT_SPAN); 
    let glyph_offset_src_x = glyph as u32;
    let glyph_offset_src_y = 0;

    let max_char_height = std::cmp::min(font.h, char_height);
    for draw_glyph_y in 0..max_char_height {

        let dst_row_offset = frame_w * 4 * (pos_y + draw_glyph_y);
        //let glyph_offset = glyph_offset_src_y + (draw_glyph_y * FONT_SPAN) + glyph_offset_src_x;
        let glyph_offset = glyph_offset_src_y + (draw_glyph_y * 256) + glyph_offset_src_x;

        let glyph_byte: u8 = font.font_data[glyph_offset as usize];

        for draw_glyph_x in 0..font.w {
        
            let test_bit: u8 = 0x80u8 >> draw_glyph_x;

            let color = if test_bit & glyph_byte > 0 {
                color_enum_to_rgba(&fg_color)
            }
            else {
                color_enum_to_rgba(&bg_color)
            };

            let dst_offset = dst_row_offset + (pos_x + draw_glyph_x * 2) * 4;
            frame[dst_offset as usize + 0] = color[0];
            frame[dst_offset as usize + 1] = color[1];
            frame[dst_offset as usize + 2] = color[2];
            frame[dst_offset as usize + 3] = color[3];

            frame[dst_offset as usize + 4] = color[0];
            frame[dst_offset as usize + 5] = color[1];
            frame[dst_offset as usize + 6] = color[2];
            frame[dst_offset as usize + 7] = color[3];            
        }
    }
}

// Draw a font glyph at an arbitrary location at normal resolution
pub fn draw_glyph1x1( 
    glyph: u8,
    fg_color: CGAColor,
    bg_color: CGAColor,
    frame: &mut [u8], 
    frame_w: u32, 
    frame_h: u32, 
    char_height: u32,
    pos_x: u32, 
    pos_y: u32,
    font: &FontInfo )
    {

    // Do not draw glyph off screen
    if pos_x + font.w > frame_w {
        return
    }
    if pos_y + font.h > frame_h {
        return
    }

    // Find the source position of the glyph
    //let glyph_offset_src_x = glyph as u32 % FONT_SPAN;
    //let glyph_offset_src_y = (glyph as u32 / FONT_SPAN) * (FONT_H * FONT_SPAN); 
    let glyph_offset_src_x = glyph as u32;
    let glyph_offset_src_y = 0;

    let max_char_height = std::cmp::min(font.h, char_height);
    for draw_glyph_y in 0..max_char_height {

        let dst_row_offset = frame_w * 4 * (pos_y + draw_glyph_y);
        //let glyph_offset = glyph_offset_src_y + (draw_glyph_y * FONT_SPAN) + glyph_offset_src_x;
        let glyph_offset = glyph_offset_src_y + (draw_glyph_y * 256) + glyph_offset_src_x;

        let glyph_byte: u8 = font.font_data[glyph_offset as usize];

        for draw_glyph_x in 0..font.w {
        
            let test_bit: u8 = 0x80u8 >> draw_glyph_x;

            let color = if test_bit & glyph_byte > 0 {
                color_enum_to_rgba(&fg_color)
            }
            else {
                color_enum_to_rgba(&bg_color)
            };

            let dst_offset = dst_row_offset + (pos_x + draw_glyph_x) * 4;
            frame[dst_offset as usize] = color[0];
            frame[dst_offset as usize + 1] = color[1];
            frame[dst_offset as usize + 2] = color[2];
            frame[dst_offset as usize + 3] = color[3];
        }
    }
}





pub fn draw_ega_lowres_gfx_mode(ega: Box<&dyn VideoCard>, frame: &mut [u8], frame_w: u32, _frame_h: u32 ) {

    for draw_y in 0..EGA_LORES_GFX_H {

        let dst_span = frame_w * 4;
        let dst1_y_idx = draw_y * dst_span;

        for draw_x in 0..EGA_LORES_GFX_W {

            let dst1_x_idx = draw_x * 4;

            let ega_bits = ega.get_pixel_raw(draw_x, draw_y);
            //if ega_bits != 0 {
            //  log::trace!("ega bits: {:06b}", ega_bits);
            //}
            let color = get_ega_gfx_color16(ega_bits);

            let draw_offset = (dst1_y_idx + dst1_x_idx) as usize;
            if draw_offset + 3 < frame.len() {
                frame[draw_offset + 0] = color[0];
                frame[draw_offset + 1] = color[1];
                frame[draw_offset + 2] = color[2];
                frame[draw_offset + 3] = color[3];
            }
        }
    }
}

pub fn draw_ega_hires_gfx_mode(ega: Box<&dyn VideoCard>, frame: &mut [u8], frame_w: u32, _frame_h: u32 ) {

    for draw_y in 0..EGA_HIRES_GFX_H {

        let dst_span = frame_w * 4;
        let dst1_y_idx = draw_y * dst_span;

        for draw_x in 0..EGA_HIRES_GFX_W {

            let dst1_x_idx = draw_x * 4;

            let ega_bits = ega.get_pixel_raw(draw_x, draw_y);

            // High resolution mode offers the entire 64 color palette
            let color = get_ega_gfx_color64(ega_bits);

            let draw_offset = (dst1_y_idx + dst1_x_idx) as usize;
            if draw_offset + 3 < frame.len() {
                frame[draw_offset + 0] = color[0];
                frame[draw_offset + 1] = color[1];
                frame[draw_offset + 2] = color[2];
                frame[draw_offset + 3] = color[3];
            }
        }
    }
}

pub fn draw_vga_hires_gfx_mode(vga: Box<&dyn VideoCard>, frame: &mut [u8], frame_w: u32, _frame_h: u32 ) {

    for draw_y in 0..VGA_HIRES_GFX_H {

        let dst_span = frame_w * 4;
        let dst1_y_idx = draw_y * dst_span;

        for draw_x in 0..VGA_HIRES_GFX_W {

            let dst1_x_idx = draw_x * 4;

            let rgba = vga.get_pixel(draw_x, draw_y);
            
            let draw_offset = (dst1_y_idx + dst1_x_idx) as usize;
            if draw_offset + 3 < frame.len() {
                frame[draw_offset + 0] = rgba[0];
                frame[draw_offset + 1] = rgba[1];
                frame[draw_offset + 2] = rgba[2];
                frame[draw_offset + 3] = rgba[3];
            }
        }
    }
}


/// Draw Video memory in VGA Mode 13h (320x200@256 colors)
/// 
/// This mode is actually 640x400, double-scanned horizontally and vertically
pub fn draw_vga_mode13h(vga: Box<&dyn VideoCard>, frame: &mut [u8], frame_w: u32, _frame_h: u32 ) {

    for draw_y in 0..VGA_LORES_GFX_H {

        let dst_span = frame_w * 4;
        let dst1_y_idx = draw_y * 2 * dst_span;
        let dst2_y_idx = dst1_y_idx + dst_span;

        for draw_x in 0..VGA_LORES_GFX_W {

            let dst1_x_idx = draw_x * 4 * 2;

            let color = vga.get_pixel(draw_x, draw_y);

            let draw_offset = (dst1_y_idx + dst1_x_idx) as usize;
            let draw_offset2 = (dst2_y_idx + dst1_x_idx) as usize;
            if draw_offset2 + 3 < frame.len() {

                frame[draw_offset + 0] = color[0];
                frame[draw_offset + 1] = color[1];
                frame[draw_offset + 2] = color[2];
                frame[draw_offset + 3] = 0xFF;
                frame[draw_offset + 4] = color[0];
                frame[draw_offset + 5] = color[1];
                frame[draw_offset + 6] = color[2];
                frame[draw_offset + 7] = 0xFF;

                frame[draw_offset2 + 0] = color[0];
                frame[draw_offset2 + 1] = color[1];
                frame[draw_offset2 + 2] = color[2];
                frame[draw_offset2 + 3] = 0xFF;  
                frame[draw_offset2 + 4] = color[0];
                frame[draw_offset2 + 5] = color[1];
                frame[draw_offset2 + 6] = color[2];
                frame[draw_offset2 + 7] = 0xFF;                                 
            }
        }
    }
}
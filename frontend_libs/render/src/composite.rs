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

    render::composite.rs

    This module contains the composite conversion routine. It takes a vector
    of CGA color index values (0-15) and converts to a pseudo-composite signal
    based on the composite generation circuit of an original IBM "old style"
    CGA card. 

    This module includes a basic conversion routine for NTSC artifact color
    from a the composite output of the composite conversion routine. It is not
    a full NTSC simulation.

    See https://github.com/dbalsom/cga_artifact_color for more details on the
    implementation.

*/

//use cgmath::{Matrix3, Vector3};
use glam::{Mat3, Mat3A, Vec3, Vec3A};

// Composite stufff
pub const EDGE_RESPONSE: f32 = 0.80;
pub const INTENSITY_GAIN: f32 = 0.25;
pub const INTENSITY_GAIN_INT: u8 = 64;
pub const LUMA_ATTENUATE: f32 = 0.75;

// Luma contribution of each color for each 1/2 Hdot of a color cycle
pub const COLOR_GEN_HALF_INT: [[u8; 8]; 8] = [
    [  0,   0,   0,   0,   0,   0,   0,   0 ], // Black
    [  0,   0,   0, 255, 255, 255, 255,   0 ], // Blue
    [255, 255,   0,   0,   0,   0, 255, 255 ], // Green
    [255,   0,   0,   0,   0, 255, 255, 255 ], // Cyan
    [  0, 255, 255, 255, 255,   0,   0,   0 ], // Red
    [  0,   0, 255, 255, 255, 255,   0,   0 ], // Magenta
    [255, 255, 255,   0,   0,   0,   0, 255 ], // Yellow
    [255, 255, 255, 255, 255, 255, 255, 255 ], // White    
];

pub const COLOR_GEN_EDGES_HALF: [[bool; 8]; 8] = [
    [false, false, false, false, false, false, false, false ], // Black
    [false, false, false, true,  false, false, true,  false ], // Blue
    [false, true,  false, false, false, false, true,  false ], // Green
    [true , false, false, false, false, true,  false, false ], // Cyan
    [false, true,  false, false, true,  false, false, false ], // Red
    [false, false, true,  false, false, true,  false, false ], // Magenta
    [false, false, true,  false, false, false, false, true  ], // Yellow
    [false, false, false, false, false, false, false, false ], // White    
];

// NTSC stuff
pub const CCYCLE: i32 = 8;
const CCYCLE_HALF: i32 = CCYCLE / 2;

const PI: f32 = 3.1415926;
const TAU: f32 = 6.2831853;

/*
#[rustfmt::skip]
static YIQ2RGB: Matrix3<f32> = Matrix3::new(
    1.000, 1.000, 1.000, 
    0.956, -0.272, -1.106, 
    0.621, -0.647, 1.703,
);
*/
#[rustfmt::skip]
static YIQ2RGB: Mat3 = Mat3::from_cols_array(
    
    &[
        1.000, 1.000, 1.000, 
        0.956, -0.272, -1.106, 
        0.621, -0.647, 1.703,
    ]
);

/// Return the hdot number (0-3) for the given x position.
#[inline]
pub fn get_cycle_hdot(x: i32) -> usize {
    (x % 4).abs() as usize
}

/// Convert a 640 pixel wide, 16 color CGA image into a 1280 pixel wide Composite image.
/// The input image should be a slice of CGA color indices (0-15).
/// The output image should be a slice of u8 values to receive the grayscale composite signal.
/// 
/// Uses integer math.
pub fn process_cga_composite_int(
    cga_buf: &[u8], 
    img_w: u32, 
    img_h: u32, 
    x_offset: u32,
    _y_offset: u32,
    stride: u32, 
    img_out: &mut [u8]
) {

    //bench_t = Instant::now();

    let mut dst_o = 0;

    for y in 0..img_h {
        for x in x_offset..(img_w - x_offset) {
            //get_sample_slice_cga(&cga_buf, img_w, img_h, x, y, &mut sample_slice);
            //let luma = get_cga_luma_avg_from_slice(&sample_slice, x as i32 - (WINDOW_SIZE / 2));

            let mut last_hhdot_value = 0;

            let src_o = (y * stride + x) as usize;
            
            // Convert 0-15 color range to 0-7
            let color = cga_buf[src_o];
            let next_color = if x < (img_w - 1) {
                cga_buf[src_o + 1 as usize] % 8
            }
            else {
                0
            };
            let base_color = color % 8;
            let is_bright = color > 7;

            let hdot = get_cycle_hdot(x as i32);

            for h in 0..2usize {

                let mut attenuate = false;
                
                let mut hhdot_value = COLOR_GEN_HALF_INT[base_color as usize][(hdot * 2 + h) as usize];
                let next_hhdot_value = match h {
                    0 => {
                        COLOR_GEN_HALF_INT[base_color as usize][((hdot * 2 + h) + 1) % 8 as usize ]
                    }
                    _ => {
                        COLOR_GEN_HALF_INT[next_color as usize][((hdot * 2 + h) + 1) % 8 as usize ]   
                    }
                };
                let hhdot_is_edge = COLOR_GEN_EDGES_HALF[base_color as usize][(hdot * 2 + h) as usize];

                if hhdot_value == 255 && last_hhdot_value == 0 {
                    // Signal is rising.
                    if hhdot_is_edge == true {
                        // Signal is rising with rising edge of color clock. Attenuate edge slew.
                        attenuate = true;
                    }
                }
                else if hhdot_value == 255 && next_hhdot_value == 0 {
                    // Signal is falling on next hhdot.
                    if hhdot_is_edge == true {
                        // Signal is falling with falling edge of color clock. Attenuate edge slew.
                        attenuate = true;
                    }
                }

                last_hhdot_value = hhdot_value;

                /*
                if attenuate {
                    hhdot_value = ((hhdot_value as u32 * 768) >> 10) as u8;
                }
                */

                // Integer version of * 0.75
                hhdot_value = ((hhdot_value as u32 * 768) >> 10) as u8;

                if is_bright {
                    hhdot_value += INTENSITY_GAIN_INT;
                }
                
                let dst_o = ((y * img_w * 2) + ((x- x_offset) * 2)) as usize;
                img_out[dst_o + h] =  hhdot_value as u8;
                
            }
            //dst_o += 2;
        }
    }

    //let us = (Instant::now() - bench_t).as_micros();
    //log::debug!("Composite conversion took: {} milliseconds", us as f32 / 1000.0 );
}

pub fn artifact_colors_fast(
    img_in: &[u8],
    img_in_w: u32,
    img_in_h: u32,    
    sync_table: &[(f32, f32, f32)],
    img_out: &mut [u8],
    img_out_w: u32,
    _img_out_h: u32,
    hue: f32,
    sat: f32,
    luma: f32,
) {

    let adjust_mat = make_adjust_mat(hue, sat, luma);

    for y in 0..img_in_h {
        
        let mut dst_o0 = ((y * 2) * (img_out_w * 4)) as usize;
        let mut dst_o1 = dst_o0 + (img_out_w * 4) as usize;

        for x in 0..img_out_w {
            //let mut yiq: Vector3<f32> = Vector3::new(0.0, 0.0, 0.0);  // cgmath
            let mut yiq = Vec3A::new(0.0, 0.0, 0.0);

            for n in -CCYCLE_HALF..CCYCLE_HALF {
                let signal = sample_gy_xy(img_in, img_in_w, img_in_h, (x * 2) as i32 + n, y as i32);

                let sti = ((x * 2) as i32 + n as i32 + CCYCLE_HALF) as usize;
                let signal_i = signal * sync_table[sti].1;
                let signal_q = signal * sync_table[sti].2;

                //log::trace!("Sync: Calc: {},{} Table: {},{}", sync.y, sync.z, sync_table[sti].1, sync_table[sti].2);
                yiq.x += signal;
                yiq.y += signal_i;
                yiq.z += signal_q;
            }
            yiq = yiq / CCYCLE as f32;

            let adjust_yiq = adjust(yiq, adjust_mat);
            let rgb = YIQ2RGB * adjust_yiq;

            img_out[dst_o0 + 0] = to_u8_clamped(rgb.x * 255.0);
            img_out[dst_o0 + 1] = to_u8_clamped(rgb.y * 255.0);
            img_out[dst_o0 + 2] = to_u8_clamped(rgb.z * 255.0);
            img_out[dst_o0 + 3] = 0xFF;

            img_out[dst_o1 + 0] = to_u8_clamped(rgb.x * 255.0);
            img_out[dst_o1 + 1] = to_u8_clamped(rgb.y * 255.0);
            img_out[dst_o1 + 2] = to_u8_clamped(rgb.z * 255.0);
            img_out[dst_o1 + 3] = 0xFF;

            dst_o0 += 4;
            dst_o1 += 4;
        }
    }
}

pub fn artifact_colors_fast_u32(
    img_in: &[u8],
    img_in_w: u32,
    img_in_h: u32,    
    sync_table: &[(f32, f32, f32)],
    img_out: &mut [u8],
    img_out_w: u32,
    _img_out_h: u32,
    hue: f32,
    sat: f32,
    luma: f32,
) {

    let img_out_u32: &mut [u32] = bytemuck::cast_slice_mut(img_out);

    let adjust_mat = make_adjust_mat(hue, sat, luma);

    for y in 0..img_in_h {
        
        let mut dst_o0 = ((y * 2) * img_out_w) as usize;
        let mut dst_o1 = dst_o0 + img_out_w as usize;

        for x in 0..img_out_w {
            //let mut yiq: Vector3<f32> = Vector3::new(0.0, 0.0, 0.0);  // cgmath
            let mut yiq = Vec3A::new(0.0, 0.0,0.0);

            for n in -CCYCLE_HALF..CCYCLE_HALF {
                let signal = sample_gy_xy(img_in, img_in_w, img_in_h, (x * 2) as i32 + n, y as i32);

                let sti = ((x * 2) as i32 + n as i32 + CCYCLE_HALF) as usize;
                let signal_i = signal * sync_table[sti].1;
                let signal_q = signal * sync_table[sti].2;

                //log::trace!("Sync: Calc: {},{} Table: {},{}", sync.y, sync.z, sync_table[sti].1, sync_table[sti].2);
                yiq.x += signal;
                yiq.y += signal_i;
                yiq.z += signal_q;
            }
            yiq = yiq / CCYCLE as f32;

            let adjust_yiq = adjust(yiq, adjust_mat);
            let rgb = YIQ2RGB * adjust_yiq;

            let pixel = to_u32_clamped(rgb.x * 255.0) << 24 | to_u32_clamped(rgb.y * 255.0) << 16 | to_u32_clamped(rgb.x * 255.0) << 8 | 0xFF;

            img_out_u32[dst_o0] = pixel;
            img_out_u32[dst_o1] = pixel;

            dst_o0 += 1;
            dst_o1 += 1;
        }
    }
}

#[inline]
/// Return the grayscale pixel at x, y, clamped at image dimensions
pub fn sample_gy_xy(img_in: &[u8], img_w: u32, img_h: u32, mut x: i32, mut y: i32) -> f32 {
    if x < 0 {
        x = 0;
    }
    if y < 0 {
        y = 0;
    }
    if x >= img_w as i32 {
        x = img_w as i32 - 1;
    }
    if y >= img_h as i32 {
        y = img_h as i32 - 1;
    }

    let io = (y * img_w as i32 + x) as usize;

    img_in[io] as f32 / 255.0
}

/*
// Adjusts a YIQ color by hue, saturation and brightness factors
pub fn adjust(yiq: Vector3<f32>, h: f32, s: f32, b: f32) -> Vector3<f32> {
    #[rustfmt::skip]
    let m: Matrix3<f32> = Matrix3::new(
        b,0.0,0.0,
        0.0,s * h.cos(),-h.sin(),
        0.0,h.sin(),s * h.cos(),
    );

    m * yiq
}
*/

// Adjusts a YIQ color by hue, saturation and brightness factors
#[inline]
pub fn adjust(yiq: Vec3A, m: Mat3A) -> Vec3A {
    m * yiq
}

pub fn make_adjust_mat(h: f32, s: f32, b: f32) -> Mat3A {
    Mat3A::from_cols_array(
        &[ 
            b,0.0,0.0,
            0.0,s * h.cos(), -h.sin(),
            0.0,h.sin(), s * h.cos(),
        ]
    )
}

#[inline]
pub fn to_u8_clamped(f: f32) -> u8 {
    if f >= 255.0 {
        255
    } else if f <= 0.0 {
        0
    } else {
        f as u8
    }
}

#[inline]
pub fn to_u32_clamped(f: f32) -> u32 {
    if f >= 255.0 {
        255
    } else if f <= 0.0 {
        0
    } else {
        f as u32
    }
}

pub fn regen_sync_table(table: &mut [(f32, f32, f32)], table_len: usize) {

    // Precalculate sync
    for x in 0..(table_len as i32 + CCYCLE) {
        let phase: f32 = ((x - CCYCLE_HALF) as f32) * TAU / 8.0;
        table[x as usize] = (phase, phase.cos(), phase.sin());
    }
}
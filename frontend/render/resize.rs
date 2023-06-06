/*
    Marty PC Emulator 
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

    ---------------------------------------------------------------------------

    render::resize.rs

    Framebuffer resizing/resampling routines.

*/

pub struct ResampleParam {
    w: u8,
    iw: u8,
    y_off_low: usize,
    y_off_high: usize,
    dy_offset: usize,
}

pub struct ResampleContext {
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
    params: Vec<ResampleParam>
}

impl ResampleContext {
    pub fn new() -> Self {

        Self {
            src_w: 0,
            src_h: 0,
            dst_w: 0,
            dst_h: 0,
            params: Vec::new()
        }
    }

    pub fn precalc(&mut self, src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) {
        self.src_h = src_h;
        self.dst_h = dst_h;

        self.params.clear();

        let ratio: f64 = (src_h - 1) as f64 / (dst_h - 1) as f64;

        for y in 0..dst_h {
            let low = f64::floor(ratio * y as f64) as u32;
            let high = f64::ceil(ratio * y as f64) as u32;
            let weight: f64 = (ratio * y as f64) - low as f64;
    
            let w = (weight * 255.0) as u8;
            let iw = 255 - w;

            let y_off_low = (low * src_w * 4) as usize;
            let y_off_high = (high * src_w * 4) as usize;
    
            let dy_offset = (y * dst_w * 4) as usize;

            self.params.push(
                ResampleParam {
                    w,
                    iw,
                    y_off_low,
                    y_off_high,
                    dy_offset
                }
            )
        }
    }
}



/// Performs a linear resize of the specified src into dst. 
/// 
/// Since we are only doing this for aspect correction, we don't need a bi-linear filter
pub fn resize_linear(
    src: &[u8], 
    _src_w: u32, 
    src_h: u32, 
    dst: &mut[u8],
    dst_w: u32, 
    dst_h: u32,
    ctx: &ResampleContext) 
{

    assert_eq!(ctx.src_h, src_h);
    assert_eq!(ctx.dst_h, dst_h);
    assert!(dst.len() >= (dst_w * dst_h * 4) as usize);

    for y in 0..(dst_h as usize) {

        for x in (0..(dst_w as usize * 4)).step_by(4) {
            
            let low_off: usize = ctx.params[y].y_off_low + x;
            let high_off: usize = ctx.params[y].y_off_high + x;

            let dyo = ctx.params[y].dy_offset + x;
            dst[dyo + 0] = ((src[low_off + 0] as u32 * ctx.params[y].iw as u32 + src[high_off + 0] as u32 * ctx.params[y].w as u32) >> 8) as u8;
            dst[dyo + 1] = ((src[low_off + 1] as u32 * ctx.params[y].iw as u32 + src[high_off + 1] as u32 * ctx.params[y].w as u32) >> 8) as u8;
            dst[dyo + 2] = ((src[low_off + 2] as u32 * ctx.params[y].iw as u32 + src[high_off + 2] as u32 * ctx.params[y].w as u32) >> 8) as u8;
            // Pre-set alpha in pixel buffer to avoid setting it here
            //dst[dyo + 3] = 255;
        }
    }
}

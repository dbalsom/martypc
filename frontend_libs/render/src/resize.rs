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

    render::resize.rs

    Framebuffer resizing/resampling routines.

*/

use std::num::NonZeroU32;

use fast_image_resize as fr;
use fr::{Resizer, ResizeAlg, FilterType, Image, PixelType};

pub struct ResampleParam {
    w: u8,
    iw: u8,
    y_off_low: usize,
    y_off_high: usize,
    dy_offset: usize,
}
pub struct ResampleContext {
    resizer: Option<Resizer>,
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
    params: Vec<ResampleParam>
}

impl ResampleContext {
    pub fn new() -> Self {

        let resizer = Resizer::new(ResizeAlg::SuperSampling(FilterType::Bilinear, 4));

        /*
        unsafe {
            //resizer.set_cpu_extensions(fr::CpuExtensions::Avx2);
            resizer.set_cpu_extensions(fr::CpuExtensions::Sse4_1);
        }
        */

        Self {
            resizer: Some(resizer),
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

pub fn resize_linear_fast(
    src: &mut [u8], 
    src_w: u32, 
    src_h: u32, 
    dst: &mut[u8],
    dst_w: u32, 
    dst_h: u32,
    ctx: &mut ResampleContext) 
{

    let src_img = Image::from_slice_u8(NonZeroU32::new(src_w).unwrap(), NonZeroU32::new(src_h).unwrap(), src, PixelType::U8x4).unwrap();
    let mut dst_img = Image::from_slice_u8(NonZeroU32::new(dst_w).unwrap(), NonZeroU32::new(dst_h).unwrap(), dst, PixelType::U8x4).unwrap();

    ctx.resizer.as_mut().unwrap().resize(&src_img.view(), &mut dst_img.view_mut()).unwrap();
}
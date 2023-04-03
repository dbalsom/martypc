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

    render/resize.rs

    Framebuffer resizing routines.

*/

/// Performs a linear resize of the specified src into dst. 
/// 
/// Since we are only doing this for aspect correction, we don't need a bi-linear filter
pub fn resize_linear(src: &[u8], src_w: u32, src_h: u32, dst: &mut[u8], dst_w: u32, dst_h: u32) {

    let ratio: f64 = (src_h - 1) as f64 / (dst_h - 1) as f64;

    for y in 0..dst_h {

        let low = f64::floor(ratio * y as f64) as u32;
        let high = f64::ceil(ratio * y as f64) as u32;
        let weight: f64 = (ratio * y as f64) - low as f64;

        let y_off_low = (low * src_w * 4) as usize;
        let y_off_high = (high * src_w * 4) as usize;

        let dy_offset = (y * dst_w * 4) as usize;
        for x in 0..dst_w {
            
            let low_off: usize = y_off_low + (x as usize * 4);
            let high_off: usize = y_off_high + (x as usize * 4);

            let r = (src[low_off+0] as f64 * (1.0 - weight) + src[high_off + 0] as f64 * weight) as u8;
            let g = (src[low_off+1] as f64 * (1.0 - weight) + src[high_off + 1] as f64 * weight) as u8;
            let b = (src[low_off+2] as f64 * (1.0 - weight) + src[high_off + 2] as f64 * weight) as u8;

            dst[dy_offset + x as usize * 4 + 0] = r;
            dst[dy_offset + x as usize * 4 + 1] = g;
            dst[dy_offset + x as usize * 4 + 2] = b;
            dst[dy_offset + x as usize * 4 + 3] = 255;
        }
    }
}

/*
    MartyPC
    https://github.com/dbalsom/martypc

    ---------------------------------------------------------------------------

    This code implements Andrew Jenner (reenigne's) sampled chroma multiplexer
    algorithm. This is now the algorithm used by DosBox, 86Box and MartyPC for
    composite color simulation.

    The code in this file specficially maintains the original UNLICENSE
    license terms.

*/

use super::CompositeParams;

#[rustfmt::skip]
const CHROMA_MULTIPLEXER: [u8; 256] = [
	  2,   2,   2,   2, 114, 174,   4,   3,   2,   1, 133, 135,   2, 113, 150,   4,
	133,   2,   1,  99, 151, 152,   2,   1,   3,   2,  96, 136, 151, 152, 151, 152,
	  2,  56,  62,   4, 111, 250, 118,   4,   0,  51, 207, 137,   1, 171, 209,   5,
	140,  50,  54, 100, 133, 202,  57,   4,   2,  50, 153, 149, 128, 198, 198, 135,
	 32,   1,  36,  81, 147, 158,   1,  42,  33,   1, 210, 254,  34, 109, 169,  77,
	177,   2,   0, 165, 189, 154,   3,  44,  33,   0,  91, 197, 178, 142, 144, 192,
	  4,   2,  61,  67, 117, 151, 112,  83,   4,   0, 249, 255,   3, 107, 249, 117,
	147,   1,  50, 162, 143, 141,  52,  54,   3,   0, 145, 206, 124, 123, 192, 193,
	 72,  78,   2,   0, 159, 208,   4,   0,  53,  58, 164, 159,  37, 159, 171,   1,
	248, 117,   4,  98, 212, 218,   5,   2,  54,  59,  93, 121, 176, 181, 134, 130,
	  1,  61,  31,   0, 160, 255,  34,   1,   1,  58, 197, 166,   0, 177, 194,   2,
	162, 111,  34,  96, 205, 253,  32,   1,   1,  57, 123, 125, 119, 188, 150, 112,
	 78,   4,   0,  75, 166, 180,  20,  38,  78,   1, 143, 246,  42, 113, 156,  37,
	252,   4,   1, 188, 175, 129,   1,  37, 118,   4,  88, 249, 202, 150, 145, 200,
	 61,  59,  60,  60, 228, 252, 117,  77,  60,  58, 248, 251,  81, 212, 254, 107,
	198,  59,  58, 169, 250, 251,  81,  80, 100,  58, 154, 250, 251, 252, 252, 252
];

const INTENSITY: [f64; 4] = [77.175381, 88.654656, 166.564623, 174.228438];

const TAU: f64 = 6.28318531;

const SCALER_MAXWIDTH: usize = 2048;

macro_rules! new_cga {
    ($c:expr, $i:expr, $r:expr, $g:expr, $b:expr) => {
        (($c as f64) / 0.72) * 0.29
            + (($i) / 0.28) * 0.32
            + (($r) / 0.28) * 0.1
            + (($g) / 0.28) * 0.22
            + (($b) / 0.28) * 0.07
    };
}

/*
macro_rules! composite_convert {
    ($self:expr, $buf:expr, $i_index:expr, $ap_index:expr, $bp_index:expr, $i:expr, $q:expr) => {
        {
            let i1 = ($buf.temp[$i_index + 1] << 3) as i32 - $buf.atemp[$ap_index + 1] as i32;

            let a = $buf.atemp[$ap_index];
            let b = $buf.btemp[$bp_index];
            let c = $buf.temp[$i_index] + $buf.temp[$i_index];
            let d = $buf.temp[$i_index - 1] + $buf.temp[$i_index + 1];

            let y = ((c + d) << 8) as i32 + $self.video_sharpness * (c - d) as i32;
            let rr: i32 = y + $self.video_ri as i32 * $i as i32 + $self.video_rq as i32 * $q as i32;
            let gg: i32 = y + $self.video_gi as i32 * $i as i32 + $self.video_gq as i32 * $q as i32;
            let bb: i32 = y + $self.video_bi as i32 * $i as i32 + $self.video_bq as i32 * $q as i32;

            *$self.srgb = (byte_clamp(rr) << 16) | (byte_clamp(gg) << 8) | byte_clamp(bb);
            $self.srgb = &mut $self.srgb[1..];
        }
    };
}
*/

pub struct ReCompositeBuffers {
    temp:  [i32; SCALER_MAXWIDTH + 10],
    atemp: [i32; SCALER_MAXWIDTH + 2],
    btemp: [i32; SCALER_MAXWIDTH + 2],
}

impl ReCompositeBuffers {
    pub fn new() -> Self {
        Self {
            temp:  [0; SCALER_MAXWIDTH + 10],
            atemp: [0; SCALER_MAXWIDTH + 2],
            btemp: [0; SCALER_MAXWIDTH + 2],
        }
    }
}

pub struct ReCompositeContext {
    brightness: f64,
    contrast: f64,
    saturation: f64,
    sharpness: f64,
    hue_offset: f64,
    composite_table: [i32; 1024],

    mode_brightness: f64,
    mode_contrast: f64,
    mode_saturation: f64,
    mode_hue: f64,
    min_v: f64,
    max_v: f64,

    video_ri: i32,
    video_rq: i32,
    video_gi: i32,
    video_gq: i32,
    video_bi: i32,
    video_bq: i32,

    video_sharpness:    i32,
    tandy_mode_control: u32,

    cgamode: u8,
    new_cga: bool,
}

impl ReCompositeContext {
    pub fn new() -> Self {
        Self {
            brightness: 0.0,
            contrast: 100.0,
            saturation: 100.0,
            sharpness: 0.0,
            hue_offset: 0.0,
            composite_table: [0; 1024],

            mode_brightness: 0.0,
            mode_contrast: 0.0,
            mode_saturation: 0.0,
            mode_hue: 0.0,
            min_v: 0.0,
            max_v: 0.0,

            video_ri: 0,
            video_rq: 0,
            video_gi: 0,
            video_gq: 0,
            video_bi: 0,
            video_bq: 0,
            video_sharpness: 0,
            tandy_mode_control: 0,

            cgamode: 0,
            new_cga: false,
        }
    }

    pub fn print(&self) {
        println!(
            "ri: {} rq: {} gi: {} gq: {} bi: {}, bq: {}",
            self.video_ri, self.video_rq, self.video_gi, self.video_gq, self.video_bi, self.video_bq,
        );
    }

    pub fn recalculate(&mut self, cgamode: u8) {
        let mut c: f64;
        let mut i: f64;
        let mut v: f64;
        let q: f64;
        let a: f64;
        let s: f64;
        let r: f64;
        let iq_adjust_i: f64;
        let iq_adjust_q: f64;
        let i0: f64;
        let i3: f64;

        const RI: f64 = 0.9563;
        const RQ: f64 = 0.6210;
        const GI: f64 = -0.2721;
        const GQ: f64 = -0.6474;
        const BI: f64 = -1.1069;
        const BQ: f64 = 1.7046;

        //log::debug!("recalculating composite parameters...");

        if !self.new_cga {
            self.min_v = CHROMA_MULTIPLEXER[0] as f64 + INTENSITY[0];
            self.max_v = CHROMA_MULTIPLEXER[255] as f64 + INTENSITY[3];
        }
        else {
            i0 = INTENSITY[0];
            i3 = INTENSITY[3];
            self.min_v = new_cga!(CHROMA_MULTIPLEXER[0], i0, i0, i0, i0);
            self.max_v = new_cga!(CHROMA_MULTIPLEXER[255], i3, i3, i3, i3);
        }
        self.mode_contrast = 256.0 / (self.max_v - self.min_v);
        self.mode_brightness = -self.min_v * self.mode_contrast;

        if (cgamode & 3) == 1 {
            // 80 column text mode
            self.mode_hue = 14.0;
        }
        else {
            // Every other mode
            self.mode_hue = 4.0;
        }

        self.mode_contrast *= self.contrast * (if self.new_cga { 1.2 } else { 1.0 }) / 100.0; /* new CGA: 120% */
        self.mode_brightness += (if self.new_cga {
            self.brightness - 10.0
        }
        else {
            self.brightness
        }) * 5.0; /* new CGA: -10 */
        self.mode_saturation = (if self.new_cga { 4.35 } else { 2.9 }) * self.saturation / 100.0; /* new CGA: 150% */

        for x in 0..1024 {
            let phase = x & 3;
            let right = (x >> 2) & 15;
            let left = (x >> 6) & 15;
            let mut rc = right;
            let mut lc = left;

            if (cgamode & 4) != 0 {
                // Adjust for B&W mode (no colorburst)
                rc = (right & 8) | (if (right & 7) != 0 { 7 } else { 0 });
                lc = (left & 8) | (if (left & 7) != 0 { 7 } else { 0 });
            }
            c = CHROMA_MULTIPLEXER[((lc & 7) << 5) | ((rc & 7) << 2) | phase] as f64;
            i = INTENSITY[(left >> 3) | ((right >> 2) & 2)];
            if !self.new_cga {
                v = c + i;
            }
            else {
                let r = INTENSITY[((left >> 2) & 1) | ((right >> 1) & 2)];
                let g = INTENSITY[((left >> 1) & 1) | (right & 2)];
                let b = INTENSITY[(left & 1) | ((right << 1) & 2)];
                v = new_cga!(c, i, r, g, b);
            }
            self.composite_table[x as usize] = (v * self.mode_contrast + self.mode_brightness) as i32;
        }

        i = (self.composite_table[6 * 68] - self.composite_table[6 * 68 + 2]) as f64;
        q = (self.composite_table[6 * 68 + 1] - self.composite_table[6 * 68 + 3]) as f64;

        a = TAU * (33.0 + 90.0 + self.hue_offset + self.mode_hue) / 360.0;
        c = a.cos();
        s = a.sin();
        r = 256.0 * self.mode_saturation / (i * i + q * q).sqrt();

        iq_adjust_i = -(i * c + q * s) * r;
        iq_adjust_q = (q * c - i * s) * r;

        self.video_ri = (RI * iq_adjust_i + RQ * iq_adjust_q) as i32;
        self.video_rq = (-RI * iq_adjust_q + RQ * iq_adjust_i) as i32;
        self.video_gi = (GI * iq_adjust_i + GQ * iq_adjust_q) as i32;
        self.video_gq = (-GI * iq_adjust_q + GQ * iq_adjust_i) as i32;
        self.video_bi = (BI * iq_adjust_i + BQ * iq_adjust_q) as i32;
        self.video_bq = (-BI * iq_adjust_q + BQ * iq_adjust_i) as i32;

        self.video_sharpness = (self.sharpness * 256.0 / 100.0) as i32;

        self.cgamode = cgamode;
    }

    /// Set adjustment parameters.
    /// Arguments are scaled as required by algorithm.
    pub fn adjust(&mut self, p: &CompositeParams) {
        self.contrast = p.contrast * 100.0;
        self.hue_offset = p.hue;
        self.saturation = p.sat * 100.0;
        self.brightness = (p.luma - 1.0) * 10.0;

        self.new_cga = p.new_cga;
    }

    pub fn composite_process(
        &mut self,
        border: u8,
        w: usize,
        buffers: &mut ReCompositeBuffers,
        in_line: &[u8],
        out_line: &mut [u32],
    ) {
        let blocks = (w / 4) as usize;

        let mut o_index = 0;
        let mut rgbi_index = 0;
        let b = &self.composite_table[(border as usize) * 68..];

        for x in 0..4 {
            buffers.temp[o_index] = b[((x + 3) & 3) as usize];
            o_index += 1;
        }

        buffers.temp[o_index] = self.composite_table
            [(((border as u32) << 6) | (((in_line[rgbi_index] & 0x0f) as u32) << 2) | 3) as usize]
            as i32;
        o_index += 1;

        for x in 0..w - 1 {
            buffers.temp[o_index] = self.composite_table[(((in_line[rgbi_index] as usize & 0x0f) << 6)
                | ((in_line[rgbi_index + 1] as usize & 0x0f) << 2)
                | (x & 3)) as usize] as i32;
            o_index += 1;
            rgbi_index += 1;
        }

        buffers.temp[o_index] = self.composite_table
            [(((in_line[rgbi_index] as u32 & 0x0f) << 6) | ((border as u32) << 2) | 3) as usize]
            as i32;
        o_index += 1;

        for x in 0..5 {
            buffers.temp[o_index] = b[(x & 3) as usize];
            o_index += 1;
        }

        if (self.cgamode & 4) != 0 {
            // B&W mode (no colorburst)

            let mut i_index = 5;
            let mut srgb_index = 0;
            for _ in 0..blocks * 4 {
                let c = (buffers.temp[i_index] + buffers.temp[i_index]) << 3;
                let d = (buffers.temp[i_index - 1] + buffers.temp[i_index + 1]) << 3;
                let y = ((c + d) << 8) + self.video_sharpness * (c - d);
                i_index += 1;
                out_line[srgb_index] = byte_clamp(y) as u32 * 0x1010101;
                srgb_index += 1;
            }
        }
        else {
            // Do full color multiplexer decoding

            let mut i_index = 4;
            let mut ap_index = 1;
            let mut bp_index = 1;

            for x in 0..(w + 2) {
                buffers.atemp[ap_index + x - 1] = buffers.temp[i_index - 4]
                    - ((buffers.temp[i_index - 2] - buffers.temp[i_index] + buffers.temp[i_index + 2]) << 1)
                    + buffers.temp[i_index + 4];
                buffers.btemp[bp_index + x - 1] = (buffers.temp[i_index - 3] - buffers.temp[i_index - 1]
                    + buffers.temp[i_index + 1]
                    - buffers.temp[i_index + 3])
                    << 1;
                i_index += 1;
            }

            i_index = 5;
            buffers.temp[i_index - 1] = (buffers.temp[i_index - 1] << 3) - buffers.atemp[ap_index - 1];
            buffers.temp[i_index] = (buffers.temp[i_index] << 3) - buffers.atemp[ap_index];
            let mut srgb_index = 0;

            let mut a;
            let mut b;
            let mut c;
            let mut d;
            let mut y;
            let mut rr;
            let mut gg;
            let mut bb;

            for _ in 0..blocks {
                // COMPOSITE_CONVERT(a, b)
                buffers.temp[i_index + 1] = (buffers.temp[i_index + 1] << 3) - buffers.atemp[ap_index + 1];
                a = buffers.atemp[ap_index];
                b = buffers.btemp[bp_index];
                c = buffers.temp[i_index] + buffers.temp[i_index];
                d = buffers.temp[i_index - 1] + buffers.temp[i_index + 1];
                y = ((c + d) << 8) + self.video_sharpness * (c - d);
                rr = y + (self.video_ri * a) + (self.video_rq * b);
                gg = y + (self.video_gi * a) + (self.video_gq * b);
                bb = y + (self.video_bi * a) + (self.video_bq * b);
                i_index += 1;
                ap_index += 1;
                bp_index += 1;

                out_line[srgb_index] = (0xFF << 24 | (byte_clamp(bb) as u32) << 16)
                    | ((byte_clamp(gg) as u32) << 8)
                    | (byte_clamp(rr) as u32);
                srgb_index += 1;

                // COMPOSITE_CONVERT(-b, a)
                buffers.temp[i_index + 1] = (buffers.temp[i_index + 1] << 3) - buffers.atemp[ap_index + 1];
                a = buffers.atemp[ap_index];
                b = buffers.btemp[bp_index];
                c = buffers.temp[i_index] + buffers.temp[i_index];
                d = buffers.temp[i_index - 1] + buffers.temp[i_index + 1];
                y = ((c + d) << 8) + self.video_sharpness * (c - d);
                rr = y + self.video_ri * -b + self.video_rq * a;
                gg = y + self.video_gi * -b + self.video_gq * a;
                bb = y + self.video_bi * -b + self.video_bq * a;
                i_index += 1;
                ap_index += 1;
                bp_index += 1;

                out_line[srgb_index] = (0xFF << 24 | (byte_clamp(bb) as u32) << 16)
                    | ((byte_clamp(gg) as u32) << 8)
                    | (byte_clamp(rr) as u32);
                srgb_index += 1;

                // COMPOSITE_CONVERT(-a, -b)
                buffers.temp[i_index + 1] = (buffers.temp[i_index + 1] << 3) - buffers.atemp[ap_index + 1];
                a = buffers.atemp[ap_index];
                b = buffers.btemp[bp_index];
                c = buffers.temp[i_index] + buffers.temp[i_index];
                d = buffers.temp[i_index - 1] + buffers.temp[i_index + 1];
                y = ((c + d) << 8) + self.video_sharpness * (c - d);
                rr = y + self.video_ri * -a + self.video_rq * -b;
                gg = y + self.video_gi * -a + self.video_gq * -b;
                bb = y + self.video_bi * -a + self.video_bq * -b;
                i_index += 1;
                ap_index += 1;
                bp_index += 1;

                out_line[srgb_index] = (0xFF << 24 | (byte_clamp(bb) as u32) << 16)
                    | ((byte_clamp(gg) as u32) << 8)
                    | (byte_clamp(rr) as u32);
                srgb_index += 1;

                // COMPOSITE_CONVERT(b, -a)
                buffers.temp[i_index + 1] = (buffers.temp[i_index + 1] << 3) - buffers.atemp[ap_index + 1];
                a = buffers.atemp[ap_index];
                b = buffers.btemp[bp_index];
                c = buffers.temp[i_index] + buffers.temp[i_index];
                d = buffers.temp[i_index - 1] + buffers.temp[i_index + 1];
                y = ((c + d) << 8) + self.video_sharpness * (c - d);
                rr = y + self.video_ri * b + self.video_rq * -a;
                gg = y + self.video_gi * b + self.video_gq * -a;
                bb = y + self.video_bi * b + self.video_bq * -a;
                i_index += 1;
                ap_index += 1;
                bp_index += 1;

                out_line[srgb_index] = (0xFF << 24 | (byte_clamp(bb) as u32) << 16)
                    | ((byte_clamp(gg) as u32) << 8)
                    | (byte_clamp(rr) as u32);
                srgb_index += 1;
            }
        }
    }
}

#[inline]
fn byte_clamp(v: i32) -> u8 {
    return (v >> 13).clamp(0, 255) as u8;
}

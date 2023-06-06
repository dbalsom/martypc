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

    benches::render_bench.rs

    Benchmarks for video rendering module.

*/

pub const CGA_FRAME_INDEX_SIZE: usize =  238944;
pub const CGA_FRAME_RGBA_SIZE: usize = CGA_FRAME_INDEX_SIZE * 8;

use rand::Rng;

use marty_render::{
    VideoRenderer, CompositeParams
};

use marty_core::{
    config::VideoType,
    videocard::DisplayExtents
};

use criterion::{black_box, criterion_group, criterion_main, Criterion};

pub fn render_cga_direct_bench(c: &mut Criterion) {
    // One-time setup code goes here

    let mut renderer = VideoRenderer::new(VideoType::CGA);
    let extents = DisplayExtents {
        field_w: 912,
        field_h: 262,
        aperture_w: 768,
        aperture_h: 236,
        aperture_x: 8,
        aperture_y: 0,
        visible_w: 0,
        visible_h: 0,
        overscan_l: 0,
        overscan_r: 0,
        overscan_t: 0,
        overscan_b: 0,
        row_stride: 912      
    };

    let composite_params: CompositeParams = Default::default();

    let mut rng = rand::thread_rng();
    let mut frame_i = Vec::with_capacity(CGA_FRAME_INDEX_SIZE);

    for i in 0..CGA_FRAME_INDEX_SIZE {
        frame_i.push(rng.gen_range(0..16));
    }
    let mut frame_i = std::iter::repeat(0).take(CGA_FRAME_INDEX_SIZE).collect::<Vec<_>>();
    let mut frame_rgb = std::iter::repeat(0).take(CGA_FRAME_RGBA_SIZE).collect::<Vec<_>>();

    c.bench_function("render_cga_direct_bench", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        b.iter(|| {
            // Measured code goes here
            renderer.draw_cga_direct(
                &mut frame_rgb, 
                768, 
                236, 
                &frame_i,
                &extents,
                false,
                &composite_params,
                None
            );
        });
    });
}

criterion_group!(render_benches, render_cga_direct_bench);
criterion_main!(render_benches);

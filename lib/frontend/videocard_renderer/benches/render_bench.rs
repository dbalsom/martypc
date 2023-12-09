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

    benches::render_bench.rs

    Benchmarks for video rendering module.

*/

pub const CGA_FRAME_INDEX_SIZE: usize = 238944;
pub const CGA_FRAME_RGBA_SIZE: usize = CGA_FRAME_INDEX_SIZE * 8;
pub const CGA_FRAME_RGBA_RESIZED: usize = 768 * 576 * 4;

use rand::Rng;

use marty_render::{CompositeParams, VideoRenderer};

use marty_core::videocard::{DisplayExtents, VideoType};

use bytemuck;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

pub fn render_cga_direct_bench(c: &mut Criterion) {
    // One-time setup code goes here

    let mut renderer = VideoRenderer::new(VideoType::CGA);

    let extents = DisplayExtents {
        field_w:    912,
        field_h:    262,
        aperture_w: 768,
        aperture_h: 236,
        aperture_x: 8,
        aperture_y: 0,
        visible_w:  0,
        visible_h:  0,
        overscan_l: 0,
        overscan_r: 0,
        overscan_t: 0,
        overscan_b: 0,
        row_stride: 912,
    };

    let composite_params: CompositeParams = Default::default();

    let mut rng = rand::thread_rng();
    let mut frame_i = Vec::with_capacity(CGA_FRAME_INDEX_SIZE);

    for i in 0..CGA_FRAME_INDEX_SIZE {
        frame_i.push(rng.gen_range(0..16));
    }
    let mut frame_i = std::iter::repeat(0).take(CGA_FRAME_INDEX_SIZE).collect::<Vec<_>>();
    let mut frame_rgb = std::iter::repeat(0).take(CGA_FRAME_RGBA_SIZE).collect::<Vec<_>>();
    let mut frame_resized_rgb = std::iter::repeat(0).take(CGA_FRAME_RGBA_RESIZED).collect::<Vec<_>>();

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
                None,
            );
        });
    });

    c.bench_function("render_cga_direct_u32_bench", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        b.iter(|| {
            // Measured code goes here
            renderer.draw_cga_direct_u32(
                &mut frame_rgb,
                768,
                236,
                &frame_i,
                &extents,
                false,
                &composite_params,
                None,
            );
        });
    });

    c.bench_function("render_cga_direct_composite_bench", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        b.iter(|| {
            // Measured code goes here
            renderer.draw_cga_direct(
                &mut frame_rgb,
                768,
                236,
                &frame_i,
                &extents,
                true,
                &composite_params,
                None,
            );
        });
    });

    c.bench_function("render_cga_direct_composite_u32_bench", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        b.iter(|| {
            // Measured code goes here
            renderer.draw_cga_direct_u32(
                &mut frame_rgb,
                768,
                236,
                &frame_i,
                &extents,
                true,
                &composite_params,
                None,
            );
        });
    });

    c.bench_function("render_resize_linear_bench", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        let mut resample_ctx = marty_render::ResampleContext::new();

        resample_ctx.precalc(768, 472, 768, 576);

        b.iter(|| {
            // Measured code goes here
            marty_render::resize_linear(&frame_rgb, 768, 472, &mut frame_resized_rgb, 768, 576, &resample_ctx);
        });
    });

    c.bench_function("render_resize_linear_fast_bench", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        let mut resample_ctx = marty_render::ResampleContext::new();

        resample_ctx.precalc(768, 472, 768, 576);

        b.iter(|| {
            // Measured code goes here
            marty_render::resize_linear_fast(
                &mut frame_rgb,
                768,
                472,
                &mut frame_resized_rgb,
                768,
                576,
                &mut resample_ctx,
            );
        });
    });

    c.bench_function("render_bytemuck_u8_to_u32_bench", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        b.iter(|| {
            // Measured code goes here
            let frame_u32: &mut [u32] = black_box(bytemuck::cast_slice_mut(&mut frame_i));
        });
    });
}

criterion_group!(render_benches, render_cga_direct_bench);
criterion_main!(render_benches);

/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2025 Daniel Balsom

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

    benches::cga_bench.rs

    Benchmarks for CGA device.

*/

use marty_core::{devices::cga::CGACard, tracelogger::TraceLogger};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use marty_core::videocard::ClockingMode;

pub fn cga_tick_bench(c: &mut Criterion) {
    // One-time setup code goes here

    c.bench_function("cga_bench_tick", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here
        let mut cga = CGACard::new(TraceLogger::None, ClockingMode::Dynamic, false);

        b.iter(|| {
            // Measured code goes here
            cga.tick();
        });
    });

    c.bench_function("cga_bench_tick_char", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        let mut cga = CGACard::new(TraceLogger::None, ClockingMode::Dynamic, false);

        b.iter(|| {
            // Measured code goes here
            cga.tick_char();
        });
    });

    c.bench_function("cga_bench_frame_by_pixel_ticks", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        let mut cga = CGACard::new(TraceLogger::None, ClockingMode::Dynamic, false);

        b.iter(|| {
            // Measured code goes here
            for _ in 0..238944 {
                cga.tick();
            }
        });
    });

    c.bench_function("cga_bench_frame_by_char_ticks", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        let mut cga = CGACard::new(TraceLogger::None, ClockingMode::Dynamic, false);

        b.iter(|| {
            // Measured code goes here
            for _ in 0..29868 {
                cga.tick_char();
            }
        });
    });

    c.bench_function("cga_bench_draw_textmode_char", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        let mut cga = CGACard::new(TraceLogger::None, ClockingMode::Dynamic, false);

        b.iter(|| {
            // Measured code goes here
            cga.draw_text_mode_hchar();
        });
    });
}

criterion_group!(benches, cga_tick_bench);
criterion_main!(benches);

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

    benches::cga_bench.rs

    Benchmarks for CGA device.

*/

use marty_core::{
    devices::cga::CGACard,
    tracelogger::TraceLogger,
};

use criterion::{black_box, criterion_group, criterion_main, Criterion};

pub fn cga_tick_bench(c: &mut Criterion) {
    // One-time setup code goes here

    c.bench_function("cga_bench_tick", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here
        let mut cga = CGACard::new(TraceLogger::None, false);

        b.iter(|| {
            // Measured code goes here
            cga.tick();
        });
    });

    c.bench_function("cga_bench_tick_char", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        let mut cga = CGACard::new(TraceLogger::None, false);

        b.iter(|| {
            // Measured code goes here
            cga.tick_char();
        });
    });    

    c.bench_function("cga_bench_frame_by_pixel_ticks", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        let mut cga = CGACard::new(TraceLogger::None, false);

        b.iter(|| {
            // Measured code goes here
            for _ in 0..238944 {
                cga.tick();
            }
        });
    });      

    c.bench_function("cga_bench_frame_by_char_ticks", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        let mut cga = CGACard::new(TraceLogger::None, false);

        b.iter(|| {
            // Measured code goes here
            for _ in 0..29868 {
                cga.tick_char();
            }
        });
    });
    
    c.bench_function("cga_bench_draw_textmode_char", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        let mut cga = CGACard::new(TraceLogger::None, false);

        b.iter(|| {
            // Measured code goes here
            cga.draw_text_mode_hchar();
        });
    });

}

criterion_group!(benches, cga_tick_bench);
criterion_main!(benches);
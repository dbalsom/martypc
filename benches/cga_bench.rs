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

use martypc::device::cga::CGACard;
use criterion::{black_box, criterion_group, criterion_main, Criterion};



pub fn cga_tick_bench(c: &mut Criterion) {
    // One-time setup code goes here

    let cga = CGACard::new();

    c.bench_function("my_bench", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        b.iter(|| {
            // Measured code goes here
            cga.tick();
        });
    });
}

criterion_group!(benches, cga_tick_bench);
criterion_main!(benches);
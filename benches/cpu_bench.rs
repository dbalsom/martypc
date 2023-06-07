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

    benches::cpu_bench.rs

    Benchmarks for the CPU

*/

use std::{
    io::{BufWriter, Write},
};

use rand::Rng;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use marty_core::{
    cpu_808x::{Cpu},
    cpu_common::CpuType,
    bus::{BusInterface, ClockFactor},
    bytequeue::ByteQueue,
    config::{MachineType, TraceMode},
    machine_manager::{MACHINE_DESCS, MachineDescriptor},
    tracelogger::TraceLogger
};

pub fn cpu_decode_bench<'a>(c: &mut Criterion) {
    let machine_desc = MACHINE_DESCS[&MachineType::IBM_PC_5150];

    //let mut bus = BusInterface::new(ClockFactor::Divisor(3), machine_desc);

    let mut trace_file_option: Box<dyn Write + 'a> = Box::new(std::io::stdout());
    let mut cpu = Cpu::new(CpuType::Intel8088, TraceMode::None, Some(trace_file_option));
    let mut rng = rand::thread_rng();
    cpu.randomize_seed(0);
    cpu.randomize_mem();

    c.bench_function("cpu_decode_bench", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        b.iter(|| {
            // Measured code goes here
            cpu.bus_mut().seek(rng.gen_range(0..0xFFF00));
            Cpu::decode(cpu.bus_mut());
        });

    });

}

criterion_group!(cpu_benches, cpu_decode_bench);
criterion_main!(cpu_benches);

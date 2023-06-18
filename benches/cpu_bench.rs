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
    cpu_808x::{Cpu, Segment, ReadWriteFlag},
    cpu_common::CpuType,
    bytequeue::ByteQueue,
    config::{MachineType, TraceMode},
    machine_manager::{MACHINE_DESCS},
    tracelogger::TraceLogger,
    config::VideoType
};

pub fn cpu_decode_bench<'a>(c: &mut Criterion) {
    let machine_desc = MACHINE_DESCS[&MachineType::IBM_PC_5150];

    //let mut bus = BusInterface::new(ClockFactor::Divisor(3), machine_desc);

    let mut trace_logger = TraceLogger::None;
    let mut cpu = Cpu::new(CpuType::Intel8088, TraceMode::None, trace_logger);

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

pub fn cpu_random_baseline<'a>(c: &mut Criterion) {
    let machine_desc = MACHINE_DESCS[&MachineType::IBM_PC_5150];

    //let mut bus = BusInterface::new(ClockFactor::Divisor(3), machine_desc);

    let mut rng = rand::thread_rng();

    c.bench_function("cpu_random_baseline", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        b.iter(|| {
            // Measured code goes here
            let addr = black_box(rng.gen_range(0..0xFFFF));
        });

    });
}

pub fn cpu_biu_write_bench<'a>(c: &mut Criterion) {
    let machine_desc = MACHINE_DESCS[&MachineType::IBM_PC_5150];

    //let mut bus = BusInterface::new(ClockFactor::Divisor(3), machine_desc);
    
    let mut trace_logger = TraceLogger::None;
    let mut cpu = Cpu::new(CpuType::Intel8088, TraceMode::None, trace_logger);

    let mut rng = rand::thread_rng();
    cpu.randomize_seed(0);
    cpu.randomize_mem();

    c.bench_function("cpu_biu_write_bench", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        b.iter(|| {
            // Measured code goes here

            let addr = rng.gen_range(0..0xFFFF);
            cpu.biu_write_u8(Segment::CS, addr << 4, 0, ReadWriteFlag::Normal);
        });

    });
}

pub fn cpu_bus_write_bench<'a>(c: &mut Criterion) {
    let machine_desc = MACHINE_DESCS[&MachineType::IBM_PC_5150];

    //let mut bus = BusInterface::new(ClockFactor::Divisor(3), machine_desc);

    let mut trace_logger = TraceLogger::None;
    let mut cpu = Cpu::new(CpuType::Intel8088, TraceMode::None, trace_logger);

    let machine_desc = MACHINE_DESCS[&MachineType::IBM_XT_5160];

    // Install devices
    cpu.bus_mut().install_devices(
        VideoType::CGA, 
        &machine_desc, 
        TraceLogger::None, 
        false
    );

    let mut rng = rand::thread_rng();
    cpu.randomize_seed(0);
    cpu.randomize_mem();

    c.bench_function("cpu_bus_write_bench", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        b.iter(|| {
            // Measured code goes here

            let addr = rng.gen_range(0..0xFFFF);
            _ = cpu.bus_mut().write_u8(addr as usize, 0xFF, 0).unwrap();
        });

    });
}

pub fn cpu_bus_read_cga_bench<'a>(c: &mut Criterion) {
    let machine_desc = MACHINE_DESCS[&MachineType::IBM_PC_5150];

    //let mut bus = BusInterface::new(ClockFactor::Divisor(3), machine_desc);

    let mut trace_logger = TraceLogger::None;
    let mut cpu = Cpu::new(CpuType::Intel8088, TraceMode::None, trace_logger);

    let machine_desc = MACHINE_DESCS[&MachineType::IBM_XT_5160];

    // Install devices
    cpu.bus_mut().install_devices(
        VideoType::CGA, 
        &machine_desc, 
        TraceLogger::None, 
        false
    );


    let mut rng = rand::thread_rng();
    cpu.randomize_seed(0);
    cpu.randomize_mem();

    c.bench_function("cpu_bus_read_cga_bench", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        b.iter(|| {
            // Measured code goes here

            // CGA memory range to target MMIO.
            let addr = rng.gen_range(0xB8000..0xBC000);
            _ = cpu.bus_mut().read_u8(addr as usize, 0).unwrap();
        });

    });
}

pub fn cpu_bus_write_cga_bench<'a>(c: &mut Criterion) {
    let machine_desc = MACHINE_DESCS[&MachineType::IBM_PC_5150];

    //let mut bus = BusInterface::new(ClockFactor::Divisor(3), machine_desc);

    let mut trace_logger = TraceLogger::None;
    let mut cpu = Cpu::new(CpuType::Intel8088, TraceMode::None, trace_logger);

    let machine_desc = MACHINE_DESCS[&MachineType::IBM_XT_5160];

    // Install devices
    cpu.bus_mut().install_devices(
        VideoType::CGA, 
        &machine_desc, 
        TraceLogger::None, 
        false
    );


    let mut rng = rand::thread_rng();
    cpu.randomize_seed(0);
    cpu.randomize_mem();

    c.bench_function("cpu_bus_write_cga_bench", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        b.iter(|| {
            // Measured code goes here

            // CGA memory range to target MMIO.
            let addr = rng.gen_range(0xB8000..0xBC000);
            _ = cpu.bus_mut().write_u8(addr as usize, 0xFF, 0).unwrap();
        });

    });
}


/*
criterion_group!(
    cpu_benches, 
    cpu_decode_bench, 
    cpu_random_baseline, 
    cpu_biu_write_bench,
    cpu_bus_write_bench,
    cpu_bus_write_cga_bench
);
*/
criterion_group!(
    cpu_benches,
    cpu_bus_write_bench,
    cpu_bus_read_cga_bench,
    cpu_bus_write_cga_bench,
    
);

criterion_main!(cpu_benches);

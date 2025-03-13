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

    benches::cpu_bench.rs

    Benchmarks for the CPU

*/

use std::io::{BufWriter, Write};

use rand::Rng;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use marty_core::{
    bytequeue::ByteQueue,
    cpu_808x::{Cpu, ReadWriteFlag},
    cpu_common::{builder::CpuBuilder, CpuType, Segment},
    device_traits::videocard::{ClockingMode, VideoType},
    machine_config::{MachineConfiguration, MACHINE_DESCS},
    machine_types::MachineType,
    tracelogger::TraceLogger,
};

pub fn cpu_decode_bench<'a>(c: &mut Criterion) {
    let machine_desc = MACHINE_DESCS[&MachineType::Ibm5150v256K];

    //let mut bus = BusInterface::new(ClockFactor::Divisor(3), machine_desc);

    let mut trace_logger = TraceLogger::None;
    let cpu_type = CpuType::Intel8088;
    let mut cpu = match CpuBuilder::new().with_cpu_type(cpu_type).build() {
        Ok(cpu) => cpu,
        Err(e) => panic!("Failed to create CPU: {}", e),
    };

    let mut rng = rand::thread_rng();
    cpu.randomize_seed(0);
    cpu.randomize_mem();

    c.bench_function("cpu_decode_bench", |b| {
        // Per-sample (note that a sample can be many iterations) setup goes here

        b.iter(|| {
            // Measured code goes here
            cpu.bus_mut().seek(rng.gen_range(0..0xFFF00));
            _ = cpu_type.decode(cpu.bus_mut(), false);
        });
    });
}

pub fn cpu_random_baseline<'a>(c: &mut Criterion) {
    let machine_desc = MACHINE_DESCS[&MachineType::Ibm5150v256K];

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

// pub fn cpu_biu_write_bench<'a>(c: &mut Criterion) {
//     let machine_desc = MACHINE_DESCS[&MachineType::Ibm5150v256K];
//
//     //let mut bus = BusInterface::new(ClockFactor::Divisor(3), machine_desc);
//
//     let mut trace_logger = TraceLogger::None;
//     let mut cpu = match CpuBuilder::new().with_cpu_type(CpuType::Intel8088).build() {
//         Ok(cpu) => cpu,
//         Err(e) => panic!("Failed to create CPU: {}", e),
//     };
//
//     let mut rng = rand::thread_rng();
//     cpu.randomize_seed(0);
//     cpu.randomize_mem();
//
//     c.bench_function("cpu_biu_write_bench", |b| {
//         // Per-sample (note that a sample can be many iterations) setup goes here
//
//         b.iter(|| {
//             // Measured code goes here
//
//             let addr = rng.gen_range(0..0xFFFF);
//             cpu.biu_write_u8(Segment::CS, addr << 4, 0, ReadWriteFlag::Normal);
//         });
//     });
// }

pub fn cpu_bus_write_bench<'a>(c: &mut Criterion) {
    let machine_desc = MACHINE_DESCS[&MachineType::Ibm5150v256K];
    let machine_config = MachineConfiguration::default();
    //let mut bus = BusInterface::new(ClockFactor::Divisor(3), machine_desc);

    let mut trace_logger = TraceLogger::None;
    let mut cpu = match CpuBuilder::new().with_cpu_type(CpuType::Intel8088).build() {
        Ok(cpu) => cpu,
        Err(e) => panic!("Failed to create CPU: {}", e),
    };

    // Install devices
    let _ = cpu.bus_mut().install_devices(&machine_desc, &machine_config, None);

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
    let machine_desc = MACHINE_DESCS[&MachineType::Ibm5150v256K];
    let machine_config = MachineConfiguration::default();
    //let mut bus = BusInterface::new(ClockFactor::Divisor(3), machine_desc);

    let mut trace_logger = TraceLogger::None;
    let mut cpu = match CpuBuilder::new().with_cpu_type(CpuType::Intel8088).build() {
        Ok(cpu) => cpu,
        Err(e) => panic!("Failed to create CPU: {}", e),
    };

    let machine_desc = MACHINE_DESCS[&MachineType::Ibm5150v256K];

    // Install devices
    let _ = cpu.bus_mut().install_devices(&machine_desc, &machine_config, None);

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
    let machine_desc = MACHINE_DESCS[&MachineType::Ibm5150v256K];
    let machine_config = MachineConfiguration::default();

    let mut cpu = match CpuBuilder::new().with_cpu_type(CpuType::Intel8088).build() {
        Ok(cpu) => cpu,
        Err(e) => panic!("Failed to create CPU: {}", e),
    };

    // Install devices
    let _ = cpu.bus_mut().install_devices(&machine_desc, &machine_config, None);

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

criterion_group!(
    cpu_benches,
    cpu_decode_bench,
    cpu_random_baseline,
    cpu_bus_write_bench,
    cpu_bus_read_cga_bench,
    cpu_bus_write_cga_bench,
);

criterion_main!(cpu_benches);

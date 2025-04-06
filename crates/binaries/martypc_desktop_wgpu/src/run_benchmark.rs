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

    run_benchmark.rs - Implement the main procedure for benchmark mode.

*/

use std::{cell::RefCell, rc::Rc, time::Instant};

use marty_config::ConfigFileParams;
use marty_core::{
    bus::ClockFactor,
    cpu_common::Cpu,
    machine::{ExecutionControl, ExecutionState, MachineBuilder, MachineRomManifest},
};
use marty_frontend_common::{
    floppy_manager::FloppyManager,
    machine_manager::MachineConfigFileEntry,
    resource_manager::ResourceManager,
    rom_manager::RomManager,
    BenchmarkEndCondition,
};

const BENCHMARK_CYCLE_BATCH: u64 = 100_000;

pub fn run_benchmark(
    config: &ConfigFileParams,
    machine_config_file: &MachineConfigFileEntry,
    rom_manifest: MachineRomManifest,
    _rm: ResourceManager,
    _romm: RomManager,
    _fm: FloppyManager,
) {
    let machine_config = machine_config_file.to_machine_config();

    let machine_builder = MachineBuilder::new()
        .with_core_config(Box::new(config))
        .with_machine_config(&machine_config)
        .with_roms(rom_manifest)
        .with_trace_mode(config.machine.cpu.trace_mode.unwrap_or_default())
        .with_sound_override(false);

    let mut machine = machine_builder.build().unwrap_or_else(|e| {
        log::error!("Failed to build machine: {:?}", e);
        std::process::exit(1);
    });

    let exec_control = Rc::new(RefCell::new(ExecutionControl::new()));
    exec_control.borrow_mut().set_state(ExecutionState::Running);

    let cycle_total;
    match config.emulator.benchmark.end_condition {
        BenchmarkEndCondition::Cycles => {
            cycle_total = config.emulator.benchmark.cycles.unwrap_or(10_000_000);
            println!("Running benchmark for {} cycles", cycle_total);
        }
        BenchmarkEndCondition::Timeout => {
            // Calculate number of cycles to run based on timeout
            let timeout_secs = config.emulator.benchmark.timeout.unwrap_or(30);
            cycle_total = (machine.get_cpu_mhz() * 1_000_000.0 * timeout_secs as f64) as u64;
            println!(
                "Running benchmark for {} virtual seconds; {} cycles",
                timeout_secs, cycle_total
            );
        }
        BenchmarkEndCondition::Trigger => {
            log::error!("Benchmark 'Trigger' end condition not implemented.");
            std::process::exit(1);
        }
    }

    let mut cycles_left = cycle_total;

    let benchmark_start = Instant::now();
    while cycles_left > 0 {
        let cycle_batch = std::cmp::min(cycles_left, BENCHMARK_CYCLE_BATCH);
        machine.run(cycle_batch as u32, &mut exec_control.borrow_mut());
        cycles_left = cycles_left.saturating_sub(BENCHMARK_CYCLE_BATCH);

        if let ExecutionState::Halted = exec_control.borrow().get_state() {
            eprintln!("Machine halted during benchmark!");
            std::process::exit(1);
        }
    }
    let benchmark_duration = benchmark_start.elapsed();
    let instruction_ct = machine.cpu_instructions();
    let (cycle_total, halt_cycles) = machine.cpu().get_cycle_ct();

    let cpu_factor = machine.get_cpu_factor();
    let sys_ticks = match cpu_factor {
        ClockFactor::Divisor(d) => cycle_total * d as u64,
        ClockFactor::Multiplier(m) => cycle_total / m as u64,
    };

    println!(
        "Benchmark complete.\nRan {} cycles and {} instructions in {:?} seconds.",
        cycle_total,
        instruction_ct,
        benchmark_duration.as_secs_f64()
    );

    println!(
        "Cycles spent in halt state: {} ({:.4}%)",
        halt_cycles,
        (halt_cycles as f64 / cycle_total as f64) * 100.0
    );

    let effective_cycles = cycle_total - halt_cycles;

    println!(
        "Cycles per instruction: {:.4}",
        effective_cycles as f64 / instruction_ct as f64
    );

    println!(
        "Effective Bus speed: {:.4} MHz",
        (sys_ticks as f64 / benchmark_duration.as_secs_f64()) / 1_000_000.0
    );

    println!(
        "Effective CPU speed: {:.4} MHz",
        (effective_cycles as f64 / benchmark_duration.as_secs_f64()) / 1_000_000.0
    );

    println!(
        "MIPS: {:.4}",
        instruction_ct as f64 / benchmark_duration.as_secs_f64() / 1_000_000.0
    );
}

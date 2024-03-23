/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2024 Daniel Balsom

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

use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use config_toml_bpaf::ConfigFileParams;
use frontend_common::{
    floppy_manager::FloppyManager,
    machine_manager::MachineConfigFileEntry,
    resource_manager::ResourceManager,
    rom_manager::RomManager,
    BenchmarkEndCondition,
};
use marty_core::bus::ClockFactor;

use marty_core::machine::{ExecutionControl, ExecutionState, MachineBuilder, MachineRomManifest};

const BENCHMARK_CYCLE_BATCH: u64 = 100_000;

pub fn run_benchmark(
    config: &ConfigFileParams,
    machine_config_file: &MachineConfigFileEntry,
    rom_manifest: MachineRomManifest,
    rm: ResourceManager,
    romm: RomManager,
    fm: FloppyManager,
) {
    let machine_config = machine_config_file.to_machine_config();

    let machine_builder = MachineBuilder::new()
        .with_core_config(Box::new(config))
        .with_machine_config(&machine_config)
        .with_roms(rom_manifest)
        .with_trace_mode(config.machine.cpu.trace_mode.unwrap_or_default());

    let mut machine = machine_builder.build().unwrap_or_else(|e| {
        log::error!("Failed to build machine: {:?}", e);
        std::process::exit(1);
    });

    let exec_control = Rc::new(RefCell::new(ExecutionControl::new()));
    exec_control.borrow_mut().set_state(ExecutionState::Running);

    let mut cycle_total = 0;
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
    }
    let benchmark_duration = benchmark_start.elapsed();

    let instruction_ct = machine.cpu_instructions();

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
        "Effective Bus speed: {:.4} MHz",
        (sys_ticks as f64 / benchmark_duration.as_secs_f64()) / 1_000_000.0
    );
    println!(
        "Effective CPU speed: {:.4} MHz",
        (cycle_total as f64 / benchmark_duration.as_secs_f64()) / 1_000_000.0
    );
    println!(
        "MIPS: {:.4}",
        instruction_ct as f64 / benchmark_duration.as_secs_f64() / 1_000_000.0
    );
}

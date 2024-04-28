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

    run_runtests.rs - Implement the main procedure for JSON test running mode.

*/

use crate::cpu_test::common::{
    clean_cycle_states,
    is_prefix_in_vec,
    opcode_extension_from_path,
    opcode_from_path,
    print_cycle_diff,
    print_summary,
    read_tests_from_file,
    validate_cycles,
    validate_memory,
    validate_registers,
};
use std::{
    collections::{HashMap, LinkedList},
    fs::{copy, create_dir, read_dir, File},
    io::{BufWriter, ErrorKind, Read, Write},
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use config_toml_bpaf::{ConfigFileParams, TestMode};

use marty_core::{
    bytequeue::ByteQueue,
    cpu_808x::{mnemonic::Mnemonic, Cpu, *},
    cpu_common::{CpuOption, CpuType},
    cpu_validator::ValidatorType,
    tracelogger::TraceLogger,
};

use crate::{
    cpu_test::common::{CpuTest, FailType, Metadata, TestFailItem, TestResult},
    trace_error,
    trace_print,
};

use crate::cpu_test::common::{TestFileLoad, TestResultSummary};
use colored::*;
use flate2::read::GzDecoder;

pub fn run_runtests(config: ConfigFileParams) {
    let mut test_path = "./tests".to_string();
    if let Some(test_dir) = &config.tests.test_dir {
        test_path = test_dir.clone();
    }

    let mut test_base_path = PathBuf::new();
    //test_base_path.push(config.emulator.basedir.clone());

    // Interpret as absolute path
    test_base_path.push(test_path);

    log::debug!("Using test path: {:?}", test_base_path);

    // Load metadata file.

    let mut metadata_path = test_base_path.clone();
    metadata_path.push("8088.json");
    let mut metadata_file = File::open(metadata_path.clone()).expect(&format!(
        "Couldn't open metadata file 8088.json at path: {:?}",
        metadata_path
    ));
    let mut contents = String::new();
    metadata_file
        .read_to_string(&mut contents)
        .expect("Failed to read metadata file.");

    let metadata: Metadata = serde_json::from_str(&contents).expect("Failed to parse metadata JSON");

    // Create 'validated' folder to receive validated tests, if in validate mode

    let mut validated_dir_path = test_base_path.clone();
    validated_dir_path.push("validated");

    if let Some(TestMode::Validate) = &config.tests.test_mode {
        match create_dir(validated_dir_path.clone()) {
            Ok(_) => {
                log::debug!("Created output path for validated tests.")
            }
            Err(e) => match e.kind() {
                ErrorKind::AlreadyExists => {
                    log::debug!("Output path already exists.")
                }
                _ => {
                    log::error!("Failed to create output directory: {:?}", e);
                    panic!("Failed to create output directory!");
                }
            },
        }
        log::debug!("Using validated dir path: {:?}", validated_dir_path);
    }

    // Convert opcode ranges into strings
    let default_range = [0, 0xFF].to_vec();
    let op_range_start = config.tests.test_opcode_range.as_ref().unwrap_or(&default_range)[0];
    let op_range_end = config.tests.test_opcode_range.as_ref().unwrap_or(&default_range)[1];

    let str_vec: Vec<String> = (op_range_start..=op_range_end)
        .map(|num| format!("{:02X}", num))
        .collect();

    log::debug!("Validating opcode list: {:?}", str_vec);

    let mut log_path = test_base_path.clone();
    if let Some(ref test_output_path) = config.tests.test_output_dir {
        log_path = PathBuf::from(test_output_path.clone());
    }
    log_path.push("validation.log");

    let mut summary = TestResultSummary {
        results: HashMap::new(),
    };

    let test_suite_start = Instant::now();

    let (tx, rx) = mpsc::sync_channel::<TestFileLoad>(1);

    thread::spawn(move || {
        match read_dir(test_base_path) {
            Ok(entries) => {
                for entry in entries {
                    if let Ok(entry) = entry {
                        // Filter for JSON files
                        if let Some(extension) = entry.path().extension() {
                            if ((extension.to_ascii_lowercase() == "json") || (extension.to_ascii_lowercase() == "gz"))
                                && is_prefix_in_vec(&entry.path(), &str_vec)
                            {
                                // Load the JSON file
                                match read_tests_from_file(entry.path()) {
                                    Some(tests) => {
                                        // Send tests through channel.

                                        _ = tx.send(TestFileLoad {
                                            path: entry.path().clone(),
                                            tests,
                                        });
                                    }
                                    None => {
                                        eprintln!("Failed to parse json from file: {:?}. Skipping...", entry.path());

                                        //let mut writer_lock = thread_logger.lock().unwrap();
                                        //_ = writeln!(&mut writer_lock, "Failed to parse json from file: {:?}. Skipping...", entry.path());
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading directory: {}", e);
                return;
            }
        }
    });

    let log = File::create(log_path).expect("Couldn't open logfile.");
    let mut log_writer = BufWriter::new(log);

    // Read in tests from channel as they are loaded and process them.

    let stop_on_failure = matches!(&config.tests.test_mode, Some(TestMode::Validate));

    while let Ok(test_load) = rx.recv() {
        //_ = writeln!(&mut writer_lock, "Running tests from file: {:?}", test_load.path);

        println!(
            "Received file {:?} with {} tests from loading thread.",
            test_load.path,
            test_load.tests.len()
        );

        let opcode =
            opcode_from_path(&test_load.path).expect(&format!("Couldn't parse opcode from path: {:?}", test_load.path));
        let extension_opt = opcode_extension_from_path(&test_load.path);

        if let (Some(opt), Some(range)) = (extension_opt, config.tests.test_extension_range.clone()) {
            if range.len() == 2 {
                if (range[0]..=range[1]).contains(&opt) {
                    log::debug!("Extension {} is in range.", opt);
                }
                else {
                    log::debug!("Extension {} is not in range. Skipping...", opt);
                    continue;
                }
            }
        }

        //let results = run_tests(&metadata, &test_load.tests, opcode, extension_opt, &config, &mut writer_lock);
        let results = run_tests(
            &metadata,
            &test_load.tests,
            opcode,
            extension_opt,
            &config,
            stop_on_failure,
            &mut log_writer,
        );

        println!(
            "Test file completed. {}/{} tests passed in {:.2} seconds.",
            results.passed,
            test_load.tests.len(),
            results.duration.as_secs_f32()
        );

        if results.failed > 0 && stop_on_failure {
            break;
        }

        //_ = writer_lock.flush();

        // If we passed all tests, and are in Validate mode, copy the passing file to the validated directory.
        if results.failed == 0 && matches!(&config.tests.test_mode, Some(TestMode::Validate)) {
            // Copy test file to validated directory.
            let mut copy_output_path = validated_dir_path.clone();

            copy_output_path.push(test_load.path.file_name().unwrap());

            log::debug!(
                "Using output path: {:?} from dir path: {:?}",
                copy_output_path,
                validated_dir_path
            );

            copy(test_load.path.clone(), copy_output_path.clone()).expect(&format!(
                "Failed to copy file {:?} to output dir: {:?}!",
                test_load.path, copy_output_path
            ));
        }

        summary
            .results
            .insert(test_load.path.file_name().unwrap().to_os_string(), results);
    }

    if matches!(&config.tests.test_mode, Some(TestMode::Validate)) {
        println!("All tests validated!");

        print_summary(&summary);
        println!("Completed in: {} seconds", test_suite_start.elapsed().as_secs());
    }
    else {
        println!("All tests run!");

        print_summary(&summary);
        println!("Completed in: {} seconds", test_suite_start.elapsed().as_secs());
    }

    //let mut writer_lock = writer_arc.lock().unwrap();
    //_ = writeln!(&mut writer_lock, "All tests validated!");
    //_ = writer_lock.flush();

    // writer & file dropped here
}

fn run_tests(
    metadata: &Metadata,
    tests: &LinkedList<CpuTest>,
    opcode: u8,
    extension_opt: Option<u8>,
    config: &ConfigFileParams,
    stop_on_failure: bool,
    log: &mut BufWriter<File>,
) -> TestResult {
    // Create the cpu trace file, if specified
    let mut cpu_trace_log = TraceLogger::None;

    if let Some(trace_filename) = &config.machine.cpu.trace_file {
        log::warn!("Using CPU trace log: {:?}", trace_filename);
        cpu_trace_log = TraceLogger::from_filename(&trace_filename);
    }

    // Create the validator trace file, if specified
    let mut validator_trace = TraceLogger::None;

    if let Some(trace_filename) = &config.validator.trace_file {
        validator_trace = TraceLogger::from_filename(&trace_filename);
    }

    let trace_mode = config.machine.cpu.trace_mode.unwrap_or_default();

    #[cfg(feature = "cpu_validator")]
    use marty_core::cpu_validator::ValidatorMode;

    let mut cpu = Cpu::new(
        CpuType::Intel8088,
        trace_mode,
        cpu_trace_log,
        #[cfg(feature = "cpu_validator")]
        ValidatorType::None,
        #[cfg(feature = "cpu_validator")]
        validator_trace,
        #[cfg(feature = "cpu_validator")]
        ValidatorMode::Instruction,
        #[cfg(feature = "cpu_validator")]
        config.validator.baud_rate.unwrap_or(1_000_000),
    );

    if config.machine.cpu.trace_on {
        cpu.set_option(CpuOption::TraceLoggingEnabled(true));
    }

    // We should have a vector of tests now.

    let total_tests = tests.len();

    trace_print!(log, "Have {} tests from file.", total_tests);
    //_ = writeln!(log, "Have {} tests from file.", total_tests);
    //println!("Have {} tests from file.", total_tests);

    let mut results = TestResult {
        duration: Duration::new(0, 0),
        pass: false,
        passed: 0,
        warning: 0,
        failed: 0,
        cycle_mismatch: 0,
        mem_mismatch: 0,
        reg_mismatch: 0,
        warn_tests: LinkedList::new(),
        failed_tests: LinkedList::new(),
    };

    let test_start = Instant::now();

    // Loop through all tests and run them.
    for (n, test) in tests.iter().enumerate() {
        // Set up CPU registers to initial state.
        //println!("Setting up initial register state...");
        //println!("{}",test.initial_state.regs);

        // Set reset vector to our test instruction ip.
        let cs = test.initial_state.regs.cs;
        let ip = test.initial_state.regs.ip;
        cpu.set_reset_vector(CpuAddress::Segmented(cs, ip));
        cpu.reset();

        cpu.set_register16(Register16::AX, test.initial_state.regs.ax);
        cpu.set_register16(Register16::CX, test.initial_state.regs.cx);
        cpu.set_register16(Register16::DX, test.initial_state.regs.dx);
        cpu.set_register16(Register16::BX, test.initial_state.regs.bx);
        cpu.set_register16(Register16::SP, test.initial_state.regs.sp);
        cpu.set_register16(Register16::BP, test.initial_state.regs.bp);
        cpu.set_register16(Register16::SI, test.initial_state.regs.si);
        cpu.set_register16(Register16::DI, test.initial_state.regs.di);
        cpu.set_register16(Register16::ES, test.initial_state.regs.es);
        cpu.set_register16(Register16::CS, test.initial_state.regs.cs);
        cpu.set_register16(Register16::SS, test.initial_state.regs.ss);
        cpu.set_register16(Register16::DS, test.initial_state.regs.ds);
        cpu.set_register16(Register16::PC, test.initial_state.regs.ip);
        cpu.set_flags(test.initial_state.regs.flags);

        // Set up memory to initial state.
        //println!("Setting up initial memory state. {} memory states provided.", test.initial_state.ram.len());
        for mem_entry in &test.initial_state.ram {
            // Validate that mem_entry[1] fits in u8.

            let byte: u8 = mem_entry[1]
                .try_into()
                .expect(&format!("Invalid memory byte value: {:?}", mem_entry[1]));
            cpu.bus_mut()
                .write_u8(mem_entry[0] as usize, byte, 0)
                .expect("Failed to write memory");
        }

        // Decode this instruction
        let instruction_address = Cpu::calc_linear_address(cpu.get_register16(Register16::CS), cpu.ip());

        cpu.bus_mut().seek(instruction_address as usize);

        let mut i = match Cpu::decode(cpu.bus_mut()) {
            Ok(i) => i,
            Err(_) => {
                _ = writeln!(log, "Instruction decode error!");
                _ = log.flush();
                log::error!("Instruction decode error!");
                panic!("Instruction decode error!");
            }
        };

        cpu.set_option(CpuOption::EnableWaitStates(false));
        cpu.set_option(CpuOption::TraceLoggingEnabled(config.machine.cpu.trace_on));

        let mut rep = false;

        i.address = instruction_address;

        let disassembly_str = format!("{}", i);

        if test.name != disassembly_str {
            log::warn!("Test disassembly mismatch!");
            _ = writeln!(log, "Test disassembly mismatch!");
        }

        let mut opcode_string = format!("{:02X}", opcode);
        if let Some(ext) = extension_opt {
            opcode_string.push_str(&format!(".{:1X}", ext))
        }

        trace_print!(
            log,
            "{}| Test {:05}: Running test for instruction: {} ({})",
            opcode_string,
            n,
            i,
            i.size
        );
        /*
        println!(
            "{}| Test {:05}: Running test for instruction: {} ({})",
            opcode_string, n, i, i.size
        );
        _ = writeln!(
            log,
            "{}| Test {:05}: Running test for instruction: {} ({})",
            opcode_string, n, i, i.size
        );*/

        // Set terminating address for CPU validator.
        let end_address =
            Cpu::calc_linear_address(cpu.get_register16(Register16::CS), cpu.ip().wrapping_add(i.size as u16));

        //log::debug!("Setting end address: {:05X}", end_address);
        cpu.set_end_address(end_address as usize);

        let mut flags_on_stack = false;
        let mut debug_mnemonic = false;

        match i.mnemonic {
            Mnemonic::MOVSB
            | Mnemonic::MOVSW
            | Mnemonic::CMPSB
            | Mnemonic::CMPSW
            | Mnemonic::STOSB
            | Mnemonic::STOSW
            | Mnemonic::LODSB
            | Mnemonic::LODSW
            | Mnemonic::SCASB
            | Mnemonic::SCASW => {
                // limit cx to 31
                cpu.set_register16(Register16::CX, cpu.get_register16(Register16::CX) & 0x7F);
                rep = true;
            }
            Mnemonic::DIV | Mnemonic::IDIV => {
                // Divide exceptions possible - set a flag to ignore undefined flag state when
                // doing memory comparison (Since flags will be pushed to stack)
                flags_on_stack = true;
                debug_mnemonic = true;
            }
            _ => {}
        }

        // We loop here to handle REP string instructions, which are broken up into 1 effective instruction
        // execution per iteration. The 8088 makes no such distinction.
        loop {
            match cpu.step(false) {
                Ok((step_result, cycles)) => {
                    //println!("{}| Instruction reported result {:?}, {} cycles", opcode_string, step_result, cycles);

                    if rep & cpu.in_rep() {
                        continue;
                    }
                    break;
                }
                Err(err) => {
                    eprintln!("{}| CPU Error: {}\n", opcode_string, err);
                    cpu.trace_flush();
                    panic!("{}| CPU Error: {}\n", opcode_string, err);
                }
            }
        }

        // Finalize instruction.
        _ = cpu.step_finish();

        // CPU is done with execution. Check final state.
        //println!("CPU completed execution.");

        // Get cycle states from CPU.
        let mut cpu_cycles = cpu.get_cycle_states().clone();

        // Clean the CPU cycle states.
        clean_cycle_states(&mut cpu_cycles);

        // Validate final register state.
        let vregs = cpu.get_vregisters();

        if validate_registers(
            &metadata,
            opcode,
            extension_opt,
            false,
            &test.final_state.regs,
            &vregs,
            log,
        ) {
            //println!("{}| Registers validated against final state.", opcode_string);
            if debug_mnemonic {
                _ = writeln!(
                    log,
                    "Test registers:\n{}\nCPU registers:\n{}\n",
                    test.final_state.regs, vregs
                );
            }
            else {
                _ = writeln!(
                    log,
                    "{}| Test {:05}: Test registers match CPU registers",
                    opcode_string, n
                );
            }

            _ = writeln!(
                log,
                "{}| Test {:05}: Test flags {:04X} matched CPU flags: {:04X}",
                opcode_string, n, test.final_state.regs.flags, vregs.flags
            );
        }
        else {
            trace_error!(log, "{}| Test {:05}: Register validation failed", opcode_string, n);
            trace_error!(log, "Test specified:");
            trace_error!(log, "{}", test.final_state.regs);
            trace_error!(log, "{}", Cpu::flags_string(test.final_state.regs.flags));
            trace_error!(log, "CPU reported:");
            trace_error!(log, "{}", vregs);
            trace_error!(log, "{}", Cpu::flags_string(cpu.get_flags()));

            if test.cycles.len() != cpu_cycles.len() {
                _ = writeln!(
                    log,
                    "{}| Test {:05}:{} Additionally, test cycles {} do not match CPU cycles: {}",
                    opcode_string,
                    n,
                    &test.name,
                    test.cycles.len(),
                    cpu_cycles.len()
                );

                print_cycle_diff(log, &test.cycles, &cpu_cycles);
                cpu.trace_flush();
            }

            trace_error!(
                log,
                "{}| Test {:05}: Test hash {} failed.",
                opcode_string,
                n,
                &test.test_hash
            );

            let item = TestFailItem {
                num:    n as u32,
                name:   test.name.clone(),
                reason: FailType::RegMismatch,
            };

            results.failed += 1;
            results.reg_mismatch += 1;
            results.failed_tests.push_back(item);

            if stop_on_failure {
                break;
            }
            continue;
        }

        // Validate cycles
        if test.cycles.len() == cpu_cycles.len() {
            _ = writeln!(
                log,
                "{}| Test {:05}:{} Test cycles {} match CPU cycles: {}",
                opcode_string,
                n,
                &test.name,
                test.cycles.len(),
                cpu_cycles.len()
            );
        }
        else if ((test.cycles.len() as i32 - cpu_cycles.len() as i32).abs() > 1) {
            // If the difference is more than 1, the test has failed.
            trace_error!(
                log,
                "{}| Test {:05}:{} Test cycles {} DO NOT MATCH CPU cycles: {}",
                opcode_string,
                n,
                &test.name,
                test.cycles.len(),
                cpu_cycles.len()
            );

            print_cycle_diff(log, &test.cycles, &cpu_cycles);
            cpu.trace_flush();

            trace_error!(
                log,
                "{}| Test {:05}: Test hash {} failed.",
                opcode_string,
                n,
                &test.test_hash
            );

            let item = TestFailItem {
                num:    n as u32,
                name:   test.name.clone(),
                reason: FailType::CycleMismatch,
            };

            results.failed += 1;
            results.cycle_mismatch += 1;
            results.failed_tests.push_back(item);

            if stop_on_failure {
                break;
            }
            continue;
        }
        else if ((test.cycles.len() as i32 - cpu_cycles.len() as i32).abs() == 1) {
            // A cycle difference of only 1 is acceptable (for now)
            _ = writeln!(
                log,
                "{}| Test {:05}:{} Test cycles {} have ONE CYCLE variance to CPU cycles: {}",
                opcode_string,
                n,
                &test.name,
                test.cycles.len(),
                cpu_cycles.len()
            );

            print_cycle_diff(log, &test.cycles, &cpu_cycles);
            cpu.trace_flush();

            let item = TestFailItem {
                num:    n as u32,
                name:   test.name.clone(),
                reason: FailType::CycleMismatch,
            };

            results.warning += 1;
            results.cycle_mismatch += 1;
            results.warn_tests.push_back(item);
            continue;
        }
        else {
            // Cycle counts match, so we can do a full cycle validation.
            let (validate_result, _) = validate_cycles(&test.cycles, &cpu_cycles, log);
            if validate_result {
                _ = writeln!(log, "{}| Test {:05}: Test cycles validated!", opcode_string, n);
            }
            else {
                print_cycle_diff(log, &test.cycles, &cpu_cycles);
                cpu.trace_flush();

                let item = TestFailItem {
                    num:    n as u32,
                    name:   test.name.clone(),
                    reason: FailType::CycleMismatch,
                };

                results.failed += 1;
                results.cycle_mismatch += 1;
                results.failed_tests.push_back(item);

                if stop_on_failure {
                    break;
                }
                continue;
            }
        }

        // Validate final memory state.
        match validate_memory(&cpu, &test.final_state.ram, flags_on_stack, log) {
            Ok(_) => {
                _ = writeln!(log, "{}| Test {:05}: Test memory validated!", opcode_string, n);
            }
            Err(err) => {
                trace_error!(
                    log,
                    "{}| Test {:05}: Memory validation error. {}",
                    opcode_string,
                    n,
                    err,
                );

                let item = TestFailItem {
                    num:    n as u32,
                    name:   test.name.clone(),
                    reason: FailType::CycleMismatch,
                };

                results.failed += 1;
                results.mem_mismatch += 1;
                results.failed_tests.push_back(item);

                if stop_on_failure {
                    break;
                }
                continue;
            }
        };

        // If we got here, we passed!
        results.passed += 1;
    }

    results.duration = test_start.elapsed();

    results
}

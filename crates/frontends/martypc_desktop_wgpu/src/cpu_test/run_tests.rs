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

use std::{
    collections::{HashMap, LinkedList},
    fs::{copy, create_dir, read_dir, File},
    io::{BufWriter, ErrorKind, Read, Write},
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use colored::*;
use flate2::read::GzDecoder;
use marty_config::{ConfigFileParams, TestMode};

use crate::{
    cpu_test::{
        common::{
            clean_cycle_states,
            is_prefix_in_vec,
            opcode_extension_from_path,
            opcode_from_path,
            print_cycle_diff,
            print_cycle_summary,
            print_summary,
            read_tests_from_file,
            validate_cycles,
            validate_memory,
            validate_registers,
            CpuTest,
            FailType,
            Metadata,
            TestFailItem,
            TestFileLoad,
            TestResult,
            TestResultSummary,
        },
        run_tests::cpu_common::CpuAddress,
    },
    trace_error,
    trace_print,
};

use crate::cpu_test::common::{
    opcode_prefix_from_path,
    print_changed_flags,
    validate_memops,
    write_summary,
    MetadataFile,
    RegisterValidationResult,
};
use marty_core::{
    bytequeue::ByteQueue,
    cpu_808x::{Cpu, *},
    cpu_common,
    cpu_common::{builder::CpuBuilder, CpuOption, CpuSubType, CpuType, Mnemonic, OperandType, Register16, Register8},
    cpu_validator::{VRegistersDelta, ValidatorMode, ValidatorType},
    tracelogger::TraceLogger,
};

static METADATA_FILE: &str = "metadata.json";

pub fn run_runtests(config: ConfigFileParams) {
    let mut test_path = PathBuf::from("./tests".to_string());
    if let Some(test_path_inner) = &config.tests.test_path {
        test_path = test_path_inner.clone();
    }

    let mut test_base_path = PathBuf::new();
    //test_base_path.push(config.emulator.basedir.clone());

    // Interpret as absolute path
    test_base_path.push(test_path);

    log::debug!("Using test path: {:?}", test_base_path);

    // Sanity check options
    let validate_cycles = config.tests.test_run_validate_cycles.unwrap_or(false);
    let validate_regs = config.tests.test_run_validate_registers.unwrap_or(false);
    let validate_memops = config.tests.test_run_validate_memops.unwrap_or(false);
    let validate_flags = config.tests.test_run_validate_flags.unwrap_or(false);

    if !validate_cycles && !validate_regs && !validate_memops && !validate_flags {
        log::error!("No validation options enabled. Nothing to do.");
        return;
    }

    // Load metadata file.
    let mut metadata_path = test_base_path.clone();
    metadata_path.push(String::from(METADATA_FILE));
    let mut metadata_file = File::open(metadata_path.clone()).expect(&format!(
        "Couldn't open metadata file '{}' at path: {:?}",
        METADATA_FILE, metadata_path
    ));
    let mut contents = String::new();
    metadata_file
        .read_to_string(&mut contents)
        .expect("Failed to read metadata file.");

    let metadata_file: MetadataFile = serde_json::from_str(&contents).expect("Failed to parse metadata JSON");
    let metadata = &metadata_file.opcodes;

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
    let mut opcode_range_start = 0x00;
    let mut opcode_range_end = 0xFF;

    // Generate the list of opcodes we are going to generate tests for.
    if let Some(test_opcode_range) = &config.tests.test_opcode_range {
        if test_opcode_range.len() > 1 {
            opcode_range_start = test_opcode_range[0];
            opcode_range_end = test_opcode_range[1];
        }
        else {
            log::error!("Invalid opcode range specified. Using default.");
        }
    }

    let mut opcode_list = Vec::from_iter(opcode_range_start..=opcode_range_end);

    let mut opcode_range_exclude = Vec::new();

    if let Some(test_opcode_exclude_list) = &config.tests.test_opcode_exclude_list {
        opcode_range_exclude = test_opcode_exclude_list.clone();
    }

    opcode_list.retain(|&x| !opcode_range_exclude.contains(&x));

    let str_vec: Vec<String> = opcode_list.iter().map(|num| format!("{:02X}", num)).collect();

    log::debug!("Validating opcode list: {:?}", str_vec);

    let mut log_path = test_base_path.clone();
    let mut output_path = PathBuf::new();
    if let Some(ref test_output_path) = config.tests.test_output_path {
        output_path = PathBuf::from(test_output_path.clone());
        log_path = PathBuf::from(test_output_path.clone());
    }
    log_path.push("test_run.log");

    let mut summary = TestResultSummary {
        results: HashMap::new(),
    };

    let test_suite_start = Instant::now();

    let (tx, rx) = mpsc::sync_channel::<TestFileLoad>(1);

    let test_base_path_clone = test_base_path.clone();
    thread::spawn(move || {
        match read_dir(test_base_path_clone) {
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

    let mut total_run_test_cycles = 0;
    let mut total_run_cpu_cycles = 0;

    while let Ok(test_load) = rx.recv() {
        //_ = writeln!(&mut writer_lock, "Running tests from file: {:?}", test_load.path);

        println!(
            "Received file {:?} with {} tests from loading thread.",
            test_load.path,
            test_load.tests.len()
        );

        let opcode =
            opcode_from_path(&test_load.path).expect(&format!("Couldn't parse opcode from path: {:?}", test_load.path));
        let opcode_prefix_opt = opcode_prefix_from_path(&test_load.path);
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
            opcode_prefix_opt,
            opcode,
            extension_opt,
            &config,
            stop_on_failure,
            &mut log_writer,
        );

        total_run_test_cycles += results.cycles.prefetched.test + results.cycles.normal.test;
        total_run_cpu_cycles += results.cycles.prefetched.cpu + results.cycles.normal.cpu;

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

        print_cycle_summary(total_run_test_cycles as usize, total_run_cpu_cycles as usize);

        if let Some(summary_file) = &config.tests.test_run_summary_file {
            let mut summary_file_path = output_path;
            summary_file_path.push(summary_file);

            println!("Writing summary to file: {:?}", summary_file_path);

            match File::create(&summary_file_path) {
                Ok(file) => {
                    log::info!("Created summary file: {:?}", summary_file_path);
                    let mut writer = BufWriter::new(file);
                    write_summary(&summary, &mut writer);
                }
                Err(e) => {
                    log::error!("Failed to create summary file: {:?}", e);
                }
            }
        }
        else {
            log::warn!("No summary file specified.")
        }
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
    prefix_opt: Option<u8>,
    opcode: u8,
    extension_opt: Option<u8>,
    config: &ConfigFileParams,
    stop_on_failure: bool,
    log: &mut BufWriter<File>,
) -> TestResult {
    // Create the cpu trace file, if specified
    let mut trace_logger = TraceLogger::None;
    if let Some(trace_filename) = &config.machine.cpu.trace_file {
        log::warn!("Using CPU trace log: {:?}", trace_filename);
        trace_logger = TraceLogger::from_filename(&trace_filename);
    }

    // Create the validator trace file, if specified
    let mut validator_trace = TraceLogger::None;
    if let Some(trace_filename) = &config.validator.trace_file {
        validator_trace = TraceLogger::from_filename(&trace_filename);
    }

    let trace_mode = config.machine.cpu.trace_mode.unwrap_or_default();

    let cpu_type = config.tests.test_cpu_type.unwrap_or(CpuType::Intel8088);

    // Create the CPU. We require the cpu_validator feature as the CPU will not collect cycle
    // states without it.
    // TODO: Make cycle state collection a CPU option instead of relying on
    //       cpu_validator feature
    let mut cpu;
    #[cfg(feature = "cpu_validator")]
    {
        use marty_core::cpu_validator::ValidatorMode;
        cpu = match CpuBuilder::new()
            .with_cpu_type(cpu_type)
            //.with_cpu_subtype(CpuSubType::Intel8088)
            .with_trace_mode(trace_mode)
            .with_trace_logger(trace_logger)
            .with_validator_type(ValidatorType::None)
            .with_validator_mode(ValidatorMode::Instruction)
            .with_validator_logger(validator_trace)
            .with_validator_baud(config.validator.baud_rate.unwrap_or(1_000_000))
            .build()
        {
            Ok(cpu) => cpu,
            Err(e) => {
                log::error!("Failed to build CPU: {}", e);
                std::process::exit(1);
            }
        }
    };
    #[cfg(not(feature = "cpu_validator"))]
    {
        panic!("Validator feature not enabled!")
    };

    // Enable cycle tracing, if specified. This can help in debugging test failures.
    if config.machine.cpu.trace_on {
        cpu.set_option(CpuOption::TraceLoggingEnabled(true));
    }

    let total_tests = tests.len();
    trace_print!(log, "Have {} tests from file.", total_tests);

    // Create an empty TestResult struct to hold our results.
    let mut results = TestResult::default();
    // Start a timer for how long this test is going to take.
    let test_start = Instant::now();

    let mut total_test_cycles_seen: usize = 0;
    let mut total_pf_test_cycles_seen: usize = 0;
    let mut total_cpu_cycles_seen: usize = 0;
    let mut total_pf_cpu_cycles_seen: usize = 0;
    let mut test_cycles_seen: usize = 0;
    let mut cpu_cycles_seen: usize = 0;

    // Loop through all tests and run them.
    for (n, test) in tests.iter().enumerate() {
        // End if we are over the specified test run limit.
        if let Some(limit) = config.tests.test_run_limit {
            if n >= limit {
                log::warn!("Test run limit reached. Stopping.");
                break;
            }
        }

        // Is test prefetched? Set a flag.
        let test_prefetched = !test.initial_state.queue.is_empty();

        // Set up CPU. Set reset vector to our test instruction ip.
        let cs = test.initial_state.regs.cs;
        let ip = test.initial_state.regs.ip;
        cpu.set_reset_vector(CpuAddress::Segmented(cs, ip));

        // If this test is prefetched, we need to specify the initial queue state that reset()
        // will use as reset() flushes the queue.
        if test_prefetched {
            cpu.set_reset_queue_contents(test.initial_state.queue.clone());
        }

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
        cpu.set_register16(Register16::SS, test.initial_state.regs.ss);
        cpu.set_register16(Register16::DS, test.initial_state.regs.ds);
        // Note: CS and IP(PC) are set via reset vector
        cpu.set_flags(test.initial_state.regs.flags);

        // Set up memory to initial state.
        for mem_entry in &test.initial_state.ram {
            // Validate that mem_entry[1] fits in u8.
            let byte: u8 = mem_entry[1]
                .try_into()
                .expect(&format!("Invalid memory byte value: {:?}", mem_entry[1]));
            cpu.bus_mut()
                .write_u8(mem_entry[0] as usize, byte, 0)
                .expect("Failed to write memory");
        }

        // Decode this instruction with the specified CPU type.
        let instruction_address = cpu_common::calc_linear_address(cpu.get_register16(Register16::CS), cpu.get_ip());
        cpu.bus_mut().seek(instruction_address as usize);
        let mut i = match cpu.get_type().decode(cpu.bus_mut(), true) {
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
            log::warn!("Test disassembly mismatch! {} != {}", test.name, disassembly_str);
            _ = writeln!(log, "Test disassembly mismatch! {} != {}", test.name, disassembly_str);
        }

        let mut opcode_string = format!("{:02X}", opcode);
        if let Some(ext) = extension_opt {
            opcode_string.push_str(&format!(".{:1X}", ext))
        }

        trace_print!(
            log,
            "{}| Test {:05}: Running test for instruction: {} {:02X?} Prefetched: {}",
            opcode_string,
            n,
            i,
            test.bytes,
            test_prefetched
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
        let end_address = cpu_common::calc_linear_address(
            cpu.get_register16(Register16::CS),
            cpu.get_ip().wrapping_add(i.size as u16),
        );

        //log::debug!("Setting end address: {:05X}", end_address);

        cpu.set_end_address(CpuAddress::Flat(end_address));

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
            | Mnemonic::SCASW
            | Mnemonic::INSB
            | Mnemonic::INSW
            | Mnemonic::OUTSB
            | Mnemonic::OUTSW => {
                // limit cx to 31
                let cx = cpu.get_register16(Register16::CX);
                cpu.set_register16(Register16::CX, cx & 0x7F);
                rep = true;
            }
            Mnemonic::DIV | Mnemonic::IDIV => {
                // Divide exceptions possible - set a flag to ignore undefined flag state when
                // doing memory comparison (Since flags will be pushed to stack)
                flags_on_stack = true;
                debug_mnemonic = false;
            }
            Mnemonic::AAD | Mnemonic::AAM => {
                debug_mnemonic = true;
            }
            Mnemonic::SHL | Mnemonic::BOUND => {
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
        _ = cpu.step_finish(None);

        // CPU is done with execution. Check final state.
        //println!("CPU completed execution.");

        // Get cycle states from CPU.
        let mut cpu_cycles = cpu.get_cycle_states().clone();
        cpu_cycles_seen = cpu_cycles.len();

        // Clean the CPU cycle states.
        cpu_cycles_seen -= clean_cycle_states(&mut cpu_cycles);

        // Validate final register state.
        let cpu_vregs = cpu.get_vregisters();

        let test_final_vregs = test.initial_state.regs.clone().apply_delta(&test.final_state.regs);

        let mut current_test_failed = false;

        let do_validate_registers = config.tests.test_run_validate_registers.unwrap_or(true);
        let do_validate_flags = config.tests.test_run_validate_flags.unwrap_or(true);
        let do_validate_undef_flags = config.tests.test_run_validate_undefined_flags.unwrap_or(false);

        let mut dump_cycles = false;

        print_changed_flags(&test.initial_state.regs, &test_final_vregs, log);

        let validate_result = validate_registers(
            config.tests.test_cpu_type.unwrap_or(CpuType::Intel8088),
            &metadata,
            prefix_opt,
            opcode,
            extension_opt,
            &test_final_vregs,
            &cpu_vregs,
            log,
        );

        if do_validate_registers {
            match validate_result {
                RegisterValidationResult::Ok => {
                    //println!("{}| Registers validated against final state.", opcode_string);
                    if debug_mnemonic {
                        _ = writeln!(
                            log,
                            "{}| Test {:05}: Test registers, initial:\n{}, Test registers, final:\n{}\nCPU registers, final::\n{}\nCPU delta:\n{}\n",
                            opcode_string, n, test.initial_state.regs, test_final_vregs, cpu_vregs, test.final_state.regs
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
                        opcode_string, n, test_final_vregs.flags, cpu_vregs.flags
                    );
                }
                _ => {
                    if let RegisterValidationResult::GeneralMismatch | RegisterValidationResult::BothMismatch =
                        validate_result
                    {
                        _ = writeln!(
                            log,
                            "{}| Test {:05}: Register validation FAILED.\nTest registers, initial:\n{}, Test registers, final:\n{}\nTest delta:\n{}\nCPU registers, final::\n{}\n",
                            opcode_string, n, test.initial_state.regs, test_final_vregs, test.final_state.regs, cpu_vregs,
                        );
                    }
                    else {
                        _ = writeln!(
                            log,
                            "{}| Test {:05}: Register validation FLAG DIFFERENCE.\nTest registers, initial:\n{}, Test registers, final:\n{}\nCPU registers, final::\n{}\n",
                            opcode_string, n, test.initial_state.regs, test_final_vregs, cpu_vregs,
                        );
                    }

                    if test.cycles.len() != cpu_cycles.len() {
                        _ = writeln!(
                            log,
                            "{}| Test {:05}: Additionally, test cycles {} do not match CPU cycles: {}",
                            opcode_string,
                            n,
                            test.cycles.len(),
                            cpu_cycles.len()
                        );
                        dump_cycles = true;
                    }

                    trace_error!(
                        log,
                        "{}| Test {:05}: Test hash {} failed.",
                        opcode_string,
                        n,
                        &test.hash.clone().unwrap_or("".to_string())
                    );

                    trace_error!(
                        log,
                        "{}| Test {:05}: Register validation result: {:?}",
                        opcode_string,
                        n,
                        validate_result
                    );

                    match validate_result {
                        RegisterValidationResult::GeneralMismatch => {
                            results.reg_mismatch += 1;
                            let item = TestFailItem {
                                num:    n as u32,
                                name:   test.name.clone(),
                                reason: FailType::RegMismatch,
                            };
                            results.failed_tests.push_back(item);
                            current_test_failed = true;
                        }
                        RegisterValidationResult::FlagMismatch(defined_flags_match, undefined_flags_match) => {
                            log::warn!(
                                "Flag status: defined: {}, undefined: {}",
                                defined_flags_match,
                                undefined_flags_match
                            );
                            if do_validate_flags && !defined_flags_match {
                                results.flag_mismatch += 1;
                                let item = TestFailItem {
                                    num:    n as u32,
                                    name:   test.name.clone(),
                                    reason: FailType::RegMismatch,
                                };
                                results.failed_tests.push_back(item);
                                current_test_failed = true;
                            }
                            if do_validate_undef_flags && !undefined_flags_match {
                                results.undef_flag_mismatch += 1;
                            }
                        }
                        RegisterValidationResult::BothMismatch => {
                            results.reg_mismatch += 1;
                            if do_validate_flags {
                                results.flag_mismatch += 1;
                            }
                            let item = TestFailItem {
                                num:    n as u32,
                                name:   test.name.clone(),
                                reason: FailType::RegMismatch,
                            };
                            results.failed_tests.push_back(item);
                            current_test_failed = true;
                        }
                        _ => unreachable!("Bad register validation result."),
                    }
                }
            }
        }

        test_cycles_seen = test.cycles.len();
        let do_validate_cycles = config.tests.test_run_validate_cycles.unwrap_or(true);

        if do_validate_cycles {
            // Validate cycles
            if test.cycles.len() == cpu_cycles.len() {
                _ = writeln!(
                    log,
                    "{}| Test {:05}: Test cycles {} match CPU cycles: {}",
                    opcode_string,
                    n,
                    test.cycles.len(),
                    cpu_cycles.len()
                );
            }
            else if ((test.cycles.len() as i32 - cpu_cycles.len() as i32).abs() > 1) {
                // If the difference is more than 1, the test has failed.
                trace_error!(
                    log,
                    "{}| Test {:05}: Test cycles {} DO NOT MATCH CPU cycles: {}",
                    opcode_string,
                    n,
                    test.cycles.len(),
                    cpu_cycles.len()
                );

                dump_cycles = true;

                let item = TestFailItem {
                    num:    n as u32,
                    name:   test.name.clone(),
                    reason: FailType::CycleMismatch,
                };

                current_test_failed = true;
                results.cycle_mismatch += 1;
                results.failed_tests.push_back(item);
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

                dump_cycles = true;

                let item = TestFailItem {
                    num:    n as u32,
                    name:   test.name.clone(),
                    reason: FailType::CycleMismatch,
                };

                results.warning += 1;
                results.cycle_mismatch += 1;
                results.warn_tests.push_back(item);
            }
            else {
                // Cycle counts match, so we can do a full cycle validation.
                let (validate_result, _) = validate_cycles(&test.cycles, &cpu_cycles, log);
                if validate_result {
                    _ = writeln!(log, "{}| Test {:05}: Test cycles validated!", opcode_string, n);
                }
                else {
                    dump_cycles = true;

                    let item = TestFailItem {
                        num:    n as u32,
                        name:   test.name.clone(),
                        reason: FailType::CycleMismatch,
                    };

                    current_test_failed = true;
                    results.cycle_mismatch += 1;
                    results.failed_tests.push_back(item);
                }
            }
        }

        let do_validate_mem = config.tests.test_run_validate_memops.unwrap_or(true);

        if do_validate_mem {
            let mut mem_valid = true;
            // Validate memops
            match validate_memops(
                &cpu,
                &test.cycles,
                &cpu_cycles,
                instruction_address,
                i.size as usize,
                &test.initial_state.ram,
                &test.final_state.ram,
                flags_on_stack,
                test.initial_state.queue.len(),
                log,
            ) {
                Ok(_) => {
                    _ = writeln!(log, "{}| Test {:05}: Final memory OPS validated!", opcode_string, n);
                }
                Err(err) => {
                    mem_valid = false;
                    trace_error!(
                        log,
                        "{}| Test {:05}: Memory OPS validation error. {}",
                        opcode_string,
                        n,
                        err,
                    );
                }
            }

            // Validate final memory state.
            match validate_memory(&cpu, &test.final_state.ram, flags_on_stack, log) {
                Ok(_) => {
                    _ = writeln!(log, "{}| Test {:05}: Final memory STATE validated!", opcode_string, n);
                }
                Err(err) => {
                    mem_valid = false;
                    trace_error!(
                        log,
                        "{}| Test {:05}: Memory STATE validation error. {}",
                        opcode_string,
                        n,
                        err,
                    );
                }
            };

            if !mem_valid {
                let item = TestFailItem {
                    num:    n as u32,
                    name:   test.name.clone(),
                    reason: FailType::MemMismatch,
                };

                current_test_failed = true;
                results.mem_mismatch += 1;
                results.failed_tests.push_back(item);

                dump_cycles = true;
            }
        }

        if dump_cycles {
            print_cycle_diff(log, &test.cycles, &cpu_cycles);
            cpu.trace_flush();
        }

        if current_test_failed {
            trace_error!(
                log,
                "{}| Test {:05}: Test hash {} FAILED.",
                opcode_string,
                n,
                &test.hash.clone().unwrap_or("".to_string())
            );

            results.failed += 1;
        }
        else {
            results.passed += 1;
        }

        if test_prefetched {
            total_pf_test_cycles_seen += test_cycles_seen;
            total_pf_cpu_cycles_seen += cpu_cycles_seen;
        }
        else {
            total_test_cycles_seen += test_cycles_seen;
            total_cpu_cycles_seen += cpu_cycles_seen;
        }

        results.cycles.normal.test = total_test_cycles_seen;
        results.cycles.prefetched.test = total_pf_test_cycles_seen;
        results.cycles.normal.cpu = total_cpu_cycles_seen;
        results.cycles.prefetched.cpu = total_pf_cpu_cycles_seen;

        if stop_on_failure {
            break;
        }
    }

    results.duration = test_start.elapsed();
    results
}

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

    run_gentests.rs - Implement the main procedure for JSON test generation mode.
                      Requires CPU validator feature.
*/

#![allow(warnings, unused)]

use marty_core::cpu_common;
use std::{
    collections::{HashMap, LinkedList},
    ffi::OsString,
    fs::{copy, create_dir, read_dir, read_to_string, File, OpenOptions},
    io::{BufReader, BufWriter, ErrorKind, Read, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use colored::*;
use config_toml_bpaf::{ConfigFileParams, TestMode};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};

use crate::cpu_test::common::{
    is_prefix_in_vec,
    opcode_extension_from_path,
    opcode_from_path,
    read_tests_from_file,
    write_tests_to_file,
    CpuTest,
    TestFileLoad,
    TestState,
};

use marty_core::{
    bytequeue::ByteQueue,
    cpu_808x::{Cpu, *},
    cpu_common::{builder::CpuBuilder, CpuAddress, CpuOption, CpuSubType, CpuType, Mnemonic, Register16, TraceMode},
    cpu_validator::{
        BusCycle,
        BusOp,
        BusOpType,
        BusState,
        CpuValidator,
        CycleState,
        VRegisters,
        ValidatorMode,
        ValidatorType,
    },
    tracelogger::TraceLogger,
};

pub fn run_processtests(config: ConfigFileParams) {
    let mut test_path = "./tests".to_string();
    if let Some(test_dir) = &config.tests.test_dir {
        test_path = test_dir.clone();
    }

    let mut test_base_path = PathBuf::new();
    //test_base_path.push(config.emulator.basedir.clone());

    // Interpret as absolute path
    test_base_path.push(test_path);

    log::debug!("Using test path: {:?}", test_base_path);

    let mut log_path = test_base_path.clone();
    log_path.push("processing.log");

    if let Some(TestMode::Process) = &config.tests.test_mode {
        log::debug!("Processing tests...")
    }
    else {
        panic!("Invalid TestMode: {:?}!", config.tests.test_mode);
    }

    let mut processed_dir_path = test_base_path.clone();
    processed_dir_path.push("processed");

    match create_dir(processed_dir_path.clone()) {
        Ok(_) => {
            log::debug!("Created output path for processed tests.")
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
    log::debug!("Using processed dir path: {:?}", processed_dir_path);

    // Convert opcode ranges into strings
    let default_range = [0, 0xFF].to_vec();
    let op_range_start = config.tests.test_opcode_range.as_ref().unwrap_or(&default_range)[0];
    let op_range_end = config.tests.test_opcode_range.as_ref().unwrap_or(&default_range)[1];

    let str_vec: Vec<String> = (op_range_start..=op_range_end)
        .map(|num| format!("{:02X}", num))
        .collect();

    log::debug!("Validating opcode list: {:?}", str_vec);

    let test_suite_start = Instant::now();

    let (tx, rx) = mpsc::sync_channel::<TestFileLoad>(1);

    // Load the test files and send them over a channel as they are loaded.
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

    while let Ok(mut test_load) = rx.recv() {
        //_ = writeln!(&mut writer_lock, "Running tests from file: {:?}", test_load.path);

        println!(
            "Received file {:?} with {} tests from loading thread.",
            test_load.path,
            test_load.tests.len()
        );

        let opcode =
            opcode_from_path(&test_load.path).expect(&format!("Couldn't parse opcode from path: {:?}", test_load.path));
        let extension_opt = opcode_extension_from_path(&test_load.path);

        let modified = process_tests(
            &mut test_load.tests,
            opcode,
            extension_opt,
            &config,
            stop_on_failure,
            &mut log_writer,
        );

        if modified {
            println!(
                "Modified file {:?}, base filename: {:?}",
                test_load.path,
                test_load.path.file_name().unwrap()
            );
            let mut out_file_path = processed_dir_path.clone();
            out_file_path.push(test_load.path.file_name().unwrap());
            if let Some(ext) = out_file_path.extension() {
                if ext == "gz" {
                    out_file_path.set_extension("");
                }
            }
            println!("Writing file: {:?}", out_file_path);
            write_tests_to_file(out_file_path, &test_load.tests);
        }
    }
}

fn process_tests(
    tests: &mut LinkedList<CpuTest>,
    opcode: u8,
    extension_opt: Option<u8>,
    config: &ConfigFileParams,
    stop_on_failure: bool,
    log: &mut BufWriter<File>,
) -> bool {
    let mut modified = false;

    #[cfg(feature = "cpu_validator")]
    use marty_core::cpu_validator::ValidatorMode;

    let mut cpu;
    #[cfg(feature = "cpu_validator")]
    {
        cpu = match CpuBuilder::new()
            .with_cpu_type(CpuType::Intel8088)
            .with_cpu_subtype(CpuSubType::Intel8088)
            .with_validator_type(ValidatorType::None)
            .with_validator_mode(ValidatorMode::Instruction)
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

    // We should have a vector of tests now.
    let total_tests = tests.len();
    _ = writeln!(log, "Have {} tests from file.", total_tests);
    println!("Have {} tests from file.", total_tests);

    //let test_start = Instant::now();

    // Loop through all tests and process them.
    for (n, test) in tests.iter_mut().enumerate() {
        // Set reset vector to our test instruction ip.
        let cs = test.initial_state.regs.cs;
        let ip = test.initial_state.regs.ip;
        cpu.set_reset_vector(CpuAddress::Segmented(cs, ip));
        cpu.reset();

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

        let disassembly_str = format!("{}", i);

        if test.name != disassembly_str {
            println!(
                "Test {:05}: disassembly mismatch! test: \"{}\" current: \"{}\"",
                n, test.name, disassembly_str
            );

            test.name = disassembly_str;
            modified = true;
            //println!("Test bytes: {:x?}", test.bytes);
            //println!("Bus bytes: {:x?}", cpu.bus_mut().get_slice_at(instruction_address as usize, test.bytes.len()));
        }
        else {
            /*
            println!(
                "Test {:05}: Disassembly matches. test: {} current: {}",
                n,
                test.name,
                disassembly_str
            );
            */
        }
    }

    modified
}

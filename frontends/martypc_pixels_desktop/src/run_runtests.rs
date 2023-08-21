/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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


use std::{
    ffi::OsStr,
    fs::{read_dir, read_to_string, File, OpenOptions},
    collections::HashMap,
    io::{BufReader, BufWriter, Write, ErrorKind, Seek, SeekFrom},
    cell::RefCell,
    rc::Rc,
    path::PathBuf,
    time::{Instant, Duration}
};

use marty_core::{
    
    bytequeue::ByteQueue,
    cpu_808x::{
        *,
        Cpu,
        mnemonic::Mnemonic,
    },
    cpu_common::{CpuType, CpuOption},
    config::{ConfigFileParams, TraceMode, ValidatorType},
    devices::pic::Pic,
    cpu_validator::{CpuValidator, BusOp, BusOpType, CycleState, BusCycle, BusState},
    arduino8088_validator::ArduinoValidator,
    tracelogger::TraceLogger
};

use crate::cpu_test::{CpuTest, TestState};

use serde::{Serialize, Deserialize};

pub struct TestResults {

    pass: bool,
    tests_passed: u32,
    tests_failed: u32,
    first_failed_test: u32,
    first_failed_test_str: String,
    test_duration: Duration
}

pub fn run_runtests(config: &ConfigFileParams) {

    let mut test_path_postfix = "tests".to_string();
    if let Some(test_dir) = &config.tests.test_dir {
        test_path_postfix = test_dir.clone();
    }

    let mut test_base_path = PathBuf::new();
    test_base_path.push(config.emulator.basedir.clone());
    test_base_path.push(test_path_postfix);

    log::debug!("Using test path: {:?}", test_base_path);

    match read_dir(test_base_path) {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    // Filter for JSON files
                    if let Some(extension) = entry.path().extension() {
                        if extension == "json" {
                            // Load the JSON file
                            match read_tests_from_file(entry.path()) {
                                Some(tests) => {
                                    let results = run_tests(&tests, config);

                                    if !results.pass {
                                        eprintln!("Test failed. Stopping execution.");
                                        return
                                    }
                                    else {
                                        println!(
                                            "Test file completed. {} tests passed in {:.2} seconds.", 
                                            results.tests_passed,
                                            results.test_duration.as_secs_f32()
                                        );
                                    }
                                }
                                None => {
                                    eprintln!("Failed to parse json from file: {:?}. Skipping...", entry.path());
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
            return
        }
    }

    println!("All tests validated!");
}

pub fn read_tests_from_file(test_path: PathBuf) -> Option<Vec<CpuTest>> {

    let test_file_opt = match File::open(test_path.clone()) {
        Ok(file) => {
            println!("Opened test file: {:?}", test_path);
            Some(file)
        },
        Err(error) => {
            match error.kind() {
                ErrorKind::NotFound => {
                    println!("File not found error: {:?}", test_path);
                },
                error => {
                    println!("Failed to open the file due to: {:?}", error);
                }
            }
            None
        }
    };

    if test_file_opt.is_none() {
        return None;
    }

    let file = test_file_opt.unwrap();

    // Scope for BufReader
    let json_reader = BufReader::new(file);

    match serde_json::from_reader(json_reader) {
        Ok(json_obj) => Some(json_obj),
        Err(e) if e.is_eof() => {
            println!("File {:?} is empty. Creating new vector.", test_path);
            Some(Vec::new())
        } 
        Err(e) => {
            eprintln!("Failed to read json from file: {:?}: {:?}", test_path, e);
            None
        }
    }
}

pub fn run_tests(tests: &Vec<CpuTest>, config: &ConfigFileParams) -> TestResults {

    // Create the cpu trace file, if specified
    let mut cpu_trace = TraceLogger::None;
    if let Some(trace_filename) = &config.emulator.trace_file {
        cpu_trace = TraceLogger::from_filename(&trace_filename);
    }

    // Create the validator trace file, if specified
    let mut validator_trace = TraceLogger::None;
    /*
    if let Some(trace_filename) = &config.validator.trace_file {
        validator_trace = TraceLogger::from_filename(&trace_filename);
    }
    */

    let mut cpu = Cpu::new(
        CpuType::Intel8088,
        config.emulator.trace_mode,
        cpu_trace,
        #[cfg(feature = "cpu_validator")]
        ValidatorType::None,
        #[cfg(feature = "cpu_validator")]
        validator_trace
    );

    // We should have a vector of tests now.
    
    let total_tests = tests.len();
    println!("Have {} tests from file.", total_tests);
    
    let mut results = TestResults {
        pass: false,
        tests_passed: 0,
        tests_failed: 0,
        first_failed_test: 0,
        first_failed_test_str: String::new(),
        test_duration: Duration::new(0, 0)
    };

    let test_start = Instant::now();

    // Loop through all tests and run them.
    for (n, test) in tests.iter().enumerate() {

        // Set up CPU registers to initial state.
        println!("Setting up initial register state...");
        println!("{}",test.initial_state.regs);

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
        cpu.set_register16(Register16::IP, test.initial_state.regs.ip);
        cpu.set_flags(test.initial_state.regs.flags);

        // Set up memory to initial state.
        println!("Setting up initial memory state. {} memory states provided.", test.initial_state.ram.len());
        for mem_entry in &test.initial_state.ram {
            // Validate that mem_entry[1] fits in u8.

            let byte: u8 = mem_entry[1].try_into().expect(&format!("Invalid memory byte value: {:?}", mem_entry[1]));
            cpu.bus_mut().write_u8(mem_entry[0] as usize, byte, 0).expect("Failed to write memory");
        }
        
        // Decode this instruction
        let instruction_address = 
            Cpu::calc_linear_address(
                cpu.get_register16(Register16::CS),  
                cpu.get_register16(Register16::IP)
            );

        cpu.bus_mut().seek(instruction_address as usize);

        let mut i = match Cpu::decode(cpu.bus_mut()) {
            Ok(i) => i,
            Err(_) => {
                log::error!("Instruction decode error!");
                panic!("Instruction decode error!");
            }                
        };

        cpu.set_option(CpuOption::EnableWaitStates(false));
        cpu.set_option(CpuOption::TraceLoggingEnabled(config.emulator.trace_on));        

        let mut rep = false;

        i.address = instruction_address;
    
        println!("Test {}: Running test for instruction: {} ({})", n, i, i.size);
        
        // Set terminating address for CPU validator.
        let end_address = 
            Cpu::calc_linear_address(
                cpu.get_register16(Register16::CS),  
                cpu.get_register16(Register16::IP).wrapping_add(i.size as u16)
            );

        cpu.set_end_address(end_address as usize);
        log::debug!("Setting end address: {:05X}", end_address);

        // We loop here to handle REP string instructions, which are broken up into 1 effective instruction
        // execution per iteration. The 8088 makes no such distinction.
        loop {
            match cpu.step(false) {
                Ok((step_result, cycles)) => {
                    println!("Instruction reported result {:?}, {} cycles", step_result, cycles);

                    if rep & cpu.in_rep() {
                        continue
                    }
                    break;
                },
                Err(err) => {
                    eprintln!("CPU Error: {}\n", err);
                    cpu.trace_flush();
                    panic!("CPU Error: {}\n", err);
                } 
            }
        }

        // CPU is done with execution. Check final state.
        println!("CPU completed execution.");

        // Validate final register state.
        let vregs = cpu.get_vregisters();
        if test.final_state.regs == vregs {
            println!("Registers validated against final state.");
        }
        else {
            eprintln!("Register validation failed!");
            eprintln!("Test specified:");
            eprintln!("{}", test.final_state.regs);
            eprintln!("{}", Cpu::flags_string(test.final_state.regs.flags));
            eprintln!("CPU reported:");
            eprintln!("{}", vregs);
            eprintln!("{}", Cpu::flags_string(cpu.get_flags()));
            
            results.test_duration = test_start.elapsed();
            results.first_failed_test = n as u32;
            results.first_failed_test_str = test.name.clone();
            results.tests_failed = 1;
            results.tests_passed = n.saturating_sub(1) as u32;
            results.pass = false;

            return results
        }

        // Validate final memory state.
        for mem_entry in &test.final_state.ram {
            
            // Validate that mem_entry[0] < 0xFFFFF
            if mem_entry[0] > 0xFFFFF {
                panic!("Test {}: Invalid memory address value: {:?}", n, mem_entry[0]);
            }

            let addr: usize = mem_entry[0] as usize;

            // Validate that mem_entry[1] fits in u8.
            let byte: u8 = mem_entry[1].try_into().expect(&format!("Test {}: Invalid memory byte value: {:?}", n, mem_entry[1]));
            
            let (mem_byte, _) = cpu.bus_mut().read_u8(addr, 0).expect("Failed to read memory!");

            if byte != mem_byte {
                eprintln!("Test {}: Memory validation error. Address: {:05X} Test value: {:02X} Actual value: {:02X}", n, addr, byte, mem_byte);
                results.test_duration = test_start.elapsed();
                results.first_failed_test = n as u32;
                results.first_failed_test_str = test.name.clone();
                results.tests_failed = 1;
                results.tests_passed = (n - 1) as u32;
                results.pass = false;
            }
        }
    }

    results.tests_passed = tests.len() as u32;
    results.test_duration = test_start.elapsed();
    results.pass = true;
    results
}   
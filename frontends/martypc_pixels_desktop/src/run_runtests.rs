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
    fs::{read_dir, create_dir, copy, read_to_string, File, OpenOptions},
    collections::HashMap,
    io::{BufReader, BufWriter, Write, ErrorKind, Read, Seek, SeekFrom},
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
    cpu_validator::{CpuValidator, BusOp, BusOpType, CycleState, BusCycle, BusState, VRegisters},
    arduino8088_validator::{ArduinoValidator},
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

#[derive(Deserialize, Debug)]
struct InnerObject {
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags_mask: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reg: Option<HashMap<String, InnerObject>>,
}

type Metadata = HashMap<String, InnerObject>;

macro_rules! trace_error {
    ($wr:expr, $($t:tt)*) => {{
        let formatted_message = format!($($t)*);
        log::error!("{}", &formatted_message);

        // Assuming you want to write the message to the BufWriter as well.
        writeln!($wr, "{}", &formatted_message).expect("Failed to write to BufWriter");
        _ = $wr.flush();
    }};
}


fn opcode_from_path(path: &PathBuf) -> Option<u8> {
    path.file_stem() // Get the filename without the extension
        .and_then(|os_str| os_str.to_str()) // Convert OsStr to &str
        .and_then(|filename| {
            let hex_str = &filename[0..2]; // Take the first two characters
            u8::from_str_radix(hex_str, 16).ok() // Parse as hexadecimal
        })
}

fn opcode_extension_from_path(path: &PathBuf) -> Option<u8> {
    path.file_stem() // Get the filename without the final extension
        .and_then(|os_str| os_str.to_str()) // Convert OsStr to &str
        .and_then(|filename| {
            // Split the filename on '.' to separate potential opcode and extension
            let parts: Vec<&str> = filename.split('.').collect();
            
            if parts.len() == 2 {
                // If there are two parts, take the second one as the extension
                u8::from_str_radix(parts[1], 16).ok()
            } else {
                None
            }
        })
}

fn is_prefix_in_vec(path: &PathBuf, vec: &Vec<String>) -> bool {
    path.file_stem() // Get filename without extension
        .and_then(|os_str| os_str.to_str()) // Convert OsStr to &str
        .map(|s| s.chars().take(2).collect::<String>().to_uppercase()) // Take first two chars and convert to uppercase
        .map_or(false, |prefix| vec.contains(&prefix)) // Check if the prefix exists in the vec
}

pub fn run_runtests(config: &ConfigFileParams) {

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
    let mut metadata_file = File::open(metadata_path.clone()).expect(&format!("Couldn't open metadata file 8088.json at path: {:?}", metadata_path));
    let mut contents = String::new();
    metadata_file.read_to_string(&mut contents).expect("Failed to read metadata file.");

    let metadata: Metadata = serde_json::from_str(&contents).expect("Failed to parse metadata JSON");

    // Create 'validated' folder to receive validated tests

    let mut validated_dir_path = test_base_path.clone();
    validated_dir_path.push("validated");

    match create_dir(validated_dir_path.clone()) {
        Ok(_) => { log::debug!("Created output path for validated tests.")},
        Err(e) => match e.kind() {
            ErrorKind::AlreadyExists => {
                log::debug!("Output path already exists.")
            }
            _ => {
                log::error!("Failed to create output directory: {:?}", e);
                panic!("Failed to create output directory!");
            }
        }
    }

    log::debug!("Using validated dir path: {:?}", validated_dir_path);

    // Convert opcode ranges into strings
    let default_range = [0,0xFF].to_vec();
    let op_range_start = config.tests.test_opcode_range.as_ref().unwrap_or(&default_range)[0];
    let op_range_end = config.tests.test_opcode_range.as_ref().unwrap_or(&default_range)[1];

    let str_vec: Vec<String> = (op_range_start..=op_range_end).map(|num| format!("{:02X}", num)).collect();

    log::debug!("Validating opcode list: {:?}", str_vec);

    let mut log_path = test_base_path.clone();
    log_path.push("validation.log");

    let log = File::create(log_path).expect("Couldn't open logfile.");
    let mut log_writer = BufWriter::new(log);

    match read_dir(test_base_path) {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    // Filter for JSON files
                    if let Some(extension) = entry.path().extension() {
                        if extension.to_ascii_lowercase() == "json" && is_prefix_in_vec(&entry.path(), &str_vec) {
                            // Load the JSON file
                            match read_tests_from_file(entry.path()) {
                                Some(tests) => {

                                    _ = writeln!(&mut log_writer, "Running tests from file: {:?}", entry.path());

                                    let opcode = opcode_from_path(&entry.path()).expect(&format!("Couldn't parse opcode from path: {:?}", entry.path()));
                                    let extension_opt = opcode_extension_from_path(&entry.path());

                                    let results = run_tests(&metadata, &tests, opcode, extension_opt, config, &mut log_writer);

                                    if !results.pass {
                                        _ = writeln!(&mut log_writer, "Test failed. Stopping execution");
                                        _ = log_writer.flush();
                                        eprintln!("Test failed. Stopping execution.");

                                        return
                                    }
                                    else {
                                        _ = writeln!(
                                            &mut log_writer, 
                                            "Test file completed. {} tests passed in {:.2} seconds.", 
                                            results.tests_passed,
                                            results.test_duration.as_secs_f32()
                                        );
                                        println!(
                                            "Test file completed. {} tests passed in {:.2} seconds.", 
                                            results.tests_passed,
                                            results.test_duration.as_secs_f32()
                                        );
                                        _ = log_writer.flush();

                                        // Copy test file to validated directory.
                                        let mut copy_output_path = validated_dir_path.clone();
                                        copy_output_path.push(entry.path().file_name().unwrap());
    
                                        log::debug!("Using output path: {:?} from dir path: {:?}", copy_output_path, validated_dir_path);

                                        copy(entry.path(), copy_output_path.clone())
                                            .expect(
                                                &format!(
                                                    "Failed to copy file {:?} to output dir: {:?}!",
                                                    entry.path(),
                                                    copy_output_path
                                                )
                                            );

                                    }
                                }
                                None => {
                                    eprintln!("Failed to parse json from file: {:?}. Skipping...", entry.path());
                                    _ = writeln!(&mut log_writer, "Failed to parse json from file: {:?}. Skipping...", entry.path());
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
    _ = writeln!(&mut log_writer, "All tests validated!");
    _ = log_writer.flush();

    // writer & file dropped here
    
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
    
    let result;

    {
        let mut file = test_file_opt.unwrap();
        let mut file_string = String::new();
        
        file.read_to_string(&mut file_string).expect("Error reading in JSON file to string!");

        /*
        // using BufReader & from_reader with serde-json is slow, see: 
        // https://docs.rs/serde_json/latest/serde_json/fn.from_reader.html
        // Scope for BufReader
        let json_reader = BufReader::new(file);


        result = match serde_json::from_reader(json_reader) {
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
        */

        result = match serde_json::from_str(&file_string) {
            Ok(json_obj) => Some(json_obj),
            Err(e) if e.is_eof() => {
                println!("JSON file {:?} is empty. Creating new vector.", test_path);
                Some(Vec::new())
            } 
            Err(e) => {
                eprintln!("Failed to read json from file: {:?}: {:?}", test_path, e);
                None
            }
        }
    }

    result
}

fn run_tests(
    metadata: &Metadata,
    tests: &Vec<CpuTest>, 
    opcode: u8,
    extension_opt: Option<u8>,
    config: &ConfigFileParams, 
    log: &mut BufWriter<File>
) -> TestResults {

    // Create the cpu trace file, if specified
    let mut cpu_trace = TraceLogger::None;
    if let Some(trace_filename) = &config.emulator.trace_file {
        log::debug!("Using instruction trace log: {:?}", trace_filename);
        cpu_trace = TraceLogger::from_filename(&trace_filename);
    }

    // Create the validator trace file, if specified
    let mut validator_trace = TraceLogger::None;
    /*
    if let Some(trace_filename) = &config.validator.trace_file {
        validator_trace = TraceLogger::from_filename(&trace_filename);
    }
    */

    #[cfg(feature = "cpu_validator")]
    use marty_core::cpu_validator::ValidatorMode;

    let mut cpu = Cpu::new(
        CpuType::Intel8088,
        config.emulator.trace_mode,
        cpu_trace,
        #[cfg(feature = "cpu_validator")]
        ValidatorType::None,
        #[cfg(feature = "cpu_validator")]
        validator_trace,
        #[cfg(feature = "cpu_validator")]
        ValidatorMode::Instruction,
        #[cfg(feature = "cpu_validator")]
        config.validator.baud_rate.unwrap_or(1_000_000)
    );

    // We should have a vector of tests now.
    
    let total_tests = tests.len();

    _ = writeln!(log, "Have {} tests from file.", total_tests);
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
                _ = writeln!(log, "Instruction decode error!");
                _ = log.flush();
                log::error!("Instruction decode error!");
                panic!("Instruction decode error!");
            }                
        };

        cpu.set_option(CpuOption::EnableWaitStates(false));
        cpu.set_option(CpuOption::TraceLoggingEnabled(config.emulator.trace_on));        

        let mut rep = false;

        i.address = instruction_address;
    
        let disassembly_str = format!("{}", i);
        
        if test.name != disassembly_str {
            log::warn!("Test disassembly mismatch!");
            _ = writeln!(log, "Test disassembly mismatch!");
        }

        println!("Test {}: Running test for instruction: {} ({})", n, i, i.size);
        
        // Set terminating address for CPU validator.
        let end_address = 
            Cpu::calc_linear_address(
                cpu.get_register16(Register16::CS),  
                cpu.get_register16(Register16::IP).wrapping_add(i.size as u16)
            );

        cpu.set_end_address(end_address as usize);
        log::debug!("Setting end address: {:05X}", end_address);

        match i.mnemonic {
            Mnemonic::MOVSB | Mnemonic::MOVSW | Mnemonic::CMPSB | Mnemonic::CMPSW | Mnemonic::STOSB | 
            Mnemonic::STOSW | Mnemonic::LODSB | Mnemonic::LODSW | Mnemonic::SCASB | Mnemonic::SCASW => {
                // limit cx to 31
                cpu.set_register16(Register16::CX, cpu.get_register16(Register16::CX) & 0x7F);
                rep = true;
            }
            _ => {}
        }

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
            log) 
        {
            println!("Registers validated against final state.");

            _ = writeln!(log, "Test {:05}: Test flags {:04X} matched CPU flags: {:04X}", n, test.final_state.regs.flags, vregs.flags);
        }
        else {

            _ = writeln!(log, "Register validation failed, test number: {}", n);

            eprintln!("Register validation failed, test number: {}", n);
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

        // Validate cycles
        if test.cycles.len() == cpu_cycles.len() {
            _ = writeln!(log, "Test {:05}: Test cycles {} matches CPU cycles: {}", n, test.cycles.len(), cpu_cycles.len());
        }
        else {
            trace_error!(log, "Test {:05}: Test cycles {} DO NOT MATCH CPU cycles: {}", n, test.cycles.len(), cpu_cycles.len());
            
            print_cycle_diff(&test.cycles, &cpu_cycles);

            cpu.trace_flush();
            panic!("Test validation failure!");
        }

        let (validate_result, _) = validate_cycles(&test.cycles, &cpu_cycles, log);
        if validate_result {
            _ = writeln!(log, "Test {:05}: Test cycles validated!", n);
        }
        else {
            print_cycle_diff(&test.cycles, &cpu_cycles);
            
            cpu.trace_flush();
            panic!("Test validation failure!");
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
            
            let mem_byte = cpu.bus().peek_u8(addr).expect("Failed to read memory!");

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


fn validate_registers(
    metadata: &Metadata, 
    opcode: u8, 
    extension_opt: Option<u8>,
    mask: bool,
    test_regs: &VRegisters, 
    cpu_regs: &VRegisters, 
    log: &mut BufWriter<File>
) -> bool {

    let mut regs_validate = true;

    if test_regs.ax != cpu_regs.ax {
        regs_validate = false;
    }
    if test_regs.bx != cpu_regs.bx {
        regs_validate = false;
    }
    if test_regs.cx != cpu_regs.cx {
        regs_validate = false;
    }
    if test_regs.dx != cpu_regs.dx {
        regs_validate = false;
    }
    if test_regs.cs != cpu_regs.cs {
        regs_validate = false;
    }
    if test_regs.ds != cpu_regs.ds {
        regs_validate = false;
    }
    if test_regs.es != cpu_regs.es {
        regs_validate = false;
    }
    if test_regs.sp != cpu_regs.sp {
        regs_validate = false;
    }
    if test_regs.sp != cpu_regs.sp {
        regs_validate = false;
    }    
    if test_regs.bp != cpu_regs.bp {
        regs_validate = false;
    }    
    if test_regs.si != cpu_regs.si {
        regs_validate = false;
    }    
    if test_regs.di != cpu_regs.di {
        regs_validate = false;
    }

    let opcode_key = format!("{:02X}", opcode);

    let opcode_inner = metadata.get(&opcode_key).expect(&format!("No metadata for opcode: {:02X}", opcode));
    let opcode_final;

    if let Some(extension) = extension_opt {

        let extension_key = format!("{:1X}", extension);

        if let Some(reg) = &opcode_inner.reg {
            opcode_final = reg.get(&extension_key).expect(&format!("No metadata for opcode: {:02X} extension: {:1X}", opcode, extension));
        }
        else {
            trace_error!(log, "no 'reg' entry for extension!");
            panic!("no 'reg' entry for extension!");
        }        
    }
    else {
        opcode_final = opcode_inner;
    }

    let flags_mask = if mask {
        opcode_final.flags_mask.unwrap_or(0xFFFF) as u16
    } 
    else {
        0xFFFF
    };

    let test_flags_masked = test_regs.flags & flags_mask;
    let cpu_flags_masked = test_regs.flags & flags_mask;

    if test_flags_masked != cpu_flags_masked {

        trace_error!(log, "CPU flags mismatch! EMU: 0b{:08b} != CPU: 0b{:08b}", test_flags_masked, cpu_flags_masked);
        //trace_error!(self, "Unmasked: EMU: 0b{:08b} != CPU: 0b{:08b}", self.current_frame.regs[1].flags, regs.flags);            
        regs_validate = false;

        let flag_diff = test_flags_masked ^ cpu_flags_masked;

        if flag_diff & CPU_FLAG_CARRY != 0 {
            trace_error!(log, "CARRY flag differs.");
        }
        if flag_diff & CPU_FLAG_PARITY != 0 {
            trace_error!(log, "PARITY flag differs.");
        }
        if flag_diff & CPU_FLAG_AUX_CARRY != 0 {
            trace_error!(log, "AUX CARRY flag differs.");
        }
        if flag_diff & CPU_FLAG_ZERO != 0 {
            trace_error!(log, "ZERO flag differs.");
        }
        if flag_diff & CPU_FLAG_SIGN != 0 {
            trace_error!(log, "SIGN flag differs.");
        }
        if flag_diff & CPU_FLAG_TRAP != 0 {
            trace_error!(log, "TRAP flag differs.");
        }
        if flag_diff & CPU_FLAG_INT_ENABLE != 0 {
            trace_error!(log, "INT flag differs.");
        }
        if flag_diff & CPU_FLAG_DIRECTION != 0 {
            trace_error!(log, "DIRECTION flag differs.");
        }
        if flag_diff & CPU_FLAG_OVERFLOW != 0 {
            trace_error!(log, "OVERFLOW flag differs.");
        }                    
        //panic!("CPU flag mismatch!")
    }

    regs_validate
}

fn validate_cycles(
    cpu_states: &[CycleState], 
    emu_states: &[CycleState],
    log: &mut BufWriter<File>
) -> (bool, usize) {

    if emu_states.len() != cpu_states.len() {
        // Cycle count mismatch
        return (false, 0)
    }

    for i in 0..cpu_states.len() {

        if emu_states[i] != cpu_states[i] {
            // Cycle state mismatch

            trace_error!(
                log, 
                "State validation failure: {:?} vs {:?}", 
                &emu_states[i], 
                &cpu_states[i]
            );

            return (false, i)
        }
    }

    (true, 0)
}

pub fn clean_cycle_states(states: &mut Vec<CycleState>) {

    let pre_clean_len = states.len();

    // Drop all states before first Fetch
    let mut found = false;
    states.retain(|state| {
        if matches!(state.q_op, QueueOp::First) {
            found = true;
        }
        found
    });

    let trimmed_ct = pre_clean_len - states.len();

    log::debug!("Clean: Deleted {} cycle states", trimmed_ct);

    for mut state in states {
        // Set address bus to 0 if no ALE signal.
        if !state.ale {
            //state.addr = 0;
        }

        // Set t-cycle to Ti if t-cycle is T1 and bus status PASV.
        if let BusCycle::T1 = state.t_state {
            if let BusState::PASV = state.b_state {
                // If we are in T1 but PASV bus, this is really a Ti state.
                state.t_state = BusCycle::Ti;
            }
        }

        // Set queue read byte to 0 if no queue op.
        if let QueueOp::Idle = state.q_op {
            state.q_byte = 0;
        }

        // Set data bus to 0 if no read or write op.
        if !state.mrdc || !state.mwtc || !state.iorc || !state.iowc {
            // Active read or write. Allow data bus value through if T3.
            if let BusCycle::T3 | BusCycle::Tw = state.t_state {
                // do nothing
            }
            else {
                // Data bus isn't active this cycle.
                state.data_bus = 0;
            }
        }
        else {
            // No active read or write.
            state.data_bus = 0;
        }
    }
}

fn print_cycle_diff(test_states: &Vec::<CycleState>, cpu_states: &[CycleState]) {

    let max_lines = std::cmp::max(cpu_states.len(), test_states.len());

    for i in 0..max_lines {

        let cpu_str;
        let emu_str;
        
        if i < test_states.len() {
            cpu_str = test_states[i].to_string();
        }
        else {
            cpu_str = String::new();
        }

        if i < cpu_states.len() {
            emu_str = cpu_states[i].to_string();
        }
        else {
            emu_str = String::new();
        }

        log::debug!("{:<80} | {:<80}", cpu_str, emu_str);
    }
}    

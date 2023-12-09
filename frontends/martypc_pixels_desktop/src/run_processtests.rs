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

#![allow(warnings, unused)]

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

use marty_core::{
    bytequeue::ByteQueue,
    cpu_808x::{mnemonic::Mnemonic, Cpu, *},
    cpu_common::{CpuOption, CpuType, TraceMode},
    cpu_validator::{BusCycle, BusOp, BusOpType, BusState, CpuValidator, CycleState, VRegisters, ValidatorType},
    tracelogger::TraceLogger,
};

use config_toml_bpaf::{ConfigFileParams, TestMode};

use crate::cpu_test::{CpuTest, TestState};

use colored::*;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};

pub enum FailType {
    CycleMismatch,
    MemMismatch,
    RegMismatch,
}

pub struct TestFileLoad {
    path:  PathBuf,
    tests: LinkedList<CpuTest>,
}

pub struct TestFailItem {
    num:    u32,
    name:   String,
    reason: FailType,
}

pub struct TestResult {
    pass: bool,
    duration: Duration,
    passed: u32,
    failed: u32,
    cycle_mismatch: u32,
    mem_mismatch: u32,
    reg_mismatch: u32,
    failed_tests: LinkedList<TestFailItem>,
}

pub struct TestResultSummary {
    results: HashMap<OsString, TestResult>,
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
            }
            else {
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

    let mut cpu = Cpu::new(
        CpuType::Intel8088,
        config.machine.cpu.trace_mode.unwrap_or_default(),
        TraceLogger::None,
        #[cfg(feature = "cpu_validator")]
        ValidatorType::None,
        #[cfg(feature = "cpu_validator")]
        TraceLogger::None,
        #[cfg(feature = "cpu_validator")]
        ValidatorMode::Instruction,
        #[cfg(feature = "cpu_validator")]
        config.validator.baud_rate.unwrap_or(1_000_000),
    );

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
        let instruction_address =
            Cpu::calc_linear_address(cpu.get_register16(Register16::CS), cpu.get_register16(Register16::IP));

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

pub fn write_tests_to_file(path: PathBuf, tests: &LinkedList<CpuTest>) {
    let file_opt: Option<File>;

    if path.exists() {
        file_opt = match OpenOptions::new().write(true).truncate(true).open(path.clone()) {
            Ok(file) => Some(file),
            Err(e) => {
                eprintln!("Couldn't reopen output file {:?} for writing: {:?}", path, e);
                None
            }
        };
    }
    else {
        file_opt = match OpenOptions::new()
            .create_new(true)
            .write(true)
            .truncate(true)
            .open(path.clone())
        {
            Ok(file) => Some(file),
            Err(e) => {
                eprintln!("Couldn't create output file {:?}: {:?}", path, e);
                None
            }
        }
    }

    if let None = file_opt {
        panic!("Couldn't open or create output file!");
    }

    let mut file = file_opt.unwrap();

    file.seek(SeekFrom::Start(0)).expect("Couldn't seek file.");
    file.set_len(0).expect("Couldn't truncate file");

    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, &tests).expect("Couldn't write JSON to output file!");
}

pub fn read_tests_from_file(test_path: PathBuf) -> Option<LinkedList<CpuTest>> {
    let test_file_opt = match File::open(test_path.clone()) {
        Ok(file) => {
            println!("Opened test file: {:?}", test_path);
            Some(file)
        }
        Err(error) => {
            match error.kind() {
                ErrorKind::NotFound => {
                    println!("File not found error: {:?}", test_path);
                }
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

        // Is file gzipped?
        match test_path.extension().and_then(std::ffi::OsStr::to_str) {
            Some("gz") => {
                let mut decoder = GzDecoder::new(BufReader::new(file));

                decoder
                    .read_to_string(&mut file_string)
                    .expect("Failed to decompress gzip archive.");
            }
            Some("json") => {
                file.read_to_string(&mut file_string)
                    .expect("Error reading in JSON file to string!");
            }
            _ => {
                log::error!("Bad extension!");
                return None;
            }
        }

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
                Some(LinkedList::new())
            }
            Err(e) => {
                eprintln!("Failed to read json from file: {:?}: {:?}", test_path, e);
                None
            }
        }
    }

    result
}

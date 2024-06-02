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

    cpu_test::common.rs - Functions common to CPU test utilities.

*/

#![allow(dead_code)]

use std::{
    collections::{HashMap, LinkedList},
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{BufReader, BufWriter, ErrorKind, Read, Seek, SeekFrom, Write},
    path::PathBuf,
    time::Duration,
};

use anyhow::{bail, Error};
use colored::Colorize;
use flate2::read::GzDecoder;
use serde_derive::{Deserialize, Serialize};

use marty_core::{
    cpu_808x::*,
    cpu_common::{CpuDispatch, CpuType, QueueOp},
    cpu_validator::{AccessType, BusCycle, BusOp, BusOpType, BusState, CycleState, VRegisters, VRegistersDelta},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct TestStateInitial {
    pub regs:  VRegisters,
    pub ram:   Vec<[u32; 2]>,
    pub queue: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestStateFinal {
    pub regs:  VRegistersDelta,
    pub ram:   Vec<[u32; 2]>,
    pub queue: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct CpuTest {
    pub name:  String,  // Human readable name (disassembly)
    pub bytes: Vec<u8>, // Instruction bytes

    #[serde(rename = "initial")]
    pub initial_state: TestStateInitial, // Initial state of CPU before test execution

    #[serde(rename = "final")]
    pub final_state: TestStateFinal, // Final state of CPU after test execution

    pub cycles: Vec<CycleState>,

    #[serde(alias = "test_hash", skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,

    #[serde(alias = "test_num", skip_serializing_if = "Option::is_none")]
    pub idx: Option<usize>,
}

pub enum FailType {
    CycleMismatch,
    MemMismatch,
    RegMismatch,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RegisterValidationResult {
    Ok,
    GeneralMismatch,
    FlagMismatch(bool, bool),
    BothMismatch,
}

pub struct TestFileLoad {
    pub path:  PathBuf,
    pub tests: LinkedList<CpuTest>,
}

pub struct TestFailItem {
    pub num:    u32,
    pub name:   String,
    pub reason: FailType,
}

#[derive(Default)]
pub struct CycleStats {
    pub test: usize,
    pub cpu:  usize,
}

#[derive(Default)]
pub struct CycleResults {
    pub prefetched: CycleStats,
    pub normal: CycleStats,
}

pub struct TestResult {
    pub pass: bool,
    pub duration: Duration,
    pub passed: u32,
    pub warning: u32,
    pub failed: u32,
    pub cycle_mismatch: u32,
    pub mem_mismatch: u32,
    pub reg_mismatch: u32,
    pub flag_mismatch: u32,
    pub undef_flag_mismatch: u32,
    pub warn_tests: LinkedList<TestFailItem>,
    pub failed_tests: LinkedList<TestFailItem>,
    pub cycles: CycleResults,
}

impl Default for TestResult {
    fn default() -> Self {
        TestResult {
            duration: Duration::new(0, 0),
            pass: false,
            passed: 0,
            warning: 0,
            failed: 0,
            cycle_mismatch: 0,
            mem_mismatch: 0,
            reg_mismatch: 0,
            flag_mismatch: 0,
            undef_flag_mismatch: 0,
            warn_tests: LinkedList::new(),
            failed_tests: LinkedList::new(),
            cycles: CycleResults::default(),
        }
    }
}

pub struct TestResultSummary {
    pub results: HashMap<OsString, TestResult>,
}

#[derive(Deserialize, Debug)]
pub struct InnerObject {
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<String>,
    #[serde(default, rename = "flags-mask", skip_serializing_if = "Option::is_none")]
    pub flags_mask: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reg: Option<HashMap<String, InnerObject>>,
}

#[derive(Deserialize, Debug)]
pub struct MetadataFile {
    pub github: Option<String>,
    pub version: String,
    pub syntax_version: u32,
    pub cpu: String,
    pub generator: String,
    pub author: Option<String>,
    pub date: Option<String>,
    pub opcodes: Metadata,
}
pub type Metadata = HashMap<String, InnerObject>;

#[derive(Copy, Clone, Debug, Default)]
pub struct MemOpValidationState {
    in_test_state: bool,
    in_cycles: bool,
    is_fetch: bool,
}

#[macro_export]
macro_rules! trace {
    ($wr:expr, $($t:tt)*) => {{
        let formatted_message = format!($($t)*);
        writeln!($wr, "{}", &formatted_message).expect("Failed to write to BufWriter");
        _ = $wr.flush();
    }};
}

#[macro_export]
macro_rules! trace_error {
    ($wr:expr, $($t:tt)*) => {{
        let formatted_message = format!($($t)*);
        //log::error!("{}", &formatted_message);
        writeln!($wr, "{}", &formatted_message).expect("Failed to write to BufWriter");
        _ = $wr.flush();
    }};
}

#[macro_export]
macro_rules! trace_print {
    ($wr:expr, $($t:tt)*) => {{
        let formatted_message = format!($($t)*);
        println!("{}", &formatted_message);
        writeln!($wr, "{}", &formatted_message).expect("Failed to write to BufWriter");
        _ = $wr.flush();
    }};
}

pub fn opcode_from_path(path: &PathBuf) -> Option<u8> {
    path.file_stem() // Get the filename without the extension
        .and_then(|os_str| os_str.to_str()) // Convert OsStr to &str
        .and_then(|filename| {
            let fn_parts = filename.split('.').collect::<Vec<&str>>();
            let opcode_str = fn_parts[0]; // Take the first part of the filename

            if opcode_str.len() == 2 {
                let hex_str = &filename[0..2]; // Take the first two characters
                u8::from_str_radix(hex_str, 16).ok() // Parse as hexadecimal
            }
            else if opcode_str.len() == 4 {
                let hex_str = &filename[2..4]; // Take the last two characters
                u8::from_str_radix(hex_str, 16).ok() // Parse as hexadecimal
            }
            else {
                log::error!("Bad filename format: {:?}", filename);
                None
            }
        })
}

pub fn opcode_prefix_from_path(path: &PathBuf) -> Option<u8> {
    path.file_stem() // Get the filename without the extension
        .and_then(|os_str| os_str.to_str()) // Convert OsStr to &str
        .and_then(|filename| {
            let fn_parts = filename.split('.').collect::<Vec<&str>>();
            let opcode_str = fn_parts[0]; // Take the first part of the filename

            if opcode_str.len() == 2 {
                None
            }
            else if opcode_str.len() == 4 {
                let hex_str = &filename[0..2]; // Take first two characters
                u8::from_str_radix(hex_str, 16).ok() // Parse as hexadecimal
            }
            else {
                log::error!("Bad filename format: {:?}", filename);
                None
            }
        })
}

pub fn opcode_extension_from_path(path: &PathBuf) -> Option<u8> {
    path.file_stem() // Get the filename without the final extension
        .and_then(|os_str| os_str.to_str()) // Convert OsStr to &str
        .and_then(|filename| {
            // Split the filename on '.' to separate potential opcode and extension
            let parts: Vec<&str> = filename.split('.').collect();

            if parts.len() == 3 {
                // If there are three parts, take the second one as the extension
                u8::from_str_radix(parts[1], 16).ok()
            }
            else {
                None
            }
        })
}

pub fn is_prefix_in_vec(path: &PathBuf, vec: &Vec<String>) -> bool {
    path.file_stem() // Get filename without extension
        .and_then(|os_str| os_str.to_str()) // Convert OsStr to &str
        .map(|s| s.chars().take(2).collect::<String>().to_uppercase()) // Take first two chars and convert to uppercase
        .map_or(false, |prefix| vec.contains(&prefix)) // Check if the prefix exists in the vec
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

        // using BufReader & from_reader with serde-json is slow, see:
        // https://docs.rs/serde_json/latest/serde_json/fn.from_reader.html
        /*
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

pub fn print_changed_flags(initial_regs: &VRegisters, final_regs: &VRegisters, log: &mut BufWriter<File>) {
    let initial_flags = initial_regs.flags;
    let final_flags = final_regs.flags;
    let flag_diff = initial_flags ^ final_flags;

    if flag_diff & CPU_FLAG_CARRY != 0 {
        trace!(
            log,
            "CARRY flag changed (initial:{}, final:{})",
            initial_flags & CPU_FLAG_CARRY != 0,
            final_flags & CPU_FLAG_CARRY != 0
        );
    }
    if flag_diff & CPU_FLAG_PARITY != 0 {
        trace!(
            log,
            "PARITY flag changed. (initial:{}, final:{})",
            initial_flags & CPU_FLAG_PARITY != 0,
            final_flags & CPU_FLAG_PARITY != 0
        );
    }
    if flag_diff & CPU_FLAG_AUX_CARRY != 0 {
        trace!(
            log,
            "AUX CARRY flag changed. (initial:{}, final:{})",
            initial_flags & CPU_FLAG_AUX_CARRY != 0,
            final_flags & CPU_FLAG_AUX_CARRY != 0
        );
    }
    if flag_diff & CPU_FLAG_ZERO != 0 {
        trace!(
            log,
            "ZERO flag changed. (initial:{}, final:{})",
            initial_flags & CPU_FLAG_ZERO != 0,
            final_flags & CPU_FLAG_ZERO != 0
        );
    }
    if flag_diff & CPU_FLAG_SIGN != 0 {
        trace!(
            log,
            "SIGN flag changed. (initial:{}, final:{})",
            initial_flags & CPU_FLAG_SIGN != 0,
            final_flags & CPU_FLAG_SIGN != 0
        );
    }
    if flag_diff & CPU_FLAG_TRAP != 0 {
        trace!(
            log,
            "TRAP flag changed. (initial:{}, final:{})",
            initial_flags & CPU_FLAG_TRAP != 0,
            final_flags & CPU_FLAG_TRAP != 0
        );
    }
    if flag_diff & CPU_FLAG_INT_ENABLE != 0 {
        trace!(
            log,
            "INT flag changed. (initial:{}, final:{})",
            initial_flags & CPU_FLAG_INT_ENABLE != 0,
            final_flags & CPU_FLAG_INT_ENABLE != 0
        );
    }
    if flag_diff & CPU_FLAG_DIRECTION != 0 {
        trace!(
            log,
            "DIRECTION flag changed. (initial:{}, final:{})",
            initial_flags & CPU_FLAG_DIRECTION != 0,
            final_flags & CPU_FLAG_DIRECTION != 0
        );
    }
    if flag_diff & CPU_FLAG_OVERFLOW != 0 {
        trace!(
            log,
            "OVERFLOW flag changed. (initial:{}, final:{})",
            initial_flags & CPU_FLAG_OVERFLOW != 0,
            final_flags & CPU_FLAG_OVERFLOW != 0
        );
    }
}

pub fn validate_registers(
    cpu_type: CpuType,
    metadata: &Metadata,
    prefix_opt: Option<u8>,
    opcode: u8,
    extension_opt: Option<u8>,
    test_regs: &VRegisters,
    cpu_regs: &VRegisters,
    log: &mut BufWriter<File>,
) -> RegisterValidationResult {
    let mut regs_validate = true;
    let mut flags_validate = true;

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
    if test_regs.ip != cpu_regs.ip {
        trace_error!(
            log,
            "IP mismatch: TEST: {:04X}[{}], CPU: {:04X}[{}]",
            test_regs.ip,
            test_regs.ip,
            cpu_regs.ip,
            cpu_regs.ip
        );
        regs_validate = false;
    }

    let mut opcode_key = format!("{:02X}", opcode);
    if let Some(prefix) = prefix_opt {
        opcode_key = format!("{:02X}{:02X}", prefix, opcode);
    }

    let opcode_inner = metadata
        .get(&opcode_key)
        .expect(&format!("{:02X}| No metadata for opcode", opcode));

    //log::warn!("opcode_inner: {:?}", opcode_inner);

    let opcode_final;

    if let Some(extension) = extension_opt {
        let extension_key = format!("{:1X}", extension);

        if let Some(reg) = &opcode_inner.reg {
            opcode_final = reg.get(&extension_key).expect(&format!(
                "{:02X}.{:1X}| No metadata for opcode extension",
                opcode, extension
            ));
        }
        else {
            trace_error!(log, "no 'reg' entry for extension!");
            panic!("no 'reg' entry for extension!");
        }
    }
    else {
        opcode_final = opcode_inner;
    }

    let flags_mask = opcode_final.flags_mask.unwrap_or(0xFFFF) as u16;
    if opcode_final.flags_mask.is_none() {
        trace!(log, "No undefined flags found in metadata for opcode {:02X}", opcode);
    }
    else {
        trace!(log, "Using defined flag mask from metadata: {:04X}", flags_mask);
    }

    let defined_test_flags = test_regs.flags & flags_mask;
    let defined_cpu_flags = cpu_regs.flags & flags_mask;

    let undefined_test_flags = test_regs.flags & !flags_mask;
    let undefined_cpu_flags = cpu_regs.flags & !flags_mask;
    let masked_flags_match = compare_flags(defined_test_flags, defined_cpu_flags, true, log);
    let undefined_flags_match = compare_flags(undefined_test_flags, undefined_cpu_flags, false, log);

    flags_validate = masked_flags_match && undefined_flags_match;

    match (regs_validate, flags_validate) {
        (true, true) => RegisterValidationResult::Ok,
        (false, true) => RegisterValidationResult::GeneralMismatch,
        (true, false) => RegisterValidationResult::FlagMismatch(masked_flags_match, undefined_flags_match),
        (false, false) => {
            log::warn!(">>>>> Both mismatch!");
            RegisterValidationResult::BothMismatch
        }
    }
}

pub fn compare_flags(test_flags: u16, cpu_flags: u16, defined: bool, log: &mut BufWriter<File>) -> bool {
    let defined_string = if defined { "DEFINED" } else { "UNDEFINED" };

    if test_flags != cpu_flags {
        trace_error!(
            log,
            "{} CPU flags mismatch! EMU: 0b{:08b} != CPU: 0b{:08b}",
            defined_string,
            test_flags,
            cpu_flags
        );

        let flag_diff = test_flags ^ cpu_flags;

        if flag_diff & CPU_FLAG_CARRY != 0 {
            trace_error!(
                log,
                "{} CARRY flag differs (cpu:{}, test:{})",
                defined_string,
                cpu_flags & CPU_FLAG_CARRY != 0,
                test_flags & CPU_FLAG_CARRY != 0
            );
        }
        if flag_diff & CPU_FLAG_PARITY != 0 {
            trace_error!(
                log,
                "{} PARITY flag differs. (cpu:{}, test:{})",
                defined_string,
                cpu_flags & CPU_FLAG_PARITY != 0,
                test_flags & CPU_FLAG_PARITY != 0
            );
        }
        if flag_diff & CPU_FLAG_AUX_CARRY != 0 {
            trace_error!(
                log,
                "{} AUX CARRY flag differs. (cpu:{}, test:{})",
                defined_string,
                cpu_flags & CPU_FLAG_AUX_CARRY != 0,
                test_flags & CPU_FLAG_AUX_CARRY != 0
            );
        }
        if flag_diff & CPU_FLAG_ZERO != 0 {
            trace_error!(
                log,
                "{} ZERO flag differs. (cpu:{}, test:{})",
                defined_string,
                cpu_flags & CPU_FLAG_ZERO != 0,
                test_flags & CPU_FLAG_ZERO != 0
            );
        }
        if flag_diff & CPU_FLAG_SIGN != 0 {
            trace_error!(
                log,
                "{} SIGN flag differs. (cpu:{}, test:{})",
                defined_string,
                cpu_flags & CPU_FLAG_SIGN != 0,
                test_flags & CPU_FLAG_SIGN != 0
            );
        }
        if flag_diff & CPU_FLAG_TRAP != 0 {
            trace_error!(
                log,
                "{} TRAP flag differs. (cpu:{}, test:{})",
                defined_string,
                cpu_flags & CPU_FLAG_TRAP != 0,
                test_flags & CPU_FLAG_TRAP != 0
            );
        }
        if flag_diff & CPU_FLAG_INT_ENABLE != 0 {
            trace_error!(
                log,
                "{} INT flag differs. (cpu:{}, test:{})",
                defined_string,
                cpu_flags & CPU_FLAG_INT_ENABLE != 0,
                test_flags & CPU_FLAG_INT_ENABLE != 0
            );
        }
        if flag_diff & CPU_FLAG_DIRECTION != 0 {
            trace_error!(
                log,
                "{} DIRECTION flag differs. (cpu:{}, test:{})",
                defined_string,
                cpu_flags & CPU_FLAG_DIRECTION != 0,
                test_flags & CPU_FLAG_DIRECTION != 0
            );
        }
        if flag_diff & CPU_FLAG_OVERFLOW != 0 {
            trace_error!(
                log,
                "{} OVERFLOW flag differs. (cpu:{}, test:{})",
                defined_string,
                cpu_flags & CPU_FLAG_OVERFLOW != 0,
                test_flags & CPU_FLAG_OVERFLOW != 0
            );
        }
        false
    }
    else {
        true
    }
}

pub fn validate_cycles(
    cpu_states: &[CycleState],
    emu_states: &[CycleState],
    log: &mut BufWriter<File>,
) -> (bool, usize) {
    if emu_states.len() != cpu_states.len() {
        // Cycle count mismatch
        return (false, 0);
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

            return (false, i);
        }
    }

    (true, 0)
}

pub fn validate_memops(
    cpu: &CpuDispatch,
    test_cycles: &Vec<CycleState>,
    cycles: &Vec<CycleState>,
    instr_addr: u32,
    instr_size: usize,
    initial_ram: &Vec<[u32; 2]>,
    final_ram: &Vec<[u32; 2]>,
    flags_on_stack: bool,
    prefetch_len: usize,
    log: &mut BufWriter<File>,
) -> Result<(), Error> {
    // Calculate stack address for flags
    let flat_stack_addr = cpu.flat_sp();
    // Flags should be 6 bytes behind the top of the stack
    let flags_addr = flat_stack_addr.wrapping_add(4);

    if flags_on_stack {
        _ = writeln!(log, "validate_memory(): flags on stack at addr {:06X}", flags_addr);
    }

    let mut read_ops: Vec<BusOp> = Vec::new();
    let mut write_ops: Vec<BusOp> = Vec::new();
    let mut fetch_ops: Vec<BusOp> = Vec::new();

    // We will track memory operations with a HashMap by address. This assumes that a single memory
    // address can only be read from or written to once during an instruction execution.
    let mut read_mem_states: HashMap<u32, MemOpValidationState> = HashMap::new();
    let mut write_mem_states: HashMap<u32, MemOpValidationState> = HashMap::new();

    for entry in initial_ram.iter() {
        let addr = entry[0];
        trace!(log, "Initial RAM state: {:06X} <= {:02X}", addr, entry[1] as u8);
        read_mem_states.insert(
            addr,
            MemOpValidationState {
                in_test_state: true,
                in_cycles: false,
                is_fetch: false,
            },
        );
    }

    for entry in final_ram.iter() {
        let addr = entry[0];
        trace!(log, "Final RAM state: {:06X} <= {:02X}", addr, entry[1] as u8);
        write_mem_states.insert(
            addr,
            MemOpValidationState {
                in_test_state: true,
                in_cycles: false,
                is_fetch: false,
            },
        );
    }

    if prefetch_len == 0 {
        // The first byte of the instruction has already been fetched by the time we start tracking. Mark it as seen.
        if let Some(entry) = read_mem_states.get_mut(&instr_addr) {
            entry.in_cycles = true;
        }
        else {
            panic!("Instruction address not present in initial ram state!")
        }
    }
    else {
        // Mark the first prefetch_len bytes as seen. As of test generation v2 the ram state is in
        // order of operation, so we know the first ram entries represent prefetched bytes and won't
        // be seen in memops.
        for i in 0..prefetch_len {
            if let Some(entry) = read_mem_states.get_mut(&initial_ram[i][0]) {
                entry.in_cycles = true;
            }
            else {
                panic!("Prefetched address not present in initial ram state!")
            }
        }
    }

    // Assume we are starting out in a CODE fetch if the segment selector is CS on the first cycle.
    let mut in_code_fetch = false;
    if cycles[0].a_type == AccessType::CodeOrNone {
        in_code_fetch = true;
    }
    for cycle in cycles {
        // Track CODE fetch bus cycles so we can ignore those reads.
        if cycle.b_state == BusState::CODE {
            in_code_fetch = true;
        }
        else if cycle.b_state != BusState::PASV {
            in_code_fetch = false;
        }

        // We know when a read or write occurs because the bus state will go PASV. Bus state will
        // remain active during wait states.
        if !cycle.mrdc && cycle.b_state == BusState::PASV {
            if !in_code_fetch {
                read_ops.push(BusOp {
                    op_type: BusOpType::MemRead,
                    addr:    cycle.addr,
                    data:    cycle.data_bus as u8,
                    flags:   0,
                });
            }
            else {
                fetch_ops.push(BusOp {
                    op_type: BusOpType::CodeRead,
                    addr:    cycle.addr,
                    data:    cycle.data_bus as u8,
                    flags:   0,
                });
            }
        }
        else if !cycle.mwtc && cycle.b_state == BusState::PASV {
            write_ops.push(BusOp {
                op_type: BusOpType::MemWrite,
                addr:    cycle.addr,
                data:    cycle.data_bus as u8,
                flags:   0,
            });
        }
    }

    for op in fetch_ops {
        if let Some(entry) = read_mem_states.get_mut(&op.addr) {
            entry.is_fetch = true;
        }
    }

    for op in read_ops {
        if let Some(entry) = read_mem_states.get_mut(&op.addr) {
            if entry.in_test_state {
                entry.in_cycles = true;
            }
            else {
                bail!("Read operation at addr {:06X} not in initial ram state!", op.addr);
            }
        }
        else {
            bail!("Read operation at addr {:06X} not in initial ram state!", op.addr);
        }
    }

    // Check that write operations are present in the 'final' ram state.
    // There is one condition where a write operation may not be present in the final ram state,
    // when the value being written is the same as the initial value. We need to ignore this
    // condition.
    for op in write_ops {
        if let Some(entry) = write_mem_states.get_mut(&op.addr) {
            if entry.in_test_state {
                entry.in_cycles = true;
            }
            else {
                if let Some(read_entry) = read_mem_states.get(&op.addr) {
                    if read_entry.in_test_state && read_entry.in_cycles {
                        // This is a write operation that is the same as the initial value. Ignore it.
                        trace!(
                            log,
                            "Ignoring missing write operation at addr {:06X} that is the same as the initial value.",
                            op.addr
                        );
                        continue;
                    }
                }
                else {
                    bail!("Write operation at addr {:06X} not in final ram state (2)!", op.addr);
                }
            }
        }
        else {
            if let Some(read_entry) = read_mem_states.get(&op.addr) {
                if read_entry.in_test_state && read_entry.in_cycles {
                    // This is a write operation that is the same as the initial value. Ignore it.
                    trace!(
                        log,
                        "Ignoring missing write operation at addr {:06X} that is the same as the initial value.",
                        op.addr
                    );
                    continue;
                }
            }
            else {
                bail!("Write operation at addr {:06X} not in final ram state (2)!", op.addr);
            }
        }
    }

    for state in read_mem_states.iter() {
        trace!(
            log,
            "Memory address {:06X} in test state: {:05} in cycles: {:05} is fetch: {:05}",
            state.0,
            state.1.in_test_state,
            state.1.in_cycles,
            state.1.is_fetch,
        )
    }

    // Check that all memory addresses in initial state were accessed in cycle execution.
    for state in read_mem_states.iter() {
        if !state.1.is_fetch && !(state.1.in_test_state && state.1.in_cycles) {
            // TODO: Scan for fetches in test state and ignore them
            //bail!("Memory READ {:06X} not read during instruction execution!", state.0);
        }
    }

    // Check that all memory addresses in final state were accessed in cycle execution.
    for state in write_mem_states.iter() {
        if !(state.1.in_test_state && state.1.in_cycles) {
            bail!("Memory WRITE {:06X} not written during instruction execution!", state.0);
        }
    }

    // All entries in the final ram entries should correspond to bus writes.
    /*    for (i, ram_entry) in final_ram.iter().enumerate() {
        let addr = ram_entry[0];
        let data = ram_entry[1];

        if write_ops[i].addr != addr || write_ops[i].data != data as u8 {
            bail!(
                "Final RAM write mismatch at addr {:06X}: {:02X} vs {:02X}",
                addr,
                write_ops[i].data,
                data
            );
        }
    }*/

    Ok(())
}

pub fn validate_memory(
    cpu: &CpuDispatch,
    final_ram: &Vec<[u32; 2]>,
    flags_on_stack: bool,
    log: &mut BufWriter<File>,
) -> Result<(), Error> {
    let _ignore_next_ops = 0;

    // Calculate stack address for flags
    let flat_stack_addr = cpu.flat_sp();
    // Flags should be 6 bytes behind the top of the stack
    let flags_addr = flat_stack_addr.wrapping_add(4);

    if flags_on_stack {
        _ = writeln!(log, "validate_memory(): flags on stack at addr {:06X}", flags_addr);
    }

    // Validate final memory state.
    for mem_entry in final_ram {
        // Validate that mem_entry[0] < 0xFFFFF
        if mem_entry[0] > 0xFFFFF {
            bail!("Memory address out of range: {:?}", mem_entry[0]);
        }

        // Stack pointer may be unaligned.
        if flags_on_stack && ((mem_entry[0] == flags_addr) || (mem_entry[0] == flags_addr + 1)) {
            // This is a write of the flags to the stack. Ignore it.
            continue;
        }

        let addr: usize = mem_entry[0] as usize;

        let byte: u8 = match mem_entry[1].try_into() {
            Ok(byte) => byte,
            Err(e) => {
                bail!("Invalid memory byte value: {:?}: {}", mem_entry[1], e);
            }
        };

        let mem_byte = match cpu.bus().peek_u8(addr) {
            Ok(byte) => byte,
            Err(e) => {
                bail!("Failed to peek mem from CPU: {}", e);
            }
        };

        if byte != mem_byte {
            bail!(
                "Address: {:05X} Test value: {:02X} Actual value: {:02X}",
                addr,
                byte,
                mem_byte
            );
        }
    }
    Ok(())
}

pub fn clean_cycle_states(states: &mut Vec<CycleState>) -> usize {
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

    for state in states {
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

    trimmed_ct
}

pub fn print_cycle_diff(log: &mut BufWriter<File>, test_states: &Vec<CycleState>, cpu_states: &[CycleState]) {
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

        _ = writeln!(log, "{:<80} | {:<80}", cpu_str, emu_str);
    }
}

pub fn print_summary(summary: &TestResultSummary) {
    // Collect and sort keys
    let mut keys: Vec<_> = summary.results.keys().collect();
    keys.sort();

    // Iterate using sorted keys
    for key in keys {
        if let Some(result) = summary.results.get(key) {
            let filename = format!("{:?}", key);
            println!(
                "File: {:15} Passed: {:6} Warning: {:6} Failed: {:6} Reg: {:6} Flags: {:6} UFlags: {:6} Mem: {:6} Cycle: {:6} ",
                filename.bright_blue(),
                result.passed,
                if result.warning > 0 {
                    format!("{:6}", result.warning.to_string().yellow())
                }
                else {
                    "0".to_string()
                },
                if result.failed > 0 {
                    format!("{:6}", result.failed.to_string().red())
                }
                else {
                    "0".to_string()
                },
                if result.reg_mismatch > 0 {
                    format!("{:6}", result.reg_mismatch.to_string().red())
                }
                else {
                    "0".to_string()
                },
                if result.flag_mismatch > 0 {
                    format!("{:6}", result.flag_mismatch.to_string().red())
                }
                else {
                    "0".to_string()
                },
                if result.undef_flag_mismatch > 0 {
                    format!("{:6}", result.undef_flag_mismatch.to_string().red())
                }
                else {
                    "0".to_string()
                },
                if result.mem_mismatch > 0 {
                    format!("{:6}", result.mem_mismatch.to_string().red())
                }
                else {
                    "0".to_string()
                },
                if result.cycle_mismatch > 0 {
                    let pf_accuracy = (result.cycles.prefetched.cpu as f64/ result.cycles.prefetched.test as f64) * 100.0;
                    let n_accuracy = (result.cycles.normal.cpu as f64/ result.cycles.normal.test as f64) * 100.0;
                    format!("{:6} n:({:3.2}%) p:({:3.2}%)", result.cycle_mismatch.to_string().red(), n_accuracy, pf_accuracy)
                }
                else {
                    "0".to_string()
                },
            );
        }
    }
}

pub(crate) fn print_cycle_summary(test_cycles: usize, cpu_cycles: usize) {
    println!("Totals: test cycles: {} Emulator cycles: {}", test_cycles, cpu_cycles);

    let diff = test_cycles as f64 - cpu_cycles as f64;

    if diff > 0.0 {
        println!(
            "Emulator less than test cycles by: {}, accuracy: {:.3}%",
            diff,
            cpu_cycles as f64 / test_cycles as f64 * 100.0
        );
    }
    else if diff < 0.0 {
        println!(
            "Emulator cycles exceeded test cycles by: {}, accuracy: {:.3}%",
            diff.abs(),
            cpu_cycles as f64 / test_cycles as f64 * 100.0
        );
    }
    else {
        println!(
            "Test cycles matched Emulator cycles! ({} == {})",
            test_cycles, cpu_cycles
        );
    }
}

pub fn write_summary<W: Write>(summary: &TestResultSummary, output: &mut W) {
    // Collect and sort keys
    let mut keys: Vec<_> = summary.results.keys().collect();
    keys.sort();

    // Print header
    _ = writeln!(output, "file,passed,warning,failed,reg,flags,uflags,mem,cycle");

    // Iterate using sorted keys
    for key in keys {
        if let Some(result) = summary.results.get(key) {
            let filename = format!("{:?}", key);
            _ = writeln!(
                output,
                "{:15},{:6},{:6},{:6},{:6},{:6},{:6},{:6},{:6}",
                filename,
                result.passed,
                if result.warning > 0 {
                    format!("{:6}", result.warning.to_string())
                }
                else {
                    "0".to_string()
                },
                if result.failed > 0 {
                    format!("{:6}", result.failed.to_string())
                }
                else {
                    "0".to_string()
                },
                if result.reg_mismatch > 0 {
                    format!("{:6}", result.reg_mismatch.to_string())
                }
                else {
                    "0".to_string()
                },
                if result.flag_mismatch > 0 {
                    format!("{:6}", result.flag_mismatch.to_string())
                }
                else {
                    "0".to_string()
                },
                if result.undef_flag_mismatch > 0 {
                    format!("{:6}", result.undef_flag_mismatch.to_string())
                }
                else {
                    "0".to_string()
                },
                if result.mem_mismatch > 0 {
                    format!("{:6}", result.mem_mismatch.to_string())
                }
                else {
                    "0".to_string()
                },
                if result.cycle_mismatch > 0 {
                    format!("{:6}", result.cycle_mismatch.to_string())
                }
                else {
                    "0".to_string()
                },
            );
        }
    }
}

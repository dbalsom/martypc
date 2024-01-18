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
use anyhow::{bail, Error};
use colored::Colorize;
use marty_core::cpu_808x::*;
use std::{
    collections::{HashMap, LinkedList},
    ffi::OsString,
    fs::{read_to_string, File, OpenOptions},
    io::{BufReader, BufWriter, ErrorKind, Read, Seek, SeekFrom, Write},
    path::PathBuf,
    time::Duration,
};

use flate2::read::GzDecoder;
use marty_core::cpu_validator::{BusCycle, BusState, CycleState, VRegisters};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TestState {
    pub regs:  VRegisters,
    pub ram:   Vec<[u32; 2]>,
    pub queue: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct CpuTest {
    pub name:  String,  // Human readable name (disassembly)
    pub bytes: Vec<u8>, // Instruction bytes

    #[serde(rename = "initial")]
    pub initial_state: TestState, // Initial state of CPU before test execution

    #[serde(rename = "final")]
    pub final_state: TestState, // Final state of CPU after test execution

    pub cycles: Vec<CycleState>,

    pub test_hash: String,
}

pub enum FailType {
    CycleMismatch,
    MemMismatch,
    RegMismatch,
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

pub struct TestResult {
    pub pass: bool,
    pub duration: Duration,
    pub passed: u32,
    pub warning: u32,
    pub failed: u32,
    pub cycle_mismatch: u32,
    pub mem_mismatch: u32,
    pub reg_mismatch: u32,
    pub warn_tests: LinkedList<TestFailItem>,
    pub failed_tests: LinkedList<TestFailItem>,
}

pub struct TestResultSummary {
    pub results: HashMap<OsString, TestResult>,
}

#[derive(Deserialize, Debug)]
pub struct InnerObject {
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags_mask: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reg: Option<HashMap<String, InnerObject>>,
}

pub type Metadata = HashMap<String, InnerObject>;

#[macro_export]
macro_rules! trace_error {
    ($wr:expr, $($t:tt)*) => {{
        let formatted_message = format!($($t)*);
        log::error!("{}", &formatted_message);

        // Assuming you want to write the message to the BufWriter as well.
        writeln!($wr, "{}", &formatted_message).expect("Failed to write to BufWriter");
        _ = $wr.flush();
    }};
}

#[macro_export]
macro_rules! trace_print {
    ($wr:expr, $($t:tt)*) => {{
        let formatted_message = format!($($t)*);
        println!("{}", &formatted_message);

        // Assuming you want to write the message to the BufWriter as well.
        writeln!($wr, "{}", &formatted_message).expect("Failed to write to BufWriter");
        _ = $wr.flush();
    }};
}

pub fn opcode_from_path(path: &PathBuf) -> Option<u8> {
    path.file_stem() // Get the filename without the extension
        .and_then(|os_str| os_str.to_str()) // Convert OsStr to &str
        .and_then(|filename| {
            let hex_str = &filename[0..2]; // Take the first two characters
            u8::from_str_radix(hex_str, 16).ok() // Parse as hexadecimal
        })
}

pub fn opcode_extension_from_path(path: &PathBuf) -> Option<u8> {
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

pub fn validate_registers(
    metadata: &Metadata,
    opcode: u8,
    extension_opt: Option<u8>,
    mask: bool,
    test_regs: &VRegisters,
    cpu_regs: &VRegisters,
    log: &mut BufWriter<File>,
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
    if test_regs.ip != cpu_regs.ip {
        log::warn!(
            "IP mismatch: {:04X}[{}] vs {:04X}[{}]",
            test_regs.ip,
            test_regs.ip,
            cpu_regs.ip,
            cpu_regs.ip
        );
        regs_validate = false;
    }

    let opcode_key = format!("{:02X}", opcode);

    let opcode_inner = metadata
        .get(&opcode_key)
        .expect(&format!("{:02X}| No metadata for opcode", opcode));
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

    let flags_mask = if mask {
        opcode_final.flags_mask.unwrap_or(0xFFFF) as u16
    }
    else {
        0xFFFF
    };

    let test_flags_masked = test_regs.flags & flags_mask;
    let cpu_flags_masked = test_regs.flags & flags_mask;

    if test_flags_masked != cpu_flags_masked {
        trace_error!(
            log,
            "CPU flags mismatch! EMU: 0b{:08b} != CPU: 0b{:08b}",
            test_flags_masked,
            cpu_flags_masked
        );
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

pub fn validate_memory(
    cpu: &Cpu,
    final_ram: &Vec<[u32; 2]>,
    flags_on_stack: bool,
    log: &mut BufWriter<File>,
) -> Result<(), Error> {
    let mut ignore_next_ops = 0;

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
                "File: {:15} Passed: {:6} Warning: {:6} Failed: {:6} Reg: {:6} Cycle: {:6} Mem: {:6}",
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
                if result.cycle_mismatch > 0 {
                    format!("{:6}", result.cycle_mismatch.to_string().red())
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
            );
        }
    }
}

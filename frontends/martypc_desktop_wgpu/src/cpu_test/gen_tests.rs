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

    gen_tests.rs - Implement the main procedure for JSON test generation mode.
                   Requires CPU validator feature.
*/

use std::{
    cell::RefCell,
    collections::{HashMap, LinkedList},
    fs::{File, OpenOptions},
    io::{BufReader, BufWriter, ErrorKind, Seek, SeekFrom, Write},
    path::PathBuf,
    rc::Rc,
    time::Instant,
};

use config_toml_bpaf::ConfigFileParams;

use marty_core::{
    arduino8088_validator::ArduinoValidator,
    bytequeue::ByteQueue,
    cpu_808x::{mnemonic::Mnemonic, Cpu, *},
    cpu_common::{CpuOption, CpuType, TraceMode},
    cpu_validator::{BusCycle, BusOp, BusOpType, BusState, CpuValidator, CycleState},
    devices::pic::Pic,
    tracelogger::TraceLogger,
};

use crate::cpu_test::common::{clean_cycle_states, write_tests_to_file, CpuTest, TestState};

use serde::{Deserialize, Serialize};

pub fn run_gentests(config: &ConfigFileParams) {
    //let pic = Rc::new(RefCell::new(Pic::new()));

    // Create the cpu trace file, if specified
    let mut cpu_trace = TraceLogger::None;
    if let Some(trace_filename) = &config.machine.cpu.trace_file {
        cpu_trace = TraceLogger::from_filename(&trace_filename);
    }

    // Create the validator trace file, if specified
    let mut validator_trace = TraceLogger::None;
    if let Some(trace_filename) = &config.validator.trace_file {
        validator_trace = TraceLogger::from_filename(&trace_filename);
    }

    #[cfg(feature = "cpu_validator")]
    use marty_core::cpu_validator::ValidatorMode;

    let trace_mode = config.machine.cpu.trace_mode.unwrap_or_default();

    let mut cpu = Cpu::new(
        CpuType::Intel8088,
        trace_mode,
        cpu_trace,
        #[cfg(feature = "cpu_validator")]
        config.validator.vtype.unwrap(),
        #[cfg(feature = "cpu_validator")]
        validator_trace,
        #[cfg(feature = "cpu_validator")]
        ValidatorMode::Instruction,
        #[cfg(feature = "cpu_validator")]
        config.validator.baud_rate.unwrap_or(1_000_000),
    );

    if let Some(seed) = config.tests.test_seed {
        log::debug!("Using random seed from config: {}", seed);
        cpu.randomize_seed(seed);
    }
    else {
        cpu.randomize_seed(1234);
    }

    cpu.randomize_mem();

    let mut test_num = 0;
    let mut opcode_range_start = 0;
    let mut opcode_range_end = 0xFF;

    // Generate the list of opcodes we are going to generate tests for.
    if let Some(test_opcode_range) = &config.tests.test_opcode_range {
        if test_opcode_range.len() > 1 {
            opcode_range_start = test_opcode_range[0];
            opcode_range_end = test_opcode_range[1];
        }
        else {
            log::error!("Invalid opcode range specified.");
            return;
        }
    }

    let mut opcode_list = Vec::from_iter(opcode_range_start..=opcode_range_end);

    let mut opcode_range_exclude = Vec::new();

    if let Some(test_opcode_exclude_list) = &config.tests.test_opcode_exclude_list {
        opcode_range_exclude = test_opcode_exclude_list.clone();
    }

    opcode_range_exclude.append(&mut vec![
        0x26, 0x2E, 0x36, 0x3E, // Segment override prefixes
        0x9B, // WAIT instruction
        //0x9D, // POPF (figure out a way to handle this?)
        0xF0, 0xF1, 0xF2, 0xF3, // Prefixes
        0xF4,
    ]);

    opcode_list.retain(|&x| !opcode_range_exclude.contains(&x));

    let test_append = config.tests.test_opcode_gen_append.unwrap_or(true);
    let test_limit = config.tests.test_opcode_gen_count.unwrap_or(5000);
    println!("Using test limit: {}", test_limit);

    let mut test_path_postfix = "tests".to_string();
    if let Some(test_dir) = &config.tests.test_dir {
        test_path_postfix = test_dir.clone();
    }

    let mut test_base_path = PathBuf::new();
    test_base_path.push(config.emulator.basedir.clone());
    test_base_path.push(test_path_postfix);

    for test_opcode in opcode_list {
        let is_grp = ArduinoValidator::is_group_opcode(test_opcode);

        let mut start_ext = 0;
        let mut end_ext = if is_grp { 7 } else { 0 };

        if let Some(range) = &config.tests.test_extension_range {
            if range.len() < 2 {
                panic!("Invalid test opcode extension range specified!");
            }

            start_ext = range[0];
            end_ext = range[1];
        }

        for op_ext in start_ext..=end_ext {
            let test_start_instant = Instant::now();

            // Attempt to open the json file for this opcode.

            // First, generate the appropriate filename.
            // If group opcode, (XX.Y.json where XX == opcode in hex, Y == opcode extension)
            // Otherwise, XX.json

            let mut test_path = test_base_path.clone();
            //log::debug!("Using base path: {:?}", test_path);

            if !is_grp {
                test_path.push(&format!("{:02X}.json", test_opcode));
            }
            else {
                test_path.push(&format!("{:02X}.{:01X}.json", test_opcode, op_ext));
            }

            log::debug!("Using filename: {:?}", test_path);

            let mut test_file_opt: Option<File> = None;
            let mut tests: LinkedList<CpuTest>;

            let mut advance_rng_ct = 0;

            // If we are not appending tests, don't bother to open the existing test file.
            if test_append {
                let file_result = File::open(test_path.clone());

                let mut had_to_create = false;

                match file_result {
                    Ok(file) => {
                        if !is_grp {
                            println!(
                                "Opened existing test file for opcode {:02X}: {:?}",
                                test_opcode, test_path
                            );
                        }
                        else {
                            println!(
                                "Opened existing test file for opcode {:02X}.{:01X}: {:?}",
                                test_opcode, op_ext, test_path
                            );
                        }

                        test_file_opt = Some(file);
                    }
                    Err(error) => match error.kind() {
                        ErrorKind::NotFound => {
                            println!("File not found: {:?} Attempting to create.", test_path);

                            match File::create(test_path.clone()) {
                                Ok(file) => {
                                    println!("Created test file: {:?}", test_path);

                                    test_file_opt = Some(file);
                                    had_to_create = true;
                                }
                                Err(err) => {
                                    eprintln!("Failed to create test file: {:?}: {:?}", test_path, err);
                                    return;
                                }
                            }
                        }
                        error => {
                            println!("Failed to open the file due to: {:?}", error);
                        }
                    },
                }

                if test_file_opt.is_none() {
                    return;
                }

                // We should have a valid file now
                let test_file = test_file_opt.unwrap();

                if !had_to_create {
                    tests = read_tests_from_file(&test_file, test_path.clone())
                        .expect("Failed to read tests from JSON file.");
                }
                else {
                    tests = LinkedList::new();
                }
            }
            else {
                // Not appending tests. Just create an empty test vec.
                tests = LinkedList::new();
            }

            // We should have a vector of tests now.
            println!("Loaded {} tests from file.", tests.len());

            //test_num = tests.len() as u32;
            advance_rng_ct = tests.len() as u32;

            'testloop: while test_num < test_limit {
                cpu.reset();
                cpu.randomize_mem();
                cpu.randomize_regs();

                let mut instruction_address =
                    Cpu::calc_linear_address(cpu.get_register16(Register16::CS), cpu.get_register16(Register16::IP));

                while (cpu.get_register16(Register16::IP) > 0xFFF0) || ((instruction_address & 0xFFFFF) > 0xFFFF0) {
                    // Avoid IP wrapping issues for now
                    cpu.randomize_regs();
                    instruction_address = Cpu::calc_linear_address(
                        cpu.get_register16(Register16::CS),
                        cpu.get_register16(Register16::IP),
                    );
                }

                test_num += 1;

                // Is the specified opcode a group instruction?
                if is_grp {
                    cpu.random_grp_instruction(test_opcode, &[op_ext]);
                }
                else {
                    cpu.random_inst_from_opcodes(&[test_opcode]);
                }

                // Decode this instruction
                instruction_address =
                    Cpu::calc_linear_address(cpu.get_register16(Register16::CS), cpu.get_register16(Register16::IP));

                cpu.bus_mut().seek(instruction_address as usize);
                let opcode = cpu.bus().peek_u8(instruction_address as usize).expect("mem err");

                let mut i = match Cpu::decode(cpu.bus_mut()) {
                    Ok(i) => i,
                    Err(_) => {
                        log::error!("Instruction decode error, skipping...");
                        continue 'testloop;
                    }
                };

                // Replicate RNG for existing test, but don't re-generate test. Skip ahead.
                // This allows us to seamlessly resume test set generation, in theory.
                if test_num < advance_rng_ct {
                    continue;
                }

                cpu.set_option(CpuOption::EnableWaitStates(false));
                cpu.set_option(CpuOption::TraceLoggingEnabled(config.machine.cpu.trace_on));

                let mut rep = false;

                i.address = instruction_address;

                println!(
                    "Test {}: Creating test for instruction: {} opcode:{:02X} addr:{:05X}",
                    test_num, i, opcode, i.address
                );

                // Set terminating address for CPU validator.

                let end_address = Cpu::calc_linear_address(
                    cpu.get_register16(Register16::CS),
                    cpu.get_register16(Register16::IP).wrapping_add(i.size as u16),
                );

                cpu.set_end_address(end_address as usize);
                log::trace!("Setting end address: {:05X}", end_address);

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
                    Mnemonic::SETMO
                    | Mnemonic::SETMOC
                    | Mnemonic::ROL
                    | Mnemonic::ROR
                    | Mnemonic::RCL
                    | Mnemonic::RCR
                    | Mnemonic::SHL
                    | Mnemonic::SHR
                    | Mnemonic::SAR => {
                        // Limit cl to 0-63.
                        cpu.set_register8(Register8::CL, cpu.get_register8(Register8::CL) & 0x3F);
                        //cpu.set_register8(Register8::CL, 3);

                        log::debug!("SHIFT OP: CL is {:02X}", cpu.get_register8(Register8::CL));
                    }
                    _ => {}
                }

                // We loop here to handle REP string instructions, which are broken up into 1 effective instruction
                // execution per iteration. The 8088 makes no such distinction.
                loop {
                    match cpu.step(false) {
                        Ok((_, cycles)) => {
                            //log::trace!("Instruction reported {} cycles", cycles);
                            if rep & cpu.in_rep() {
                                continue;
                            }
                            break;
                        }
                        Err(err) => {
                            log::error!("CPU Error: {}\n", err);
                            cpu.trace_flush();
                            break 'testloop;
                        }
                    }
                }

                // Finalize instruction.
                _ = cpu.step_finish();

                let validator = cpu.get_validator().as_ref().unwrap();

                let cpu_test = get_test_info(validator);

                tests.push_front(cpu_test);

                // Write every 1000 tests to file.
                if tests.len() % 1000 == 0 {
                    write_tests_to_file(test_path.clone(), &tests);
                }
            }

            let test_elapsed = test_start_instant.elapsed().as_secs_f32();

            println!(
                "Test generation complete for opcode: {:02}. Generated {} tests in {:.2} seconds",
                test_opcode, test_num, test_elapsed
            );
            let avg_test_elapsed = test_elapsed / test_num as f32;
            println!("Avg test time: {:.2}", avg_test_elapsed);

            write_tests_to_file(test_path, &tests);
        }
    }

    //std::process::exit(0);
}

pub fn read_tests_from_file(file: &File, path: PathBuf) -> Option<LinkedList<CpuTest>> {
    // Scope for BufReader
    let json_reader = BufReader::new(file);

    let tests = match serde_json::from_reader(json_reader) {
        Ok(json_obj) => Some(json_obj),
        Err(e) if e.is_eof() => {
            println!("File is empty. Creating new LinkedList.");
            Some(LinkedList::new())
        }
        Err(e) => {
            eprintln!("Failed to read json from file: {:?}: {:?}", path, e);
            None
        }
    };

    tests
}

pub fn get_test_info(validator: &Box<dyn CpuValidator>) -> CpuTest {
    let name = validator.name();
    let bytes = validator.instr_bytes();

    let initial_regs = validator.initial_regs();
    let final_regs = validator.final_regs();

    let cpu_ops = validator.cpu_ops();
    let cpu_reads = validator.cpu_reads();

    //log::debug!("Got {} CPU reads from instruction.", cpu_reads.len());

    let (initial_state, initial_ram) = initial_state_from_ops(initial_regs.cs, initial_regs.ip, &bytes, &cpu_ops);

    //let mut read_ram = ram_from_reads(cpu_reads);
    //initial_ram.append(&mut read_ram);

    let final_ram = final_state_from_ops(initial_state, cpu_ops);

    let mut cycle_states = validator.cycle_states().clone();
    if cycle_states.is_empty() {
        panic!("Got 0 cycles from instruction!");
    }
    let initial_queue = cycle_states[0].queue_vec();
    let mut final_queue = cycle_states[cycle_states.len() - 1].queue_vec();

    // The instruction ended when the byte for the next instruction was fetched from the queue.
    // Reflect this read by popping a byte from the final_queue.
    // This should always work; as we should always terminate with at least one byte in the queue
    _ = final_queue.pop();

    clean_cycle_states(&mut cycle_states);

    log::debug!("Got {} CPU cycles from instruction.", cycle_states.len());

    if cycle_states.len() == 0 {
        panic!("Got 0 cycles from instruction!");
    }

    CpuTest {
        name,
        bytes,
        initial_state: TestState {
            regs:  initial_regs,
            ram:   initial_ram,
            queue: initial_queue,
        },
        final_state: TestState {
            regs:  final_regs,
            ram:   final_ram,
            queue: final_queue,
        },
        cycles: cycle_states,
        test_hash: String::new(),
    }
}

/// Try to calculate the initial memory state from a list of Bus operations.
/// This is harder than anticipated due to the particular fetch behavior of the validator.
/// The validator feeds NOPs to the CPU for every fetch after the last instruction byte
/// of the instruction being validated.
/// In contrast, the emulator will continue to fetch from memory. We can substitute the
/// bytes fetched by the emulator, but only if those bytes haven't been modified by the
/// instruction prior to be fetched!
///
/// If we do detect self modifying code, we can insert random bytes(?) as the original
/// value doesn't matter
pub fn initial_state_from_ops(
    cs: u16,
    ip: u16,
    instr_bytes: &Vec<u8>,
    all_ops: &Vec<BusOp>,
) -> (HashMap<u32, u8>, Vec<[u32; 2]>) {
    //let mut ram_ops = all_ops.clone();
    //let mut ram: Vec<[u32; 2]> = Vec::new();

    let mut initial_state: HashMap<u32, u8> = HashMap::new();
    let mut code_addresses: HashMap<u32, (u8, bool)> = HashMap::new();

    // Add the instruction bytes to the initial state. They cannot be modified
    // by the validated instruction because every instruction is done fetching
    // operands by the time it does any writes, so they had to be in the
    // initial state.
    let mut pc = ip;

    for byte in instr_bytes {
        let flat_addr = Cpu::calc_linear_address(cs, pc);
        code_addresses.insert(flat_addr, (*byte, true));
        initial_state.insert(flat_addr, *byte);
        pc = pc.wrapping_add(1);
    }

    let mut shadowed_addresses: HashMap<u32, bool> = HashMap::new();
    let mut read_addresses: HashMap<u32, u8> = HashMap::new();
    let mut write_addresses: HashMap<u32, u8> = HashMap::new();

    for op in all_ops {
        match op.op_type {
            BusOpType::MemRead => {
                read_addresses.insert(op.addr, op.data);

                if write_addresses.get(&op.addr).is_some() {
                    // Reading from an address the instruction wrote to (not sure if this ever happens?)
                    // In any case, don't add this to the initial state since it happened after a write.
                    log::debug!("Reading from written address: [{:05X}]:{:02X}!", op.addr, op.data);
                }
                else {
                    // This address was never written to, so the value here must have been part of the
                    // initial state.
                    initial_state.insert(op.addr, op.data);
                }
            }
            BusOpType::CodeRead => {
                if let Some((byte, flag)) = code_addresses.get(&op.addr) {
                    if *flag == true {
                        // This operation corresponds to an initial fetch.
                        // Just as a sanity check, compare bytes.
                        assert_eq!(*byte, op.data);
                        //log::debug!("Validated initial instruction fetch: [{:05X}]:{:02X}", op.addr, op.data);
                    }
                    else {
                        // How can we be fetching the same byte twice?
                        panic!("Illegal duplicate fetch!");
                    }
                }
                else {
                    // Fetch outside of instruction boundaries.

                    // Check if we are fetching from a shadowed address.
                    if shadowed_addresses.get(&op.addr).is_some() {
                        // We are fetching from an address we wrote to and don't know the value of.
                        log::debug!(
                            "Detected self modifying code! Fetch from: [{:05X}] was written to by BusOp.",
                            op.addr
                        );

                        // Initial state would have been NOP.
                        code_addresses.insert(op.addr, (0x90, false));
                    }
                    else {
                        // Address wasn't shadowed, so safe to add this fetch to the initial state.
                        //log::debug!("Adding subsequent instruction fetch to initial state [{:05X}]:{:02X}", op.addr, op.data);
                        initial_state.insert(op.addr, 0x90);
                    }
                }
            }
            BusOpType::MemWrite => {
                // Check if this address was read from previously.
                if read_addresses.get(&op.addr).is_some() || code_addresses.get(&op.addr).is_some() {
                    // Modifying a previously read address. This is fine.
                }
                else {
                    // This address was never read from, so this write shadows
                    // the original value at this address. Mark it as a
                    // shadowed address.
                    shadowed_addresses.insert(op.addr, true);

                    // Since this isn't a fetch, we don't have to add it to the initial state
                    // - whatever it was isn't important
                }

                write_addresses.insert(op.addr, op.data);
            }
            _ => {}
        }
    }

    // Collapse initial state hash into vector of arrays
    let mut ram_vec: Vec<[u32; 2]> = initial_state.iter().map(|(&addr, &data)| [addr, data as u32]).collect();

    ram_vec.sort_by(|a, b| a[0].cmp(&b[0]));

    (initial_state, ram_vec)
}

pub fn ram_from_reads(reads: Vec<BusOp>) -> Vec<[u32; 2]> {
    let mut ram_reads = reads.clone();

    // Filter out IO reads, these are not used for ram setup
    ram_reads.retain(|&op| !matches!(op.op_type, BusOpType::IoRead));

    let ram = ram_reads.iter().map(|&x| [x.addr, x.data as u32]).collect();

    ram
}

pub fn final_state_from_ops(initial_state: HashMap<u32, u8>, all_ops: Vec<BusOp>) -> Vec<[u32; 2]> {
    let mut ram_ops = all_ops.clone();
    // We modify the intitial state by inserting write operations into it.
    let mut final_state = initial_state.clone();

    // Filter out IO reads, these are not used for ram setup
    ram_ops.retain(|&op| !matches!(op.op_type, BusOpType::IoRead));
    // Filter out IO writes, these are not used for ram setup
    ram_ops.retain(|&op| !matches!(op.op_type, BusOpType::IoWrite));

    let mut write_addresses: HashMap<u32, u8> = HashMap::new();
    //let mut ram_hash: HashMap<u32, u8> = HashMap::new();

    for op in ram_ops {
        match op.op_type {
            BusOpType::MemRead => {
                // Check if this read is already in memory. If it is, it must have the same value
                // or we are out of sync!
                match initial_state.get(&op.addr) {
                    Some(d) => {
                        if *d != op.data {
                            // Read op doesn't match initial state. Invalid!
                            panic!(
                                "Memop sync fail. MemRead [{:05X}]:{:02X}, hash value: {:02X}",
                                op.addr, op.data, d
                            );
                        }
                    }
                    None => {
                        // Read from mem op not in initial state. If we didn't write to this value, this read is invalid.

                        if write_addresses.get(&op.addr).is_some() {
                            // Ok, we wrote to this address at some point, so we can read it even if it wasn't in the
                            // initial state.
                        }
                        else {
                            // We never wrote to this address, and it's not in the initial state. This is invalid!
                            panic!("Memop sync fail. MemRead from address not in initial state and not written: [{:05X}]:{:02X}", op.addr, op.data);
                        }
                    }
                }
            }
            BusOpType::MemWrite => {
                // No need to check writes; just insert the value.
                write_addresses.insert(op.addr, op.data);
                final_state.insert(op.addr, op.data);
            }
            _ => {}
        }
    }

    // Collapse ram hash into vector of arrays
    let mut ram_vec: Vec<[u32; 2]> = final_state.iter().map(|(&addr, &data)| [addr, data as u32]).collect();

    ram_vec.sort_by(|a, b| a[0].cmp(&b[0]));

    ram_vec
}

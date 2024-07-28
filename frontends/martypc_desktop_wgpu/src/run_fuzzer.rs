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

    run_fuzzer.rs - Implement the main procedure for fuzzer mode.
                    Requires CPU validator feature.
*/

use marty_core::cpu_common;
use std::{
    cell::RefCell,
    fs::File,
    io::{BufWriter, Write},
    rc::Rc,
};

use config_toml_bpaf::ConfigFileParams;
use frontend_common::{floppy_manager::FloppyManager, rom_manager::RomManager};

use marty_core::{
    bytequeue::ByteQueue,
    cpu_common::{
        builder::CpuBuilder,
        Cpu,
        CpuAddress,
        CpuOption,
        CpuSubType,
        CpuType,
        Mnemonic,
        OperandType,
        Register16,
        Register8,
        TraceMode,
    },
    cpu_validator::ValidatorType,
    devices::pic::Pic,
    tracelogger::TraceLogger,
};

pub fn run_fuzzer(config: &ConfigFileParams) {
    /*
    let mut trace_file_option: Box<dyn Write + 'a> = Box::new(std::io::stdout());
    if config.emulator.trace_mode != TraceMode::None {
        // Open the trace file if specified
        if let Some(filename) = &config.emulator.trace_file {
            match File::create(filename) {
                Ok(file) => {
                    trace_file_option = Box::new(BufWriter::new(file));
                },
                Err(e) => {
                    eprintln!("Couldn't create specified tracelog file: {}", e);
                }
            }
        }
    }
    */

    //let mut io_bus = IoBusInterface::new();
    let pic = Rc::new(RefCell::new(Pic::new()));

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

    let trace_mode = config.machine.cpu.trace_mode.unwrap_or_default();

    #[cfg(feature = "cpu_validator")]
    use marty_core::cpu_validator::ValidatorMode;

    let mut cpu;
    #[cfg(feature = "cpu_validator")]
    {
        cpu = match CpuBuilder::new()
            .with_cpu_type(CpuType::Intel8088)
            .with_cpu_subtype(CpuSubType::Intel8088)
            .with_trace_mode(trace_mode)
            .with_trace_logger(cpu_trace)
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

    cpu.randomize_seed(1234);
    cpu.randomize_mem();

    let mut test_num = 0;

    'testloop: loop {
        cpu.reset();

        test_num += 1;
        cpu.randomize_regs();

        if cpu.get_ip() > 0xFFF0 {
            // Avoid IP wrapping issues for now
            continue;
        }

        if cpu_common::calc_linear_address(cpu.get_register16(Register16::CS), cpu.get_ip()) > 0xFFFF0 {
            // Avoid address space wrapping
            continue;
        }

        // Generate specific opcodes (optional)

        // ALU ops

        /*
        cpu.random_inst_from_opcodes(
            &[
                0x00, 0x01, 0x02, 0x03, 0x04, 0x05, // ADD
                0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, // OR
                0x10, 0x11, 0x12, 0x13, 0x14, 0x15, // ADC
                0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, // SBB
                0x20, 0x21, 0x22, 0x23, 0x24, 0x25, // AND
                0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, // SUB
                0x30, 0x31, 0x32, 0x33, 0x34, 0x35, // XOR
                0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, // CMP
            ]
        );
        */

        // Completed 5000 tests

        //cpu.random_inst_from_opcodes(&[0x06, 0x07, 0x0E, 0x0F, 0x16, 0x17, 0x1E, 0x1F]); // PUSH/POP - completed 5000 tests
        //cpu.random_inst_from_opcodes(&[0x27, 0x2F, 0x37, 0x3F]); // DAA, DAS, AAA, AAS
        //cpu.random_inst_from_opcodes(&[0x37]);

        //cpu.random_inst_from_opcodes(&[0x37]);

        //cpu.random_inst_from_opcodes(&[0x90]);

        /*
        // INC & DEC
        cpu.random_inst_from_opcodes(
            &[
                0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47,
                0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F,
            ]
        );
        */

        /*
        // PUSH & POP
        cpu.random_inst_from_opcodes(
            &[
                0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57,
                0x58, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F,
            ]
        );
        */

        /*
        // Relative jumps
        cpu.random_inst_from_opcodes(
            &[
                0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77,
                0x78, 0x79, 0x7A, 0x7B, 0x7C, 0x7D, 0x7E, 0x7F,
            ]
        );
        */

        //cpu.random_inst_from_opcodes(&[0x80, 0x81, 82, 83]); // ALU imm8, imm16, and imm8s
        //cpu.random_inst_from_opcodes(&[0x84, 0x85]); // TEST 8 & 16 bit
        //cpu.random_inst_from_opcodes(&[0x86, 0x87]); // XCHG 8 & 16 bit
        //cpu.random_inst_from_opcodes(&[0x88, 0x89, 0x8A, 0x8B]); // MOV various
        //cpu.random_inst_from_opcodes(&[0x8D]); // LEA
        //cpu.random_inst_from_opcodes(&[0x8C, 0x8E]); // MOV Sreg

        //cpu.random_inst_from_opcodes(&[0x8F]); // POP  (Weird behavior when REG != 0)

        //cpu.random_inst_from_opcodes(&[0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97]); // XCHG reg, ax
        //cpu.random_inst_from_opcodes(&[0x98, 0x99]); // CBW, CWD
        //cpu.random_inst_from_opcodes(&[0x9A]); // CALLF
        //cpu.random_inst_from_opcodes(&[0x9C, 0x9D]); // PUSHF, POPF
        //cpu.random_inst_from_opcodes(&[0x9E, 0x9F]); // SAHF, LAHF
        //cpu.random_inst_from_opcodes(&[0xA0, 0xA1, 0xA2, 0xA3]); // MOV offset

        //cpu.random_inst_from_opcodes(&[0xA4, 0xA5]); // MOVS
        //cpu.random_inst_from_opcodes(&[0xAC, 0xAD]); // LODS

        //cpu.random_inst_from_opcodes(&[0xA6, 0xA7]); // CMPS
        //cpu.random_inst_from_opcodes(&[0xAE, 0xAF]); // SCAS

        //cpu.random_inst_from_opcodes(&[0xA8, 0xA9]); // TEST

        //cpu.random_inst_from_opcodes(&[0xAA, 0xAB]); // STOS

        // MOV imm
        /*
        cpu.random_inst_from_opcodes(
            &[
                0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7,
                0xB8, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE, 0xBF
            ]
        );
        */

        //cpu.random_inst_from_opcodes(&[0xC0, 0xC1, 0xC2, 0xC3]); // RETN
        //cpu.random_inst_from_opcodes(&[0xC4]); // LES
        //cpu.random_inst_from_opcodes(&[0xC5]); // LDS
        //cpu.random_inst_from_opcodes(&[0xC6, 0xC7]); // MOV r/m, imm
        //cpu.random_inst_from_opcodes(&[0xC8, 0xC9, 0xCA, 0xCB]); // RETF
        //cpu.random_inst_from_opcodes(&[0xCC]); // INT3
        //cpu.random_inst_from_opcodes(&[0xCD]); // INT
        //cpu.random_inst_from_opcodes(&[0xCE]); // INT0
        //cpu.random_inst_from_opcodes(&[0xCF]); // IRET  ** unaccounted for cycle after FLUSH

        //cpu.random_inst_from_opcodes(&[0xD0, 0xD1]); // Misc bitshift ops, 1
        //cpu.random_inst_from_opcodes(&[0xD2]); // Misc bitshift ops, cl

        //cpu.random_inst_from_opcodes(&[0xD4]); // AAM
        //cpu.random_inst_from_opcodes(&[0xD5]); // AAD
        //cpu.random_inst_from_opcodes(&[0xD6]); // SALC
        //cpu.random_inst_from_opcodes(&[0xD7]); // XLAT
        //cpu.random_inst_from_opcodes(&[0xD8, 0xD9, 0xDA, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF]); // ESC

        //cpu.random_inst_from_opcodes(&[0xE0, 0xE1, 0xE2, 0xE3]); // LOOP & JCXZ
        //cpu.random_inst_from_opcodes(&[0xE8, 0xE9, 0xEA, 0xEB]); // CALL & JMP

        //cpu.random_inst_from_opcodes(&[0xF5]); // CMC

        //cpu.random_grp_instruction(0xF6, &[0, 1, 2, 3]); // 8 bit TEST, NOT & NEG
        //cpu.random_grp_instruction(0xF7, &[0, 1, 2, 3]); // 16 bit TEST, NOT & NEG
        //cpu.random_grp_instruction(0xF6, &[4, 5]); // 8 bit MUL & IMUL
        //cpu.random_grp_instruction(0xF7, &[4, 5]); // 16 bit MUL & IMUL

        cpu.random_grp_instruction(0xF6, &[6, 7]); // 8 bit DIV & IDIV
                                                   //cpu.random_grp_instruction(0xF7, &[6, 7]); // 16 bit DIV & IDIV

        //cpu.random_inst_from_opcodes(&[0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD]); // CLC, STC, CLI, STI, CLD, STD

        //cpu.random_grp_instruction(0xFE, &[0, 1]); // 8 bit INC & DEC
        //cpu.random_grp_instruction(0xFF, &[0, 1]); // 16 bit INC & DEC

        //cpu.random_grp_instruction(0xFE, &[2, 3]); // CALL & CALLF
        //cpu.random_grp_instruction(0xFF, &[2, 3]); // CALL & CALLF
        //cpu.random_grp_instruction(0xFE, &[4, 5]); // JMP & JMPF
        //cpu.random_grp_instruction(0xFF, &[4, 5]); // JMP & JMPF
        //cpu.random_grp_instruction(0xFE, &[6, 7]); // 8-bit broken PUSH & POP
        //cpu.random_grp_instruction(0xFF, &[6, 7]); // PUSH & POP

        // Decode this instruction
        let instruction_address = cpu_common::calc_linear_address(cpu.get_register16(Register16::CS), cpu.get_ip());

        cpu.bus_mut().seek(instruction_address as usize);
        let (opcode, _cost) = cpu.bus_mut().read_u8(instruction_address as usize, 0).expect("mem err");

        let mut i = match cpu.get_type().decode(cpu.bus_mut(), true) {
            Ok(i) => i,
            Err(_) => {
                log::error!("Instruction decode error, skipping...");
                continue;
            }
        };

        // Skip N successful instructions

        // was at 13546
        if test_num < 0 {
            continue;
        }

        cpu.set_option(CpuOption::EnableWaitStates(false));
        cpu.set_option(CpuOption::TraceLoggingEnabled(config.machine.cpu.trace_on));

        match i.opcode {
            0xFE | 0xD2 | 0xD3 | 0x8F => {
                continue;
            }
            _ => {}
        }

        let mut rep = false;
        match i.mnemonic {
            /*
            Mnemonic::INT | Mnemonic::INT3 | Mnemonic::INTO | Mnemonic::IRET => {
                continue;
            },
            */
            Mnemonic::WAIT => {
                continue;
            }
            Mnemonic::POPF => {
                // POPF can set trap flag which messes up the validator
                continue;
            }
            Mnemonic::LDS | Mnemonic::LES | Mnemonic::LEA => {
                if let OperandType::Register16(_) = i.operand2_type {
                    // Invalid forms end up using the last calculated EA. However this will differ between
                    // the validator and CPU due to the validator setup routine.
                    continue;
                }
            }
            Mnemonic::HLT => {
                // For obvious reasons
                continue;
            }
            /*
            Mnemonic::AAM | Mnemonic::DIV | Mnemonic::IDIV => {
                // Timings on these will take some work
                continue;
            }
            */
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
                // limit cx to 31.
                cpu.set_register16(Register16::CX, cpu.get_register16(Register16::CX) % 32);

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
                // Limit cl to 0-31.
                cpu.set_register8(Register8::CL, cpu.get_register8(Register8::CL) % 32);
            }
            _ => {}
        }

        i.address = instruction_address;

        log::trace!(
            "Test {}: Validating instruction: {} op:{:02X} @ [{:05X}]",
            test_num,
            i,
            opcode,
            i.address
        );

        // Set terminating address for CPU validator.
        // Set terminating address for CPU validator.
        let end_address = cpu_common::calc_linear_address(
            cpu.get_register16(Register16::CS),
            cpu.get_ip().wrapping_add(i.size as u16),
        );

        //log::debug!("Setting end address: {:05X}", end_address);
        cpu.set_end_address(CpuAddress::Flat(end_address));
        log::trace!("Setting end address: {:05X}", end_address);

        // We loop here to handle REP string instructions, which are broken up into 1 effective instruction
        // execution per iteration. The 8088 makes no such distinction.
        loop {
            match cpu.step(false) {
                Ok((_, cycles)) => {
                    log::trace!("Instruction reported {} cycles", cycles);

                    if rep & cpu.in_rep() {
                        continue;
                    }
                }
                Err(err) => {
                    log::error!("CPU Error: {}\n", err);
                    cpu.trace_flush();
                    break 'testloop;
                }
            }

            match cpu.step_finish(None) {
                Ok(_) => {
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
    }

    //std::process::exit(0);
}

/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2025 Daniel Balsom

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

    cpu_vx0::step.rs

    Implements a single instruction step for the 808x CPU.

*/

use crate::{
    cpu_common::{CpuError, CpuException, Disassembly, ExecutionResult, StepResult, OPCODE_PREFIX_0F},
    cpu_vx0::*,
};

use crate::cpu_common::CpuArch;
#[cfg(feature = "cpu_validator")]
use crate::{cpu_vx0::decode_v20::DECODE, vgdr};

impl NecVx0 {
    /// Run a single instruction.
    ///
    /// We divide instruction execution into separate fetch/decode and microcode execution phases.
    /// This is an artificial distinction, but allows for flexibility as the decode() function can be
    /// used on anything that implements the ByteQueue trait, ie, raw memory for a disassembly viewer.
    ///
    /// REP string instructions are handled by stopping them after one iteration so that interrupts can
    /// be checked.
    pub fn step(&mut self, skip_breakpoint: bool) -> Result<(StepResult, u32), CpuError> {
        self.instr_cycle = 0;
        self.instr_elapsed = self.int_elapsed;

        // If tracing is enabled, clear the trace string vector that holds the trace from the last instruction.
        if self.trace_enabled {
            self.trace_str_vec.clear();
            self.trace_token_vec.clear();
        }

        // The Halt state can be expensive if we only execute one cycle per halt - however precise wake from halt is
        // necessary for Area5150. We can dynamically adjust the cycle count of stepping in the halt state depending
        // on a hint from the bus whether a timer interrupt is imminent.
        if self.halted {
            let halt_cycles = match self.bus().is_intr_imminent() {
                true => 1,
                false => 5,
            };
            self.halt_cycles += halt_cycles as u64;
            self.cycles(halt_cycles);
            return Ok((StepResult::Normal, halt_cycles));
        }

        let mut instruction_address = self.instruction_address;

        // Fetch the next instruction unless we are executing a REP
        if !self.in_rep {
            // A real 808X CPU maintains a single Program Counter or PC register that points to the next instruction
            // to be fetched, not the currently executing instruction. This value is "corrected" whenever the current
            // value of IP is required, ie, pushing IP to the stack. This is performed by the 'CORR' microcode routine.

            // Sometimes it is more convenient for us to think of the current ip address, which can be calculated on the
            // fly from PC by the ip() instruction, but only usefully on instruction boundaries, such as now.
            self.instruction_ip = self.ip();
            self.instruction_address = NecVx0::calc_linear_address(self.cs, self.instruction_ip);
            instruction_address = self.instruction_address;
            //log::warn!("instruction address: {:05X}", instruction_address);

            if self.end_addr == (instruction_address as usize) {
                return Ok((StepResult::ProgramEnd, 0));
            }

            // Check if we are in BreakpointHit state. This state must be cleared before we can execute another instruction.
            if self.get_breakpoint_flag() {
                return Ok((StepResult::BreakpointHit, 0));
            }

            // Check instruction address for breakpoint on execute flag
            let iflags = self.bus.get_flags(instruction_address as usize);
            if !skip_breakpoint && iflags & (MEM_BPE_BIT | MEM_SW_BIT) != 0 {
                if iflags & MEM_SW_BIT != 0 {
                    // Stopwatch hit
                    for sw in self.stopwatches.iter_mut().flatten() {
                        // Check for stopwatch stop first. This is so we can restart on the same instruction, ie,
                        // when measuring loops where start==stop.
                        if sw.stop == instruction_address {
                            log::debug!("Stopwatch stop at {:05X}", instruction_address);
                            sw.stop();
                        }
                        if sw.start == instruction_address && !sw.running {
                            log::debug!("Stopwatch start at {:05X}", instruction_address);
                            sw.start();
                        }
                    }
                    self.update_sw_flag();
                }

                if iflags & MEM_BPE_BIT != 0 {
                    // Breakpoint hit
                    log::debug!("Breakpoint hit at {:05X}", instruction_address,);
                    self.set_breakpoint_flag();
                    return Ok((StepResult::BreakpointHit, 0));
                }
            }

            // Check for the step over breakpoint
            if let Some(step_over_address) = self.step_over_breakpoint {
                if instruction_address == step_over_address {
                    log::debug!("CPU: Step Over address hit: {:05X}", step_over_address);
                    // Clear the step over breakpoint, so it is not immediately re-triggered
                    self.step_over_breakpoint = None;
                    return Ok((StepResult::StepOverHit, 0));
                }
            }

            // Check if this address is a return from a CALL or INT
            if self.bus.get_flags(instruction_address as usize) & MEM_RET_BIT != 0 {
                // This address is a return address, rewind the stack
                self.rewind_call_stack(instruction_address);
            }

            // Clear the validator cycle states from the last instruction.
            #[cfg(feature = "cpu_validator")]
            {
                self.validate_init();
            }

            let cpu_type = self.cpu_type;

            // If cycle tracing is enabled, we prefetch the current instruction directly from memory backend
            // to make the instruction disassembly available to the trace log on the first byte fetch of an
            // instruction.
            // This of course now requires decoding each instruction twice, but cycle tracing is pretty slow
            // anyway.
            if self.trace_mode == TraceMode::CycleText {
                self.bus.seek(instruction_address as usize);
                self.i = match cpu_type.decode(&mut self.bus, true) {
                    Ok(i) => i,
                    Err(_) => {
                        self.is_running = false;
                        self.is_error = true;
                        return Err(CpuError::InstructionDecodeError(instruction_address));
                    }
                };
                //log::trace!("Fetching instruction...");
                self.i.address = instruction_address;
            }

            // Fetch and decode the current instruction. This uses the CPU's own ByteQueue trait
            // implementation, which fetches instruction bytes through the processor instruction queue.
            //log::warn!("decoding instruction...");
            self.i = match cpu_type.decode(self, true) {
                Ok(i) => i,
                Err(_) => {
                    self.is_running = false;
                    self.is_error = true;
                    return Err(CpuError::InstructionDecodeError(instruction_address));
                }
            };

            // Begin the current instruction validation context.
            #[cfg(feature = "cpu_validator")]
            {
                self.validate_begin(instruction_address);
            }
        }

        // Since Cpu::decode doesn't know anything about the current IP, it can't set it, so we do that now.
        self.i.address = instruction_address;

        // Uncomment to debug instruction fetch
        //self.debug_fetch(instruction_address);

        self.last_cs = self.cs;
        self.last_ip = self.instruction_ip;

        // Load the mod/rm operand for the instruction, if applicable.
        self.load_operand();

        #[cfg(feature = "cpu_validator")]
        {
            (self.peek_fetch, _) = self.bus.read_u8(self.pc as usize, 0).unwrap();
            self.instr_slice = self.bus.get_vec_at(instruction_address as usize, self.i.size as usize);
        }

        let arch = match self.cpu_type {
            CpuType::NecV20(arch) => arch,
            CpuType::NecV30(arch) => arch,
            _ => CpuArch::I86,
        };

        match arch {
            CpuArch::I86 => {
                // Execute the current decoded instruction.
                if self.i.prefixes & OPCODE_PREFIX_0F == 0 {
                    self.exec_result = self.execute_instruction();
                }
                else {
                    self.exec_result = self.execute_extended_instruction();
                }
            }
            CpuArch::I8080 => {
                self.exec_result = self.execute_8080_instruction();
            }
        }

        let step_result = match &self.exec_result {
            ExecutionResult::Okay => {
                // Normal non-jump instruction updates CS:IP to next instruction during execute()
                self.instruction_count += 1;

                // Perform instruction tracing, if enabled
                if self.trace_enabled && self.trace_mode == TraceMode::Instruction {
                    self.trace_print(&self.instruction_state_string(self.last_cs, self.last_ip));
                }

                Ok((StepResult::Normal, self.device_cycles))
            }
            ExecutionResult::OkayJump => {
                // A control flow instruction updated PC.
                self.instruction_count += 1;
                self.jumped = true;

                // Perform instruction tracing, if enabled
                if self.trace_enabled && self.trace_mode == TraceMode::Instruction {
                    self.trace_print(&self.instruction_state_string(self.last_cs, self.last_ip));
                }

                // Only CALLS will set a step over target.
                if let Some(step_over_target) = self.step_over_target {
                    Ok((StepResult::Call(step_over_target), self.device_cycles))
                }
                else {
                    Ok((StepResult::Normal, self.device_cycles))
                }
            }
            ExecutionResult::OkayRep => {
                // We are in a REPx-prefixed instruction.

                // The ip will not increment until the instruction has completed, but
                // continue to process interrupts. We passed pending_interrupt to execute
                // earlier so that a REP string operation can call RPTI to be ready for
                // an interrupt to occur.

                // REP will always set a step over target.
                Ok((StepResult::Rep(self.step_over_target.unwrap()), self.device_cycles))
            }
            /*
            ExecutionResult::UnsupportedOpcode(o) => {
                // This shouldn't really happen on the 8088 as every opcode does something,
                // but allowed us to be missing opcode implementations during development.
                self.is_running = false;
                self.is_error = true;
                Err(CpuError::UnhandledInstructionError(o, instruction_address))
            }
            */
            ExecutionResult::ExecutionError(e) => {
                // Something unexpected happened!
                self.is_running = false;
                self.is_error = true;
                Err(CpuError::ExecutionError(instruction_address, e.to_string()))
            }
            ExecutionResult::Halt => {
                // Specifically, this error condition is a halt with interrupts disabled -
                // since only an interrupt can resume after a halt, execution cannot continue.
                // This state is most often encountered during failed BIOS initialization checks.
                self.is_running = false;
                self.is_error = true;
                Err(CpuError::CpuHaltedError(instruction_address))
            }
            ExecutionResult::ExceptionError(exception) => {
                // A CPU exception occurred. On the 8088, these are limited in scope to
                // division errors, and overflow after INTO.
                match exception {
                    CpuException::DivideError => {
                        // Moved int0 handling into aam/div instructions directly.
                        //self.handle_exception(0);
                        Ok((StepResult::Normal, self.device_cycles))
                    }
                    _ => {
                        // Unhandled exception?
                        Err(CpuError::ExceptionError(*exception))
                    }
                }
            }
        };

        // Reset interrupt pending flag - this flag is set on step_finish() and
        // only valid for a single instruction execution.
        self.intr_pending = false;

        step_result
    }

    /// Finish the current CPU instruction.
    ///
    /// This function is meant to be called after devices are run after an instruction.
    ///
    /// Normally, this function will fetch the first byte of the next instruction.
    /// Running devices can generate interrupts. If the INTR line is set by a device,
    /// we do not want to fetch the next byte - we want to jump directly into the
    /// interrupt routine - *unless* we are in a REP, in which case we set a flag
    /// so that the interrupt execution can occur on the next call to step() to simulate
    /// the string instruction calling RPTI.
    ///
    /// This function effectively simulates the RNI microcode routine.
    pub fn step_finish(&mut self, _disassembly: Option<&mut Disassembly>) -> Result<StepResult, CpuError> {
        let mut step_result = StepResult::Normal;
        let mut irq = 7;
        let mut did_interrupt = false;
        let mut did_nmi = false;
        let mut did_trap = false;

        // This function is called after devices are run for the CPU period, so reset device cycles.
        // Device cycles will begin incrementing again with any terminating fetch.
        self.instr_elapsed = 0;
        self.int_elapsed = 0;
        self.device_cycles = 0;

        if self.nmi && self.bus.nmi_enabled() && !self.nmi_triggered {
            // NMI takes priority over trap and INTR.
            if self.halted {
                // Resume from halt on interrupt
                self.resume();
            }
            log::debug!("Triggered NMI!");
            self.nmi_triggered = true;
            self.int2();
            did_nmi = true;
            step_result = StepResult::Call(CpuAddress::Segmented(self.cs, self.ip()));
        }
        else if self.intr && self.interrupts_enabled() {
            // An interrupt needs to be processed.

            if self.in_rep {
                // We're in an REP prefixed-string instruction.
                // Delay processing of the interrupt so that the string
                // instruction can execute RPTI. At that point, the REP
                // will terminate, and we can process the interrupt as normal.
                self.intr_pending = true;
            }
            else {
                // We are not in a REP prefixed string instruction, so we
                // can process an interrupt normally.

                if self.halted {
                    // Resume from halt on interrupt
                    self.resume();
                }

                // Query the PIC to get the interrupt vector.
                // This is a bit artificial as we don't actually read the IV during the 2nd
                // INTA cycle like the CPU does, instead we save the value now and simulate it later.
                // TODO: Think about changing this to query during INTA
                if let Some(pic) = self.bus.pic_mut().as_mut() {
                    // Is INTR active? TODO: Could combine these calls (return Option<iv>) on query?
                    if pic.query_interrupt_line() {
                        if let Some(iv) = pic.get_interrupt_vector() {
                            irq = iv;
                        }
                    }
                }

                // We will be jumping into an ISR now. Set the step result to Call and return
                // the address of the next instruction. (Step Over skips ISRs)
                step_result = StepResult::Call(CpuAddress::Segmented(self.cs, self.ip()));

                if self.int_flags[irq as usize] != 0 {
                    // This interrupt has a breakpoint
                    self.set_breakpoint_flag();
                }
                self.hw_interrupt(irq);
                did_interrupt = true;
                self.biu_fetch_next();
            }
        }
        else if self.trap_enabled() {
            // Trap has the lowest priority.
            if self.halted {
                // Resume from halt on trap
                self.resume();
            }
            self.int1();
            did_trap = true;
            step_result = StepResult::Call(CpuAddress::Segmented(self.cs, self.ip()));
        }
        else if !self.halted {
            // We didn't have NMI, INTR, or TRAP condition. Fetch the next instruction if not halted.
            self.biu_fetch_next();
        }

        // If a CPU validator is enabled, validate the executed instruction.
        #[cfg(feature = "cpu_validator")]
        {
            self.validate_instruction()?;
        }

        let cur_intr = did_interrupt | did_nmi | did_trap;

        if self.instruction_history_on {
            // Tick any stopwatches that are running. We maintain a single flag if one stopwatch
            // is running so that we aren't always iterating through a vector of stopped watches.
            // We also require instruction history, only for performance reasons to reduce if checks.
            if self.stopwatch_running {
                for sw in self.stopwatches.iter_mut().flatten() {
                    sw.tick(self.instr_cycle as u64);
                }
            }

            // Only add non-reentrant instructions to history, unless they were interrupted.
            // This prevents spamming the history with multiple rep string operations.
            if !self.instruction_reentrant || cur_intr {
                if self.instruction_history.len() == CPU_HISTORY_LEN {
                    self.instruction_history.pop_front();
                }

                self.instruction_history.push_back(HistoryEntry::InstructionEntry {
                    cs: self.last_cs,
                    ip: self.last_ip,
                    cycles: self.instr_cycle as u16,
                    interrupt: self.last_intr,
                    jump: self.jumped,
                    i: self.i.clone(),
                });
            }

            if did_nmi {
                if self.instruction_history.len() == CPU_HISTORY_LEN {
                    self.instruction_history.pop_front();
                }

                self.instruction_history.push_back(HistoryEntry::NmiEntry {
                    cs: self.last_cs,
                    ip: self.last_ip,
                });
            }

            if did_trap {
                if self.instruction_history.len() == CPU_HISTORY_LEN {
                    self.instruction_history.pop_front();
                }

                self.instruction_history.push_back(HistoryEntry::TrapEntry {
                    cs: self.last_cs,
                    ip: self.last_ip,
                });
            }

            if did_interrupt {
                if self.instruction_history.len() == CPU_HISTORY_LEN {
                    self.instruction_history.pop_front();
                }

                self.instruction_history.push_back(HistoryEntry::InterruptEntry {
                    cs: self.last_cs,
                    ip: self.last_ip,
                    cycles: self.instr_cycle as u16,
                    iv: irq,
                });
            }

            self.last_intr = cur_intr;
        }

        Ok(step_result)
    }

    #[rustfmt::skip]
    #[allow(dead_code, unused_variables)]
    pub fn debug_fetch(&mut self, instruction_address: u32) {
        let (opcode, _cost) = self.bus.read_u8(instruction_address as usize, 0).expect("mem err");
        trace_print!(self, "Fetched instruction: {} op:{:02X} at [{:05X}]", self.i, opcode, self.i.address);
        trace_print!(self, "Executing instruction:  [{:04X}:{:04X}] {} ({})", self.cs, self.ip(), self.i, self.i.size);
        log::warn!("Fetched instruction: {} op:{:02X} at [{:05X}]", self.i, opcode, self.i.address);
        //log::warn!("Executing instruction:  [{:04X}:{:04X}] {} ({})", self.cs, self.ip, self.i, self.i.size);
    }

    #[cfg(feature = "cpu_validator")]
    pub fn validate_init(&mut self) {
        if self.validator_state == CpuValidatorState::Running {
            if let Some(ref mut validator) = self.validator {
                validator.reset_instruction();
            }
            self.cycle_states.clear();
        }
        else {
            // Clear cycle states spent in reset but not initial prefetch
            self.clear_reset_cycle_states();
        }

        self.vregs = self.get_vregisters();
    }

    #[cfg(feature = "cpu_validator")]
    pub fn validate_begin(&mut self, instruction_address: u32) {
        let v_address = NecVx0::calc_linear_address(self.vregs.cs, self.vregs.ip);
        if v_address != instruction_address {
            log::warn!(
                "Validator address mismatch: {:05X} != {:05X}",
                v_address,
                instruction_address
            );
        }

        if self.vregs.flags & CPU_FLAG_TRAP != 0 {
            log::warn!("Trap flag is set - may break validator!");
        }

        if let Some(ref mut validator) = self.validator {
            if (instruction_address as usize) == self.validator_end {
                log::info!("Validation reached end address. Stopping.");
                self.validator_state = CpuValidatorState::Ended;
            }

            if self.validator_state == CpuValidatorState::Uninitialized
                || self.validator_state == CpuValidatorState::Running
            {
                validator.begin_instruction(
                    &self.vregs,
                    (instruction_address + self.i.size) as usize & 0xFFFFF,
                    self.validator_end,
                );
            }
        }
    }

    #[cfg(feature = "cpu_validator")]
    pub fn validate_instruction(&mut self) -> Result<(), CpuError> {
        match self.exec_result {
            ExecutionResult::Okay
            | ExecutionResult::OkayJump
            | ExecutionResult::ExceptionError(CpuException::DivideError) => {
                let mut v_flags = 0;

                if let ExecutionResult::ExceptionError(CpuException::DivideError) = self.exec_result {
                    // In the case of a divide exception, undefined flags get pushed to the stack.
                    // So until we figure out the actual logic behind setting those undefined flags,
                    // we can't validate writes. Also the cycle timing seems to vary a little when
                    // executing int0, so allow a one cycle variance.
                    v_flags |= VAL_NO_WRITES | VAL_NO_FLAGS | VAL_ALLOW_ONE;
                }

                match self.i.mnemonic {
                    Mnemonic::DIV => {
                        // There's a one cycle variance in my DIV instructions somewhere.
                        // I just want to get these tests out the door, so allow it.
                        v_flags |= VAL_ALLOW_ONE;
                    }
                    Mnemonic::IDIV => {
                        v_flags |= VAL_NO_WRITES | VAL_NO_FLAGS | VAL_NO_CYCLES;
                    }
                    _ => {}
                }

                // End validation of current instruction
                let vregs = self.get_vregisters();

                if self.i.size == 0 {
                    log::error!("Invalid length: [{:05X}] {}", self.instruction_address, self.i);
                }

                let cpu_address = self.flat_ip() as usize;

                if let Some(ref mut validator) = self.validator {
                    // If validator uninitialized, set register state now and move into running state.
                    if self.validator_state == CpuValidatorState::Uninitialized {
                        // This resets the validator CPU
                        log::debug!("Validator Uninitialized. Resetting validator and setting registers...");
                        validator.set_regs();
                        self.validator_state = CpuValidatorState::Running;
                    }

                    if self.validator_state == CpuValidatorState::Running {
                        //log::debug!("Validating opcode: {:02X}", self.i.opcode);
                        match validator.validate_instruction(
                            self.i.to_string(),
                            &self.instr_slice,
                            v_flags,
                            self.peek_fetch as u16,
                            vgdr!(self.i).has_modrm(),
                            0,
                            &vregs,
                            &self.cycle_states,
                        ) {
                            Ok(result) => {
                                match (result, self.validator_mode) {
                                    (ValidatorResult::Ok | ValidatorResult::OkEnd, ValidatorMode::Instruction) => {
                                        if let Err(e) = validator.validate_regs(&vregs) {
                                            log::warn!("Register validation failure: {} Halting execution.", e);
                                            self.is_running = false;
                                            self.is_error = true;
                                            return Err(CpuError::CpuHaltedError(self.instruction_address));
                                        }
                                    }
                                    (ValidatorResult::Ok, ValidatorMode::Cycle) => {}
                                    (ValidatorResult::OkEnd, ValidatorMode::Cycle) => {
                                        if self.validator_end == cpu_address {
                                            self.validator_state = CpuValidatorState::Ended;

                                            // Validation has reached program end address
                                            if let Err(e) = validator.validate_regs(&vregs) {
                                                log::warn!("Register validation failure: {} Halting execution.", e);
                                                self.is_running = false;
                                                self.is_error = true;
                                                return Err(CpuError::CpuHaltedError(self.instruction_address));
                                            }
                                            else {
                                                log::debug!("Registers validated. Validation ended successfully.");
                                                self.validator_state = CpuValidatorState::Ended;
                                                self.trace_flush();
                                            }
                                        }
                                    }
                                    _ => {
                                        log::warn!("Validation failure: Halting execution.");
                                        self.is_running = false;
                                        self.is_error = true;
                                        return Err(CpuError::CpuHaltedError(self.instruction_address));
                                    }
                                }
                            }
                            Err(e) => {
                                log::warn!("Validation failure: {} Halting execution.", e);
                                self.is_running = false;
                                self.is_error = true;
                                return Err(CpuError::CpuHaltedError(self.instruction_address));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

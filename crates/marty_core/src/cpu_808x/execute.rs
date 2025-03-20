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

    cpu_808x::execute.rs

    Executes an instruction after it has been fetched.
    Includes all main opcode implementations.
*/

use crate::{
    cpu_808x::{decode::DECODE, *},
    cpu_common::{
        CpuAddress,
        CpuException,
        ExecutionResult,
        Mnemonic,
        QueueOp,
        OPCODE_PREFIX_REP1,
        OPCODE_PREFIX_REP2,
    },
};

// rustfmt chokes on large match statements.
#[rustfmt::skip]
impl Intel808x {
    /// Execute the current instruction. At the phase this function is called we have
    /// fetched and decoded any prefixes, the opcode byte, modrm and any displacement
    /// and populated an Instruction struct.
    ///
    /// Additionally, if an EA was to be loaded, the load has already been performed.
    ///
    /// For each opcode, we execute cycles equivalent to the microcode routine for
    /// that function. Microcode line numbers are usually provided for cycle tracing.
    ///
    /// The microcode instruction with a terminating RNI should not be executed, as this
    /// requires the next instruction byte to be fetched and is handled by finalize().
    #[rustfmt::skip]
    pub fn execute_instruction(&mut self) -> ExecutionResult {
        self.step_over_target = None;
        self.trace_comment("EXECUTE");
        
        // TODO: Check optimization here. We could reset several flags at once if they were in a
        //       bitfield.
        // Reset instruction reentrancy flag
        self.instruction_reentrant = false;
        // Reset exception state
        self.exception = CpuException::NoException;
        // Reset jumped flag.
        self.jumped = false;
        // Reset trap suppression flag
        self.trap_suppressed = false;
        // Decrement trap counters.
        self.trap_enable_delay = self.trap_enable_delay.saturating_sub(1);
        self.trap_disable_delay = self.trap_disable_delay.saturating_sub(1);

        // If we have an NX loaded RNI cycle from the previous instruction, execute it.
        // Otherwise, wait one cycle before beginning instruction if there was no modrm.
        if self.nx {
            self.trace_comment("RNI");
            self.next_mc();
            self.cycle();
            self.nx = false;
        }
        else if self.last_queue_op == QueueOp::First {
            self.cycle();
        }

        // Set the microcode PC for this opcode.
        self.mc_pc = DECODE[self.i.decode_idx].mc;

        // Check for REPx prefixes
        if self.i.prefixes & (OPCODE_PREFIX_REP1 | OPCODE_PREFIX_REP2) != 0 {
            // A REPx prefix was set
            let mut invalid_rep = false;

            match self.i.mnemonic {
                Mnemonic::STOSB | Mnemonic::STOSW | Mnemonic::LODSB | Mnemonic::LODSW | Mnemonic::MOVSB | Mnemonic::MOVSW => {
                    self.rep_type = RepType::Rep;
                }
                Mnemonic::SCASB | Mnemonic::SCASW | Mnemonic::CMPSB | Mnemonic::CMPSW => {
                    // Valid string ops with REP prefix
                    if self.i.prefixes & OPCODE_PREFIX_REP1 != 0 {
                        self.rep_type = RepType::Repne;
                    }
                    else {
                        self.rep_type = RepType::Repe;
                    }
                }
                Mnemonic::MUL | Mnemonic::IMUL | Mnemonic::DIV | Mnemonic::IDIV => {
                    // REP prefix on MUL/DIV negates the product/quotient.
                    self.rep_type = RepType::MulDiv;
                }
                _ => {
                    invalid_rep = true;
                }
            }

            if !invalid_rep {
                self.in_rep = true;
                self.rep_mnemonic = self.i.mnemonic;
            }
        }

        // Reset the wait cycle after STI
        self.interrupt_inhibit = false;

        // `Off-the-rails` detection. Try to determine when we are executing garbage code.
        // Keep a tally of how many Opcode 0x00's we've executed in a row. Too many likely means we've run
        // off the rails into uninitialized memory, whereupon we halt so that we can check things out.

        // This is now optional in the configuration file, as some test applications like acid88 won't work
        // otherwise.
        if self.off_rails_detection {
            if self.i.opcode == 0x00 {
                self.opcode0_counter = self.opcode0_counter.wrapping_add(1);
                if self.opcode0_counter > 5 {
                    // Halt permanently by clearing interrupt flag
                    self.clear_flag(Flag::Interrupt);
                    self.halted = true;
                    self.instruction_reentrant = true;
                }
            }
            else {
                self.opcode0_counter = 0;
            }
        }
        
        // Execute the instruction microcode
        let mc_fn = DECODE[self.i.decode_idx].mc_fn;
        mc_fn(self);

        // Reset REP init flag. This flag is set after a rep-prefixed instruction is executed for the first time. It
        // should be preserved between executions of a rep-prefixed instruction unless an interrupt occurs, in which
        // case the rep-prefix instruction terminates normally after RPTI. This flag determines whether RPTS is
        // run when executing the instruction.
        if !self.in_rep {
            self.rep_init = false;
        }
        else {
            self.instruction_reentrant = true;
        }

        if self.jumped {
            ExecutionResult::OkayJump
        }
        else if self.in_rep {
            if let RepType::MulDiv = self.rep_type {
                // Rep prefix on MUL/DIV just sets flags, do not rep
                self.in_rep = false;
                ExecutionResult::Okay
            }
            else { 
                self.rep_init = true;
                // Set step-over target so that we can skip long REP instructions.
                // Normally the step behavior during REP is to perform a single iteration.
                self.step_over_target = Some(CpuAddress::Segmented(self.cs, self.ip()));
                ExecutionResult::OkayRep
            }
        }
        else if self.halted && !self.reported_halt && !self.get_flag(Flag::Interrupt) && !self.get_flag(Flag::Trap) {
            // CPU was halted with interrupts disabled - will not continue
            self.reported_halt = true;
            ExecutionResult::Halt
        }
        else {
            match self.exception {
                CpuException::DivideError => ExecutionResult::ExceptionError(self.exception),
                CpuException::NoException => ExecutionResult::Okay,
                _ => panic!("Invalid exception type!")
            }
        }           

    }
}

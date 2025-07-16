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
*/

use core::fmt::Display;
use std::error::Error;

use ard808x_client::*;

use super::{BusOp, BusOpType, OPCODE_NOP};
use crate::{
    arduino8088_validator::{code_stream::CodeStream, queue::*, remote_program::RemoteProgram, *},
    cpu_common::QueueOp,
    cpu_validator::*,
};

macro_rules! trace {
    ($log:ident, $($t:tt)*) => {{
        $log.print(&format!($($t)*));
        $log.print("\n".to_string());
    }};
}

const ADDRESS_SPACE: usize = 1_048_576;

// Code to perform a full prefetch on a given CPU. We utilize an undefined opcode on V20 that has
// no side effects
static NULL_PREFETCH_PGM: [u8; 0] = [];
static INTEL808X_PREFETCH_PGM: [u8; 4] = [0xAA, 0xAA, 0xAA, 0xAA];
static NECVX0_PREFETCH_PGM: [u8; 2] = [0x63, 0xC0];

static INTEL_PREFIXES: [u8; 8] = [0x26, 0x2E, 0x36, 0x3E, 0xF0, 0xF1, 0xF2, 0xF3];
static NEC_PREFIXES: [u8; 10] = [0x26, 0x2E, 0x36, 0x3E, 0xF0, 0xF1, 0xF2, 0xF3, 0x64, 0x65];

#[derive(Copy, Clone, Debug)]
pub enum RemoteCpuError {
    BusOpAddressMismatch(u32, u32),
    SubsequentByteFetchOutOfRange(u32, u32),
    CannotOweMultipleOps,
    BusOpUnderflow,
}

impl From<CpuType> for CpuWidth {
    fn from(cpu_type: CpuType) -> Self {
        match cpu_type {
            CpuType::Intel8088 | CpuType::NecV20(_) => CpuWidth::Eight,
            CpuType::Intel8086 | CpuType::NecV30(_) => CpuWidth::Sixteen,
        }
    }
}

impl Error for RemoteCpuError {}
impl Display for RemoteCpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RemoteCpuError::BusOpAddressMismatch(latch, op_addr) => {
                write!(
                    f,
                    "Address latch doesn't match address of emulator bus operation: Latch: {:05X} Op addr: {:05X}",
                    latch, op_addr
                )
            }
            RemoteCpuError::SubsequentByteFetchOutOfRange(q_addr, instr_end) => {
                write!(
                    f,
                    "Subsequent byte read out of range of instruction: addr: {:05X} end: {:05X}",
                    q_addr, instr_end
                )
            }
            RemoteCpuError::CannotOweMultipleOps => {
                write!(f, "Cannot owe more than one bus operation.")
            }
            RemoteCpuError::BusOpUnderflow => {
                write!(f, "CPU executed more bus operations than EMU reported.")
            }
        }
    }
}

#[derive(Default, Debug)]
pub struct RemoteCpuRegisters {
    ax:    u16,
    bx:    u16,
    cx:    u16,
    dx:    u16,
    ss:    u16,
    ds:    u16,
    es:    u16,
    sp:    u16,
    bp:    u16,
    si:    u16,
    di:    u16,
    cs:    u16,
    ip:    u16,
    flags: u16,
}

#[derive(Copy, Clone, Debug, Default)]
pub enum RunState {
    #[default]
    Init,
    Preload,
    Program,
    Finalize,
}

pub struct RemoteCpu {
    cpu_type: CpuType,
    width: CpuWidth,
    cpu_client: CpuClient,
    mode: ValidatorMode,
    prefetch: bool,

    regs: RemoteCpuRegisters,
    memory: Vec<u8>,
    pc: usize,
    instr_end_addr: usize,
    program_started: bool,
    program_end_addr: usize,
    validator_state: ProgramState,
    run_state: RunState,

    prefetch_pgm: Option<RemoteProgram>,
    code_stream:  CodeStream,

    address_latch: u32,
    address_bus: u32,
    status: u8,
    command_status: u8,
    control_status: u8,
    data_bus: u16,
    data_type: QueueDataType,
    data_width: DataWidth,

    cycle_num: u32,
    mcycle_state: BusState,
    bus_state: BusState,
    bus_cycle: BusCycle,
    access_type: AccessType,
    wait_states: u32,
    just_reset: bool,
    last_cycle_state: Option<CycleState>,

    queue: InstructionQueue,
    queue_byte: u8,
    queue_type: QueueDataType,
    queue_first_fetch: bool,
    queue_fetch_n: u8,
    queue_fetch_addr: u32,
    peek_fetch: u16,
    rni: bool,
    opcode: u8,
    instr_str: String,
    instr_bytes: Vec<u8>,
    instr_addr: u32,
    end_instruction: bool,
    finalize: bool,
    flushed: bool,
    discard_front: bool,
    error: Option<RemoteCpuError>,
    cycle_states: Vec<CycleState>,
    visited_fetches: HashSet<u32>,

    // Validator stuff
    busop_n: usize,
    fetchop_n: usize,
    owe_busop: bool,
    fetch_rollover: bool,
    fetching_beyond: bool,
    program_ended: bool,
    v_pc: usize,

    ignore_underflow: bool,
}

impl RemoteCpu {
    pub fn new(cpu_type: CpuType, cpu_client: CpuClient) -> RemoteCpu {
        let width = CpuWidth::from(cpu_type);
        RemoteCpu {
            cpu_type,
            width,
            cpu_client,
            mode: ValidatorMode::Instruction,
            prefetch: false,

            regs: Default::default(),
            memory: vec![0; ADDRESS_SPACE],
            pc: 0,
            instr_end_addr: 0,
            program_started: true,
            program_end_addr: 0,
            validator_state: ProgramState::Reset,
            run_state: RunState::Init,

            prefetch_pgm: Some(RemoteProgram::new(
                RemoteCpu::prefetch_pgm_bytes(cpu_type),
                OPCODE_NOP,
                CpuWidth::from(cpu_type),
                true,
            )),
            code_stream: CodeStream::new(width),

            address_latch: 0,
            address_bus: 0,
            status: 0,
            command_status: 0,
            control_status: 0,
            data_bus: 0,
            data_type: QueueDataType::Program,
            data_width: Default::default(),
            cycle_num: 0,
            mcycle_state: BusState::PASV,
            bus_state: BusState::PASV,
            bus_cycle: BusCycle::T1,
            access_type: AccessType::CodeOrNone,
            wait_states: 0,
            just_reset: false,
            last_cycle_state: None,
            queue: InstructionQueue::new(width),
            queue_byte: 0,
            queue_type: QueueDataType::Program,
            queue_first_fetch: true,
            queue_fetch_n: 0,
            queue_fetch_addr: 0,
            peek_fetch: 0,
            rni: false,
            opcode: 0,
            instr_str: String::new(),
            instr_addr: 0,
            instr_bytes: Vec::new(),
            end_instruction: false,
            finalize: false,
            flushed: false,
            discard_front: false,
            error: None,
            cycle_states: Vec::new(),
            visited_fetches: HashSet::new(),

            busop_n: 0,
            fetchop_n: 0,
            owe_busop: false,
            fetch_rollover: false,
            fetching_beyond: false,
            program_ended: false,
            v_pc: 0,

            ignore_underflow: false,
        }
    }

    pub fn update_state(&mut self, cycle: bool) -> CycleState {
        /*
        self.program_state = self.cpu_client.get_program_state().expect("Failed to get program state!");
        self.status = self.cpu_client.read_status().expect("Failed to get status!");
        self.command_status = self.cpu_client.read_8288_command().expect("Failed to get 8288 command status!");
        self.control_status = self.cpu_client.read_8288_control().expect("Failed to get 8288 control status!");
        self.data_bus = self.cpu_client.read_data_bus().expect("Failed to get data bus!");
        */

        match cycle {
            true => {
                (
                    self.validator_state,
                    self.control_status,
                    self.status,
                    self.command_status,
                    self.data_bus,
                ) = self
                    .cpu_client
                    .cycle_get_cycle_state()
                    .expect("Failed to get cycle state!");
            }
            false => {
                (
                    self.validator_state,
                    self.control_status,
                    self.status,
                    self.command_status,
                    self.data_bus,
                ) = self.cpu_client.get_cycle_state().expect("Failed to get cycle state!");
            }
        }

        self.data_width = DataWidth::from((self.bhe(), self.a0()));
        self.access_type = AccessType::from(self.status);
        self.bus_state = get_bus_state!(self.status);
        let q_op = get_queue_op!(self.status);
        let mut q = [0; 6];
        self.queue.to_slice(&mut q);

        CycleState {
            n: self.cycle_num,
            addr: self.address_bus,
            t_state: self.bus_cycle,
            a_type: self.access_type,
            b_state: self.bus_state,
            ale: self.control_status & CONTROL_ALE_BIT != 0,
            bhe: self.command_status & COMMAND_BHE_BIT != 0,
            mrdc: self.command_status & COMMAND_MRDC_BIT != 0,
            amwc: self.command_status & COMMAND_AMWC_BIT != 0,
            mwtc: self.command_status & COMMAND_MWTC_BIT != 0,
            iorc: self.command_status & COMMAND_IORC_BIT != 0,
            aiowc: self.command_status & COMMAND_AIOWC_BIT != 0,
            iowc: self.command_status & COMMAND_IOWC_BIT != 0,
            inta: self.command_status & COMMAND_INTA_BIT != 0,
            q_op,
            q_byte: 0,
            q_len: 0,
            q,
            data_bus: 0,
        }
    }

    pub fn set_instr_end_addr(&mut self, end_addr: usize) {
        self.instr_end_addr = end_addr;
    }

    pub fn set_program_end_addr(&mut self, end_addr: usize) {
        self.program_end_addr = end_addr;
    }

    pub fn set_instr_string(&mut self, instr_str: String) {
        self.instr_str = instr_str;
    }

    pub fn set_instr_bytes(&mut self, instr_bytes: &[u8]) {
        self.instr_bytes = instr_bytes.to_vec();

        if let Some(prefetch_pgm) = &mut self.prefetch_pgm {
            // Set the next fetch byte to the first byte of the instruction.
            if !instr_bytes.is_empty() {
                log::trace!("Setting prefetch program next-fetch byte: {:02X}", instr_bytes[0]);
                prefetch_pgm.set_next_fetch(instr_bytes[0]);
            }
        }
    }

    pub fn reset(&mut self) {
        self.bus_cycle = BusCycle::T1;
        self.mcycle_state = BusState::CODE; // First state after reset is a code fetch
        self.run_state = RunState::Init;
        self.last_cycle_state = None;
        self.fetching_beyond = false;
        self.program_ended = false;
        self.fetch_rollover = false;
        self.just_reset = true;
        self.queue.flush();
        self.cycle_states.clear();
        self.visited_fetches.clear();
    }

    pub fn is_last_wait(&self) -> bool {
        matches!(self.bus_cycle, BusCycle::T3 | BusCycle::Tw) && (self.wait_states == 0)
    }

    pub fn is_prefix(&self, opcode: u8) -> bool {
        match self.cpu_type {
            CpuType::Intel8088 | CpuType::Intel8086 => INTEL_PREFIXES.contains(&opcode),
            CpuType::NecV20(_) | CpuType::NecV30(_) => NEC_PREFIXES.contains(&opcode),
        }
    }

    /// Return whether we are inside the preload program.
    pub fn in_prefetch_pgm(&self) -> bool {
        self.prefetch && self.have_prefetch_pgm()
    }

    /// Return whether we are inside the preload program.
    pub fn have_prefetch_pgm(&self) -> bool {
        if let Some(program) = &self.prefetch_pgm {
            !program.is_finished()
        }
        else {
            false
        }
    }

    pub fn in_preload(&self) -> bool {
        matches!(self.run_state, RunState::Preload)
    }

    /// Return the status of the BHE pin.
    pub fn bhe(&self) -> bool {
        (self.command_status & COMMAND_BHE_BIT) == 0
    }

    /// Return the status of the A0 line.
    /// The A0 line is the least significant bit of the address bus.
    pub fn a0(&self) -> bool {
        (self.address_latch & 0x01) != 0
    }

    pub fn data_width(&self) -> DataWidth {
        DataWidth::from((self.bhe(), self.a0()))
    }

    /// Handle a bus read operation, either code fetch, memory or IO read.
    /// Code fetches are allowed to underflow in certain circumstances.
    /// TODO: This should probably return a Result instead of setting an internal error condition.
    pub fn handle_bus_read(
        &mut self,
        emu_mem_ops: &[BusOp],
        emu_fetch_ops: &[BusOp],
        cpu_mem_ops: &mut Vec<BusOp>,
        cpu_fetch_ops: &mut Vec<BusOp>,
        q_op: QueueOp,
        log: &mut TraceLogger,
    ) {
        // TODO: Add check for PASV bus state before reading
        if (self.command_status & COMMAND_MRDC_BIT) == 0 {
            let a0 = self.a0();
            // MRDC is active-low. CPU is reading from bus.
            if self.mcycle_state == BusState::CODE {
                // CPU is fetching code.
                if self.prefetch {
                    // We are executing the prefetch program. Feed bytes from program until exhausted.
                    if self.in_prefetch_pgm() {
                        let program = self.prefetch_pgm.as_mut().unwrap();

                        let bytes_fetched = program.read_program(a0, &mut self.code_stream, QueueDataType::Program);

                        self.data_type = QueueDataType::PrefetchProgram;
                        let value = self.code_stream.pop_data_bus();

                        log::trace!(">>> Prefetch program fetch!");
                        trace!(
                            log,
                            ">>> Fetched {} bytes ({:04X}) from prefetch program, new pgm pc: {}",
                            bytes_fetched,
                            value.bus_value(),
                            program.pos()
                        );
                        self.data_bus = value.bus_value();

                        if !self.in_prefetch_pgm() {
                            trace!(log, ">>> Ending prefetch program fetch.");
                            self.prefetch = false;

                            if bytes_fetched == 1 {
                                // One byte of the program is the next-fetch byte, ie, part of the user program.
                                // We need to accept the emulator fetch for this byte.

                                // Add emu op to CPU FetchOp list
                                self.data_type = QueueDataType::PrefetchProgramHalf;
                                cpu_fetch_ops.push(emu_fetch_ops[self.fetchop_n]);
                                self.fetchop_n += 1;
                            }
                        }

                        self.cpu_client
                            .write_data_bus(self.data_bus)
                            .expect("Failed to write data bus.");
                    }
                    else {
                        panic!("Prefetch program underflow!");
                    }
                }
                else if self.fetchop_n < emu_fetch_ops.len() {
                    let fetch_addr = emu_fetch_ops[self.fetchop_n].addr as usize;

                    let (fetching_beyond, partial_fetch_left) = self.is_fetch_past_end(fetch_addr, self.instr_end_addr);
                    self.fetching_beyond = fetching_beyond;
                    self.program_ended = self.fetching_beyond;
                    if self.fetching_beyond {
                        trace!(
                            log,
                            "fetching_beyond == {:?} partial_fetch_left == {:?}",
                            fetching_beyond,
                            partial_fetch_left
                        );
                    }

                    if !self.fetching_beyond || partial_fetch_left {
                        // Feed emulator byte to CPU depending on validator mode.
                        match self.mode {
                            ValidatorMode::Cycle => {
                                self.data_bus = emu_fetch_ops[self.fetchop_n].data;
                                self.data_type = QueueDataType::Program;
                            }
                            ValidatorMode::Instruction => {
                                let fetch_addr = emu_fetch_ops[self.fetchop_n].addr;

                                // Are we fetching completely outside the instruction byte range?
                                if fetch_addr >= self.instr_end_addr as u32 || fetch_addr < self.instr_addr {
                                    self.data_type = QueueDataType::Finalize;
                                    self.data_bus = OPCODE_NOPS;
                                }
                                else if matches!(self.data_width, DataWidth::Sixteen)
                                    && fetch_addr == self.instr_end_addr as u32 - 1
                                {
                                    // We are fetching the last byte of the instruction, but as 16 bits.
                                    // The second byte of the instruction is not valid.

                                    // Check if we've visited this instruction already
                                    if self.visited_fetches.contains(&fetch_addr) {
                                        // We've already fetched this word - we've jumped back into the instruction and should end.
                                        trace!(
                                            log,
                                            "Already-visisted fetch: {:05X} Likely flow control into instruction bytes. Ending.",
                                            emu_fetch_ops[self.fetchop_n].addr,
                                        );

                                        self.data_type = QueueDataType::Finalize;
                                        self.data_bus = OPCODE_NOPS;
                                    }
                                    else {
                                        trace!(
                                            log,
                                            "accepting half of fetch: {:05X} < {:05X}",
                                            emu_fetch_ops[self.fetchop_n].addr,
                                            self.instr_end_addr
                                        );
                                        self.data_bus =
                                            (emu_fetch_ops[self.fetchop_n].data & 0xFF) | ((OPCODE_NOP as u16) << 8);
                                        self.data_type = QueueDataType::FinalizeHalf;
                                    }
                                }
                                else {
                                    // This fetch appears to be complete within the instruction, but it is only valid if the
                                    // fetch hasn't already been visited (We may have jumped back into the instruction).
                                    if self.visited_fetches.contains(&fetch_addr) {
                                        // We've already fetched this byte - we've jumped back into the instruction and should end.
                                        trace!(
                                            log,
                                            "Already-visisted fetch: {:05X} Likely flow control into instruction bytes. Ending.",
                                            emu_fetch_ops[self.fetchop_n].addr,
                                        );

                                        self.data_type = QueueDataType::Finalize;
                                        self.data_bus = OPCODE_NOPS;
                                    }
                                    else {
                                        trace!(
                                            log,
                                            "accepting fetch: {:05X} < {:05X}",
                                            emu_fetch_ops[self.fetchop_n].addr,
                                            self.instr_end_addr
                                        );
                                        self.data_bus = emu_fetch_ops[self.fetchop_n].data;
                                        self.data_type = QueueDataType::Program;
                                    }
                                }
                            }
                        }

                        // Add emu op to CPU FetchOp list
                        cpu_fetch_ops.push(emu_fetch_ops[self.fetchop_n]);
                        self.visited_fetches.insert(emu_fetch_ops[self.fetchop_n].addr);
                        self.v_pc += 1;

                        if emu_fetch_ops[self.fetchop_n].addr != self.address_latch {
                            trace!(log, "CPU fetch address != EMU fetch address");
                        }
                        trace!(
                            log,
                            "CPU fetch: [{:05X}][{:05X}] -> 0x{:02X} cycle: {}",
                            self.address_latch,
                            emu_fetch_ops[self.fetchop_n].addr,
                            self.data_bus,
                            self.cycle_num
                        );

                        self.cpu_client
                            .write_data_bus(self.data_bus)
                            .expect("Failed to write data bus.");

                        self.fetchop_n += 1;
                    }
                    else {
                        // We are fetching beyond the end of the current instruction (fetched from instr_end_addr) ...
                        // flag the byte in the queue so that we know to end this instruction when
                        // it is read out from the queue.

                        if emu_fetch_ops[self.fetchop_n].addr != self.address_latch {
                            trace!(log, "CPU fetch address != EMU fetch address");
                        }

                        trace!(
                            log,
                            "CPU fetch next: [{:05X}][{:05X}] -> 0x{:02X} cycle: {}",
                            self.address_latch,
                            emu_fetch_ops[self.fetchop_n].addr,
                            emu_fetch_ops[self.fetchop_n].data,
                            self.cycle_num
                        );
                        /*
                        log::trace!(
                            "Fetch past end addr: {:05X} >= {:05X} ",
                            emu_mem_ops[self.busop_n].addr,
                            self.instr_end_addr
                        );
                        */
                        // Fetch is past end address, send terminating NOP.

                        cpu_fetch_ops.push(emu_fetch_ops[self.fetchop_n]);

                        // If we've reached the program end address, set the finalize flag on the queue byte so that
                        // program state can be moved to Finalize and registers read out for comparison.

                        // Otherwise we've just reached the end of the instruction, and set the end instruction flag.

                        if let ValidatorMode::Instruction = self.mode {
                            if self.fetching_beyond {
                                self.data_type = QueueDataType::Finalize;
                                self.data_bus = OPCODE_NOPS;
                            }

                            if let QueueOp::Flush = q_op {
                                trace!(
                                    log,
                                    "Queue flush detected during CODE fetch. Sending NOP to be discarded."
                                );
                                self.data_bus = OPCODE_NOPS;
                                self.data_type = QueueDataType::Program;
                                self.cpu_client
                                    .write_data_bus(self.data_bus)
                                    .expect("Failed to write data bus.");
                            }
                            else {
                                // Start the STORE program.
                                trace!(log, "Setting next STORE program byte");
                                self.cpu_client.prefetch_store().expect("Failed to write store program");
                            }
                        }
                        else {
                            if self.program_ended {
                                self.fetching_beyond = true;
                                self.data_type = QueueDataType::Finalize;
                                self.data_bus = OPCODE_NOPS;
                            }
                            else {
                                //log::debug!("Setting data type to EndInstruction: data: {:02X}", emu_mem_ops[self.busop_n].data);
                                self.data_type = QueueDataType::EndInstruction;
                                self.data_bus = emu_mem_ops[self.busop_n].data;
                            }

                            self.cpu_client
                                .write_data_bus(self.data_bus)
                                .expect("Failed to write data bus.");
                            self.fetchop_n += 1;
                        }
                    }
                }
                else {
                    if !self.prefetch && !self.fetching_beyond {
                        trace!(log, "Fatal: fetch underflow within instruction bounds!");
                        self.error = Some(RemoteCpuError::CannotOweMultipleOps);
                        return;
                    }
                    // We are allowed to miss a terminating fetch.

                    // This is because while the emulator ends an instruction at the cycle in which the next
                    // instruction byte is read from the queue, the validator must wait until signalled by
                    // the queue status lines - meaning one cycle later than the emulator.

                    // This means the validator can be expecting a fetch without a busop from the emulator
                    // having been pushed.

                    if self.fetch_rollover {
                        trace!(log, "Can't rollover fetch twice!");
                        self.error = Some(RemoteCpuError::CannotOweMultipleOps);
                    }
                    else if self.busop_n == emu_mem_ops.len() {
                        match self.mode {
                            ValidatorMode::Cycle => {
                                if (self.address_latch as usize) == self.program_end_addr {
                                    self.data_type = QueueDataType::EndInstruction;
                                    self.data_bus = OPCODE_NOPS;
                                }
                                else {
                                    trace!(log, "Fetch op underflow on terminating fetch. Substituting fetch peek.");
                                    // Substitute instruction byte for fetch op.
                                    self.data_bus = self.peek_fetch;
                                    self.data_type = QueueDataType::Program;
                                    self.fetch_rollover = true;
                                }
                            }
                            ValidatorMode::Instruction => {
                                self.data_type = QueueDataType::EndInstruction;
                                self.data_bus = OPCODE_NOPS;
                            }
                        }

                        self.cpu_client
                            .write_data_bus(self.data_bus)
                            .expect("Failed to write data bus.");
                    }
                    else {
                        match self.mode {
                            ValidatorMode::Instruction => {
                                trace!(log, "Fetch op underflow. Substituting NOP");
                                self.data_type = QueueDataType::EndInstruction;
                                self.data_bus = OPCODE_NOPS;
                            }
                            ValidatorMode::Cycle => {
                                trace!(log, "Fetch op underflow in cycle mode. Cannot continue");
                                self.error = Some(RemoteCpuError::CannotOweMultipleOps);
                            }
                        }
                    }
                }

                /*
                if self.v_pc < instr.len() {
                    // Feed current instruction to CPU
                    self.data_bus = instr[self.v_pc];
                    self.data_type = QueueDataType::Program;
                    self.v_pc += 1;
                }
                else {
                    // Fetch past end of instruction. Send terminating NOP
                    self.data_bus = OPCODE_NOP;
                    self.data_type = QueueDataType::Finalize;
                }
                */

                //log::trace!("CPU fetch: {:02X}", self.data_bus);
                //self.cpu_client.write_data_bus(self.data_bus).expect("Failed to write data bus.");
            }
            else if self.bus_state == BusState::MEMR {
                // CPU is reading data from memory.

                if self.busop_n < emu_mem_ops.len() {
                    //assert!(emu_mem_ops[self.busop_n].op_type == BusOpType::MemRead);

                    // Feed emulator byte to CPU
                    self.data_bus = emu_mem_ops[self.busop_n].data;
                    // Add emu op to CPU BusOp list
                    cpu_mem_ops.push(emu_mem_ops[self.busop_n]);
                    self.busop_n += 1;

                    trace!(log, "Bus OP {:02}: CPU read: {:02X}", self.busop_n, self.data_bus);
                    self.cpu_client
                        .write_data_bus(self.data_bus)
                        .expect("Failed to write data bus.");
                }
                else if !self.ignore_underflow {
                    self.error = Some(RemoteCpuError::BusOpUnderflow);
                }
                else {
                    trace!(
                        log,
                        "Bus op underflow on MEMR with ignore_underflow==true. Substituing 0."
                    );

                    cpu_mem_ops.push(BusOp {
                        op_type: BusOpType::MemRead,
                        addr: self.address_latch,
                        bhe: self.command_status & COMMAND_BHE_BIT == 0,
                        data: 0x0,
                        flags: 0,
                    });
                    self.cpu_client.write_data_bus(0xFF).expect("Failed to write data bus.");
                }
            }
        }

        // IORC status is active-low.
        if (self.command_status & COMMAND_IORC_BIT) == 0 {
            // CPU is reading from IO address.

            if self.busop_n < emu_mem_ops.len() {
                // Feed emulator byte to CPU
                self.data_bus = emu_mem_ops[self.busop_n].data;
                // Add emu op to CPU BusOp list
                cpu_mem_ops.push(emu_mem_ops[self.busop_n].clone());
                self.busop_n += 1;

                trace!(log, "Bus OP {:02}: CPU IN: {:02X}", self.busop_n, self.data_bus);
                self.cpu_client
                    .write_data_bus(self.data_bus)
                    .expect("Failed to write data bus.");
            }
            else {
                trace!(log, "Bus op underflow on IN. Substituing 0xFF");
                //self.error = Some(RemoteCpuError::BusOpUnderflow)
                self.cpu_client.write_data_bus(0xFF).expect("Failed to write data bus.");
            }
        }
    }

    pub fn is_fetch_past_end(&self, fetch_addr: usize, end_addr: usize) -> (bool, bool) {
        match self.width {
            CpuWidth::Eight => (fetch_addr >= end_addr, false),
            CpuWidth::Sixteen => (fetch_addr >= (end_addr - 1), (end_addr - fetch_addr) == 1),
        }
    }

    pub fn handle_bus_write(&mut self, emu_mem_ops: &Vec<BusOp>, cpu_mem_ops: &mut Vec<BusOp>, log: &mut TraceLogger) {
        // MWTC status is active-low.
        if ((self.command_status & COMMAND_AMWC_BIT) == 0) || ((self.command_status & COMMAND_MWTC_BIT) == 0) {
            // CPU is writing to bus. MWTC is only active on t3, so we don't need an additional check.

            // We need to ignore writes during preloading, as we may be executing STOSB as part of a prefetch program.
            if !self.program_started {
                trace!(log, "Ignoring write during prefetch program.");
                return;
            }

            if self.busop_n < emu_mem_ops.len() {
                //assert!(emu_mem_ops[self.busop_n].op_type == BusOpType::MemWrite);

                // Read byte from CPU
                self.data_bus = self.cpu_client.read_data_bus().expect("Failed to read data bus.");

                trace!(
                    log,
                    "Bus OP {:02}: CPU write: [{:05X}] <- {:02X}",
                    self.busop_n,
                    self.address_latch,
                    self.data_bus
                );

                // Add write op to CPU BusOp list
                cpu_mem_ops.push(BusOp {
                    op_type: BusOpType::MemWrite,
                    addr: self.address_latch,
                    bhe: self.command_status & COMMAND_BHE_BIT == 0,
                    data: self.data_bus,
                    flags: 0,
                });
                self.busop_n += 1;
            }
            else if !self.ignore_underflow {
                trace!(log, "Bus op underflow on write.");
                self.error = Some(RemoteCpuError::BusOpUnderflow);
            }
            else {
                trace!(log, "Bus op underflow on write with ignore_underflow==true");
                cpu_mem_ops.push(BusOp {
                    op_type: BusOpType::MemWrite,
                    addr: self.address_latch,
                    bhe: self.command_status & COMMAND_BHE_BIT == 0,
                    data: self.data_bus,
                    flags: 0,
                });
            }
        }

        // IOWC status is active-low.
        if ((self.command_status & COMMAND_AIOWC_BIT) == 0) || ((self.command_status & COMMAND_IOWC_BIT) == 0) {
            // CPU is writing to IO address.

            if self.busop_n < emu_mem_ops.len() {
                // Read byte from CPU
                self.data_bus = self.cpu_client.read_data_bus().expect("Failed to read data bus.");

                trace!(log, "CPU OUT: [{:05X}] <- {:02X}", self.address_latch, self.data_bus);

                // Add write op to CPU BusOp list
                cpu_mem_ops.push(BusOp {
                    op_type: BusOpType::IoWrite,
                    addr: self.address_latch,
                    bhe: self.command_status & COMMAND_BHE_BIT == 0,
                    data: self.data_bus,
                    flags: 0,
                });
                self.busop_n += 1;
            }
            else {
                log::error!("Bus op underflow on OUT");
                self.error = Some(RemoteCpuError::BusOpUnderflow);
            }
        }
    }

    pub fn cycle(
        &mut self,
        _instr: &[u8],
        emu_fetch_ops: &Vec<BusOp>,
        emu_mem_ops: &Vec<BusOp>,
        cpu_fetch_ops: &mut Vec<BusOp>,
        cpu_mem_ops: &mut Vec<BusOp>,
        initial_queue: &mut Vec<u8>,
        log: &mut TraceLogger,
    ) -> Result<CycleState, ValidatorError> {
        // Disable cycling; update_state(); will cycle cpu
        //self.cpu_client.cycle().expect("Failed to cycle cpu!");
        self.cycle_num += 1;

        //log::trace!("Cycle #{}", self.cycle_num);

        // Transition into next state.
        self.bus_cycle = match self.bus_cycle {
            BusCycle::Ti => {
                // We get out of Ti state on ALE
                BusCycle::Ti
            }
            BusCycle::T1 => {
                // Capture the state of the bus transfer in T1, as the state will go PASV in t3-t4
                //self.mcycle_state = get_bus_state!(self.status);

                // Only exit T1 state if bus transfer state indicates a bus transfer
                match self.mcycle_state {
                    BusState::PASV => BusCycle::T1,
                    BusState::HALT => BusCycle::T1,
                    _ => BusCycle::T2,
                }
            }
            BusCycle::T2 => BusCycle::T3,
            BusCycle::T3 => {
                // TODO: Handle wait states
                BusCycle::T4
            }
            BusCycle::Tw => {
                // TODO: Handle wait states
                BusCycle::T4
            }
            BusCycle::T4 => BusCycle::T1,
        };

        let mut cycle_info = self.update_state(true);
        if self.validator_state == ProgramState::ExecuteDone {
            return Ok(cycle_info);
        }
        let q_op = get_queue_op!(self.status);

        // Handle current T-state
        match self.bus_cycle {
            BusCycle::Ti => {}
            BusCycle::T1 => {
                // Capture the state of the bus transfer in T1, as the state will go PASV in t3-t4
                self.mcycle_state = get_bus_state!(self.status);
            }
            BusCycle::T2 => {
                self.handle_bus_read(emu_mem_ops, emu_fetch_ops, cpu_mem_ops, cpu_fetch_ops, q_op, log);
            }
            BusCycle::T3 => {
                // TODO: Handle wait states
                if self.is_last_wait() {
                    self.handle_bus_write(emu_mem_ops, cpu_mem_ops, log);
                }
            }
            BusCycle::Tw => {
                // TODO: Handle wait states
                if self.is_last_wait() {
                    self.handle_bus_write(emu_mem_ops, cpu_mem_ops, log);
                }
            }
            BusCycle::T4 => {
                if self.mcycle_state == BusState::CODE {
                    // We completed a code fetch, so add to prefetch queue

                    /*
                    log::trace!(
                        "Queue Push! byte:{:02X}, type: {:?} addr: {:05X} cycle: {}",
                        self.data_bus,
                        self.data_type,
                        self.address_latch,
                        self.cycle_num
                    );
                    */
                    self.queue
                        .push(self.data_bus, self.data_width, self.data_type, self.address_latch);
                    // Reset queue data type
                    self.data_type = QueueDataType::Program;
                }
            }
        };

        if (self.control_status & CONTROL_ALE_BIT) != 0 {
            if self.bus_cycle != BusCycle::T1 {
                trace!(
                    log,
                    "ALE on non-T1 cycle state! ({:?}) CPU desynchronized. Cycle: {}",
                    self.bus_cycle,
                    self.cycle_num
                );
                //self.bus_cycle = BusCycle::T1;
                return Err(ValidatorError::CpuDesynced);
            }

            let addr = self
                .cpu_client
                .read_address_latch()
                .expect("Failed to get address latch!");
            self.address_latch = addr;
            self.address_bus = addr;

            cycle_info.addr = self.address_latch;
        }
        else {
            self.address_bus = self.cpu_client.read_address().expect("Failed to get address bus!");
            cycle_info.addr = self.address_bus;
        }

        // Handle queue activity
        match q_op {
            QueueOp::First | QueueOp::Subsequent => {
                // We fetched a byte from the queue last cycle

                // Save the previous queue state for setting initial queue.
                let prev_queue = self.queue.to_vec();
                (self.queue_byte, self.queue_type, self.queue_fetch_addr) = self.queue.pop();

                if q_op == QueueOp::First {
                    // First byte of instruction fetched.
                    self.queue_fetch_n = 0;
                    self.opcode = self.queue_byte;

                    match (self.run_state, self.queue_type) {
                        (_, QueueDataType::Finalize) => {
                            trace!(
                                log,
                                "Byte read from queue with Finalize flag set. Finalizing execution."
                            );

                            if let Err(e) = self.cpu_client.finalize() {
                                trace!(log, "Client error: Failed to finalize! Err: {}", e);
                                return Err(ValidatorError::CpuError);
                            }
                            self.end_instruction = true;
                            self.finalize = true;
                            self.run_state = RunState::Finalize;
                        }
                        (RunState::Preload, QueueDataType::Program) => {
                            // We are transitioning from the preload program to the main program.
                            trace!(log, "Main program started at cycle: {}", self.cycle_num);
                            // Save the initial queue state.
                            *initial_queue = prev_queue;
                            self.program_started = true;
                            self.run_state = RunState::Program;
                        }
                        (_, _) => {}
                    }
                }
                else {
                    // Subsequent byte of instruction fetched

                    if (self.queue_fetch_addr as usize) == self.instr_end_addr {
                        // We popped a subsequent byte that is out of range of the current instruction. This is an invalid state.
                        trace!(log, "Popped subsequent byte out of range!");
                        self.error = Some(RemoteCpuError::SubsequentByteFetchOutOfRange(
                            self.queue_fetch_addr,
                            self.instr_end_addr as u32,
                        ));
                        self.end_instruction = true;
                    }

                    self.queue_fetch_n += 1;
                }
            }
            QueueOp::Flush => {
                trace!(log, " >>> Flush!");
                // Queue was flushed last cycle
                self.flushed = true;
                self.queue.flush();
            }
            _ => {}
        }

        cycle_info.data_bus = self.data_bus;
        cycle_info.q_byte = self.queue_byte;
        self.queue.to_slice(&mut cycle_info.q);
        cycle_info.q_len = self.queue.len() as u32;
        cycle_info.n = self.cycle_num;

        Ok(cycle_info)
    }

    pub fn get_error(&self) -> Option<RemoteCpuError> {
        self.error
    }

    pub fn get_states(&self) -> &Vec<CycleState> {
        &self.cycle_states
    }

    pub fn prefetch_pgm_bytes(cpu_type: CpuType) -> &'static [u8] {
        match cpu_type {
            CpuType::Intel8088 | CpuType::Intel8086 => &INTEL808X_PREFETCH_PGM,
            CpuType::NecV20(_) | CpuType::NecV30(_) => &NECVX0_PREFETCH_PGM,
        }
    }

    pub fn prefetch_program(&self) -> Option<&RemoteProgram> {
        if let Some(program) = &self.prefetch_pgm {
            return Some(program);
        }
        None
    }

    pub fn step(
        &mut self,
        mode: ValidatorMode,
        prefetch: bool,
        instr: &[u8],
        instr_addr: u32,
        cycle_trace: bool,
        peek_fetch: u16,
        emu_fetch_ops: &Vec<BusOp>,
        emu_mem_ops: &Vec<BusOp>,
        cpu_fetch_ops: &mut Vec<BusOp>,
        cpu_mem_ops: &mut Vec<BusOp>,
        initial_queue: &mut Vec<u8>,
        ignore_underflow: bool,
        log: &mut TraceLogger,
    ) -> Result<(Vec<CycleState>, bool), ValidatorError> {
        self.error = None;
        self.mode = mode;
        self.prefetch = prefetch;
        self.instr_addr = instr_addr;
        self.peek_fetch = peek_fetch;
        self.busop_n = 0;
        self.fetchop_n = 0;
        self.queue_first_fetch = false;
        self.rni = false;
        self.v_pc = 0;
        self.cycle_num = 0;
        self.end_instruction = false;
        self.finalize = false;
        self.flushed = false;
        self.discard_front = false;
        self.fetching_beyond = false;
        self.ignore_underflow = ignore_underflow;
        self.cycle_states.clear();

        // Do prefetch program initialization
        if self.prefetch {
            self.program_started = false;
            self.run_state = RunState::Preload;
        }
        else {
            // Start the program immediately if not prefetching.
            self.program_started = true;
            self.run_state = RunState::Program;
        }

        self.address_latch = self
            .cpu_client
            .read_address_latch()
            .expect("Failed to get address latch!");

        let mut cycle_vec: Vec<CycleState> = Vec::new();

        // Include post-reset cycle state if we just reset the CPU
        // as reset includes the first T1 cycle with ALE.
        if self.just_reset {
            let cycle_state = self.update_state(false);
            cycle_vec.push(cycle_state);
            self.just_reset = false;
        }

        // Discard first fetch if we are rolling over a missed terminating fetch from the previous instruction.
        if self.fetch_rollover && (!emu_fetch_ops.is_empty()) {
            trace!(log, "Discarding fetch from previous instruction.");
            self.fetchop_n += 1;
            self.discard_front = true;
            self.fetch_rollover = false;
        }

        // We end an instruction when the QS status lines indicate we have fetched the first byte of the next
        // instruction. But by the time the QS status lines have updated, we are already in the first cycle
        // of that instruction. So we save the last cycle from the previous instruction, and add it to the
        // cycle vector here.

        // We also need to update the queue_first_fetch status if this was a fetch for a non-prefix opcode.
        if let Some(cycle_state) = self.last_cycle_state {
            if cycle_state.q_op == QueueOp::First && !self.is_prefix(cycle_state.q_byte) {
                // This was a fetch for an opcode
                self.queue_first_fetch = true;
            }
            cycle_vec.push(cycle_state);
            self.last_cycle_state = None;
        }

        // cycle trace if enabled
        if cycle_trace {
            //println!("{}", self.get_cpu_state_str());
            trace!(log, "{}", self.get_cpu_state_str());
        }

        while !self.end_instruction {
            let mut cycle_state = match self.cycle(
                instr,
                emu_fetch_ops,
                emu_mem_ops,
                cpu_fetch_ops,
                cpu_mem_ops,
                initial_queue,
                log,
            ) {
                Ok(cycle_state) => cycle_state,
                Err(e) => {
                    trace!(log, "CPU error during step(): {}", e);
                    RemoteCpu::dump_cycles(&cycle_vec);
                    return Err(e);
                }
            };

            if self.program_started && !self.end_instruction {
                cycle_vec.push(cycle_state);
            }
            else {
                cycle_state.n = 0;
                self.last_cycle_state = Some(cycle_state);
            }

            if let Some(e) = &self.error {
                trace!(log, "CPU error during step(): {}", e);
                RemoteCpu::dump_cycles(&cycle_vec);
                self.cycle_states = cycle_vec;
                return Err(ValidatorError::CpuError);
            }

            /*
            if self.cycle_num > 200 {
                trace!(log, "CPU cycle timeout!");
                RemoteCpu::dump_cycles(&cycle_vec);
                return Err(ValidatorError::CpuError);
            }
            */

            // cycle trace if enabled
            if cycle_trace {
                trace!(log, "{}", self.get_cpu_state_str());
            }
        }

        if let Some(prefetch_pgm) = &mut self.prefetch_pgm {
            // Reset prefetch program
            prefetch_pgm.reset();
            trace!(log, "Resetting prefetch program");
        }

        Ok((cycle_vec, self.discard_front))
    }

    pub fn dump_cycles(cycles: &Vec<CycleState>) {
        for cycle in cycles {
            log::warn!("{}", cycle);
        }
    }

    pub fn in_finalize(&mut self) -> bool {
        self.finalize
    }

    pub fn load(&mut self, reg_buf: &[u8]) -> Result<bool, CpuClientError> {
        self.cpu_client.load_registers_from_buf(reg_buf)?;
        Ok(true)
    }

    pub fn store(&mut self) -> Result<VRegisters, CpuClientError> {
        let mut buf: [u8; 28] = [0; 28];
        self.cpu_client.store_registers_to_buf(&mut buf)?;

        Ok(ArduinoValidator::buf_to_regs(&buf))
    }

    pub fn get_last_error(&mut self) -> Result<String, CpuClientError> {
        self.cpu_client.get_last_error()
    }

    pub fn calc_linear_address(segment: u16, offset: u16) -> u32 {
        (((segment as u32) << 4) + offset as u32) & 0xFFFFF
    }

    /// Adjust stored IP register to address of terminating opcode fetch
    pub fn adjust_ip(&mut self, regs: &mut VRegisters) {
        let flat_csip = RemoteCpu::calc_linear_address(regs.cs, regs.ip);

        let ip_offset = flat_csip.wrapping_sub(self.queue_fetch_addr);

        regs.ip = regs.ip.wrapping_sub(ip_offset as u16);
    }

    pub fn queue(&self) -> Vec<u8> {
        self.queue.to_vec()
    }

    pub fn get_queue_str(q: &[u8], len: usize) -> String {
        let mut outer = "[".to_string();
        let mut inner = String::new();

        for q_byte in q.iter().take(len) {
            inner.push_str(&format!("{:02X}", *q_byte));
        }

        outer.push_str(&format!("{:8}]", inner));
        outer
    }

    pub fn get_cpu_state_str(&mut self) -> String {
        let ale_str = match self.control_status & CONTROL_ALE_BIT != 0 {
            true => "A:",
            false => "  ",
        };

        let mut seg_str = "  ";
        if self.bus_cycle != BusCycle::T1 {
            // Segment status only valid in T2+
            seg_str = match get_segment!(self.status) {
                Segment::ES => "ES",
                Segment::SS => "SS",
                Segment::CS => "CS",
                Segment::DS => "DS",
            };
        }

        let q_op = get_queue_op!(self.status);
        let q_op_chr = match q_op {
            QueueOp::Idle => ' ',
            QueueOp::First => 'F',
            QueueOp::Flush => 'E',
            QueueOp::Subsequent => 'S',
        };

        // All read/write signals are active/low
        let rs_chr = match self.command_status & 0b0000_0001 == 0 {
            true => 'R',
            false => '.',
        };
        let aws_chr = match self.command_status & 0b0000_0010 == 0 {
            true => 'A',
            false => '.',
        };
        let ws_chr = match self.command_status & 0b0000_0100 == 0 {
            true => 'W',
            false => '.',
        };
        let ior_chr = match self.command_status & 0b0000_1000 == 0 {
            true => 'R',
            false => '.',
        };
        let aiow_chr = match self.command_status & 0b0001_0000 == 0 {
            true => 'A',
            false => '.',
        };

        let iow_chr = match self.command_status & 0b0010_0000 == 0 {
            true => 'W',
            false => '.',
        };

        let bus_str = match get_bus_state!(self.status) {
            BusState::INTA => "INTA",
            BusState::IOR => "IOR ",
            BusState::IOW => "IOW ",
            BusState::HALT => "HALT",
            BusState::CODE => "CODE",
            BusState::MEMR => "MEMR",
            BusState::MEMW => "MEMW",
            BusState::PASV => "PASV",
        };

        let t_str = match self.bus_cycle {
            BusCycle::Ti => "Ti",
            BusCycle::T1 => "T1",
            BusCycle::T2 => "T2",
            BusCycle::T3 => "T3",
            BusCycle::T4 => "T4",
            BusCycle::Tw => "Tw",
        };

        let is_reading = is_reading!(self.command_status);
        let is_writing = is_writing!(self.command_status);

        let mut xfer_str = "        ".to_string();
        if is_reading {
            xfer_str = format!("R-> {:04X}", self.data_bus);
        }
        else if is_writing {
            xfer_str = format!("<-W {:04X}", self.data_bus);
        }

        // Handle queue activity

        let mut q_read_str = String::new();

        if q_op == QueueOp::First {
            // First byte of opcode read from queue. Decode it to opcode or group specifier
            if !self.program_started {
                q_read_str = format!("<-q {:02X} ({:?})", self.queue_byte, self.queue_type);
            }
            else {
                q_read_str = format!("<-q {:02X} {} ({:?})", self.queue_byte, self.instr_str, self.queue_type);
            }
        }
        else if q_op == QueueOp::Subsequent {
            q_read_str = format!("<-q {:02X}", self.queue_byte);
        }

        format!(
            "{:08} {:02}[{:05X}] {:02} M:{}{}{} I:{}{}{} {:04} {:02} {:06} | {:1}{:1} [{:08}] {}",
            self.cycle_num,
            ale_str,
            self.address_latch,
            seg_str,
            rs_chr,
            aws_chr,
            ws_chr,
            ior_chr,
            aiow_chr,
            iow_chr,
            bus_str,
            t_str,
            xfer_str,
            q_op_chr,
            self.queue.len(),
            self.queue.to_string(),
            q_read_str
        )
    }

    pub fn print_regs(regs: &VRegisters) {
        println!("{}", regs);
    }

    pub fn get_reg_str(regs: &VRegisters) -> String {
        regs.to_string()
    }

    pub fn flags_string(f: u16) -> String {
        let c_chr = if CPU_FLAG_CARRY & f != 0 { 'C' } else { 'c' };
        let p_chr = if CPU_FLAG_PARITY & f != 0 { 'P' } else { 'p' };
        let a_chr = if CPU_FLAG_AUX_CARRY & f != 0 { 'A' } else { 'a' };
        let z_chr = if CPU_FLAG_ZERO & f != 0 { 'Z' } else { 'z' };
        let s_chr = if CPU_FLAG_SIGN & f != 0 { 'S' } else { 's' };
        let t_chr = if CPU_FLAG_TRAP & f != 0 { 'T' } else { 't' };
        let i_chr = if CPU_FLAG_INT_ENABLE & f != 0 { 'I' } else { 'i' };
        let d_chr = if CPU_FLAG_DIRECTION & f != 0 { 'D' } else { 'd' };
        let o_chr = if CPU_FLAG_OVERFLOW & f != 0 { 'O' } else { 'o' };

        format!(
            "1111{}{}{}{}{}{}0{}0{}1{}",
            o_chr, d_chr, i_chr, t_chr, s_chr, z_chr, a_chr, p_chr, c_chr
        )
    }
}

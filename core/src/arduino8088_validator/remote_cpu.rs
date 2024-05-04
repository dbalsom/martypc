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
*/

use core::fmt::Display;
use std::error::Error;

use crate::cpu_validator::*;

use super::{BusOp, BusOpType, QueueOp, OPCODE_NOP};
use crate::{
    arduino8088_client::*,
    arduino8088_validator::{queue::*, *},
};

macro_rules! trace {
    ($log:ident, $($t:tt)*) => {{
        $log.print(&format!($($t)*));
        $log.print("\n".to_string());
    }};
}

const ADDRESS_SPACE: usize = 1_048_576;

#[derive(Copy, Clone, Debug)]
pub enum RemoteCpuError {
    BusOpAddressMismatch(u32, u32),
    SubsequentByteFetchOutOfRange(u32, u32),
    CannotOweMultipleOps,
    BusOpUnderflow,
}

impl Error for RemoteCpuError {}
impl Display for RemoteCpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &*self {
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

pub struct RemoteCpu {
    cpu_client: CpuClient,
    mode: ValidatorMode,
    regs: RemoteCpuRegisters,
    memory: Vec<u8>,
    pc: usize,
    instr_end_addr: usize,
    program_end_addr: usize,
    program_state: ProgramState,

    address_latch: u32,
    status: u8,
    command_status: u8,
    control_status: u8,
    data_bus: u8,
    data_type: QueueDataType,

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
    instr_addr: u32,
    end_instruction: bool,
    finalize: bool,
    flushed: bool,
    discard_front: bool,
    error: Option<RemoteCpuError>,
    cycle_states: Vec<CycleState>,

    // Validator stuff
    busop_n: usize,
    owe_busop: bool,
    fetch_rollover: bool,
    instruction_ended: bool,
    program_ended: bool,
    prefetch_n: usize,
    v_pc: usize,
}

impl RemoteCpu {
    pub fn new(cpu_client: CpuClient) -> RemoteCpu {
        RemoteCpu {
            cpu_client,
            mode: ValidatorMode::Instruction,
            regs: Default::default(),
            memory: vec![0; ADDRESS_SPACE],
            pc: 0,
            instr_end_addr: 0,
            program_end_addr: 0,
            program_state: ProgramState::Reset,
            address_latch: 0,
            status: 0,
            command_status: 0,
            control_status: 0,
            data_bus: 0,
            data_type: QueueDataType::Program,
            cycle_num: 0,
            mcycle_state: BusState::PASV,
            bus_state: BusState::PASV,
            bus_cycle: BusCycle::T1,
            access_type: AccessType::CodeOrNone,
            wait_states: 0,
            just_reset: false,
            last_cycle_state: None,
            queue: InstructionQueue::new(),
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
            end_instruction: false,
            finalize: false,
            flushed: false,
            discard_front: false,
            error: None,
            cycle_states: Vec::new(),

            busop_n: 0,
            owe_busop: false,
            fetch_rollover: false,
            instruction_ended: false,
            program_ended: false,
            prefetch_n: 0,
            v_pc: 0,
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
                    self.program_state,
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
                    self.program_state,
                    self.control_status,
                    self.status,
                    self.command_status,
                    self.data_bus,
                ) = self.cpu_client.get_cycle_state().expect("Failed to get cycle state!");
            }
        }

        self.access_type = get_access_type!(self.status);
        self.bus_state = get_bus_state!(self.status);
        let q_op = get_queue_op!(self.status);
        let mut q = [0; 4];
        self.queue.to_slice(&mut q);

        CycleState {
            n: self.cycle_num,
            addr: self.address_latch,
            t_state: self.bus_cycle,
            a_type: self.access_type,
            b_state: self.bus_state,
            ale: self.command_status & COMMAND_ALE_BIT != 0,
            mrdc: self.command_status & COMMAND_MRDC_BIT != 0,
            amwc: self.command_status & COMMAND_AMWC_BIT != 0,
            mwtc: self.command_status & COMMAND_MWTC_BIT != 0,
            iorc: self.command_status & COMMAND_IORC_BIT != 0,
            aiowc: self.command_status & COMMAND_AIOWC_BIT != 0,
            iowc: self.command_status & COMMAND_IOWC_BIT != 0,
            inta: self.command_status & COMMAND_INTA_BIT != 0,
            q_op: q_op,
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

    pub fn reset(&mut self) {
        self.bus_cycle = BusCycle::T1;
        self.mcycle_state = BusState::CODE; // First state after reset is a code fetch
        self.last_cycle_state = None;
        self.instruction_ended = false;
        self.program_ended = false;
        self.fetch_rollover = false;
        self.just_reset = true;
        self.queue.flush();
        self.cycle_states.clear();
    }

    pub fn is_last_wait(&self) -> bool {
        if self.bus_cycle == BusCycle::T3 && self.wait_states == 0 {
            true
        }
        else if self.bus_cycle == BusCycle::Tw && self.wait_states == 0 {
            true
        }
        else {
            false
        }
    }

    pub fn is_prefix(opcode: u8) -> bool {
        match opcode {
            0x26 | 0x2E | 0x36 | 0x3E | 0xF0 | 0xF2 | 0xF3 => true,
            _ => false,
        }
    }

    pub fn handle_bus_read(&mut self, emu_mem_ops: &Vec<BusOp>, cpu_mem_ops: &mut Vec<BusOp>, log: &mut TraceLogger) {
        if (self.command_status & COMMAND_MRDC_BIT) == 0 {
            // MRDC is active-low. CPU is reading from bus.

            if self.mcycle_state == BusState::CODE {
                // CPU is reading code.

                //log::debug!("bus_state: {:?} bus ops len: {}", self.bus_state, emu_mem_ops.len());
                if self.busop_n < emu_mem_ops.len() {
                    // Bus type must match
                    //assert!(emu_mem_ops[self.busop_n].op_type == BusOpType::CodeRead);

                    if (emu_mem_ops[self.busop_n].addr as usize) == self.instr_end_addr {
                        self.instruction_ended = true;
                    }

                    if (emu_mem_ops[self.busop_n].addr as usize) == self.program_end_addr {
                        self.program_ended = true;
                    }

                    if !self.instruction_ended {
                        /*
                        if instr[self.v_pc] != emu_mem_ops[self.busop_n].data {
                            log::error!(
                                "Emu fetch op doesn't match instruction vector byte: Fetch: {:02X} Instr: {:02X}",
                                emu_mem_ops[self.busop_n].data,
                                instr[self.v_pc]
                            );
                        }
                        */

                        // Feed emulator byte to CPU depending on validator mode.
                        match self.mode {
                            ValidatorMode::Cycle => {
                                self.data_bus = emu_mem_ops[self.busop_n].data;
                                self.data_type = QueueDataType::Program;
                            }
                            ValidatorMode::Instruction => {
                                if emu_mem_ops[self.busop_n].addr as usize >= self.instr_end_addr {
                                    self.data_type = QueueDataType::Finalize;
                                    self.data_bus = OPCODE_NOP;
                                }
                                else {
                                    trace!(
                                        log,
                                        "accepting fetch: {:05X} < {:05X}",
                                        emu_mem_ops[self.busop_n].addr,
                                        self.instr_end_addr
                                    );
                                    self.data_bus = emu_mem_ops[self.busop_n].data;
                                    self.data_type = QueueDataType::Program;
                                }
                            }
                        }

                        // Add emu op to CPU BusOp list
                        cpu_mem_ops.push(emu_mem_ops[self.busop_n].clone());

                        self.v_pc += 1;

                        if emu_mem_ops[self.busop_n].addr != self.address_latch {
                            trace!(log, "CPU fetch address != EMU fetch address");
                        }
                        trace!(
                            log,
                            "CPU fetch: [{:05X}][{:05X}] -> 0x{:02X} cycle: {}",
                            self.address_latch,
                            emu_mem_ops[self.busop_n].addr,
                            self.data_bus,
                            self.cycle_num
                        );

                        self.cpu_client
                            .write_data_bus(self.data_bus)
                            .expect("Failed to write data bus.");
                        self.busop_n += 1;
                    }
                    else {
                        // We have reached the end of the current instruction (fetched from instr_end_addr) ...
                        // flag the byte in the queue so we know to end this instruction when it is read out from the queue.

                        if emu_mem_ops[self.busop_n].addr != self.address_latch {
                            trace!(log, "CPU fetch address != EMU fetch address");
                        }

                        trace!(
                            log,
                            "CPU fetch next: [{:05X}][{:05X}] -> 0x{:02X} cycle: {}",
                            self.address_latch,
                            emu_mem_ops[self.busop_n].addr,
                            emu_mem_ops[self.busop_n].data,
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

                        cpu_mem_ops.push(emu_mem_ops[self.busop_n].clone());

                        // If we've reached the program end address, set the finalize flag on the queue byte so that
                        // program state can be moved to Finalize and registers read out for comparison.

                        // Otherwise we've just reached the end of the instruction, and set the end instruction flag.

                        if let ValidatorMode::Instruction = self.mode {
                            if self.instruction_ended {
                                self.data_type = QueueDataType::Finalize;
                                self.data_bus = OPCODE_NOP;
                            }
                        }
                        else {
                            if self.program_ended {
                                self.instruction_ended = true;
                                self.data_type = QueueDataType::Finalize;
                                self.data_bus = OPCODE_NOP;
                            }
                            else {
                                //log::debug!("Setting data type to EndInstruction: data: {:02X}", emu_mem_ops[self.busop_n].data);
                                self.data_type = QueueDataType::EndInstruction;
                                self.data_bus = emu_mem_ops[self.busop_n].data;
                            }
                        }

                        self.cpu_client
                            .write_data_bus(self.data_bus)
                            .expect("Failed to write data bus.");
                        self.busop_n += 1;
                    }
                }
                else {
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
                    else {
                        if self.busop_n == emu_mem_ops.len() {
                            match self.mode {
                                ValidatorMode::Cycle => {
                                    if ((self.address_latch as usize) == self.program_end_addr) {
                                        self.data_type = QueueDataType::EndInstruction;
                                        self.data_bus = OPCODE_NOP;
                                    }
                                    else {
                                        trace!(log, "Bus op underflow on terminating fetch. Substituting fetch peek.");
                                        // Substitute instruction byte for fetch op.
                                        self.data_bus = (self.peek_fetch & 0xFF) as u8;
                                        self.data_type = QueueDataType::Program;
                                        self.fetch_rollover = true;
                                    }
                                }
                                ValidatorMode::Instruction => {
                                    self.data_type = QueueDataType::EndInstruction;
                                    self.data_bus = OPCODE_NOP;
                                }
                            }

                            self.cpu_client
                                .write_data_bus(self.data_bus)
                                .expect("Failed to write data bus.");
                        }
                        else {
                            trace!(log, "Bus op underflow past terminating fetch.");
                            self.error = Some(RemoteCpuError::CannotOweMultipleOps);
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
                    cpu_mem_ops.push(emu_mem_ops[self.busop_n].clone());
                    self.busop_n += 1;

                    trace!(log, "CPU read: {:02X}", self.data_bus);
                    self.cpu_client
                        .write_data_bus(self.data_bus)
                        .expect("Failed to write data bus.");
                }
                else {
                    self.error = Some(RemoteCpuError::BusOpUnderflow);
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

                trace!(log, "CPU IN: {:02X}", self.data_bus);
                self.cpu_client
                    .write_data_bus(self.data_bus)
                    .expect("Failed to write data bus.");
            }
            else {
                trace!(log, "Bus op underflow on IN. Substituing 0xFF");
                ///self.error = Some(RemoteCpuError::BusOpUnderflow)
                ///
                self.cpu_client.write_data_bus(0xFF).expect("Failed to write data bus.");
            }
        }
    }

    pub fn handle_bus_write(&mut self, emu_mem_ops: &Vec<BusOp>, cpu_mem_ops: &mut Vec<BusOp>, log: &mut TraceLogger) {
        // MWTC status is active-low.
        if ((self.command_status & COMMAND_AMWC_BIT) == 0) || ((self.command_status & COMMAND_MWTC_BIT) == 0) {
            // CPU is writing to bus. MWTC is only active on t3 so we don't need an additional check.

            if self.busop_n < emu_mem_ops.len() {
                //assert!(emu_mem_ops[self.busop_n].op_type == BusOpType::MemWrite);

                // Read byte from CPU
                self.data_bus = self.cpu_client.read_data_bus().expect("Failed to read data bus.");

                trace!(log, "CPU write: [{:05X}] <- {:02X}", self.address_latch, self.data_bus);

                // Add write op to CPU BusOp list
                cpu_mem_ops.push(BusOp {
                    op_type: BusOpType::MemWrite,
                    addr:    self.address_latch,
                    data:    self.data_bus,
                    flags:   0,
                });
                self.busop_n += 1;
            }
            else {
                trace!(log, "Bus op underflow on write");
                self.error = Some(RemoteCpuError::BusOpUnderflow);
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
                    addr:    self.address_latch,
                    data:    self.data_bus,
                    flags:   0,
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
        instr: &[u8],
        emu_prefetch: &Vec<BusOp>,
        emu_mem_ops: &Vec<BusOp>,
        _cpu_prefetch: &mut Vec<BusOp>,
        cpu_mem_ops: &mut Vec<BusOp>,
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
        if self.program_state == ProgramState::ExecuteDone {
            return Ok(cycle_info);
        }

        // Handle current T-state
        match self.bus_cycle {
            BusCycle::Ti => {}
            BusCycle::T1 => {
                // Capture the state of the bus transfer in T1, as the state will go PASV in t3-t4
                self.mcycle_state = get_bus_state!(self.status);
            }
            BusCycle::T2 => {
                self.handle_bus_read(emu_mem_ops, cpu_mem_ops, log);
            }
            BusCycle::T3 => {
                // TODO: Handle wait states
                if self.is_last_wait() {
                    self.handle_bus_write(emu_mem_ops, cpu_mem_ops, log);
                }
            }
            BusCycle::Tw => {
                // TODO: Handle wait states
                if self.is_last_wait() {}
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

                    self.queue.push(self.data_bus, self.data_type, self.address_latch);
                    // Reset queue data type
                    self.data_type = QueueDataType::Program;
                }
            }
        };

        if (self.command_status & COMMAND_ALE_BIT) != 0 {
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

            self.address_latch = self
                .cpu_client
                .read_address_latch()
                .expect("Failed to get address latch!");
            cycle_info.addr = self.address_latch;
        }

        // Handle queue activity
        let q_op = get_queue_op!(self.status);

        match q_op {
            QueueOp::First | QueueOp::Subsequent => {
                // We fetched a byte from the queue last cycle
                (self.queue_byte, self.queue_type, self.queue_fetch_addr) = self.queue.pop();

                /*
                log::trace!(
                    "Queue pop! byte: {:02X} type: {:?} addr: {:05X} end: {:05X}",
                    self.queue_byte,
                    self.queue_type,
                    self.queue_fetch_addr,
                    self.instr_end_addr
                );
                */

                if q_op == QueueOp::First {
                    // First byte of instruction fetched.

                    self.queue_fetch_n = 0;
                    self.opcode = self.queue_byte;

                    // Is this byte flagged as the end of execution?
                    if self.queue_type == QueueDataType::Finalize {
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
                    }

                    if self.queue_first_fetch {
                        // Popped a byte for the next instruction. End the current instruction execution
                        trace!(
                            log,
                            "Next \"first byte\" read from queue. Ending instruction at cycle: {}",
                            self.cycle_num
                        );
                        self.end_instruction = true;
                    }

                    if !RemoteCpu::is_prefix(self.opcode) {
                        // Prefixes are also flagged as "first byte" fetches.
                        // If not a prefix, we have read the first actual instruction byte.
                        self.queue_first_fetch = true;
                    }

                    /*
                    if (self.queue_fetch_addr as usize) == self.instr_end_addr {
                        // Popped a byte for the next instruction. End the current instruction execution
                        // as the next instruction is starting on the next cycle.
                        log::trace!("Byte read from queue with address past current instruction. Ending instruction.");
                        self.end_instruction = true;
                    }
                    */
                }
                else {
                    // Subsequent byte of instruction fetched

                    if (self.queue_fetch_addr as usize) == self.instr_end_addr {
                        // We popped a subsequent byte that is out of range of the current instruction. This is an invalid state.

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

        cycle_info.data_bus = self.data_bus as u16;
        cycle_info.q_byte = self.queue_byte;
        self.queue.to_slice(&mut cycle_info.q);
        cycle_info.q_len = self.queue.len() as u32;
        cycle_info.n = self.cycle_num;

        Ok(cycle_info)
    }

    /*
    pub fn run(
        &mut self,
        instr: &[u8],
        instr_addr: u32,
        cycle_trace: bool,
        emu_prefetch: &Vec<BusOp>,
        emu_mem_ops: &Vec<BusOp>,
        cpu_prefetch: &mut Vec<BusOp>,
        cpu_mem_ops: &mut Vec<BusOp>
    ) -> Result<Vec::<CycleState>, ValidatorError> {

        self.instr_addr = instr_addr;
        self.busop_n = 0;
        self.prefetch_n = 0;
        self.v_pc = 0;
        self.cycle_num = 0;
        self.finalize = false;
        self.flushed = false;

        self.address_latch = self.cpu_client.read_address_latch().expect("Failed to get address latch!");

        let mut cycle_vec: Vec<CycleState> = Vec::new();

        let cycle_state = self.update_state();

        cycle_vec.push(cycle_state);

        // ALE should be active at start of execution
        if self.command_status & COMMAND_ALE_BIT == 0 {
            log::warn!("Execution is not starting on T1.");
        }

        // cycle trace if enabled
        if cycle_trace == true {
            println!("{}", self.get_cpu_state_str());
        }

        while self.program_state != ProgramState::ExecuteDone {
            let cycle_state = self.cycle(instr, emu_prefetch, emu_mem_ops, cpu_prefetch, cpu_mem_ops)?;

            if !self.finalize {
                cycle_vec.push(cycle_state);
            }

            // cycle trace if enabled
            if cycle_trace == true {
                //println!("{}", self.get_cpu_state_str());
            }

        }

        Ok(cycle_vec)
    }
    */

    pub fn get_error(&self) -> Option<RemoteCpuError> {
        self.error
    }

    pub fn get_states(&self) -> &Vec<CycleState> {
        &self.cycle_states
    }

    pub fn step(
        &mut self,
        mode: ValidatorMode,
        instr: &[u8],
        instr_addr: u32,
        cycle_trace: bool,
        peek_fetch: u16,
        emu_prefetch: &Vec<BusOp>,
        emu_mem_ops: &Vec<BusOp>,
        cpu_prefetch: &mut Vec<BusOp>,
        cpu_mem_ops: &mut Vec<BusOp>,
        log: &mut TraceLogger,
    ) -> Result<(Vec<CycleState>, bool), ValidatorError> {
        self.error = None;
        self.mode = mode;

        self.instr_addr = instr_addr;
        self.peek_fetch = peek_fetch;
        self.busop_n = 0;
        self.prefetch_n = 0;
        self.queue_first_fetch = false;
        self.rni = false;
        self.v_pc = 0;
        self.cycle_num = 0;
        self.end_instruction = false;
        self.finalize = false;
        self.flushed = false;
        self.discard_front = false;
        self.instruction_ended = false;
        self.cycle_states.clear();

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

        // Discard first fetch if we are rolling over an missed terminating fetch from the previous instruction.
        if self.fetch_rollover && (emu_mem_ops.len() >= 1) && (emu_mem_ops[0].op_type == BusOpType::CodeRead) {
            trace!(log, "Discarding fetch from previous instruction.");
            self.busop_n += 1;
            self.discard_front = true;
            self.fetch_rollover = false;
        }

        // We end an instruction when the QS status lines indicate we have fetched the first byte of the next
        // instruction. But by the time the QS status lines have updated, we are already in the first cycle
        // of that instruction. So we save the last cycle from the previous instruction, and add it to the
        // cycle vector here.

        // We also need to update the queue_first_fetch status if this was a fetch for a non-prefix opcode.
        if let Some(cycle_state) = self.last_cycle_state {
            if cycle_state.q_op == QueueOp::First && !RemoteCpu::is_prefix(cycle_state.q_byte) {
                // This was a fetch for an opcode
                self.queue_first_fetch = true;
            }
            cycle_vec.push(cycle_state);
            self.last_cycle_state = None;
        }

        // cycle trace if enabled
        if cycle_trace == true {
            //println!("{}", self.get_cpu_state_str());
            trace!(log, "{}", self.get_cpu_state_str());
        }

        while !self.end_instruction {
            let mut cycle_state = match self.cycle(instr, emu_prefetch, emu_mem_ops, cpu_prefetch, cpu_mem_ops, log) {
                Ok(cycle_state) => cycle_state,
                Err(e) => {
                    trace!(log, "CPU error during step(): {}", e);
                    RemoteCpu::dump_cycles(&cycle_vec);
                    return Err(e);
                }
            };

            if !self.end_instruction {
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
            if cycle_trace == true {
                //println!("{}", self.get_cpu_state_str());
                let end = self.end_instruction;
                trace!(log, "{}: {}", end, self.get_cpu_state_str());
                //trace!(log, "end instr? {}", self.end_instruction);
            }
        }

        Ok((cycle_vec, self.discard_front))
    }

    pub fn dump_cycles(cycles: &Vec<CycleState>) {
        for cycle in cycles {
            log::warn!("{}", RemoteCpu::get_cycle_state_str(cycle));
        }
    }

    pub fn in_finalize(&mut self) -> bool {
        self.finalize
    }

    pub fn load(&mut self, reg_buf: &[u8]) -> Result<bool, CpuClientError> {
        self.cpu_client.load_registers_from_buf(&reg_buf)?;
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
        ((segment as u32) << 4) + offset as u32 & 0xFFFFFu32
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

        for i in 0..len {
            inner.push_str(&format!("{:02X}", q[i]));
        }
        outer.push_str(&format!("{:8}]", inner));
        outer
    }

    pub fn get_cycle_state_str(c: &CycleState) -> String {
        let ale_str = match c.ale {
            true => "A:",
            false => "  ",
        };

        let mut seg_str = "  ";
        if c.t_state != BusCycle::T1 {
            // Segment status only valid in T2+
            seg_str = match c.a_type {
                AccessType::AlternateData => "ES",
                AccessType::Stack => "SS",
                AccessType::CodeOrNone => "CS",
                AccessType::Data => "DS",
            };
        }

        let q_op_chr = match c.q_op {
            QueueOp::Idle => ' ',
            QueueOp::First => 'F',
            QueueOp::Flush => 'E',
            QueueOp::Subsequent => 'S',
        };

        // All read/write signals are active/low
        let rs_chr = match !c.mrdc {
            true => 'R',
            false => '.',
        };
        let aws_chr = match !c.aiowc {
            true => 'A',
            false => '.',
        };
        let ws_chr = match !c.mwtc {
            true => 'W',
            false => '.',
        };
        let ior_chr = match !c.iorc {
            true => 'R',
            false => '.',
        };
        let aiow_chr = match !c.aiowc {
            true => 'A',
            false => '.',
        };
        let iow_chr = match !c.iowc {
            true => 'W',
            false => '.',
        };

        let bus_str = match c.b_state {
            BusState::INTA => "INTA",
            BusState::IOR => "IOR ",
            BusState::IOW => "IOW ",
            BusState::HALT => "HALT",
            BusState::CODE => "CODE",
            BusState::MEMR => "MEMR",
            BusState::MEMW => "MEMW",
            BusState::PASV => "PASV",
        };

        let t_str = match c.t_state {
            BusCycle::Ti => "Ti",
            BusCycle::T1 => "T1",
            BusCycle::T2 => "T2",
            BusCycle::T3 => "T3",
            BusCycle::T4 => "T4",
            BusCycle::Tw => "Tw",
        };

        let is_reading = !c.mrdc | !c.iorc;
        let is_writing = !c.mwtc | !c.aiowc | !c.iowc;

        let mut xfer_str = "      ".to_string();
        if is_reading {
            xfer_str = format!("<-r {:02X}", c.data_bus);
        }
        else if is_writing {
            xfer_str = format!("w-> {:02X}", c.data_bus);
        }

        let mut q_read_str = String::new();

        if c.q_op == QueueOp::First {
            // First byte of opcode read from queue. Decode it to opcode or group specifier
            q_read_str = format!("<-q {:02X}", c.q_byte);
        }
        else if c.q_op == QueueOp::Subsequent {
            q_read_str = format!("<-q {:02X}", c.q_byte);
        }

        format!(
            //"{:08} {:02}[{:05X}] {:02} M:{}{}{} I:{}{}{} {:04} {:02} {:06} | {:1}{:1} [{:08}] {}",
            "{:08} {:02}[{:05X}] {:02} M:{}{}{} I:{}{}{} {:04} {:02} {:06} | {:1}{:1} {} {:6}",
            c.n,
            ale_str,
            c.addr,
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
            c.q_len,
            //self.queue.to_string(),
            RemoteCpu::get_queue_str(&c.q, c.q_len as usize),
            q_read_str,
        )
    }

    pub fn get_cpu_state_str(&mut self) -> String {
        let ale_str = match self.command_status & COMMAND_ALE_BIT != 0 {
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
            _ => ' ',
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

        let mut xfer_str = "      ".to_string();
        if is_reading {
            xfer_str = format!("<-r {:02X}", self.data_bus);
        }
        else if is_writing {
            xfer_str = format!("w-> {:02X}", self.data_bus);
        }

        // Handle queue activity

        let mut q_read_str = String::new();

        if q_op == QueueOp::First {
            // First byte of opcode read from queue. Decode it to opcode or group specifier
            q_read_str = format!("<-q {:02X} {}", self.queue_byte, self.instr_str);
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

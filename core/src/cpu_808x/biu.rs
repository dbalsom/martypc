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

    cpu_808x::biu.rs

    Implement CPU behavior specific to the BIU (Bus Interface Unit)

*/
use crate::{
    bytequeue::*,
    cpu_808x::*,
    cpu_common::{operands::OperandSize, QueueOp, Segment},
};

pub const QUEUE_SIZE: usize = 4;
pub const QUEUE_POLICY_LEN: usize = 3;

pub enum ReadWriteFlag {
    Normal,
    RNI,
}

impl ByteQueue for Intel808x {
    fn seek(&mut self, _pos: usize) {
        // Instruction queue does not support seeking
    }

    fn tell(&self) -> usize {
        //log::trace!("pc: {:05X} qlen: {}", self.pc, self.queue.len());
        //self.pc as usize - (self.queue.len() + (self.queue.has_preload() as usize))
        self.pc as usize - (self.queue.len_p())
    }

    fn wait(&mut self, cycles: u32) {
        self.cycles(cycles);
    }

    fn wait_i(&mut self, cycles: u32, instr: &[u16]) {
        self.cycles_i(cycles, instr);
    }

    fn wait_comment(&mut self, comment: &'static str) {
        self.trace_comment(comment);
    }

    fn set_pc(&mut self, pc: u16) {
        self.mc_pc = pc;
    }

    fn q_read_u8(&mut self, dtype: QueueType, reader: QueueReader) -> u8 {
        self.biu_queue_read(dtype, reader)
    }

    fn q_read_i8(&mut self, dtype: QueueType, reader: QueueReader) -> i8 {
        self.biu_queue_read(dtype, reader) as i8
    }

    fn q_read_u16(&mut self, dtype: QueueType, reader: QueueReader) -> u16 {
        let lo = self.biu_queue_read(dtype, reader);
        let ho = self.biu_queue_read(QueueType::Subsequent, reader);

        (ho as u16) << 8 | (lo as u16)
    }

    fn q_read_i16(&mut self, dtype: QueueType, reader: QueueReader) -> i16 {
        let lo = self.biu_queue_read(dtype, reader);
        let ho = self.biu_queue_read(QueueType::Subsequent, reader);

        ((ho as u16) << 8 | (lo as u16)) as i16
    }

    fn q_peek_u8(&mut self) -> u8 {
        let (byte, _cost) = self.bus.read_u8(self.flat_ip() as usize, 0).unwrap();
        byte
    }

    fn q_peek_i8(&mut self) -> i8 {
        let (byte, _cost) = self.bus.read_u8(self.flat_ip() as usize, 0).unwrap();
        byte as i8
    }

    fn q_peek_u16(&mut self) -> u16 {
        let (word, _cost) = self.bus.read_u16(self.flat_ip() as usize, 0).unwrap();
        word
    }

    fn q_peek_i16(&mut self) -> i16 {
        let (word, _cost) = self.bus.read_u16(self.flat_ip() as usize, 0).unwrap();
        word as i16
    }

    fn q_peek_farptr16(&mut self) -> (u16, u16) {
        let read_offset = self.flat_ip() as usize;

        let (offset, _cost) = self.bus.read_u16(read_offset, 0).unwrap();
        let (segment, _cost) = self.bus.read_u16(read_offset + 2, 0).unwrap();
        (segment, offset)
    }
}

impl Intel808x {
    /// Read a byte from the instruction queue.
    /// Either return a byte currently in the queue, or fetch a byte into the queue and
    /// then return it.
    ///
    /// Regardless of 8088 or 8086, the queue is read from one byte at a time.
    ///
    /// QueueType is used to set the QS status lines for first/subsequent byte fetches.
    /// QueueReader is used to advance the microcode instruction if the queue read is
    /// from the EU executing an instruction. The BIU reading the queue to fetch an
    /// instruction will not advance the microcode PC.
    pub fn biu_queue_read(&mut self, dtype: QueueType, reader: QueueReader) -> u8 {
        let byte;
        //trace_print!(self, "biu_queue_read()");

        if let Some(preload_byte) = self.queue.get_preload() {
            // We have a preloaded byte from finalizing the last instruction.
            self.last_queue_op = QueueOp::First;
            self.last_queue_byte = preload_byte;

            // Since we have a preloaded fetch, the next instruction will always begin
            // execution on the next cycle. If NX bit is set, advance the MC PC to
            // execute the RNI from the previous instruction.
            self.next_mc();
            if self.nx {
                self.nx = false;
            }

            self.biu_fetch_on_queue_read();
            return preload_byte;
        }

        if self.queue.len() > 0 {
            // The queue has an available byte. Return it.

            //self.trace_print("biu_queue_read: pop()");
            //self.trace_comment("Q_READ");
            byte = self.queue.pop();
            self.biu_fetch_on_queue_read();
        }
        else {
            // Queue is empty, wait for a byte to be fetched into the queue then return it.
            // Fetching is automatic, therefore, just cycle the cpu until a byte appears...
            while self.queue.len() == 0 {
                self.cycle();
            }

            // ...and pop it out.
            byte = self.queue.pop();
        }

        self.queue_byte = byte;

        let mut advance_pc = false;

        // TODO: These enums duplicate functionality
        self.queue_op = match dtype {
            QueueType::First => QueueOp::First,
            QueueType::Subsequent => {
                match reader {
                    QueueReader::Biu => QueueOp::Subsequent,
                    QueueReader::Eu => {
                        // Advance the microcode PC.
                        advance_pc = true;
                        QueueOp::Subsequent
                    }
                }
            }
        };

        self.cycle();
        if advance_pc {
            if self.nx {
                self.nx = false;
            }
            self.mc_pc += 1;
        }
        byte
    }

    #[inline]
    pub fn biu_fetch_on_queue_read(&mut self) {
        if self.queue.at_policy_len() {
            match self.fetch_state {
                FetchState::Suspended => {
                    self.trace_comment("FETCH_ON_SUSP_READ");
                    self.ta_cycle = TaCycle::Td;
                }
                FetchState::PausedFull => {
                    self.trace_comment("FETCH_ON_READ");
                    //self.ta_cycle = TaCycle::Ta;
                    self.ta_cycle = TaCycle::Td;
                }
                _ => {}
            }

            self.biu_fetch_start();
        }
    }

    /// This function will cycle the CPU until a byte is available in the instruction queue,
    /// then read it out, to prepare for execution of the next instruction.
    ///
    /// We consider this byte 'preloaded' - this does not correspond to a real CPU state
    pub fn biu_fetch_next(&mut self) {
        // Don't fetch if we are in a string instruction that is still repeating.
        if !self.in_rep {
            self.trace_comment("FETCH");
            let mut fetch_timeout = 0;

            /*
            if MICROCODE_FLAGS_8088[self.mc_pc as usize] == RNI {
                trace_print!(self, "Executed terminating RNI!");
            }
            */

            if self.queue.len() == 0 {
                while {
                    if self.nx {
                        self.trace_comment("NX");
                        self.next_mc();
                        self.nx = false;
                        self.rni = false;
                    }
                    self.cycle();
                    self.mc_pc = MC_NONE;
                    fetch_timeout += 1;
                    if fetch_timeout == 20 {
                        self.trace_flush();
                        panic!("FETCH timeout! wait states: {}", self.wait_states);
                    }
                    self.queue.len() == 0
                } {}
                // Should be a byte in the queue now. Preload it
                self.queue.set_preload();
                self.queue_op = QueueOp::First;

                // Check if reading the queue will initiate a new fetch.
                self.biu_fetch_on_queue_read();

                self.trace_comment("FETCH_END");
                self.cycle();
            }
            else {
                self.queue.set_preload();
                self.queue_op = QueueOp::First;

                // Check if reading the queue will initiate a new fetch.
                self.biu_fetch_on_queue_read();

                if self.nx {
                    self.trace_comment("NX");
                    self.next_mc();
                }

                if self.rni {
                    self.trace_comment("RNI");
                    self.rni = false;
                }

                self.trace_comment("FETCH_END");
                self.cycle();
            }
        }
    }

    /// Implements the SUSP microcode routine to suspend prefetching. SUSP does not
    /// return until the end of any current bus cycle.
    pub fn biu_fetch_suspend(&mut self) {
        self.trace_comment("SUSP");
        self.fetch_state = FetchState::Suspended;

        // SUSP waits for any current fetch to complete.
        if self.bus_status_latch == BusStatus::CodeFetch {
            self.biu_bus_wait_finish();
        }

        // Reset pipeline status or we will hang
        self.ta_cycle = TaCycle::Td;
        self.pl_status = BusStatus::Passive;
    }

    /// Simulate the logic that disables prefetching in the halt state. This is not exactly the same
    /// logic as used to suspend prefetching via SUSP.
    pub fn biu_fetch_halt(&mut self) {
        self.trace_comment("HALT_FETCH");

        match self.t_cycle {
            TCycle::T1 | TCycle::T2 => {
                // We have time to prevent a prefetch decision.
                self.fetch_state = FetchState::Halted;
            }
            _ => {
                // We halted too late - a prefetch will be attempted.
            }
        }
    }

    /// Implement the FLUSH microcode subroutine to flush the instruction queue. This will trigger
    /// a new fetch cycle immediately to refill the queue.
    pub fn biu_queue_flush(&mut self) {
        self.queue.flush();
        self.queue_op = QueueOp::Flush;
        self.trace_comment("FLUSH");
        self.fetch_state = FetchState::Normal;

        // Start a new prefetch address cycle
        self.biu_fetch_start();
    }

    pub fn biu_queue_has_room(&mut self) -> bool {
        match self.cpu_subtype {
            CpuSubType::Intel8088 | CpuSubType::Harris80C88 => self.queue.len() < QUEUE_SIZE,
            CpuSubType::Intel8086 => {
                // 8086 fetches two bytes at a time, so must be two free bytes in queue
                self.queue.len() < QUEUE_SIZE - 1
            }
            _ => {
                panic!("Unsupported CPU subtype")
            }
        }
    }

    /// Decide whether to start a code fetch this cycle. Should be called at Ti and end of T2.
    pub fn biu_make_fetch_decision(&mut self) {
        if matches!(self.fetch_state, FetchState::Delayed(0)) && self.biu_queue_has_room() {
            // If fetch_state is Delayed, we can assume this is being called on Ti.
            self.trace_comment("FETCH_RESUME");
            self.biu_fetch_start();
            //self.fetch_state = FetchState::Normal;
            return;
        }
        else if self.bus_status == BusStatus::CodeFetch
            && self.queue.at_policy_len()
            && self.bus_pending != BusPendingType::EuEarly
        {
            self.fetch_state = FetchState::Delayed(3);
        }

        if !self.biu_queue_has_room() {
            // Queue is full, suspend fetching.
            self.fetch_state = FetchState::PausedFull;
        }
        else if self.bus_pending != BusPendingType::EuEarly
            && self.fetch_state != FetchState::Suspended
            && !(self.queue.at_policy_len() && self.bus_status_latch == BusStatus::CodeFetch)
        {
            // EU has not claimed the bus this m-cycle, and we are not at a queue policy length
            // during a code fetch, and fetching is not suspended. Begin a code fetch address cycle.
            self.biu_fetch_start();
        }
    }

    /// Issue a HALT.  HALT is a unique bus status code, but not a real bus state. It is hacked
    /// in by miscellaneous logic for one cycle.
    pub fn biu_halt(&mut self) {
        self.fetch_state = FetchState::Halted;
        self.biu_bus_wait_finish();
        if let TCycle::T4 = self.t_cycle {
            self.cycle();
        }
        self.t_cycle = TCycle::Ti;
        self.cycle();

        self.bus_status = BusStatus::Halt;
        self.bus_status_latch = BusStatus::Halt;
        self.bus_segment = Segment::CS;
        self.t_cycle = TCycle::T1;
        self.i8288.ale = true;
        self.data_bus = 0;
        self.transfer_size = self.fetch_size;
        self.operand_size = OperandSize::Operand8;
        self.transfer_n = 1;
        self.final_transfer = true;

        self.cycle();
    }

    /// Issue an interrupt acknowledge, consisting of two consecutive INTA bus cycles.
    pub fn biu_inta(&mut self, vector: u8) {
        self.biu_bus_begin(
            BusStatus::InterruptAck,
            Segment::None,
            0,
            0,
            TransferSize::Byte,
            OperandSize::Operand16,
            true,
        );

        self.biu_bus_wait_finish();

        self.biu_bus_begin(
            BusStatus::InterruptAck,
            Segment::None,
            0,
            vector as u16,
            TransferSize::Byte,
            OperandSize::Operand16,
            false,
        );

        self.biu_bus_wait_finish();
    }

    pub fn biu_read_u8(&mut self, seg: Segment, offset: u16, flag: ReadWriteFlag) -> u8 {
        let addr = self.calc_linear_address_seg(seg, offset);

        self.biu_bus_begin(
            BusStatus::MemRead,
            seg,
            addr,
            0,
            TransferSize::Byte,
            OperandSize::Operand8,
            true,
        );
        match flag {
            ReadWriteFlag::Normal => self.biu_bus_wait_finish(),
            ReadWriteFlag::RNI => self.biu_bus_wait_until_tx(),
        };

        (self.data_bus & 0x00FF) as u8
    }

    pub fn biu_write_u8(&mut self, seg: Segment, offset: u16, byte: u8, flag: ReadWriteFlag) {
        let addr = self.calc_linear_address_seg(seg, offset);

        self.biu_bus_begin(
            BusStatus::MemWrite,
            seg,
            addr,
            byte as u16,
            TransferSize::Byte,
            OperandSize::Operand8,
            true,
        );
        match flag {
            ReadWriteFlag::Normal => self.biu_bus_wait_finish(),
            ReadWriteFlag::RNI => self.biu_bus_wait_until_tx(),
        };
    }

    pub fn biu_io_read_u8(&mut self, addr: u16) -> u8 {
        self.biu_bus_begin(
            BusStatus::IoRead,
            Segment::None,
            addr as u32,
            0,
            TransferSize::Byte,
            OperandSize::Operand8,
            true,
        );
        self.biu_bus_wait_finish();
        (self.data_bus & 0x00FF) as u8
    }

    pub fn biu_io_write_u8(&mut self, addr: u16, byte: u8, flag: ReadWriteFlag) {
        self.biu_bus_begin(
            BusStatus::IoWrite,
            Segment::None,
            addr as u32,
            byte as u16,
            TransferSize::Byte,
            OperandSize::Operand8,
            true,
        );
        match flag {
            ReadWriteFlag::Normal => self.biu_bus_wait_finish(),
            ReadWriteFlag::RNI => self.biu_bus_wait_until_tx(),
        };

        //validate_write_u8!(self, addr, (self.data_bus & 0x00FF) as u8);
    }

    pub fn biu_io_read_u16(&mut self, addr: u16, flag: ReadWriteFlag) -> u16 {
        let mut word;

        self.biu_bus_begin(
            BusStatus::IoRead,
            Segment::None,
            addr as u32,
            0,
            TransferSize::Byte,
            OperandSize::Operand16,
            true,
        );
        self.biu_bus_wait_finish();

        word = self.data_bus & 0x00FF;

        self.biu_bus_begin(
            BusStatus::IoRead,
            Segment::None,
            addr.wrapping_add(1) as u32,
            0,
            TransferSize::Byte,
            OperandSize::Operand16,
            false,
        );

        match flag {
            ReadWriteFlag::Normal => self.biu_bus_wait_finish(),
            ReadWriteFlag::RNI => self.biu_bus_wait_until_tx(),
        };

        word |= (self.data_bus & 0x00FF) << 8;

        word
    }

    pub fn biu_io_write_u16(&mut self, addr: u16, word: u16, flag: ReadWriteFlag) {
        self.biu_bus_begin(
            BusStatus::IoWrite,
            Segment::None,
            addr as u32,
            word & 0x00FF,
            TransferSize::Byte,
            OperandSize::Operand16,
            true,
        );

        self.biu_bus_wait_finish();

        self.biu_bus_begin(
            BusStatus::IoWrite,
            Segment::None,
            addr.wrapping_add(1) as u32,
            (word >> 8) & 0x00FF,
            TransferSize::Byte,
            OperandSize::Operand16,
            false,
        );

        match flag {
            ReadWriteFlag::Normal => self.biu_bus_wait_finish(),
            ReadWriteFlag::RNI => self.biu_bus_wait_until_tx(),
        };
    }

    /// Request a word size (16-bit) bus read transfer from the BIU.
    /// The 8088 divides word transfers up into two consecutive byte size transfers.
    pub fn biu_read_u16(&mut self, seg: Segment, offset: u16, flag: ReadWriteFlag) -> u16 {
        let mut word;
        let mut addr = self.calc_linear_address_seg(seg, offset);

        self.biu_bus_begin(
            BusStatus::MemRead,
            seg,
            addr,
            0,
            TransferSize::Byte,
            OperandSize::Operand16,
            true,
        );

        self.biu_bus_wait_finish();
        word = self.data_bus & 0x00FF;
        addr = self.calc_linear_address_seg(seg, offset.wrapping_add(1));

        self.biu_bus_begin(
            BusStatus::MemRead,
            seg,
            addr,
            0,
            TransferSize::Byte,
            OperandSize::Operand16,
            false,
        );
        match flag {
            ReadWriteFlag::Normal => self.biu_bus_wait_finish(),
            ReadWriteFlag::RNI => {
                // self.bus_wait_until(TCycle::T3)
                self.biu_bus_wait_finish()
            }
        };
        word |= (self.data_bus & 0x00FF) << 8;

        word
    }

    /// Request a word size (16-bit) bus write transfer from the BIU.
    /// The 8088 divides word transfers up into two consecutive byte size transfers.
    pub fn biu_write_u16(&mut self, seg: Segment, offset: u16, word: u16, flag: ReadWriteFlag) {
        let mut addr = self.calc_linear_address_seg(seg, offset);

        // 8088 performs two consecutive byte transfers
        self.biu_bus_begin(
            BusStatus::MemWrite,
            seg,
            addr,
            word & 0x00FF,
            TransferSize::Byte,
            OperandSize::Operand16,
            true,
        );

        self.biu_bus_wait_finish();
        addr = self.calc_linear_address_seg(seg, offset.wrapping_add(1));

        self.biu_bus_begin(
            BusStatus::MemWrite,
            seg,
            addr,
            (word >> 8) & 0x00FF,
            TransferSize::Byte,
            OperandSize::Operand16,
            false,
        );

        match flag {
            ReadWriteFlag::Normal => self.biu_bus_wait_finish(),
            ReadWriteFlag::RNI => self.biu_bus_wait_until_tx(),
        };
    }

    /// If in an active bus cycle, cycle the cpu until the bus cycle has reached T4.
    #[inline]
    pub fn biu_bus_wait_finish(&mut self) -> u32 {
        let mut elapsed = 0;

        if let BusStatus::Passive = self.bus_status_latch {
            // No bus cycle in progress
            0
        }
        else {
            while self.t_cycle != TCycle::T4 {
                self.cycle();
                elapsed += 1;
            }
            elapsed
        }
    }

    /// If in a fetch delay, cycle the CPU until we are not (and set abort ta_cycle)
    #[inline]
    pub fn biu_bus_wait_delay(&mut self) -> bool {
        let mut was_delay = false;
        //self.trace_print(&format!("biu_bus_wait_delay(): fetch_state: {:?}", self.fetch_state));
        match self.fetch_state {
            FetchState::Delayed(0) => {
                // If delay has expired, we can begin a new bus cycle from Tr, so set state to Td (Done)
                was_delay = true;
                self.ta_cycle = TaCycle::Td;
            }
            FetchState::Delayed(delay) => {
                // If we have a delay in progress, we can abort, so set the state to Ta (Abort)
                self.cycles(delay as u32);
                was_delay = true;
                self.ta_cycle = TaCycle::Ta;
            }
            _ => {}
        }
        was_delay
    }

    /// If an address cycle is in progress, cycle the cpu until the address cycle has completed
    /// or has aborted.
    #[inline]
    pub fn biu_bus_wait_address(&mut self) -> u32 {
        let mut elapsed = 0;
        while !matches!(self.ta_cycle, TaCycle::Td | TaCycle::Ta) {
            self.cycle();
            elapsed += 1;
        }
        elapsed
    }

    /// If in an active bus cycle, cycle the cpu until the bus cycle has reached at least T2.
    pub fn biu_bus_wait_halt(&mut self) -> u32 {
        if matches!(self.bus_status_latch, BusStatus::Passive) && self.t_cycle == TCycle::T1 {
            self.cycle();
            return 1;
        }
        0
    }

    /// If in an active bus cycle, cycle the CPU until the target T-state is reached.
    ///
    /// This function is usually used on a terminal write to wait for T3-TwLast to
    /// handle RNI in microcode. The next instruction byte will be fetched on this
    /// terminating cycle and the beginning of execution will overlap with T4.
    pub fn biu_bus_wait_until_tx(&mut self) -> u32 {
        let mut bus_cycles_elapsed = 0;
        return match self.bus_status_latch {
            BusStatus::MemRead
            | BusStatus::MemWrite
            | BusStatus::IoRead
            | BusStatus::IoWrite
            | BusStatus::CodeFetch => {
                self.trace_comment("WAIT_TX");
                while !self.is_last_wait() {
                    self.cycle();
                    bus_cycles_elapsed += 1;
                }
                self.trace_comment("TX");
                /*
                if target_state == TCycle::Tw {
                    // Interpret waiting for Tw as waiting for T3 or Last Tw
                    loop {
                        match (self.t_cycle, effective_wait_states) {
                            (TCycle::T3, 0) => {
                                self.trace_comment(" >> wait match!");
                                if self.bus_wait_states == 0 {
                                    self.trace_comment(">> no bus_wait_states");
                                    return bus_cycles_elapsed
                                }
                                else {
                                    self.trace_comment(">> wait state!");
                                    self.cycle();
                                }
                            }
                            (TCycle::T3, n) | (TCycle::Tw, n) => {
                                log::trace!("waits: {}", n);
                                for _ in 0..n {
                                    self.cycle();
                                    bus_cycles_elapsed += 1;
                                }
                                return bus_cycles_elapsed
                            }
                            _ => {
                                self.cycle();
                                bus_cycles_elapsed += 1;
                            }
                        }
                    }
                }
                else {
                    while self.t_cycle != target_state {
                        self.cycle();
                        bus_cycles_elapsed += 1;
                    }
                }
                */

                bus_cycles_elapsed
            }
            _ => 0,
        };
    }

    /// Begins a new bus cycle of the specified type.
    ///
    /// This is a complex operation; we must wait for the current bus transfer, if any, to complete.
    /// We must process any delays and penalties as appropriate. For example, if we initiate a
    /// request for a bus cycle when a prefetch is scheduled, we will incur a 2 cycle abort penalty.
    ///
    /// Note: this function is for EU bus requests only. It cannot start a CODE fetch.
    pub fn biu_bus_begin(
        &mut self,
        new_bus_status: BusStatus,
        bus_segment: Segment,
        address: u32,
        data: u16,
        size: TransferSize,
        op_size: OperandSize,
        first: bool,
    ) {
        self.trace_comment("BUS_BEGIN");

        assert_ne!(
            new_bus_status,
            BusStatus::CodeFetch,
            "Cannot start a CODE fetch with biu_bus_begin()"
        );

        // Check this address for a memory access breakpoint
        if self.bus.get_flags(address as usize) & MEM_BPA_BIT != 0 {
            // Breakpoint hit
            self.state = CpuState::BreakpointHit;
        }

        let mut fetch_abort = false;

        match self.t_cycle {
            TCycle::Tinit => {
                panic!("Can't start a bus cycle on Tinit")
            }
            TCycle::Ti => {
                // Bus is idle, enter Tr state immediately.
                self.biu_address_start(new_bus_status);
            }
            TCycle::T1 | TCycle::T2 => {
                // We can enter Tr state immediately since we are requesting the bus before a
                // fetch decision.
                self.bus_pending = BusPendingType::EuEarly;
                assert_eq!(self.ta_cycle, TaCycle::Td);
                self.biu_address_start(new_bus_status);
            }
            _ => {
                if matches!(self.pl_status, BusStatus::CodeFetch) {
                    self.bus_pending = BusPendingType::EuLate;
                    fetch_abort = true;
                }
                else if !self.final_transfer {
                    self.bus_pending = BusPendingType::EuEarly;
                }
            }
        }

        // Wait for any current bus cycle to terminate. If no bus cycle is in progress, this is a
        // no-op.
        let _ = self.biu_bus_wait_finish();

        // Wait for any fetch delay to complete.
        let was_delay = self.biu_bus_wait_delay();

        // Wait for address cycle to complete.
        self.trace_comment("WAIT_ADDRESS");
        self.biu_bus_wait_address();
        self.trace_comment("WAIT_ADDRESS_DONE");

        // If we aborted a fetch, begin the address cycle for the new bus cycle now and wait for it
        // to complete.
        if was_delay || fetch_abort {
            self.biu_address_start(new_bus_status);
            self.biu_bus_wait_address();
        }

        // If there was an active bus cycle, we're now on T4 - tick over to T1 to get
        // ready to start the new bus cycle.
        if self.t_cycle == TCycle::T4 && self.bus_status_latch != BusStatus::CodeFetch {
            // We should be in a T0 or Td address state now; with any prefetch aborted.
            //assert_eq!(self.ta_cycle, TaCycle::Td);

            self.cycle();
        }

        // Set the final_transfer flag if this is the last bus cycle of an atomic bus transfer
        // (word read/writes cannot be interrupted by prefetches)
        if let TransferSize::Word = size {
            self.transfer_n = 1;
            self.final_transfer = true;
        }
        else if first {
            self.final_transfer = match op_size {
                OperandSize::Operand8 => {
                    self.transfer_n = 1;
                    true
                }
                OperandSize::Operand16 => {
                    self.transfer_n = 1;
                    false
                }
                _ => panic!("invalid OperandSize"),
            }
        }
        else {
            // first == false is only possible if doing word transfer on 8088
            self.transfer_n = 2;
            self.final_transfer = true;
        }

        // Finally, begin the new bus state.
        self.bus_pending = BusPendingType::None;
        self.pl_status = BusStatus::Passive; // Pipeline status must always be reset on T1
        self.bus_pending = BusPendingType::None; // Can't be pending anymore as we're starting the EU cycle
        self.bus_status = new_bus_status;
        self.bus_status_latch = new_bus_status;
        self.bus_segment = bus_segment;
        self.t_cycle = TCycle::Tinit;
        self.address_bus = address;
        self.address_latch = address;
        self.i8288.ale = true;
        self.data_bus = data;
        self.transfer_size = size;
        self.operand_size = op_size;
    }

    pub fn biu_bus_end(&mut self) {
        // Reset i8288 signals
        self.i8288.mrdc = false;
        self.i8288.amwc = false;
        self.i8288.mwtc = false;
        self.i8288.iorc = false;
        self.i8288.aiowc = false;
        self.i8288.iowc = false;

        //self.bus_pending = BusPendingType::None;
    }

    #[inline]
    pub fn biu_fetch_start(&mut self) {
        // Only start a fetch if the EU hasn't claimed the bus, and we're not already in a fetch
        // address cycle.
        if self.bus_pending != BusPendingType::EuEarly && self.pl_status != BusStatus::CodeFetch {
            self.fetch_state = FetchState::Normal;
            self.biu_address_start(BusStatus::CodeFetch);
        }
    }

    /// Start a new address cycle. This will set the Ta cycle to Tr and switch the pipeline slots.
    /// ta_cycle should never be set to Tr outside of this function.
    #[inline]
    pub fn biu_address_start(&mut self, new_bus_status: BusStatus) {
        // Switch pipeline slots and start a new address cycle.
        if self.t_cycle != TCycle::Ti {
            // If the bus is not idle, we usually want to change pipeline slots on a new address
            // cycle, except when we abort a fetch. It is clearer to me to remain in the same
            // slot on an abort.
            if self.ta_cycle != TaCycle::Ta {
                self.pl_slot = !self.pl_slot;
            }
        }
        else {
            self.pl_slot = false;
        }

        // Skip Tr if we aborted. (T0 of abort was our Tr)
        if self.ta_cycle == TaCycle::Ta {
            self.trace_comment("ADDRESS_START_ABT");
            self.ta_cycle = TaCycle::Ts;
        }
        else {
            // Did not abort, start new address cycle at Tr
            self.trace_comment("ADDRESS_START");
            self.ta_cycle = TaCycle::Tr;
        }
        self.pl_status = new_bus_status;
    }

    /// Perform a prefetch abort. This should be called on T4 of a code fetch when a late eu bus
    /// request is present. Whether this is a T0 or a Tr state is academic, the next cycle will be
    /// a Ts state regardless.
    #[inline]
    pub fn biu_fetch_abort(&mut self) {
        self.trace_comment("ABORT");
        self.ta_cycle = TaCycle::Ta;
    }

    pub fn biu_fetch_bus_begin(&mut self) {
        let addr = Intel808x::calc_linear_address(self.cs, self.pc);
        if self.biu_queue_has_room() {
            //trace_print!(self, "Setting address bus to PC: {:05X}", self.pc);
            self.fetch_state = FetchState::Normal;
            self.pl_status = BusStatus::Passive; // Pipeline status must always be reset on T1
            self.bus_status = BusStatus::CodeFetch;
            self.bus_status_latch = BusStatus::CodeFetch;
            self.bus_segment = Segment::CS;
            self.t_cycle = TCycle::Tinit;
            self.address_bus = addr;
            self.address_latch = addr;
            self.i8288.ale = true;
            self.data_bus = 0;
            self.transfer_size = self.fetch_size;
            self.operand_size = match self.fetch_size {
                TransferSize::Byte => OperandSize::Operand8,
                TransferSize::Word => OperandSize::Operand16,
            };
            self.transfer_n = 1;
            self.final_transfer = true;
        }
        //else if !self.bus_pending_eu {
        // Cancel fetch if queue is full and no pending bus request from EU that
        // would otherwise trigger an abort.
        //self.biu_abort_fetch_full();
        //}
    }
}

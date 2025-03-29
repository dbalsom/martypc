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

    cpu_808x::biu.rs

    Implement CPU behavior specific to the BIU (Bus Interface Unit)

*/
use crate::{
    bytequeue::*,
    cpu_808x::*,
    cpu_common::{operands::OperandSize, QueueOp, Segment},
};

#[cfg(feature = "cpu_validator")]
use crate::cpu_validator::{BusType, ReadType};

pub const QUEUE_SIZE: usize = 4;
pub const QUEUE_POLICY_LEN: usize = 3;

#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub enum BusWidth {
    #[default]
    Byte,
    Word,
}

impl BusWidth {
    pub fn fmt_width(&self) -> usize {
        match self {
            BusWidth::Byte => 2,
            BusWidth::Word => 4,
        }
    }
}

macro_rules! validate_read_u8 {
    ($myself: expr, $addr: expr, $data: expr, $btype: expr, $rtype: expr) => {{
        #[cfg(feature = "cpu_validator")]
        if let Some(ref mut validator) = &mut $myself.validator {
            validator.emu_read_byte($addr, $data, $btype, $rtype)
        }
    }};
}

macro_rules! validate_write_u8 {
    ($myself: expr, $addr: expr, $data: expr, $btype: expr) => {{
        #[cfg(feature = "cpu_validator")]
        if let Some(ref mut validator) = &mut $myself.validator {
            validator.emu_write_byte($addr, $data, $btype)
        }
    }};
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

        // If we read from the queue while prefetching is delayed, we can abort the delay state,
        // if reading the queue would take us below the queue policy length.
        if matches!(self.fetch_state, FetchState::Delayed(_)) && self.queue.at_policy_threshold() {
            self.trace_comment("CANCEL_DELAY");
            self.fetch_state = FetchState::Delayed(0);
        }

        if let Some(preload_byte) = self.queue.get_preload() {
            // We have a preloaded byte from finalizing the last instruction.
            self.last_queue_op = QueueOp::First;
            self.last_queue_byte = preload_byte;

            // Since we have a preloaded fetch, the next instruction will always begin
            // execution on the next cycle. If NX bit is set, advance the MC PC to
            // execute the RNI from the previous instruction.
            self.next_mc();
            self.nx = false;

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
            self.nx = false;
            self.mc_pc += 1;
        }
        byte
    }

    #[inline]
    pub fn biu_fetch_on_queue_read(&mut self) {
        self.trace_comment("FOQR");
        if self.bus_status == BusStatus::Passive && self.queue.has_room_for_fetch() {
            match self.fetch_state {
                FetchState::Suspended => {
                    self.trace_comment("FETCH_ON_SUSP_READ");
                    self.ta_cycle = TaCycle::Td;
                }
                FetchState::PausedFull => {
                    if self.t_cycle == TCycle::Ti {
                        self.trace_comment("FETCH_ON_FULL_READ");
                        //self.ta_cycle = TaCycle::Ta;
                        self.ta_cycle = TaCycle::Td;
                    }
                    else {
                        // If we are in an active bus cycle, fetch will resume at T4 fetch
                        // decision, so we don't need to do anything here.
                        return;
                    }
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
            self.trace_comment("FETCH_NEXT");
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
                        panic!(
                            "FETCH timeout! wait states: {} fetch state: {:?} t_cycle: {:?} ta_cycle: {:?}",
                            self.bus_wait_states, self.fetch_state, self.t_cycle, self.ta_cycle
                        );
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

        // TODO: Attempt to fix this logic.  As written it would cause prefetching to continue
        //       after halt until the queue was full, which is clearly not correct.
        self.fetch_state = FetchState::Halted;

        /*        match self.t_cycle {
            TCycle::T1 | TCycle::T2 => {
                // We have time to prevent a prefetch decision.
                self.fetch_state = FetchState::Halted;
            }
            _ => {
                // We halted too late - a prefetch will be attempted.
            }
        }*/
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

    /// Decide whether to start a code fetch this cycle. Should be called at Ti and end of T2.
    pub fn biu_make_fetch_decision(&mut self) {
        self.trace_comment("DECIDE");
        if !self.queue.has_room_for_fetch() {
            // Queue is full, suspend fetching.
            self.trace_comment("PAUSE_FULL");
            self.fetch_state = FetchState::PausedFull;
        }
        else if self.bus_pending != BusPendingType::EuEarly {
            // If the EU has not claimed the bus...
            if !matches!(self.fetch_state, FetchState::Suspended | FetchState::Halted) {
                self.trace_comment("FETCH_CHECK");
                // And prefetching isn't suspended or halted...
                if self.queue.at_policy_len() && self.bus_status_latch == BusStatus::CodeFetch {
                    // If we are at a queue policy length during a code fetch. Delay the fetch.
                    if self.ta_cycle == TaCycle::Td {
                        if let FetchState::Delayed(count) = self.fetch_state {
                            if count == 0 {
                                self.fetch_state = FetchState::Delayed(3);
                            }
                        }
                        else {
                            self.fetch_state = FetchState::Delayed(3);
                        }
                    }
                }
                else if self.ta_cycle == TaCycle::Td {
                    // The EU has not claimed the bus this m-cycle, and we are not at a queue policy length
                    // during a code fetch, and fetching is not suspended. Begin a code fetch address cycle.
                    self.trace_comment("FETCH_NORMAL");
                    self.biu_fetch_start();
                }
            }
        }
    }

    /// Make a fetch decision on T4. Normally, a fetch decision is made at the end of T2, but if
    /// fetching was delayed by policy, it will resume at the end of T4 if the specified conditions
    /// apply.
    #[inline]
    pub fn biu_make_fetch_decision_t4(&mut self) {
        if self.ta_cycle == TaCycle::Td && self.biu_is_last_transfer() && self.queue.has_room_for_fetch() {
            if self.fetch_state == FetchState::PausedFull {
                // The queue was full, but now has room.
                // We set the 'abort' ta_cycle to skip Tr, but this is not actually an abort.
                // I just don't feel like creating another flag for this.
                self.ta_cycle = TaCycle::Ta;
            }
            self.trace_comment("T4_FETCH_RESUME");
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

    pub fn biu_read_u8(&mut self, seg: Segment, offset: u16) -> u8 {
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
        self.biu_bus_wait_finish();

        (self.data_bus & 0x00FF) as u8
    }

    pub fn biu_write_u8(&mut self, seg: Segment, offset: u16, byte: u8) {
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
        self.biu_bus_wait_until_tx()
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

    pub fn biu_io_write_u8(&mut self, addr: u16, byte: u8) {
        self.biu_bus_begin(
            BusStatus::IoWrite,
            Segment::None,
            addr as u32,
            byte as u16,
            TransferSize::Byte,
            OperandSize::Operand8,
            true,
        );
        self.biu_bus_wait_until_tx()
    }

    pub fn biu_io_read_u16(&mut self, addr: u16) -> u16 {
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
        self.biu_bus_wait_finish();

        word |= (self.data_bus & 0x00FF) << 8;
        word
    }

    pub fn biu_io_write_u16(&mut self, addr: u16, word: u16) {
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

        self.biu_bus_wait_until_tx()
    }

    #[inline(always)]
    pub fn biu_read_u16(&mut self, seg: Segment, offset: u16) -> u16 {
        match self.bus_width {
            BusWidth::Byte => self.biu_read_u16_native8(seg, offset),
            BusWidth::Word => self.biu_read_u16_native16(seg, offset),
        }
    }

    /// Request a word size (16-bit) bus read transfer from the BIU.
    /// The 8088 divides word transfers up into two consecutive byte size transfers.
    pub fn biu_read_u16_native8(&mut self, seg: Segment, offset: u16) -> u16 {
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
        self.biu_bus_wait_finish();

        word |= (self.data_bus & 0x00FF) << 8;
        word
    }

    pub fn biu_read_u16_native16(&mut self, seg: Segment, offset: u16) -> u16 {
        if offset & 1 != 0 {
            // Unaligned word read, fall back to 8-bit path.
            return self.biu_read_u16_native8(seg, offset);
        }

        let addr = self.calc_linear_address_seg(seg, offset);

        self.biu_bus_begin(
            BusStatus::MemRead,
            seg,
            addr,
            0,
            TransferSize::Word,
            OperandSize::Operand16,
            true,
        );
        self.biu_bus_wait_finish();

        self.data_bus
    }

    #[inline(always)]
    pub fn biu_write_u16(&mut self, seg: Segment, offset: u16, word: u16) {
        match self.bus_width {
            BusWidth::Byte => self.biu_write_u16_native8(seg, offset, word),
            BusWidth::Word => self.biu_write_u16_native16(seg, offset, word),
        }
    }

    /// Request a word size (16-bit) bus write transfer from the BIU.
    /// The 8088 divides word transfers up into two consecutive byte size transfers.
    pub fn biu_write_u16_native8(&mut self, seg: Segment, offset: u16, word: u16) {
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

        self.biu_bus_wait_until_tx()
    }

    /// Request a word size (16-bit) bus write transfer from the BIU.
    /// The 8086 can complete a word transfer in a single bus cycle.
    pub fn biu_write_u16_native16(&mut self, seg: Segment, offset: u16, word: u16) {
        let addr = self.calc_linear_address_seg(seg, offset);
        if offset & 1 != 0 {
            // Unaligned word write, fall back to 8-bit path.
            return self.biu_write_u16_native8(seg, offset, word);
        }
        self.biu_bus_begin(
            BusStatus::MemWrite,
            seg,
            addr,
            word,
            TransferSize::Word,
            OperandSize::Operand16,
            true,
        );
        self.biu_bus_wait_until_tx()
    }

    /// If in an active bus cycle, cycle the cpu until the bus cycle has reached T4.
    #[inline]
    pub fn biu_bus_wait_finish(&mut self) {
        match self.bus_status_latch {
            BusStatus::Passive => {}
            _ => {
                while !matches!(self.t_cycle, TCycle::T4) {
                    self.cycle();
                }
            }
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

    #[inline]
    pub fn biu_bus_wait_t0(&mut self) {
        while !matches!(self.ta_cycle, TaCycle::T0) {
            self.cycle();
        }
    }

    /// If an address cycle is in progress, cycle the cpu until the address cycle has completed
    /// or has aborted.
    #[inline]
    pub fn biu_bus_wait_address(&mut self) {
        while !matches!(self.ta_cycle, TaCycle::Td | TaCycle::Ta) {
            self.cycle();
        }
    }

    /// If in an active bus cycle, cycle the cpu until the bus cycle has reached at least T2.
    #[inline]
    pub fn biu_bus_wait_halt(&mut self) {
        if matches!(self.bus_status_latch, BusStatus::Passive) && self.t_cycle == TCycle::T1 {
            self.cycle();
        }
    }

    /// If in an active bus cycle, cycle the CPU until the target T-state is reached.
    ///
    /// This function is usually used on a terminal write to wait for T3-TwLast to
    /// handle RNI in microcode. The next instruction byte will be fetched on this
    /// terminating cycle and the beginning of execution will overlap with T4.
    pub fn biu_bus_wait_until_tx(&mut self) {
        match self.bus_status_latch {
            BusStatus::MemRead
            | BusStatus::MemWrite
            | BusStatus::IoRead
            | BusStatus::IoWrite
            | BusStatus::CodeFetch => {
                self.trace_comment("WAIT_TX");
                while !self.is_last_wait() {
                    self.cycle();
                }
                self.trace_comment("TX");
            }
            _ => {}
        }
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

        match self.ta_cycle {
            TaCycle::Td => self.trace_comment("TD"),
            TaCycle::Tr => self.trace_comment("TR"),
            TaCycle::Ts => self.trace_comment("TS"),
            TaCycle::T0 => self.trace_comment("T0"),
            TaCycle::Ta => self.trace_comment("TA"),
        };

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
            TCycle::Ti => {
                // Bus is idle, enter Tr state immediately, except in the special case we are resuming
                // a fetch after a delay and the queue is at the policy length
                // if self.ta_cycle.in_address_cycle() && self.queue.at_policy_len() {
                //     self.biu_bus_wait_t0();
                // }
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

    /// Reset i8288 signals after an m-cycle has completed.
    #[inline]
    pub fn biu_bus_end(&mut self) {
        self.i8288.mrdc = false;
        self.i8288.amwc = false;
        self.i8288.mwtc = false;
        self.i8288.iorc = false;
        self.i8288.aiowc = false;
        self.i8288.iowc = false;
    }

    #[inline]
    pub fn biu_fetch_start(&mut self) {
        // Only start a fetch if the EU hasn't claimed the bus, we're not already in a fetch
        // address cycle, and fetching is not delayed.
        self.trace_comment("BFS");
        if self.bus_pending != BusPendingType::EuEarly && self.pl_status != BusStatus::CodeFetch {
            match self.fetch_state {
                FetchState::Delayed(_) => {}
                _ => {
                    self.fetch_state = FetchState::Normal;
                    self.biu_address_start(BusStatus::CodeFetch);
                }
            }
        }
    }

    /// Start a new address cycle. This will set `ta_cycle` to `TaCycle::Tr` and switch the pipeline
    /// slots.
    /// `ta_cycle` should never be set to Tr outside of this function.
    #[inline]
    pub fn biu_address_start(&mut self, new_bus_status: BusStatus) {
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
            self.trace_comment("AS_TS");
            self.ta_cycle = TaCycle::Ts;
        }
        else {
            // Did not abort, start new address cycle at Tr
            self.trace_comment("AS_TR");
            self.ta_cycle = TaCycle::Tr;
        }
        self.pl_status = new_bus_status;
    }

    /// Perform a (pre)fetch abort.
    /// Usually, this is called on T4 of a code fetch when a 'late' EU bus request is present.
    /// A late EU request is a request made on T2 or T3 of a bus cycle.
    /// Whether this is a T0 or a Tr state is academic, the next cycle will be
    /// a Ts state regardless.
    #[inline]
    pub fn biu_fetch_abort(&mut self) {
        self.trace_comment("ABORT");
        self.ta_cycle = TaCycle::Ta;
    }

    /// Begin a code fetch bus cycle. This is a special case of `biu_bus_begin` that is only
    /// used for code fetches.
    pub fn biu_bus_begin_fetch(&mut self) {
        let mut addr = Intel808x::calc_linear_address(self.cs, self.pc);

        if self.queue.has_room_for_fetch() {
            self.operand_size = match self.fetch_size {
                TransferSize::Byte => OperandSize::Operand8,
                TransferSize::Word => {
                    if addr & 1 != 0 {
                        // Unaligned code fetch - instruct the 8086 to discard one byte of the next fetch,
                        // while forcing fetch to even address.
                        addr = addr & !0x1;
                        self.queue.set_discard();
                    }
                    OperandSize::Operand16
                }
            };

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
            self.transfer_n = 1;
            self.final_transfer = true;
        }
    }

    /// Return a bool representing whether the current bus m-cycle is the last cycle of a transfer.
    /// A 'transfer' is either a word or byte size bus operation. On the 8088, word size transfers
    /// are always split into two byte transfers, but on the 8086 a word-aligned transfer can be
    /// word sized.
    #[inline]
    pub fn biu_is_last_transfer(&mut self) -> bool {
        (self.transfer_size == TransferSize::Word) || (self.transfer_n > 1)
    }

    pub fn biu_do_bus_transfer(&mut self) {
        let byte;

        match (self.bus_status_latch, self.transfer_size) {
            (BusStatus::CodeFetch, TransferSize::Byte) => {
                (byte, _) = self
                    .bus
                    .read_u8(self.address_latch as usize, self.instr_elapsed)
                    .unwrap();
                self.data_bus = byte as u16;

                validate_read_u8!(
                    self,
                    self.address_latch,
                    (self.data_bus & 0x00FF) as u8,
                    BusType::Mem,
                    ReadType::Code
                );
            }
            (BusStatus::CodeFetch, TransferSize::Word) => {
                (self.data_bus, _) = self
                    .bus
                    .read_u16(self.address_latch as usize, self.instr_elapsed)
                    .unwrap();
            }
            (BusStatus::MemRead, TransferSize::Byte) => {
                (byte, _) = self
                    .bus
                    .read_u8(self.address_latch as usize, self.instr_elapsed)
                    .unwrap();
                self.instr_elapsed = 0;
                self.data_bus = byte as u16;

                validate_read_u8!(
                    self,
                    self.address_latch,
                    (self.data_bus & 0x00FF) as u8,
                    BusType::Mem,
                    ReadType::Data
                );
            }
            (BusStatus::MemRead, TransferSize::Word) => {
                (self.data_bus, _) = self
                    .bus
                    .read_u16(self.address_latch as usize, self.instr_elapsed)
                    .unwrap();
                self.instr_elapsed = 0;
            }
            (BusStatus::MemWrite, TransferSize::Byte) => {
                self.i8288.mwtc = true;
                _ = self
                    .bus
                    .write_u8(
                        self.address_latch as usize,
                        (self.data_bus & 0x00FF) as u8,
                        self.instr_elapsed,
                    )
                    .unwrap();
                self.instr_elapsed = 0;

                validate_write_u8!(self, self.address_latch, (self.data_bus & 0x00FF) as u8, BusType::Mem);
            }
            (BusStatus::MemWrite, TransferSize::Word) => {
                self.i8288.mwtc = true;
                _ = self
                    .bus
                    .write_u16(self.address_latch as usize, self.data_bus, self.instr_elapsed)
                    .unwrap();
                self.instr_elapsed = 0;
            }
            (BusStatus::IoRead, TransferSize::Byte) => {
                self.i8288.iorc = true;
                byte = self
                    .bus
                    .io_read_u8((self.address_latch & 0xFFFF) as u16, self.instr_elapsed);
                self.data_bus = byte as u16;
                self.instr_elapsed = 0;

                validate_read_u8!(
                    self,
                    self.address_latch,
                    (self.data_bus & 0x00FF) as u8,
                    BusType::Io,
                    ReadType::Data
                );
            }
            (BusStatus::IoRead, TransferSize::Word) => {
                self.i8288.iorc = true;
                self.data_bus = self
                    .bus
                    .io_read_u16((self.address_latch & 0xFFFF) as u16, self.instr_elapsed);
                self.instr_elapsed = 0;
            }
            (BusStatus::IoWrite, TransferSize::Byte) => {
                self.i8288.iowc = true;
                self.bus.io_write_u8(
                    (self.address_latch & 0xFFFF) as u16,
                    (self.data_bus & 0x00FF) as u8,
                    self.instr_elapsed,
                    Some(&mut self.analyzer),
                );
                self.instr_elapsed = 0;

                validate_write_u8!(self, self.address_latch, (self.data_bus & 0x00FF) as u8, BusType::Io);
            }
            (BusStatus::IoWrite, TransferSize::Word) => {
                self.i8288.iowc = true;
                self.bus.io_write_u16(
                    (self.address_latch & 0xFFFF) as u16,
                    self.data_bus,
                    self.instr_elapsed,
                    Some(&mut self.analyzer),
                );
                self.instr_elapsed = 0;

                validate_write_u8!(self, self.address_latch, (self.data_bus & 0x00FF) as u8, BusType::Io);
            }
            (BusStatus::InterruptAck, TransferSize::Byte) => {
                // The vector is read from the PIC directly before we even enter an INTA bus state, so there's
                // nothing to do.

                //log::debug!("in INTA transfer_n: {}", self.transfer_n);
                // Deassert lock
                if self.transfer_n == 2 {
                    //log::debug!("deasserting lock! transfer_n: {}", self.transfer_n);
                    self.lock = false;
                    self.intr = false;
                }
                //self.transfer_n += 1;
            }
            _ => {
                trace_print!(self, "Unhandled bus state!");
                log::warn!("Unhandled bus status: {:?}!", self.bus_status_latch);
            }
        }

        self.bus_status = BusStatus::Passive;
        self.address_bus = (self.address_bus & !0xFF) | (self.data_bus as u32);
    }
}

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

    cpu_808x::logging.rs

    Implements cycle-state logging facilities.

*/

use crate::{
    cpu_808x::{
        microcode::{MC_CORR, MC_JUMP, MC_NONE, MC_RTN, MICROCODE_NUL, MICROCODE_SRC_8088},
        BusStatus,
        Cpu,
        DmaState,
        Intel808x,
        TCycle,
        TaCycle,
        CPU_FLAG_AUX_CARRY,
        CPU_FLAG_CARRY,
        CPU_FLAG_DIRECTION,
        CPU_FLAG_INT_ENABLE,
        CPU_FLAG_OVERFLOW,
        CPU_FLAG_PARITY,
        CPU_FLAG_SIGN,
        CPU_FLAG_TRAP,
        CPU_FLAG_ZERO,
    },
    cpu_common::{AnalyzerEntry, QueueOp, Segment, TraceMode},
    syntax_token::SyntaxToken,
};

pub enum BusSlotStatus {
    SlotA(BusStatus, TCycle),
    SlotB(BusStatus, TaCycle),
}

impl Intel808x {
    #[inline]
    pub fn do_cycle_trace(&mut self) {
        match self.trace_mode {
            TraceMode::CycleText => {
                // Get value of timer channel #1 for DMA printout
                let mut dma_count = 0;

                if let Some(pit) = self.bus.pit_mut().as_mut() {
                    (_, dma_count, _) = pit.get_channel_count(1);
                }

                let state_str = self.cycle_state_string(dma_count, false);
                self.trace_print(&state_str);
                self.trace_str_vec.push(state_str);

                self.trace_comment.clear();
                self.trace_instr = MC_NONE;
            }
            TraceMode::CycleCsv => {
                // Get value of timer channel #1 for DMA printout
                let mut dma_count = 0;

                if let Some(pit) = self.bus.pit_mut().as_mut() {
                    (_, dma_count, _) = pit.get_channel_count(1);
                }

                let token_vec = self.cycle_state_tokens(dma_count, false);
                //self.trace_print(&state_str);
                self.trace_token_vec.push(token_vec);

                self.trace_comment.clear();
                self.trace_instr = MC_NONE;
            }
            TraceMode::CycleSigrok => {
                self.push_analyzer_entry();
            }
            _ => {}
        }
    }

    pub fn instruction_state_string(&self, last_cs: u16, last_ip: u16) -> String {
        let mut instr_str = String::new();

        instr_str.push_str(&format!("{:04x}:{:04x} {}\n", last_cs, last_ip, self.i));
        instr_str.push_str(&format!(
            "AX: {:04x} BX: {:04x} CX: {:04x} DX: {:04x}\n",
            self.a.x(),
            self.b.x(),
            self.c.x(),
            self.d.x()
        ));
        instr_str.push_str(&format!(
            "SP: {:04x} BP: {:04x} SI: {:04x} DI: {:04x}\n",
            self.sp, self.bp, self.si, self.di
        ));
        instr_str.push_str(&format!(
            "CS: {:04x} DS: {:04x} ES: {:04x} SS: {:04x}\n",
            self.cs, self.ds, self.es, self.ss
        ));
        instr_str.push_str(&format!("IP: {:04x} FLAGS: {:04x}", self.ip(), self.flags));

        instr_str
    }

    pub fn emit_header(&mut self) {
        match self.trace_mode {
            TraceMode::CycleSigrok => self.trace_print(AnalyzerEntry::emit_header()),
            _ => {}
        }
    }

    pub fn push_analyzer_entry(&mut self) {
        if self.analyzer.need_flush() {
            log::debug!("Emitting {} analyzer entries", self.analyzer.entries.len());
            self.emit_analyzer_entries();
            //self.analyzer.flush();
        }

        let mut vs = false;
        let mut hs = false;
        let mut den = false;
        if let Some(video) = self.bus().primary_video() {
            let (vs_b, hs_b, den_b, _brd_b) = video.get_sync();
            vs = vs_b;
            hs = hs_b;
            den = den_b;
        }

        let mut intr = false;
        if let Some(pic) = self.bus().pic() {
            (intr, _) = pic.calc_intr();
        }

        self.analyzer.push(AnalyzerEntry {
            cycle: self.cycle_num,
            address_bus: self.address_bus,
            ready: self.ready,
            q: !matches!(self.last_queue_op, QueueOp::Idle),
            q_op: self.last_queue_op as u8,
            bus_status: self.bus_status as u8,
            dma_req: self.dma_req,
            dma_holda: self.dma_holda,
            intr,

            vs,
            hs,
            den,
            // The rest of the fields must be filled out by devices
            ..Default::default()
        });
    }

    pub fn emit_analyzer_entries(&mut self) {
        while let Some(entry) = self.analyzer.pop_complete() {
            self.trace_emit(&entry.emit_edge(1, self.t_step));
            self.trace_emit(&entry.emit_edge(0, self.t_step));
        }
    }

    pub fn trace_csv_line(&mut self) {
        let q = self.last_queue_op as u8;
        let s = self.bus_status as u8;

        let mut vs = 0;
        let mut hs = 0;
        let mut den = 0;
        let mut brd = 0;
        if let Some(video) = self.bus().primary_video() {
            let (vs_b, hs_b, den_b, brd_b) = video.get_sync();
            vs = vs_b as u8;
            hs = hs_b as u8;
            den = den_b as u8;
            brd = brd_b as u8;
        }

        // Segment status bits are valid after ALE.
        if !self.i8288.ale {
            let seg_n = match self.bus_segment {
                Segment::ES => 0,
                Segment::SS => 1,
                Segment::CS | Segment::None => 2,
                Segment::DS => 3,
            };
            self.address_bus = (self.address_bus & 0b1100_1111_1111_1111_1111) | (seg_n << 16);
        }

        // Calculate timestamp
        self.t_stamp = self.cycle_num as f64 * self.t_step;

        // "Time(s),addr,clk,ready,qs,s,clk0,intr,dr0,vs,hs"
        // sigrok import string:
        // t,x20,l,l,x2,x3,l,l,l,l,l,l,l,l
        self.trace_emit(&format!(
            "{},{:05X},1,{},{},{},{},{},{},{},{},{},{},{}",
            self.t_stamp,
            self.address_bus,
            self.ready as u8,
            q,
            s,
            self.clk0 as u8,
            self.intr as u8,
            self.dma_req as u8,
            self.dma_holda as u8,
            vs,
            hs,
            den,
            brd
        ));

        self.trace_emit(&format!(
            "{},{:05X},0,{},{},{},{},{},{},{},{},{},{},{}",
            self.t_stamp + self.t_step_h,
            self.address_bus,
            self.ready as u8,
            q,
            s,
            self.clk0 as u8,
            self.intr as u8,
            self.dma_req as u8,
            self.dma_holda as u8,
            vs,
            hs,
            den,
            brd
        ));
    }

    pub fn cycle_state_string(&self, dma_count: u16, short: bool) -> String {
        let ale_str = match self.i8288.ale {
            true => "A:",
            false => "  ",
        };

        let mut seg_str = "  ";
        if self.t_cycle != TCycle::T1 {
            // Segment status only valid in T2+
            seg_str = match self.bus_segment {
                Segment::None => "  ",
                Segment::SS => "SS",
                Segment::ES => "ES",
                Segment::CS => "CS",
                Segment::DS => "DS",
            };
        }

        let q_op_chr = match self.last_queue_op {
            QueueOp::Idle => ' ',
            QueueOp::First => 'F',
            QueueOp::Flush => 'E',
            QueueOp::Subsequent => 'S',
        };

        let q_preload_char = match self.queue.has_preload() {
            true => '*',
            false => ' ',
        };

        /*
        let mut f_op_chr = match self.fetch_state {
            FetchState::Scheduled(_) => 'S',
            FetchState::Aborted(_) => 'A',
            //FetchState::Suspended => '!',
            _ => ' '
        };

        if self.fetch_suspended {
            f_op_chr = '!'
        }
        */

        // All read/write signals are active/low
        let rs_chr = match self.i8288.mrdc {
            true => 'R',
            false => '.',
        };
        let aws_chr = match self.i8288.amwc {
            true => 'A',
            false => '.',
        };
        let ws_chr = match self.i8288.mwtc {
            true => 'W',
            false => '.',
        };
        let ior_chr = match self.i8288.iorc {
            true => 'R',
            false => '.',
        };
        let aiow_chr = match self.i8288.aiowc {
            true => 'A',
            false => '.',
        };
        let iow_chr = match self.i8288.iowc {
            true => 'W',
            false => '.',
        };

        let is_reading = self.i8288.mrdc | self.i8288.iorc;
        let is_writing = self.i8288.mwtc | self.i8288.iowc;

        let mut xfer_str = "      ".to_string();
        if is_reading {
            xfer_str = format!("<-r {:02X}", self.data_bus);
        }
        else if is_writing {
            xfer_str = format!("w-> {:02X}", self.data_bus);
        }

        // Handle queue activity

        let mut q_read_str = "      ".to_string();

        let mut instr_str = String::new();

        if self.last_queue_op == QueueOp::First || self.last_queue_op == QueueOp::Subsequent {
            // Queue byte was read.
            q_read_str = format!("<-q {:02X}", self.last_queue_byte);
        }

        if self.last_queue_op == QueueOp::First {
            // First byte of opcode read from queue. Decode the full instruction
            instr_str = format!(
                "[{:04X}:{:04X}] {} ({}) ",
                self.cs, self.instruction_ip, self.i, self.i.size
            );
        }

        //let mut microcode_str = "   ".to_string();
        let microcode_line_str = match self.trace_instr {
            MC_JUMP => "JMP".to_string(),
            MC_RTN => "RET".to_string(),
            MC_CORR => "COR".to_string(),
            MC_NONE => "   ".to_string(),
            _ => {
                format!("{:03X}", self.trace_instr)
            }
        };

        let microcode_op_str = match self.trace_instr {
            i if usize::from(i) < MICROCODE_SRC_8088.len() => MICROCODE_SRC_8088[i as usize].to_string(),
            _ => MICROCODE_NUL.to_string(),
        };

        let _dma_dreq_chr = match self.dma_aen {
            true => 'R',
            false => '.',
        };

        let tx_cycle = match self.is_last_wait() {
            true => 'x',
            false => '.',
        };

        let ready_chr = if self.have_wait_states() { '.' } else { 'R' };

        let dma_count_str = &format!("{:02} {:02}", dma_count, self.dram_refresh_cycle_num);

        let dma_str = match self.dma_state {
            DmaState::Idle => dma_count_str,
            DmaState::Dreq => "DREQ",
            DmaState::Hrq => "HRQ ",
            DmaState::HoldA => "HLDA",
            DmaState::Operating(n) => match n {
                0 => "S1",
                1 => "S2",
                2 => "S3",
                3 => "S4",
                _ => dma_count_str,
            }, //DmaState::DmaWait(..) => "DMAW"
            DmaState::End => "END",
        };

        let mut cycle_str;

        let (slot0bus, slot0t, slot1bus, slot1t) = self.get_pl_slot_strings();

        if short {
            cycle_str = format!(
                "{:04} {:02}[{:05X}] {:02} {}{} M:{}{}{} I:{}{}{} |{:5}| {:04} {:02} | {:06} | {:<14}| {:1}{:1}{:1}[{:08}] {} | {:03} | {}",
                self.instr_cycle,
                ale_str,
                self.address_latch,
                seg_str,
                ready_chr,
                self.bus_wait_states,
                rs_chr, aws_chr, ws_chr, ior_chr, aiow_chr, iow_chr,
                dma_str,
                self.bus_status,
                self.t_cycle,
                xfer_str,
                format!("{:?}", self.bus_pending),
                q_op_chr,
                self.last_queue_len,
                q_preload_char,
                self.queue.to_string(),
                q_read_str,
                microcode_line_str,
                instr_str
            );
        }
        else {
            cycle_str = format!(
                "{:08}:{:04} {:02}[{:05X}] {:02} {}{}{} M:{}{}{} I:{}{}{} |{:5}| {:04} {:02} | {:04} {:02} {:04} {:02} | {:06} | {:<8}| {:<10} | {:1}{:1}{:1}[{:08}] {} | {}: {} | {}",
                self.cycle_num,
                self.instr_cycle,
                ale_str,
                self.address_latch,
                seg_str,
                ready_chr,
                self.bus_wait_states,
                tx_cycle,
                rs_chr, aws_chr, ws_chr, ior_chr, aiow_chr, iow_chr,
                dma_str,
                self.bus_status,
                self.t_cycle,
                slot0bus,
                slot0t,
                slot1bus,
                slot1t,
                xfer_str,
                format!("{:?}", self.bus_pending),
                format!("{:?}", self.fetch_state),
                q_op_chr,
                self.last_queue_len,
                q_preload_char,
                self.queue.to_string(),
                q_read_str,
                microcode_line_str,
                microcode_op_str,
                instr_str
            );
        }

        for c in &self.trace_comment {
            cycle_str.push_str(&format!("; {}", c));
        }

        cycle_str
    }

    pub fn cycle_state_tokens(&self, dma_count: u16, _short: bool) -> Vec<SyntaxToken> {
        let ale_str = match self.i8288.ale {
            true => "A",
            false => " ",
        }
        .to_string();
        let ale_token = SyntaxToken::Text(ale_str);

        let mut seg_str = "  ";
        if self.t_cycle != TCycle::T1 {
            // Segment status only valid in T2+
            seg_str = match self.bus_segment {
                Segment::None => "  ",
                Segment::SS => "SS",
                Segment::ES => "ES",
                Segment::CS => "CS",
                Segment::DS => "DS",
            };
        }
        let seg_token = SyntaxToken::Text(seg_str.to_string());

        let q_op_chr = match self.last_queue_op {
            QueueOp::Idle => ' ',
            QueueOp::First => 'F',
            QueueOp::Flush => 'E',
            QueueOp::Subsequent => 'S',
        };
        let q_op_token = SyntaxToken::Text(q_op_chr.to_string());

        let _q_preload_char = match self.queue.has_preload() {
            true => '*',
            false => ' ',
        };

        /*
        let mut f_op_chr = match self.fetch_state {
            FetchState::Scheduled(_) => 'S',
            FetchState::Aborted(_) => 'A',
            //FetchState::Suspended => '!',
            _ => ' '
        };

        if self.fetch_suspended {
            f_op_chr = '!'
        }
        */

        // All read/write signals are active/low
        let rs_chr = match self.i8288.mrdc {
            true => 'R',
            false => '.',
        };
        let aws_chr = match self.i8288.amwc {
            true => 'A',
            false => '.',
        };
        let ws_chr = match self.i8288.mwtc {
            true => 'W',
            false => '.',
        };
        let ior_chr = match self.i8288.iorc {
            true => 'R',
            false => '.',
        };
        let aiow_chr = match self.i8288.aiowc {
            true => 'A',
            false => '.',
        };
        let iow_chr = match self.i8288.iowc {
            true => 'W',
            false => '.',
        };

        let bus_str = match self.bus_status_latch {
            BusStatus::InterruptAck => "IRQA",
            BusStatus::IoRead => "IOR ",
            BusStatus::IoWrite => "IOW ",
            BusStatus::Halt => "HALT",
            BusStatus::CodeFetch => "CODE",
            BusStatus::MemRead => "MEMR",
            BusStatus::MemWrite => "MEMW",
            BusStatus::Passive => "PASV",
        };
        let bus_str_token = SyntaxToken::Text(bus_str.to_string());

        let t_str = match self.t_cycle {
            TCycle::Tinit => "Tx",
            TCycle::Ti => "Ti",
            TCycle::T1 => "T1",
            TCycle::T2 => "T2",
            TCycle::T3 => "T3",
            TCycle::T4 => "T4",
            TCycle::Tw => "Tw",
        };
        let t_str_token = SyntaxToken::Text(t_str.to_string());

        let is_reading = self.i8288.mrdc | self.i8288.iorc;
        let is_writing = self.i8288.mwtc | self.i8288.iowc;

        let mut xfer_str = "      ".to_string();
        if is_reading {
            xfer_str = format!("<-r {:02X}", self.data_bus);
        }
        else if is_writing {
            xfer_str = format!("w-> {:02X}", self.data_bus);
        }

        // Handle queue activity

        let mut q_read_str = "      ".to_string();

        let mut instr_str = String::new();

        if self.last_queue_op == QueueOp::First || self.last_queue_op == QueueOp::Subsequent {
            // Queue byte was read.
            q_read_str = format!("<-q {:02X}", self.last_queue_byte);
        }
        let q_read_token = SyntaxToken::Text(q_read_str.to_string());

        if self.last_queue_op == QueueOp::First {
            // First byte of opcode read from queue. Decode the full instruction
            instr_str = format!(
                "[{:04X}:{:04X}] {} ({}) ",
                self.cs, self.instruction_ip, self.i, self.i.size
            );
        }
        let instr_str_token = SyntaxToken::Text(instr_str.to_string());

        //let mut microcode_str = "   ".to_string();
        let microcode_line_str = match self.trace_instr {
            MC_JUMP => "JMP".to_string(),
            MC_RTN => "RET".to_string(),
            MC_CORR => "COR".to_string(),
            MC_NONE => "   ".to_string(),
            _ => {
                format!("{:03X}", self.trace_instr)
            }
        };
        let microcode_line_token = SyntaxToken::Text(microcode_line_str.to_string());

        let microcode_op_str = match self.trace_instr {
            i if usize::from(i) < MICROCODE_SRC_8088.len() => MICROCODE_SRC_8088[i as usize].to_string(),
            _ => MICROCODE_NUL.to_string(),
        };
        let microcode_op_token = SyntaxToken::Text(microcode_op_str.to_string());

        let _dma_dreq_chr = match self.dma_aen {
            true => 'R',
            false => '.',
        };

        let tx_cycle = match self.is_last_wait() {
            true => 'x',
            false => '.',
        };

        let ready_chr = if self.bus_wait_states > 0 { '.' } else { 'R' };

        let dma_count_str = &format!("{:02} {:02}", dma_count, self.dram_refresh_cycle_num);

        let dma_str = match self.dma_state {
            DmaState::Idle => dma_count_str,
            DmaState::Dreq => "DREQ",
            DmaState::Hrq => "HRQ ",
            DmaState::HoldA => "HLDA",
            DmaState::Operating(n) => match n {
                4 => "S1",
                3 => "S2",
                2 => "S3",
                1 => "S4",
                _ => "S?",
            }, //DmaState::DmaWait(..) => "DMAW"
            DmaState::End => "END",
        };
        let _dma_str_token = SyntaxToken::Text(dma_str.to_string());

        let mut comment_str = String::new();
        for c in &self.trace_comment {
            comment_str.push_str(&format!("; {}", c));
        }

        let bus_signal_token = SyntaxToken::Text(format!(
            "M:{}{}{} I:{}{}{}",
            rs_chr, aws_chr, ws_chr, ior_chr, aiow_chr, iow_chr
        ));

        let token_vec = vec![
            SyntaxToken::Text(format!("{:04}", self.cycle_num)),
            SyntaxToken::Text(format!("{:04}", self.instr_cycle)),
            ale_token,
            SyntaxToken::Text(format!("{:05X}", self.address_bus)),
            seg_token,
            SyntaxToken::Text(ready_chr.to_string()),
            SyntaxToken::Text(self.bus_wait_states.to_string()),
            SyntaxToken::Text(tx_cycle.to_string()),
            bus_signal_token,
            SyntaxToken::Text(dma_str.to_string()),
            bus_str_token,
            t_str_token,
            SyntaxToken::Text(xfer_str),
            SyntaxToken::Text(format!("{:?}", self.fetch_state)),
            q_op_token,
            SyntaxToken::Text(self.last_queue_len.to_string()),
            SyntaxToken::Text(self.queue.to_string()),
            q_read_token,
            microcode_line_token,
            microcode_op_token,
            instr_str_token,
            SyntaxToken::Text(comment_str),
        ];

        token_vec
    }

    pub fn cycle_table_header(&self) -> Vec<String> {
        vec![
            "Cycle".to_string(),
            "icyc".to_string(),
            "ALE".to_string(),
            "Addr  ".to_string(),
            "Seg".to_string(),
            "Rdy".to_string(),
            "WS".to_string(),
            "Tx".to_string(),
            "8288       ".to_string(),
            "DMA  ".to_string(),
            "Bus ".to_string(),
            "T ".to_string(),
            "Xfer  ".to_string(),
            "BIU".to_string(),
            "Fetch       ".to_string(),
            "Qop".to_string(),
            "Ql".to_string(),
            "Queue   ".to_string(),
            "Qrd   ".to_string(),
            "MCPC".to_string(),
            "Microcode".to_string(),
            "Instr                   ".to_string(),
            "Comments".to_string(),
        ]
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

    /// Convert the two slots returned by get_pl_slots() into pairs of (bus, t-state) strings for display.
    pub fn get_pl_slot_strings(&self) -> (String, String, String, String) {
        let mut slot0_bus_str = String::from("    ");
        let mut slot0_t_str = String::from("  ");

        let mut slot1_bus_str = String::from("    ");
        let mut slot1_t_str = String::from("  ");

        let pl_slots = self.get_pl_slots();

        match pl_slots[0] {
            Some(BusSlotStatus::SlotA(status, cycle)) => {
                slot0_bus_str = format!("{:04}", status);
                slot0_t_str = format!("{:02}", cycle);
            }
            Some(BusSlotStatus::SlotB(status, cycle)) => {
                slot0_bus_str = format!("{:04}", status);
                slot0_t_str = format!("{:02}", cycle);
            }
            None => {}
        }
        match pl_slots[1] {
            Some(BusSlotStatus::SlotA(status, cycle)) => {
                slot1_bus_str = format!("{:04}", status);
                slot1_t_str = format!("{:02}", cycle);
            }
            Some(BusSlotStatus::SlotB(status, cycle)) => {
                slot1_bus_str = format!("{:04}", status);
                slot1_t_str = format!("{:02}", cycle);
            }
            None => {}
        }

        (slot0_bus_str, slot0_t_str, slot1_bus_str, slot1_t_str)
    }

    /// Internally, we don't use pipeline slots to model the pipeline state. But visualizing the
    /// bus states and t-cycles as two separate pipelines is more clear in logs. This function
    /// splits the bus states into two slots based on the current pipeline slot flag which should
    /// be toggled every time we enter a Tr cycle.
    pub fn get_pl_slots(&self) -> [Option<BusSlotStatus>; 2] {
        let mut slots = [None, None];

        // Scenario 1: T cycle is Ti, Ta cycle is inactive. Always emit Ti in slot 0.
        if self.t_cycle == TCycle::Ti && self.ta_cycle == TaCycle::Td {
            slots[0] = Some(BusSlotStatus::SlotA(self.bus_status_latch, self.t_cycle));
            slots[1] = None;
            return slots;
        }
        // Scenario 2: T cycle is Ti, Ta cycle is valid. Emit Ta in slot 0.
        if self.t_cycle == TCycle::Ti && self.ta_cycle != TaCycle::Td {
            slots[0] = Some(BusSlotStatus::SlotB(self.bus_status_latch, self.ta_cycle));
            slots[1] = None;
            return slots;
        }
        // Scenario 3: T cycle is T1-T4, Ta cycle is inactive. Emit T cycle in pl_slot.
        if self.t_cycle != TCycle::Ti && self.ta_cycle == TaCycle::Td {
            slots[self.pl_slot as usize] = Some(BusSlotStatus::SlotA(self.bus_status_latch, self.t_cycle));
            slots[(!self.pl_slot) as usize] = None;
            return slots;
        }
        // Scenario 4: T cycle is T1-T4, Ta cycle is valid. Emit the Ta cycle in pl_slot, T cycle in the other.
        if self.t_cycle != TCycle::Ti && self.ta_cycle != TaCycle::Td {
            slots[self.pl_slot as usize] = Some(BusSlotStatus::SlotB(self.pl_status, self.ta_cycle));
            slots[(!self.pl_slot) as usize] = Some(BusSlotStatus::SlotA(self.bus_status_latch, self.t_cycle));
            return slots;
        }
        panic!("Unhandled pl_slot scenario in get_pl_slots()");
    }
}

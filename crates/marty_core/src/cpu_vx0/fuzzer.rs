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

    cpu_vx0::fuzzer.rs

    Miscellaneous routines to generate random CPU state and instructions.

*/

use rand::{Rng, SeedableRng};

use crate::{
    cpu_common::Segment,
    cpu_vx0::{
        modrm::{ModRmByte, MODRM_REG_MASK},
        *,
    },
};

const RNG_SEED: u64 = 0x58158258u64;

macro_rules! get_rand {
    ($myself: expr) => {
        $myself.rng.as_mut().unwrap().gen()
    };
}

macro_rules! get_weighted_rand {
    ($myself:expr) => {{
        let p: f64 = $myself.rng.as_mut().unwrap().gen();
        if p < 0.05 {
            0
        }
        else {
            $myself.rng.as_mut().unwrap().gen()
        }
    }};
}

macro_rules! get_rand_range {
    ($myself: expr, $begin: expr, $end: expr) => {
        $myself.rng.as_mut().unwrap().gen_range($begin..$end)
    };
}

const DISABLE_SEG_OVERRIDES: [u8; 121] = [
    0x06, 0x07, 0x0E, 0x16, 0x17, 0x1E, 0x1F, 0x27, 0x2F, 0x37, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47,
    0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A,
    0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60, 0x61, 0x68, 0x6A, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79,
    0x7A, 0x7B, 0x7C, 0x7D, 0x7E, 0x7F, 0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9B, 0x9C, 0x9D,
    0x9E, 0x9F, 0xA8, 0xA9, 0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE,
    0xBF, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD, 0xCE, 0xCF, 0xD4, 0xD5, 0xE4, 0xE5, 0xE6, 0xE7, 0xEC, 0xED, 0xEE, 0xEF,
    0xF5, 0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD,
];

impl NecVx0 {
    #[allow(dead_code)]
    pub fn randomize_seed(&mut self, mut seed: u64) {
        if seed == 0 {
            seed = RNG_SEED;
        }
        self.rng = Some(rand::rngs::StdRng::seed_from_u64(seed));
    }

    #[allow(dead_code)]
    pub fn randomize_regs(&mut self) {
        self.cs = get_rand!(self);
        self.pc = get_rand!(self);

        self.set_reset_vector(CpuAddress::Segmented(self.cs, self.pc));
        self.reset();

        for reg in REGISTER16_LUT {
            let n: u16 = get_weighted_rand!(self);
            self.set_register16(reg, n);
        }

        // Flush queue
        self.queue.flush();

        self.ds = get_rand!(self);
        self.ss = get_rand!(self);
        self.es = get_rand!(self);

        // Randomize flags
        let mut flags: u16 = get_rand!(self);
        // Clear trap & interrupt flags
        flags &= !CPU_FLAG_TRAP;
        flags &= !CPU_FLAG_INT_ENABLE;

        self.set_flags(flags);

        //self.set_flags(0);
    }

    #[allow(dead_code)]
    pub fn randomize_mem(&mut self, weight: bool) {
        for i in 0..self.bus.size() {
            let n: u8 = if weight {
                let p = self.rng.as_mut().unwrap().gen::<f64>();
                if p < 0.01 {
                    0
                }
                else if p < 0.02 {
                    0xFF
                }
                else {
                    get_rand!(self)
                }
            }
            else {
                get_rand!(self)
            };

            self.bus.write_u8(i, n, 0).expect("Mem err");
        }

        // Write a basic IVT to handle DIV exceptions.
        self.bus.write_u16(0x00000, 0x0400, 0).expect("Mem err writing IVT");
        self.bus.write_u16(0x00002, 0x0000, 0).expect("Mem err writing IVT");
        self.bus.write_u8(0x00400, 0xCF, 0).expect("Mem err writing IRET");
    }

    #[allow(dead_code)]
    pub fn random_inst_from_opcodes(&mut self, opcode_list: &[u8], prefix: Option<u8>) {
        let mut instr: VecDeque<u8> = VecDeque::new();

        // Randomly pick one opcode from the provided list
        let opcode_i = get_rand_range!(self, 0, opcode_list.len());
        let opcode = opcode_list[opcode_i];

        if let Some(prefix) = prefix {
            instr.push_back(prefix);
        }
        instr.push_back(opcode);

        let mut enable_segment_prefix = true;

        if DISABLE_SEG_OVERRIDES.contains(&opcode) {
            enable_segment_prefix = false;
        }

        let mut mask_nesting_level = false;
        let mut mask_ins_immediate = false;

        // Add rep prefixes to string ops with 50% probability
        let do_rep_prefix: u8 = get_rand!(self);
        match prefix {
            None => {
                match opcode {
                    0xC8 => {
                        // ENTER
                        mask_nesting_level = true;
                    }
                    0x6C..=0x6F | 0xA4..=0xA7 | 0xAA..=0xAF => {
                        // String ops
                        match do_rep_prefix {
                            0..=32 => {
                                instr.push_front(0x64); // REPNC
                            }
                            33..=64 => {
                                instr.push_front(0x65); // REPC
                            }
                            65..=96 => {
                                instr.push_front(0xF2); // REPNZ
                            }
                            97..=128 => {
                                instr.push_front(0xF3); // REPZ
                            }
                            _ => {}
                        }

                        // Mask CX to 7 bits.
                        self.set_register16(Register16::CX, self.get_register16(Register16::CX) & 0x7F);
                    }
                    0x9D => {
                        // POPF.
                        // We need to modify the word at SS:SP to clear the trap flag bit.

                        let flat_addr = self.calc_linear_address_seg(Segment::SS, self.sp);

                        let (mut flag_word, _) = self
                            .bus_mut()
                            .read_u16(flat_addr as usize, 0)
                            .expect("Couldn't read stack!");

                        // Clear trap flag
                        flag_word &= !CPU_FLAG_TRAP;

                        self.bus_mut()
                            .write_u16(flat_addr as usize, flag_word, 0)
                            .expect("Couldn't write stack!");
                    }
                    0xCF => {
                        // IRET.
                        // We need to modify the word at SS:SP + 4 to clear the trap flag bit.

                        let flat_addr = self.calc_linear_address_seg(Segment::SS, self.sp.wrapping_add(4));

                        let (mut flag_word, _) = self
                            .bus_mut()
                            .read_u16(flat_addr as usize, 0)
                            .expect("Couldn't read stack!");

                        // Clear trap flag
                        flag_word &= !CPU_FLAG_TRAP;

                        self.bus_mut()
                            .write_u16(flat_addr as usize, flag_word, 0)
                            .expect("Couldn't write stack!");
                    }
                    0xD2 | 0xD3 => {
                        // Shifts and rotates by cl.
                        // Mask CL to 6 bits to shorten tests.
                        // This will still catch emulators that are masking CL to 5 bits.

                        self.c.set_l(self.c.l() & 0x3F);
                    }
                    _ => {}
                }
            }
            Some(0x0F) => match opcode {
                0x20 | 0x22 | 0x26 => {
                    // V20 BCD string instructions.

                    // Mask CL to 7 bits.
                    self.c.set_l(self.c.l() & 0x7F);
                    // Don't allow a 0 count.
                    while self.c.l() == 0 {
                        self.c.set_l(get_rand!(self));
                    }
                }
                _ => {}
            },
            _ => {
                panic!("Invalid prefix!");
            }
        }

        let mut modrm_valid = false;
        let mut modrm_byte: u8 = get_rand!(self);

        while !modrm_valid {
            modrm_byte = get_rand!(self);

            // Filter out invalid forms of some instructions that cannot
            // reasonably be validated.
            match prefix {
                None => {
                    match opcode {
                        // BOUND
                        0x62 | 0x63 => {
                            if modrm_byte & 0xC0 == 0xC0 {
                                // Reg form, invalid.
                                continue;
                            }
                        }
                        // LEA
                        0x8D => {
                            if modrm_byte & 0xC0 == 0xC0 {
                                // Reg form, invalid.
                                continue;
                            }
                        }
                        // LES | LDS
                        0xC4 | 0xC5 => {
                            if modrm_byte & 0xC0 == 0xC0 {
                                // Reg form, invalid.
                                continue;
                            }
                        }
                        0x8E => {
                            // Mov Sreg, modrm
                            if ((modrm_byte >> 3) & 0x03) == 0x01 {
                                // CS register destination. invalid.
                                continue;
                            }
                        }
                        // POP
                        0x8F => {
                            if (modrm_byte >> 3) & 0x07 != 0 {
                                // reg != 0, invalid.
                                continue;
                            }
                            if (modrm_byte & 0xC0) == 0xC0 {
                                // register form invalid
                                continue;
                            }
                            //log::debug!("Picked valid modrm for 0x8F: {:02X}", modrm_byte);
                        }
                        _ => {}
                    }
                }
                Some(0x0F) => match opcode {
                    0x31 | 0x33 | 0x39 | 0x3B => {
                        // INS/EXT. mod!=3 is invalid.
                        if modrm_byte & 0xC0 != 0xC0 {
                            continue;
                        }

                        mask_ins_immediate = true;

                        // We have to mask the operand registers to 4 bits.
                        let modrm = ModRmByte::from(modrm_byte);

                        let op1_reg8 = modrm.get_op1_reg8();

                        if let 0x33 | 0x3B = opcode {
                            if let Register8::AH = op1_reg8 {
                                // AH is invalid op1 for EXT. Will throw a ~1000 cycle tantrum
                                continue;
                            }

                            if let Register8::AL = op1_reg8 {
                                // AL is invalid op1 for INS. Can cause incorrectly sized operations
                                continue;
                            }
                        }

                        self.set_register8(op1_reg8, self.get_register8(op1_reg8) & 0x0F);

                        let op2_reg8 = modrm.get_op2_reg8();
                        self.set_register8(op2_reg8, self.get_register8(op2_reg8) & 0x0F);
                    }
                    _ => {}
                },
                _ => {}
            }

            modrm_valid = true;
        }

        // Add 'modrm' byte (even if not used)
        //let modrm_byte: u8 = get_rand!(self);

        instr.push_back(modrm_byte);

        // Add a segment override prefix with 50% probability
        let do_segment_prefix: u8 = get_rand!(self);

        if enable_segment_prefix && do_segment_prefix > 127 {
            // use last 4 bits to determine prefix
            match do_segment_prefix & 0x03 {
                0b00 => instr.push_front(0x26), // ES override
                0b01 => instr.push_front(0x2E), // CS override
                0b10 => instr.push_front(0x36), // SS override
                0b11 => instr.push_front(0x3E), // DS override
                _ => {}
            }
        }

        // Add five random instruction bytes (+modrm makes 6)
        for i in 0..5 {
            let instr_byte: u8 = get_rand!(self);
            if mask_nesting_level && (i == 1) {
                // This is actually the third byte, since we pushed one byte for modrm already
                // (even if no modrm)
                instr.push_back(instr_byte & 0x1F);
            }
            else if mask_ins_immediate && (i == 0) {
                // Mask immediate (bit length) operand to 0x0F or things get screwy
                instr.push_back(instr_byte & 0x0F);
            }
            else {
                instr.push_back(instr_byte);
            }
        }

        // Copy instruction to memory at CS:IP
        let addr = NecVx0::calc_linear_address(self.cs, self.pc);
        log::debug!("Using instruction vector: {:X?}", instr.make_contiguous());
        self.bus
            .copy_from(instr.make_contiguous(), (addr & 0xFFFFF) as usize, 0, false)
            .unwrap();
    }

    #[allow(dead_code)]
    pub fn random_grp_instruction(&mut self, opcode: u8, extension_list: &[u8]) {
        let mut instr: VecDeque<u8> = VecDeque::new();

        // Randomly pick one extension from the provided list
        let extension_i = get_rand_range!(self, 0, extension_list.len());
        let extension = extension_list[extension_i];

        instr.push_back(opcode);

        let do_rep_prefix: u8 = get_rand!(self);

        if let (0xF6 | 0xF7, 0x07) = (opcode, extension) {
            // IDIV
            // REP prefixes on IDIV invert quotient (undocumented)
            match do_rep_prefix {
                0..=0x5 => {
                    // Inject REP prefix at 5% probability
                    instr.push_front(0xF2); // REPNZ
                }
                0x06..=0x10 => {
                    // Inject REP prefix at 5% probability
                    instr.push_front(0xF3); // REPZ
                }
                _ => {}
            }
        }

        // Add a segment override prefix with 50% probability
        let do_segment_prefix: u8 = get_rand!(self);

        if do_segment_prefix > 127 {
            // use last 4 bits to determine prefix
            match do_segment_prefix & 0x03 {
                0b00 => instr.push_front(0x26), // ES override
                0b01 => instr.push_front(0x2E), // CS override
                0b10 => instr.push_front(0x36), // SS override
                0b11 => instr.push_front(0x3E), // DS override
                _ => {}
            }
        }

        let mut modrm_valid = false;
        // Add a modrm
        let mut modrm_byte: u8 = get_rand!(self);

        while !modrm_valid {
            modrm_byte = get_rand!(self);

            // Inject the operand extension. First, clear the REG bits
            modrm_byte &= !MODRM_REG_MASK;

            // Now set the reg bits to extension #
            modrm_byte |= (extension << 3) & MODRM_REG_MASK;

            // Filter out invalid forms of some instructions that cannot
            // reasonably be validated.
            if let (0xFE | 0xFF, 0x03 | 0x05) = (opcode, extension) {
                // FE.3 FE.5 FF.3 FF.5 CALLF
                if modrm_byte & 0xC0 == 0xC0 {
                    // Reg form, invalid.
                    continue;
                }
            }

            modrm_valid = true;
        }

        // Finally push the modrm
        instr.push_back(modrm_byte);

        // Adjust immediate operands
        let mut adjust_immediate8 = false;
        let mut immediate_idx = 0;
        match opcode {
            0xC0 | 0xC1 => {
                // Limit iteration count for C0 and C1.
                // We need to mask the immediate operand to 6 bits.
                adjust_immediate8 = true;
                let (_, modrm_size) = ModRmByte::peek(modrm_byte);
                immediate_idx = modrm_size - 1;
            }
            _ => {}
        }

        // Add five random instruction bytes (6 - modrm)
        for i in 0..6 {
            let instr_byte: u8 = get_rand!(self);

            if adjust_immediate8 && (i == immediate_idx) {
                //log::warn!("Adjusting immediate for C0 & C1");
                instr.push_back(instr_byte & 0x3F);
            }
            else {
                instr.push_back(instr_byte);
            }
        }

        // Copy instruction to memory at CS:IP
        let addr = NecVx0::calc_linear_address(self.cs, self.pc);
        log::debug!("Using instruction vector: {:X?}", instr.make_contiguous());
        self.bus
            .copy_from(instr.make_contiguous(), addr as usize, 0, false)
            .unwrap();
    }
}

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

    cpu_808x::fuzzer.rs

    Miscellaneous routines to generate random CPU state and instructions.

*/

use rand::{Rng, SeedableRng};
//use rand::rngs::StdRng;

use crate::cpu_808x::{modrm::MODRM_REG_MASK, *};

const RNG_SEED: u64 = 0x58158258u64;

macro_rules! get_rand {
    ($myself: expr) => {
        $myself.rng.as_mut().unwrap().gen()
    };
}

macro_rules! get_rand_range {
    ($myself: expr, $begin: expr, $end: expr) => {
        $myself.rng.as_mut().unwrap().gen_range($begin..$end)
    };
}

impl Cpu {
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

        for i in 0..REGISTER16_LUT.len() {
            let n: u16 = get_rand!(self);
            self.set_register16(REGISTER16_LUT[i], n);
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
    pub fn randomize_mem(&mut self) {
        for i in 0..self.bus.size() {
            let n: u8 = get_rand!(self);
            self.bus.write_u8(i, n, 0).expect("Mem err");
        }

        // Write a basic IVT to handle DIV exceptions.
        self.bus.write_u16(0x00000, 0x0400, 0).expect("Mem err writing IVT");
        self.bus.write_u16(0x00002, 0x0000, 0).expect("Mem err writing IVT");
        self.bus.write_u8(0x00400, 0xCF, 0).expect("Mem err writing IRET");
    }

    #[allow(dead_code)]
    pub fn random_inst_from_opcodes(&mut self, opcode_list: &[u8]) {
        let mut instr: VecDeque<u8> = VecDeque::new();

        // Randomly pick one opcode from the provided list
        let opcode_i = get_rand_range!(self, 0, opcode_list.len());
        let opcode = opcode_list[opcode_i];

        instr.push_back(opcode);

        let mut enable_segment_prefix = true;

        // Add rep prefixes to string ops with 50% probability
        let do_rep_prefix: u8 = get_rand!(self);
        match opcode {
            0xA4..=0xA7 | 0xAA..=0xAF => {
                // String ops
                match do_rep_prefix {
                    0..=64 => {
                        instr.push_front(0xF2); // REPNZ
                    }
                    65..=128 => {
                        instr.push_front(0xF3); // REPZ
                    }
                    _ => {}
                }

                // Mask CX to 8 bits.
                //self.cx = self.cx & 0x00FF;
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
                flag_word = flag_word & !CPU_FLAG_TRAP;

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
                flag_word = flag_word & !CPU_FLAG_TRAP;

                self.bus_mut()
                    .write_u16(flat_addr as usize, flag_word, 0)
                    .expect("Couldn't write stack!");
            }
            0xD2 | 0xD3 => {
                // Shifts and rotates by cl.
                // Mask CL to 6 bits to shorten tests.
                // This will still catch emulators that are masking CL to 5 bits.

                self.cl = self.cl & 0x3F;
            }
            0xC0..=0xC3 | 0xC8..=0xCF => {
                // RETN, RETF, INT[X], IRET
                enable_segment_prefix = false;
            }
            0xF5 | 0xF8..=0xFD => {
                // Clear/set flags
                enable_segment_prefix = false;
            }
            _ => {}
        }

        let mut modrm_valid = false;
        let mut modrm_byte: u8 = get_rand!(self);

        while !modrm_valid {
            modrm_byte = get_rand!(self);

            // Filter out invalid forms of some instructions that cannot
            // reasonably be validated.
            match opcode {
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
        for _ in 0..5 {
            let instr_byte: u8 = get_rand!(self);

            instr.push_back(instr_byte);
        }

        // Copy instruction to memory at CS:IP
        let addr = Cpu::calc_linear_address(self.cs, self.pc);
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

        match (opcode, extension) {
            (0xF6 | 0xF7, 0x07) => {
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
            _ => {}
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
            match opcode {
                // FF group opcode
                0xFF => {
                    match modrm_byte & 0b00_111_000 {
                        0b00_011_000 => {
                            // FF.3 CALLF
                            if modrm_byte & 0xC0 == 0xC0 {
                                // Reg form, invalid.
                                continue;
                            }
                        }
                        0b00_101_000 => {
                            // FF.5 JMPF
                            if modrm_byte & 0xC0 == 0xC0 {
                                // Reg form, invalid.
                                continue;
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }

            modrm_valid = true;
        }

        // Finally push the modrm
        instr.push_back(modrm_byte);

        // Add five random instruction bytes (6 - modrm)
        for _ in 0..6 {
            let instr_byte: u8 = get_rand!(self);

            instr.push_back(instr_byte);
        }

        // Copy instruction to memory at CS:IP
        let addr = Cpu::calc_linear_address(self.cs, self.pc);
        log::debug!("Using instruction vector: {:X?}", instr.make_contiguous());
        self.bus
            .copy_from(instr.make_contiguous(), addr as usize, 0, false)
            .unwrap();
    }
}

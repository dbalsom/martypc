/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

use crate::cpu_808x::*;
use crate::cpu_808x::modrm::MODRM_REG_MASK;

const RNG_SEED: u64 = 0x58158258u64;

macro_rules! get_rand {
    ($myself: expr) => {
        $myself.rng.as_mut().unwrap().gen()
    }
}

macro_rules! get_rand_range {
    ($myself: expr, $begin: expr, $end: expr) => {
        $myself.rng.as_mut().unwrap().gen_range($begin..$end)
    }
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
        self.ip = get_rand!(self);

        self.set_reset_vector(CpuAddress::Segmented(self.cs, self.ip));
        self.reset();

        for i in 0..REGISTER16_LUT.len() {
            let n: u16 = get_rand!(self);
            self.set_register16(REGISTER16_LUT[i], n);
        }

        // Adjust pc
        self.pc = Cpu::calc_linear_address(self.cs, self.ip);
        // Flush queue
        self.queue.flush();

        self.ds = get_rand!(self);
        self.ss = get_rand!(self);
        self.es = get_rand!(self);

        // Randomize flags
        let mut flags: u16 = get_rand!(self);
        // Clear trap flag
        flags &= !CPU_FLAG_TRAP;
        self.set_flags(flags);

        //self.set_flags(0);
    }

    #[allow(dead_code)]
    pub fn randomize_mem(&mut self) {

        for i in 0..self.bus.size() {

            let n: u8 = get_rand!(self);
            self.bus.write_u8(i, n, 0).expect("Mem err");
        }
    }

    #[allow(dead_code)]
    pub fn random_inst_from_opcodes(&mut self, opcode_list: &[u8]) {

        let mut instr: VecDeque<u8> = VecDeque::new();

        // Randomly pick one opcode from the provided list
        let opcode_i = get_rand_range!(self, 0, opcode_list.len());
        let opcode = opcode_list[opcode_i];

        instr.push_back(opcode);

        // Add rep prefixes to string ops with 50% probability
        let do_rep_prefix: u8 = get_rand!(self);
        match opcode {
            0xA4..=0xA7 | 0xAA..=0xAF => { // String ops
                match do_rep_prefix {
                    0..=64 => {
                        instr.push_front(0xF2); // REPNZ
                    }
                    65..=128 => {
                        instr.push_front(0xF3);  // REPZ
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

        // Add six random instruction bytes
        for _ in 0..6 {
            let instr_byte: u8 = get_rand!(self);

            instr.push_back(instr_byte);
        }

        // Copy instruction to memory at CS:IP
        let addr = Cpu::calc_linear_address(self.cs, self.ip);
        self.bus.copy_from(instr.make_contiguous(), addr as usize, 0, false).unwrap();

    }

    #[allow(dead_code)]
    pub fn random_grp_instruction(&mut self, opcode: u8, extension_list: &[u8]) {

        let mut instr: VecDeque<u8> = VecDeque::new();

        // Randomly pick one extension from the provided list
        let extension_i = get_rand_range!(self, 0, extension_list.len());
        let extension = extension_list[extension_i];

        instr.push_back(opcode);

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


        // Add a modrm
        let mut modrm_byte: u8 = get_rand!(self);

        // Inject the operand extension. First, clear the REG bits
        modrm_byte &= !MODRM_REG_MASK;

        // Now set the reg bits to extension #
        modrm_byte |= (extension << 3) & MODRM_REG_MASK;

        // Finally push the modrm
        instr.push_back(modrm_byte);

        // Add five random instruction bytes (6 - modrm)
        for _ in 0..6 {
            let instr_byte: u8 = get_rand!(self);

            instr.push_back(instr_byte);
        }

        // Copy instruction to memory at CS:IP
        let addr = Cpu::calc_linear_address(self.cs, self.ip);
        self.bus.copy_from(instr.make_contiguous(), addr as usize, 0, false).unwrap();

    }
    

}
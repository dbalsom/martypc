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

    cpu_808x::gdr.rs

    Provides routines for querying an entry of the Group Decode ROM.

    Microcode disassembly by reenigne:
    https://www.reenigne.org/blog/8086-microcode-disassembled/

*/
#![allow(dead_code)]

/// Technically a PLA, the Group Decode ROM emits 15 signals given an 8-bit opcode for output.
/// These signals are encoded as a bitfield.
pub const GDR_IO: u16 = 0b0000_0000_0000_0001; // Instruction is an I/O instruction
pub const GDR_NO_LOAD_EA: u16 = 0b0000_0000_0000_0010; // Instruction does not load its EA (write-only)
pub const GDR_GRP_345: u16 = 0b0000_0000_0000_0100; // Instruction is a Group 3, 4, or 5 instruction
pub const GDR_PREFIX: u16 = 0b0000_0000_0000_1000; // Instruction is a prefix byte
pub const GDR_NO_MODRM: u16 = 0b0000_0000_0001_0000; // Instruction does not have a modrm byte
pub const GDR_SPECIAL_ALU: u16 = 0b0000_0000_0010_0000;
pub const GDR_CLEARS_COND: u16 = 0b0000_0000_0100_0000;
pub const GDR_USES_AREG: u16 = 0b0000_0000_1000_0000; // Instruction uses the AL or AX register specifically
pub const GDR_USES_SREG: u16 = 0b0000_0001_0000_0000; // Instruction uses a segment register
pub const GDR_D_VALID: u16 = 0b0000_0010_0000_0000; // 'D' bit is valid for instruction
pub const GRD_NO_MC: u16 = 0b0000_0100_0000_0000; // Instruction has no microcode
pub const GDR_W_VALID: u16 = 0b0000_1000_0000_0000; // 'W' bit is valid for instruction
pub const GDR_FORCE_BYTE: u16 = 0b0001_0000_0000_0000; // Instruction forces a byte operation
pub const GDR_L8: u16 = 0b0010_0000_0000_0000; // Instruction sets L8 flag
pub const GDR_UPDATE_CARRY: u16 = 0b0100_0000_0000_0000; // Instruction updates Carry flag

pub struct GdrEntry(pub u16);

impl GdrEntry {
    pub fn new(data: u16) -> Self {
        Self(data)
    }
    #[inline]
    pub fn get(&self) -> u16 {
        self.0
    }
    #[inline(always)]
    pub fn has_modrm(&self) -> bool {
        self.0 & GDR_NO_MODRM == 0
    }
    #[inline(always)]
    pub fn loads_ea(&self) -> bool {
        self.0 & GDR_NO_LOAD_EA == 0
    }
    #[inline(always)]
    pub fn w_valid(&self) -> bool {
        self.0 & GDR_W_VALID != 0
    }
    #[inline(always)]
    pub fn d_valid(&self) -> bool {
        self.0 & GDR_D_VALID != 0
    }
    #[inline(always)]
    pub fn force_byte(&self) -> bool {
        self.0 & GDR_FORCE_BYTE != 0
    }
    #[inline(always)]
    pub fn set_l8(&self) -> bool {
        self.0 & GDR_L8 != 0
    }
}

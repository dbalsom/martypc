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

    cpu_common::addressing.rs

    This module defines addressing modes shared between CPU types.

*/

use crate::cpu_common::{calc_linear_address, Register16};
use std::{fmt, fmt::Display};

#[derive(Copy, Clone, Debug)]
pub enum Displacement {
    NoDisp,
    Pending8,
    Pending16,
    Disp8(i8),
    Disp16(i16),
}

#[derive(Copy, Clone, Debug)]
pub enum AddressingMode {
    BxSi,
    BxDi,
    BpSi,
    BpDi,
    Si,
    Di,
    Disp16(Displacement),
    Bx,
    BxSiDisp8(Displacement),
    BxDiDisp8(Displacement),
    BpSiDisp8(Displacement),
    BpDiDisp8(Displacement),
    SiDisp8(Displacement),
    DiDisp8(Displacement),
    BpDisp8(Displacement),
    BxDisp8(Displacement),
    BxSiDisp16(Displacement),
    BxDiDisp16(Displacement),
    BpSiDisp16(Displacement),
    BpDiDisp16(Displacement),
    SiDisp16(Displacement),
    DiDisp16(Displacement),
    BpDisp16(Displacement),
    BxDisp16(Displacement),
    RegisterMode,
    RegisterIndirect(Register16),
}

pub(crate) struct SignedHex<T>(pub T);
pub(crate) struct WithPlusSign<T>(pub T);
pub(crate) struct WithSign<T>(pub T);

impl Displacement {
    pub fn get_i16(&self) -> i16 {
        match self {
            Displacement::Disp8(disp) => *disp as i16,
            Displacement::Disp16(disp) => *disp,
            _ => 0,
        }
    }
    pub fn get_u16(&self) -> u16 {
        match self {
            Displacement::Disp8(disp) => (*disp as i16) as u16,
            Displacement::Disp16(disp) => *disp as u16,
            _ => 0,
        }
    }
}

impl fmt::Display for Displacement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Displacement::Pending8 | Displacement::Pending16 | Displacement::NoDisp => {
                write!(f, "Invalid Displacement")
            }
            Displacement::Disp8(i) => {
                write!(f, "{:X}h", i)
            }
            Displacement::Disp16(i) => {
                write!(f, "{:X}h", i)
            }
        }
    }
}

impl fmt::Display for SignedHex<Displacement> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Displacement::Pending8 | Displacement::Pending16 | Displacement::NoDisp => {
                write!(f, "Invalid Displacement")
            }
            Displacement::Disp8(i) => {
                if *i < 0 {
                    write!(f, "{:X}h", !i.wrapping_sub(1))
                }
                else {
                    write!(f, "{:X}h", i)
                }
            }
            Displacement::Disp16(i) => {
                if *i < 0 {
                    write!(f, "{:X}h", !i.wrapping_sub(1))
                }
                else {
                    write!(f, "{:X}h", i)
                }
            }
        }
    }
}

impl Display for WithPlusSign<Displacement> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Displacement::Pending8 | Displacement::Pending16 | Displacement::NoDisp => {
                write!(f, "Invalid Displacement")
            }
            Displacement::Disp8(i) => {
                if *i < 0 {
                    write!(f, "-{}", SignedHex(self.0))
                }
                else {
                    write!(f, "+{}", SignedHex(self.0))
                }
            }
            Displacement::Disp16(i) => {
                if *i < 0 {
                    write!(f, "-{}", SignedHex(self.0))
                }
                else {
                    write!(f, "+{}", SignedHex(self.0))
                }
            }
        }
    }
}

impl Display for WithSign<Displacement> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Displacement::Pending8 | Displacement::Pending16 | Displacement::NoDisp => {
                write!(f, "Invalid Displacement")
            }
            Displacement::Disp8(i) => {
                if *i < 0 {
                    write!(f, "-{}", SignedHex(self.0))
                }
                else {
                    write!(f, "{}", SignedHex(self.0))
                }
            }
            Displacement::Disp16(i) => {
                if *i < 0 {
                    write!(f, "-{}", SignedHex(self.0))
                }
                else {
                    write!(f, "{}", SignedHex(self.0))
                }
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum CpuAddress {
    Flat(u32),
    Segmented(u16, u16),
    Offset(u16),
}

impl CpuAddress {
    pub fn to_flat(&self) -> CpuAddress {
        match self {
            CpuAddress::Flat(_) => *self,
            CpuAddress::Segmented(s, o) => CpuAddress::Flat(calc_linear_address(*s, *o)),
            CpuAddress::Offset(a) => CpuAddress::Flat(*a as u32),
        }
    }

    pub fn to_flat_u32(&self) -> u32 {
        match self {
            CpuAddress::Flat(a) => *a,
            CpuAddress::Segmented(s, o) => calc_linear_address(*s, *o),
            CpuAddress::Offset(a) => *a as u32,
        }
    }
}

impl Default for CpuAddress {
    fn default() -> CpuAddress {
        CpuAddress::Segmented(0, 0)
    }
}

impl From<CpuAddress> for u32 {
    fn from(cpu_address: CpuAddress) -> Self {
        match cpu_address {
            CpuAddress::Flat(a) => a,
            CpuAddress::Segmented(s, o) => calc_linear_address(s, o),
            CpuAddress::Offset(a) => a as Self,
        }
    }
}

impl From<&CpuAddress> for u32 {
    fn from(cpu_address: &CpuAddress) -> Self {
        match cpu_address {
            CpuAddress::Flat(a) => *a,
            CpuAddress::Segmented(s, o) => calc_linear_address(*s, *o),
            CpuAddress::Offset(a) => *a as Self,
        }
    }
}

impl From<CpuAddress> for usize {
    fn from(cpu_address: CpuAddress) -> Self {
        match cpu_address {
            CpuAddress::Flat(a) => a as usize,
            CpuAddress::Segmented(s, o) => calc_linear_address(s, o) as usize,
            CpuAddress::Offset(a) => a as Self,
        }
    }
}

impl From<&CpuAddress> for usize {
    fn from(cpu_address: &CpuAddress) -> Self {
        match cpu_address {
            CpuAddress::Flat(a) => *a as usize,
            CpuAddress::Segmented(s, o) => calc_linear_address(*s, *o) as usize,
            CpuAddress::Offset(a) => *a as Self,
        }
    }
}

impl Display for CpuAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CpuAddress::Flat(a) => write!(f, "{:05X}", a),
            CpuAddress::Segmented(s, o) => write!(f, "{:04X}:{:04X}", s, o),
            CpuAddress::Offset(a) => write!(f, "{:04X}", a),
        }
    }
}

impl PartialEq for CpuAddress {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (CpuAddress::Flat(a), CpuAddress::Flat(b)) => a == b,
            (CpuAddress::Flat(a), CpuAddress::Segmented(s, o)) => {
                let b = calc_linear_address(*s, *o);
                *a == b
            }
            (CpuAddress::Flat(_a), CpuAddress::Offset(_b)) => false,
            (CpuAddress::Segmented(s, o), CpuAddress::Flat(b)) => {
                let a = calc_linear_address(*s, *o);
                a == *b
            }
            (CpuAddress::Segmented(s1, o1), CpuAddress::Segmented(s2, o2)) => *s1 == *s2 && *o1 == *o2,
            _ => false,
        }
    }
}

impl Eq for CpuAddress {}

impl PartialOrd for CpuAddress {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CpuAddress {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_flat_u32().cmp(&other.to_flat_u32())
    }
}

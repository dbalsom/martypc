/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

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

    --------------------------------------------------------------------------
*/

//! Shared service interrupt test helpers.

use crate::cpu_common::Cpu;

pub(super) fn write_guest_bytes<C: Cpu>(cpu: &mut C, segment: u16, offset: u16, bytes: &[u8]) {
    for (index, value) in bytes.iter().copied().enumerate() {
        let address = crate::cpu_common::calc_linear_address(segment, offset.wrapping_add(index as u16));
        cpu.bus_mut().write_u8(address as usize, value, 0).unwrap();
    }
}

pub(super) fn read_guest_bytes<C: Cpu>(cpu: &mut C, segment: u16, offset: u16, length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| {
            let address = crate::cpu_common::calc_linear_address(segment, offset.wrapping_add(index as u16));
            cpu.bus_mut().read_u8(address as usize, 0).unwrap().0
        })
        .collect()
}

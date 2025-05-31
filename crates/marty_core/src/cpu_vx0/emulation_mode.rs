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
*/

//! Functions related to the NEC Vx0's 8080 emulation mode.

use crate::{
    cpu_common::{CpuArch, CpuType},
    cpu_vx0::{Flag, NecVx0},
};

impl NecVx0 {
    /// Return whether the CPU is in 8080 emulation mode
    #[inline]
    pub fn in_emulation_mode(&self) -> bool {
        self.emulation_mode
    }

    pub fn enter_emulation_mode(&mut self) {
        if self.in_emulation_mode() {
            // We're already in emulation mode...
            log::warn!("Vx0: Entering 8080 emulation mode while already in emulation mode");
        }
        // Clearing the mode flag enters emulation mode.
        self.clear_flag(Flag::Mode);

        // Set the CPU type to toggle disassembly mode
        self.cpu_type = match self.cpu_type {
            CpuType::NecV20(_) => CpuType::NecV20(CpuArch::I8080),
            CpuType::NecV30(_) => CpuType::NecV30(CpuArch::I8080),
            _ => {
                panic!("Invalid CPU type")
            }
        };
        self.decode.set_emulation_table();
        self.emulation_mode = true;
        //log::debug!("Entered emulation mode");
    }

    pub fn exit_emulation_mode(&mut self) {
        if !self.in_emulation_mode() {
            // We're not in emulation mode...
            log::warn!("Vx0: Exiting 8080 emulation mode while not in emulation mode");
        }

        // Setting the mode flag enters native mode.
        self.set_flag(Flag::Mode);

        // Set the CPU type to toggle disassembly mode
        self.cpu_type = match self.cpu_type {
            CpuType::NecV20(_) => CpuType::NecV20(CpuArch::I86),
            CpuType::NecV30(_) => CpuType::NecV30(CpuArch::I86),
            _ => {
                panic!("Invalid CPU type")
            }
        };
        self.decode.set_native_table();
        self.emulation_mode = false;
        //log::debug!("Exited emulation mode");
    }
}

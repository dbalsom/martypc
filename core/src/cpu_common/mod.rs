/*
    Marty PC Emulator 
    (C)2023 Daniel Balsom
    https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

    ---------------------------------------------------------------------------

    cpu_common::mod.rs

    Implements common functionality shared by different CPU types.

*/

#![allow(dead_code)]


#[derive (Copy, Clone, Debug)]
pub enum CpuType {
    Intel8088,
    Intel8086,
}

impl Default for CpuType {
    fn default() -> Self { CpuType::Intel8088 }
}

#[derive (Debug)]
pub enum CpuOption {
    InstructionHistory(bool),
    SimulateDramRefresh(bool, u32, u32),
    DramRefreshAdjust(u32),
    HaltResumeDelay(u32),
    OffRailsDetection(bool),
    EnableWaitStates(bool),
    TraceLoggingEnabled(bool)
}

use crate::cpu_808x::*;

pub mod alu;

impl<'a> Cpu<'a> {

    pub fn common_test(&self) {
        //log::trace!("I'm a common cpu function!");
    }
}
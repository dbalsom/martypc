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
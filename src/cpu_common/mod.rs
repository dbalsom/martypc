#![allow(dead_code)]


#[derive (Debug)]
pub enum CpuType {
    Intel8088,
    Intel8086,
}

impl Default for CpuType {
    fn default() -> Self { CpuType::Intel8088 }
}

pub enum CpuOption {
    InstructionHistory(bool),
    SimulateDramRefresh(bool, u32)
}

use crate::cpu_808x::*;

pub mod alu;

impl<'a> Cpu<'a> {

    pub fn common_test(&self) {
        //log::trace!("I'm a common cpu function!");
    }
}
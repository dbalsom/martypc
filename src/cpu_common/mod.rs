#![allow(dead_code)]

use crate::cpu_808x::*;

pub mod alu;

impl<'a> Cpu<'a> {

    pub fn common_test(&self) {
        //log::trace!("I'm a common cpu function!");
    }
}
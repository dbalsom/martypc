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

    --------------------------------------------------------------------------

    bpaf_toml_cnofig::coreconfig.rs

    Routines to parse configuration file and command line arguments.

    This library implements CoreConfig for BPAF & TOML parsing.
    This file implements the CoreConfig trait.
*/

use std::path::PathBuf;

use crate::ConfigFileParams;

use marty_core::{
    coreconfig::{CoreConfig, VideoCardDefinition},
    cpu_common::TraceMode,
    cpu_validator::ValidatorType,
    devices::{hdc::HardDiskControllerType, keyboard::KeyboardType},
    machine_manager::MachineType,
};

/*
#[derive(Debug, Deserialize)]
pub struct ConfigFileParams {
    pub emulator: Emulator,
    pub gui: Gui,
    pub input: Input,
    pub machine: Machine,
    pub cpu: Cpu,
    pub validator: Validator,
    pub tests: Tests
}
 */
impl CoreConfig for ConfigFileParams {
    fn get_base_dir(&self) -> PathBuf {
        self.emulator.basedir.clone()
    }
    fn get_machine_type(&self) -> MachineType {
        self.machine.model
    }
    fn get_machine_nobios(&self) -> bool {
        self.machine.no_bios
    }
    fn get_machine_turbo(&self) -> bool {
        self.machine.turbo
    }
    fn get_keyboard_type(&self) -> Option<KeyboardType> {
        self.machine.keyboard_type
    }
    fn get_keyboard_layout(&self) -> Option<String> {
        self.machine.keyboard_layout.clone()
    }
    fn get_keyboard_debug(&self) -> bool {
        self.emulator.debug_keyboard
    }

    fn get_video_cards(&self) -> Vec<VideoCardDefinition> {
        self.machine.videocard.clone().unwrap_or_else(Vec::new)
    }
    //fn get_video_type(&self) -> Option<VideoType> { self.machine.primary_video }
    //fn get_video_clockingmode(&self) -> Option<ClockingMode> { self.machine.clocking_mode }
    //fn get_video_debug(&self) -> bool { self.emulator.video_frame_debug }
    fn get_hdc_type(&self) -> Option<HardDiskControllerType> {
        self.machine.hdc
    }
    fn get_validator_type(&self) -> Option<ValidatorType> {
        self.validator.vtype
    }
    fn get_validator_trace_file(&self) -> Option<PathBuf> {
        self.validator.trace_file.clone()
    }
    fn get_validator_baud(&self) -> Option<u32> {
        self.validator.baud_rate
    }
    fn get_cpu_trace_mode(&self) -> Option<TraceMode> {
        self.emulator.trace_mode
    }
    fn get_cpu_trace_on(&self) -> bool {
        self.emulator.trace_on
    }
    fn get_cpu_trace_file(&self) -> Option<PathBuf> {
        self.emulator.trace_file.clone()
    }
}

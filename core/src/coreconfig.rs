/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2024 Daniel Balsom

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

    coreconfig.rs

    Definition of the CoreConfig trait which provides an interface for the
    core to retrieve and set configuration options from a configuration store.

    Front end implementations must implement this trait for their specific
    configuration formats.

*/

use crate::{
    cpu_common::TraceMode,
    cpu_validator::ValidatorType,
    device_traits::videocard::{ClockingMode, VideoType},
    devices::keyboard::KeyboardType,
    machine_types::{HardDiskControllerType, MachineType},
};
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Copy, Clone, Debug, Deserialize)]
pub struct VideoCardDefinition {
    pub video_type: VideoType,
    pub composite: Option<bool>,
    pub snow: Option<bool>,
    pub clocking_mode: Option<ClockingMode>,
    pub debug: Option<bool>,
}

pub trait CoreConfig {
    fn get_base_dir(&self) -> PathBuf;
    fn get_machine_type(&self) -> MachineType;
    fn get_machine_noroms(&self) -> bool;
    fn get_machine_turbo(&self) -> bool;
    fn get_keyboard_type(&self) -> Option<KeyboardType>;
    fn get_keyboard_layout(&self) -> Option<String>;
    fn get_keyboard_debug(&self) -> bool;
    //fn get_video_type(&self) -> Option<VideoType>;
    //fn get_video_clockingmode(&self) -> Option<ClockingMode>;
    //fn get_video_debug(&self) -> bool;
    fn get_hdc_type(&self) -> Option<HardDiskControllerType>;
    fn get_validator_type(&self) -> Option<ValidatorType>;
    fn get_validator_trace_file(&self) -> Option<PathBuf>;
    fn get_validator_baud(&self) -> Option<u32>;
    fn get_cpu_trace_mode(&self) -> Option<TraceMode>;
    fn get_cpu_trace_on(&self) -> bool;
    fn get_cpu_trace_file(&self) -> Option<PathBuf>;
}

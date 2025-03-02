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

    --------------------------------------------------------------------------
*/

//! The Frontend Common library provides facilities, data types and traits common to all front ends.
//! This avoids duplication of code and type definitions.
//!
//! Various Managers are provided here to help with the task of managing resources and state.
//! - TimeStepManager: Manages the timing of the emulator via callbacks that fire at specified
//!     intervals, to allow desynchronized guest FPS targeting vs GUI updates.
//! - ResourceManager: Manages the loading and unloading of resources, such as ROMs, disk images,
//!     cartridges, etc.
//! - DisplayManager: Manages the display of the emulator, including window management, scaling,
//!     aspect ratio, shaders (if applicable to platform), etc.
//! - MachineManager: Manages the parsing of machine configurations and the creation of machines
//!     from those configurations.
//! - RomManager: Manages the loading and resolution of ROMs based on requirements from a machine
//!     configuration
//! - FloppyManager: Manages the loading and unloading of floppy disk images
//! - VhdManager: Manages the loading and unloading of VHD disk images
//! - CartridgeManager: Manages the loading and unloading of ROM cartridges (PCjr specific)
#![feature(trait_alias)]

use serde_derive::Deserialize;

pub mod cartridge_manager;
pub mod color;
pub mod constants;
pub mod display_manager;

pub mod display_scaler;
//mod emulator_manager;
pub mod async_exec;
pub mod floppy_manager;
pub mod machine_manager;
pub mod resource_manager;
pub mod rom_manager;
pub mod thread_events;
pub mod timestep_manager;
pub mod types;
pub mod vhd_manager;

pub type FileTreeNode = resource_manager::tree::TreeNode;
pub type MartyGuiTheme = types::gui::MartyGuiTheme;
pub type HotkeyEvent = types::hotkeys::HotkeyEvent;
pub type HotkeyScope = types::hotkeys::HotkeyScope;
pub type HotkeyConfigEntry = types::hotkeys::HotkeyConfigEntry;
pub type JoyKeyEntry = types::joykeys::JoyKeyEntry;
pub type RelativeDirectory = types::floppy::RelativeDirectory;

pub use async_exec::exec_async;

#[derive(Copy, Clone, Debug, Default, PartialEq, Deserialize)]
pub enum BenchmarkEndCondition {
    #[default]
    Cycles,
    Timeout,
    Trigger,
}

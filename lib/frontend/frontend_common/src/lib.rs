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

    frontend_common::lib.rs

    This library provides facilities, data types and traits and common to all
    front ends.

*/

use serde_derive::Deserialize;

pub mod cartridge_manager;
pub mod color;
pub mod constants;
pub mod display_manager;
#[cfg(feature = "use_wgpu")]
pub mod display_scaler;
pub mod floppy_manager;
pub mod machine_manager;
pub mod resource_manager;
pub mod rom_manager;
pub mod timestep_manager;
pub mod types;
pub mod vhd_manager;

pub type FileTreeNode = resource_manager::tree::TreeNode;
pub type MartyGuiTheme = types::gui::MartyGuiTheme;
pub type HotkeyEvent = types::hotkeys::HotkeyEvent;
pub type HotkeyScope = types::hotkeys::HotkeyScope;
pub type HotkeyConfigEntry = types::hotkeys::HotkeyConfigEntry;

#[derive(Copy, Clone, Debug, Default, PartialEq, Deserialize)]
pub enum BenchmarkEndCondition {
    #[default]
    Cycles,
    Timeout,
    Trigger,
}

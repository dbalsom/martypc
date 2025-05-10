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

//! Process events received from the emulator GUI.
//! Typically, the GUI is implemented by the `marty_egui` crate.

use std::{
    ffi::OsString,
    io::Cursor,
    mem::discriminant,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use crate::{emulator, emulator::Emulator, gui::GuiState};

use marty_core::{
    breakpoints::BreakPointType,
    cpu_common,
    cpu_common::{Cpu, CpuOption, Register16},
    device_traits::videocard::ClockingMode,
    device_types::fdc::FloppyImageType,
    machine::{MachineOption, MachineState},
    vhd,
    vhd::VirtualHardDisk,
};
use marty_egui_common::GuiCommonEvent;
use marty_frontend_common::{
    constants::{LONG_NOTIFICATION_TIME, NORMAL_NOTIFICATION_TIME, SHORT_NOTIFICATION_TIME},
    floppy_manager::FloppyError,
    marty_common::types::ui::MouseCaptureMode,
    thread_events::{FileSelectionContext, FrontendThreadEvent},
    timestep_manager::{TimestepManager, TimestepUpdate},
    types::floppy::FloppyImageSource,
};

use anyhow::Error;
use winit::event_loop::ActiveEventLoop;

//noinspection RsBorrowChecker
pub fn handle_egui_event(
    emu: &mut Emulator,
    tm: &TimestepManager,
    tmu: &mut TimestepUpdate,
    gui_event: &GuiCommonEvent,
) {
    match gui_event {
        _ => {
            log::warn!("Unhandled GUI event: {:?}", discriminant(gui_event));
        }
    }
}

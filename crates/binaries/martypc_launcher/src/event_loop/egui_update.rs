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

    event_loop/egui_update

    Update the egui menu and widget state.
*/
use crate::{emulator::Emulator, event_loop::egui_events::handle_egui_event};

use marty_core::{
    bytequeue::ByteQueue,
    cpu_808x::Cpu,
    cpu_common,
    cpu_common::{CpuAddress, CpuOption, TraceMode},
    machine,
    syntax_token::SyntaxToken,
    util,
};
use marty_egui::GuiWindow;
use marty_frontend_common::timestep_manager::{TimestepManager, TimestepUpdate};

pub fn update_egui(emu: &mut Emulator, tm: &TimestepManager, tmu: &mut TimestepUpdate) {
    // Is the machine in an error state? If so, display an error dialog.
    if let Some(err) = emu.machine.get_error_str() {
        emu.gui.show_error(err);
        emu.gui.show_window(GuiWindow::DisassemblyViewer);
    }
    else {
        // No error? Make sure we close the error dialog.
        emu.gui.clear_error();
    }

    // Handle custom events received from our GUI
    loop {
        if let Some(gui_event) = emu.gui.get_event() {
            //log::warn!("Handling GUI event!");
            handle_egui_event(emu, tm, tmu, &gui_event);
        }
        else {
            break;
        }
    }
}

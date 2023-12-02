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

    event_loop/render_frame.rs

    Handle rendering of video targets at the end of event processing.
*/

use crate::Emulator;
use display_backend_pixels::DisplayBackend;
use display_manager_wgpu::DisplayManager;

pub fn render_frame(emu: &mut Emulator) {
    // First, run each renderer to resolve all videocard views.
    // Every renderer will have an associated card and backend.
    emu.dm.for_each_renderer(|renderer, vid, backend_buf| {
        if let Some(videocard) = emu.machine.bus_mut().video_mut(&vid) {
            //log::debug!("Drawing renderer for vid: {:?}", vid);
            renderer.draw(
                videocard.get_display_buf(),
                backend_buf,
                videocard.get_display_extents(),
                false,
                None,
            )
        }
    });

    // Prepare guis for rendering.
    emu.dm.for_each_gui(|gui, window| gui.prepare(window, &mut emu.gui));

    // Next, render each backend
    emu.dm.for_each_backend(|backend, scaler, gui_opt| {
        backend.render(Some(scaler), gui_opt);
    });
}

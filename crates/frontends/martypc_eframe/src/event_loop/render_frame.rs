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

    event_loop/render_frame.rs

    Handle rendering of video targets at the end of event processing.
*/
use crate::emulator::Emulator;
use std::sync::Arc;

//use display_backend_eframe::DisplayBackend;
use display_manager_eframe::{DisplayBackend, DisplayManager, EFrameDisplayManager};
use marty_core::{device_traits::videocard::BufferSelect, machine::ExecutionState};
use marty_egui::GuiBoolean;

pub fn render_frame(emu: &mut Emulator, dm: &mut EFrameDisplayManager) {
    // First, run each renderer to resolve all videocard views.
    // Every renderer will have an associated card and backend.
    dm.for_each_renderer(|renderer, vid, backend_buf| {
        if let Some(videocard) = emu.machine.bus_mut().video_mut(&vid) {
            // Check if the emulator is paused - if paused, optionally select the back buffer
            // so we can watch the raster beam draw
            let mut beam_pos = None;
            match emu.exec_control.borrow_mut().get_state() {
                ExecutionState::Paused | ExecutionState::BreakpointHit | ExecutionState::Halted => {
                    if emu.gui.get_option(GuiBoolean::ShowBackBuffer).unwrap_or(false) {
                        renderer.select_buffer(BufferSelect::Back);
                        if emu.gui.get_option(GuiBoolean::ShowRasterPosition).unwrap_or(false) {
                            beam_pos = videocard.get_beam_pos();
                        }
                    }
                    else {
                        renderer.select_buffer(BufferSelect::Front);
                    }
                }
                _ => {
                    renderer.select_buffer(BufferSelect::Front);
                }
            }

            let extents = videocard.get_display_extents();

            // Update mode byte.
            if renderer.get_mode_byte() != extents.mode_byte {
                // Mode byte has changed, recalculate composite parameters
                renderer.cga_direct_mode_update(extents.mode_byte);
                renderer.set_mode_byte(extents.mode_byte);
            }

            //log::debug!("Drawing renderer for vid: {:?}", vid);
            renderer.draw(
                videocard.get_buf(renderer.get_selected_buffer()),
                backend_buf,
                extents,
                beam_pos,
                videocard.get_palette(),
            )
        }
    });

    // Don't need this as eframe does not host guis ...
    // Prepare guis for rendering.
    //emu.dm.for_each_gui(|gui, window| gui.prepare(window, &mut emu.gui));

    // eframe should handle this for us
    // // Inform window manager that we are about to present
    // emu.dm.for_each_viewport(|window, _on_top| {
    //     window.pre_present_notify();
    //     None
    // });

    // Finally, render each surface

    dm.for_each_surface(None, |backend, surface, scaler, gui_opt| {
        // log::debug!(
        //     "Rendering surface. Scaler? {} Gui? {}",
        //     scaler.is_some(),
        //     gui_opt.is_some()
        // );
        // if let Err(e) = backend.render(surface, scaler, None) {
        //     log::error!("Failed to render backend: {}", e);
        // }

        let device = backend.device();
        let queue = backend.queue();

        _ = surface.write().unwrap().update_backing(device, queue);
    });
}

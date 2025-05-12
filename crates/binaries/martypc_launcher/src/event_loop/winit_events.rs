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

use crate::{
    emulator::Emulator,
    event_loop::winit_keyboard::{handle_modifiers, handle_winit_key_event},
};
use marty_frontend_common::timestep_manager::TimestepManager;

use winit::{event::WindowEvent, window::WindowId};

pub fn handle_window_event(
    emu: &mut Emulator,
    ctx: egui::Context,
    _tm: &mut TimestepManager,
    window_id: WindowId,
    event: WindowEvent,
    window_has_focus: bool,
    gui_has_focus: bool,
) {
    let mut pass_to_egui = false;

    //log::debug!("Handling WindowEvent, gui_has_focus: {}", gui_has_focus);

    match event {
        // WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
        //     log::debug!("Got ScaleFactorChanged: {}", scale_factor);
        //     dm.with_target_by_wid(window_id, |dt| {
        //         log::debug!("Setting new scale factor: {}", scale_factor);
        //         dt.set_scale_factor(scale_factor);
        //     });
        // }
        // WindowEvent::Resized(size) => {
        //     // let event_window = dm
        //     //     .viewport_by_id(window_id)
        //     //     .expect(&format!("Couldn't resolve window id {:?} to window.", window_id));
        //     //
        //     // let is_fullscreen = match event_window.fullscreen() {
        //     //     Some(_) => {
        //     //         log::debug!("Resize event received while in fullscreen mode.");
        //     //         true
        //     //     }
        //     //     None => false,
        //     // };
        //
        //     // wgpu under macOS and Intel graphics has a bug where it will draw stripes instead of the
        //     // shader clear color when in fullscreen. The workaround is to create the surface at 1 pixel
        //     // smaller than fullscreen.
        //     let (adjust_x, adjust_y) = if is_fullscreen && emu.config.emulator.backend.macos_stripe_fix {
        //         log::debug!("Adjusting for macOS stripe fix.");
        //         (1, 1)
        //     }
        //     else {
        //         (0, 0)
        //     };
        //
        //     if size.width > 0 && size.height > 0 {
        //         if let Err(e) = dm.on_viewport_resized(
        //             window_id,
        //             size.width.saturating_sub(adjust_x),
        //             size.height.saturating_sub(adjust_y),
        //         ) {
        //             log::error!("Failed to resize window: {}", e);
        //         }
        //     }
        //     else {
        //         log::debug!("Ignoring invalid size: {:?}", size);
        //         return;
        //     }
        // }
        WindowEvent::CloseRequested => {
            log::debug!("Close requested!");
            //elwt.exit();
            return;
        }
        WindowEvent::ModifiersChanged(modifiers) => {
            handle_modifiers(emu, window_id, &event, &modifiers);
            pass_to_egui = true;
        }
        WindowEvent::KeyboardInput {
            event: ref key_event, ..
        } => {
            if !window_has_focus {
                return;
            }
            pass_to_egui = !handle_winit_key_event(emu, ctx, window_id, key_event, gui_has_focus);
        }
        WindowEvent::Focused(state) => match state {
            true => {
                //log::debug!("Window {:?} gained focus", window_id);
                // dm.for_each_target(|dtc, _| {
                //     if dtc.window_opts.as_ref().is_some_and(|opts| opts.always_on_top) {
                //         dtc.window.as_ref().map(|window| {
                //             window.set_window_level(WindowLevel::AlwaysOnTop);
                //         });
                //         dtc.set_on_top(true);
                //     }
                // });
            }
            false => {
                //log::debug!("Window {:?} lost focus", window_id);
                // dm.for_each_viewport(|window, on_top| {
                //     if on_top {
                //         window.set_window_level(WindowLevel::Normal);
                //     }
                //     Some(false)
                // });
            }
        },
        _ => {
            pass_to_egui = true;
        }
    }

    // Pass any unhandled events to egui for handling.
    if pass_to_egui {
        // dm.with_gui_by_wid(window_id, |gui, window| {
        //     //log::debug!("Passing event to egui: {:?}", &event);
        //     gui.handle_event(window, &event)
        // });
    }
}

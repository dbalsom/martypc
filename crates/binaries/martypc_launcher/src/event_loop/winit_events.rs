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

    event_loop/winit_events.rs

    Process received winit events.
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

// pub fn handle_event(emu: &mut Emulator, dm: &mut EFrameDisplayManager, tm: &mut TimestepManager, event: WindowEvent) {
//     match event {
//         Event::NewEvents(StartCause::Init) => {
//             log::debug!("StartCause::Init");
//
//             // Initialization stuff here
//             emu.stat_counter.last_second = Instant::now();
//
//             // -- Update display info
//             //let dti = dm.display_info(&emu.machine);
//             //emu.gui.init_display_info(dti);
//         }
//
//         Event::DeviceEvent { event, .. } => {
//             match event {
//                 DeviceEvent::MouseMotion { delta: (x, y) } => {
//                     // We can get a lot more mouse updates than we want to send to the virtual mouse,
//                     // so add up all deltas between each mouse polling period
//                     emu.mouse_data.have_update = true;
//                     emu.mouse_data.frame_delta_x += x;
//                     emu.mouse_data.frame_delta_y += y;
//                 }
//                 DeviceEvent::Button { button, state } => {
//                     // Button ID is a raw u32. It appears that the id's for relative buttons are not consistent
//                     // across platforms. 1 == left button on windows, 3 == left button on macOS. So we resolve
//                     // button ids to button enums based on platform. There is a config option to override button
//                     // order.
//
//                     // Resolve the winit button id to a button enum based on platform and reverse flag.
//                     //log::debug!("Button: {:?} State: {:?}", button, state);
//                     let mbutton = button_from_id(button, emu.mouse_data.reverse_buttons);
//
//                     // A mouse click could be faster than one frame (pressed & released in 16.6ms), therefore mouse
//                     // clicks are 'sticky', if a button was pressed during the last update period it will be sent as
//                     // pressed during virtual mouse update.
//
//                     match (mbutton, state) {
//                         (MouseButton::Left, ElementState::Pressed) => {
//                             emu.mouse_data.l_button_was_pressed = true;
//                             emu.mouse_data.l_button_is_pressed = true;
//                             emu.mouse_data.have_update = true;
//                         }
//                         (MouseButton::Left, ElementState::Released) => {
//                             emu.mouse_data.l_button_is_pressed = false;
//                             emu.mouse_data.l_button_was_released = true;
//                             emu.mouse_data.have_update = true;
//                         }
//                         (MouseButton::Right, ElementState::Pressed) => {
//                             emu.mouse_data.r_button_was_pressed = true;
//                             emu.mouse_data.r_button_is_pressed = true;
//                             emu.mouse_data.have_update = true;
//                         }
//                         (MouseButton::Right, ElementState::Released) => {
//                             emu.mouse_data.r_button_is_pressed = false;
//                             emu.mouse_data.r_button_was_released = true;
//                             emu.mouse_data.have_update = true;
//                         }
//                         _ => {}
//                     }
//                     //log::debug!("Mouse button: {:?} state: {:?}", button, state);
//                 }
//                 _ => {}
//             }
//         }
//         Event::WindowEvent { window_id, event, .. } => {
//             let mut pass_to_egui = false;
//             match event {
//                 WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
//                     log::debug!("Got ScaleFactorChanged: {}", scale_factor);
//                     dm.with_target_by_wid(window_id, |dt| {
//                         log::debug!("Setting new scale factor: {}", scale_factor);
//                         dt.set_scale_factor(scale_factor);
//                     });
//                 }
//                 WindowEvent::Resized(size) => {
//                     let event_window = dm
//                         .viewport_by_id(window_id)
//                         .expect(&format!("Couldn't resolve window id {:?} to window.", window_id));
//
//                     let is_fullscreen = match event_window.fullscreen() {
//                         Some(_) => {
//                             log::debug!("Resize event received while in fullscreen mode.");
//                             true
//                         }
//                         None => false,
//                     };
//
//                     // wgpu under macOS and Intel graphics has a bug where it will draw stripes instead of the
//                     // shader clear color when in fullscreen. The workaround is to create the surface at 1 pixel
//                     // smaller than fullscreen.
//                     let (adjust_x, adjust_y) = if is_fullscreen && emu.config.emulator.backend.macos_stripe_fix {
//                         log::debug!("Adjusting for macOS stripe fix.");
//                         (1, 1)
//                     }
//                     else {
//                         (0, 0)
//                     };
//
//                     if size.width > 0 && size.height > 0 {
//                         if let Err(e) = dm.on_viewport_resized(
//                             window_id,
//                             size.width.saturating_sub(adjust_x),
//                             size.height.saturating_sub(adjust_y),
//                         ) {
//                             log::error!("Failed to resize window: {}", e);
//                         }
//                     }
//                     else {
//                         log::debug!("Ignoring invalid size: {:?}", size);
//                         return;
//                     }
//                 }
//                 WindowEvent::CloseRequested => {
//                     log::debug!("Close requested!");
//                     //elwt.exit();
//                     return;
//                 }
//                 WindowEvent::ModifiersChanged(modifiers) => {
//                     handle_modifiers(emu, window_id, &event, &modifiers);
//                     pass_to_egui = true;
//                 }
//                 WindowEvent::KeyboardInput {
//                     event: ref key_event, ..
//                 } => {
//                     pass_to_egui = !handle_key_event(emu, window_id, key_event);
//                 }
//                 WindowEvent::Focused(state) => match state {
//                     true => {
//                         log::debug!("Window {:?} gained focus", window_id);
//                         // dm.for_each_target(|dtc, _| {
//                         //     if dtc.window_opts.as_ref().is_some_and(|opts| opts.always_on_top) {
//                         //         dtc.window.as_ref().map(|window| {
//                         //             window.set_window_level(WindowLevel::AlwaysOnTop);
//                         //         });
//                         //         dtc.set_on_top(true);
//                         //     }
//                         // });
//                     }
//                     false => {
//                         log::debug!("Window {:?} lost focus", window_id);
//                         // dm.for_each_viewport(|window, on_top| {
//                         //     if on_top {
//                         //         window.set_window_level(WindowLevel::Normal);
//                         //     }
//                         //     Some(false)
//                         // });
//                     }
//                 },
//                 _ => {
//                     pass_to_egui = true;
//                 }
//             }
//
//             // Pass any unhandled events to egui for handling.
//             if pass_to_egui {
//                 // dm.with_gui_by_wid(window_id, |gui, window| {
//                 //     //log::debug!("Passing event to egui: {:?}", &event);
//                 //     gui.handle_event(window, &event)
//                 // });
//             }
//         }
//         // AboutToWait used to be MainEventsCleared in previous versions of Winit.
//         // But unlike that event, in Winit 0.29.4, this event does not appear to be throttled,
//         // so can run millions of times per second. So we will instead request a redraw here and
//         // move emulator logic to RedrawRequested.
//         Event::AboutToWait => {
//             // Throttle updates to maximum of 1000Hz
//             //std::thread::sleep(Duration::from_millis(1));
//             // emu.dm.for_each_viewport(|window, _on_top| {
//             //     window.request_redraw();
//             //     None
//             // });
//         }
//         _ => (),
//     }
// }

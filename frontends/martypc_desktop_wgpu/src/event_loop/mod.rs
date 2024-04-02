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

    event_loop/mod.rs

    Main winit event handler. This handler is repeatedly called by the winit
    event loop closure. The event handler is split into a few different
    functionality domains for readability as it was originally very long.
*/

mod egui_events;
mod egui_update;
mod keyboard;
mod render_frame;
mod update;

use keyboard::handle_modifiers;

use std::time::Instant;
use winit::{
    event::{DeviceEvent, ElementState, Event, StartCause, WindowEvent},
    event_loop::EventLoopWindowTarget,
    window::WindowLevel,
};

use crate::{
    event_loop::{keyboard::handle_key_event, update::process_update},
    input::*,
    Emulator,
};
use display_manager_wgpu::DisplayManager;
use frontend_common::timestep_manager::TimestepManager;

pub fn handle_event(emu: &mut Emulator, tm: &mut TimestepManager, event: Event<()>, elwt: &EventLoopWindowTarget<()>) {
    match event {
        Event::NewEvents(StartCause::Init) => {
            // Initialization stuff here
            emu.stat_counter.last_second = Instant::now();

            // -- Update display info
            let dti = emu.dm.get_display_info(&emu.machine);
            emu.gui.init_display_info(dti);
        }

        Event::DeviceEvent { event, .. } => {
            match event {
                DeviceEvent::MouseMotion { delta: (x, y) } => {
                    // We can get a lot more mouse updates than we want to send to the virtual mouse,
                    // so add up all deltas between each mouse polling period
                    emu.mouse_data.have_update = true;
                    emu.mouse_data.frame_delta_x += x;
                    emu.mouse_data.frame_delta_y += y;
                }
                DeviceEvent::Button { button, state } => {
                    // Button ID is a raw u32. It appears that the id's for relative buttons are not consistent
                    // across platforms. 1 == left button on windows, 3 == left button on macOS. So we resolve
                    // button ids to button enums based on platform. There is a config option to override button
                    // order.

                    // Resolve the winit button id to a button enum based on platform and reverse flag.
                    //log::debug!("Button: {:?} State: {:?}", button, state);
                    let mbutton = button_from_id(button, emu.mouse_data.reverse_buttons);

                    // A mouse click could be faster than one frame (pressed & released in 16.6ms), therefore mouse
                    // clicks are 'sticky', if a button was pressed during the last update period it will be sent as
                    // pressed during virtual mouse update.

                    match (mbutton, state) {
                        (MouseButton::Left, ElementState::Pressed) => {
                            emu.mouse_data.l_button_was_pressed = true;
                            emu.mouse_data.l_button_is_pressed = true;
                            emu.mouse_data.have_update = true;
                        }
                        (MouseButton::Left, ElementState::Released) => {
                            emu.mouse_data.l_button_is_pressed = false;
                            emu.mouse_data.l_button_was_released = true;
                            emu.mouse_data.have_update = true;
                        }
                        (MouseButton::Right, ElementState::Pressed) => {
                            emu.mouse_data.r_button_was_pressed = true;
                            emu.mouse_data.r_button_is_pressed = true;
                            emu.mouse_data.have_update = true;
                        }
                        (MouseButton::Right, ElementState::Released) => {
                            emu.mouse_data.r_button_is_pressed = false;
                            emu.mouse_data.r_button_was_released = true;
                            emu.mouse_data.have_update = true;
                        }
                        _ => {}
                    }
                    //log::debug!("Mouse button: {:?} state: {:?}", button, state);
                }
                _ => {}
            }
        }
        Event::WindowEvent { window_id, event, .. } => {
            let mut pass_to_egui = false;
            match event {
                WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                    log::debug!("Got ScaleFactorChanged: {}", scale_factor);
                    emu.dm.with_target_by_wid(window_id, |dt| {
                        log::debug!("Setting new scale factor: {}", scale_factor);
                        dt.set_scale_factor(scale_factor);
                    });
                }
                WindowEvent::Resized(size) => {
                    if size.width > 0 && size.height > 0 {
                        if let Err(e) = emu.dm.on_window_resized(window_id, size.width, size.height) {
                            log::error!("Failed to resize window: {}", e);
                        }
                    }
                    else {
                        log::debug!("Ignoring invalid size: {:?}", size);
                        return;
                    }
                }
                WindowEvent::CloseRequested => {
                    elwt.exit();
                    return;
                }
                WindowEvent::ModifiersChanged(modifiers) => {
                    handle_modifiers(emu, window_id, &event, &modifiers);
                    pass_to_egui = true;
                }
                WindowEvent::KeyboardInput {
                    event: ref key_event, ..
                } => {
                    pass_to_egui = !handle_key_event(emu, window_id, key_event);
                }
                WindowEvent::RedrawRequested => {
                    process_update(emu, tm, elwt);
                }
                WindowEvent::Focused(state) => match state {
                    true => {
                        log::debug!("Window {:?} gained focus", window_id);
                        emu.dm.for_each_target(|dtc, _| {
                            if dtc.window_opts.as_ref().is_some_and(|opts| opts.always_on_top) {
                                dtc.window.as_ref().map(|window| {
                                    window.set_window_level(WindowLevel::AlwaysOnTop);
                                });
                                dtc.set_on_top(true);
                            }
                        });
                    }
                    false => {
                        log::debug!("Window {:?} lost focus", window_id);
                        emu.dm.for_each_window(|window, on_top| {
                            if on_top {
                                window.set_window_level(WindowLevel::Normal);
                            }
                            Some(false)
                        });
                    }
                },
                _ => {
                    pass_to_egui = true;
                }
            }

            // Pass any unhandled events to egui for handling.
            if pass_to_egui {
                emu.dm.with_gui_by_wid(window_id, |gui, window| {
                    //log::debug!("Passing event to egui: {:?}", &event);
                    gui.handle_event(window, &event)
                });
            }
        }
        // AboutToWait used to be MainEventsCleared in previous versions of Winit.
        // But unlike that event, in Winit 0.29.4, this event does not appear to be throttled,
        // so can run millions of times per second. So we will instead request a redraw here and
        // move emulator logic to RedrawRequested.
        Event::AboutToWait => {
            // Throttle updates to maximum of 1000Hz
            //std::thread::sleep(Duration::from_millis(1));
            emu.dm.for_each_window(|window, _on_top| {
                window.request_redraw();
                None
            });
        }
        _ => (),
    }
}

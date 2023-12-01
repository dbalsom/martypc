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

    event_loop/keyboard.rs

    Handle keyboard events.

*/

use winit::{
    event::{
        ElementState,
        KeyEvent,
        Modifiers,
        WindowEvent
    }
};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use display_manager_wgpu::DisplayManager;

use crate::input::TranslateKey;
use crate::{Emulator};


pub fn handle_modifiers(emu: &mut Emulator, event: &WindowEvent, modifiers: &Modifiers) {
    let state = modifiers.state();

    emu.kb_data.ctrl_pressed = state.control_key();
    emu.kb_data.modifiers.control = state.control_key();
    emu.kb_data.modifiers.alt = state.alt_key();
    emu.kb_data.modifiers.shift = state.shift_key();
    emu.kb_data.modifiers.meta = state.super_key();
    if let Some(gui) = emu.dm.get_main_gui_mut() {
        gui.handle_event(event)
    }
}

pub fn handle_key_event(
    emu: &mut Emulator,
    window_id: WindowId,
    key_event: &KeyEvent) -> bool
{
    // Destructure the KeyEvent.
    let KeyEvent {
        physical_key,
        state,
        repeat,
        ..
    } = key_event;

    if !repeat && emu.flags.debug_keyboard {
        println!("{:?}", key_event);
    }

    // Winit 0.29.2 changed the type returned by KeyEvent from KeyCode to PhysicalKey, which wraps
    // a KeyCode or Unknown. We will just handle KeyCodes here and print a debug warning on Unknown.

    // Determine if a GUI widget has focus.
    // TODO: This will only check the main window(?)
    let gui_has_focus = {
        emu.dm.get_main_gui_mut().map_or(false, |gui| gui.has_focus())
    };

    // Get the window for this event.
    let event_window =
        emu.dm
            .get_window_by_id(window_id)
            .expect(
                &format!("Couldn't resolve window id {:?} to window.", window_id)
            );

    match (physical_key, gui_has_focus) {
        (PhysicalKey::Code(keycode), gui_focus) => {
            // An egui widget doesn't have focus, so send an event to the emulated machine

            //handle_hotkey(emu, keycode);

            // Match global hotkeys regardless of egui focus
            match (state, keycode) {
                (winit::event::ElementState::Pressed, KeyCode::F1) => {
                    if emu.kb_data.ctrl_pressed {
                        log::info!("Control F1 pressed. Toggling egui state.");
                        emu.flags.render_gui = !emu.flags.render_gui;
                    }
                },
                (winit::event::ElementState::Pressed, KeyCode::F10) => {
                    if emu.kb_data.ctrl_pressed {
                        // Ctrl-F10 pressed. Toggle mouse capture.
                        log::info!("Control F10 pressed. Capturing mouse cursor.");
                        if !emu.mouse_data.is_captured {
                            let mut grab_success = false;


                            match event_window.set_cursor_grab(winit::window::CursorGrabMode::Confined) {
                                Ok(_) => {
                                    emu.mouse_data.is_captured = true;
                                    grab_success = true;
                                }
                                Err(_) => {
                                    // Try alternate grab mode (Windows/Mac require opposite modes)
                                    match event_window.set_cursor_grab(winit::window::CursorGrabMode::Locked) {
                                        Ok(_) => {
                                            emu.mouse_data.is_captured = true;
                                            grab_success = true;
                                        }
                                        Err(e) => log::error!("Couldn't set cursor grab mode: {:?}", e)
                                    }
                                }
                            }
                            // Hide mouse cursor if grab successful
                            if grab_success {
                                event_window.set_cursor_visible(false);
                            }
                        } else {
                            // Cursor is grabbed, ungrab
                            match event_window.set_cursor_grab(winit::window::CursorGrabMode::None) {
                                Ok(_) => emu.mouse_data.is_captured = false,
                                Err(e) => log::error!("Couldn't set cursor grab mode: {:?}", e)
                            }
                            event_window.set_cursor_visible(true);
                        }
                    }
                }

                _ => {}
            }

            match gui_focus {
                true => {
                    if emu.flags.debug_keyboard {
                        println!("Keyboard event sent to framework.");
                    }
                    // Inidicate caller should pass event to egui.
                    return false;
                }
                false => {
                    // egui does not have focus - send keystroke to machine
                    // TODO: widgets seems to lose focus before 'enter' is processed in a text entry,
                    //       passing the enter keycode to the emulator

                    // ignore host typematic repeat
                    if !repeat {
                        match state {
                            ElementState::Pressed => {
                                emu.machine.key_press(keycode.to_internal(), emu.kb_data.modifiers);
                                if emu.flags.debug_keyboard {
                                    println!("Key pressed: {:?}", keycode);
                                    //log::debug!("Key pressed, keycode: {:?}: xt: {:02X}", keycode, keycode);
                                }
                                return true;
                            },
                            ElementState::Released => {
                                emu.machine.key_release(keycode.to_internal());
                                return true;
                            }
                        }
                    }
                }
            }
        }
        (PhysicalKey::Unidentified(keycode), _ ) => {
            log::warn!("Unidentified keycode: {:?}", keycode);
            return false; // Send it along in case egui knows what to do with it.
        }
    }

    return false;
}
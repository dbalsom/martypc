/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

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
use crate::{emulator::Emulator, event_loop::hotkeys::process_hotkeys, input::TranslateKey};

use display_manager_eframe::EFrameDisplayManager;
use marty_frontend_common::types::joykeys::JoyKeyInput;

use winit::{
    event::{ElementState, KeyEvent, Modifiers, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowId,
};

pub fn handle_modifiers(emu: &mut Emulator, _wid: WindowId, _event: &WindowEvent, modifiers: &Modifiers) {
    let state = modifiers.state();

    emu.kb_data.ctrl_pressed = state.control_key();
    emu.kb_data.modifiers.control = state.control_key();
    emu.kb_data.modifiers.alt = state.alt_key();
    emu.kb_data.modifiers.shift = state.shift_key();
    emu.kb_data.modifiers.meta = state.super_key();

    // emu.dm
    //     .with_gui_by_wid(wid, |gui, window| gui.handle_event(window, event));
}

/// Handle a KeyEvent from Winit. Return true if the event is handled; otherwise returns false
/// to indicate that the event should be forwarded to the immediate-mode GUI for processing.
pub fn handle_winit_key_event(
    emu: &mut Emulator,
    dm: &mut EFrameDisplayManager,
    context: egui::Context,
    window_id: WindowId,
    key_event: &KeyEvent,
    gui_has_focus: bool,
) -> bool {
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
    // let gui_has_focus = emu
    //     .dm
    //     .get_gui_by_window_id(window_id)
    //     .map_or(false, |gui| gui.has_focus());

    match (physical_key, gui_has_focus) {
        (PhysicalKey::Code(keycode), gui_focus) => {
            // An egui widget doesn't have focus, so send an event to the emulated machine

            process_hotkeys(
                emu,
                dm,
                context,
                keycode.to_internal(),
                matches!(state, ElementState::Pressed),
                gui_focus,
            );

            if let Some(controller_slot) = emu.gi.joykey_mapping() {
                log::debug!("Got joykey mapping: {:?}", controller_slot);
                if process_joykeys(emu, *keycode, matches!(state, ElementState::Pressed), controller_slot) {
                    return true;
                }
            }

            // Get the window for this event.
            // let _event_window = emu
            //     .dm
            //     .viewport_by_id(window_id)
            //     .expect(&format!("Couldn't resolve window id {:?} to window.", window_id));

            match gui_focus {
                true => {
                    if emu.flags.debug_keyboard {
                        println!("Keyboard event sent to framework.");
                    }
                    // Indicate caller should pass event to egui.
                    return false;
                }
                false => {
                    // egui does not have focus - send keystroke to machine
                    // TODO: widgets seems to lose focus before 'enter' is processed in a text entry,
                    //       passing the enter keycode to the emulator

                    // Only send keystrokes to the machine if it is running. This avoids sending keystrokes
                    // to the machine when interacting with the debugger.
                    // TODO: Make this optional?
                    if emu.exec_control.borrow_mut().get_state().is_running() {
                        // ignore host typematic repeat
                        if !repeat {
                            return match state {
                                ElementState::Pressed => {
                                    emu.machine.key_press(keycode.to_internal(), emu.kb_data.modifiers);
                                    if emu.flags.debug_keyboard {
                                        println!("Window: {:?} Key pressed: {:?}", window_id, keycode);
                                        //log::debug!("Key pressed, keycode: {:?}: xt: {:02X}", keycode, keycode);
                                    }
                                    true
                                }
                                ElementState::Released => {
                                    emu.machine.key_release(keycode.to_internal());
                                    if emu.flags.debug_keyboard {
                                        println!("Window: {:?} Key released: {:?}", window_id, keycode);
                                    }
                                    true
                                }
                            };
                        }
                    }
                }
            }
        }
        (PhysicalKey::Unidentified(keycode), _) => {
            log::warn!("Unidentified keycode: {:?}", keycode);
            return false; // Send it along in case egui knows what to do with it.
        }
    }

    false
}

/// Process keys for joystick emulation, if enabled. Returns true if the key was processed.
/// Processed keys should not be sent on to the emulator.
#[allow(unreachable_patterns)]
pub fn process_joykeys(emu: &mut Emulator, keycode: KeyCode, pressed: bool, controller: usize) -> bool {
    let martykey = keycode.to_internal();

    let mut joykey = None;
    emu.joy_data.key_state.entry(martykey).and_modify(|v| {
        joykey = Some(v.0);
        emu.joy_data.joy_state.entry(v.0).and_modify(|k| {
            *k = pressed;
        });
        v.1 = pressed
    });

    if let Some(key) = joykey {
        if let Some(gameport) = emu.machine.bus_mut().game_port_mut() {
            match key {
                JoyKeyInput::JoyButton1 => {
                    log::debug!("process_joykeys(): JoyButton1 pressed");
                    gameport.set_button(controller, 0, pressed);
                }
                JoyKeyInput::JoyButton2 => {
                    log::debug!("process_joykeys(): JoyButton2 pressed");
                    gameport.set_button(controller, 1, pressed);
                }
                _ => {
                    // Update the stick position
                    log::debug!("process_joykeys(): Stick input");
                    let (x, y) = emu.joy_data.get_xy();
                    gameport.set_stick_pos(controller, 0, Some(x), Some(y));
                }
            }
        }
    }

    joykey.is_some()
}

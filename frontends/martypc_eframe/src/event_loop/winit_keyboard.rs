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

    event_loop/keyboard.rs

    Handle keyboard events.

*/
use std::str::FromStr;

use crate::{emulator::Emulator, input::TranslateKey};

use display_manager_eframe::{DisplayManager, EFrameDisplayManager};
use frontend_common::{
    constants::LONG_NOTIFICATION_TIME,
    display_manager::DtHandle,
    types::joykeys::JoyKeyInput,
    HotkeyEvent,
};
use marty_core::{
    keys::MartyKey,
    machine::{ExecutionOperation, MachineState},
};

use eframe::WebKeyboardEvent;
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
                *keycode,
                matches!(state, ElementState::Pressed),
                window_id,
                gui_focus,
            );

            if process_joykeys(
                emu,
                *keycode,
                matches!(state, ElementState::Pressed),
                window_id,
                gui_focus,
            ) {
                return true;
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
        (PhysicalKey::Unidentified(keycode), _) => {
            log::warn!("Unidentified keycode: {:?}", keycode);
            return false; // Send it along in case egui knows what to do with it.
        }
    }

    false
}

#[allow(unreachable_patterns)]
pub fn process_hotkeys(
    emu: &mut Emulator,
    dm: &mut EFrameDisplayManager,
    keycode: KeyCode,
    pressed: bool,
    window_id: WindowId,
    gui_focus: bool,
) {
    let mut event_opt = None;
    if pressed {
        event_opt = emu
            .hkm
            .keydown(keycode.to_internal(), gui_focus, emu.mouse_data.is_captured);
    }
    else {
        emu.hkm.keyup(keycode.to_internal())
    }

    for hotkey in event_opt.unwrap_or_default().iter() {
        match hotkey {
            HotkeyEvent::ToggleGui => {
                log::debug!("ToggleGui hotkey triggered. Toggling GUI visibility.");
                emu.flags.render_gui = !emu.flags.render_gui;
            }
            // HotkeyEvent::CaptureMouse => {
            //     // Get the window for this event.
            //     let event_window = dm
            //         .viewport_by_id(window_id)
            //         .expect(&format!("Couldn't resolve window id {:?} to window.", window_id));
            //
            //     log::debug!("CaptureMouse hotkey triggered. Capturing mouse cursor.");
            //     if !emu.mouse_data.is_captured {
            //         let mut grab_success = false;
            //
            //         match event_window.set_cursor_grab(winit::window::CursorGrabMode::Confined) {
            //             Ok(_) => {
            //                 emu.mouse_data.is_captured = true;
            //                 grab_success = true;
            //             }
            //             Err(_) => {
            //                 // Try alternate grab mode (Windows/Mac require opposite modes)
            //                 match event_window.set_cursor_grab(winit::window::CursorGrabMode::Locked) {
            //                     Ok(_) => {
            //                         emu.mouse_data.is_captured = true;
            //                         grab_success = true;
            //                     }
            //                     Err(e) => {
            //                         log::error!("Couldn't set cursor grab mode: {:?}", e)
            //                     }
            //                 }
            //             }
            //         }
            //         // Hide mouse cursor if grab successful
            //         if grab_success {
            //             event_window.set_cursor_visible(false);
            //         }
            //     }
            //     else {
            //         // Cursor is grabbed, ungrab
            //         match event_window.set_cursor_grab(winit::window::CursorGrabMode::None) {
            //             Ok(_) => emu.mouse_data.is_captured = false,
            //             Err(e) => log::error!("Couldn't set cursor grab mode: {:?}", e),
            //         }
            //         event_window.set_cursor_visible(true);
            //     }
            // }
            HotkeyEvent::CtrlAltDel => {
                log::debug!("CtrlAltDel hotkey triggered. Sending Ctrl-Alt-Del to machine.");
                emu.machine.emit_ctrl_alt_del();
            }
            HotkeyEvent::Reboot => {
                log::debug!("Reboot hotkey triggered. Restarting machine.");
                emu.machine.change_state(MachineState::Rebooting);
            }
            // HotkeyEvent::ToggleFullscreen => {
            //     log::debug!("ToggleFullscreen hotkey triggered.");
            //     // Get the window for this event.
            //     let event_window = emu
            //         .dm
            //         .viewport_by_id(window_id)
            //         .expect(&format!("Couldn't resolve window id {:?} to window.", window_id));
            //
            //     match event_window.fullscreen() {
            //         Some(_) => {
            //             log::debug!("ToggleFullscreen: Resetting fullscreen state.");
            //             event_window.set_fullscreen(None);
            //         }
            //         None => {
            //             log::debug!("ToggleFullscreen: Entering fullscreen state.");
            //             event_window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
            //         }
            //     }
            // }
            HotkeyEvent::Screenshot => {
                log::debug!("Screenshot hotkey triggered. Capturing screenshot.");

                let screenshot_path = emu.rm.get_resource_path("screenshot").unwrap();

                // Take as screenshot of the primary display target.
                if let Err(err) = dm.save_screenshot(DtHandle::default(), screenshot_path) {
                    log::error!("Failed to save screenshot: {}", err);
                    emu.gui
                        .toasts()
                        .error(format!("{}", err))
                        .duration(Some(LONG_NOTIFICATION_TIME));
                }
            }
            HotkeyEvent::DebugStep => {
                emu.exec_control.borrow_mut().set_op(ExecutionOperation::Step);
            }
            HotkeyEvent::DebugStepOver => {
                emu.exec_control.borrow_mut().set_op(ExecutionOperation::StepOver);
            }
            HotkeyEvent::JoyToggle => {
                log::debug!("JoyToggle hotkey triggered. Toggling joystick keyboard emulation.");
                emu.joy_data.enabled = !emu.joy_data.enabled;
            }
            _ => {
                log::debug!("Unhandled Hotkey triggered: {:?}", hotkey);
            }
        }
    }
}

/// Process keys for joystick emulation, if enabled. Returns true if the key was processed.
/// Processed keys should not be sent on to the emulator.
#[allow(unreachable_patterns)]
pub fn process_joykeys(
    emu: &mut Emulator,
    keycode: KeyCode,
    pressed: bool,
    _window_id: WindowId,
    _gui_focus: bool,
) -> bool {
    if !emu.joy_data.enabled {
        return false;
    }
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
                    gameport.set_button(0, 0, pressed);
                }
                JoyKeyInput::JoyButton2 => {
                    gameport.set_button(0, 1, pressed);
                }
                _ => {
                    // Update the stick position
                    let (x, y) = emu.joy_data.get_xy();
                    gameport.set_stick_pos(0, 0, Some(x), Some(y));
                }
            }
        }
    }

    joykey.is_some()
}

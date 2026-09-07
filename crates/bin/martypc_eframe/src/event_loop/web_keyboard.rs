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
*/

//! Handle web keyboard events. eframe does not use winit for web targets, and its keyboard support
//! is insufficient for emulation purposes. We forked eframe to install a hook that sends us the
//! key code from the `web_sys::KeyboardEvent`, cloned into a [WebKeyboardEvent] struct for
//! Send + Sync.

use std::str::FromStr;

use crate::{emulator::Emulator, event_loop::hotkeys::process_hotkeys};
use display_manager_eframe::EFrameDisplayManager;
use eframe::WebKeyboardEvent;
use marty_common::types::keys::MartyKey;

/// Returns whether MartyPC should suppress the browser's default action for a raw key event.
pub fn should_prevent_default(event: &WebKeyboardEvent) -> bool {
    let code = event.key.as_str();
    let modifiers = event.modifiers;

    // Suppress every modified key combination. The browser reports the Windows key as Meta;
    // match the key itself as its modifier flag may not be set on the initial keydown event.
    if !modifiers.is_none() || matches!(code, "MetaLeft" | "MetaRight") {
        return true;
    }

    // Suppress browser actions for keys that need to reach the emulated machine unchanged.
    // F12 is intentionally left available for opening the browser developer tools.
    matches!(
        code,
        "Escape"
            | "Tab"
            | "Space"
            | "PageDown"
            | "PageUp"
            | "Home"
            | "End"
            | "F1"
            | "F2"
            | "F3"
            | "F4"
            | "F5"
            | "F6"
            | "F7"
            | "F9"
            | "F10"
            | "F11"
            | "Slash"
            | "Quote"
            | "PrintScreen"
    )
}

pub fn handle_web_key_event(
    emu: &mut Emulator,
    dm: &mut EFrameDisplayManager,
    ctx: egui::Context,
    event: WebKeyboardEvent,
    gui_focus: bool,
) {
    if let Ok(marty_key) = MartyKey::from_str(&event.key) {
        emu.kb_data.ctrl_pressed = event.modifiers.ctrl;
        emu.kb_data.modifiers.control = event.modifiers.ctrl;
        emu.kb_data.modifiers.alt = event.modifiers.alt;
        emu.kb_data.modifiers.shift = event.modifiers.shift;
        emu.kb_data.modifiers.meta = event.modifiers.mac_cmd;

        process_hotkeys(emu, dm, ctx, marty_key, event.pressed, gui_focus);

        if !gui_focus {
            if event.pressed {
                emu.machine.key_press(marty_key, emu.kb_data.modifiers);
            }
            else {
                emu.machine.key_release(marty_key);
            }
        }
    }
    else {
        log::warn!("Couldn't convert key: {} to MartyKey", event.key);
    }
}

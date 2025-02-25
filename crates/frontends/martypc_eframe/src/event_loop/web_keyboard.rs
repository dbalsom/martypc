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

//! Handle web keyboard events. eframe does not use winit for web targets, and its keyboard support
//! is insufficient for emulation purposes. We forked eframe to install a hook that sends us the
//! key code from the `web_sys::KeyboardEvent`, cloned into a [WebKeyboardEvent] struct for
//! Send + Sync.

use std::str::FromStr;

use crate::emulator::Emulator;
use display_manager_eframe::EFrameDisplayManager;
use eframe::WebKeyboardEvent;
use marty_core::keys::MartyKey;

pub fn handle_web_key_event(
    emu: &mut Emulator,
    _dm: &mut EFrameDisplayManager,
    event: WebKeyboardEvent,
    gui_focus: bool,
) {
    if let Ok(marty_key) = MartyKey::from_str(&event.key) {
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

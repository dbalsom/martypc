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

   ---------------------------------------------------------------------------

   frontend_common::types::hotkeys.rs

   Define frontend types for hotkeys.

*/

use std::fmt;

use marty_common::types::keys::MartyKey;
use serde_derive::Deserialize;
use strum_macros::EnumIter;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, EnumIter, Deserialize)]
pub enum HotkeyEvent {
    Quit,
    CaptureMouse,
    CtrlAltDel,
    Reboot,
    Screenshot,
    ToggleGui,
    ToggleFullscreen,
    DebugStep,
    DebugStepOver,
    JoyToggle,
    JoyButton1,
    JoyButton2,
    JoyUp,
    JoyLeft,
    JoyRight,
    JoyDown,
}

impl fmt::Display for HotkeyEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Quit => "Quit",
            Self::CaptureMouse => "Capture or release mouse",
            Self::CtrlAltDel => "Send Ctrl+Alt+Del",
            Self::Reboot => "Reboot",
            Self::Screenshot => "Take screenshot",
            Self::ToggleGui => "Toggle GUI",
            Self::ToggleFullscreen => "Toggle fullscreen",
            Self::DebugStep => "Debug step",
            Self::DebugStepOver => "Debug step over",
            Self::JoyToggle => "Toggle keyboard joystick",
            Self::JoyButton1 => "Joystick button 1",
            Self::JoyButton2 => "Joystick button 2",
            Self::JoyUp => "Joystick up",
            Self::JoyLeft => "Joystick left",
            Self::JoyRight => "Joystick right",
            Self::JoyDown => "Joystick down",
        };

        f.write_str(label)
    }
}

#[derive(Copy, Clone, Debug, Deserialize)]
pub enum HotkeyScope {
    Any,
    Gui,
    Machine,
    Captured,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HotkeyConfigEntry {
    pub event: HotkeyEvent,
    pub keys: Vec<MartyKey>,
    pub capture_disable: bool,
    pub scope: HotkeyScope,
}

#[cfg(test)]
mod tests {
    use super::HotkeyEvent;

    #[test]
    fn event_labels_are_user_facing() {
        assert_eq!(HotkeyEvent::CaptureMouse.to_string(), "Capture or release mouse");
        assert_eq!(HotkeyEvent::CtrlAltDel.to_string(), "Send Ctrl+Alt+Del");
        assert_eq!(HotkeyEvent::JoyButton1.to_string(), "Joystick button 1");
    }
}

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

//! Provide a [GamepadInterface] implementation that does not use any gamepad
//! input library. `Joykey` mappings are still supported.

use marty_frontend_common::types::gamepad::{GamepadId, GamepadInfo, JoystickMapping};

pub struct GamepadInterface {
    mapping: (Option<JoystickMapping>, Option<JoystickMapping>),
}

pub enum GamepadEvent {
    Connected(GamepadInfo),
    Disconnected(GamepadId),
    Event(()),
}

impl GamepadInterface {
    pub fn new(auto_connect: bool, deadzone: f32) -> Self {
        Self { mapping: (None, None) }
    }

    #[inline]
    pub fn gamepads(&self) -> impl Iterator<Item = GamepadInfo> + '_ {
        std::iter::empty()
    }

    #[inline]
    pub fn next_event(&mut self) -> Option<()> {
        None
    }

    #[inline]
    pub fn poll(&mut self) -> Vec<GamepadEvent> {
        Vec::new()
    }

    #[inline]
    pub fn mapping(&self) -> (Option<JoystickMapping>, Option<JoystickMapping>) {
        self.mapping
    }

    #[inline]
    pub fn set_mapping(&mut self, mapping: (Option<JoystickMapping>, Option<JoystickMapping>)) {
        self.mapping = mapping;
    }

    #[inline]
    pub fn select_id(&self, id: GamepadId) -> Option<usize> {
        None
    }

    #[inline]
    pub fn is_joykey(&self, slot: usize) -> bool {
        if slot == 0 {
            matches!(self.mapping.0, Some(JoystickMapping::JoyKeys))
        }
        else if slot == 1 {
            matches!(self.mapping.1, Some(JoystickMapping::JoyKeys))
        }
        else {
            false
        }
    }

    /// Return which joystick slot is mapped to a joykey mapping, or None
    /// if no joykey mapping is set.
    #[inline]
    pub fn joykey_mapping(&self) -> Option<usize> {
        if let Some(JoystickMapping::JoyKeys) = self.mapping.0 {
            Some(0)
        }
        else if let Some(JoystickMapping::JoyKeys) = self.mapping.1 {
            Some(1)
        }
        else {
            None
        }
    }

    pub fn toggle_joykeys(&mut self, slot: usize) -> bool {
        if slot == 0 {
            if self.mapping.0 == Some(JoystickMapping::JoyKeys) {
                self.mapping.0 = None;
                false
            }
            else {
                self.mapping.0 = Some(JoystickMapping::JoyKeys);
                if self.mapping.1 == Some(JoystickMapping::JoyKeys) {
                    self.mapping.1 = None;
                }
                true
            }
        }
        else if slot == 1 {
            if self.mapping.1 == Some(JoystickMapping::JoyKeys) {
                self.mapping.1 = None;
                false
            }
            else {
                self.mapping.1 = Some(JoystickMapping::JoyKeys);
                if self.mapping.0 == Some(JoystickMapping::JoyKeys) {
                    self.mapping.0 = None;
                }
                true
            }
        }
        else {
            false
        }
    }
}

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

use marty_frontend_common::types::gamepad::{GamepadId, GamepadInfo};

use gilrs::{Event, Gilrs};

pub struct GamepadInterface {
    gilrs:   Gilrs,
    mapping: (Option<GamepadId>, Option<GamepadId>),
}

impl GamepadInterface {
    pub fn new() -> Self {
        Self {
            gilrs:   Gilrs::new().unwrap(),
            mapping: (None, None),
        }
    }

    pub fn gamepads(&self) -> impl Iterator<Item = GamepadInfo> + '_ {
        self.gilrs.gamepads().map(|(id, gamepad)| GamepadInfo {
            id: id.to_string(),
            internal_id: id,
            name: gamepad.name().to_string(),
        })
    }

    pub fn next_event(&mut self) -> Option<Event> {
        self.gilrs.next_event()
    }

    pub fn set_mapping(&mut self, mapping: (Option<GamepadId>, Option<GamepadId>)) {
        self.mapping = mapping;
    }

    pub fn select_id(&self, id: GamepadId) -> Option<usize> {
        if Some(id) == self.mapping.0 {
            Some(0)
        }
        else if Some(id) == self.mapping.1 {
            Some(1)
        }
        else {
            None
        }
    }
}

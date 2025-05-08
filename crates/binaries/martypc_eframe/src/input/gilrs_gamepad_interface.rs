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

//! Provide a [GamepadInterface] implementation that uses the `gilrs` crate
//! for gamepad input.

use marty_frontend_common::types::gamepad::{GamepadId, GamepadInfo, JoystickMapping};

use gilrs::{Axis, Event, Gamepad, Gilrs};
use marty_frontend_common::marty_common::MartyHashMap;

pub struct GamepadInterface {
    gilrs: Gilrs,
    mapping: (Option<JoystickMapping>, Option<JoystickMapping>),
    gamepads: MartyHashMap<GamepadId, GamepadInfo>,
    auto_connect: bool,
    deadzone: f32,
}

pub enum GamepadEvent {
    Connected(GamepadInfo),
    Disconnected(GamepadId),
    Event(gilrs::Event),
}

impl GamepadInterface {
    pub fn new(auto_connect: bool, deadzone: f32) -> Self {
        log::debug!(
            "GamepadInterface: Auto connect: {} Deadzone: {}",
            auto_connect,
            deadzone
        );
        Self {
            gilrs: Gilrs::new().unwrap(),
            mapping: (None, None),
            gamepads: MartyHashMap::default(),
            auto_connect,
            deadzone,
        }
    }

    pub fn gamepads(&self) -> impl Iterator<Item = GamepadInfo> + '_ {
        self.gilrs.gamepads().filter_map(|(id, gamepad)| {
            if !Self::is_real_gamepad(&gamepad) {
                return None;
            }
            Some(GamepadInfo {
                id: id.to_string(),
                internal_id: id,
                name: gamepad.name().to_string(),
            })
        })
    }

    #[inline]
    pub fn deadzone(&self) -> f32 {
        self.deadzone
    }

    #[inline]
    pub fn next_event(&mut self) -> Option<Event> {
        self.gilrs.next_event()
    }

    pub fn poll(&mut self) -> Vec<GamepadEvent> {
        let mut events = Vec::new();

        while let Some(ev) = self.gilrs.next_event() {
            use gilrs::EventType::*;

            match ev.event {
                Connected => {
                    let id = ev.id;
                    let gamepad = self.gilrs.gamepad(id);

                    if !Self::is_real_gamepad(&gamepad) {
                        continue;
                    }

                    let info = GamepadInfo {
                        id: id.to_string(),
                        internal_id: id,
                        name: gamepad.name().to_string(),
                    };
                    self.gamepads.insert(id, info.clone());

                    if self.auto_connect {
                        self.auto_connect(ev.id);
                    }

                    events.push(GamepadEvent::Connected(info));
                }
                Disconnected => {
                    if self.gamepads.remove(&ev.id).is_some() {
                        events.push(GamepadEvent::Disconnected(ev.id));
                    }
                    if self.mapping.0 == Some(JoystickMapping::Gamepad(ev.id)) {
                        self.mapping.0 = None;
                    }
                    if self.mapping.1 == Some(JoystickMapping::Gamepad(ev.id)) {
                        self.mapping.1 = None;
                    }
                }
                _ => {
                    if self.auto_connect {
                        self.auto_connect(ev.id);
                    }
                    events.push(GamepadEvent::Event(ev));
                }
            }
        }

        events
    }

    /// Connect the specified gamepad into the first available mapping slot.
    /// This will be slot 0 if it is empty, or slot 1 if slot 1 is empty and the id is not already
    /// mapped to slot 0.
    fn auto_connect(&mut self, id: GamepadId) {
        if self.mapping.0.is_none() {
            log::debug!("Auto-connecting gamepad {} to joystick slot 0", id);
            self.mapping.0 = Some(JoystickMapping::Gamepad(id));
        }
        else if let Some(JoystickMapping::Gamepad(id_0)) = self.mapping.0 {
            if id != id_0 && self.mapping.1.is_none() {
                log::debug!("Auto-connecting gamepad {} to joystick slot 1", id);
                self.mapping.1 = Some(JoystickMapping::Gamepad(id));
            }
        }
    }

    fn is_real_gamepad(gamepad: &Gamepad<'_>) -> bool {
        //let name = gamepad.name().to_lowercase();
        //let product = gamepad.product_id();

        let vendor = gamepad.vendor_id();
        // No, Cyberpower, you do not make gamepads
        if vendor == Some(0x0764) {
            return false;
        }
        // Filter gamepads with no stick
        let has_inputs = gamepad.axis_data(Axis::LeftStickX).is_some();
        has_inputs
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
        if Some(JoystickMapping::Gamepad(id)) == self.mapping.0 {
            Some(0)
        }
        else if Some(JoystickMapping::Gamepad(id)) == self.mapping.1 {
            Some(1)
        }
        else {
            None
        }
    }

    /// Return whether the mapping for the given joystick slot is a joykey mapping.
    /// This can be used to control dispatch to the joykey interface.
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
        //log::debug!("joykey_mapping(): mapping: {:?}", self.mapping);
        if let Some(JoystickMapping::JoyKeys) = self.mapping.0 {
            //log::debug!("Joykey mapping found in slot 0");
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

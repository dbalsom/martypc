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

    input.rs

    Routines for interfacing winit window input to emulator input.
    This module defines the MartyKey enum which is the frontend-independent
    MartyKey enum based on the W3C naming convention for UI input events:

    https://w3c.github.io/uievents-code/#code-value-tables
*/

pub mod joystick;
pub mod keyboard;
pub mod mouse;

use crate::types::hotkeys::{HotkeyConfigEntry, HotkeyEvent, HotkeyScope};
use marty_core::keys::MartyKey;
use std::{
    collections::{HashMap, HashSet},
    env::consts::OS,
};
use strum::IntoEnumIterator;

pub enum MouseButton {
    Left,
    Right,
    Middle,
}

pub struct HotkeyState {
    pub keyset: HashSet<MartyKey>,
    pub pressed: HashSet<MartyKey>,
    pub scope: HotkeyScope,
    pub capture_disable: bool,
    pub len: usize,
}

impl Default for HotkeyState {
    fn default() -> Self {
        HotkeyState {
            keyset: HashSet::new(),
            pressed: HashSet::new(),
            scope: HotkeyScope::Any,
            capture_disable: false,
            len: 0,
        }
    }
}

pub struct HotkeyManager {
    pub hotkeys: HashMap<HotkeyEvent, HotkeyState>,
}

impl Default for HotkeyManager {
    fn default() -> Self {
        let mut hotkeys = HashMap::new();
        for hotkey in HotkeyEvent::iter() {
            hotkeys.insert(hotkey, HotkeyState::default());
        }
        HotkeyManager { hotkeys }
    }
}

impl HotkeyManager {
    pub fn new() -> Self {
        HotkeyManager::default()
    }

    pub fn add_hotkeys(&mut self, hotkey_list: Vec<HotkeyConfigEntry>) {
        for entry in hotkey_list {
            self.add_hotkey(entry.event, entry.keys, entry.scope, entry.capture_disable);
        }
    }

    pub fn add_hotkey(
        &mut self,
        hotkey: HotkeyEvent,
        keyvec: Vec<MartyKey>,
        scope: HotkeyScope,
        capture_disable: bool,
    ) {
        let len = keyvec.len();
        self.hotkeys.insert(
            hotkey,
            HotkeyState {
                keyset: HashSet::from_iter(keyvec.iter().cloned()),
                pressed: HashSet::new(),
                scope,
                capture_disable,
                len,
            },
        );
    }

    pub fn keydown(&mut self, key: MartyKey, gui_focus: bool, input_captured: bool) -> Option<Vec<HotkeyEvent>> {
        let mut events = Vec::new();
        for (hotkey, state) in self.hotkeys.iter_mut() {
            let mut process_key = match state.scope {
                HotkeyScope::Any => true,
                HotkeyScope::Gui => gui_focus,
                HotkeyScope::Machine => !gui_focus,
                HotkeyScope::Captured => input_captured,
            };

            if state.capture_disable && input_captured {
                process_key = false;
            }

            if process_key && state.keyset.contains(&key) {
                state.pressed.insert(key);
                if state.pressed.len() == state.len {
                    log::debug!("Hotkey matched: {:?}, len: {}", hotkey, state.len);
                    events.push(*hotkey);
                }
            }
        }

        if events.is_empty() {
            None
        }
        else {
            Some(events)
        }
    }

    pub fn keyup(&mut self, key: MartyKey) {
        for state in self.hotkeys.values_mut() {
            if state.keyset.contains(&key) {
                state.pressed.remove(&key);
            }
        }
    }
}

pub trait TranslateKey {
    fn to_internal(&self) -> MartyKey;
}

pub fn button_from_id(id: u32, reverse: bool) -> MouseButton {
    match (OS, id, reverse) {
        ("windows", 0, false) => MouseButton::Left,
        ("windows", 0, true) => MouseButton::Right,
        ("windows", 1, false) => MouseButton::Right,
        ("windows", 1, true) => MouseButton::Left,
        ("linux", 0, false) => MouseButton::Left, // TODO: Verify this
        ("linux", 0, true) => MouseButton::Right,
        ("linux", 1, false) => MouseButton::Right,
        ("linux", 1, true) => MouseButton::Left,
        ("macos", 1, false) => MouseButton::Right, // MacOS is reversed!
        ("macos", 1, true) => MouseButton::Left,
        ("macos", 0, false) => MouseButton::Left,
        ("macos", 0, true) => MouseButton::Right,
        (_, 0, false) => MouseButton::Left,
        (_, 0, true) => MouseButton::Right,
        (_, 1, false) => MouseButton::Right,
        (_, 1, true) => MouseButton::Left,
        _ => MouseButton::Middle, // TODO: This assumes middle button is always 2, valid?
    }
}

/// Return the winit button id for
pub fn get_mouse_buttons(reverse: bool) -> (u32, u32) {
    match (OS, reverse) {
        ("windows", false) => (1, 3),
        ("windows", true) => (3, 1),
        ("linux", false) => (1, 3), // TODO: Verify this
        ("linux", true) => (3, 1),
        ("macos", false) => (3, 1), // MacOS is reversed!
        ("macos", true) => (3, 1),
        (_, false) => (1, 3),
        (_, true) => (3, 1),
    }
}

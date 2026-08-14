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

use std::collections::HashMap;

use marty_common::{types::keys::MartyKey, MartyHashSet, MartyIndexSet};
use marty_frontend_common::{HotkeyConfigEntry, HotkeyEvent, HotkeyScope};

use strum::IntoEnumIterator;

pub struct HotkeyState {
    pub keyset: MartyIndexSet<MartyKey>,
    pub pressed: MartyHashSet<MartyKey>,
    pub scope: HotkeyScope,
    pub capture_disable: bool,
}

impl Default for HotkeyState {
    fn default() -> Self {
        HotkeyState {
            keyset: MartyIndexSet::default(),
            pressed: MartyHashSet::default(),
            scope: HotkeyScope::Any,
            capture_disable: false,
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
        let keyset = MartyIndexSet::from_iter(keyvec);
        self.hotkeys.insert(
            hotkey,
            HotkeyState {
                keyset,
                pressed: MartyHashSet::default(),
                scope,
                capture_disable,
            },
        );
    }

    /// Return the configured key combination in a stable, user-facing form.
    pub fn hotkey_string(&self, hotkey: HotkeyEvent) -> Option<String> {
        Some(
            self.hotkey_keys(hotkey)?
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("+"),
        )
    }

    /// Iterate the configured key combination in definition order.
    ///
    /// This is the presentation-neutral counterpart to [`Self::hotkey_string`]. Frontends can
    /// use it to render each key independently, such as with a pill-style hotkey widget.
    pub fn hotkey_keys(&self, hotkey: HotkeyEvent) -> Option<impl ExactSizeIterator<Item = &MartyKey>> {
        let state = self.hotkeys.get(&hotkey)?;
        (!state.keyset.is_empty()).then_some(state.keyset.iter())
    }

    /// Return all configured bindings in a stable order suitable for presentation.
    pub fn hotkey_bindings(&self) -> Vec<(HotkeyEvent, Vec<MartyKey>)> {
        HotkeyEvent::iter()
            .filter_map(|event| self.hotkey_keys(event).map(|keys| (event, keys.copied().collect())))
            .collect()
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
                if state.pressed.len() == state.keyset.len() {
                    log::debug!("Hotkey matched: {:?}, len: {}", hotkey, state.keyset.len());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stringifies_keys_in_configuration_order() {
        let mut manager = HotkeyManager::new();
        manager.add_hotkey(
            HotkeyEvent::CaptureMouse,
            vec![MartyKey::ControlLeft, MartyKey::F10],
            HotkeyScope::Any,
            false,
        );

        assert_eq!(
            manager.hotkey_string(HotkeyEvent::CaptureMouse).as_deref(),
            Some("Left Ctrl+F10")
        );
        assert_eq!(
            manager
                .hotkey_keys(HotkeyEvent::CaptureMouse)
                .unwrap()
                .copied()
                .collect::<Vec<_>>(),
            vec![MartyKey::ControlLeft, MartyKey::F10]
        );
    }

    #[test]
    fn duplicate_keys_are_deduplicated_for_matching() {
        let mut manager = HotkeyManager::new();
        manager.add_hotkey(
            HotkeyEvent::CaptureMouse,
            vec![MartyKey::ControlLeft, MartyKey::F10, MartyKey::ControlLeft],
            HotkeyScope::Any,
            false,
        );

        assert_eq!(
            manager.hotkey_string(HotkeyEvent::CaptureMouse).as_deref(),
            Some("Left Ctrl+F10")
        );
        assert!(manager.keydown(MartyKey::ControlLeft, false, false).is_none());
        assert_eq!(
            manager.keydown(MartyKey::F10, false, false),
            Some(vec![HotkeyEvent::CaptureMouse])
        );
    }

    #[test]
    fn returns_none_for_an_unbound_event() {
        let manager = HotkeyManager::new();
        assert_eq!(manager.hotkey_string(HotkeyEvent::CaptureMouse), None);
    }

    #[test]
    fn bindings_have_stable_event_and_key_order() {
        let mut manager = HotkeyManager::new();
        manager.add_hotkey(
            HotkeyEvent::Screenshot,
            vec![MartyKey::ControlLeft, MartyKey::F12],
            HotkeyScope::Any,
            false,
        );
        manager.add_hotkey(
            HotkeyEvent::Quit,
            vec![MartyKey::ControlLeft, MartyKey::KeyQ],
            HotkeyScope::Any,
            false,
        );

        assert_eq!(
            manager.hotkey_bindings(),
            vec![
                (HotkeyEvent::Quit, vec![MartyKey::ControlLeft, MartyKey::KeyQ]),
                (HotkeyEvent::Screenshot, vec![MartyKey::ControlLeft, MartyKey::F12]),
            ]
        );
    }
}

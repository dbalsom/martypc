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

    input.rs

    Routines for interfacing winit window input to emulator input.
    This module defines the MartyKey enum which is the frontend-independent
    MartyKey enum based on the W3C naming convention for UI input events:

    https://w3c.github.io/uievents-code/#code-value-tables
*/

use marty_core::keys::MartyKey;
use marty_frontend_common::{HotkeyConfigEntry, HotkeyEvent, HotkeyScope};
use std::{
    collections::{HashMap, HashSet},
    env::consts::OS,
};
use strum::IntoEnumIterator;

#[cfg(feature = "use_winit")]
use winit::keyboard::KeyCode;

use egui::Key;

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

// Implement mapping from Winit Keycode to MartyKey.
// Most of these names are the same, except for Super -> Meta.
#[cfg(feature = "use_winit")]
impl TranslateKey for KeyCode {
    fn to_internal(&self) -> MartyKey {
        match self {
            KeyCode::Backquote => MartyKey::Backquote,
            KeyCode::Backslash => MartyKey::Backslash,
            KeyCode::BracketLeft => MartyKey::BracketLeft,
            KeyCode::BracketRight => MartyKey::BracketRight,
            KeyCode::Comma => MartyKey::Comma,
            KeyCode::Digit0 => MartyKey::Digit0,
            KeyCode::Digit1 => MartyKey::Digit1,
            KeyCode::Digit2 => MartyKey::Digit2,
            KeyCode::Digit3 => MartyKey::Digit3,
            KeyCode::Digit4 => MartyKey::Digit4,
            KeyCode::Digit5 => MartyKey::Digit5,
            KeyCode::Digit6 => MartyKey::Digit6,
            KeyCode::Digit7 => MartyKey::Digit7,
            KeyCode::Digit8 => MartyKey::Digit8,
            KeyCode::Digit9 => MartyKey::Digit9,
            KeyCode::Equal => MartyKey::Equal,
            KeyCode::IntlBackslash => MartyKey::IntlBackslash,
            KeyCode::IntlRo => MartyKey::IntlRo,
            KeyCode::IntlYen => MartyKey::IntlYen,
            KeyCode::KeyA => MartyKey::KeyA,
            KeyCode::KeyB => MartyKey::KeyB,
            KeyCode::KeyC => MartyKey::KeyC,
            KeyCode::KeyD => MartyKey::KeyD,
            KeyCode::KeyE => MartyKey::KeyE,
            KeyCode::KeyF => MartyKey::KeyF,
            KeyCode::KeyG => MartyKey::KeyG,
            KeyCode::KeyH => MartyKey::KeyH,
            KeyCode::KeyI => MartyKey::KeyI,
            KeyCode::KeyJ => MartyKey::KeyJ,
            KeyCode::KeyK => MartyKey::KeyK,
            KeyCode::KeyL => MartyKey::KeyL,
            KeyCode::KeyM => MartyKey::KeyM,
            KeyCode::KeyN => MartyKey::KeyN,
            KeyCode::KeyO => MartyKey::KeyO,
            KeyCode::KeyP => MartyKey::KeyP,
            KeyCode::KeyQ => MartyKey::KeyQ,
            KeyCode::KeyR => MartyKey::KeyR,
            KeyCode::KeyS => MartyKey::KeyS,
            KeyCode::KeyT => MartyKey::KeyT,
            KeyCode::KeyU => MartyKey::KeyU,
            KeyCode::KeyV => MartyKey::KeyV,
            KeyCode::KeyW => MartyKey::KeyW,
            KeyCode::KeyX => MartyKey::KeyX,
            KeyCode::KeyY => MartyKey::KeyY,
            KeyCode::KeyZ => MartyKey::KeyZ,
            KeyCode::Minus => MartyKey::Minus,
            KeyCode::Period => MartyKey::Period,
            KeyCode::Quote => MartyKey::Quote,
            KeyCode::Semicolon => MartyKey::Semicolon,
            KeyCode::Slash => MartyKey::Slash,
            KeyCode::AltLeft => MartyKey::AltLeft,
            KeyCode::AltRight => MartyKey::AltRight,
            KeyCode::Backspace => MartyKey::Backspace,
            KeyCode::CapsLock => MartyKey::CapsLock,
            KeyCode::ContextMenu => MartyKey::ContextMenu,
            KeyCode::ControlLeft => MartyKey::ControlLeft,
            KeyCode::ControlRight => MartyKey::ControlRight,
            KeyCode::Enter => MartyKey::Enter,
            KeyCode::SuperLeft => MartyKey::MetaLeft,
            KeyCode::SuperRight => MartyKey::MetaRight,
            KeyCode::ShiftLeft => MartyKey::ShiftLeft,
            KeyCode::ShiftRight => MartyKey::ShiftRight,
            KeyCode::Space => MartyKey::Space,
            KeyCode::Tab => MartyKey::Tab,
            KeyCode::Convert => MartyKey::Convert,
            KeyCode::KanaMode => MartyKey::KanaMode,
            KeyCode::Lang1 => MartyKey::Lang1,
            KeyCode::Lang2 => MartyKey::Lang2,
            KeyCode::Lang3 => MartyKey::Lang3,
            KeyCode::Lang4 => MartyKey::Lang4,
            KeyCode::Lang5 => MartyKey::Lang5,
            KeyCode::NonConvert => MartyKey::NonConvert,
            KeyCode::Delete => MartyKey::Delete,
            KeyCode::End => MartyKey::End,
            KeyCode::Help => MartyKey::Help,
            KeyCode::Home => MartyKey::Home,
            KeyCode::Insert => MartyKey::Insert,
            KeyCode::PageDown => MartyKey::PageDown,
            KeyCode::PageUp => MartyKey::PageUp,
            KeyCode::ArrowDown => MartyKey::ArrowDown,
            KeyCode::ArrowLeft => MartyKey::ArrowLeft,
            KeyCode::ArrowRight => MartyKey::ArrowRight,
            KeyCode::ArrowUp => MartyKey::ArrowUp,
            KeyCode::NumLock => MartyKey::NumLock,
            KeyCode::Numpad0 => MartyKey::Numpad0,
            KeyCode::Numpad1 => MartyKey::Numpad1,
            KeyCode::Numpad2 => MartyKey::Numpad2,
            KeyCode::Numpad3 => MartyKey::Numpad3,
            KeyCode::Numpad4 => MartyKey::Numpad4,
            KeyCode::Numpad5 => MartyKey::Numpad5,
            KeyCode::Numpad6 => MartyKey::Numpad6,
            KeyCode::Numpad7 => MartyKey::Numpad7,
            KeyCode::Numpad8 => MartyKey::Numpad8,
            KeyCode::Numpad9 => MartyKey::Numpad9,
            KeyCode::NumpadAdd => MartyKey::NumpadAdd,
            KeyCode::NumpadBackspace => MartyKey::NumpadBackspace,
            KeyCode::NumpadClear => MartyKey::NumpadClear,
            KeyCode::NumpadClearEntry => MartyKey::NumpadClearEntry,
            KeyCode::NumpadComma => MartyKey::NumpadComma,
            KeyCode::NumpadDecimal => MartyKey::NumpadDecimal,
            KeyCode::NumpadDivide => MartyKey::NumpadDivide,
            KeyCode::NumpadEnter => MartyKey::NumpadEnter,
            KeyCode::NumpadEqual => MartyKey::NumpadEqual,
            KeyCode::NumpadHash => MartyKey::NumpadHash,
            KeyCode::NumpadMemoryAdd => MartyKey::NumpadMemoryAdd,
            KeyCode::NumpadMemoryClear => MartyKey::NumpadMemoryClear,
            KeyCode::NumpadMemoryRecall => MartyKey::NumpadMemoryRecall,
            KeyCode::NumpadMemoryStore => MartyKey::NumpadMemoryStore,
            KeyCode::NumpadMemorySubtract => MartyKey::NumpadMemorySubtract,
            KeyCode::NumpadMultiply => MartyKey::NumpadMultiply,
            KeyCode::NumpadParenLeft => MartyKey::NumpadParenLeft,
            KeyCode::NumpadParenRight => MartyKey::NumpadParenRight,
            KeyCode::NumpadStar => MartyKey::NumpadStar,
            KeyCode::NumpadSubtract => MartyKey::NumpadSubtract,
            KeyCode::Escape => MartyKey::Escape,
            KeyCode::Fn => MartyKey::Fn,
            KeyCode::FnLock => MartyKey::FnLock,
            KeyCode::PrintScreen => MartyKey::PrintScreen,
            KeyCode::ScrollLock => MartyKey::ScrollLock,
            KeyCode::Pause => MartyKey::Pause,
            KeyCode::BrowserBack => MartyKey::BrowserBack,
            KeyCode::BrowserFavorites => MartyKey::BrowserFavorites,
            KeyCode::BrowserForward => MartyKey::BrowserForward,
            KeyCode::BrowserHome => MartyKey::BrowserHome,
            KeyCode::BrowserRefresh => MartyKey::BrowserRefresh,
            KeyCode::BrowserSearch => MartyKey::BrowserSearch,
            KeyCode::BrowserStop => MartyKey::BrowserStop,
            KeyCode::Eject => MartyKey::Eject,
            KeyCode::LaunchApp1 => MartyKey::LaunchApp1,
            KeyCode::LaunchApp2 => MartyKey::LaunchApp2,
            KeyCode::LaunchMail => MartyKey::LaunchMail,
            KeyCode::MediaPlayPause => MartyKey::MediaPlayPause,
            KeyCode::MediaSelect => MartyKey::MediaSelect,
            KeyCode::MediaStop => MartyKey::MediaStop,
            KeyCode::MediaTrackNext => MartyKey::MediaTrackNext,
            KeyCode::MediaTrackPrevious => MartyKey::MediaTrackPrevious,
            KeyCode::Power => MartyKey::Power,
            KeyCode::Sleep => MartyKey::Sleep,
            KeyCode::AudioVolumeDown => MartyKey::AudioVolumeDown,
            KeyCode::AudioVolumeMute => MartyKey::AudioVolumeMute,
            KeyCode::AudioVolumeUp => MartyKey::AudioVolumeUp,
            KeyCode::WakeUp => MartyKey::WakeUp,
            KeyCode::Meta => MartyKey::Meta,
            KeyCode::Hyper => MartyKey::Hyper,
            KeyCode::Turbo => MartyKey::Turbo,
            KeyCode::Abort => MartyKey::Abort,
            KeyCode::Resume => MartyKey::Resume,
            KeyCode::Suspend => MartyKey::Suspend,
            KeyCode::Again => MartyKey::Again,
            KeyCode::Copy => MartyKey::Copy,
            KeyCode::Cut => MartyKey::Cut,
            KeyCode::Find => MartyKey::Find,
            KeyCode::Open => MartyKey::Open,
            KeyCode::Paste => MartyKey::Paste,
            KeyCode::Props => MartyKey::Props,
            KeyCode::Select => MartyKey::Select,
            KeyCode::Undo => MartyKey::Undo,
            KeyCode::Hiragana => MartyKey::Hiragana,
            KeyCode::Katakana => MartyKey::Katakana,
            KeyCode::F1 => MartyKey::F1,
            KeyCode::F2 => MartyKey::F2,
            KeyCode::F3 => MartyKey::F3,
            KeyCode::F4 => MartyKey::F4,
            KeyCode::F5 => MartyKey::F5,
            KeyCode::F6 => MartyKey::F6,
            KeyCode::F7 => MartyKey::F7,
            KeyCode::F8 => MartyKey::F8,
            KeyCode::F9 => MartyKey::F9,
            KeyCode::F10 => MartyKey::F10,
            KeyCode::F11 => MartyKey::F11,
            KeyCode::F12 => MartyKey::F12,
            KeyCode::F13 => MartyKey::F13,
            KeyCode::F14 => MartyKey::F14,
            KeyCode::F15 => MartyKey::F15,
            KeyCode::F16 => MartyKey::F16,
            KeyCode::F17 => MartyKey::F17,
            KeyCode::F18 => MartyKey::F18,
            KeyCode::F19 => MartyKey::F19,
            KeyCode::F20 => MartyKey::F20,
            KeyCode::F21 => MartyKey::F21,
            KeyCode::F22 => MartyKey::F22,
            KeyCode::F23 => MartyKey::F23,
            KeyCode::F24 => MartyKey::F24,
            KeyCode::F25 => MartyKey::F25,
            KeyCode::F26 => MartyKey::F26,
            KeyCode::F27 => MartyKey::F27,
            KeyCode::F28 => MartyKey::F28,
            KeyCode::F29 => MartyKey::F29,
            KeyCode::F30 => MartyKey::F30,
            KeyCode::F31 => MartyKey::F31,
            KeyCode::F32 => MartyKey::F32,
            KeyCode::F33 => MartyKey::F33,
            KeyCode::F34 => MartyKey::F34,
            KeyCode::F35 => MartyKey::F35,
            _ => MartyKey::None,
        }
    }
}

impl TranslateKey for Key {
    fn to_internal(&self) -> MartyKey {
        match self {
            Key::Backtick => MartyKey::Backquote,
            Key::Backslash => MartyKey::Backslash,
            //Key::BracketLeft => MartyKey::BracketLeft,
            //Key::BracketRight => MartyKey::BracketRight,
            Key::Comma => MartyKey::Comma,
            Key::Num0 => MartyKey::Digit0, // eframe does not discriminate between digit row and numpad digits. :(
            Key::Num1 => MartyKey::Digit1,
            Key::Num2 => MartyKey::Digit2,
            Key::Num3 => MartyKey::Digit3,
            Key::Num4 => MartyKey::Digit4,
            Key::Num5 => MartyKey::Digit5,
            Key::Num6 => MartyKey::Digit6,
            Key::Num7 => MartyKey::Digit7,
            Key::Num8 => MartyKey::Digit8,
            Key::Num9 => MartyKey::Digit9,
            Key::Equals => MartyKey::Equal,
            // Key::IntlBackslash => MartyKey::IntlBackslash,
            // Key::IntlRo => MartyKey::IntlRo,
            // Key::IntlYen => MartyKey::IntlYen,
            Key::A => MartyKey::KeyA,
            Key::B => MartyKey::KeyB,
            Key::C => MartyKey::KeyC,
            Key::D => MartyKey::KeyD,
            Key::E => MartyKey::KeyE,
            Key::F => MartyKey::KeyF,
            Key::G => MartyKey::KeyG,
            Key::H => MartyKey::KeyH,
            Key::I => MartyKey::KeyI,
            Key::J => MartyKey::KeyJ,
            Key::K => MartyKey::KeyK,
            Key::L => MartyKey::KeyL,
            Key::M => MartyKey::KeyM,
            Key::N => MartyKey::KeyN,
            Key::O => MartyKey::KeyO,
            Key::P => MartyKey::KeyP,
            Key::Q => MartyKey::KeyQ,
            Key::R => MartyKey::KeyR,
            Key::S => MartyKey::KeyS,
            Key::T => MartyKey::KeyT,
            Key::U => MartyKey::KeyU,
            Key::V => MartyKey::KeyV,
            Key::W => MartyKey::KeyW,
            Key::X => MartyKey::KeyX,
            Key::Y => MartyKey::KeyY,
            Key::Z => MartyKey::KeyZ,
            Key::Minus => MartyKey::Minus,
            Key::Period => MartyKey::Period,
            Key::Quote => MartyKey::Quote,
            Key::Semicolon => MartyKey::Semicolon,
            Key::Slash => MartyKey::Slash,
            // egui does not report modifier keypresses :(
            //Key::AltLeft => MartyKey::AltLeft,
            //Key::AltRight => MartyKey::AltRight,
            Key::Backspace => MartyKey::Backspace,
            // Key::CapsLock => MartyKey::CapsLock,
            // Key::ContextMenu => MartyKey::ContextMenu,
            // Key::ControlLeft => MartyKey::ControlLeft,
            // Key::ControlRight => MartyKey::ControlRight,
            Key::Enter => MartyKey::Enter,
            // Key::SuperLeft => MartyKey::MetaLeft,
            // Key::SuperRight => MartyKey::MetaRight,
            // Key::ShiftLeft => MartyKey::ShiftLeft,
            // Key::ShiftRight => MartyKey::ShiftRight,
            Key::Space => MartyKey::Space,
            Key::Tab => MartyKey::Tab,
            // Key::Convert => MartyKey::Convert,
            // Key::KanaMode => MartyKey::KanaMode,
            // Key::Lang1 => MartyKey::Lang1,
            // Key::Lang2 => MartyKey::Lang2,
            // Key::Lang3 => MartyKey::Lang3,
            // Key::Lang4 => MartyKey::Lang4,
            // Key::Lang5 => MartyKey::Lang5,
            // Key::NonConvert => MartyKey::NonConvert,
            Key::Delete => MartyKey::Delete,
            Key::End => MartyKey::End,
            //Key::Help => MartyKey::Help,
            Key::Home => MartyKey::Home,
            Key::Insert => MartyKey::Insert,
            Key::PageDown => MartyKey::PageDown,
            Key::PageUp => MartyKey::PageUp,
            Key::ArrowDown => MartyKey::ArrowDown,
            Key::ArrowLeft => MartyKey::ArrowLeft,
            Key::ArrowRight => MartyKey::ArrowRight,
            Key::ArrowUp => MartyKey::ArrowUp,
            // Key::NumLock => MartyKey::NumLock,
            // Key::Numpad0 => MartyKey::Numpad0,
            // Key::Numpad1 => MartyKey::Numpad1,
            // Key::Numpad2 => MartyKey::Numpad2,
            // Key::Numpad3 => MartyKey::Numpad3,
            // Key::Numpad4 => MartyKey::Numpad4,
            // Key::Numpad5 => MartyKey::Numpad5,
            // Key::Numpad6 => MartyKey::Numpad6,
            // Key::Numpad7 => MartyKey::Numpad7,
            // Key::Numpad8 => MartyKey::Numpad8,
            // Key::Numpad9 => MartyKey::Numpad9,
            // Key::NumpadAdd => MartyKey::NumpadAdd,
            // Key::NumpadBackspace => MartyKey::NumpadBackspace,
            // Key::NumpadClear => MartyKey::NumpadClear,
            // Key::NumpadClearEntry => MartyKey::NumpadClearEntry,
            // Key::NumpadComma => MartyKey::NumpadComma,
            // Key::NumpadDecimal => MartyKey::NumpadDecimal,
            // Key::NumpadDivide => MartyKey::NumpadDivide,
            // Key::NumpadEnter => MartyKey::NumpadEnter,
            // Key::NumpadEqual => MartyKey::NumpadEqual,
            // Key::NumpadHash => MartyKey::NumpadHash,
            // Key::NumpadMemoryAdd => MartyKey::NumpadMemoryAdd,
            // Key::NumpadMemoryClear => MartyKey::NumpadMemoryClear,
            // Key::NumpadMemoryRecall => MartyKey::NumpadMemoryRecall,
            // Key::NumpadMemoryStore => MartyKey::NumpadMemoryStore,
            // Key::NumpadMemorySubtract => MartyKey::NumpadMemorySubtract,
            // Key::NumpadMultiply => MartyKey::NumpadMultiply,
            // Key::NumpadParenLeft => MartyKey::NumpadParenLeft,
            // Key::NumpadParenRight => MartyKey::NumpadParenRight,
            // Key::NumpadStar => MartyKey::NumpadStar,
            // Key::NumpadSubtract => MartyKey::NumpadSubtract,
            Key::Escape => MartyKey::Escape,
            // Key::Fn => MartyKey::Fn,
            // Key::FnLock => MartyKey::FnLock,
            // Key::PrintScreen => MartyKey::PrintScreen,
            // Key::ScrollLock => MartyKey::ScrollLock,
            // Key::Pause => MartyKey::Pause,
            // Key::BrowserBack => MartyKey::BrowserBack,
            // Key::BrowserFavorites => MartyKey::BrowserFavorites,
            // Key::BrowserForward => MartyKey::BrowserForward,
            // Key::BrowserHome => MartyKey::BrowserHome,
            // Key::BrowserRefresh => MartyKey::BrowserRefresh,
            // Key::BrowserSearch => MartyKey::BrowserSearch,
            // Key::BrowserStop => MartyKey::BrowserStop,
            // Key::Eject => MartyKey::Eject,
            // Key::LaunchApp1 => MartyKey::LaunchApp1,
            // Key::LaunchApp2 => MartyKey::LaunchApp2,
            // Key::LaunchMail => MartyKey::LaunchMail,
            // Key::MediaPlayPause => MartyKey::MediaPlayPause,
            // Key::MediaSelect => MartyKey::MediaSelect,
            // Key::MediaStop => MartyKey::MediaStop,
            // Key::MediaTrackNext => MartyKey::MediaTrackNext,
            // Key::MediaTrackPrevious => MartyKey::MediaTrackPrevious,
            // Key::Power => MartyKey::Power,
            // Key::Sleep => MartyKey::Sleep,
            // Key::AudioVolumeDown => MartyKey::AudioVolumeDown,
            // Key::AudioVolumeMute => MartyKey::AudioVolumeMute,
            // Key::AudioVolumeUp => MartyKey::AudioVolumeUp,
            // Key::WakeUp => MartyKey::WakeUp,
            // Key::Meta => MartyKey::Meta,
            // Key::Hyper => MartyKey::Hyper,
            // Key::Turbo => MartyKey::Turbo,
            // Key::Abort => MartyKey::Abort,
            // Key::Resume => MartyKey::Resume,
            // Key::Suspend => MartyKey::Suspend,
            // Key::Again => MartyKey::Again,
            Key::Copy => MartyKey::Copy,
            Key::Cut => MartyKey::Cut,
            // Key::Find => MartyKey::Find,
            // Key::Open => MartyKey::Open,
            Key::Paste => MartyKey::Paste,
            // Key::Props => MartyKey::Props,
            // Key::Select => MartyKey::Select,
            // Key::Undo => MartyKey::Undo,
            // Key::Hiragana => MartyKey::Hiragana,
            // Key::Katakana => MartyKey::Katakana,
            Key::F1 => MartyKey::F1,
            Key::F2 => MartyKey::F2,
            Key::F3 => MartyKey::F3,
            Key::F4 => MartyKey::F4,
            Key::F5 => MartyKey::F5,
            Key::F6 => MartyKey::F6,
            Key::F7 => MartyKey::F7,
            Key::F8 => MartyKey::F8,
            Key::F9 => MartyKey::F9,
            Key::F10 => MartyKey::F10,
            Key::F11 => MartyKey::F11,
            Key::F12 => MartyKey::F12,
            Key::F13 => MartyKey::F13,
            Key::F14 => MartyKey::F14,
            Key::F15 => MartyKey::F15,
            Key::F16 => MartyKey::F16,
            Key::F17 => MartyKey::F17,
            Key::F18 => MartyKey::F18,
            Key::F19 => MartyKey::F19,
            Key::F20 => MartyKey::F20,
            Key::F21 => MartyKey::F21,
            Key::F22 => MartyKey::F22,
            Key::F23 => MartyKey::F23,
            Key::F24 => MartyKey::F24,
            Key::F25 => MartyKey::F25,
            Key::F26 => MartyKey::F26,
            Key::F27 => MartyKey::F27,
            Key::F28 => MartyKey::F28,
            Key::F29 => MartyKey::F29,
            Key::F30 => MartyKey::F30,
            Key::F31 => MartyKey::F31,
            Key::F32 => MartyKey::F32,
            Key::F33 => MartyKey::F33,
            Key::F34 => MartyKey::F34,
            Key::F35 => MartyKey::F35,
            _ => MartyKey::None,
        }
    }
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

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

    keys.rs

    Defines the MartyKey enum, MartyPC's internal keycode representation.
    Names are based on the W3C's keycode naming convention:

    https://w3c.github.io/uievents-code/#code-value-tables

    Frontend libraries should define a TranslateKey trait to convert the
    implementation-specific keycodes into MartyKey(s), such as:

    pub trait TranslateKey {
        fn to_internal(key_code: ImplementationKeyCode) -> MartyKey;
    }
*/
use std::fmt;

use serde::Deserialize;
use strum_macros::{EnumIter, EnumString};

#[derive(Copy, Clone, Debug, EnumIter, EnumString, Deserialize, PartialEq, Eq, Hash)]
pub enum MartyKey {
    None,
    Backquote,
    Backslash,
    BracketLeft,
    BracketRight,
    Comma,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Equal,
    IntlBackslash,
    IntlRo,
    IntlYen,
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    Minus,
    Period,
    Quote,
    Semicolon,
    Slash,
    AltLeft,
    AltRight,
    Backspace,
    CapsLock,
    ContextMenu,
    ControlLeft,
    ControlRight,
    Enter,
    MetaLeft,
    MetaRight,
    ShiftLeft,
    ShiftRight,
    Space,
    Tab,
    Convert,
    KanaMode,
    Lang1,
    Lang2,
    Lang3,
    Lang4,
    Lang5,
    NonConvert,
    Delete,
    End,
    Help,
    Home,
    Insert,
    PageDown,
    PageUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    NumLock,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadAdd,
    NumpadBackspace,
    NumpadClear,
    NumpadClearEntry,
    NumpadComma,
    NumpadDecimal,
    NumpadDivide,
    NumpadEnter,
    NumpadEqual,
    NumpadHash,
    NumpadMemoryAdd,
    NumpadMemoryClear,
    NumpadMemoryRecall,
    NumpadMemoryStore,
    NumpadMemorySubtract,
    NumpadMultiply,
    NumpadParenLeft,
    NumpadParenRight,
    NumpadStar,
    NumpadSubtract,
    Escape,
    Fn,
    FnLock,
    PrintScreen,
    ScrollLock,
    Pause,
    BrowserBack,
    BrowserFavorites,
    BrowserForward,
    BrowserHome,
    BrowserRefresh,
    BrowserSearch,
    BrowserStop,
    Eject,
    LaunchApp1,
    LaunchApp2,
    LaunchMail,
    MediaPlayPause,
    MediaSelect,
    MediaStop,
    MediaTrackNext,
    MediaTrackPrevious,
    Power,
    Sleep,
    AudioVolumeDown,
    AudioVolumeMute,
    AudioVolumeUp,
    WakeUp,
    Meta,
    Hyper,
    Turbo,
    Abort,
    Resume,
    Suspend,
    Again,
    Copy,
    Cut,
    Find,
    Open,
    Paste,
    Props,
    Select,
    Undo,
    Hiragana,
    Katakana,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    F25,
    F26,
    F27,
    F28,
    F29,
    F30,
    F31,
    F32,
    F33,
    F34,
    F35,
}

impl fmt::Display for MartyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            MartyKey::None => "None",
            MartyKey::Backquote => "`",
            MartyKey::Backslash => "\\",
            MartyKey::BracketLeft => "[",
            MartyKey::BracketRight => "]",
            MartyKey::Comma => ",",
            MartyKey::Digit0 => "0",
            MartyKey::Digit1 => "1",
            MartyKey::Digit2 => "2",
            MartyKey::Digit3 => "3",
            MartyKey::Digit4 => "4",
            MartyKey::Digit5 => "5",
            MartyKey::Digit6 => "6",
            MartyKey::Digit7 => "7",
            MartyKey::Digit8 => "8",
            MartyKey::Digit9 => "9",
            MartyKey::Equal => "=",
            MartyKey::KeyA => "A",
            MartyKey::KeyB => "B",
            MartyKey::KeyC => "C",
            MartyKey::KeyD => "D",
            MartyKey::KeyE => "E",
            MartyKey::KeyF => "F",
            MartyKey::KeyG => "G",
            MartyKey::KeyH => "H",
            MartyKey::KeyI => "I",
            MartyKey::KeyJ => "J",
            MartyKey::KeyK => "K",
            MartyKey::KeyL => "L",
            MartyKey::KeyM => "M",
            MartyKey::KeyN => "N",
            MartyKey::KeyO => "O",
            MartyKey::KeyP => "P",
            MartyKey::KeyQ => "Q",
            MartyKey::KeyR => "R",
            MartyKey::KeyS => "S",
            MartyKey::KeyT => "T",
            MartyKey::KeyU => "U",
            MartyKey::KeyV => "V",
            MartyKey::KeyW => "W",
            MartyKey::KeyX => "X",
            MartyKey::KeyY => "Y",
            MartyKey::KeyZ => "Z",
            MartyKey::Minus => "-",
            MartyKey::Period => ".",
            MartyKey::Quote => "'",
            MartyKey::Semicolon => ";",
            MartyKey::Slash => "/",
            MartyKey::AltLeft => "Left Alt",
            MartyKey::AltRight => "Right Alt",
            MartyKey::CapsLock => "Caps Lock",
            MartyKey::ContextMenu => "Context Menu",
            MartyKey::ControlLeft => "Left Ctrl",
            MartyKey::ControlRight => "Right Ctrl",
            MartyKey::MetaLeft => "Left Meta",
            MartyKey::MetaRight => "Right Meta",
            MartyKey::ShiftLeft => "Left Shift",
            MartyKey::ShiftRight => "Right Shift",
            MartyKey::PageDown => "Page Down",
            MartyKey::PageUp => "Page Up",
            MartyKey::ArrowDown => "Down",
            MartyKey::ArrowLeft => "Left",
            MartyKey::ArrowRight => "Right",
            MartyKey::ArrowUp => "Up",
            MartyKey::NumLock => "Num Lock",
            MartyKey::NumpadAdd => "Numpad +",
            MartyKey::NumpadDecimal => "Numpad .",
            MartyKey::NumpadDivide => "Numpad /",
            MartyKey::NumpadEnter => "Numpad Enter",
            MartyKey::NumpadEqual => "Numpad =",
            MartyKey::NumpadMultiply => "Numpad *",
            MartyKey::NumpadSubtract => "Numpad -",
            MartyKey::Escape => "Esc",
            MartyKey::PrintScreen => "Print Screen",
            MartyKey::ScrollLock => "Scroll Lock",
            MartyKey::MediaPlayPause => "Media Play/Pause",
            MartyKey::MediaTrackNext => "Next Track",
            MartyKey::MediaTrackPrevious => "Previous Track",
            MartyKey::AudioVolumeDown => "Volume Down",
            MartyKey::AudioVolumeMute => "Volume Mute",
            MartyKey::AudioVolumeUp => "Volume Up",
            _ => return write!(f, "{self:?}"),
        };

        f.write_str(label)
    }
}

#[cfg(test)]
mod tests {
    use super::MartyKey;

    #[test]
    fn displays_common_hotkey_keys_for_users() {
        assert_eq!(MartyKey::ControlLeft.to_string(), "Left Ctrl");
        assert_eq!(MartyKey::KeyM.to_string(), "M");
        assert_eq!(MartyKey::Digit7.to_string(), "7");
        assert_eq!(MartyKey::F10.to_string(), "F10");
        assert_eq!(MartyKey::ArrowUp.to_string(), "Up");
    }

    #[test]
    fn falls_back_to_the_w3c_name_for_uncommon_keys() {
        assert_eq!(MartyKey::IntlYen.to_string(), "IntlYen");
    }
}

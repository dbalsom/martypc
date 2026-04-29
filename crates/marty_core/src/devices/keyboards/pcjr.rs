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

    devices::keyboards::pcjr.rs
*/

//! Implementation of the PCjr keyboard.

use crate::{device_traits::keyboard::MartyKeyboard, devices::keyboard_common::KeycodeMapping};

use crate::keys::MartyKey;
use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
pub struct PcJrKeyboard {
    pub(crate) keycode_mappings: Vec<KeycodeMapping>,
}

impl MartyKeyboard for PcJrKeyboard {
    /// The PCjr keyboard always produces a single scancode for each keypress.
    fn keycode_to_scancodes(key_code: MartyKey) -> Vec<u8> {
        let mut scancodes = Vec::new();
        let scancode = match key_code {
            MartyKey::Escape => Some(0x01),
            MartyKey::Digit1 => Some(0x02),
            MartyKey::Digit2 => Some(0x03),
            MartyKey::Digit3 => Some(0x04),
            MartyKey::Digit4 => Some(0x05),
            MartyKey::Digit5 => Some(0x06),
            MartyKey::Digit6 => Some(0x07),
            MartyKey::Digit7 => Some(0x08),
            MartyKey::Digit8 => Some(0x09),
            MartyKey::Digit9 => Some(0x0A),
            MartyKey::Digit0 => Some(0x0B),
            MartyKey::Minus => Some(0x0C),
            MartyKey::Equal => Some(0x0D),
            MartyKey::Backspace => Some(0x0E),
            // Function mapped to F12.
            MartyKey::F12 => Some(0x54), // FN 54
            MartyKey::Tab => Some(0x0F),
            MartyKey::KeyQ => Some(0x10),
            MartyKey::KeyW => Some(0x11),
            MartyKey::KeyE => Some(0x12),
            MartyKey::KeyR => Some(0x13),
            MartyKey::KeyT => Some(0x14),
            MartyKey::KeyY => Some(0x15),
            MartyKey::KeyU => Some(0x16),
            MartyKey::KeyI => Some(0x17),
            MartyKey::KeyO => Some(0x18),
            MartyKey::KeyP => Some(0x19),
            MartyKey::BracketLeft => Some(0x1A),
            MartyKey::BracketRight => Some(0x1B),
            MartyKey::Enter => Some(0x1C),
            // PCjr only has one CTRL key.
            MartyKey::ControlRight | MartyKey::ControlLeft => Some(0x1D),
            MartyKey::KeyA => Some(0x1E),
            MartyKey::KeyS => Some(0x1F),
            MartyKey::KeyD => Some(0x20),
            MartyKey::KeyF => Some(0x21),
            MartyKey::KeyG => Some(0x22),
            MartyKey::KeyH => Some(0x23),
            MartyKey::KeyJ => Some(0x24),
            MartyKey::KeyK => Some(0x25),
            MartyKey::KeyL => Some(0x26),
            MartyKey::Semicolon => Some(0x27),
            MartyKey::Quote => Some(0x28), // Apostrophe
            MartyKey::ShiftLeft => Some(0x2A),
            MartyKey::KeyZ => Some(0x2C),
            MartyKey::KeyX => Some(0x2D),
            MartyKey::KeyC => Some(0x2E),
            MartyKey::KeyV => Some(0x2F),
            MartyKey::KeyB => Some(0x30),
            MartyKey::KeyN => Some(0x31),
            MartyKey::KeyM => Some(0x32),
            MartyKey::Comma => Some(0x33),
            MartyKey::Period => Some(0x34),
            MartyKey::Slash => Some(0x35),
            MartyKey::ShiftRight => Some(0x36),
            MartyKey::Numpad4 | MartyKey::ArrowLeft => Some(0x4B), // CUR.LF 4B
            MartyKey::Numpad6 | MartyKey::ArrowRight => Some(0x4D), // CUR.RT 4D
            MartyKey::Numpad8 | MartyKey::ArrowUp => Some(0x48),   // CUR.UP 48
            MartyKey::Numpad2 | MartyKey::ArrowDown => Some(0x50), // CUR.DWN 50
            // PCjr has only one ALT key.
            MartyKey::AltLeft | MartyKey::AltRight => Some(0x38),
            MartyKey::Space => Some(0x39),
            MartyKey::CapsLock => Some(0x3A),
            MartyKey::Insert => Some(0x52),
            MartyKey::Delete => Some(0x53),
            // PCjr does not have the following keys:
            // MartyKey::F1 => Some(0x3b),
            // MartyKey::F2 => Some(0x3c),
            // MartyKey::F3 => Some(0x3d),
            // MartyKey::F4 => Some(0x3e),
            // MartyKey::F5 => Some(0x3f),
            // MartyKey::F6 => Some(0x40),
            // MartyKey::F7 => Some(0x41),
            // MartyKey::F8 => Some(0x42),
            // MartyKey::F9 => Some(0x43),
            // MartyKey::F10 => Some(0x44),
            //MartyKey::Backslash => Some(0x2B),
            //MartyKey::Backquote => Some(0x29), // Grave
            //MartyKey::PrintScreen => Some(0x37),
            //MartyKey::NumLock => Some(0x45),
            //MartyKey::ScrollLock => Some(0x46),
            //MartyKey::Numpad1 | MartyKey::End => Some(0x4F),
            //MartyKey::Numpad3 | MartyKey::PageDown => Some(0x51),
            //MartyKey::Numpad7 | MartyKey::Home => Some(0x47),
            //MartyKey::Numpad9 | MartyKey::PageUp => Some(0x49),
            //MartyKey::NumpadSubtract => Some(0x4A),
            //MartyKey::NumpadAdd => Some(0x4E),
            //MartyKey::NumpadDecimal => Some(0x53),
            //MartyKey::NumpadEnter => Some(0x1C),
            //MartyKey::NumpadDivide => None,
            //MartyKey::NumpadMultiply => None,
            //MartyKey::NumpadEqual => Some(0x0D),
            _ => None,
        };
        if let Some(s) = scancode {
            //log::debug!("Converted key: {:?} to scancode: {:02X}", key_code, s);
            scancodes.push(s);
        }
        scancodes
    }
}

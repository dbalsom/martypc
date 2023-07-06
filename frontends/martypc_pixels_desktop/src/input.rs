/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

    Routines for interfacing window input to emulator input.
*/

use std::env::consts::OS;

use winit::keyboard::{KeyCode, ModifiersKeyState};

pub enum MouseButton {
    Left,
    Right,
    Middle,
}

pub fn button_from_id(id: u32, reverse: bool) -> MouseButton {
    match (OS, id, reverse) {
        ("windows", 1, false) => MouseButton::Left,
        ("windows", 1, true) => MouseButton::Right,
        ("windows", 3, false) => MouseButton::Right,
        ("windows", 3, true) => MouseButton::Left,
        ("linux", 1, false) => MouseButton::Left, // TODO: Verify this
        ("linux", 1, true) => MouseButton::Right,
        ("linux", 3, false) => MouseButton::Right, 
        ("linux", 3, true) => MouseButton::Left, 
        ("macos", 1, false) => MouseButton::Right, // MacOS is reversed!
        ("macos", 1, true) => MouseButton::Left,
        ("macos", 3, false) => MouseButton::Left,
        ("macos", 3, true) => MouseButton::Right,
        (_, 1, false) => MouseButton::Left,
        (_, 1, true) => MouseButton::Right,
        (_, 3, false) => MouseButton::Right,
        (_, 3, true) => MouseButton::Left,
        _ => MouseButton::Middle // TODO: This assumes middle button is always 2, valid?
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


pub fn match_keycode( keycode: KeyCode ) -> Option<u8> {

    match keycode {
        // From Left to Right on IBM XT keyboard
        KeyCode::F1  => Some(0x3b),
        KeyCode::F2  => Some(0x3c),
        KeyCode::F3  => Some(0x3d),
        KeyCode::F4  => Some(0x3e),
        KeyCode::F5  => Some(0x3f),
        KeyCode::F6  => Some(0x40),
        KeyCode::F7  => Some(0x41),
        KeyCode::F8  => Some(0x42),
        KeyCode::F9  => Some(0x43),
        KeyCode::F10 => Some(0x44),
        KeyCode::Escape => Some(0x01),
        KeyCode::Tab => Some(0x0F),
        KeyCode::ControlLeft => Some(0x1D),
        KeyCode::ShiftLeft => Some(0x2A),
        KeyCode::AltLeft => Some(0x38),
        KeyCode::Digit1 => Some(0x02),
        KeyCode::Digit2 => Some(0x03),
        KeyCode::Digit3 => Some(0x04),
        KeyCode::Digit4 => Some(0x05),
        KeyCode::Digit5 => Some(0x06),
        KeyCode::Digit6 => Some(0x07),
        KeyCode::Digit7 => Some(0x08),
        KeyCode::Digit8 => Some(0x09),
        KeyCode::Digit9 => Some(0x0A),
        KeyCode::Digit0 => Some(0x0B),
        KeyCode::Minus => Some(0x0C),
        KeyCode::Equal => Some(0x0D),
        KeyCode::KeyA => Some(0x1E),
        KeyCode::KeyB => Some(0x30),
        KeyCode::KeyC => Some(0x2E),
        KeyCode::KeyD => Some(0x20),
        KeyCode::KeyE => Some(0x12),
        KeyCode::KeyF => Some(0x21),
        KeyCode::KeyG => Some(0x22),
        KeyCode::KeyH => Some(0x23),
        KeyCode::KeyI => Some(0x17),
        KeyCode::KeyJ => Some(0x24),
        KeyCode::KeyK => Some(0x25),
        KeyCode::KeyL => Some(0x26),
        KeyCode::KeyM => Some(0x32),
        KeyCode::KeyN => Some(0x31),
        KeyCode::KeyO => Some(0x18),
        KeyCode::KeyP => Some(0x19),
        KeyCode::KeyQ => Some(0x10),
        KeyCode::KeyR => Some(0x13),
        KeyCode::KeyS => Some(0x1F),
        KeyCode::KeyT => Some(0x14),
        KeyCode::KeyU => Some(0x16),
        KeyCode::KeyV => Some(0x2F),
        KeyCode::KeyW => Some(0x11),
        KeyCode::KeyX => Some(0x2D),
        KeyCode::KeyY => Some(0x15),
        KeyCode::KeyZ => Some(0x2C),

        KeyCode::Backslash => Some(0x2B),
        KeyCode::Space => Some(0x39),
        KeyCode::Backspace => Some(0x0E),
        KeyCode::BracketLeft => Some(0x1A),
        KeyCode::BracketRight => Some(0x1B),
        KeyCode::Semicolon => Some(0x27),
        KeyCode::Backquote => Some(0x29),       // Grave
        KeyCode::Quote => Some(0x28),           // Apostrophe
        KeyCode::Comma => Some(0x33),
        KeyCode::Period => Some(0x34),
        KeyCode::Slash => Some(0x35),
        KeyCode::Enter => Some(0x1C),           // Return
        KeyCode::ShiftRight => Some(0x36),
        KeyCode::CapsLock => Some(0x3A),         // 'Capital'?
        KeyCode::PrintScreen => Some(0x37),        // 'Snapshot'
        KeyCode::Insert => Some(0x52),
        KeyCode::Delete => Some(0x53),
        KeyCode::NumLock => Some(0x45),
        KeyCode::ScrollLock => Some(0x46),
        KeyCode::Numpad0 => Some(0x52),
        KeyCode::Numpad1 => Some(0x4F),
        KeyCode::Numpad2 => Some(0x50),
        KeyCode::Numpad3 => Some(0x51),
        KeyCode::Numpad4 => Some(0x4B),
        KeyCode::Numpad5 => Some(0x4C),
        KeyCode::Numpad6 => Some(0x4D),
        KeyCode::Numpad7 => Some(0x47),
        KeyCode::Numpad8 => Some(0x48),
        KeyCode::Numpad9 => Some(0x49),
        KeyCode::NumpadSubtract => Some(0x4A),
        KeyCode::NumpadAdd => Some(0x4E),
        KeyCode::ArrowLeft => Some(0x4B),
        KeyCode::ArrowRight => Some(0x4D),
        KeyCode::ArrowUp => Some(0x48),
        KeyCode::ArrowDown => Some(0x50),
        _=>None
    }

}
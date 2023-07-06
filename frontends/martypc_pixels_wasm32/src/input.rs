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

use winit::event::VirtualKeyCode;

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


pub fn match_virtual_keycode( vkc: VirtualKeyCode ) -> Option<u8> {

    match vkc {
        // From Left to Right on IBM XT keyboard
        VirtualKeyCode::F1  => Some(0x3b),
        VirtualKeyCode::F2  => Some(0x3c),
        VirtualKeyCode::F3  => Some(0x3d),
        VirtualKeyCode::F4  => Some(0x3e),
        VirtualKeyCode::F5  => Some(0x3f),
        VirtualKeyCode::F6  => Some(0x40),
        VirtualKeyCode::F7  => Some(0x41),
        VirtualKeyCode::F8  => Some(0x42),
        VirtualKeyCode::F9  => Some(0x43),
        VirtualKeyCode::F10 => Some(0x44),

        VirtualKeyCode::Escape => Some(0x01),
        VirtualKeyCode::Tab => Some(0x0F),
        VirtualKeyCode::LControl => Some(0x1D),
        VirtualKeyCode::LShift => Some(0x2A),
        VirtualKeyCode::LAlt => Some(0x38),

        VirtualKeyCode::Key1 => Some(0x02),
        VirtualKeyCode::Key2 => Some(0x03),
        VirtualKeyCode::Key3 => Some(0x04),
        VirtualKeyCode::Key4 => Some(0x05),
        VirtualKeyCode::Key5 => Some(0x06),
        VirtualKeyCode::Key6 => Some(0x07),
        VirtualKeyCode::Key7 => Some(0x08),
        VirtualKeyCode::Key8 => Some(0x09),
        VirtualKeyCode::Key9 => Some(0x0A),
        VirtualKeyCode::Key0 => Some(0x0B),
        VirtualKeyCode::Minus => Some(0x0C),
        VirtualKeyCode::Equals => Some(0x0D),
        VirtualKeyCode::A => Some(0x1E),
        VirtualKeyCode::B => Some(0x30),
        VirtualKeyCode::C => Some(0x2E),
        VirtualKeyCode::D => Some(0x20),
        VirtualKeyCode::E => Some(0x12),
        VirtualKeyCode::F => Some(0x21),
        VirtualKeyCode::G => Some(0x22),
        VirtualKeyCode::H => Some(0x23),
        VirtualKeyCode::I => Some(0x17),
        VirtualKeyCode::J => Some(0x24),
        VirtualKeyCode::K => Some(0x25),
        VirtualKeyCode::L => Some(0x26),
        VirtualKeyCode::M => Some(0x32),
        VirtualKeyCode::N => Some(0x31),
        VirtualKeyCode::O => Some(0x18),
        VirtualKeyCode::P => Some(0x19),
        VirtualKeyCode::Q => Some(0x10),
        VirtualKeyCode::R => Some(0x13),
        VirtualKeyCode::S => Some(0x1F),
        VirtualKeyCode::T => Some(0x14),
        VirtualKeyCode::U => Some(0x16),
        VirtualKeyCode::V => Some(0x2F),
        VirtualKeyCode::W => Some(0x11),
        VirtualKeyCode::X => Some(0x2D),
        VirtualKeyCode::Y => Some(0x15),
        VirtualKeyCode::Z => Some(0x2C),

        VirtualKeyCode::Backslash => Some(0x2B),
        VirtualKeyCode::Space => Some(0x39),
        VirtualKeyCode::Back => Some(0x0E),
        VirtualKeyCode::LBracket => Some(0x1A),
        VirtualKeyCode::RBracket => Some(0x1B),
        VirtualKeyCode::Semicolon => Some(0x27),
        VirtualKeyCode::Grave => Some(0x29),
        VirtualKeyCode::Apostrophe => Some(0x28),

        VirtualKeyCode::Comma => Some(0x33),
        VirtualKeyCode::Period => Some(0x34),
        VirtualKeyCode::Slash => Some(0x35),
        VirtualKeyCode::Return => Some(0x1C),
        VirtualKeyCode::RShift => Some(0x36),
        VirtualKeyCode::Capital => Some(0x3A),
        VirtualKeyCode::Snapshot => Some(0x37),
        VirtualKeyCode::Insert => Some(0x52),
        VirtualKeyCode::Delete => Some(0x53),
        VirtualKeyCode::Numlock => Some(0x45),
        VirtualKeyCode::Scroll => Some(0x46),
        VirtualKeyCode::Numpad0 => Some(0x52),
        VirtualKeyCode::Numpad1 => Some(0x4F),
        VirtualKeyCode::Numpad2 => Some(0x50),
        VirtualKeyCode::Numpad3 => Some(0x51),
        VirtualKeyCode::Numpad4 => Some(0x4B),
        VirtualKeyCode::Numpad5 => Some(0x4C),
        VirtualKeyCode::Numpad6 => Some(0x4D),
        VirtualKeyCode::Numpad7 => Some(0x47),
        VirtualKeyCode::Numpad8 => Some(0x48),
        VirtualKeyCode::Numpad9 => Some(0x49),
        VirtualKeyCode::NumpadSubtract => Some(0x4A),
        VirtualKeyCode::NumpadAdd => Some(0x4E),
        
        VirtualKeyCode::Left => Some(0x4B),
        VirtualKeyCode::Right => Some(0x4D),
        VirtualKeyCode::Up => Some(0x48),
        VirtualKeyCode::Down => Some(0x50),
        _=>None
    }

}
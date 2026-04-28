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

    device_types::keyboard.rs

    Defines types common to implementations of a Hard Disk Controller
*/

use std::str::FromStr;

use crate::{
    device_traits::keyboard::MartyKeyboard,
    devices::keyboards::{model_f::ModelF, pcjr::PcJrKeyboard, tandy1000::Tandy1000Keyboard},
    keys::MartyKey,
};

use serde_derive::Deserialize;

// Define the various types of keyboard we can emulate.
#[derive(Copy, Clone, Debug, Deserialize, PartialEq)]
pub enum KeyboardType {
    ModelF,
    ModelM,
    Tandy1000,
    Pcjr,
}

impl KeyboardType {
    pub fn keycode_to_scancodes(&self, key_code: MartyKey) -> Vec<u8> {
        match self {
            KeyboardType::ModelF => ModelF::keycode_to_scancodes(key_code),
            KeyboardType::Tandy1000 => Tandy1000Keyboard::keycode_to_scancodes(key_code),
            KeyboardType::Pcjr => PcJrKeyboard::keycode_to_scancodes(key_code),
            _ => unimplemented!(),
        }
    }
}

impl FromStr for KeyboardType {
    type Err = String;
    fn from_str(s: &str) -> anyhow::Result<Self, String>
    where
        Self: Sized,
    {
        match s {
            "ModelF" => Ok(KeyboardType::ModelF),
            "ModelM" => Ok(KeyboardType::ModelM),
            "Tandy1000" => Ok(KeyboardType::Tandy1000),
            "PCjr" => Ok(KeyboardType::Pcjr),
            _ => Err("Bad value for keyboard_type".to_string()),
        }
    }
}

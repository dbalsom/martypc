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

   ---------------------------------------------------------------------------
*/
use std::{fmt::Display, str::FromStr};

use serde::Deserialize;

#[derive(Copy, Clone, Default, Debug, PartialEq, Hash, Deserialize)]
pub enum ControllerLayout {
    #[default]
    TwoJoysticksTwoButtons,
    OneJoystickFourButtons,
}

impl FromStr for ControllerLayout {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "twojoystickstwobuttons" => Ok(ControllerLayout::TwoJoysticksTwoButtons),
            "onejoystickfourbuttons" => Ok(ControllerLayout::OneJoystickFourButtons),
            _ => Err(format!(
                "Invalid controller layout: {}. Expected 'TwoJoysticksTwoButtons' or 'OneJoystickFourButtons'",
                s
            )),
        }
    }
}

impl Display for ControllerLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ControllerLayout::TwoJoysticksTwoButtons => write!(f, "Two Joysticks with Two Buttons"),
            ControllerLayout::OneJoystickFourButtons => write!(f, "One Joystick with Four Buttons"),
        }
    }
}

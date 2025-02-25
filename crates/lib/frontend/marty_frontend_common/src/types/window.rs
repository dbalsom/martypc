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

   ---------------------------------------------------------------------------
*/

//! [WindowDefinition] is shared between an implementation of [DisplayManager]
//! and the `marty_config` crate, to enable reading of a [WindowDefinition] from
//! a configuration file.

use marty_common::VideoDimensions;
use serde_derive::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct WindowDefinition {
    #[serde(default)]
    pub enabled: bool,
    pub name: String,
    pub background_color: Option<u32>,
    #[serde(default)]
    pub background: bool,
    pub size: Option<VideoDimensions>,
    #[serde(default)]
    pub resizable: bool,
    pub card_id: Option<usize>,
    pub card_scale: Option<f32>,
    #[serde(default)]
    pub always_on_top: bool,
    pub scaler_preset: Option<String>,
}

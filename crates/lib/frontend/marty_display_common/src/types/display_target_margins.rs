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

   frontend_common::types::display_target_margins.rs

   Define the DisplayTargetMargins type and methods.

*/

#[derive(Copy, Clone, Default, Debug)]
pub struct DisplayTargetMargins {
    pub l: u32,
    pub r: u32,
    pub t: u32,
    pub b: u32,
}

impl DisplayTargetMargins {
    pub fn from_t(t: u32) -> Self {
        Self {
            t,
            ..Default::default()
        }
    }
    pub fn from_b(b: u32) -> Self {
        Self {
            b,
            ..Default::default()
        }
    }
    pub fn from_l(l: u32) -> Self {
        Self {
            l,
            ..Default::default()
        }
    }
    pub fn from_r(r: u32) -> Self {
        Self {
            r,
            ..Default::default()
        }
    }
}

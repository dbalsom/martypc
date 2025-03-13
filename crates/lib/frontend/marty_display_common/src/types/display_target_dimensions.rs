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

//! Provide the [DisplayTargetDimensions] type.
//! We have several different types defined that are essentially a u32 tuple.
//! Perhaps we can consolidate these into a single `Vec2` type.

use marty_frontend_common::constants::*;

use crate::types::display_target_margins::DisplayTargetMargins;
use marty_common::VideoDimensions;

#[cfg(feature = "use_display")]
use display_backend_trait::BufferDimensions;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DisplayTargetDimensions {
    w: u32,
    h: u32,
}

impl Default for DisplayTargetDimensions {
    fn default() -> Self {
        Self {
            w: DEFAULT_RESOLUTION_W,
            h: DEFAULT_RESOLUTION_H,
        }
    }
}

impl DisplayTargetDimensions {
    pub fn new(w: u32, h: u32) -> Self {
        Self { w, h }
    }
}

impl DisplayTargetDimensions {
    #[allow(dead_code)]
    fn size_with_margins(&self, margins: DisplayTargetMargins) -> Self {
        Self {
            w: self.w + margins.l + margins.r,
            h: self.h + margins.t + margins.b,
        }
    }
}

impl From<VideoDimensions> for DisplayTargetDimensions {
    fn from(t: VideoDimensions) -> Self {
        DisplayTargetDimensions { w: t.w, h: t.h }
    }
}

#[cfg(feature = "use_display")]
impl From<DisplayTargetDimensions> for BufferDimensions {
    fn from(t: DisplayTargetDimensions) -> Self {
        BufferDimensions {
            w: t.w,
            h: t.h,
            pitch: t.w,
        }
    }
}

impl From<DisplayTargetDimensions> for VideoDimensions {
    fn from(t: DisplayTargetDimensions) -> Self {
        VideoDimensions { w: t.w, h: t.h }
    }
}

impl From<DisplayTargetDimensions> for (u32, u32) {
    fn from(t: DisplayTargetDimensions) -> (u32, u32) {
        (t.w, t.h)
    }
}

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

   common::util.rs

   Common emulator library.
   Define utility methods.
*/

use web_time::Duration;

/// Format the provided Duration using the most appropriate unit given the magnitude of the Duration.
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs_f64();
    let millis = secs * 1_000.0;
    let micros = millis * 1_000.0;
    let nanos = micros * 1_000.0;

    if nanos < 1_000.0 {
        format!("{:.0}ns", nanos)
    }
    else if micros < 1_000.0 {
        format!("{:.3}µs", micros)
    }
    else if millis < 1_000.0 {
        format!("{:.3}ms", millis)
    }
    else {
        format!("{:.3}s", secs)
    }
}

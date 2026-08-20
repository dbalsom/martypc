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
*/

pub const NTSC_CLOCK: f64 = 315.0 / 22.0;
pub const NTSC_HORIZ_REFRESH: f64 = NTSC_CLOCK / 912.0 * 1_000_000.0;
pub const NTSC_VERT_REFRESH: f64 = NTSC_CLOCK / (912.0 * 262.0) * 1_000_000.0;

pub enum VideoSyncPolarity {
    Negative,
    Positive,
}

pub struct VideoSyncState {
    pub vsync: bool,
    pub vsync_polarity: VideoSyncPolarity,
    pub hsync: bool,
    pub hsync_polarity: VideoSyncPolarity,
}

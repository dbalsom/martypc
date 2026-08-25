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

   ---------------------------------------------------------------------------
*/

//! Define frontend types for sound sources

/// [SoundSourceKind] describes the type of a given sound source.
/// MachineSounds are typically recorded audio files that are played back.
/// Emulated sounds produce samples on their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundSourceKind {
    // Samples machine sounds (floppy drive, etc.)
    MachineSounds,
    /// Emulated sound source (pc speaker, adlib, etc.)
    Emulated,
}

#[derive(Clone, Debug)]
pub struct SoundSourceInfo {
    pub kind: SoundSourceKind,
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_ct: u64,
    pub latency_ms: f32,
    pub volume: f32,
    pub muted: bool,
    pub len: usize,
}

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
*/

//! [AtaString] formats strings used in the ATA device identification structure.

use binrw::binrw;
use std::str::FromStr;

#[binrw]
#[derive(Debug, Default)]
pub struct AtaString<const N: usize> {
    #[br(count = N)]
    // On write, explicitly check length:
    #[bw(assert(raw.len() == N, "raw length must be N"))]
    raw: Vec<u8>,
}

impl<const N: usize> FromStr for AtaString<N> {
    type Err = std::string::FromUtf8Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // We want exactly N bytes.
        // Typically, N is a multiple of 2 (since these are 16-bit words).
        let mut buf = vec![b' '; N];
        let bytes = s.as_bytes();
        let len = bytes.len().min(N);
        buf[..len].clone_from_slice(&bytes[..len]);

        // Now swap each pair in place to match ATA’s weird
        // big-endian-within-each-16-bit-word requirement.
        for chunk in buf.chunks_mut(2) {
            chunk.swap(0, 1);
        }

        Ok(Self { raw: buf })
    }
}

impl<const N: usize> AtaString<N> {
    pub fn as_str(&self) -> String {
        // Reverse the swapping to get a normal ASCII string
        // if you ever want to read it back out.
        let mut unwrapped = self.raw.clone();
        for chunk in unwrapped.chunks_mut(2) {
            chunk.swap(0, 1);
        }
        // Then we can convert it to a Rust string (losing trailing spaces, etc.).
        String::from_utf8_lossy(&unwrapped).to_string()
    }
}

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

    bytebuf.rs

    Implements structured read/write routines from a buffer of bytes.

*/

#![allow(dead_code)]
#![allow(clippy::identity_op)]

use std::{error::Error, fmt::Display, fs::File, io::Read, mem::size_of};

#[derive(Debug)]
pub enum ByteBufError {
    ReadOutOfBoundsError,
    SeekOutOfBoundsError,
    FileReadError,
}
impl Error for ByteBufError {}
impl Display for ByteBufError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            ByteBufError::ReadOutOfBoundsError => {
                write!(f, "An attempt was made to read out of buffer bounds.")
            }
            ByteBufError::SeekOutOfBoundsError => {
                write!(f, "An attempt was made to move the buffer cursor out of bounds.")
            }
            ByteBufError::FileReadError => write!(f, "Error reading file into ByteBuf."),
        }
    }
}

pub struct ByteBuf {
    cursor: usize,
    vec:    Vec<u8>,
}

impl From<Vec<u8>> for ByteBuf {
    fn from(vec: Vec<u8>) -> Self {
        Self { cursor: 0, vec }
    }
}

impl From<&[u8]> for ByteBuf {
    fn from(slice: &[u8]) -> Self {
        Self::from(slice.to_vec())
    }
}

impl ByteBuf {
    // Create a new, 0-initialized ByteBuf of the specified length
    pub fn new(size: usize) -> ByteBuf {
        ByteBuf {
            cursor: 0,
            vec:    vec![0; size],
        }
    }

    pub fn from_file(mut file: File, size: usize) -> Result<ByteBuf, ByteBufError> {
        let mut buffer = Vec::new();

        file.read_to_end(&mut buffer).map_err(|_| ByteBufError::FileReadError)?;
        buffer.resize(size, 0u8);
        Ok(ByteBuf {
            cursor: 0,
            vec:    buffer,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.vec.is_empty()
    }

    pub fn len(&self) -> usize {
        self.vec.len()
    }

    pub fn tell(&self) -> usize {
        self.cursor
    }

    pub fn seek(&mut self, disp: usize) -> Result<(), ByteBufError> {
        if disp >= self.vec.len() {
            return Err(ByteBufError::SeekOutOfBoundsError);
        }
        self.cursor = disp;
        Ok(())
    }

    pub fn seek_back(&mut self, disp: usize) -> Result<(), ByteBufError> {
        if disp > self.cursor {
            return Err(ByteBufError::SeekOutOfBoundsError);
        }
        self.cursor -= disp;
        Ok(())
    }

    pub fn seek_fwd(&mut self, disp: usize) -> Result<(), ByteBufError> {
        if disp == 0 {
            return Ok(());
        }
        if self.cursor + disp >= self.vec.len() {
            return Err(ByteBufError::SeekOutOfBoundsError);
        }
        self.cursor += disp;
        Ok(())
    }

    /// Copy 'len' bytes from buffer into destination
    pub fn read_bytes(&mut self, dest: &mut [u8], len: usize) -> Result<(), ByteBufError> {
        if self.cursor + len <= self.vec.len() && len <= dest.len() {
            for byte in dest.iter_mut().take(len) {
                *byte = self.vec[self.cursor];
                self.cursor += 1;
            }
            return Ok(());
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Copy bytes into the buffer from the source slice
    pub fn write_bytes(&mut self, src: &[u8], len: usize) -> Result<(), ByteBufError> {
        if self.cursor + len <= self.vec.len() && len <= src.len() {
            for byte in src.iter().take(len) {
                self.vec[self.cursor] = *byte;
                self.cursor += 1;
            }
            return Ok(());
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Read a u8 from the buffer
    pub fn read_u8(&mut self) -> Result<u8, ByteBufError> {
        if self.cursor < self.vec.len() {
            let b: u8 = self.vec[self.cursor];
            self.cursor += 1;
            return Ok(b);
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Read an i8 from the buffer
    pub fn read_i8(&mut self) -> Result<i8, ByteBufError> {
        if self.cursor < self.vec.len() {
            let b: i8 = self.vec[self.cursor] as i8;
            self.cursor += 1;
            return Ok(b);
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Write a u8 to the buffer
    pub fn write_u8(&mut self, b: u8) -> Result<(), ByteBufError> {
        if self.cursor < self.vec.len() {
            self.vec[self.cursor] = b;
            self.cursor += 1;
            return Ok(());
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Read a u16 in little endian order.
    pub fn read_u16_le(&mut self) -> Result<u16, ByteBufError> {
        if self.cursor + size_of::<u16>() <= self.vec.len() {
            let w: u16 = self.vec[self.cursor] as u16 | (self.vec[self.cursor + 1] as u16) << 8;
            self.cursor += size_of::<u16>();
            return Ok(w);
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Read an i16 in little endian order
    pub fn read_i16_le(&mut self) -> Result<i16, ByteBufError> {
        if self.cursor + size_of::<u16>() <= self.vec.len() {
            let w: i16 = (self.vec[self.cursor] as u16 | (self.vec[self.cursor + 1] as u16) << 8) as i16;
            self.cursor += size_of::<u16>();
            return Ok(w);
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Write a u16 in little endian order.
    pub fn write_u16_le(&mut self, w: u16) -> Result<(), ByteBufError> {
        if self.cursor + size_of::<u16>() <= self.vec.len() {
            self.vec[self.cursor] = (w & 0x00FF) as u8;
            self.vec[self.cursor + 1] = (w >> 8) as u8;
            self.cursor += size_of::<u16>();
            return Ok(());
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Read a u16 in big endian order.
    pub fn read_u16_be(&mut self) -> Result<u16, ByteBufError> {
        if self.cursor + size_of::<u16>() <= self.vec.len() {
            let w: u16 = (self.vec[self.cursor] as u16) << 8 | self.vec[self.cursor + 1] as u16;
            self.cursor += size_of::<u16>();
            return Ok(w);
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    // Read an i16 in big endian order
    pub fn read_i16_be(&mut self) -> Result<i16, ByteBufError> {
        if self.cursor + size_of::<u16>() <= self.vec.len() {
            let w: i16 = ((self.vec[self.cursor] as u16) << 8 | self.vec[self.cursor + 1] as u16) as i16;
            self.cursor += size_of::<u16>();
            return Ok(w);
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Write a u16 in big endian order.
    pub fn write_u16_be(&mut self, w: u16) -> Result<(), ByteBufError> {
        if self.cursor + size_of::<u16>() <= self.vec.len() {
            self.vec[self.cursor] = (w >> 8) as u8;
            self.vec[self.cursor + 1] = (w & 0x00FF) as u8;
            self.cursor += size_of::<u16>();
            return Ok(());
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Read a u32 in little endian order.
    pub fn read_u32_le(&mut self) -> Result<u32, ByteBufError> {
        if self.cursor + size_of::<u32>() <= self.vec.len() {
            let dw: u32 = (self.vec[self.cursor] as u32)
                | (self.vec[self.cursor + 1] as u32) << 8
                | (self.vec[self.cursor + 2] as u32) << 16
                | (self.vec[self.cursor + 3] as u32) << 24;
            self.cursor += size_of::<u32>();
            return Ok(dw);
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Read a i32 in little endian order.
    pub fn read_i32_le(&mut self) -> Result<i32, ByteBufError> {
        if self.cursor + size_of::<u32>() <= self.vec.len() {
            let dw: u32 = (self.vec[self.cursor] as u32)
                | (self.vec[self.cursor + 1] as u32) << 8
                | (self.vec[self.cursor + 2] as u32) << 16
                | (self.vec[self.cursor + 3] as u32) << 24;
            self.cursor += size_of::<u32>();
            return Ok(dw as i32);
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Read a u32 in big endian order.
    pub fn read_u32_be(&mut self) -> Result<u32, ByteBufError> {
        if self.cursor + size_of::<u32>() <= self.vec.len() {
            let dw: u32 = (self.vec[self.cursor] as u32) << 24
                | (self.vec[self.cursor + 1] as u32) << 16
                | (self.vec[self.cursor + 2] as u32) << 8
                | (self.vec[self.cursor + 3] as u32);
            self.cursor += size_of::<u32>();
            return Ok(dw);
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Write a u32 in little endian order.
    pub fn write_u32_le(&mut self, dw: u32) -> Result<(), ByteBufError> {
        if self.cursor + size_of::<u32>() <= self.vec.len() {
            self.vec[self.cursor + 0] = (dw & 0xFF) as u8;
            self.vec[self.cursor + 1] = (dw >> 8 & 0xFF) as u8;
            self.vec[self.cursor + 2] = (dw >> 16 & 0xFF) as u8;
            self.vec[self.cursor + 3] = (dw >> 24 & 0xFF) as u8;
            self.cursor += size_of::<u32>();
            return Ok(());
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Write a u32 in big endian order.
    pub fn write_u32_be(&mut self, dw: u32) -> Result<(), ByteBufError> {
        if self.cursor + size_of::<u32>() <= self.vec.len() {
            self.vec[self.cursor + 0] = (dw >> 24 & 0xFF) as u8;
            self.vec[self.cursor + 1] = (dw >> 16 & 0xFF) as u8;
            self.vec[self.cursor + 2] = (dw >> 8 & 0xFF) as u8;
            self.vec[self.cursor + 3] = (dw & 0xFF) as u8;
            self.cursor += size_of::<u32>();
            return Ok(());
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Read a u64 in big endian order.
    pub fn read_u64_be(&mut self) -> Result<u64, ByteBufError> {
        if self.cursor + size_of::<u64>() <= self.vec.len() {
            let ddw: u64 = (self.vec[self.cursor] as u64) << 56
                | (self.vec[self.cursor + 1] as u64) << 48
                | (self.vec[self.cursor + 2] as u64) << 40
                | (self.vec[self.cursor + 3] as u64) << 32
                | (self.vec[self.cursor + 4] as u64) << 24
                | (self.vec[self.cursor + 5] as u64) << 16
                | (self.vec[self.cursor + 6] as u64) << 8
                | (self.vec[self.cursor + 7] as u64);
            self.cursor += size_of::<u64>();
            return Ok(ddw);
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Write a u64 in big endian order.
    pub fn write_u64_be(&mut self, ddw: u64) -> Result<(), ByteBufError> {
        if self.cursor + size_of::<u64>() <= self.vec.len() {
            self.vec[self.cursor + 0] = (ddw >> 56 & 0xFF) as u8;
            self.vec[self.cursor + 1] = (ddw >> 48 & 0xFF) as u8;
            self.vec[self.cursor + 2] = (ddw >> 40 & 0xFF) as u8;
            self.vec[self.cursor + 3] = (ddw >> 32 & 0xFF) as u8;
            self.vec[self.cursor + 4] = (ddw >> 24 & 0xFF) as u8;
            self.vec[self.cursor + 5] = (ddw >> 16 & 0xFF) as u8;
            self.vec[self.cursor + 6] = (ddw >> 8 & 0xFF) as u8;
            self.vec[self.cursor + 7] = (ddw & 0xFF) as u8;
            self.cursor += size_of::<u64>();
            return Ok(());
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }
}

pub struct ByteBufWriter<'a> {
    cursor: usize,
    buf:    &'a mut [u8],
}

impl<'a> ByteBufWriter<'a> {
    pub fn from_slice(buf: &mut [u8]) -> ByteBufWriter<'_> {
        ByteBufWriter { cursor: 0, buf }
    }

    pub fn take(&mut self) -> &mut [u8] {
        self.buf
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn tell(&self) -> usize {
        self.cursor
    }

    pub fn seek(&mut self, disp: usize) -> Result<(), ByteBufError> {
        if disp >= self.buf.len() {
            return Err(ByteBufError::SeekOutOfBoundsError);
        }
        self.cursor = disp;
        Ok(())
    }

    pub fn seek_back(&mut self, disp: usize) -> Result<(), ByteBufError> {
        if disp > self.cursor {
            return Err(ByteBufError::SeekOutOfBoundsError);
        }
        self.cursor -= disp;
        Ok(())
    }

    pub fn seek_fwd(&mut self, disp: usize) -> Result<(), ByteBufError> {
        if disp == 0 {
            return Ok(());
        }
        if self.cursor + disp >= self.buf.len() {
            return Err(ByteBufError::SeekOutOfBoundsError);
        }
        self.cursor += disp;
        Ok(())
    }

    /// Copy bytes into the buffer from the source slice
    pub fn write_bytes(&mut self, src: &[u8], len: usize) -> Result<(), ByteBufError> {
        if self.cursor + len <= self.buf.len() && len <= src.len() {
            for &byte in src.iter().take(len) {
                self.buf[self.cursor] = byte;
                self.cursor += 1;
            }
            return Ok(());
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Write a u8 to the buffer
    pub fn write_u8(&mut self, b: u8) -> Result<(), ByteBufError> {
        if self.cursor < self.buf.len() {
            self.buf[self.cursor] = b;
            self.cursor += 1;
            return Ok(());
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Write a u16 in little endian order.
    pub fn write_u16_le(&mut self, w: u16) -> Result<(), ByteBufError> {
        if self.cursor + size_of::<u16>() <= self.buf.len() {
            self.buf[self.cursor] = (w & 0x00FF) as u8;
            self.buf[self.cursor + 1] = (w >> 8) as u8;
            self.cursor += size_of::<u16>();
            return Ok(());
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Write a u16 in big endian order.
    pub fn write_u16_be(&mut self, w: u16) -> Result<(), ByteBufError> {
        if self.cursor + size_of::<u16>() <= self.buf.len() {
            self.buf[self.cursor] = (w >> 8) as u8;
            self.buf[self.cursor + 1] = (w & 0x00FF) as u8;
            self.cursor += size_of::<u16>();
            return Ok(());
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Write a u32 in little endian order.
    pub fn write_u32_le(&mut self, dw: u32) -> Result<(), ByteBufError> {
        if self.cursor + size_of::<u32>() <= self.buf.len() {
            self.buf[self.cursor + 0] = (dw & 0xFF) as u8;
            self.buf[self.cursor + 1] = (dw >> 8 & 0xFF) as u8;
            self.buf[self.cursor + 2] = (dw >> 16 & 0xFF) as u8;
            self.buf[self.cursor + 3] = (dw >> 24 & 0xFF) as u8;
            self.cursor += size_of::<u32>();
            return Ok(());
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Write a u32 in big endian order.
    pub fn write_u32_be(&mut self, dw: u32) -> Result<(), ByteBufError> {
        if self.cursor + size_of::<u32>() <= self.buf.len() {
            self.buf[self.cursor + 0] = (dw >> 24 & 0xFF) as u8;
            self.buf[self.cursor + 1] = (dw >> 16 & 0xFF) as u8;
            self.buf[self.cursor + 2] = (dw >> 8 & 0xFF) as u8;
            self.buf[self.cursor + 3] = (dw & 0xFF) as u8;
            self.cursor += size_of::<u32>();
            return Ok(());
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Write a u64 in big endian order.
    pub fn write_u64_be(&mut self, ddw: u64) -> Result<(), ByteBufError> {
        if self.cursor + size_of::<u64>() <= self.buf.len() {
            self.buf[self.cursor + 0] = (ddw >> 56 & 0xFF) as u8;
            self.buf[self.cursor + 1] = (ddw >> 48 & 0xFF) as u8;
            self.buf[self.cursor + 2] = (ddw >> 40 & 0xFF) as u8;
            self.buf[self.cursor + 3] = (ddw >> 32 & 0xFF) as u8;
            self.buf[self.cursor + 4] = (ddw >> 24 & 0xFF) as u8;
            self.buf[self.cursor + 5] = (ddw >> 16 & 0xFF) as u8;
            self.buf[self.cursor + 6] = (ddw >> 8 & 0xFF) as u8;
            self.buf[self.cursor + 7] = (ddw & 0xFF) as u8;
            self.cursor += size_of::<u64>();
            return Ok(());
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! bytebuf_short_access_test {
        ($name:ident, $ty:ty, $method:ident $(, $arg:expr)*) => {
            #[test]
            fn $name() {
                let width = size_of::<$ty>();
                for available in 0..=width {
                    for prefix in [0, 1] {
                        let original = vec![0xA5; prefix + available];
                        let mut buf = ByteBuf::from(original.as_slice());
                        if prefix != 0 {
                            buf.read_u8().unwrap();
                        }

                        let result = buf.$method($($arg),*);
                        if available < width {
                            assert!(
                                matches!(result, Err(ByteBufError::ReadOutOfBoundsError)),
                                "{} with {available} bytes remaining at cursor {prefix}: {result:?}",
                                stringify!($method),
                            );
                            assert_eq!(buf.tell(), prefix);
                            assert_eq!(buf.vec, original);
                        }
                        else {
                            assert!(result.is_ok(), "{}: {result:?}", stringify!($method));
                            assert_eq!(buf.tell(), prefix + width);
                        }
                    }
                }
            }
        };
    }

    macro_rules! writer_short_access_test {
        ($name:ident, $ty:ty, $method:ident, $value:expr) => {
            #[test]
            fn $name() {
                let width = size_of::<$ty>();
                for available in 0..=width {
                    for prefix in [0, 1] {
                        let original = vec![0xA5; prefix + available];
                        let mut storage = original.clone();
                        let mut buf = ByteBufWriter::from_slice(&mut storage);
                        if prefix != 0 {
                            buf.write_u8(0xA5).unwrap();
                        }

                        let result = buf.$method($value);
                        if available < width {
                            assert!(
                                matches!(result, Err(ByteBufError::ReadOutOfBoundsError)),
                                "{} with {available} bytes remaining at cursor {prefix}: {result:?}",
                                stringify!($method),
                            );
                            assert_eq!(buf.tell(), prefix);
                            assert_eq!(buf.take(), original.as_slice());
                        }
                        else {
                            assert!(result.is_ok(), "{}: {result:?}", stringify!($method));
                            assert_eq!(buf.tell(), prefix + width);
                        }
                    }
                }
            }
        };
    }

    bytebuf_short_access_test!(read_u8_short_buffer, u8, read_u8);
    bytebuf_short_access_test!(read_i8_short_buffer, i8, read_i8);
    bytebuf_short_access_test!(read_u16_le_short_buffer, u16, read_u16_le);
    bytebuf_short_access_test!(read_i16_le_short_buffer, i16, read_i16_le);
    bytebuf_short_access_test!(read_u16_be_short_buffer, u16, read_u16_be);
    bytebuf_short_access_test!(read_i16_be_short_buffer, i16, read_i16_be);
    bytebuf_short_access_test!(read_u32_le_short_buffer, u32, read_u32_le);
    bytebuf_short_access_test!(read_i32_le_short_buffer, i32, read_i32_le);
    bytebuf_short_access_test!(read_u32_be_short_buffer, u32, read_u32_be);
    bytebuf_short_access_test!(read_u64_be_short_buffer, u64, read_u64_be);

    bytebuf_short_access_test!(write_u8_short_buffer, u8, write_u8, 0x12);
    bytebuf_short_access_test!(write_u16_le_short_buffer, u16, write_u16_le, 0x1234);
    bytebuf_short_access_test!(write_u16_be_short_buffer, u16, write_u16_be, 0x1234);
    bytebuf_short_access_test!(write_u32_le_short_buffer, u32, write_u32_le, 0x12345678);
    bytebuf_short_access_test!(write_u32_be_short_buffer, u32, write_u32_be, 0x12345678);
    bytebuf_short_access_test!(write_u64_be_short_buffer, u64, write_u64_be, 0x0123456789ABCDEF);

    writer_short_access_test!(writer_u8_short_buffer, u8, write_u8, 0x12);
    writer_short_access_test!(writer_u16_le_short_buffer, u16, write_u16_le, 0x1234);
    writer_short_access_test!(writer_u16_be_short_buffer, u16, write_u16_be, 0x1234);
    writer_short_access_test!(writer_u32_le_short_buffer, u32, write_u32_le, 0x12345678);
    writer_short_access_test!(writer_u32_be_short_buffer, u32, write_u32_be, 0x12345678);
    writer_short_access_test!(writer_u64_be_short_buffer, u64, write_u64_be, 0x0123456789ABCDEF);

    #[test]
    fn read_bytes_short_buffer() {
        for requested in [1, 2, 4, 8] {
            for available in 0..requested {
                for prefix in [0, 1] {
                    let mut buf = ByteBuf::from(vec![0xA5; prefix + available]);
                    if prefix != 0 {
                        buf.read_u8().unwrap();
                    }
                    let mut dest = vec![0xCC; requested];

                    assert!(matches!(
                        buf.read_bytes(&mut dest, requested),
                        Err(ByteBufError::ReadOutOfBoundsError)
                    ));
                    assert_eq!(buf.tell(), prefix);
                    assert_eq!(dest, vec![0xCC; requested]);
                    assert_eq!(buf.vec, vec![0xA5; prefix + available]);
                }
            }
        }
    }

    #[test]
    fn write_bytes_short_buffer() {
        for requested in [1, 2, 4, 8] {
            for available in 0..requested {
                for prefix in [0, 1] {
                    let mut buf = ByteBuf::from(vec![0xA5; prefix + available]);
                    if prefix != 0 {
                        buf.read_u8().unwrap();
                    }

                    assert!(matches!(
                        buf.write_bytes(&[0x12; 8], requested),
                        Err(ByteBufError::ReadOutOfBoundsError)
                    ));
                    assert_eq!(buf.tell(), prefix);
                    assert_eq!(buf.vec, vec![0xA5; prefix + available]);
                }
            }
        }
    }

    #[test]
    fn read_bytes_rejects_short_destination() {
        const REQUESTED: usize = 4;

        for prefix in [0, 1] {
            for dest_len in 0..REQUESTED {
                let original = vec![0xA5; prefix + REQUESTED];
                let mut buf = ByteBuf::from(original.as_slice());
                buf.seek(prefix).unwrap();
                let mut dest = vec![0xCC; dest_len];

                let result = buf.read_bytes(&mut dest, REQUESTED);
                assert!(
                    matches!(result, Err(ByteBufError::ReadOutOfBoundsError)),
                    "requested {REQUESTED} bytes into a {dest_len}-byte destination at cursor {prefix}: {result:?}",
                );
                assert_eq!(buf.tell(), prefix);
                assert_eq!(dest, vec![0xCC; dest_len]);
                assert_eq!(buf.vec, original);
            }
        }
    }

    #[test]
    fn write_bytes_rejects_short_source() {
        const REQUESTED: usize = 4;

        for prefix in [0, 1] {
            for src_len in 0..REQUESTED {
                let original = vec![0xA5; prefix + REQUESTED];
                let mut buf = ByteBuf::from(original.as_slice());
                buf.seek(prefix).unwrap();
                let src = vec![0x12; src_len];

                let result = buf.write_bytes(&src, REQUESTED);
                assert!(
                    matches!(result, Err(ByteBufError::ReadOutOfBoundsError)),
                    "requested {REQUESTED} bytes from a {src_len}-byte source at cursor {prefix}: {result:?}",
                );
                assert_eq!(buf.tell(), prefix);
                assert_eq!(buf.vec, original);
            }
        }
    }

    #[test]
    fn writer_write_bytes_short_buffer() {
        for requested in [1, 2, 4, 8] {
            for available in 0..requested {
                for prefix in [0, 1] {
                    let mut storage = vec![0xA5; prefix + available];
                    let mut buf = ByteBufWriter::from_slice(&mut storage);
                    if prefix != 0 {
                        buf.write_u8(0xA5).unwrap();
                    }

                    assert!(matches!(
                        buf.write_bytes(&[0x12; 8], requested),
                        Err(ByteBufError::ReadOutOfBoundsError)
                    ));
                    assert_eq!(buf.tell(), prefix);
                    assert_eq!(buf.take(), vec![0xA5; prefix + available].as_slice());
                }
            }
        }
    }

    #[test]
    fn seek_empty_buffer() {
        let mut buf = ByteBuf::new(0);
        for disp in [0, 1] {
            assert!(matches!(buf.seek(disp), Err(ByteBufError::SeekOutOfBoundsError)));
            assert_eq!(buf.tell(), 0);
        }
    }

    #[test]
    fn seek_fwd_empty_buffer() {
        let mut buf = ByteBuf::new(0);
        buf.seek_fwd(0).unwrap();
        assert_eq!(buf.tell(), 0);
        assert!(matches!(buf.seek_fwd(1), Err(ByteBufError::SeekOutOfBoundsError)));
        assert_eq!(buf.tell(), 0);
    }

    #[test]
    fn seek_fwd_zero_is_noop_at_every_cursor() {
        let original = [0xA5; 4];
        let mut buf = ByteBuf::from(original.as_slice());
        for cursor in 0..=original.len() {
            buf.seek_fwd(0).unwrap();
            assert_eq!(buf.tell(), cursor);
            assert_eq!(buf.vec, original);
            if cursor < original.len() {
                buf.read_u8().unwrap();
            }
        }
        assert!(matches!(buf.seek_fwd(1), Err(ByteBufError::SeekOutOfBoundsError)));
        assert_eq!(buf.tell(), original.len());
    }

    #[test]
    fn writer_seek_empty_buffer() {
        let mut storage = [];
        let mut buf = ByteBufWriter::from_slice(&mut storage);
        for disp in [0, 1] {
            assert!(matches!(buf.seek(disp), Err(ByteBufError::SeekOutOfBoundsError)));
            assert_eq!(buf.tell(), 0);
        }
    }

    #[test]
    fn writer_seek_fwd_empty_buffer() {
        let mut storage = [];
        let mut buf = ByteBufWriter::from_slice(&mut storage);
        buf.seek_fwd(0).unwrap();
        assert_eq!(buf.tell(), 0);
        assert!(matches!(buf.seek_fwd(1), Err(ByteBufError::SeekOutOfBoundsError)));
        assert_eq!(buf.tell(), 0);
    }

    #[test]
    fn writer_seek_fwd_zero_is_noop_at_every_cursor() {
        let original = [0xA5; 4];
        let mut storage = original;
        let mut buf = ByteBufWriter::from_slice(&mut storage);
        for cursor in 0..=original.len() {
            buf.seek_fwd(0).unwrap();
            assert_eq!(buf.tell(), cursor);
            assert_eq!(buf.take(), original.as_slice());
            if cursor < original.len() {
                buf.write_u8(0xA5).unwrap();
            }
        }
        assert!(matches!(buf.seek_fwd(1), Err(ByteBufError::SeekOutOfBoundsError)));
        assert_eq!(buf.tell(), original.len());
    }

    #[test]
    fn seek_back_before_start() {
        for len in [0, 4] {
            for cursor in 0..=len {
                let mut buf = ByteBuf::new(len);
                for _ in 0..cursor {
                    buf.read_u8().unwrap();
                }

                assert!(matches!(
                    buf.seek_back(cursor + 1),
                    Err(ByteBufError::SeekOutOfBoundsError)
                ));
                assert_eq!(buf.tell(), cursor);
                buf.seek_back(cursor).unwrap();
                assert_eq!(buf.tell(), 0);
            }
        }
    }

    #[test]
    fn writer_seek_back_before_start() {
        for len in [0, 4] {
            for cursor in 0..=len {
                let mut storage = vec![0xA5; len];
                let mut buf = ByteBufWriter::from_slice(&mut storage);
                for _ in 0..cursor {
                    buf.write_u8(0xA5).unwrap();
                }

                assert!(matches!(
                    buf.seek_back(cursor + 1),
                    Err(ByteBufError::SeekOutOfBoundsError)
                ));
                assert_eq!(buf.tell(), cursor);
                buf.seek_back(cursor).unwrap();
                assert_eq!(buf.tell(), 0);
                assert_eq!(buf.take(), vec![0xA5; len].as_slice());
            }
        }
    }

    #[test]
    fn test_16() {
        let array: [u8; 16] = [0; 16];

        let mut buf = ByteBuf::from(array.as_slice());

        let a1: u16 = 0x0102;
        let a2: u16 = 0x0304;
        let a3: i16 = -1;
        let a4: i16 = -1234;

        let a5: u16 = 0x1234;
        let a6: u16 = 0x4321;
        let a7: i16 = -1;
        let a8: i16 = -30000;

        buf.write_u16_le(a1).unwrap();
        buf.write_u16_le(a2).unwrap();
        buf.write_u16_le(a3 as u16).unwrap();
        buf.write_u16_le(a4 as u16).unwrap();

        buf.write_u16_be(a5).unwrap();
        buf.write_u16_be(a6).unwrap();
        buf.write_u16_be(a7 as u16).unwrap();
        buf.write_u16_be(a8 as u16).unwrap();

        assert_eq!(buf.tell(), 16);
        buf.seek_back(16).unwrap();

        let b1 = buf.read_u16_le().unwrap();
        let b2 = buf.read_u16_le().unwrap();
        let b3 = buf.read_i16_le().unwrap();
        let b4 = buf.read_i16_le().unwrap();

        let b5 = buf.read_u16_be().unwrap();
        let b6 = buf.read_u16_be().unwrap();
        let b7 = buf.read_i16_be().unwrap();
        let b8 = buf.read_i16_be().unwrap();

        assert_eq!(a1, b1);
        assert_eq!(a2, b2);
        assert_eq!(a3, b3);
        assert_eq!(a4, b4);
        assert_eq!(a5, b5);
        assert_eq!(a6, b6);
        assert_eq!(a7, b7);
        assert_eq!(a8, b8);
    }

    #[test]
    fn test_32() {
        let array: [u8; 32] = [0; 32];

        let mut buf = ByteBuf::from(array.as_slice());

        let a1: u32 = 0x01020304;
        let a2: u32 = 0x04030201;
        let a3: i32 = i32::MAX;
        let a4: i32 = i32::MIN;

        buf.write_u32_le(a1).unwrap();
        buf.write_u32_le(a2).unwrap();
        buf.write_u32_le(a3 as u32).unwrap();
        buf.write_u32_le(a4 as u32).unwrap();
        buf.seek_back(16).unwrap();

        let b1 = buf.read_u32_le().unwrap();
        let b2 = buf.read_u32_le().unwrap();
        let b3 = buf.read_i32_le().unwrap();
        let b4 = buf.read_i32_le().unwrap();

        assert_eq!(a1, b1);
        assert_eq!(a2, b2);
        assert_eq!(a3, b3);
        assert_eq!(a4, b4);
    }
}

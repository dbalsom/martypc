/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

    memerror.rs

    Defines the Memory Error enum.
*/


#![allow(dead_code)]
use std::error::Error;
use core::fmt::Display;

#[derive(Debug)]
pub enum MemError {
    ReadOutOfBoundsError,
    SeekOutOfBoundsError,
    FileReadError,
    MmioError,
}
impl Error for MemError {}
impl Display for MemError{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            MemError::ReadOutOfBoundsError => write!(f, "An attempt was made to read out of buffer bounds."),
            MemError::SeekOutOfBoundsError => write!(f, "An attempt was made to move the buffer cursor out of bounds."),
            MemError::FileReadError => write!(f, "Error reading file into MemBuf."),
            MemError::MmioError => write!(f, "Error accessing map for memory mapped device.")
        }
    }
}
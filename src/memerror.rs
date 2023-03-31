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
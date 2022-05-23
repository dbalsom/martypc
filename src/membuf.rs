#![allow(dead_code)]

use std::fmt::Display;
use std::error::Error;
use std::fs::File;
use std::io::Read;

#[derive(Debug)]
pub enum MemBufError {
    ReadOutOfBoundsError,
    SeekOutOfBoundsError,
    FileReadError,
}
impl Error for MemBufError {}
impl Display for MemBufError{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            MemBufError::ReadOutOfBoundsError=>write!(f, "An attempt was made to read out of buffer bounds."),
            MemBufError::SeekOutOfBoundsError=>write!(f, "An attempt was made to move the buffer cursor out of bounds."),
            MemBufError::FileReadError=>write!(f, "Error reading file into MemBuf.")
        }
    }
}
pub struct MemBuf {
    size: usize,
    cursor: usize,
    vec: Vec<u8>,
    rom: bool
}

impl MemBuf {
    pub fn from_vec(vec: &Vec<u8>, size: usize, rom: bool) -> MemBuf {
        MemBuf {
            size: size,
            cursor: 0,
            vec: vec.to_vec(),
            rom: rom
        }
    }
    pub fn from_file(mut file: File, size: usize, rom: bool) -> Result<MemBuf, MemBufError> {
        let mut buffer = Vec::new();

        let bytes_read = file.read_to_end(&mut buffer).unwrap();
        buffer.resize(size, 0u8);
        Ok(MemBuf {
            size: size,
            cursor: 0,
            vec: buffer,
            rom: rom
        })
    }
    pub fn len(&self) -> usize {
        self.vec.len()
    }
    pub fn tell(&self) -> usize {
        self.cursor
    }
    pub fn seek(&mut self, disp: usize) -> Result<(), MemBufError> {
        if disp > self.vec.len() - 1 {
            return Err(MemBufError::SeekOutOfBoundsError)
        }
        self.cursor = disp;
        return Ok(())
    }
    pub fn seek_back(&mut self, disp: usize) -> Result<(), MemBufError> {
        if disp > self.cursor {
            return Err(MemBufError::SeekOutOfBoundsError)
        }
        self.cursor = self.cursor - disp;
        return Ok(())
    }
    pub fn seek_fwd(&mut self, disp: usize) -> Result<(), MemBufError> {
        if self.cursor + disp > self.vec.len() - 1 {
            return Err(MemBufError::SeekOutOfBoundsError)
        }
        self.cursor = self.cursor + disp;
        return Ok(())
    }
    pub fn read_u8(&self) -> Result<u8, MemBufError> {
        if self.cursor < self.vec.len() {
            let b: u8 = self.vec[self.cursor];
            return Ok(b)
        }
        Err(MemBufError::ReadOutOfBoundsError)
    }
    pub fn read_u8s(&mut self) -> Result<u8, MemBufError> {
        if self.cursor < self.vec.len() {
            let b: u8 = self.vec[self.cursor];
            self.cursor = self.cursor + 1;
            return Ok(b)
        }
        Err(MemBufError::ReadOutOfBoundsError)
    }
    pub fn read_i8(&self) -> Result<i8, MemBufError> {
        if self.cursor < self.vec.len() {
            let b: i8 = self.vec[self.cursor] as i8;
            return Ok(b)
        }
        Err(MemBufError::ReadOutOfBoundsError)
    }   
    pub fn read_i8s(&mut self) -> Result<i8, MemBufError> {
        if self.cursor < self.vec.len() {
            let b: i8 = self.vec[self.cursor] as i8;
            self.cursor = self.cursor + 1;
            return Ok(b)
        }
        Err(MemBufError::ReadOutOfBoundsError)
    }  
    pub fn read_u16(&self) -> Result<u16, MemBufError> {
        if self.cursor < self.vec.len() - 1 {
            let w: u16 = self.vec[self.cursor] as u16 | (self.vec[self.cursor+1] as u16) << 8;
            return Ok(w)
        }
        Err(MemBufError::ReadOutOfBoundsError)
    }
    pub fn read_u16s(&mut self) -> Result<u16, MemBufError> {
        if self.cursor < self.vec.len() - 1 {
            let w: u16 = self.vec[self.cursor] as u16 | (self.vec[self.cursor+1] as u16) << 8;
            self.cursor = self.cursor + 2;
            return Ok(w)
        }
        Err(MemBufError::ReadOutOfBoundsError)
    }
    pub fn read_i16(&self) -> Result<i16, MemBufError> {
        if self.cursor < self.vec.len() - 1 {
            let w: i16 = (self.vec[self.cursor] as u16 | (self.vec[self.cursor+1] as u16) << 8) as i16;
            return Ok(w)
        }
        Err(MemBufError::ReadOutOfBoundsError)
    }   
    pub fn read_i16s(&mut self) -> Result<i16, MemBufError> {
        if self.cursor < self.vec.len() - 1 {
            let w: i16 = (self.vec[self.cursor] as u16 | (self.vec[self.cursor+1] as u16) << 8) as i16;
            self.cursor = self.cursor + 2;
            return Ok(w)
        }
        Err(MemBufError::ReadOutOfBoundsError)
    }   
}
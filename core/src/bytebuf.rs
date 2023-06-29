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

    bytebuf.rs

    Implements structured read/write routines from a buffer of bytes.

*/

#![allow(dead_code)]

use std::fmt::Display;
use std::error::Error;
use std::fs::File;
use std::io::Read;

#[derive(Debug)]
pub enum ByteBufError {
    ReadOutOfBoundsError,
    SeekOutOfBoundsError,
    FileReadError,
}
impl Error for ByteBufError {}
impl Display for ByteBufError{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            ByteBufError::ReadOutOfBoundsError=>write!(f, "An attempt was made to read out of buffer bounds."),
            ByteBufError::SeekOutOfBoundsError=>write!(f, "An attempt was made to move the buffer cursor out of bounds."),
            ByteBufError::FileReadError=>write!(f, "Error reading file into ByteBuf.")
        }
    }
}
pub struct ByteBuf {
    size: usize,
    cursor: usize,
    vec: Vec<u8>,
}

impl ByteBuf {

    // Create a new, 0-initialized ByteBuf of the specified length
    pub fn new(size: usize) -> ByteBuf {
        ByteBuf {
            size,
            cursor: 0,
            vec: vec![0; size]
        }
    }

    // Create a ByteBuf from a supplied vector
    pub fn from_vec(vec: Vec<u8>) -> ByteBuf {
        ByteBuf {
            size: vec.len(),
            cursor: 0,
            vec: vec,
        }
    }

    // Create a ByteBuf from a supplied vector, copying the contents
    pub fn copy_vec(vec: &Vec<u8>) -> ByteBuf {
        ByteBuf {
            size: vec.len(),
            cursor: 0,
            vec: vec.to_vec(),
        }
    }

    /// Create a ByteBuf from a supplied slice, copying the contents
    pub fn from_slice(slice: &[u8]) -> ByteBuf {
        ByteBuf {
            size: slice.len(),
            cursor: 0,
            vec: slice.to_vec()
        }
    }

    pub fn from_file(mut file: File, size: usize) -> Result<ByteBuf, ByteBufError> {
        let mut buffer = Vec::new();

        let _bytes_read = file.read_to_end(&mut buffer).unwrap();
        buffer.resize(size, 0u8);
        Ok(ByteBuf {
            size: size,
            cursor: 0,
            vec: buffer,
        })
    }


    pub fn len(&self) -> usize {
        self.vec.len()
    }
    pub fn tell(&self) -> usize {
        self.cursor
    }
    pub fn seek(&mut self, disp: usize) -> Result<(), ByteBufError> {
        if disp > self.vec.len() - 1 {
            return Err(ByteBufError::SeekOutOfBoundsError)
        }
        self.cursor = disp;
        return Ok(())
    }
    pub fn seek_back(&mut self, disp: usize) -> Result<(), ByteBufError> {
        if disp > self.cursor {
            return Err(ByteBufError::SeekOutOfBoundsError)
        }
        self.cursor = self.cursor - disp;
        return Ok(())
    }
    pub fn seek_fwd(&mut self, disp: usize) -> Result<(), ByteBufError> {
        if self.cursor + disp > self.vec.len() - 1 {
            return Err(ByteBufError::SeekOutOfBoundsError)
        }
        self.cursor = self.cursor + disp;
        return Ok(())
    }
    /// Copy 'len' bytes from buffer into destination
    pub fn read_bytes(&mut self, dest: &mut [u8], len: usize) -> Result<(), ByteBufError> {
        if self.cursor <= self.vec.len() - len {
            for i in 0..len {
                dest[i] = self.vec[self.cursor];
                self.cursor += 1;
            }
            return Ok(())
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }
   
    /// Copy bytes into the buffer from the source slice
    pub fn write_bytes(&mut self, src: &[u8], len: usize) -> Result<(), ByteBufError> {
        if self.cursor <= self.vec.len() - len {
            for i in 0..len {
                self.vec[self.cursor] = src[i];
                self.cursor += 1;
            }
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Read a u8 from the buffer
    pub fn read_u8(&mut self) -> Result<u8, ByteBufError> {
        if self.cursor <= self.vec.len() - 1 {
            let b: u8 = self.vec[self.cursor];
            self.cursor += 1;
            return Ok(b)
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Read an i8 from the buffer
    pub fn read_i8(&mut self) -> Result<i8, ByteBufError> {
        if self.cursor <= self.vec.len() - 1 {
            let b: i8 = self.vec[self.cursor] as i8;
            self.cursor += 1;
            return Ok(b)
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }   

    /// Write a u8 to the buffer
    pub fn write_u8(&mut self, b: u8) -> Result<(), ByteBufError> {
        if self.cursor <= self.vec.len() - 1 {
            self.vec[self.cursor] = b;
            self.cursor += 1;
            return Ok(())
        }
        Err(ByteBufError::ReadOutOfBoundsError)        
    }   

    /// Read a u16 in little endian order.
    pub fn read_u16_le(&mut self) -> Result<u16, ByteBufError> {
        if self.cursor <= self.vec.len() - 2 {
            let w: u16 = self.vec[self.cursor] as u16 | (self.vec[self.cursor+1] as u16) << 8;
            self.cursor += 2;
            return Ok(w)
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }
  
    /// Read an i16 in little endian order
    pub fn read_i16_le(&mut self) -> Result<i16, ByteBufError> {
        if self.cursor <= self.vec.len() - 2 {
            let w: i16 = (self.vec[self.cursor] as u16 | (self.vec[self.cursor+1] as u16) << 8) as i16;
            self.cursor += 2;
            return Ok(w)
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }   

    /// Write a u16 in little endian order.
    pub fn write_u16_le(&mut self, w: u16) -> Result<(), ByteBufError> {
        if self.cursor <= self.vec.len() - 2 {
            self.vec[self.cursor] = (w & 0x00FF) as u8;
            self.vec[self.cursor+1] = (w >> 8) as u8;
            self.cursor += 2;
            return Ok(())
        }
        Err(ByteBufError::ReadOutOfBoundsError)            
    }

    /// Read a u16 in big endian order.
    pub fn read_u16_be(&mut self) -> Result<u16, ByteBufError> {
        if self.cursor <= self.vec.len() - 2 {
            let w: u16 = (self.vec[self.cursor] as u16) << 8 | self.vec[self.cursor+1] as u16;          
            self.cursor += 2;  
            return Ok(w)
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }
  
    // Read an i16 in big endian order
    pub fn read_i16_be(&mut self) -> Result<i16, ByteBufError> {
        if self.cursor <= self.vec.len() - 2 {
            let w: i16 = ((self.vec[self.cursor] as u16) << 8 | self.vec[self.cursor+1] as u16) as i16;
            self.cursor += 2;
            return Ok(w)
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }   

    /// Write a u16 in big endian order.
    pub fn write_u16_be(&mut self, w: u16) -> Result<(), ByteBufError> {
        if self.cursor <= self.vec.len() - 2 {
            self.vec[self.cursor] = (w >> 8) as u8;
            self.vec[self.cursor+1] = (w & 0x00FF) as u8;
            self.cursor += 2;
            return Ok(())
        }
        Err(ByteBufError::ReadOutOfBoundsError)            
    }

    /// Read a u32 in little endian order.
    pub fn read_u32_le(&mut self) -> Result<u32, ByteBufError> {
        if self.cursor <= self.vec.len() - 4 {
            let dw: u32 = (self.vec[self.cursor] as u32)
                | (self.vec[self.cursor+1] as u32) << 8 
                | (self.vec[self.cursor+2] as u32) << 16 
                | (self.vec[self.cursor+3] as u32) << 24;
            self.cursor += 4;  
            return Ok(dw)
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Read a i32 in little endian order.
    pub fn read_i32_le(&mut self) -> Result<i32, ByteBufError> {
        if self.cursor <= self.vec.len() - 4 {
            let dw: u32 = (self.vec[self.cursor] as u32)
                | (self.vec[self.cursor+1] as u32) << 8 
                | (self.vec[self.cursor+2] as u32) << 16 
                | (self.vec[self.cursor+3] as u32) << 24;
            self.cursor += 4;  
            return Ok(dw as i32)
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }    

    /// Read a u32 in big endian order.
    pub fn read_u32_be(&mut self) -> Result<u32, ByteBufError> {
        if self.cursor <= self.vec.len() - 4 {
            let dw: u32 = (self.vec[self.cursor] as u32) << 24 
                | (self.vec[self.cursor+1] as u32) << 16 
                | (self.vec[self.cursor+2] as u32) << 8 
                | (self.vec[self.cursor+3] as u32);     
            self.cursor += 4;  
            return Ok(dw)
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Write a u32 in little endian order.
    pub fn write_u32_le(&mut self, dw: u32) -> Result<(), ByteBufError> {
        if self.cursor <= self.vec.len() - 4 {
            self.vec[self.cursor+0] = (dw & 0xFF) as u8;
            self.vec[self.cursor+1] = (dw >> 8  & 0xFF) as u8;
            self.vec[self.cursor+2] = (dw >> 16 & 0xFF) as u8;
            self.vec[self.cursor+3] = (dw >> 24 & 0xFF) as u8;
            self.cursor += 4;  
            return Ok(())
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }  

    /// Write a u32 in big endian order.
    pub fn write_u32_be(&mut self, dw: u32) -> Result<(), ByteBufError> {
        if self.cursor <= self.vec.len() - 4 {

            self.vec[self.cursor+0] = (dw >> 24 & 0xFF) as u8;
            self.vec[self.cursor+1] = (dw >> 16 & 0xFF) as u8;
            self.vec[self.cursor+2] = (dw >> 8  & 0xFF) as u8;
            self.vec[self.cursor+3] = (dw & 0xFF) as u8;
            self.cursor += 4;  
            return Ok(())
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }    

    /// Read a u64 in big endian order.
    pub fn read_u64_be(&mut self) -> Result<u64, ByteBufError> {
        if self.cursor <= self.vec.len() - 8 {
            let ddw: u64 = (self.vec[self.cursor] as u64) << 56
                | (self.vec[self.cursor+1] as u64) << 48
                | (self.vec[self.cursor+2] as u64) << 40
                | (self.vec[self.cursor+3] as u64) << 32
                | (self.vec[self.cursor+4] as u64) << 24 
                | (self.vec[self.cursor+5] as u64) << 16 
                | (self.vec[self.cursor+6] as u64) << 8 
                | (self.vec[self.cursor+7] as u64);     
            self.cursor += 8;  
            return Ok(ddw)
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }
       
    /// Write a u64 in big endian order.
    pub fn write_u64_be(&mut self, ddw: u64) -> Result<(), ByteBufError> {
        if self.cursor <= self.vec.len() - 4 {
            self.vec[self.cursor+0] = (ddw >> 56 & 0xFF) as u8;
            self.vec[self.cursor+1] = (ddw >> 48 & 0xFF) as u8;
            self.vec[self.cursor+2] = (ddw >> 40 & 0xFF) as u8;
            self.vec[self.cursor+3] = (ddw >> 32 & 0xFF) as u8;
            self.vec[self.cursor+4] = (ddw >> 24 & 0xFF) as u8;
            self.vec[self.cursor+5] = (ddw >> 16 & 0xFF) as u8;
            self.vec[self.cursor+6] = (ddw >> 8  & 0xFF) as u8;
            self.vec[self.cursor+7] = (ddw & 0xFF) as u8;
            self.cursor += 8;  
            return Ok(())
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }    


}

pub struct ByteBufWriter<'a> {
    cursor: usize,
    buf: &'a mut [u8]
}

impl <'a> ByteBufWriter<'a> {
    pub fn from_slice(buf: &mut [u8]) -> ByteBufWriter {
        
        ByteBufWriter {
            cursor: 0,
            buf: buf
        }
    }
    
    pub fn take(&mut self) -> &mut [u8] {
        return self.buf;
    }
    
    pub fn len(&self) -> usize {
        self.buf.len()
    }
    pub fn tell(&self) -> usize {
        self.cursor
    }
    pub fn seek(&mut self, disp: usize) -> Result<(), ByteBufError> {
        if disp > self.buf.len() - 1 {
            return Err(ByteBufError::SeekOutOfBoundsError)
        }
        self.cursor = disp;
        return Ok(())
    }
    pub fn seek_back(&mut self, disp: usize) -> Result<(), ByteBufError> {
        if disp > self.cursor {
            return Err(ByteBufError::SeekOutOfBoundsError)
        }
        self.cursor = self.cursor - disp;
        return Ok(())
    }
    pub fn seek_fwd(&mut self, disp: usize) -> Result<(), ByteBufError> {
        if self.cursor + disp > self.buf.len() - 1 {
            return Err(ByteBufError::SeekOutOfBoundsError)
        }
        self.cursor = self.cursor + disp;
        return Ok(())
    }

    /// Copy bytes into the buffer from the source slice
    pub fn write_bytes(&mut self, src: &[u8], len: usize) -> Result<(), ByteBufError> {
        if self.cursor <= self.buf.len() - len && len <= src.len() {
            for i in 0..len {
                self.buf[self.cursor] = src[i];
                self.cursor += 1;
            }
            return Ok(())
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Write a u8 to the buffer
    pub fn write_u8(&mut self, b: u8) -> Result<(), ByteBufError> {
        if self.cursor <= self.buf.len() - 1 {
            self.buf[self.cursor] = b;
            self.cursor += 1;
            return Ok(())
        }
        Err(ByteBufError::ReadOutOfBoundsError)        
    }   
    
    /// Write a u16 in little endian order.
    pub fn write_u16_le(&mut self, w: u16) -> Result<(), ByteBufError> {
        if self.cursor <= self.buf.len() - 2 {
            self.buf[self.cursor] = (w & 0x00FF) as u8;
            self.buf[self.cursor+1] = (w >> 8) as u8;
            self.cursor += 2;
            return Ok(())
        }
        Err(ByteBufError::ReadOutOfBoundsError)            
    }    
    
    /// Write a u16 in big endian order.
    pub fn write_u16_be(&mut self, w: u16) -> Result<(), ByteBufError> {
        if self.cursor <= self.buf.len() - 2 {
            self.buf[self.cursor] = (w >> 8) as u8;
            self.buf[self.cursor+1] = (w & 0x00FF) as u8;
            self.cursor += 2;
            return Ok(())
        }
        Err(ByteBufError::ReadOutOfBoundsError)            
    }    

    /// Write a u32 in little endian order.
    pub fn write_u32_le(&mut self, dw: u32) -> Result<(), ByteBufError> {
        if self.cursor <= self.buf.len() - 4 {
            self.buf[self.cursor+0] = (dw & 0xFF) as u8;
            self.buf[self.cursor+1] = (dw >> 8  & 0xFF) as u8;
            self.buf[self.cursor+2] = (dw >> 16 & 0xFF) as u8;
            self.buf[self.cursor+3] = (dw >> 24 & 0xFF) as u8;
            self.cursor += 4;  
            return Ok(())
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Write a u32 in big endian order.
    pub fn write_u32_be(&mut self, dw: u32) -> Result<(), ByteBufError> {
        if self.cursor <= self.buf.len() - 4 {

            self.buf[self.cursor+0] = (dw >> 24 & 0xFF) as u8;
            self.buf[self.cursor+1] = (dw >> 16 & 0xFF) as u8;
            self.buf[self.cursor+2] = (dw >> 8  & 0xFF) as u8;
            self.buf[self.cursor+3] = (dw & 0xFF) as u8;
            self.cursor += 4;  
            return Ok(())
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }

    /// Write a u64 in big endian order.
    pub fn write_u64_be(&mut self, ddw: u64) -> Result<(), ByteBufError> {
        if self.cursor <= self.buf.len() - 4 {
            self.buf[self.cursor+0] = (ddw >> 56 & 0xFF) as u8;
            self.buf[self.cursor+1] = (ddw >> 48 & 0xFF) as u8;
            self.buf[self.cursor+2] = (ddw >> 40 & 0xFF) as u8;
            self.buf[self.cursor+3] = (ddw >> 32 & 0xFF) as u8;
            self.buf[self.cursor+4] = (ddw >> 24 & 0xFF) as u8;
            self.buf[self.cursor+5] = (ddw >> 16 & 0xFF) as u8;
            self.buf[self.cursor+6] = (ddw >> 8  & 0xFF) as u8;
            self.buf[self.cursor+7] = (ddw & 0xFF) as u8;
            self.cursor += 8;  
            return Ok(())
        }
        Err(ByteBufError::ReadOutOfBoundsError)
    }        
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    
    fn test_16() {

        let array: [u8; 16]= [0; 16];

        let mut buf = ByteBuf::from_slice(&array);

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

    fn test_32() {

        let array: [u8; 16]= [0; 16];

        let mut buf = ByteBuf::from_slice(&array);

        let a1: u32 = 0x01020304;
        let a2: u32 = 0x04030201;
        let a3: i32 = i32::MAX;
        let a4: i32 = i32::MIN;

        buf.write_u32_le(a1).unwrap();
        buf.write_u32_le(a2).unwrap();
        buf.write_u32_le(a3 as u32).unwrap();
        buf.write_u32_le(a4 as u32).unwrap();
        buf.seek_back(8).unwrap();

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
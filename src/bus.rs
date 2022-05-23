#![allow(dead_code)]
use std::error::Error;
use std::fs::File;
use std::io::Read;

use crate::byteinterface::ByteInterface;
use crate::memerror::MemError;

const ADDRESS_SPACE: usize = 1_048_576;
const DEFAULT_CYCLE_COST: u32 = 4;

struct MemRangeDescriptor {
    start: usize,
    end: usize,
    size: usize,
    cycle_cost: u32,
    read_only: bool
}
pub struct BusInterface {
    memory: Vec<u8>,
    desc_vec: Vec<MemRangeDescriptor>,
    cursor: usize
}

impl ByteInterface for BusInterface {
    fn set_cursor(&mut self, pos: usize) {
        self.cursor = pos;
    }
    fn tell(&self) -> usize {
        self.cursor
    }
    fn read_u8(&mut self, cost: &mut u32) -> u8 {
        if self.cursor < self.memory.len() {
            let b: u8 = self.memory[self.cursor];
            *cost += DEFAULT_CYCLE_COST;
            self.cursor += 1;
            return b
        }
        *cost += DEFAULT_CYCLE_COST;
        0xffu8
    }
    fn read_i8(&mut self, cost: &mut u32) -> i8 {
        if self.cursor < self.memory.len() {
            let b: i8 = self.memory[self.cursor] as i8;
            *cost += DEFAULT_CYCLE_COST;
            self.cursor += 1;
            return b
        }
        *cost += DEFAULT_CYCLE_COST;
        -1i8       
    }
    fn write_u8(&mut self, data: u8, cost: &mut u32) {
        if self.cursor < self.memory.len() {
            self.memory[self.cursor] = data;
            *cost += DEFAULT_CYCLE_COST;
            self.cursor += 1;
        }
        *cost += DEFAULT_CYCLE_COST;
    }
    fn write_i8(&mut self, data: i8, cost: &mut u32) {
        if self.cursor < self.memory.len() {
            self.memory[self.cursor] = data as u8;
            *cost += DEFAULT_CYCLE_COST;
            self.cursor += 1;
        }
        *cost += DEFAULT_CYCLE_COST;
    }    
    fn read_u16(&mut self, cost: &mut u32) -> u16 {
        if self.cursor < self.memory.len() - 1 {
            let w: u16 = self.memory[self.cursor] as u16 | (self.memory[self.cursor+1] as u16) << 8;
            *cost += DEFAULT_CYCLE_COST * 2;
            self.cursor += 2;
            return w
        }
        *cost += DEFAULT_CYCLE_COST * 2;
        0xffffu16   
    }
    fn read_i16(&mut self, cost: &mut u32) -> i16 {
        if self.cursor < self.memory.len() - 1 {
            let w: i16 = (self.memory[self.cursor] as u16 | (self.memory[self.cursor+1] as u16) << 8) as i16;
            *cost += DEFAULT_CYCLE_COST * 2;
            self.cursor += 2;
            return w
        }
        *cost += DEFAULT_CYCLE_COST * 2;
        -1i16
    }
    fn write_u16(&mut self, data: u16, cost: &mut u32) {
        if self.cursor < self.memory.len() - 1 {
            // Little Endian is LO byte first
            self.memory[self.cursor] |= (data & 0xFF) as u8;
            self.memory[self.cursor + 1] |= (data >> 8) as u8; 
            *cost += DEFAULT_CYCLE_COST * 2;
            self.cursor += 2;
        }
        *cost += DEFAULT_CYCLE_COST * 2; 
    }    
    fn write_i16(&mut self, data: i16, cost: &mut u32) {
        if self.cursor < self.memory.len() - 1 {
            // Little Endian is LO byte first
            self.memory[self.cursor] |= ((data as u16) & 0xFF) as u8;
            self.memory[self.cursor + 1] |= ((data as u16) >> 8) as u8;
            *cost += DEFAULT_CYCLE_COST * 2;
            self.cursor += 2;
        }
        *cost += DEFAULT_CYCLE_COST * 2; 
    }     
}

impl BusInterface {
    pub fn new() -> BusInterface {
        BusInterface {
            memory: vec![0; ADDRESS_SPACE],
            desc_vec: Vec::new(),
            cursor: 0
        }
    }

    pub fn copy_from(&mut self, src_vec: Vec<u8>, location: usize, cycle_cost: u32, read_only: bool) -> Result<(), bool> {
        
        let src_size = src_vec.len();
        if location as usize + src_size > self.memory.len() {
            // copy request goes out of bounds
            return Err(false)
        }

        let mem_slice: &mut [u8] = &mut self.memory[location..location+src_size];
        
        for (dst, src) in mem_slice.iter_mut().zip(src_vec.as_slice()) {
            *dst = *src;
        }

        self.desc_vec.push({
            MemRangeDescriptor {
                start: location,
                end: location + src_size,
                size: src_size,
                cycle_cost: cycle_cost,
                read_only: read_only
            }
        });

        Ok(())
    }

    pub fn patch_from(&mut self, src_vec: Vec<u8>, location: usize) -> Result<(), bool> {
        let src_size = src_vec.len();
        if location as usize + src_size > self.memory.len() {
            // copy request goes out of bounds
            return Err(false)
        }

        let mem_slice: &mut [u8] = &mut self.memory[location..location+src_size];
        
        for (dst, src) in mem_slice.iter_mut().zip(src_vec.as_slice()) {
            *dst = *src;
        }
        Ok(())
    }

    pub fn get_slice_at(&self, start: usize, len: usize ) -> &[u8] {
        &self.memory[start..start+len]
    }

    pub fn set_descriptor(&mut self, start: usize, end: usize, cycle_cost: u32, read_only: bool) {
        // TODO: prevent overlapping descriptors
        self.desc_vec.push({
            MemRangeDescriptor {
                start: start,
                end: end,
                size: end - start,
                cycle_cost: cycle_cost,
                read_only: read_only
            }
        });        
    }

    pub fn read_u8(&self, address: usize ) -> Result<(u8, u32), MemError> {
        if address < self.memory.len() {
            let b: u8 = self.memory[address];
            return Ok((b, DEFAULT_CYCLE_COST))
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn read_i8(&self, address: usize ) -> Result<(i8, u32), MemError> {
        if address < self.memory.len() {
            let b: i8 = self.memory[address] as i8;
            return Ok((b, DEFAULT_CYCLE_COST))
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn read_u16(&self, address: usize ) -> Result<(u16, u32), MemError> {
        if address < self.memory.len() - 1 {
            let w: u16 = self.memory[address] as u16 | (self.memory[address+1] as u16) << 8;
            return Ok((w, DEFAULT_CYCLE_COST))
        }
        Err(MemError::ReadOutOfBoundsError)
    }
    pub fn read_i16(&self, address: usize ) -> Result<(i16, u32), MemError> {
        if address < self.memory.len() - 1 {
            let w: i16 = (self.memory[address] as u16 | (self.memory[address+1] as u16) << 8) as i16;
            return Ok((w, DEFAULT_CYCLE_COST))
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn write_u8(&mut self, address: usize, data: u8) -> Result<u32, MemError> {
        if address < self.memory.len() {
            self.memory[address] = data;
            return Ok(DEFAULT_CYCLE_COST)
        }
        Err(MemError::ReadOutOfBoundsError)
    }
    pub fn write_i8(&mut self, address: usize, data: i8) -> Result<u32, MemError> {
        if address < self.memory.len() {
            self.memory[address] = data as u8;
            return Ok(DEFAULT_CYCLE_COST)
        }
        Err(MemError::ReadOutOfBoundsError)
    }    

    pub fn write_u16(&mut self, address: usize, data: u16) -> Result<u32, MemError> {
        if address < self.memory.len() - 1 {
            // Little Endian is LO byte first
            self.memory[address] = (data & 0xFF) as u8;
            self.memory[address+1] = (data >> 8) as u8;
            return Ok(DEFAULT_CYCLE_COST)
        }
        Err(MemError::ReadOutOfBoundsError)
    }
    
    pub fn write_i16(&mut self, address: usize, data: u16) -> Result<u32, MemError> {
        if address < self.memory.len() - 1 {
            // Little Endian is LO byte first
            self.memory[address] = (data as u16 & 0xFF) as u8;
            self.memory[address+1] = (data as u16 >> 8) as u8;
            return Ok(DEFAULT_CYCLE_COST)
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn dump_flat(&self, address: usize, size: usize) -> String {

        if address + size >= self.memory.len() {
            return "REQUEST OUT OF BOUNDS".to_string()
        }
        else {
            let mut dump_str = String::new();
            let dump_slice = &self.memory[address..address+size];
            let mut display_address = address;

            for dump_row in dump_slice.chunks_exact(16) {

                let mut dump_line = String::new();
                let mut ascii_line = String::new();

                for byte in dump_row {
                    dump_line.push_str(&format!("{:02x} ", byte) );

                    let char_str = match byte {
                        00..=31 => ".".to_string(),
                        32..=127 => format!("{}", *byte as char),
                        128.. => ".".to_string()
                    };
                    ascii_line.push_str(&char_str)
                }
                dump_str.push_str(&format!("{:05X} {} {}\n", display_address, dump_line, ascii_line));
                display_address += 16;
            }
            return dump_str
        }
    }
}
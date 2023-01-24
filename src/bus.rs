#![allow(dead_code)]
use std::rc::Rc;
use std::cell::RefCell;

use crate::bytequeue::*;

use crate::memerror::MemError;

const ADDRESS_SPACE: usize = 1_048_576;
const DEFAULT_WAIT_STATES: u32 = 0;
const ROM_BIT: u8 = 0b1000_0000;

pub trait MemoryMappedDevice {  
    fn read_u8(&mut self, address: usize) -> u8;
    fn read_u16(&mut self, address: usize) -> u16;

    fn write_u8(&mut self, address: usize, data: u8); 
    fn write_u16(&mut self, address: usize, data: u16);
}

pub struct MemRangeDescriptor {
    address: usize,
    size: usize,
    cycle_cost: u32,
    read_only: bool
}
impl MemRangeDescriptor {
    pub fn new(address: usize, size: usize, read_only: bool) -> Self {
        Self {
            address,
            size,
            cycle_cost: 0,
            read_only,
        }
    }
}


pub struct BusInterface {
    memory: Vec<u8>,
    memory_mask: Vec<u8>,
    desc_vec: Vec<MemRangeDescriptor>,
    map: Vec<(MemRangeDescriptor, Rc<RefCell<dyn MemoryMappedDevice>>)>,
    first_map: usize,
    last_map: usize,
    cursor: usize
}

impl ByteQueue for BusInterface {
    fn seek(&mut self, pos: usize) {
        self.cursor = pos;
    }

    fn tell(&self) -> usize {
        self.cursor
    }

    fn delay(&mut self, _delay: u32) {}
    fn wait(&mut self, _cycles: u32) {}
    fn clear_delay(&mut self) {}

    fn q_read_u8(&mut self, _dtype: QueueType) -> u8 {
        if self.cursor < self.memory.len() {
            let b: u8 = self.memory[self.cursor];
            self.cursor += 1;
            return b
        }
        0xffu8
    }

    fn q_read_i8(&mut self, _dtype: QueueType) -> i8 {
        if self.cursor < self.memory.len() {
            let b: i8 = self.memory[self.cursor] as i8;
            self.cursor += 1;
            return b
        }
        -1i8       
    }

    fn q_read_u16(&mut self, _dtype: QueueType) -> u16 {
        if self.cursor < self.memory.len() - 1 {
            let w: u16 = self.memory[self.cursor] as u16 | (self.memory[self.cursor+1] as u16) << 8;
            self.cursor += 2;
            return w
        }
        0xffffu16   
    }

    fn q_read_i16(&mut self, _dtype: QueueType) -> i16 {
        if self.cursor < self.memory.len() - 1 {
            let w: i16 = (self.memory[self.cursor] as u16 | (self.memory[self.cursor+1] as u16) << 8) as i16;
            self.cursor += 2;
            return w
        }
        -1i16
    }    
}

impl Default for BusInterface {
    fn default() -> Self {
        BusInterface {
            memory: vec![0; ADDRESS_SPACE],
            memory_mask: vec![0; ADDRESS_SPACE],
            desc_vec: Vec::new(),
            map: Vec::new(),
            cursor: 0,
            first_map: 0,
            last_map: 0
        }        
    }
}

impl BusInterface {
    pub fn new() -> BusInterface {
        BusInterface {
            memory: vec![0; ADDRESS_SPACE],
            memory_mask: vec![0; ADDRESS_SPACE],
            desc_vec: Vec::new(),
            map: Vec::new(),
            cursor: 0,
            first_map: 0,
            last_map: 0
        }
    }

    pub fn size(&self) -> usize {
        self.memory.len()
    }

    /// Register a memory-mapped device.
    /// 
    /// The MemoryMappedDevice trait's read & write methods will be called instead for memory in the range
    /// specified withing MemRangeDescriptor.
    pub fn register_map(&mut self, device: Rc<RefCell<dyn MemoryMappedDevice>>, mem_descriptor: MemRangeDescriptor) {
        if mem_descriptor.address < self.first_map {
            self.first_map = mem_descriptor.address;
        }
        if (mem_descriptor.address + mem_descriptor.size) > self.last_map {
            self.last_map = mem_descriptor.address + mem_descriptor.size;
        }
        self.map.push((mem_descriptor, device));
    }

    pub fn copy_from(&mut self, src: &[u8], location: usize, cycle_cost: u32, read_only: bool) -> Result<(), bool> {
        
        let src_size = src.len();
        if location as usize + src_size > self.memory.len() {
            // copy request goes out of bounds
            return Err(false)
        }

        let mem_slice: &mut [u8] = &mut self.memory[location..location + src_size];
        let mask_slice: &mut [u8] = &mut self.memory_mask[location..location + src_size];
        for (dst, src) in mem_slice.iter_mut().zip(src) {
            *dst = *src;
        }

        // Write access mask
        let access_bit = match read_only {
            true => ROM_BIT,
            false => 0x00
        };
        for dst in mask_slice.iter_mut() {
            *dst = cycle_cost as u8 & 0xEF | access_bit;
        }

        self.desc_vec.push({
            MemRangeDescriptor {
                address: location,
                size: src_size,
                cycle_cost: cycle_cost,
                read_only: read_only
            }
        });

        Ok(())
    }

    /// Write the specified bytes from src_vec into memory at location 'location'
    /// 
    /// Does not obey memory mapping
    pub fn patch_from(&mut self, src_vec: &Vec<u8>, location: usize) -> Result<(), bool> {
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

    pub fn set_descriptor(&mut self, start: usize, size: usize, cycle_cost: u32, read_only: bool) {
        // TODO: prevent overlapping descriptors
        self.desc_vec.push({
            MemRangeDescriptor {
                address: start,
                size: size,
                cycle_cost: cycle_cost,
                read_only: read_only
            }
        });        
    }

    pub fn reset(&mut self) {
        // Clear mem range descriptors
        self.desc_vec.clear();

        // Set all bytes to 0
        for byte_ref in &mut self.memory {
            *byte_ref = 0;
        }
    }

    pub fn read_u8(&self, address: usize ) -> Result<(u8, u32), MemError> {
        if address < self.memory.len() {

            // Handle memory-mapped devices
            for map_entry in &self.map {
                if address >= map_entry.0.address && address < map_entry.0.address + map_entry.0.size {
                    return Ok((map_entry.1.borrow_mut().read_u8(address), map_entry.0.cycle_cost));
                }
            }
            let b: u8 = self.memory[address];
            return Ok((b, DEFAULT_WAIT_STATES))
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn read_i8(&self, address: usize ) -> Result<(i8, u32), MemError> {
        if address < self.memory.len() {
            
            // Handle memory-mapped devices
            for map_entry in &self.map {
                if address >= map_entry.0.address && address < map_entry.0.address + map_entry.0.size {
                    return Ok((map_entry.1.borrow_mut().read_u8(address) as i8, map_entry.0.cycle_cost));
                }
            }

            let b: i8 = self.memory[address] as i8;
            return Ok((b, DEFAULT_WAIT_STATES))
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn read_u16(&self, address: usize ) -> Result<(u16, u32), MemError> {
        if address < self.memory.len() - 1 {

            // Handle memory-mapped devices
            for map_entry in &self.map {
                if address >= map_entry.0.address && address < map_entry.0.address + map_entry.0.size - 1 {
                    return Ok((map_entry.1.borrow_mut().read_u16(address), map_entry.0.cycle_cost));
                }
            }

            let w: u16 = self.memory[address] as u16 | (self.memory[address+1] as u16) << 8;
            return Ok((w, DEFAULT_WAIT_STATES))
        }
        Err(MemError::ReadOutOfBoundsError)
    }
    pub fn read_i16(&self, address: usize ) -> Result<(i16, u32), MemError> {
        if address < self.memory.len() - 1 {

            // Handle memory-mapped devices
            for map_entry in &self.map {
                if address >= map_entry.0.address && address < map_entry.0.address + map_entry.0.size - 1 {
                    return Ok((map_entry.1.borrow_mut().read_u16(address) as i16, map_entry.0.cycle_cost));
                }
            }

            let w: i16 = (self.memory[address] as u16 | (self.memory[address+1] as u16) << 8) as i16;
            return Ok((w, DEFAULT_WAIT_STATES))
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn write_u8(&mut self, address: usize, data: u8) -> Result<u32, MemError> {
        if address < self.memory.len() {

            // Handle memory-mapped devices
            for map_entry in &self.map {
                if address >= map_entry.0.address && address < map_entry.0.address + map_entry.0.size {
                    map_entry.1.borrow_mut().write_u8(address, data);
                    return Ok(map_entry.0.cycle_cost);
                }
            }
            
            if self.memory_mask[address] & ROM_BIT == 0 {
                self.memory[address] = data;                
            }
            return Ok(DEFAULT_WAIT_STATES)
        }
        Err(MemError::ReadOutOfBoundsError)
    }
    pub fn write_i8(&mut self, address: usize, data: i8) -> Result<u32, MemError> {
        if address < self.memory.len() {

            // Handle memory-mapped devices
            for map_entry in &self.map {
                if address >= map_entry.0.address && address < map_entry.0.address + map_entry.0.size {
                    map_entry.1.borrow_mut().write_u8(address, data as u8);
                    return Ok(map_entry.0.cycle_cost);
                }
            }

            if self.memory_mask[address] & ROM_BIT == 0 {
                self.memory[address] = data as u8;
            }
            return Ok(DEFAULT_WAIT_STATES)
        }
        Err(MemError::ReadOutOfBoundsError)
    }    

    pub fn write_u16(&mut self, address: usize, data: u16) -> Result<u32, MemError> {
        if address < self.memory.len() - 1 {

            // Handle memory-mapped devices
            for map_entry in &self.map {
                if address >= map_entry.0.address && address < map_entry.0.address + map_entry.0.size - 1 {
                    map_entry.1.borrow_mut().write_u8(address, (data & 0xFF) as u8);
                    map_entry.1.borrow_mut().write_u8(address + 1, (data >> 8) as u8);
                    return Ok(map_entry.0.cycle_cost);
                }
            }

            // Little Endian is LO byte first
            if self.memory_mask[address] & ROM_BIT == 0 {
                self.memory[address] = (data & 0xFF) as u8;
                self.memory[address+1] = (data >> 8) as u8;              
            }
            return Ok(DEFAULT_WAIT_STATES)
        }
        Err(MemError::ReadOutOfBoundsError)
    }
    
    pub fn write_i16(&mut self, address: usize, data: i16) -> Result<u32, MemError> {
        if address < self.memory.len() - 1 {

            // Handle memory-mapped devices
            for map_entry in &self.map {
                if address >= map_entry.0.address && address < map_entry.0.address + map_entry.0.size - 1 {
                    map_entry.1.borrow_mut().write_u8(address, ((data as u16) & 0xFF) as u8);
                    map_entry.1.borrow_mut().write_u8(address + 1, ((data as u16) >> 8) as u8);
                    return Ok(map_entry.0.cycle_cost);
                }
            }

            // Little Endian is LO byte first
            if self.memory_mask[address] & ROM_BIT == 0 {
                self.memory[address] = (data as u16 & 0xFF) as u8;
                self.memory[address+1] = (data as u16 >> 8) as u8;
            }
            return Ok(DEFAULT_WAIT_STATES)
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    /// Dump memory to a string representation.
    /// 
    /// Does not honor memory mappings.
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
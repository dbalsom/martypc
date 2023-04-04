#![allow(dead_code)]
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use std::fmt;
use std::io::{BufWriter, Write};

use ringbuf::{Producer};

use crate::cpu_808x::*;
use crate::bytequeue::*;
use crate::fdc::FloppyController;
use crate::pit::Pit;
use crate::syntax_token::SyntaxToken;
use crate::machine_manager::MachineDescriptor;
use crate::config::VideoType;
use crate::pic::*;
use crate::dma::*;
use crate::ppi::*;
use crate::serial::*;
use crate::hdc::*;
use crate::mouse::*;
use crate::tracelogger::TraceLogger;
use crate::videocard::{VideoCard, VideoCardDispatch};

use crate::cga::{self, CGACard};
use crate::ega::{self, EGACard};
use crate::vga::{self, VGACard};
use crate::cga::*;
use crate::memerror::MemError;

pub const NO_IO_BYTE: u8 = 0xFF; // This is the byte read from a unconnected IO address.
pub const FLOATING_BUS_BYTE: u8 = 0x00; // This is the byte read from an unmapped memory address.

const ADDRESS_SPACE: usize = 1_048_576;
const DEFAULT_WAIT_STATES: u32 = 0;

const ROM_BIT: u8 = 0b1000_0000;
pub const MEM_RET_BIT: u8 = 0b0100_0000; // Bit to signify that this address is a return address for a CALL or INT
pub const MEM_BPE_BIT: u8 = 0b0010_0000; // Bit to signify that this address is associated with a breakpoint on execute
pub const MEM_BPA_BIT: u8 = 0b0001_0000; // Bit to signify that this address is associated with a breakpoint on access
pub const MEM_CP_BIT: u8  = 0b0000_1000; // Bit to signify that this address is a ROM checkpoint

pub trait MemoryMappedDevice {  
    fn read_u8(&mut self, address: usize) -> u8;
    fn read_u16(&mut self, address: usize) -> u16;

    fn write_u8(&mut self, address: usize, data: u8); 
    fn write_u16(&mut self, address: usize, data: u16);
}

pub struct MemoryDebug {
    addr: String,
    byte: String,
    word: String,
    dword: String,
    instr: String
}

impl fmt::Display for MemoryDebug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ADDR: {}\nBYTE: {}\nWORD: {}\nDWORD: {}\nINSTR: {}", self.addr, self.byte, self.word, self.dword, self.instr)
    }
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

pub enum IoDeviceType {
    Ppi,
    Pit,
    DmaPrimary,
    DmaSecondary,
    PicPrimary,
    PicSecondary,
    Serial,
    FloppyController,
    HardDiskController,
    Mouse,
    Cga,
    Ega,
    Vga,
}

pub enum IoDeviceDispatch {
    Static(IoDeviceType),
    Dynamic(Box<dyn IoDevice + 'static>)
}

pub trait IoDevice {
    fn read_u8(&mut self, port: u16) -> u8;
    fn write_u8(&mut self, port: u16, data: u8, bus: Option<&mut BusInterface>);
    fn port_list(&self) -> Vec<u16>;
}

pub struct MmioData {
    first_map: usize,
    last_map: usize
}

impl MmioData {
    fn new() -> Self {
        Self {
            first_map: 0xFFFFF,
            last_map: 0x00000
        }
    }
}

// Main bus struct.
// Bus contains both the system memory and IO, and owns all connected devices.
// This ownership heirachy allows us to avoid needing RefCells for devices.
//
// All devices are wrapped in Options. Some devices are actually optional, depending
// on the machine type. 
// But this allows us to 'disassociate' devices from the bus on io writes to allow
// us to call them with bus as an argument.
pub struct BusInterface {
    memory: Vec<u8>,
    memory_mask: Vec<u8>,
    desc_vec: Vec<MemRangeDescriptor>,
    mmio_map: Vec<(MemRangeDescriptor, IoDeviceType)>,
    mmio_data: MmioData,
    cursor: usize,

    io_map: HashMap<u16, IoDeviceType>,
    ppi: Option<Ppi>,
    pit: Option<Pit>,
    dma1: Option<DMAController>,
    dma2: Option<DMAController>,
    pic1: Option<Pic>,
    pic2: Option<Pic>,
    serial: Option<SerialPortController>,
    fdc: Option<FloppyController>,
    hdc: Option<HardDiskController>,
    mouse: Option<Mouse>,
    video: VideoCardDispatch,
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
    fn wait_i(&mut self, _cycles: u32, _instr: &[u16]) {}
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

    fn q_peek_u8(&mut self) -> u8 {
        if self.cursor < self.memory.len() {
            let b: u8 = self.memory[self.cursor];
            return b
        }
        0xffu8
    }

    fn q_peek_i8(&mut self) -> i8 {
        if self.cursor < self.memory.len() {
            let b: i8 = self.memory[self.cursor] as i8;
            return b
        }
        -1i8   
    }

    fn q_peek_u16(&mut self) -> u16 {
        if self.cursor < self.memory.len() - 1 {
            let w: u16 = self.memory[self.cursor] as u16 | (self.memory[self.cursor+1] as u16) << 8;
            return w
        }
        0xffffu16   
    }    

    fn q_peek_i16(&mut self) -> i16 {
        if self.cursor < self.memory.len() - 1 {
            let w: i16 = (self.memory[self.cursor] as u16 | (self.memory[self.cursor+1] as u16) << 8) as i16;
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
            mmio_map: Vec::new(),
            mmio_data: MmioData::new(),
            cursor: 0,


            io_map: HashMap::new(),
            ppi: None,
            pit: None,
            dma1: None,
            dma2: None,
            pic1: None,
            pic2: None,            
            serial: None,
            fdc: None,
            hdc: None,
            mouse: None,
            video: VideoCardDispatch::None
        }        
    }
}

impl BusInterface {
    pub fn new() -> BusInterface {
        BusInterface {
            memory: vec![0; ADDRESS_SPACE],
            memory_mask: vec![0; ADDRESS_SPACE],
            desc_vec: Vec::new(),
            mmio_map: Vec::new(),
            mmio_data: MmioData::new(),            
            cursor: 0,

            io_map: HashMap::new(),
            ppi: None,
            pit: None,
            dma1: None,
            dma2: None,
            pic1: None,
            pic2: None,        
            serial: None,    
            fdc: None,
            hdc: None,
            mouse: None,
            video: VideoCardDispatch::None,
        }
    }

    pub fn size(&self) -> usize {
        self.memory.len()
    }

    /// Register a memory-mapped device.
    /// 
    /// The MemoryMappedDevice trait's read & write methods will be called instead for memory in the range
    /// specified withing MemRangeDescriptor.
    pub fn register_map(&mut self, device: IoDeviceType, mem_descriptor: MemRangeDescriptor) {
        if mem_descriptor.address < self.mmio_data.first_map {
            self.mmio_data.first_map = mem_descriptor.address;
        }
        if (mem_descriptor.address + mem_descriptor.size) > self.mmio_data.last_map {
            self.mmio_data.last_map = mem_descriptor.address + mem_descriptor.size;
        }
        self.mmio_map.push((mem_descriptor, device));
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
            *dst |= access_bit;
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

    pub fn clear(&mut self) {

        // Remove return flags
        for byte_ref in &mut self.memory_mask {
            *byte_ref &= !MEM_RET_BIT;
        } 

        // Set all bytes to 0
        for byte_ref in &mut self.memory {
            *byte_ref = 0;
        }
    }

    pub fn reset(&mut self) {
        // Clear mem range descriptors
        self.desc_vec.clear();

        self.clear();
    }

    pub fn read_u8(&mut self, address: usize ) -> Result<(u8, u32), MemError> {
        if address < self.memory.len() {
            if address < self.mmio_data.first_map || address > self.mmio_data.last_map {
                // Address is not mapped.
                let b: u8 = self.memory[address];
                return Ok((b, DEFAULT_WAIT_STATES))
            }
            else {
                // Handle memory-mapped devices
                for map_entry in &self.mmio_map {
                    if address >= map_entry.0.address && address < map_entry.0.address + map_entry.0.size {

                        match map_entry.1 {
                            IoDeviceType::Cga | IoDeviceType::Ega | IoDeviceType::Vga => {
                                match &mut self.video {
                                    VideoCardDispatch::Cga(cga) => {
                                        return Ok((MemoryMappedDevice::read_u8(cga, address), DEFAULT_WAIT_STATES));
                                    }
                                    VideoCardDispatch::Ega(ega) => {
                                        return Ok((MemoryMappedDevice::read_u8(ega, address), DEFAULT_WAIT_STATES));
                                    }
                                    VideoCardDispatch::Vga(vga) => {
                                        return Ok((MemoryMappedDevice::read_u8(vga, address), DEFAULT_WAIT_STATES));
                                    }
                                    _ => {}
                                }
                            }
                            _=> {}
                        }
                        return Err(MemError::MmioError)
                    }
                }
                // We didn't match any mmio devices, return raw memory
                let b: u8 = self.memory[address];
                return Ok((b, DEFAULT_WAIT_STATES))
            }
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    /*
    pub fn read_i8(&self, address: usize ) -> Result<(i8, u32), MemError> {
        if address < self.memory.len() {
            
            // Handle memory-mapped devices
            for map_entry in &self.mmio_map {
                if address >= map_entry.0.address && address < map_entry.0.address + map_entry.0.size {
                    return Ok((map_entry.1.borrow_mut().read_u8(address) as i8, map_entry.0.cycle_cost));
                }
            }

            let b: i8 = self.memory[address] as i8;
            return Ok((b, DEFAULT_WAIT_STATES))
        }
        Err(MemError::ReadOutOfBoundsError)
    }
    */

    pub fn read_u16(&mut self, address: usize ) -> Result<(u16, u32), MemError> {
        if address < self.memory.len() - 1 {
            if address < self.mmio_data.first_map || address > self.mmio_data.last_map {
                // Address is not mapped.
                let w: u16 = self.memory[address] as u16 | (self.memory[address + 1] as u16) << 8;
                return Ok((w, DEFAULT_WAIT_STATES))
            }
            else {
                // Handle memory-mapped devices
                for map_entry in &self.mmio_map {
                    if address >= map_entry.0.address && address < (map_entry.0.address + map_entry.0.size - 1) {

                        match map_entry.1 {
                            IoDeviceType::Cga | IoDeviceType::Ega | IoDeviceType::Vga => {
                                match &mut self.video {
                                    VideoCardDispatch::Cga(cga) => {
                                        return Ok((cga.read_u16(address), DEFAULT_WAIT_STATES));
                                    }
                                    VideoCardDispatch::Ega(ega) => {
                                        return Ok((ega.read_u16(address), DEFAULT_WAIT_STATES));
                                    }
                                    VideoCardDispatch::Vga(vga) => {
                                        return Ok((vga.read_u16(address), DEFAULT_WAIT_STATES));
                                    }
                                    _ => {}
                                }
                            }
                            _=> {}
                        }
                        return Err(MemError::MmioError)
                    }
                }
                // We didn't match any mmio devices, return raw memory
                let w: u16 = self.memory[address] as u16 | (self.memory[address + 1] as u16) << 8;
                return Ok((w, DEFAULT_WAIT_STATES))            
            }
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    /*
    pub fn read_i16(&self, address: usize ) -> Result<(i16, u32), MemError> {
        if address < self.memory.len() - 1 {

            // Handle memory-mapped devices
            for map_entry in &self.mmio_map {
                if address >= map_entry.0.address && address < map_entry.0.address + map_entry.0.size - 1 {
                    return Ok((map_entry.1.borrow_mut().read_u16(address) as i16, map_entry.0.cycle_cost));
                }
            }

            let w: i16 = (self.memory[address] as u16 | (self.memory[address+1] as u16) << 8) as i16;
            return Ok((w, DEFAULT_WAIT_STATES))
        }
        Err(MemError::ReadOutOfBoundsError)
    }
    */

    pub fn write_u8(&mut self, address: usize, data: u8) -> Result<u32, MemError> {
        if address < self.memory.len() {
            if address < self.mmio_data.first_map || address > self.mmio_data.last_map {
                // Address is not mapped.
                if self.memory_mask[address] & ROM_BIT == 0 {
                    self.memory[address] = data;                
                }
                return Ok(DEFAULT_WAIT_STATES);
            }
            else {

                let mut handled = false;
                // Handle memory-mapped devices
                for map_entry in &self.mmio_map {
                    if address >= map_entry.0.address && address < map_entry.0.address + map_entry.0.size {

                        match map_entry.1 {
                            IoDeviceType::Cga | IoDeviceType::Ega | IoDeviceType::Vga => {
                                match &mut self.video {
                                    VideoCardDispatch::Cga(cga) => {
                                        MemoryMappedDevice::write_u8( cga, address, data);
                                    }
                                    VideoCardDispatch::Ega(ega) => {
                                        MemoryMappedDevice::write_u8( ega, address, data);
                                    }
                                    VideoCardDispatch::Vga(vga) => {
                                        MemoryMappedDevice::write_u8(vga, address, data);
                                    }
                                    _ => {}
                                }
                            }
                            _=> {}
                        }                        
                        return Ok(map_entry.0.cycle_cost);
                    }
                }
                
                // We didn't match any mmio devices, write to memory.
                if self.memory_mask[address] & ROM_BIT == 0 {
                    self.memory[address] = data;                
                }
                return Ok(DEFAULT_WAIT_STATES);
            }
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    /*
    pub fn write_i8(&mut self, address: usize, data: i8) -> Result<u32, MemError> {
        if address < self.memory.len() {

            // Handle memory-mapped devices
            for map_entry in &self.mmio_map {
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
    }*/

    pub fn write_u16(&mut self, address: usize, data: u16) -> Result<u32, MemError> {
        if address < self.memory.len() - 1 {
            if address < self.mmio_data.first_map || address > self.mmio_data.last_map {
                // Address is not mapped.

                // Little Endian is LO byte first
                if self.memory_mask[address] & ROM_BIT == 0 {
                    self.memory[address] = (data & 0xFF) as u8;
                    self.memory[address+1] = (data >> 8) as u8;              
                }
                return Ok(DEFAULT_WAIT_STATES);
            }
            else {
                let mut handled = false;
                // Handle memory-mapped devices
                for map_entry in &self.mmio_map {
                    if address >= map_entry.0.address && address < map_entry.0.address + map_entry.0.size - 1 {

                        match map_entry.1 {
                            IoDeviceType::Cga | IoDeviceType::Ega | IoDeviceType::Vga => {
                                match &mut self.video {
                                    VideoCardDispatch::Cga(cga) => {
                                        MemoryMappedDevice::write_u8(cga, address, (data & 0xFF) as u8);
                                        MemoryMappedDevice::write_u8(cga, address + 1, (data >> 8) as u8);
                                    }
                                    VideoCardDispatch::Ega(ega) => {
                                        MemoryMappedDevice::write_u8(ega, address, (data & 0xFF) as u8);
                                        MemoryMappedDevice::write_u8(ega, address + 1, (data >> 8) as u8);
                                    }
                                    VideoCardDispatch::Vga(vga) => {
                                        MemoryMappedDevice::write_u8(vga, address, (data & 0xFF) as u8);
                                        MemoryMappedDevice::write_u8(vga, address + 1, (data >> 8) as u8);
                                    }
                                    _ => {}
                                }
                            }
                            _=> {}
                        }                             
                        return Ok(map_entry.0.cycle_cost);
                    }
                }

                // We didn't match any mmio devices, write to memory.
                if self.memory_mask[address] & ROM_BIT == 0 {
                    self.memory[address] = (data & 0xFF) as u8;
                    self.memory[address+1] = (data >> 8) as u8;              
                }
                return Ok(DEFAULT_WAIT_STATES);
            }
        }
        Err(MemError::ReadOutOfBoundsError)
    }
    
    /*
    pub fn write_i16(&mut self, address: usize, data: i16) -> Result<u32, MemError> {
        if address < self.memory.len() - 1 {

            // Handle memory-mapped devices
            for map_entry in &self.mmio_map {
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
    */

    /// Get bit flags for the specified byte at address
    #[inline]
    pub fn get_flags(&self, address: usize) -> u8 {
        if address < self.memory.len() - 1 {
            self.memory_mask[address]
        }
        else {
            0
        }
    }

    /// Set bit flags for the specified byte at address
    pub fn set_flags(&mut self, address: usize, flags: u8) {
        if address < self.memory.len() - 1 {     
            //log::trace!("set flag for address: {:05X}: {:02X}", address, flags);
            self.memory_mask[address] |= flags;
        }
    }

    /// Clear the specified flags for the specified byte at address
    /// Do not allow ROM bit to be cleared
    pub fn clear_flags(&mut self, address: usize, flags: u8) {
        if address < self.memory.len() - 1 {     
            self.memory_mask[address] &= !(flags & 0x7F);
        }
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

    /// Dump memory to a vector of string representations.
    /// 
    /// Does not honor memory mappings.
    pub fn dump_flat_vec(&self, address: usize, size: usize) -> Vec<String> {

        let mut vec = Vec::new();

        if address + size >= self.memory.len() {
            vec.push("REQUEST OUT OF BOUNDS".to_string());
            return vec;
        }
        else {

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

                vec.push(format!("{:05X} {} {}\n", display_address, dump_line, ascii_line));

                display_address += 16;
            }
        }
        vec
    }

    /// Dump memory to a vector of vectors of SyntaxTokens.
    /// 
    /// Does not honor memory mappings.
    pub fn dump_flat_tokens(&self, address: usize, mut size: usize) -> Vec<Vec<SyntaxToken>> {

        let mut vec: Vec<Vec<SyntaxToken>> = Vec::new();

        if address >= self.memory.len() {
            // Start address is invalid. Send only an error token.

            let mut linevec = Vec::new();

            linevec.push(SyntaxToken::ErrorString("REQUEST OUT OF BOUNDS".to_string()));
            vec.push(linevec);

            return vec;
        }
        else if address + size >= self.memory.len() {
            // Request size invalid. Send truncated result.

            let new_size = size - ((address + size) - self.memory.len());

            size = new_size
        }


            let dump_slice = &self.memory[address..address+size];
            let mut display_address = address;

            for dump_row in dump_slice.chunks_exact(16) {

                let mut line_vec = Vec::new();

                // Push memory flat address tokens
                line_vec.push(
                    SyntaxToken::MemoryAddressFlat(
                        display_address as u32,
                        format!("{:05X}", display_address)
                    )
                );

                // Build hex byte value tokens
                let mut i = 0;
                for byte in dump_row {
                    line_vec.push(
                        SyntaxToken::MemoryByteHexValue(
                            (display_address + i) as u32, 
                            *byte,
                            format!("{:02X}", *byte),
                            0
                        )
                    );
                    i += 1;
                }

                // Build ASCII representation tokens
                let mut i = 0;
                for byte in dump_row {
                    let char_str = match byte {
                        00..=31 => ".".to_string(),
                        32..=127 => format!("{}", *byte as char),
                        128.. => ".".to_string()
                    };
                    line_vec.push(
                        SyntaxToken::MemoryByteAsciiValue(
                            (display_address + i) as u32,
                            *byte,
                            char_str, 
                            0
                        )
                    );
                    i += 1;
                }

                vec.push(line_vec);
                display_address += 16;
            }

        vec
    }

    pub fn get_memory_debug(&mut self, address: usize) -> MemoryDebug {
        let mut debug = MemoryDebug {
            addr: format!("{:05X}", address),
            byte: String::new(),
            word: String::new(),
            dword: String::new(),
            instr: String::new()
        };

        if address < self.memory.len() - 1 {
            debug.byte = format!("{:02X}", self.memory[address]);
        }
        if address < self.memory.len() - 2 {
            debug.word = format!("{:04X}", (self.memory[address] as u16) | ((self.memory[address+1] as u16) << 8));
        }
        if address < self.memory.len() - 4 {
            debug.dword = format!("{:04X}", (self.memory[address] as u32) 
                | ((self.memory[address+1] as u32) << 8)
                | ((self.memory[address+2] as u32) << 16)
                | ((self.memory[address+3] as u32) << 24)
            );
        }

        self.seek(address);

        debug.instr = match Cpu::decode(self) {
            Ok(instruction) => {
                format!("{}", instruction)
            },
            Err(_) => {
                "Invalid".to_string()
            }
        };


        debug
    }
    
    pub fn install_devices(
        &mut self, 
        video_type: VideoType, 
        machine_desc: &MachineDescriptor, 
        video_trace: TraceLogger
    ) 
    {

        // Create PPI if PPI is defined for this machine type
        if machine_desc.have_ppi {
            self.ppi = Some(Ppi::new(machine_desc.machine_type, video_type, machine_desc.num_floppies));
            // Add PPI ports to io_map
            let port_list = self.ppi.as_mut().unwrap().port_list();
            self.io_map.extend(port_list.into_iter().map(|p| (p, IoDeviceType::Ppi)));
        }

        // Create the PIT. One PIT will always exist. Pick the device type from MachineDesc.
        let mut pit = Pit::new(machine_desc.pit_type);

        // Add PIT ports to io_map
        let port_list = pit.port_list();
        self.io_map.extend(port_list.into_iter().map(|p| (p, IoDeviceType::Pit)));
        
        // Tie gates for pit channel 0 & 1 high. 
        pit.set_channel_gate(0, true, self);
        pit.set_channel_gate(1, true, self);
        
        self.pit = Some(pit);

        // Create DMA. One DMA controller will always exist.
        let dma1 = DMAController::new();
        
        // Add DMA ports to io_map
        let port_list = dma1.port_list();
        self.io_map.extend(port_list.into_iter().map(|p| (p, IoDeviceType::DmaPrimary)));
        self.dma1 = Some(dma1);

        // Create PIC. One PIC will always exist.
        let pic1 = Pic::new();
        // Add PIC ports to io_map
        let port_list = pic1.port_list();
        self.io_map.extend(port_list.into_iter().map(|p| (p, IoDeviceType::PicPrimary)));
        self.pic1 = Some(pic1);

        // Create FDC. 
        let fdc = FloppyController::new();
        // Add FDC ports to io_map
        let port_list = fdc.port_list();
        self.io_map.extend(port_list.into_iter().map(|p| (p, IoDeviceType::FloppyController)));
        self.fdc = Some(fdc);

        // Create HDC. This should probably be specified in the MachineDesc with an option to override it
        // (Such as using a XTIDE instead of Xebec on PC & XT, perhaps)
        let hdc = HardDiskController::new(DRIVE_TYPE2_DIP);
        // Add HDC ports to io_map
        let port_list = hdc.port_list();
        self.io_map.extend(port_list.into_iter().map(|p| (p, IoDeviceType::HardDiskController)));
        self.hdc = Some(hdc);   

        // Create serial port.
        let serial = SerialPortController::new();
        // Add Serial Controller ports to io_map
        let port_list = serial.port_list();
        self.io_map.extend(port_list.into_iter().map(|p| (p, IoDeviceType::Serial)));
        self.serial = Some(serial);

        // Create mouse.
        let mouse = Mouse::new();
        self.mouse = Some(mouse);

        // Create video card depending on VideoType
        match video_type {
            VideoType::CGA => {
                let cga = CGACard::new(video_trace);
                let port_list = cga.port_list();
                self.io_map.extend(port_list.into_iter().map(|p| (p, IoDeviceType::Cga)));

                let mem_descriptor = MemRangeDescriptor::new(cga::CGA_MEM_ADDRESS, cga::CGA_MEM_APERTURE, false );
                self.register_map(IoDeviceType::Cga, mem_descriptor);

                self.video = VideoCardDispatch::Cga(cga)
            }
            VideoType::EGA => {
                let ega = EGACard::new();
                let port_list = ega.port_list();
                self.io_map.extend(port_list.into_iter().map(|p| (p, IoDeviceType::Ega)));

                let mem_descriptor = MemRangeDescriptor::new(ega::EGA_GFX_ADDRESS, ega::EGA_GFX_PLANE_SIZE, false );
                self.register_map(IoDeviceType::Ega, mem_descriptor);

                self.video = VideoCardDispatch::Ega(ega)
            }
            VideoType::VGA => {
                let vga = VGACard::new(video_trace);
                let port_list = vga.port_list();
                self.io_map.extend(port_list.into_iter().map(|p| (p, IoDeviceType::Vga)));

                //let mem_descriptor = MemRangeDescriptor::new(0xB8000, vga::VGA_TEXT_PLANE_SIZE, false );
                //cpu.bus_mut().register_map(IoDeviceType::Vga, mem_descriptor);

                let mem_descriptor = MemRangeDescriptor::new(vga::VGA_GFX_ADDRESS, vga::VGA_GFX_PLANE_SIZE, false );
                self.register_map(IoDeviceType::Vga, mem_descriptor);

                self.video = VideoCardDispatch::Vga(vga)
            }
            _=> {
                // MDA not implemented
                todo!("MDA not implemented");
            }
        }
    
    }

    pub fn run_devices(&mut self, us: f64, kb_byte_opt: Option<u8>, speaker_buf_producer: &mut Producer<u8>) {

        // Send keyboard events to devices.
        if let Some(kb_byte) = kb_byte_opt {
            //log::debug!("Got keyboard byte: {:02X}", kb_byte);
            if let Some(ppi) = &mut self.ppi {
                ppi.send_keyboard(kb_byte);
            }
            if let Some(pic) = &mut self.pic1 {
                pic.request_interrupt(1);
            }
        }

        // There will always be a PIC, so safe to unwrap.
        let pic = self.pic1.as_mut().unwrap();

        // There will always be a PIT, so safe to unwrap.
        let mut pit = self.pit.take().unwrap();

        // Run the PPI if present. PPI takes PIC to generate keyboard interrupts.
        if let Some(ppi) = &mut self.ppi {
            ppi.run(pic, us);
        }

        // Run the PIT. The PIT communicates with lots of things, so we send it the entire 
        // bus.
        pit.run(self, speaker_buf_producer, us);

        // Put the PIT back.
        self.pit = Some(pit);
        
        // Run the DMA controller.
        let mut dma1 = self.dma1.take().unwrap();
        dma1.run(self);

        // Run the FDC, passing it DMA controller while DMA is still unattached.
        if let Some(mut fdc) = self.fdc.take() {
            fdc.run(&mut dma1, self, us);
            self.fdc = Some(fdc);
        }

        // Run the HDC, passing it DMA controller while DMA is still unattached.
        if let Some(mut hdc) = self.hdc.take() {
            hdc.run(&mut dma1, self, us);
            self.hdc = Some(hdc);
        }

        // Replace the DMA controller.
        self.dma1 = Some(dma1);

        // Run the serial port and mouse.
        if let Some(serial) = &mut self.serial {
            serial.run(&mut self.pic1.as_mut().unwrap(), us);

            if let Some(mouse) = &mut self.mouse {
                mouse.run(serial, us);
            }            
        }

        // Run the video device.
        match &mut self.video {
            VideoCardDispatch::Cga(cga) => {
                cga.run(us);
            },
            VideoCardDispatch::Ega(ega) => {
                ega.run(us);
            }
            VideoCardDispatch::Vga(vga) => {
                vga.run(us);
            }
            VideoCardDispatch::None => {}
        }
    }

    /// Call the reset methods for all devices on the bus
    pub fn reset_devices(&mut self) {
        self.pit.as_mut().unwrap().reset();
        self.pic1.as_mut().unwrap().reset();
        //self.video.borrow_mut().reset();
    }

    pub fn io_read_u8(&mut self, port: u16) -> u8 {
        /*
        let handler_opt = self.handlers.get_mut(&port);
        if let Some(handler) = handler_opt {
            // We found a IoHandler in hashmap
            let mut writeable_thing = handler.device.borrow_mut();
            let func_ptr = handler.read_u8;
            func_ptr(&mut *writeable_thing, port)
        }
        else {
            // Unhandled IO address reads return 0xFF
            0xFF
        }
        */
        if let Some(device_id) = self.io_map.get(&port) {
            match device_id {
                IoDeviceType::Ppi => {
                    if let Some(ppi) = &mut self.ppi {
                        ppi.read_u8(port)
                    }
                    else {
                        NO_IO_BYTE
                    }
                }
                IoDeviceType::Pit => {
                    // There will always be a PIT, so safe to unwrap
                    self.pit.as_mut().unwrap().read_u8(port)
                }
                IoDeviceType::DmaPrimary => {
                    // There will always be a primary DMA, so safe to unwrap                    
                    self.dma1.as_mut().unwrap().read_u8(port)
                }
                IoDeviceType::DmaSecondary => {
                    // Secondary DMA may not exist
                    if let Some(dma2) = &mut self.dma2 {
                        dma2.read_u8(port)
                    }
                    else {
                        NO_IO_BYTE
                    }
                }
                IoDeviceType::PicPrimary => {
                    // There will always be a primary PIC, so safe to unwrap
                    self.pic1.as_mut().unwrap().read_u8(port)
                }
                IoDeviceType::PicSecondary => {
                    // Secondary PIC may not exist
                    if let Some(pic2) = &mut self.pic2 {
                        pic2.read_u8(port)
                    }
                    else {
                        NO_IO_BYTE
                    }
                }
                IoDeviceType::FloppyController => {
                    if let Some(fdc) = &mut self.fdc {
                        fdc.read_u8(port)
                    }                     
                    else {
                        NO_IO_BYTE
                    }      
                }
                IoDeviceType::HardDiskController => {
                    if let Some(hdc) = &mut self.hdc {
                        hdc.read_u8(port)
                    }
                    else {
                        NO_IO_BYTE
                    }        
                }
                IoDeviceType::Serial => {
                    if let Some(serial) = &mut self.serial {
                        // Serial port write does not need bus.
                        serial.read_u8(port)
                    } 
                    else {
                        NO_IO_BYTE
                    }
                }
                       
                IoDeviceType::Cga | IoDeviceType::Ega | IoDeviceType::Vga => {
                    match &mut self.video {
                        VideoCardDispatch::Cga(cga) => {
                            IoDevice::read_u8(cga, port)
                        },
                        VideoCardDispatch::Ega(ega) => {
                            IoDevice::read_u8(ega, port)
                        }
                        VideoCardDispatch::Vga(vga) => {
                            IoDevice::read_u8(vga, port)
                        }
                        VideoCardDispatch::None => NO_IO_BYTE
                    }
                }
                _ => {
                    NO_IO_BYTE
                }
            }
        }
        else {
            // Unhandled IO address read
            NO_IO_BYTE
        }

    }

    pub fn io_write_u8(&mut self, port: u16, data: u8) {
        /*
        let handler_opt = self.handlers.get_mut(&port);
        if let Some(handler) = handler_opt {
            // We found a IoHandler in hashmap
            let mut writeable_thing = handler.device.borrow_mut();
            let func_ptr = handler.write_u8;
            func_ptr(&mut *writeable_thing, port, data);
        }
        */

        if let Some(device_id) = self.io_map.get(&port) {
            match device_id {
                IoDeviceType::Ppi => {
                    if let Some(mut ppi) = self.ppi.take() {
                        ppi.write_u8(port, data, Some(self));
                        self.ppi = Some(ppi);
                    }
                }
                IoDeviceType::Pit => {
                    if let Some(mut pit) = self.pit.take() {
                        pit.write_u8(port, data, Some(self));
                        self.pit = Some(pit);
                    }
                }
                IoDeviceType::DmaPrimary => {
                    if let Some(mut dma1) = self.dma1.take() {
                        dma1.write_u8(port, data, Some(self));
                        self.dma1 = Some(dma1);
                    }
                }
                IoDeviceType::DmaSecondary => {
                    if let Some(mut dma2) = self.dma2.take() {
                        dma2.write_u8(port, data, Some(self));
                        self.dma2 = Some(dma2);
                    }                    
                }
                IoDeviceType::PicPrimary => {
                    if let Some(mut pic1) = self.pic1.take() {
                        pic1.write_u8(port, data, Some(self));
                        self.pic1 = Some(pic1);
                    }                    
                }
                IoDeviceType::PicSecondary => {
                    if let Some(mut pic2) = self.pic2.take() {
                        pic2.write_u8(port, data, Some(self));
                        self.pic2 = Some(pic2);
                    }                               
                }
                IoDeviceType::FloppyController => {
                    if let Some(mut fdc) = self.fdc.take() {
                        fdc.write_u8(port, data, Some(self));
                        self.fdc = Some(fdc);
                    }                           
                }
                IoDeviceType::HardDiskController => {
                    if let Some(mut hdc) = self.hdc.take() {
                        hdc.write_u8(port, data, Some(self));
                        self.hdc = Some(hdc);
                    }                            
                }
                IoDeviceType::Serial => {
                    if let Some(serial) = &mut self.serial {
                        // Serial port write does not need bus.
                        serial.write_u8(port, data, None);
                    }
                }
                IoDeviceType::Cga | IoDeviceType::Ega | IoDeviceType::Vga => {
                    match &mut self.video {
                        VideoCardDispatch::Cga(cga) => {
                            IoDevice::write_u8(cga, port, data, None)
                        },
                        VideoCardDispatch::Ega(ega) => {
                            IoDevice::write_u8(ega, port, data, None)
                        }
                        VideoCardDispatch::Vga(vga) => {
                            IoDevice::write_u8(vga, port, data, None)
                        }
                        VideoCardDispatch::None => {}
                    }
                }
                _ => {}
            }
        }

    }

    // Device accessors
    pub fn pit(&self) -> &Option<Pit> {
        &self.pit
    }

    pub fn pit_mut(&mut self) -> &mut Option<Pit> {
        &mut self.pit
    }

    pub fn pic_mut(&mut self) -> &mut Option<Pic> {
        &mut self.pic1
    }

    pub fn ppi_mut(&mut self) -> &mut Option<Ppi> {
        &mut self.ppi
    }

    pub fn dma_mut(&mut self) -> &mut Option<DMAController> {
        &mut self.dma1
    }

    pub fn serial_mut(&mut self) -> &mut Option<SerialPortController> {
        &mut self.serial
    }

    pub fn fdc_mut(&mut self) -> &mut Option<FloppyController> {
        &mut self.fdc
    }

    pub fn hdc_mut(&mut self) -> &mut Option<HardDiskController> {
        &mut self.hdc
    }    

    pub fn mouse_mut(&mut self) -> &mut Option<Mouse> {
        &mut self.mouse
    }

    pub fn video(&self) -> Option<Box<&dyn VideoCard>> {

        match &self.video {
            VideoCardDispatch::Cga(cga) => {
                Some(Box::new(cga as &dyn VideoCard))
            }
            VideoCardDispatch::Ega(ega) => {
                Some(Box::new(ega as &dyn VideoCard))
            }
            VideoCardDispatch::Vga(vga) => {
                Some(Box::new(vga as &dyn VideoCard))
            }
            VideoCardDispatch::None => {
                None
            }
        }
    }

    pub fn video_mut(&mut self) -> Option<Box<&mut dyn VideoCard>> {

        match &mut self.video {
            VideoCardDispatch::Cga(cga) => {
                Some(Box::new(cga as &mut dyn VideoCard))
            }
            VideoCardDispatch::Ega(ega) => {
                Some(Box::new(ega as &mut dyn VideoCard))
            }
            VideoCardDispatch::Vga(vga) => {
                Some(Box::new(vga as &mut dyn VideoCard))
            }
            VideoCardDispatch::None => {
                None
            }
        }
    }
}
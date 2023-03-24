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
use crate::videocard::{VideoCard, VideoCardDispatch};

use crate::ega::EGACard;
use crate::vga::VGACard;
use crate::cga::*;
use crate::memerror::MemError;

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
    fn write_u8(&mut self, port: u16, data: u8, bus: &mut BusInterface);
    fn port_list(&self) -> Vec<u16>;
}

pub struct BusInterface<'b> {
    memory: Vec<u8>,
    memory_mask: Vec<u8>,
    desc_vec: Vec<MemRangeDescriptor>,
    map: Vec<(MemRangeDescriptor, Rc<RefCell<dyn MemoryMappedDevice>>)>,
    first_map: usize,
    last_map: usize,
    cursor: usize,

    io_map: HashMap<u16, IoDeviceType>,
    ppi: Option<Ppi>,
    pit: Option<Pit>,
    dma1: DMAController,
    dma2: Option<DMAController>,
    pic1: Pic,
    pic2: Option<Pic>,
    serial: Option<SerialPortController>,
    fdc: FloppyController,
    hdc: Option<HardDiskController>,
    mouse: Option<Mouse>,
    video: VideoCardDispatch<'b>,
    video_ref: Option<&'b mut dyn VideoCard>
}

impl<'b> ByteQueue for BusInterface<'b> {
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

    fn q_peek_u8(&self) -> u8 {
        if self.cursor < self.memory.len() {
            let b: u8 = self.memory[self.cursor];
            return b
        }
        0xffu8
    }

    fn q_peek_i8(&self) -> i8 {
        if self.cursor < self.memory.len() {
            let b: i8 = self.memory[self.cursor] as i8;
            return b
        }
        -1i8   
    }

    fn q_peek_u16(&self) -> u16 {
        if self.cursor < self.memory.len() - 1 {
            let w: u16 = self.memory[self.cursor] as u16 | (self.memory[self.cursor+1] as u16) << 8;
            return w
        }
        0xffffu16   
    }    

    fn q_peek_i16(&self) -> i16 {
        if self.cursor < self.memory.len() - 1 {
            let w: i16 = (self.memory[self.cursor] as u16 | (self.memory[self.cursor+1] as u16) << 8) as i16;
            return w
        }
        -1i16
    }      
}

impl<'b> Default for BusInterface<'b> {
    fn default() -> Self {
        BusInterface {
            memory: vec![0; ADDRESS_SPACE],
            memory_mask: vec![0; ADDRESS_SPACE],
            desc_vec: Vec::new(),
            map: Vec::new(),
            cursor: 0,
            first_map: 0,
            last_map: 0,

            io_map: HashMap::new(),
            ppi: None,
            pit: None,
            dma1: DMAController::new(),
            dma2: None,
            pic1: Pic::new(),
            pic2: None,            
            serial: None,
            fdc: FloppyController::new(),
            hdc: None,
            mouse: None,
            video: VideoCardDispatch::None,
            video_ref: None
        }        
    }
}

impl<'b> BusInterface<'b> {
    pub fn new() -> BusInterface<'b> {
        BusInterface {
            memory: vec![0; ADDRESS_SPACE],
            memory_mask: vec![0; ADDRESS_SPACE],
            desc_vec: Vec::new(),
            map: Vec::new(),
            cursor: 0,
            first_map: 0,
            last_map: 0,

            io_map: HashMap::new(),
            ppi: None,
            pit: None,
            dma1: DMAController::new(),
            dma2: None,
            pic1: Pic::new(),
            pic2: None,        
            serial: None,    
            fdc: FloppyController::new(),
            hdc: None,
            mouse: None,
            video: VideoCardDispatch::None,
            video_ref: None
        }
    }

    pub fn size(&self) -> usize {
        self.memory.len()
    }

    pub fn get_ppi_mut(&mut self) -> &mut Option<Ppi> {
        &mut self.ppi
    }

    pub fn get_dma_mut(&mut self) -> &mut DMAController {
        &mut self.dma1
    }

    pub fn get_pic_mut(&mut self) -> &mut Pic {
        &mut self.pic1
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
    
    pub fn install_devices<TraceWriter: Write + 'b>(
        &mut self, 
        video_type: VideoType, 
        machine_desc: &MachineDescriptor, 
        video_trace_writer: Option<TraceWriter>
    ) 
    {

        // Create PPI if PPI is defined for this machine type
        if machine_desc.have_ppi {
            let ppi = Ppi::new(machine_desc.machine_type, video_type, machine_desc.num_floppies);
            // Add PPI ports to io_map
            self.io_map.extend(ppi.port_list().iter().map(|p| (*p, IoDeviceType::Ppi)));
            self.ppi = Some(ppi);
        }

        // Create the PIT. One PIT will always exist.
        self.pit = Some(Pit::new());
        self.io_map.extend(self.pit.as_mut().unwrap().port_list().iter().map(|p| (*p, IoDeviceType::Pit)));

        // Create DMA. One DMA controller will always exist.
        self.dma1 = DMAController::new();
        // Add DMA ports to io_map
        self.io_map.extend(self.dma1.port_list().iter().map(|p| (*p, IoDeviceType::DmaPrimary)));

        // Create PIC. One PIC will always exist.
        self.pic1 = Pic::new();
        // Add PIC ports to io_map
        self.io_map.extend(self.pic1.port_list().iter().map(|p| (*p, IoDeviceType::PicPrimary)));

        // Create video card depending on VideoType
        match video_type {
            VideoType::CGA => {
                self.video = VideoCardDispatch::Cga(CGACard::new())
            }
            VideoType::EGA => {
                self.video = VideoCardDispatch::Ega(EGACard::new())
            }
            VideoType::VGA => {
                self.video = VideoCardDispatch::Vga(VGACard::new(video_trace_writer))
            }
            _=> {
                // MDA not implemented
                todo!("MDA not implemented");
            }
        }
    
    }

    pub fn run_devices(&mut self, us: f64, speaker_buf_producer: &mut Producer<u8>) {

    }

    /// Call the reset methods for all devices on the bus
    pub fn reset_devices(&mut self) {
        self.pit.as_mut().unwrap().reset();
        self.pic1.reset();
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
                        0xFF
                    }
                }
                IoDeviceType::Pit => {
                    self.pit.as_mut().unwrap().read_u8(port)
                }
                IoDeviceType::DmaPrimary => {
                    self.dma1.read_u8(port)
                }
                IoDeviceType::DmaSecondary => {
                    0xFF
                }
                IoDeviceType::PicPrimary => {
                    self.pic1.read_u8(port)
                }
                IoDeviceType::PicSecondary => {
                    0xFF
                }
                IoDeviceType::Cga => {
                    0xFF
                }
                IoDeviceType::Ega => {
                    0xFF
                }
                IoDeviceType::Vga => {
                    0xFF
                }
                _ => {
                    0xFF
                }
            }
        }
        else {
            // Unhandled IO address reads return 0xFF
            0xFF
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
                        ppi.write_u8(port, data, self);
                        self.ppi = Some(ppi);
                    }
                }
                IoDeviceType::Pit => {
                    if let Some(mut pit) = self.pit.take() {
                        pit.write_u8(port, data, self);
                        self.pit = Some(pit);
                    }

                }
                IoDeviceType::DmaPrimary => {}
                IoDeviceType::DmaSecondary => {}
                IoDeviceType::PicPrimary => {}
                IoDeviceType::PicSecondary => {}
                IoDeviceType::FloppyController => {}
                IoDeviceType::HardDiskController => {}
                IoDeviceType::Cga => {}
                IoDeviceType::Ega => {}
                IoDeviceType::Vga => {}
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

    pub fn pic_mut(&mut self) -> &mut Pic {
        &mut self.pic1
    }

    pub fn ppi_mut(&mut self) -> &mut Option<Ppi> {
        &mut self.ppi
    }

    pub fn dma_mut(&mut self) -> &mut DMAController {
        &mut self.dma1
    }

    pub fn serial_mut(&mut self) -> &mut Option<SerialPortController> {
        &mut self.serial
    }

    pub fn fdc_mut(&mut self) -> &mut FloppyController {
        &mut self.fdc
    }

    pub fn hdc_mut(&mut self) -> &mut Option<HardDiskController> {
        &mut self.hdc
    }    

    pub fn mouse(&mut self) -> &Option<Mouse> {
        &self.mouse
    }

    pub fn video_mut(&mut self) -> &'b mut Option<&mut dyn VideoCard> {

        match &mut self.video {
            VideoCardDispatch::Cga(cga) => {
                self.video_ref = Some(cga);
            }
            VideoCardDispatch::Ega(ega) => {
                self.video_ref = Some(ega)
            }
            VideoCardDispatch::Vga(vga) => {
                self.video_ref = Some(vga)
            }
            VideoCardDispatch::None => {
                self.video_ref = None
            }
        }
        &mut self.video_ref
    }

}
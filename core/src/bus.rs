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

    bus.rs

    Implement the system bus.

    The bus module implements memory read and write routines.

    The bus module is the logical owner of all memory and devices in the
    system, and implements both IO and MMIO dispatch to installed devices.
*/

#![allow(dead_code)]
use std::{
    collections::{HashMap, VecDeque},
    fmt,
    path::Path
};

use ringbuf::{Producer};

use crate::cpu_808x::*;
use crate::bytequeue::*;

use crate::syntax_token::SyntaxToken;
use crate::machine::KeybufferEntry;
use crate::machine_manager::MachineDescriptor;
use crate::config::{ClockingMode, KeyboardType, VideoType};

use crate::devices::{
    pit::Pit,
    pic::*,
    dma::*,
    ppi::*,
    serial::*,
    fdc::FloppyController,
    hdc::*,
    mouse::*,
    keyboard::*,
};

use crate::tracelogger::TraceLogger;
use crate::videocard::{VideoCard, VideoCardDispatch};

use crate::devices::cga::{self, CGACard};
#[cfg(feature = "ega")]
use crate::devices::ega::{self, EGACard};
#[cfg(feature = "vga")]
use crate::devices::vga::{self, VGACard};
use crate::memerror::MemError;

pub const NO_IO_BYTE: u8 = 0xFF; // This is the byte read from a unconnected IO address.
pub const FLOATING_BUS_BYTE: u8 = 0x00; // This is the byte read from an unmapped memory address.

const ADDRESS_SPACE: usize = 1_048_576;
const DEFAULT_WAIT_STATES: u32 = 0;

const MMIO_MAP_SIZE: usize =  0x2000;
const MMIO_MAP_SHIFT: usize = 13;

pub const MEM_ROM_BIT: u8   = 0b1000_0000; // Bit to signify that this address is ROM
pub const MEM_RET_BIT: u8   = 0b0100_0000; // Bit to signify that this address is a return address for a CALL or INT
pub const MEM_BPE_BIT: u8   = 0b0010_0000; // Bit to signify that this address is associated with a breakpoint on execute
pub const MEM_BPA_BIT: u8   = 0b0001_0000; // Bit to signify that this address is associated with a breakpoint on access
pub const MEM_CP_BIT: u8    = 0b0000_1000; // Bit to signify that this address is a ROM checkpoint
pub const MEM_MMIO_BIT: u8  = 0b0000_0100; // Bit to signify that this address is MMIO mapped

pub const KB_UPDATE_RATE: f64 = 5000.0; // Keyboard device update rate in microseconds 

#[derive (Copy, Clone, Debug)]
pub enum ClockFactor {
    Divisor(u8),
    Multiplier(u8)
}

#[derive (Copy, Clone, Debug)]
pub enum DeviceRunTimeUnit {
    SystemTicks(u32),
    Microseconds(f64),
}

pub enum DeviceEvent {
    DramRefreshUpdate(u16, u16, u32),
    DramRefreshEnable(bool)
}

pub trait MemoryMappedDevice {  
    fn get_read_wait(&mut self, address: usize, cycles: u32) -> u32;
    fn mmio_read_u8(&mut self, address: usize, cycles: u32) -> (u8, u32);
    fn mmio_read_u16(&mut self, address: usize, cycles: u32) -> (u16, u32);
    fn mmio_peek_u8(&self, address: usize) -> u8;
    fn mmio_peek_u16(&self, address: usize) -> u16;

    fn get_write_wait(&mut self, address: usize, cycles: u32) -> u32;
    fn mmio_write_u8(&mut self, address: usize, data: u8, cycles: u32) -> u32; 
    fn mmio_write_u16(&mut self, address: usize, data: u16, cycles: u32) -> u32;
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
    fn read_u8(&mut self, port: u16, delta: DeviceRunTimeUnit ) -> u8;
    fn write_u8(&mut self, port: u16, data: u8, bus: Option<&mut BusInterface>, delta: DeviceRunTimeUnit);
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

#[derive (Copy, Clone)]
pub enum MmioDeviceType {
    None,
    Memory,
    Video,
    Cga,
    Ega,
    Vga,
    Rom
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
    cpu_factor: ClockFactor,
    machine_desc: Option<MachineDescriptor>,
    keyboard_type: KeyboardType,
    keyboard: Keyboard,
    memory: Vec<u8>,
    memory_mask: Vec<u8>,
    desc_vec: Vec<MemRangeDescriptor>,
    mmio_map: Vec<(MemRangeDescriptor, MmioDeviceType)>,
    mmio_map_fast: [MmioDeviceType; 128],
    mmio_data: MmioData,
    cursor: usize,

    io_map: HashMap<u16, IoDeviceType>,
    ppi: Option<Ppi>,
    pit: Option<Pit>,
    dma_counter: u16,
    dma1: Option<DMAController>,
    dma2: Option<DMAController>,
    pic1: Option<Pic>,
    pic2: Option<Pic>,
    serial: Option<SerialPortController>,
    fdc: Option<FloppyController>,
    hdc: Option<HardDiskController>,
    mouse: Option<Mouse>,
    video: VideoCardDispatch,

    cycles_to_ticks: [u32; 256],    // TODO: Benchmarks don't show any faster than raw multiplication. It's not slower either though.
    pit_ticks_advance: u32,         // We can schedule extra PIT ticks to add when run() occurs. This is generally used for PIT phase offset adjustment.

    timer_trigger1_armed: bool,
    timer_trigger2_armed: bool,

    cga_tick_accum: u32,
    kb_us_accum: f64,
    refresh_active: bool
}

impl ByteQueue for BusInterface {
    fn seek(&mut self, pos: usize) {
        self.cursor = pos;
    }

    fn tell(&self) -> usize {
        self.cursor
    }

    fn wait(&mut self, _cycles: u32) {}
    fn wait_i(&mut self, _cycles: u32, _instr: &[u16]) {}
    fn wait_comment(&mut self, _comment: &str) {}
    fn set_pc(&mut self, _pc: u16) {}

    fn q_read_u8(&mut self, _dtype: QueueType, _reader: QueueReader) -> u8 {
        if self.cursor < self.memory.len() {
            let b: u8 = self.memory[self.cursor];
            self.cursor += 1;
            return b
        }
        0xffu8
    }

    fn q_read_i8(&mut self, _dtype: QueueType, _reader: QueueReader) -> i8 {
        if self.cursor < self.memory.len() {
            let b: i8 = self.memory[self.cursor] as i8;
            self.cursor += 1;
            return b
        }
        -1i8       
    }

    fn q_read_u16(&mut self, _dtype: QueueType, _reader: QueueReader) -> u16 {
        if self.cursor < self.memory.len() - 1 {
            let w: u16 = self.memory[self.cursor] as u16 | (self.memory[self.cursor+1] as u16) << 8;
            self.cursor += 2;
            return w
        }
        0xffffu16   
    }

    fn q_read_i16(&mut self, _dtype: QueueType, _reader: QueueReader) -> i16 {
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

    fn q_peek_farptr16(&mut self) -> (u16, u16) {         
        if self.cursor < self.memory.len() - 3 {
            let offset: u16 = self.memory[self.cursor] as u16 | (self.memory[self.cursor+1] as u16) << 8;
            let segment: u16 = self.memory[self.cursor+2] as u16 | (self.memory[self.cursor+3] as u16) << 8;
            return (segment, offset)
        }
        (0xffffu16, 0xffffu16)
    }
}

impl Default for BusInterface {
    fn default() -> Self {
        BusInterface {

            cpu_factor: ClockFactor::Divisor(3),

            machine_desc: None,
            keyboard_type: KeyboardType::ModelF,
            keyboard: Keyboard::new(KeyboardType::ModelF, false),
            memory: vec![0; ADDRESS_SPACE],
            memory_mask: vec![0; ADDRESS_SPACE],
            desc_vec: Vec::new(),
            mmio_map: Vec::new(),
            mmio_map_fast: [MmioDeviceType::Memory; ADDRESS_SPACE >> MMIO_MAP_SHIFT],
            mmio_data: MmioData::new(),
            cursor: 0,

            io_map: HashMap::new(),
            ppi: None,
            pit: None,
            dma_counter: 0,
            dma1: None,
            dma2: None,
            pic1: None,
            pic2: None,            
            serial: None,
            fdc: None,
            hdc: None,
            mouse: None,
            video: VideoCardDispatch::None,

            cycles_to_ticks: [0; 256],
            pit_ticks_advance: 0,

            timer_trigger1_armed: false,
            timer_trigger2_armed: false,     

            cga_tick_accum: 0,
            kb_us_accum: 0.0,
            refresh_active: false
        }        
    }
}

impl BusInterface {
    pub fn new(
        cpu_factor: ClockFactor, 
        machine_desc: MachineDescriptor,
        keyboard_type: KeyboardType,
    ) -> BusInterface 
    {
        BusInterface {
            cpu_factor,
            machine_desc: Some(machine_desc),
            keyboard_type,
            keyboard: Keyboard::new(keyboard_type, false),
            ..BusInterface::default()     
        }
    }

    pub fn size(&self) -> usize {
        self.memory.len()
    }

    /// Register a memory-mapped device.
    /// 
    /// The MemoryMappedDevice trait's read & write methods will be called instead for memory in the range
    /// specified withing MemRangeDescriptor.
    pub fn register_map(&mut self, device: MmioDeviceType, mem_descriptor: MemRangeDescriptor) {
        if mem_descriptor.address < self.mmio_data.first_map {
            self.mmio_data.first_map = mem_descriptor.address;
        }
        if (mem_descriptor.address + mem_descriptor.size) > self.mmio_data.last_map {
            self.mmio_data.last_map = mem_descriptor.address + mem_descriptor.size;
        }

        // Mark memory flag bit as MMIO for this range.
        for i in mem_descriptor.address..(mem_descriptor.address + mem_descriptor.size) {
            self.memory_mask[i] |= MEM_MMIO_BIT;
        }

        // Add entry to mmio_map_fast

        assert!(mem_descriptor.size % MMIO_MAP_SIZE == 0);
        let map_segs = mem_descriptor.size / MMIO_MAP_SIZE;

        for i in 0..map_segs {
            self.mmio_map_fast[(mem_descriptor.address >> MMIO_MAP_SHIFT) + i] = device.clone();
        }
        
        self.mmio_map.push((mem_descriptor, device));
    }

    pub fn copy_from(&mut self, src: &[u8], location: usize, cycle_cost: u32, read_only: bool) -> Result<(), bool> {
        
        let src_size = src.len();
        if location as usize + src_size > self.memory.len() {
            // copy request goes out of bounds
            log::error!("copy out of range: {} len: {}", location, src_size);
            return Err(false)
        }
        
        let mem_slice: &mut [u8] = &mut self.memory[location..location + src_size];
        let mask_slice: &mut [u8] = &mut self.memory_mask[location..location + src_size];

        for (dst, src) in mem_slice.iter_mut().zip(src) {
            *dst = *src;
        }

        // Write access mask
        let access_bit = match read_only {
            true => MEM_ROM_BIT,
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

    pub fn get_vec_at(&self, start: usize, len: usize ) -> Vec<u8> {
        self.memory[start..start+len].to_vec()
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

    pub fn set_cpu_factor(&mut self, cpu_factor: ClockFactor) {
        self.cpu_factor = cpu_factor;

        self.recalculate_cycle_lut();
    }

    pub fn recalculate_cycle_lut(&mut self) {

        for c in 0..256 {
            self.cycles_to_ticks[c as usize] = self.cpu_cycles_to_system_ticks(c);
        }
    }

    #[inline]
    /// Convert a count of CPU cycles to system clock ticks based on the current CPU
    /// clock divisor.
    fn cpu_cycles_to_system_ticks(&self, cycles: u32) -> u32 {
        match self.cpu_factor {
            ClockFactor::Divisor(n) => cycles * (n as u32),
            ClockFactor::Multiplier(n) => cycles / (n as u32)
        }
    }    

    #[inline]
    /// Convert a count of system clock ticks to CPU cycles based on the current CPU
    /// clock divisor. If a clock Divisor is set, the dividend will be rounded upwards.
    fn system_ticks_to_cpu_cycles(&self, ticks: u32) -> u32 {
        match self.cpu_factor {
            ClockFactor::Divisor(n) => (ticks + (n as u32) - 1) / (n as u32),
            ClockFactor::Multiplier(n) => ticks * (n as u32)
        }
    }        

    pub fn get_read_wait(&mut self, address: usize, cycles: u32) -> Result<u32, MemError> {
        if address < self.memory.len() {
            if address < self.mmio_data.first_map || address > self.mmio_data.last_map {
                // Address is not mapped.
                return Ok(DEFAULT_WAIT_STATES)
            }
            else {
                // Handle memory-mapped devices
                for map_entry in &self.mmio_map {
                    if address >= map_entry.0.address && address < map_entry.0.address + map_entry.0.size {

                        // Convert cpu cycles to system ticks
                        let system_ticks = self.cpu_cycles_to_system_ticks(cycles);

                        match map_entry.1 {
                            MmioDeviceType::Video => {
                                match &mut self.video {
                                    VideoCardDispatch::Cga(cga) => {
                                        let syswait = cga.get_read_wait(address, system_ticks);
                                        return Ok(self.system_ticks_to_cpu_cycles(syswait));
                                    }
                                    #[cfg(feature = "ega")]
                                    VideoCardDispatch::Ega(ega) => {
                                        let syswait = ega.get_read_wait(address, system_ticks);
                                        return Ok(self.system_ticks_to_cpu_cycles(syswait));
                                    }
                                    #[cfg(feature = "vga")]
                                    VideoCardDispatch::Vga(vga) => {
                                        let syswait = vga.get_read_wait(address, system_ticks);
                                        return Ok(self.system_ticks_to_cpu_cycles(syswait));
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
                return Ok(DEFAULT_WAIT_STATES)
            }
        }
        Err(MemError::ReadOutOfBoundsError)        
    }

    pub fn get_write_wait(&mut self, address: usize, cycles: u32) -> Result<u32, MemError> {
        if address < self.memory.len() {
            if address < self.mmio_data.first_map || address > self.mmio_data.last_map {
                // Address is not mapped.
                return Ok(DEFAULT_WAIT_STATES)
            }
            else {
                // Handle memory-mapped devices
                for map_entry in &self.mmio_map {
                    if address >= map_entry.0.address && address < map_entry.0.address + map_entry.0.size {

                        // Convert cpu cycles to system ticks
                        let system_ticks = self.cpu_cycles_to_system_ticks(cycles);

                        match map_entry.1 {
                            MmioDeviceType::Video => {
                                match &mut self.video {
                                    VideoCardDispatch::Cga(cga) => {
                                        let syswait = cga.get_write_wait(address, system_ticks);
                                        return Ok(self.system_ticks_to_cpu_cycles(syswait));
                                    }
                                    #[cfg(feature = "ega")]
                                    VideoCardDispatch::Ega(ega) => {
                                        let syswait = ega.get_write_wait(address, system_ticks);
                                        return Ok(self.system_ticks_to_cpu_cycles(syswait));
                                    }
                                    #[cfg(feature = "vga")]
                                    VideoCardDispatch::Vga(vga) => {
                                        let syswait = vga.get_write_wait(address, system_ticks);
                                        return Ok(self.system_ticks_to_cpu_cycles(syswait));
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
                return Ok(DEFAULT_WAIT_STATES)
            }
        }
        Err(MemError::ReadOutOfBoundsError)        
    }    

    pub fn read_u8(&mut self, address: usize, cycles: u32) -> Result<(u8, u32), MemError> {
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

                        // Convert cpu cycles to system ticks
                        let system_ticks = self.cpu_cycles_to_system_ticks(cycles);

                        match map_entry.1 {
                            MmioDeviceType::Video => {
                                match &mut self.video {
                                    VideoCardDispatch::Cga(cga) => {
                                        let (data, syswait) = MemoryMappedDevice::mmio_read_u8(cga, address, system_ticks);
                                        return Ok((data, self.system_ticks_to_cpu_cycles(syswait)));
                                    }
                                    #[cfg(feature = "ega")]
                                    VideoCardDispatch::Ega(ega) => {
                                        let (data, syswait) = MemoryMappedDevice::mmio_read_u8(ega, address, system_ticks);
                                        return Ok((data, 0));
                                    }
                                    #[cfg(feature = "vga")]
                                    VideoCardDispatch::Vga(vga) => {
                                        let (data, syswait) = MemoryMappedDevice::mmio_read_u8(vga, address, system_ticks);
                                        return Ok((data, 0));
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

    pub fn peek_u8(&self, address: usize) -> Result<u8, MemError> {
        if address < self.memory.len() {
            if address < self.mmio_data.first_map || address > self.mmio_data.last_map {
                // Address is not mapped.
                let b: u8 = self.memory[address];
                return Ok(b)
            }
            else {
                // Handle memory-mapped devices
                for map_entry in &self.mmio_map {
                    if address >= map_entry.0.address && address < map_entry.0.address + map_entry.0.size {

                        match map_entry.1 {
                            MmioDeviceType::Video => {
                                match &self.video {
                                    VideoCardDispatch::Cga(cga) => {
                                        let data = MemoryMappedDevice::mmio_peek_u8(cga, address);
                                        return Ok(data);
                                    }
                                    #[cfg(feature = "ega")]
                                    VideoCardDispatch::Ega(ega) => {
                                        let data = MemoryMappedDevice::mmio_peek_u8(ega, address);
                                        return Ok(data);
                                    }
                                    #[cfg(feature = "vga")]
                                    VideoCardDispatch::Vga(vga) => {
                                        let data = MemoryMappedDevice::mmio_peek_u8(vga, address);
                                        return Ok(data);
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
                return Ok(b)
            }
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn read_u16(&mut self, address: usize, cycles: u32) -> Result<(u16, u32), MemError> {
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

                        // Convert cpu cycles to system ticks
                        let system_ticks = self.cpu_cycles_to_system_ticks(cycles);

                        match map_entry.1 {
                            MmioDeviceType::Cga | MmioDeviceType::Ega | MmioDeviceType::Vga => {
                                match &mut self.video {
                                    VideoCardDispatch::Cga(cga) => {
                                        //let (data, syswait) = MemoryMappedDevice::read_u16(cga, address, system_ticks);
                                        let (data, syswait) = cga.mmio_read_u16(address, system_ticks);
                                        return Ok((data, self.system_ticks_to_cpu_cycles(syswait)));
                                    }
                                    #[cfg(feature = "ega")]
                                    VideoCardDispatch::Ega(ega) => {
                                        let (data, syswait) = MemoryMappedDevice::mmio_read_u16(ega, address, system_ticks);
                                        return Ok((data, 0));
                                    }
                                    #[cfg(feature = "vga")]
                                    VideoCardDispatch::Vga(vga) => {
                                        let (data, syswait) = MemoryMappedDevice::mmio_read_u16(vga, address, system_ticks);
                                        return Ok((data, 0));
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

    pub fn write_u8(&mut self, address: usize, data: u8, cycles: u32) -> Result<u32, MemError> {
        if address < self.memory.len() {
            if self.memory_mask[address] & (MEM_MMIO_BIT | MEM_ROM_BIT) == 0 {
                // Address is not mapped and not ROM, write to it.
                self.memory[address] = data;                
                return Ok(DEFAULT_WAIT_STATES);
            }
            else {
                /*
                // Handle memory-mapped devices
                for map_entry in &self.mmio_map {
                    if address >= map_entry.0.address && address < map_entry.0.address + map_entry.0.size {

                        // Convert cpu cycles to system ticks
                        let system_ticks = self.cpu_cycles_to_system_ticks(cycles);
                        //let system_ticks = self.cycles_to_ticks[cycles as usize];

                        match map_entry.1 {
                            MmioDeviceType::Cga | MmioDeviceType::Ega | MmioDeviceType::Vga => {
                                match &mut self.video {
                                    VideoCardDispatch::Cga(cga) => {
                                        let syswait = MemoryMappedDevice::mmio_write_u8(cga, address, data, system_ticks);
                                        //return Ok(self.system_ticks_to_cpu_cycles(syswait)); // temporary wait state value. 
                                        return Ok(0);
                                    }
                                    #[cfg(feature = "ega")]
                                    VideoCardDispatch::Ega(ega) => {
                                        MemoryMappedDevice::mmio_write_u8( ega, address, data, system_ticks);
                                    }
                                    #[cfg(feature = "vga")]
                                    VideoCardDispatch::Vga(vga) => {
                                        MemoryMappedDevice::mmio_write_u8(vga, address, data, system_ticks);
                                    }
                                    _ => {}
                                }
                            }
                            _=> {}
                        }                        
                        return Ok(map_entry.0.cycle_cost);
                    }
                }
                return Ok(DEFAULT_WAIT_STATES);
                */
                
                match self.mmio_map_fast[address >> MMIO_MAP_SHIFT] {
                    MmioDeviceType::Video => {
                        
                        let system_ticks = self.cycles_to_ticks[cycles as usize];

                        match &mut self.video {
                            VideoCardDispatch::Cga(cga) => {
                                let syswait = cga.mmio_write_u8(address, data, system_ticks);
                                //return Ok(self.system_ticks_to_cpu_cycles(syswait)); // temporary wait state value. 
                                return Ok(0);
                            }
                            #[cfg(feature = "ega")]
                            VideoCardDispatch::Ega(ega) => {
                                MemoryMappedDevice::mmio_write_u8( ega, address, data, system_ticks);
                            }
                            #[cfg(feature = "vga")]
                            VideoCardDispatch::Vga(vga) => {
                                MemoryMappedDevice::mmio_write_u8(vga, address, data, system_ticks);
                            }
                            _ => {}
                        }
                    },
                    _ => {
                        if self.memory_mask[address] & MEM_ROM_BIT == 0 {
                            self.memory[address] = data;                
                        }
                    }
                }
                return Ok(DEFAULT_WAIT_STATES);
            }
        }
        Err(MemError::ReadOutOfBoundsError)
    }

    pub fn write_u16(&mut self, address: usize, data: u16, cycles: u32) -> Result<u32, MemError> {
        if address < self.memory.len() - 1 {
            if address < self.mmio_data.first_map || address > self.mmio_data.last_map {
                // Address is not mapped.

                // Little Endian is LO byte first
                if self.memory_mask[address] & MEM_ROM_BIT == 0 {
                    self.memory[address] = (data & 0xFF) as u8;
                    self.memory[address+1] = (data >> 8) as u8;              
                }
                return Ok(DEFAULT_WAIT_STATES);
            }
            else {
                // Handle memory-mapped devices
                for map_entry in &self.mmio_map {
                    if address >= map_entry.0.address && address < map_entry.0.address + map_entry.0.size - 1 {

                        // Convert cpu cycles to system ticks
                        let system_ticks = self.cpu_cycles_to_system_ticks(cycles);

                        match map_entry.1 {
                            MmioDeviceType::Cga | MmioDeviceType::Ega | MmioDeviceType::Vga => {
                                match &mut self.video {
                                    VideoCardDispatch::Cga(cga) => {
                                        let mut syswait;
                                        syswait = MemoryMappedDevice::mmio_write_u8(cga, address, (data & 0xFF) as u8, system_ticks);
                                        syswait += MemoryMappedDevice::mmio_write_u8(cga, address + 1, (data >> 8) as u8, 0);
                                        return Ok(self.system_ticks_to_cpu_cycles(syswait)); // temporary wait state value. 
                                    }
                                    #[cfg(feature = "ega")]
                                    VideoCardDispatch::Ega(ega) => {
                                        MemoryMappedDevice::mmio_write_u8(ega, address, (data & 0xFF) as u8, system_ticks);
                                        MemoryMappedDevice::mmio_write_u8(ega, address + 1, (data >> 8) as u8, 0);
                                    }
                                    #[cfg(feature = "vga")]
                                    VideoCardDispatch::Vga(vga) => {
                                        MemoryMappedDevice::mmio_write_u8(vga, address, (data & 0xFF) as u8, system_ticks);
                                        MemoryMappedDevice::mmio_write_u8(vga, address + 1, (data >> 8) as u8, 0);
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
                if self.memory_mask[address] & MEM_ROM_BIT == 0 {
                    self.memory[address] = (data & 0xFF) as u8;
                    self.memory[address+1] = (data >> 8) as u8;              
                }
                return Ok(DEFAULT_WAIT_STATES);
            }
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
    pub fn dump_flat_tokens(&self, address: usize, cursor: usize, mut size: usize) -> Vec<Vec<SyntaxToken>> {

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

                if (display_address + i) == cursor {
                    line_vec.push(
                        SyntaxToken::MemoryByteHexValue(
                            (display_address + i) as u32, 
                            *byte,
                            format!("{:02X}", *byte),
                            true, // Set cursor on this byte
                            0
                        )
                    );
                }
                else {
                    line_vec.push(
                        SyntaxToken::MemoryByteHexValue(
                            (display_address + i) as u32, 
                            *byte,
                            format!("{:02X}", *byte),
                            false,
                            0
                        )
                    );
                }
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

    /// Dump memory to a vector of vectors of SyntaxTokens.
    /// 
    /// Uses bus peek functions to resolve MMIO addresses.
    pub fn dump_flat_tokens_ex(&self, address: usize, cursor: usize, mut size: usize) -> Vec<Vec<SyntaxToken>> {

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

        let addr_vec = Vec::from_iter(address..address+size);
        let mut display_address = address;

        for dump_addr_row in addr_vec.chunks_exact(16) {

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
            for addr in dump_addr_row {

                let byte = self.peek_u8(*addr).unwrap();

                if (display_address + i) == cursor {
                    line_vec.push(
                        SyntaxToken::MemoryByteHexValue(
                            (display_address + i) as u32, 
                            byte,
                            format!("{:02X}", byte),
                            true, // Set cursor on this byte
                            0
                        )
                    );
                }
                else {
                    line_vec.push(
                        SyntaxToken::MemoryByteHexValue(
                            (display_address + i) as u32, 
                            byte,
                            format!("{:02X}", byte),
                            false,
                            0
                        )
                    );
                }
                i += 1;
            }

            // Build ASCII representation tokens
            let mut i = 0;
            for addr in dump_addr_row {

                let byte = self.peek_u8(*addr).unwrap();

                let char_str = match byte {
                    00..=31 => ".".to_string(),
                    32..=127 => format!("{}", byte as char),
                    128.. => ".".to_string()
                };
                line_vec.push(
                    SyntaxToken::MemoryByteAsciiValue(
                        (display_address + i) as u32,
                        byte,
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

    pub fn dump_mem(&self, path: &Path) {
        
        let mut filename = path.to_path_buf();
        filename.push("mem.bin");

        let len = 0x100000;
        let address = 0;
        log::debug!("Dumping {} bytes at address {:05X}", len, address);

        match std::fs::write(filename.clone(), &self.memory) {
            Ok(_) => {
                log::debug!("Wrote memory dump: {}", filename.display())
            }
            Err(e) => {
                log::error!("Failed to write memory dump '{}': {}", filename.display(), e)
            }
        }
    }

    pub fn dump_ivr_tokens(&mut self) -> Vec<Vec<SyntaxToken>> {

        let mut vec: Vec<Vec<SyntaxToken>> = Vec::new();

        for v in 0..256 {
            let mut ivr_vec = Vec::new();
            let (ip, _) = self.read_u16((v * 4) as usize, 0).unwrap();
            let (cs, _) = self.read_u16(((v*4) + 2) as usize, 0).unwrap();

            ivr_vec.push(SyntaxToken::Text(format!("{:03}", v)));
            ivr_vec.push(SyntaxToken::Colon);
            ivr_vec.push(SyntaxToken::MemoryAddressSeg16(cs, ip, format!("[{:04X}]:[{:04X}]", cs, ip)));
            vec.push(ivr_vec);
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
        clock_mode: ClockingMode,
        machine_desc: &MachineDescriptor, 
        video_trace: TraceLogger,
        video_frame_debug: bool,
    ) 
    {

        // Create PPI if PPI is defined for this machine type
        if machine_desc.have_ppi {
            self.ppi = Some(Ppi::new(machine_desc.machine_type, video_type, machine_desc.num_floppies));
            // Add PPI ports to io_map
            let port_list = self.ppi.as_mut().unwrap().port_list();
            self.io_map.extend(port_list.into_iter().map(|p| (p, IoDeviceType::Ppi)));
        }

        // Create the PIT. One PIT will always exist, but it may be an 8253 or 8254. 
        // Pick the device type from MachineDesc.
        // Provide the timer with its base crystal and divisor.
        let mut pit = 
            Pit::new(
                machine_desc.pit_type,
                if let Some(crystal) = machine_desc.timer_crystal {
                    crystal
                }
                else {
                    machine_desc.system_crystal
                },
                machine_desc.timer_divisor
            );

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
                let cga = CGACard::new(video_trace, clock_mode, video_frame_debug);
                let port_list = cga.port_list();
                self.io_map.extend(port_list.into_iter().map(|p| (p, IoDeviceType::Cga)));

                let mem_descriptor = MemRangeDescriptor::new(cga::CGA_MEM_ADDRESS, cga::CGA_MEM_APERTURE, false );
                self.register_map(MmioDeviceType::Video, mem_descriptor);

                self.video = VideoCardDispatch::Cga(cga)
            }
            #[cfg(feature = "ega")]
            VideoType::EGA => {
                let ega = EGACard::new(video_trace, clock_mode, video_frame_debug);
                let port_list = ega.port_list();
                self.io_map.extend(port_list.into_iter().map(|p| (p, IoDeviceType::Ega)));

                let cga_mem_descriptor = MemRangeDescriptor::new(cga::CGA_MEM_ADDRESS, cga::CGA_MEM_APERTURE, false );
                let ega_mem_descriptor = MemRangeDescriptor::new(ega::EGA_MEM_ADDRESS, ega::EGA_GFX_PLANE_SIZE, false );
                self.register_map(MmioDeviceType::Video, cga_mem_descriptor);
                self.register_map(MmioDeviceType::Video, ega_mem_descriptor);

                self.video = VideoCardDispatch::Ega(ega)
            }
            #[cfg(feature = "vga")]
            VideoType::VGA => {
                let vga = VGACard::new(video_trace);
                let port_list = vga.port_list();
                self.io_map.extend(port_list.into_iter().map(|p| (p, IoDeviceType::Vga)));

                //let mem_descriptor = MemRangeDescriptor::new(0xB8000, vga::VGA_TEXT_PLANE_SIZE, false );
                //cpu.bus_mut().register_map(IoDeviceType::Vga, mem_descriptor);

                let mem_descriptor = MemRangeDescriptor::new(vga::VGA_GFX_ADDRESS, vga::VGA_GFX_PLANE_SIZE, false );
                self.register_map(MmioDeviceType::Video, mem_descriptor);

                self.video = VideoCardDispatch::Vga(vga)
            }
            _=> {
                // MDA not implemented
                todo!("MDA not implemented");
            }
        }
    
        self.machine_desc = Some(machine_desc.clone());
    }

    /// Return whether NMI is enabled.
    /// On the 5150 & 5160, NMI generation can be disabled via the PPI.
    pub fn nmi_enabled(&self) -> bool {

        if self.machine_desc.unwrap().have_ppi {

            if let Some(ppi) = &self.ppi {
                ppi.nmi_enabled()
            }
            else {
                true
            }
        }
        else {
            // TODO: Determine what controls NMI masking on AT (i8042?)
            true
        }
    }

    // Schedule extra ticks for the PIT.
    pub fn adjust_pit(&mut self, ticks: u32) {
        log::debug!("Scheduling {} extra system ticks for PIT", ticks);
        self.pit_ticks_advance += ticks;
    }

    pub fn run_devices(
        &mut self, 
        us: f64, 
        sys_ticks: u32, 
        kb_event_opt: Option<KeybufferEntry>, 
        kb_buf: &mut VecDeque<KeybufferEntry>,
        speaker_buf_producer: &mut Producer<u8>
    ) -> Option<DeviceEvent>
        
    {

        let mut event = None;

        // Send keyboard events to devices.
        if let Some(kb_event) = kb_event_opt {
            //log::debug!("Got keyboard byte: {:02X}", kb_byte);

            match kb_event.pressed {
                true => self.keyboard.key_down(kb_event.keycode, &kb_event.modifiers, Some(kb_buf)),
                false => self.keyboard.key_up(kb_event.keycode)
            }
            
            // Read a byte from the keyboard
            if let Some(kb_byte) = self.keyboard.recv_scancode() {

                // Do we have a PPI? if so, send the scancode to the PPI
                if let Some(ppi) = &mut self.ppi {

                    ppi.send_keyboard(kb_byte);
    
                    if ppi.kb_enabled() {
                        if let Some(pic) = &mut self.pic1 {
                            // TODO: Should we let the PPI do this directly?
                            //log::warn!("sending kb interrupt for byte: {:02X}", kb_byte);
                            pic.pulse_interrupt(1);
                        }   
                    }
                }
            }
        }

        // Accumulate us and run the keyboard when scheduled.
        self.kb_us_accum += us;
        if self.kb_us_accum > KB_UPDATE_RATE {
            self.keyboard.run(KB_UPDATE_RATE);
            self.kb_us_accum -= KB_UPDATE_RATE;

            // Read a byte from the keyboard
            if let Some(kb_byte) = self.keyboard.recv_scancode() {

                // Do we have a PPI? if so, send the scancode to the PPI
                if let Some(ppi) = &mut self.ppi {

                    ppi.send_keyboard(kb_byte);
    
                    if ppi.kb_enabled() {
                        if let Some(pic) = &mut self.pic1 {
                            // TODO: Should we let the PPI do this directly?
                            //log::warn!("sending kb interrupt for byte: {:02X}", kb_byte);
                            pic.pulse_interrupt(1);
                        }   
                    }
                }
            }            
        }


        // There will always be a PIC, so safe to unwrap.
        let pic = self.pic1.as_mut().unwrap();

        pic.run(sys_ticks);

        // There will always be a PIT, so safe to unwrap.
        let mut pit = self.pit.take().unwrap();

        // Run the PPI if present. PPI takes PIC to generate keyboard interrupts.
        if let Some(ppi) = &mut self.ppi {
            ppi.run(pic, us);
        }

        // Run the PIT. The PIT communicates with lots of things, so we send it the entire bus.
        // The PIT may have a separate clock crystal, such as in the IBM AT. In this case, there may not 
        // be an integer number of PIT ticks per system ticks. Therefore the PIT can take either
        // system ticks (PC/XT) or microseconds as an update parameter.
        if let Some(_crystal) = self.machine_desc.unwrap().timer_crystal {
            pit.run(self, speaker_buf_producer, DeviceRunTimeUnit::Microseconds(us));
        }
        else {
            // We can only adjust phase of PIT if we are using system ticks, and that's okay. It's only really useful
            // on an 5150/5160. 
            pit.run(self, speaker_buf_producer, DeviceRunTimeUnit::SystemTicks(sys_ticks + self.pit_ticks_advance));
            self.pit_ticks_advance = 0;
        }

        // Has PIT channel 1 (DMA timer) changed?
        let (pit_dirty, pit_counting, pit_ticked) = pit.is_dirty(1);

        if pit_dirty {
            log::debug!("Pit is dirty! counting: {} ticked: {}", pit_counting, pit_ticked);
        }

        if pit_counting && pit_dirty {
            // Pit is dirty and counting. Update the the DMA scheduler.

            let (dma_count_register, dma_counting_element) = pit.get_channel_count(1);

            // Get the timer accumulator to provide tick offset to DMA scheduler.
            // The timer ticks every 12 system ticks by default on PC/XT; if 11 ticks are stored in the accumulator,
            // this represents two CPU cycles, so we need to adjust the scheduler by that much.
            let dma_add_ticks = pit.get_timer_accum();

            log::debug!("pit dirty and counting! count register: {} counting element: {} ", dma_count_register, dma_counting_element);
            
            if (dma_counting_element <= dma_count_register) {
                // DRAM refresh DMA counter has changed. If the counting element is in range,
                // update the CPU's DRAM refresh simulation.
                log::debug!("DRAM refresh DMA counter updated: {}, {}, +{}", dma_count_register, dma_counting_element, dma_add_ticks);
                self.dma_counter = dma_count_register;
    
                // Invert the dma counter value as Cpu counts up toward total

                if dma_counting_element == 0 && !pit_ticked {
                    // Counter is still at initial 0 - not a terminal count.
                    event = Some(DeviceEvent::DramRefreshUpdate(dma_count_register, 0, 0));
                }
                else {
                    // Timer is at terminal count!
                    event = Some(DeviceEvent::DramRefreshUpdate(dma_count_register, dma_counting_element, dma_add_ticks));
                }
                self.refresh_active = true;
            }
        }
        else if !pit_counting && self.refresh_active {
            // Timer 1 isn't counting anymore! Disable DRAM refresh...
            log::debug!("Channel 1 not counting. Disabling DRAM refresh...");
            event = Some(DeviceEvent::DramRefreshEnable(false));
            self.refresh_active = false;
        }
        
        // Save current count info.
        let (pit_reload_value, pit_counting_element) = pit.get_channel_count(0);

        // Do hack for Area5150 :(
        if pit_reload_value == 5117 {
            if !self.timer_trigger1_armed {
                self.timer_trigger1_armed = true;
                log::warn!("Area5150 hack armed for lake effect.");
            }
        } 
        else if pit_reload_value == 5162 {
            if !self.timer_trigger2_armed {
                self.timer_trigger2_armed = true;
                log::warn!("Area5150 hack armed for wibble effect.");
            }
        }
        
        /*
        if pit_reload_value == 19912 && (self.timer_trigger1_armed || self.timer_trigger2_armed) {
            self.timer_trigger1_armed = false;
            self.timer_trigger2_armed = false;
        }
        */

        // Put the PIT back.
        self.pit = Some(pit);
        
        let mut dma1 = self.dma1.take().unwrap();

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
        
        // Run the DMA controller.
        dma1.run(self);

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

                self.cga_tick_accum += sys_ticks;
                
                if self.cga_tick_accum > 8 {
                    cga.run(DeviceRunTimeUnit::SystemTicks(self.cga_tick_accum));
                    self.cga_tick_accum = 0;

                    if self.timer_trigger1_armed && pit_reload_value == 19912 {
                        // Do hack for Area5150. TODO: Figure out why this is necessary.
                        
                        // With VerticalTotalAdjust == 0, ticks per frame are 233472.
                        let screen_tick_pos = cga.get_screen_ticks();
    
                        //let screen_target = 17256
                        //let screen_target = 16344;
                        let screen_target = 15432 + 40;
                        // Only adjust if we are late
                        if screen_tick_pos > screen_target {
                            let ticks_adj = screen_tick_pos - screen_target;
                            log::warn!(
                                "Doing Area5150 hack. Target: {} Pos: {} Rewinding CGA by {} ticks. (Timer: {})", 
                                screen_target, 
                                screen_tick_pos, 
                                ticks_adj,
                                pit_counting_element
                            );
                            
                            //cga.debug_tick(233472 - ticks_adj as u32);
                            
                            //cga.run(DeviceRunTimeUnit::SystemTicks(233472 - ticks_adj as u32));
                        }
                        
                        self.timer_trigger1_armed = false;
                    }
                    else if self.timer_trigger2_armed && pit_reload_value == 19912 {
                        // Do hack for Area5150. TODO: Figure out why this is necessary.
                        
                        // With VerticalTotalAdjust == 0, ticks per frame are 233472.
                        let screen_tick_pos = cga.get_screen_ticks();
    
                        //let screen_target = 17256;
                        let screen_target = 16344 + 40;
                        // Only adjust if we are late
                        if screen_tick_pos > screen_target {
                            let ticks_adj = screen_tick_pos - screen_target;
                            log::warn!(
                                "Doing Area5150 hack. Target: {} Pos: {} Rewinding CGA by {} ticks. (Timer: {})", 
                                screen_target, 
                                screen_tick_pos, 
                                ticks_adj,
                                pit_counting_element
                            );
                            
                            //cga.debug_tick(233472 - ticks_adj as u32);
                            
                            //cga.run(DeviceRunTimeUnit::SystemTicks(233472 - ticks_adj as u32));
                        }
                        
                        self.timer_trigger2_armed = false;
                    }         
                }
            },
            #[cfg(feature = "ega")]
            VideoCardDispatch::Ega(ega) => {
                ega.run(DeviceRunTimeUnit::Microseconds(us));
            }
            #[cfg(feature = "vga")]
            VideoCardDispatch::Vga(vga) => {
                vga.run(DeviceRunTimeUnit::Microseconds(us));
            }
            VideoCardDispatch::None => {}
        }

        event
    }

    /// Call the reset methods for all devices on the bus
    pub fn reset_devices(&mut self) {

        // Reset PIT
        if let Some(pit) = self.pit.as_mut() {
            pit.reset();
        }
    
        // Reset PIC
        if let Some(pic1) = self.pic1.as_mut() {
            pic1.reset();
        }

        // Reset DMA
        if let Some(dma1) = self.dma1.as_mut() {
            dma1.reset();
        }

        // Reset video card
        if let Some(video) = self.video_mut() {
            video.reset();
        }
    }

    /// Call the reset methods for devices to be reset on warm boot
    pub fn reset_devices_warm(&mut self) {
        self.pit.as_mut().unwrap().reset();
        //self.pic1.as_mut().unwrap().reset();
    }    

    /// Read an 8-bit value from an IO port.
    /// 
    /// We provide the elapsed cycle count for the current instruction. This allows a device
    /// to optionally tick itself to bring itself in sync with CPU state.
    pub fn io_read_u8(&mut self, port: u16, cycles: u32) -> u8 {
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

        // Convert cycles to system clock ticks
        let sys_ticks = match self.cpu_factor {
            ClockFactor::Divisor(d) => {
                d as u32 * cycles
            }
            ClockFactor::Multiplier(m) => {
                cycles / m as u32
            }
        };
        let nul_delta = DeviceRunTimeUnit::Microseconds(0.0);

        if let Some(device_id) = self.io_map.get(&port) {
            match device_id {
                IoDeviceType::Ppi => {
                    if let Some(ppi) = &mut self.ppi {
                        ppi.read_u8(port, nul_delta)
                    }
                    else {
                        NO_IO_BYTE
                    }
                }
                IoDeviceType::Pit => {
                    // There will always be a PIT, so safe to unwrap
                    self.pit.as_mut().unwrap().read_u8(port, DeviceRunTimeUnit::SystemTicks(sys_ticks))
                    //self.pit.as_mut().unwrap().read_u8(port, nul_delta)
                }
                IoDeviceType::DmaPrimary => {
                    // There will always be a primary DMA, so safe to unwrap                    
                    self.dma1.as_mut().unwrap().read_u8(port, nul_delta)
                }
                IoDeviceType::DmaSecondary => {
                    // Secondary DMA may not exist
                    if let Some(dma2) = &mut self.dma2 {
                        dma2.read_u8(port, nul_delta)
                    }
                    else {
                        NO_IO_BYTE
                    }
                }
                IoDeviceType::PicPrimary => {
                    // There will always be a primary PIC, so safe to unwrap
                    self.pic1.as_mut().unwrap().read_u8(port, nul_delta)
                }
                IoDeviceType::PicSecondary => {
                    // Secondary PIC may not exist
                    if let Some(pic2) = &mut self.pic2 {
                        pic2.read_u8(port, nul_delta)
                    }
                    else {
                        NO_IO_BYTE
                    }
                }
                IoDeviceType::FloppyController => {
                    if let Some(fdc) = &mut self.fdc {
                        fdc.read_u8(port, nul_delta)
                    }                     
                    else {
                        NO_IO_BYTE
                    }      
                }
                IoDeviceType::HardDiskController => {
                    if let Some(hdc) = &mut self.hdc {
                        hdc.read_u8(port, nul_delta)
                    }
                    else {
                        NO_IO_BYTE
                    }        
                }
                IoDeviceType::Serial => {
                    if let Some(serial) = &mut self.serial {
                        // Serial port write does not need bus.
                        serial.read_u8(port, nul_delta)
                    } 
                    else {
                        NO_IO_BYTE
                    }
                }
                       
                IoDeviceType::Cga | IoDeviceType::Ega | IoDeviceType::Vga => {
                    match &mut self.video {
                        VideoCardDispatch::Cga(cga) => {
                            IoDevice::read_u8(cga, port, DeviceRunTimeUnit::SystemTicks(sys_ticks))
                        },
                        #[cfg(feature = "ega")]
                        VideoCardDispatch::Ega(ega) => {
                            IoDevice::read_u8(ega, port, nul_delta)
                        }
                        #[cfg(feature = "vga")]
                        VideoCardDispatch::Vga(vga) => {
                            IoDevice::read_u8(vga, port, nul_delta)
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

    /// Write an 8-bit value to an IO port.
    /// 
    /// We provide the elapsed cycle count for the current instruction. This allows a device
    /// to optionally tick itself to bring itself in sync with CPU state.
    pub fn io_write_u8(&mut self, port: u16, data: u8, cycles: u32) {
        /*
        let handler_opt = self.handlers.get_mut(&port);
        if let Some(handler) = handler_opt {
            // We found a IoHandler in hashmap
            let mut writeable_thing = handler.device.borrow_mut();
            let func_ptr = handler.write_u8;
            func_ptr(&mut *writeable_thing, port, data);
        }
        */

        // Convert cycles to system clock ticks
        let sys_ticks = match self.cpu_factor {
            ClockFactor::Divisor(n) => cycles * (n as u32),
            ClockFactor::Multiplier(n) => cycles / (n as u32)
        };

        let nul_delta = DeviceRunTimeUnit::Microseconds(0.0);

        if let Some(device_id) = self.io_map.get(&port) {
            match device_id {
                IoDeviceType::Ppi => {
                    if let Some(mut ppi) = self.ppi.take() {
                        ppi.write_u8(port, data, Some(self), nul_delta);
                        self.ppi = Some(ppi);
                    }
                }
                IoDeviceType::Pit => {
                    if let Some(mut pit) = self.pit.take() {
                        //log::debug!("writing PIT with {} cycles", cycles);
                        pit.write_u8(port, data, Some(self), DeviceRunTimeUnit::SystemTicks(sys_ticks));
                        self.pit = Some(pit);
                    }
                }
                IoDeviceType::DmaPrimary => {
                    if let Some(mut dma1) = self.dma1.take() {
                        dma1.write_u8(port, data, Some(self), nul_delta);
                        self.dma1 = Some(dma1);
                    }
                }
                IoDeviceType::DmaSecondary => {
                    if let Some(mut dma2) = self.dma2.take() {
                        dma2.write_u8(port, data, Some(self), nul_delta);
                        self.dma2 = Some(dma2);
                    }                    
                }
                IoDeviceType::PicPrimary => {
                    if let Some(mut pic1) = self.pic1.take() {
                        pic1.write_u8(port, data, Some(self), nul_delta);
                        self.pic1 = Some(pic1);
                    }                    
                }
                IoDeviceType::PicSecondary => {
                    if let Some(mut pic2) = self.pic2.take() {
                        pic2.write_u8(port, data, Some(self), nul_delta);
                        self.pic2 = Some(pic2);
                    }                               
                }
                IoDeviceType::FloppyController => {
                    if let Some(mut fdc) = self.fdc.take() {
                        fdc.write_u8(port, data, Some(self), nul_delta);
                        self.fdc = Some(fdc);
                    }                           
                }
                IoDeviceType::HardDiskController => {
                    if let Some(mut hdc) = self.hdc.take() {
                        hdc.write_u8(port, data, Some(self), nul_delta);
                        self.hdc = Some(hdc);
                    }                            
                }
                IoDeviceType::Serial => {
                    if let Some(serial) = &mut self.serial {
                        // Serial port write does not need bus.
                        serial.write_u8(port, data, None, nul_delta);
                    }
                }
                IoDeviceType::Cga | IoDeviceType::Ega | IoDeviceType::Vga => {
                    match &mut self.video {
                        VideoCardDispatch::Cga(cga) => {
                            IoDevice::write_u8(cga, port, data, None, DeviceRunTimeUnit::SystemTicks(sys_ticks))
                        },
                        #[cfg(feature = "ega")]
                        VideoCardDispatch::Ega(ega) => {
                            IoDevice::write_u8(ega, port, data, None, nul_delta)
                        }
                        #[cfg(feature = "vga")]
                        VideoCardDispatch::Vga(vga) => {
                            IoDevice::write_u8(vga, port, data, None, nul_delta)
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
            #[cfg(feature = "ega")]
            VideoCardDispatch::Ega(ega) => {
                Some(Box::new(ega as &dyn VideoCard))
            }
            #[cfg(feature = "vga")]
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
            #[cfg(feature = "ega")]
            VideoCardDispatch::Ega(ega) => {
                Some(Box::new(ega as &mut dyn VideoCard))
            }
            #[cfg(feature = "vga")]
            VideoCardDispatch::Vga(vga) => {
                Some(Box::new(vga as &mut dyn VideoCard))
            }
            VideoCardDispatch::None => {
                None
            }
        }
    }

    pub fn keyboard_mut(&mut self) -> &mut Keyboard {
        &mut self.keyboard
    }


}